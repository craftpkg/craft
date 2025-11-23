use crate::Result;
use std::future::Future;

pub trait Actor<T> {
    fn with(t: T) -> Self;
    fn run(&self) -> impl Future<Output = Result<()>> + Send;
}
