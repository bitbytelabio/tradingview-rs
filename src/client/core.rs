use std::sync::Arc;

pub trait DataClient {
    fn new(auth_token: Option<&str>) -> Arc<Self>;
}
