//! Pure guards, not execution authority. Verus correspondence is reviewed;
//! private tokens, ledger conservation and backend observations are external.

pub const fn r63_graph_can_acquire_v1(
    terminal: bool,
    reserved: bool,
    submissions: usize,
    events: usize,
) -> bool {
    !terminal && !reserved && submissions == 0 && events == 0
}
pub const fn r63_graph_can_issue_v1(terminal: bool, exact_token: bool, issue_closed: bool) -> bool {
    !terminal && exact_token && !issue_closed
}
pub const fn r63_graph_can_release_v1(
    terminal: bool,
    exact_token: bool,
    issue_closed: bool,
    submissions: usize,
    events: usize,
) -> bool {
    !terminal && exact_token && issue_closed && submissions == 0 && events == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustive_reservation_guards_match_independent_cases() {
        for terminal in [false, true] {
            for reserved in [false, true] {
                for exact in [false, true] {
                    for closed in [false, true] {
                        for submissions in [0, 1, 2, usize::MAX] {
                            for events in [0, 1, 2, usize::MAX] {
                                assert_eq!(
                                    r63_graph_can_acquire_v1(
                                        terminal,
                                        reserved,
                                        submissions,
                                        events
                                    ),
                                    matches!(
                                        (terminal, reserved, submissions, events),
                                        (false, false, 0, 0)
                                    )
                                );
                                assert_eq!(
                                    r63_graph_can_issue_v1(terminal, exact, closed),
                                    matches!((terminal, exact, closed), (false, true, false))
                                );
                                assert_eq!(
                                    r63_graph_can_release_v1(
                                        terminal,
                                        exact,
                                        closed,
                                        submissions,
                                        events
                                    ),
                                    matches!(
                                        (terminal, exact, closed, submissions, events),
                                        (false, true, true, 0, 0)
                                    )
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
