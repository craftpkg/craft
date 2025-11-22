use anyhow::Result;
use std::future::Future;

pub trait Pipeline<T> {
    fn run(&self) -> impl Future<Output = Result<T>> + Send;
}
