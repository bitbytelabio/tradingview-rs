use parking_lot::RwLock;
use pyo3::BoundObject;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use std::sync::Arc;

use crate::PyObject;
#[derive(Default, Clone)]
/// Manages Python synchronous callbacks and thread-safe dispatch onto the asyncio event loop.
pub struct CallbackDispatcher {
    event_loop: Arc<RwLock<Option<PyObject>>>,
    callbacks: Arc<RwLock<Vec<PyObject>>>,
    trampoline: Arc<RwLock<Option<PyObject>>>,
}

impl CallbackDispatcher {
    pub fn new() -> Self {
        Self {
            event_loop: Arc::new(RwLock::new(None)),
            callbacks: Arc::new(RwLock::new(Vec::new())),
            trampoline: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize event loop and trampoline from the current Python context.
    pub fn init_loop(&self, py: Python<'_>) -> PyResult<()> {
        let mut loop_guard = self.event_loop.write();
        if loop_guard.is_none() {
            let asyncio = py.import("asyncio")?;
            let current_loop = match asyncio.getattr("get_running_loop")?.call0() {
                Ok(l) => l.unbind(),
                Err(_) => asyncio.getattr("get_event_loop")?.call0()?.unbind(),
            };
            *loop_guard = Some(current_loop);
        }

        let mut tramp_guard = self.trampoline.write();
        if tramp_guard.is_none() {
            let trampoline = py
                .import("tradingview._trampoline")?
                .getattr("callback_trampoline")?
                .unbind();
            *tramp_guard = Some(trampoline);
        }

        Ok(())
    }

    /// Register a synchronous callable. Rejects coroutine functions (`async def`) with TypeError.
    pub fn add_callback(&self, py: Python<'_>, callback: PyObject) -> PyResult<()> {
        let asyncio = py.import("asyncio")?;
        let is_coro_fn: bool = asyncio
            .getattr("iscoroutinefunction")?
            .call1((&callback,))?
            .extract()?;
        if is_coro_fn {
            return Err(PyTypeError::new_err(
                "Callback must be a synchronous callable, not a coroutine function. Schedule async work using asyncio.create_task() inside the callback.",
            ));
        }

        self.init_loop(py)?;
        self.callbacks.write().push(callback);
        Ok(())
    }

    /// Dispatch an item to all registered callbacks on the Python event loop thread.
    pub fn dispatch<T>(&self, item: T)
    where
        T: for<'py> IntoPyObject<'py> + Send + 'static,
    {
        if self.callbacks.read().is_empty() {
            return;
        }

        Python::attach(|py| {
            let callbacks: Vec<PyObject> = self
                .callbacks
                .read()
                .iter()
                .map(|cb| cb.clone_ref(py))
                .collect();
            let loop_obj = self.event_loop.read().as_ref().map(|l| l.clone_ref(py));
            let tramp_obj = self.trampoline.read().as_ref().map(|t| t.clone_ref(py));

            if let (Some(py_loop), Some(trampoline)) = (loop_obj, tramp_obj) {
                let py_loop_bound = py_loop.bind(py);

                if let Ok(py_item) = item.into_pyobject(py) {
                    let item_obj: PyObject = py_item.into_any().unbind();
                    for cb in callbacks {
                        let _ = py_loop_bound.call_method1(
                            "call_soon_threadsafe",
                            (trampoline.bind(py), cb.bind(py), item_obj.bind(py)),
                        );
                    }
                }
            }
        });
    }
}
