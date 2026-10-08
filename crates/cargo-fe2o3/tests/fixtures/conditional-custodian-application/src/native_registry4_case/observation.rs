//! Only joins observer futures; this helper owns no native resource or receipt.
use std::{future::Future, pin::Pin, task::Poll};

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Driver(E),
    Observer { member: usize, error: E },
    DriverClosedBeforeObservation,
}

/// Stop borrowing the existing driver only after all four copied-result
/// observers resolve. The caller must still resume the owning scope to destroy
/// its common native backing. No completion order is inferred from poll order.
pub async fn all_copied_before_close<D, O, E>(driver: D, observers: [O; 4]) -> Result<(), Error<E>>
where
    D: Future<Output = Result<(), E>>,
    O: Future<Output = Result<(), E>> + Unpin,
{
    all_copied_before_close_n(driver, observers).await
}

/// Same bounded observation join for the explicit caller's original roster.
pub async fn all_copied_before_close_n<D, O, E, const N: usize>(
    driver: D,
    mut observers: [O; N],
) -> Result<(), Error<E>>
where
    D: Future<Output = Result<(), E>>,
    O: Future<Output = Result<(), E>> + Unpin,
{
    let mut driver = std::pin::pin!(driver);
    let mut ready = [false; N];
    std::future::poll_fn(|cx| {
        // The shared driver's cooperative boundary separates its copied-result
        // scan from the subsequent common-destruction scan.
        if let Poll::Ready(result) = driver.as_mut().poll(cx) {
            return Poll::Ready(match result {
                Err(error) => Err(Error::Driver(error)),
                Ok(()) => Err(Error::DriverClosedBeforeObservation),
            });
        }
        for member in 0..N {
            if ready[member] {
                continue;
            }
            match Pin::new(&mut observers[member]).poll(cx) {
                Poll::Ready(Ok(())) => ready[member] = true,
                Poll::Ready(Err(error)) => {
                    return Poll::Ready(Err(Error::Observer { member, error }));
                }
                Poll::Pending => {}
            }
        }
        if ready.into_iter().all(|ready| ready) {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        future::poll_fn,
        rc::Rc,
        task::{Context, Waker},
    };

    #[test]
    fn all_sixteen_original_observers_are_required_before_return() {
        let scans = Cell::new(0);
        let driver = poll_fn(|_| {
            scans.set(scans.get() + 1);
            Poll::Pending::<Result<(), ()>>
        });
        let polls = core::array::from_fn::<_, 16, _>(|_| Cell::new(0));
        let observers = core::array::from_fn::<_, 16, _>(|member| {
            let scans = &scans;
            let count = &polls[member];
            poll_fn(move |_| {
                count.set(count.get() + 1);
                if scans.get() > member {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        });
        let mut joined = std::pin::pin!(all_copied_before_close_n(driver, observers));
        let mut cx = Context::from_waker(Waker::noop());
        for expected in 1..16 {
            assert!(joined.as_mut().poll(&mut cx).is_pending());
            assert_eq!(scans.get(), expected);
        }
        assert_eq!(joined.as_mut().poll(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(
            polls.each_ref().map(|count| count.get()),
            core::array::from_fn(|i| i + 1)
        );
    }

    #[test]
    fn all_four_are_required_and_finished_observers_are_not_repolled() {
        let scans = Rc::new(Cell::new(0));
        let driver = poll_fn(|_| {
            scans.set(scans.get() + 1);
            Poll::Pending::<Result<(), ()>>
        });
        let polls = core::array::from_fn::<_, 4, _>(|_| Cell::new(0));
        let observers = core::array::from_fn(|member| {
            let scans = &scans;
            let count = &polls[member];
            poll_fn(move |_| {
                count.set(count.get() + 1);
                if scans.get() > member {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        });
        let mut joined = std::pin::pin!(all_copied_before_close(driver, observers));
        let mut cx = Context::from_waker(Waker::noop());
        for scan in 1..4 {
            assert!(joined.as_mut().poll(&mut cx).is_pending());
            assert_eq!(scans.get(), scan);
        }
        assert_eq!(joined.as_mut().poll(&mut cx), Poll::Ready(Ok(())));
        assert_eq!(scans.get(), 4);
        assert_eq!(polls.each_ref().map(|count| count.get()), [1, 2, 3, 4]);
    }

    #[test]
    fn closed_driver_is_not_an_early_result_observation() {
        let mut joined = std::pin::pin!(all_copied_before_close(
            std::future::ready(Ok::<_, ()>(())),
            core::array::from_fn(|_| std::future::ready(Ok(()))),
        ));
        assert_eq!(
            joined
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(Err(Error::DriverClosedBeforeObservation)),
        );
    }

    #[test]
    fn driver_and_exact_member_errors_are_preserved() {
        let mut driver = std::pin::pin!(all_copied_before_close(
            std::future::ready(Err::<(), _>(7)),
            core::array::from_fn(|_| std::future::ready(Ok(()))),
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(
            driver.as_mut().poll(&mut cx),
            Poll::Ready(Err(Error::Driver(7)))
        );
        let mut observer = std::pin::pin!(all_copied_before_close(
            std::future::pending(),
            core::array::from_fn(|member| std::future::ready(if member == 3 {
                Err(9)
            } else {
                Ok(())
            })),
        ));
        assert_eq!(
            observer.as_mut().poll(&mut cx),
            Poll::Ready(Err(Error::Observer {
                member: 3,
                error: 9
            })),
        );
    }
}
