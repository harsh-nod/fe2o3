use super::*;

fn compare_register(
    owner: &mut ContextProducerReadJournalV1,
    key: ContextWriterKeyV1,
) -> ContextWriterReferenceV1 {
    let journal = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    let expected = owner.baseline_register_writer_v1(key).unwrap();
    assert_eq!(owner.guard_accesses_for_test_v1(), 4);
    let frozen = snapshot(owner);
    owner.restore_writer_for_test_v1(&journal, expected.slot);
    assert_eq!(snapshot(owner), before);
    assert_eq!(owner.register_writer(key), Ok(expected));
    assert_eq!(owner.guard_accesses_for_test_v1(), 4);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    assert_eq!(owner.lookup_reserved(expected), Ok(key));
    expected
}

fn compare_abort(owner: &mut ContextProducerReadJournalV1, reference: ContextWriterReferenceV1) {
    let journal = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    assert_eq!(owner.baseline_abort_reserved_v1(reference), Ok(()));
    assert_eq!(owner.guard_accesses_for_test_v1(), 3);
    let frozen = snapshot(owner);
    owner.restore_writer_for_test_v1(&journal, reference.slot);
    assert_eq!(snapshot(owner), before);
    assert_eq!(owner.abort_reserved(reference), Ok(()));
    assert_eq!(owner.guard_accesses_for_test_v1(), 3);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
}

#[test]
fn writer_shared_owner_preserves_producer_statuses_and_stable_custody() {
    for status in [
        Status::Pending,
        Status::Success,
        Status::NoEffect,
        Status::Unknown,
    ] {
        let mut f = Fixture::new(4);
        let producer = f.acquire(20);
        let read = f.stable_read(f.other);
        let mut stable = [None];
        f.journal
            .acquire_reads(key(21), &[read], &mut stable)
            .unwrap();
        match status {
            Status::Pending => {}
            Status::Success => f
                .journal
                .settle_success(
                    f.producer,
                    &ContextWriterSuccessEvidenceV1 { writer: f.producer },
                )
                .unwrap(),
            Status::NoEffect => f
                .journal
                .settle_no_effect(
                    f.producer,
                    &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                )
                .unwrap(),
            Status::Unknown => f.journal.mark_unknown(f.producer).unwrap(),
        }
        let writer = compare_register(&mut f.journal, key(30));
        compare_abort(&mut f.journal, writer);
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.baseline_abort_reserved_v1(writer),
            Err(Error::InvalidReference)
        );
        assert_eq!(
            f.journal.abort_reserved(writer),
            Err(Error::InvalidReference)
        );
        assert_eq!(
            f.journal.baseline_register_writer_v1(key(30)),
            Err(Error::WriterReplay)
        );
        assert_eq!(f.journal.register_writer(key(30)), Err(Error::WriterReplay));
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(f.journal.producer_read_status(producer), Ok(status));
        assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
        assert_eq!(f.journal.lookup_read(stable[0].unwrap()), Ok(read));
        assert_invariant(&f.journal);
    }
}

#[test]
fn writer_shared_owner_registration_then_begin_rejects_reserved_operations() {
    let mut f = Fixture::new(1);
    let writer = compare_register(&mut f.journal, key(30));
    f.journal.begin_write(writer, &[f.member(f.other)]).unwrap();
    let before = snapshot(&f.journal);
    assert_eq!(
        f.journal.lookup_reserved(writer),
        Err(Error::InvalidReference)
    );
    assert_eq!(
        f.journal.baseline_abort_reserved_v1(writer),
        Err(Error::InvalidReference)
    );
    assert_eq!(
        f.journal.abort_reserved(writer),
        Err(Error::InvalidReference)
    );
    assert_eq!(snapshot(&f.journal), before);
    assert_invariant(&f.journal);
}

#[test]
fn writer_shared_owner_does_not_admit_or_repair_unrelated_malformed_custody() {
    use crate::context_read_leases::StableReadFaultV1;
    let mut owner = ContextProducerReadJournalV1::new(7, 1, 1, 1).unwrap();
    owner.counts.clear();
    owner.free.push(usize::MAX);
    owner.next_incarnation = 0;
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
    owner
        .stable
        .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(u64::MAX));
    let writer = compare_register(&mut owner, key(10));
    compare_abort(&mut owner, writer);
    assert!(owner.counts.is_empty());
    assert_eq!(owner.next_incarnation, 0);
    assert_eq!(owner.free.last(), Some(&usize::MAX));
}
