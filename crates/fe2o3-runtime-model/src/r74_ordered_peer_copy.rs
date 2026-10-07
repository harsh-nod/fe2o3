//! Pure ordered-copy admission and cursor guards. These values grant no native
//! authority. Ticket authenticity, DMA effects and full-currentness observations
//! are obligations of the adapter, not consequences of this model.

pub const MAX_ORDERED_PEER_COPY_SEGMENTS_V1: usize = 4096;

/// Offsets relative to the source and destination bounding regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedPeerCopySegmentV1 {
    pub source_offset: u64,
    pub destination_offset: u64,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedPeerCopyAdmissionErrorV1 {
    Count,
    SourceRange,
    DestinationRange,
    Length,
    TotalOverflow,
}

/// Validate every descriptor without reordering, deduplicating or allocating.
/// Overlapping destination writes are allowed and must execute in list order.
pub fn validate_ordered_peer_copy_segments_v1(
    source_base: u64,
    source_len: u64,
    destination_base: u64,
    destination_len: u64,
    segments: &[OrderedPeerCopySegmentV1],
) -> Result<u64, OrderedPeerCopyAdmissionErrorV1> {
    use OrderedPeerCopyAdmissionErrorV1 as E;
    if segments.is_empty() || segments.len() > MAX_ORDERED_PEER_COPY_SEGMENTS_V1 {
        return Err(E::Count);
    }
    if source_len == 0 || source_base.checked_add(source_len).is_none() {
        return Err(E::SourceRange);
    }
    if destination_len == 0 || destination_base.checked_add(destination_len).is_none() {
        return Err(E::DestinationRange);
    }
    let mut total = 0_u64;
    for segment in segments {
        if segment.byte_len == 0 {
            return Err(E::Length);
        }
        if segment
            .source_offset
            .checked_add(segment.byte_len)
            .is_none_or(|end| end > source_len)
        {
            return Err(E::SourceRange);
        }
        if segment
            .destination_offset
            .checked_add(segment.byte_len)
            .is_none_or(|end| end > destination_len)
        {
            return Err(E::DestinationRange);
        }
        total = total
            .checked_add(segment.byte_len)
            .ok_or(E::TotalOverflow)?;
    }
    Ok(total)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedPeerCopyOutcomeV1 {
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedPeerCopyActionV1 {
    Open,
    Publish,
    Complete {
        segment: u32,
    },
    /// The adapter recovered both mappings without a usable completion result.
    Recover,
    /// The adapter observed a successful full-currentness close.
    Close,
    CloseFailed,
    Succeed,
    Fail,
    Cancel,
    Quarantine,
}

/// Descriptive state of a single-ticket serial executor. `Publish` records an
/// irreversible publication attempt; it need not imply a confirmed DMA effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedPeerCopyCursorV1 {
    count: u32,
    completed: u32,
    ticket: bool,
    ever_published: bool,
    recovered_failure: bool,
    open: bool,
    outcome: OrderedPeerCopyOutcomeV1,
}

impl OrderedPeerCopyCursorV1 {
    pub const fn new(count: usize) -> Option<Self> {
        if count == 0 || count > MAX_ORDERED_PEER_COPY_SEGMENTS_V1 {
            return None;
        }
        Some(Self {
            count: count as u32,
            completed: 0,
            ticket: false,
            ever_published: false,
            recovered_failure: false,
            open: false,
            outcome: OrderedPeerCopyOutcomeV1::Running,
        })
    }

    pub const fn completed(self) -> u32 {
        self.completed
    }

    pub const fn count(self) -> u32 {
        self.count
    }

    pub const fn has_ticket(self) -> bool {
        self.ticket
    }

    pub const fn ever_published(self) -> bool {
        self.ever_published
    }

    pub const fn is_open(self) -> bool {
        self.open
    }

    pub const fn outcome(self) -> OrderedPeerCopyOutcomeV1 {
        self.outcome
    }

    /// Invalid observations cannot advance or rewrite the cursor.
    pub const fn transition(self, action: OrderedPeerCopyActionV1) -> Option<Self> {
        use OrderedPeerCopyActionV1 as A;
        use OrderedPeerCopyOutcomeV1 as O;
        if !matches!(self.outcome, O::Running) {
            return None;
        }
        let mut next = self;
        match action {
            A::Open if !self.open && !self.recovered_failure => next.open = true,
            A::Publish
                if self.open
                    && !self.recovered_failure
                    && !self.ticket
                    && self.completed < self.count =>
            {
                next.ticket = true;
                next.ever_published = true;
            }
            A::Complete { segment } if self.open && self.ticket && segment == self.completed => {
                next.ticket = false;
                next.completed += 1;
            }
            A::Recover if self.open && self.ticket => {
                next.ticket = false;
                next.recovered_failure = true;
            }
            A::Close if self.open => next.open = false,
            A::CloseFailed if self.open => {
                next.open = false;
                next.outcome = O::Terminal;
            }
            A::Succeed if !self.open && !self.ticket && self.completed == self.count => {
                next.outcome = O::Succeeded;
            }
            A::Fail if !self.open && !self.ticket => next.outcome = O::Failed,
            A::Cancel if !self.open && !self.ever_published => next.outcome = O::Cancelled,
            A::Quarantine if !self.open => next.outcome = O::Terminal,
            _ => return None,
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use OrderedPeerCopyActionV1 as A;
    use OrderedPeerCopyOutcomeV1 as O;
    use alloc::{vec, vec::Vec};

    #[test]
    fn all_reachable_small_states_preserve_order_and_settlement() {
        for count in 1..=4 {
            let mut seen = vec![OrderedPeerCopyCursorV1::new(count).unwrap()];
            let mut index = 0;
            while index < seen.len() {
                let state = seen[index];
                index += 1;
                let mut actions = vec![
                    A::Open,
                    A::Publish,
                    A::Recover,
                    A::Close,
                    A::CloseFailed,
                    A::Succeed,
                    A::Fail,
                    A::Cancel,
                    A::Quarantine,
                ];
                actions.extend((0..=count as u32 + 1).map(|segment| A::Complete { segment }));
                for action in actions {
                    let Some(next) = state.transition(action) else {
                        continue;
                    };
                    assert_eq!(state.outcome, O::Running);
                    assert!(next.completed <= next.count);
                    assert!(next.completed >= state.completed);
                    assert!(!state.ever_published || next.ever_published);
                    assert!(!next.ticket || next.completed < next.count && next.ever_published);
                    assert!(next.completed == 0 || next.ever_published);
                    if next.completed != state.completed {
                        assert_eq!(
                            action,
                            A::Complete {
                                segment: state.completed
                            }
                        );
                        assert!(state.ticket && state.open);
                        assert_eq!(next.completed, state.completed + 1);
                    }
                    if next.outcome == O::Succeeded {
                        assert!(!next.open && !next.ticket && next.completed == next.count);
                    }
                    if next.outcome == O::Cancelled {
                        assert!(!next.open && !next.ticket && !next.ever_published);
                        assert_eq!(next.completed, 0);
                    }
                    if !seen.contains(&next) {
                        seen.push(next);
                    }
                }
            }
        }
    }

    #[test]
    fn serial_count_is_independent_of_native_ring_capacity() {
        for count in [1, 2, 63, 64, 65, MAX_ORDERED_PEER_COPY_SEGMENTS_V1] {
            let mut cursor = OrderedPeerCopyCursorV1::new(count).unwrap();
            for segment in 0..count as u32 {
                cursor = cursor.transition(A::Open).unwrap();
                cursor = cursor.transition(A::Publish).unwrap();
                assert!(cursor.transition(A::Publish).is_none());
                cursor = cursor.transition(A::Close).unwrap();
                assert!(cursor.transition(A::Cancel).is_none());
                cursor = cursor.transition(A::Open).unwrap();
                cursor = cursor.transition(A::Complete { segment }).unwrap();
                cursor = cursor.transition(A::Close).unwrap();
                assert!(cursor.transition(A::Cancel).is_none());
            }
            assert_eq!(
                cursor.transition(A::Succeed).unwrap().outcome(),
                O::Succeeded
            );
        }
        assert!(OrderedPeerCopyCursorV1::new(0).is_none());
        assert!(OrderedPeerCopyCursorV1::new(MAX_ORDERED_PEER_COPY_SEGMENTS_V1 + 1).is_none());
    }

    #[test]
    fn whole_roster_ranges_counts_and_total_are_checked() {
        use OrderedPeerCopyAdmissionErrorV1 as E;
        let segment = OrderedPeerCopySegmentV1 {
            source_offset: 1,
            destination_offset: 2,
            byte_len: 3,
        };
        let validate = |segments: &[OrderedPeerCopySegmentV1]| {
            validate_ordered_peer_copy_segments_v1(10, 5, 20, 8, segments)
        };
        assert_eq!(validate(&[segment, segment]), Ok(6));
        assert_eq!(validate(&[]), Err(E::Count));
        for (mutation, error) in [
            (
                OrderedPeerCopySegmentV1 {
                    byte_len: 0,
                    ..segment
                },
                E::Length,
            ),
            (
                OrderedPeerCopySegmentV1 {
                    source_offset: 3,
                    ..segment
                },
                E::SourceRange,
            ),
            (
                OrderedPeerCopySegmentV1 {
                    source_offset: u64::MAX,
                    ..segment
                },
                E::SourceRange,
            ),
            (
                OrderedPeerCopySegmentV1 {
                    destination_offset: 6,
                    ..segment
                },
                E::DestinationRange,
            ),
            (
                OrderedPeerCopySegmentV1 {
                    destination_offset: u64::MAX,
                    ..segment
                },
                E::DestinationRange,
            ),
        ] {
            assert_eq!(validate(&[segment, mutation]), Err(error));
        }
        let large = OrderedPeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: u64::MAX,
        };
        assert_eq!(
            validate_ordered_peer_copy_segments_v1(0, u64::MAX, 0, u64::MAX, &[large, large]),
            Err(E::TotalOverflow)
        );
        assert_eq!(
            validate_ordered_peer_copy_segments_v1(1, u64::MAX, 0, 8, &[segment]),
            Err(E::SourceRange)
        );
        assert_eq!(
            validate_ordered_peer_copy_segments_v1(0, 8, 1, u64::MAX, &[segment]),
            Err(E::DestinationRange)
        );
        let maximum: Vec<_> = vec![segment; MAX_ORDERED_PEER_COPY_SEGMENTS_V1];
        assert_eq!(
            validate(&maximum),
            Ok(3 * MAX_ORDERED_PEER_COPY_SEGMENTS_V1 as u64)
        );
        assert_eq!(
            validate(&vec![segment; MAX_ORDERED_PEER_COPY_SEGMENTS_V1 + 1]),
            Err(E::Count)
        );
    }
}
