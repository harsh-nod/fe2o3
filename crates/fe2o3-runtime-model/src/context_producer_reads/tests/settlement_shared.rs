use super::*;
use crate::context_read_leases::StableReadFaultV1;

#[test]
fn settlement_shared_owner_chain_preserves_live_and_malformed_outer_custody() {
    for success in [false, true] {
        for malformed in [false, true] {
            let mut f = Fixture::new(4);
            let a = f.acquire(20);
            let b = f.acquire(21);
            let read = f.stable_read(f.other);
            let mut stable = [None];
            f.journal
                .acquire_reads(key(22), &[read], &mut stable)
                .unwrap();
            if malformed {
                f.journal.counts.clear();
                f.journal.free[0] = usize::MAX;
                f.journal.next_incarnation = 0;
                f.journal
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
                f.journal
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(0));
            }
            let copy = f.journal.guard_copy_for_test_v1();
            let storage = f.journal.guard_owner_storage_v1();
            let before = snapshot(&f.journal);
            let result = if success {
                f.journal.baseline_settle_success_v1(
                    f.producer,
                    &ContextWriterSuccessEvidenceV1 { writer: f.producer },
                )
            } else {
                f.journal.baseline_settle_no_effect_v1(
                    f.producer,
                    &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                )
            };
            assert_eq!(result, Ok(()));
            assert_eq!(f.journal.guard_accesses_for_test_v1(), 12);
            let frozen = snapshot(&f.journal);
            f.journal.restore_settlement_for_test_v1(&copy, f.producer);
            assert_eq!(snapshot(&f.journal), before);
            let result = if success {
                f.journal.settle_success(
                    f.producer,
                    &ContextWriterSuccessEvidenceV1 { writer: f.producer },
                )
            } else {
                f.journal.settle_no_effect(
                    f.producer,
                    &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                )
            };
            assert_eq!(result, Ok(()));
            assert_eq!(f.journal.guard_accesses_for_test_v1(), 12);
            assert_eq!(snapshot(&f.journal), frozen);
            assert_eq!(f.journal.guard_owner_storage_v1(), storage);
            if !malformed {
                let status = if success {
                    Status::Success
                } else {
                    Status::NoEffect
                };
                assert_eq!(f.journal.producer_read_status(a), Ok(status));
                assert_eq!(f.journal.producer_read_status(b), Ok(status));
                assert_eq!(f.journal.lookup_producer_read(a), Ok(f.request));
                assert_eq!(f.journal.lookup_producer_read(b), Ok(f.request));
                assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
                assert_eq!(f.journal.reader_count(f.source), Ok(2));
                assert_eq!(f.journal.reader_count(f.other), Ok(1));
                assert_eq!(f.journal.retained_read_count(), 3);
                let state = f.journal.lookup_allocation(f.source).unwrap();
                assert_eq!(state.attempt_epoch, 1);
                assert_eq!(state.content_lineage, u64::from(success));
                assert_eq!(state.pending_writer, None);
            }
        }
    }
}
