//! Borrows the existing driver; no native receipt, timer or scheduler is added.
use std::{future::Future, pin::Pin, task::Poll};

pub async fn all_copied<D, O, E>(driver: D, observers: &mut [Option<O>]) -> Result<(), String>
where
    D: Future<Output = Result<(), E>>,
    O: Future<Output = Result<(), E>> + Unpin,
    E: std::fmt::Debug,
{
    if observers.len() != super::MEMBERS || observers.iter().any(Option::is_none) {
        return Err("independent arena observer roster is incomplete".into());
    }
    let mut driver = std::pin::pin!(driver);
    std::future::poll_fn(|cx| {
        if let Poll::Ready(result) = driver.as_mut().poll(cx) {
            return Poll::Ready(Err(format!(
                "driver closed before all result observation: {result:?}"
            )));
        }
        for (index, slot) in observers.iter_mut().enumerate() {
            let Some(observer) = slot else {
                continue;
            };
            match Pin::new(observer).poll(cx) {
                Poll::Ready(Ok(())) => {
                    *slot = None;
                }
                Poll::Ready(Err(error)) => {
                    return Poll::Ready(Err(format!("member {index}: {error:?}")));
                }
                Poll::Pending => {}
            }
        }
        if observers.iter().all(Option::is_none) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}
