use crate::{Context, Result};
use std::future::Future;

pub trait Actor<T> {
    fn with(context: Context, payload: T) -> Self;
    fn run(&self) -> impl Future<Output = Result<()>> + Send;
}
