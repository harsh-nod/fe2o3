use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_POLICY: BusyRetryPolicy = BusyRetryPolicy {
    timeout: Duration::from_secs(5),
    poll_interval: Duration::from_millis(1),
};

#[derive(Clone, Copy)]
struct BusyRetryPolicy {
    timeout: Duration,
    poll_interval: Duration,
}

pub(crate) fn retry_transient_busy<T, E>(
    operation: impl FnMut() -> Result<T, E>,
    is_busy: impl Fn(&E) -> bool,
) -> Result<T, E> {
    retry_transient_busy_with_policy(operation, is_busy, DEFAULT_POLICY)
}

fn retry_transient_busy_with_policy<T, E>(
    mut operation: impl FnMut() -> Result<T, E>,
    is_busy: impl Fn(&E) -> bool,
    policy: BusyRetryPolicy,
) -> Result<T, E> {
    let deadline = Instant::now() + policy.timeout;
    loop {
        match operation() {
            Err(error) if is_busy(&error) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(error);
                }
                thread::sleep(policy.poll_interval.min(deadline.duration_since(now)));
                if Instant::now() >= deadline {
                    return Err(error);
                }
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestError {
        Busy,
        Fatal,
    }

    #[test]
    fn retries_only_busy_until_success() {
        let mut calls = 0;
        let result = retry_transient_busy_with_policy(
            || {
                calls += 1;
                if calls < 3 {
                    Err(TestError::Busy)
                } else {
                    Ok(17)
                }
            },
            |error| *error == TestError::Busy,
            BusyRetryPolicy {
                timeout: Duration::from_millis(10),
                poll_interval: Duration::ZERO,
            },
        );
        assert_eq!(result, Ok(17));
        assert_eq!(calls, 3);
    }

    #[test]
    fn returns_non_busy_without_retrying() {
        let mut calls = 0;
        let result = retry_transient_busy_with_policy(
            || {
                calls += 1;
                Err::<(), _>(TestError::Fatal)
            },
            |error| *error == TestError::Busy,
            BusyRetryPolicy {
                timeout: Duration::from_millis(10),
                poll_interval: Duration::ZERO,
            },
        );
        assert_eq!(result, Err(TestError::Fatal));
        assert_eq!(calls, 1);
    }

    #[test]
    fn preserves_non_busy_error_after_busy_retries() {
        let mut calls = 0;
        let result: Result<(), TestError> = retry_transient_busy_with_policy(
            || {
                calls += 1;
                if calls < 3 {
                    Err(TestError::Busy)
                } else {
                    Err(TestError::Fatal)
                }
            },
            |error| *error == TestError::Busy,
            BusyRetryPolicy {
                timeout: Duration::from_millis(10),
                poll_interval: Duration::ZERO,
            },
        );
        assert_eq!(result, Err(TestError::Fatal));
        assert_eq!(calls, 3);
    }

    #[test]
    fn perpetual_busy_is_bounded_and_preserved() {
        let started = Instant::now();
        let mut calls = 0;
        let result = retry_transient_busy_with_policy(
            || {
                calls += 1;
                Err::<(), _>(TestError::Busy)
            },
            |error| *error == TestError::Busy,
            BusyRetryPolicy {
                timeout: Duration::from_millis(3),
                poll_interval: Duration::from_millis(1),
            },
        );
        assert_eq!(result, Err(TestError::Busy));
        assert!(calls >= 2);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
