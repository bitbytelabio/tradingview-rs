import sys
from typing import Any, Callable

def callback_trampoline(cb: Callable[[Any], None], item: Any) -> None:
    """Invokes a callback with exception isolation via sys.unraisablehook."""
    try:
        cb(item)
    except Exception as exc:
        if hasattr(sys, "unraisablehook"):
            unraisable_args_cls = getattr(sys, "UnraisableHookArgs", None)
            if unraisable_args_cls is not None:
                sys.unraisablehook(
                    unraisable_args_cls(exc, "Exception in TradingView streaming callback", cb)
                )
            else:
                sys.unraisablehook(exc)  # type: ignore[arg-type]
