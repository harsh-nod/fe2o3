//! Pure per-segment version guards, not execution or content authority.
//!
//! The separately authenticated Verus predicates are related by review. These
//! guards do not prove ledger partitioning, whole-set transactionality, epoch
//! projection, native observations, or terminal-report construction.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R65GraphVersionStateV1 {
    /// Existing bytes under the backend input contract, not initialization evidence.
    AvailableAtAdmission,
    Planned,
    InFlight,
    Committed,
    NotProduced,
    Failed,
}

pub const fn r65_version_input_ready_v1(
    current: Option<usize>,
    expected: usize,
    phase: R65GraphVersionStateV1,
) -> bool {
    matches!(current, Some(actual) if actual == expected)
        && matches!(
            phase,
            R65GraphVersionStateV1::AvailableAtAdmission | R65GraphVersionStateV1::Committed
        )
}

pub const fn r65_version_begin_write_v1(
    phase: R65GraphVersionStateV1,
    pending: Option<usize>,
    current: Option<usize>,
    predecessor: usize,
) -> bool {
    matches!(phase, R65GraphVersionStateV1::Planned)
        && pending.is_none()
        && matches!(current, Some(actual) if actual == predecessor)
}

pub const fn r65_version_commit_write_v1(
    phase: R65GraphVersionStateV1,
    pending: Option<usize>,
    output: usize,
    current: Option<usize>,
) -> bool {
    matches!(phase, R65GraphVersionStateV1::InFlight)
        && matches!(pending, Some(actual) if actual == output)
        && current.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use R65GraphVersionStateV1 as P;

    #[test]
    fn r65_version_guards_exhaust_phase_and_index_cases() {
        let phases = [
            P::AvailableAtAdmission,
            P::Planned,
            P::InFlight,
            P::Committed,
            P::NotProduced,
            P::Failed,
        ];
        let indices = [0, 1, usize::MAX];
        let pointers = [None, Some(0), Some(1), Some(usize::MAX)];
        for phase in phases {
            for current in pointers {
                for expected in indices {
                    let input = current == Some(expected)
                        && [P::AvailableAtAdmission, P::Committed].contains(&phase);
                    assert_eq!(r65_version_input_ready_v1(current, expected, phase), input);
                    for pending in pointers {
                        let begin =
                            phase == P::Planned && pending.is_none() && current == Some(expected);
                        let commit =
                            phase == P::InFlight && pending == Some(expected) && current.is_none();
                        assert_eq!(
                            r65_version_begin_write_v1(phase, pending, current, expected),
                            begin
                        );
                        assert_eq!(
                            r65_version_commit_write_v1(phase, pending, expected, current),
                            commit
                        );
                    }
                }
            }
        }
    }
}
