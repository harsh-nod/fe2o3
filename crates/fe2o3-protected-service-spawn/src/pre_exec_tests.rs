use super::*;
use rustix::io::Errno;

#[test]
fn gate_interruptions_exhaust_exactly_the_prepaid_attempts() {
    assert_eq!(MAX_CHILD_GATE_ATTEMPTS_V2, 64);
    let mut attempts = 0;
    let result = read_child_gate(|_| {
        attempts += 1;
        Err(Errno::INTR)
    });
    assert_eq!(result, Err(()));
    assert_eq!(attempts, MAX_CHILD_GATE_ATTEMPTS_V2);
}

#[test]
fn gate_release_can_arrive_on_any_prepaid_attempt_including_the_last() {
    for release_attempt in 1..=MAX_CHILD_GATE_ATTEMPTS_V2 {
        let mut attempts = 0;
        let result = read_child_gate(|release| {
            attempts += 1;
            if attempts == release_attempt {
                *release = crate::PROTECTED_SERVICE_GATE_RELEASE_V1;
                Ok(1)
            } else {
                Err(Errno::INTR)
            }
        });
        assert_eq!(result, Ok(crate::PROTECTED_SERVICE_GATE_RELEASE_V1));
        assert_eq!(attempts, release_attempt);
    }
}

#[test]
fn gate_eof_invalid_length_or_permanent_failure_never_retries() {
    for read_result in [Ok(0), Ok(2), Err(Errno::AGAIN), Err(Errno::BADF)] {
        let mut attempts = 0;
        let result = read_child_gate(|_| {
            attempts += 1;
            read_result
        });
        assert_eq!(result, Err(()));
        assert_eq!(attempts, 1);
    }
}
