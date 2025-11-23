use crate::Result;
use std::fmt::Display;
use std::future::Future;

pub trait Actor<T: Display> {
    fn with(t: T) -> Self;
    fn run(&self) -> impl Future<Output = Result<()>> + Send;
}
