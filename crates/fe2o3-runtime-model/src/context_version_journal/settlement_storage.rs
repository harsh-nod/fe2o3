use super::ContextVersionJournalErrorV1;

include!("settlement_return_body.rs");

#[derive(Clone, Copy)]
pub(super) struct SettlementReturnStorageV1 {
    pub(super) writer_free_len: usize,
    pub(super) member_free_len: usize,
    pub(super) writer_limit: usize,
    pub(super) writer_storage: usize,
    pub(super) member_limit: usize,
    pub(super) member_storage: usize,
    pub(super) scratch_len: usize,
}

impl SettlementReturnStorageV1 {
    #[inline]
    pub(super) fn check(&self, count: usize) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_return_admission_body!(self, count, ContextVersionJournalErrorV1::InvalidState)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    fn choices(required: u128) -> Vec<usize> {
        let mut values = vec![0, usize::MAX];
        for value in [required.saturating_sub(1), required, required + 1] {
            if let Ok(value) = usize::try_from(value) {
                values.push(value);
            }
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    #[test]
    fn scalar_return_admission_matches_widened_boundary_oracle() {
        let mut accepted = 0;
        let mut rejected = 0;
        for (writer_free_len, member_free_len, count) in [
            (0, 0, 0),
            (0, 0, 1),
            (1, 2, 3),
            (usize::MAX - 1, 0, 0),
            (usize::MAX, 0, 0),
            (0, usize::MAX, 0),
            (0, usize::MAX, 1),
            (0, usize::MAX - 1, 1),
            (0, usize::MAX - 1, 2),
            (0, 0, usize::MAX),
            (usize::MAX, usize::MAX, usize::MAX),
        ] {
            let writer_returns = writer_free_len as u128 + 1;
            let member_returns = member_free_len as u128 + count as u128;
            for writer_limit in choices(writer_returns) {
                for writer_storage in choices(writer_returns) {
                    for member_limit in choices(member_returns) {
                        for member_storage in choices(member_returns) {
                            for scratch_len in choices(count as u128) {
                                let observed = SettlementReturnStorageV1 {
                                    writer_free_len,
                                    member_free_len,
                                    writer_limit,
                                    writer_storage,
                                    member_limit,
                                    member_storage,
                                    scratch_len,
                                };
                                let allowed = writer_returns <= usize::MAX as u128
                                    && member_returns <= usize::MAX as u128
                                    && writer_returns <= writer_limit as u128
                                    && writer_returns <= writer_storage as u128
                                    && member_returns <= member_limit as u128
                                    && member_returns <= member_storage as u128
                                    && count <= scratch_len;
                                let expected = if allowed {
                                    accepted += 1;
                                    Ok(())
                                } else {
                                    rejected += 1;
                                    Err(ContextVersionJournalErrorV1::InvalidState)
                                };
                                assert_eq!(observed.check(count), expected);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!((accepted, rejected), (819, 6177));
    }
}
