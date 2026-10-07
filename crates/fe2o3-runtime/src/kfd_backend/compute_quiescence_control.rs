//! Scalar observation control shared with Verus; no native or retain-map authority.

include!("compute_quiescence_body.rs");

macro_rules! quiescence_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QuiescenceActionV1 {
    Invalid,
    Complete,
    Advance,
    Wait,
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct QuiescenceStepV1 {
    pub(super) cursor: usize,
    pub(super) polled: bool,
    pub(super) action: QuiescenceActionV1,
}

pub(super) fn quiescence_step_v1(
    cursor: usize,
    len: usize,
    polled: bool,
    exact: bool,
) -> QuiescenceStepV1 {
    compute_quiescence_step_body!(quiescence_rust_expr, cursor, len, polled, exact)
}

pub(super) fn quiescence_complete_v1(cursor: usize, len: usize) -> bool {
    compute_quiescence_complete_body!(quiescence_rust_expr, cursor, len)
}

impl super::PendingComputeSubmissionV1 {
    pub(super) fn quiescence_complete_v1(&self) -> bool {
        quiescence_complete_v1(self.quiescence_cursor, self.quiescence_dependencies.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiescence_control_exhausts_decisions_and_word_boundaries() {
        for len in [0, 1, 2, usize::MAX - 1, usize::MAX] {
            for cursor in [0, 1, 2, usize::MAX - 1, usize::MAX] {
                for polled in [false, true] {
                    for exact in [false, true] {
                        let out = quiescence_step_v1(cursor, len, polled, exact);
                        let expected = if cursor > len {
                            QuiescenceActionV1::Invalid
                        } else if cursor == len {
                            QuiescenceActionV1::Complete
                        } else if exact {
                            QuiescenceActionV1::Advance
                        } else if polled {
                            QuiescenceActionV1::Wait
                        } else {
                            QuiescenceActionV1::Poll
                        };
                        assert_eq!(out.action, expected);
                        assert_eq!(
                            out.cursor,
                            if expected == QuiescenceActionV1::Advance {
                                cursor + 1
                            } else {
                                cursor
                            }
                        );
                        assert_eq!(out.polled, polled || expected == QuiescenceActionV1::Poll);
                        assert_eq!(quiescence_complete_v1(cursor, len), cursor == len);
                        if out.action == QuiescenceActionV1::Poll {
                            assert_eq!(
                                quiescence_step_v1(out.cursor, len, out.polled, false).action,
                                QuiescenceActionV1::Wait
                            );
                        }
                    }
                }
            }
        }
    }
}
