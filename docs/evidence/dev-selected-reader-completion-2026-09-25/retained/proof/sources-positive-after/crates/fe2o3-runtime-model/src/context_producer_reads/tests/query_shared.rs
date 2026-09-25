use super::*;

mod performance;

const SCENARIOS: [&str; 13] = [
    "pending",
    "unknown",
    "success",
    "no_effect",
    "success_reused",
    "no_effect_reused",
    "allocation",
    "backlink",
    "device",
    "writer",
    "reference",
    "pending_lineage",
    "resolved_lineage",
];
const OPERATIONS: [&str; 4] = ["status", "validate", "lookup", "query"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Status(Status),
    Read(ContextProducerReadV1),
    Validated,
}

fn next_attempt(f: &mut Fixture) {
    f.journal
        .settle_success(
            f.producer,
            &ContextWriterSuccessEvidenceV1 { writer: f.producer },
        )
        .unwrap();
    let producer = f.journal.register_writer(key(11)).unwrap();
    let member = f.member(f.source);
    f.journal.begin_write(producer, &[member]).unwrap();
    let state = f.journal.lookup_allocation(f.source).unwrap();
    f.producer = producer;
    f.request.producer = producer;
    f.request.read.attempt_epoch = state.attempt_epoch;
    f.request.read.content_lineage = state.content_lineage;
    assert_eq!((state.attempt_epoch, state.content_lineage), (2, 1));
}

fn setup(capacity: usize, scenario: &str) -> (Fixture, ContextProducerReadReferenceV1) {
    let mut f = Fixture::new(capacity);
    if scenario.ends_with("lineage") {
        next_attempt(&mut f);
    }
    let mut reference = f.acquire(20);
    let _retained = f.acquire(21);
    match scenario {
        "pending" | "allocation" | "backlink" | "device" | "writer" | "reference" => {}
        "unknown" => f.journal.mark_unknown(f.producer).unwrap(),
        "success" | "success_reused" => f
            .journal
            .settle_success(
                f.producer,
                &ContextWriterSuccessEvidenceV1 { writer: f.producer },
            )
            .unwrap(),
        "no_effect" | "no_effect_reused" | "resolved_lineage" => f
            .journal
            .settle_no_effect(
                f.producer,
                &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
            )
            .unwrap(),
        "pending_lineage" => {}
        _ => unreachable!(),
    }
    if scenario.ends_with("reused") {
        let next = f.journal.register_writer(key(30)).unwrap();
        assert_eq!(next.slot, f.producer.slot);
        assert_ne!(next.key, f.producer.key);
    }
    match scenario {
        "allocation" => f.request.read.allocation.slot = usize::MAX,
        "backlink" => f.journal.stable.guard_break_backlink_for_test_v1(f.source),
        "device" => f.request.read.device.local += 1,
        "writer" => f
            .journal
            .stable
            .query_replace_writer_for_test_v1(f.producer, None),
        "reference" => reference.incarnation += 1,
        "pending_lineage" | "resolved_lineage" => f.request.read.content_lineage = 0,
        _ => {}
    }
    f.journal.reservations[reference.slot]
        .as_mut()
        .unwrap()
        .request = f.request;
    (f, reference)
}

fn status_result(scenario: &str) -> Result<Status, Error> {
    match scenario {
        "pending" | "reference" => Ok(Status::Pending),
        "unknown" => Ok(Status::Unknown),
        "success" | "success_reused" => Ok(Status::Success),
        "no_effect" | "no_effect_reused" => Ok(Status::NoEffect),
        "allocation" => Err(Error::InvalidAllocationReference),
        "device" => Err(Error::AllocationDeviceMismatch),
        "writer" => Err(Error::InvalidReference),
        "backlink" | "pending_lineage" | "resolved_lineage" => Err(Error::InvalidState),
        _ => unreachable!(),
    }
}

fn expected(f: &Fixture, scenario: &str, operation: &str) -> Result<Outcome, Error> {
    if scenario == "reference" && matches!(operation, "lookup" | "query") {
        return Err(Error::InvalidReference);
    }
    let status = status_result(scenario)?;
    match operation {
        "status" | "query" => Ok(Outcome::Status(status)),
        "validate" if status == Status::Pending => Ok(Outcome::Validated),
        "validate" => Err(Error::AllocationBusy),
        "lookup" => Ok(Outcome::Read(f.request)),
        _ => unreachable!(),
    }
}

fn accesses(scenario: &str, operation: &str, candidate: bool) -> usize {
    if scenario == "reference" && matches!(operation, "lookup" | "query") {
        return 0;
    }
    let base = match scenario {
        "pending" | "unknown" | "writer" | "reference" => 3,
        "backlink" | "device" | "pending_lineage" => 2,
        _ => 1,
    };
    if operation == "query" && !candidate && status_result(scenario).is_ok() {
        base * 2
    } else {
        base
    }
}

fn run(
    f: &Fixture,
    reference: ContextProducerReadReferenceV1,
    operation: &str,
    candidate: bool,
) -> Result<Outcome, Error> {
    match (operation, candidate) {
        ("status", true) => f.journal.status(&f.request).map(Outcome::Status),
        ("status", false) => f
            .journal
            .baseline_status_v1(&f.request)
            .map(Outcome::Status),
        ("validate", true) => f
            .journal
            .validate_producer_read(&f.request)
            .map(|()| Outcome::Validated),
        ("validate", false) => f
            .journal
            .baseline_validate_producer_read_v1(&f.request)
            .map(|()| Outcome::Validated),
        ("lookup", true) => f.journal.lookup_producer_read(reference).map(Outcome::Read),
        ("lookup", false) => f
            .journal
            .baseline_lookup_producer_read_v1(reference)
            .map(Outcome::Read),
        ("query", true) => f
            .journal
            .producer_read_status(reference)
            .map(Outcome::Status),
        ("query", false) => f
            .journal
            .baseline_producer_read_status_v1(reference)
            .map(Outcome::Status),
        _ => unreachable!(),
    }
}

fn qualify(
    f: &Fixture,
    reference: ContextProducerReadReferenceV1,
    scenario: &str,
    operation: &str,
) {
    let before = snapshot(&f.journal);
    let storage = f.journal.guard_owner_storage_v1();
    for candidate in [false, true] {
        f.journal.reset_access_count_for_test_v1();
        assert_eq!(
            run(f, reference, operation, candidate),
            expected(f, scenario, operation)
        );
        assert_eq!(
            f.journal.guard_accesses_for_test_v1(),
            accesses(scenario, operation, candidate)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(f.journal.guard_owner_storage_v1(), storage);
    }
}

#[test]
fn shared_query_fixtures_match_frozen_results_state_storage_and_accesses() {
    for capacity in [8, 65536] {
        for scenario in SCENARIOS {
            let (f, reference) = setup(capacity, scenario);
            for operation in OPERATIONS {
                qualify(&f, reference, scenario, operation);
            }
        }
    }
}

#[test]
fn shared_query_checks_full_retained_identity_before_stored_request() {
    for fault in 0..7 {
        let (mut f, mut reference) = setup(8, "device");
        match fault {
            0 => reference.slot = usize::MAX,
            1 => reference.slot = 7,
            2 => reference.incarnation += 1,
            3 => reference.consumer.context_generation += 1,
            4 => reference.consumer.local += 1,
            5 => reference.consumer.kind = ContextWriterKindV1::Synchronous,
            6 => {
                f.journal.reservations[reference.slot]
                    .as_mut()
                    .unwrap()
                    .reference
                    .slot += 1
            }
            _ => unreachable!(),
        }
        let before = snapshot(&f.journal);
        for operation in ["lookup", "query"] {
            for candidate in [false, true] {
                assert_eq!(
                    run(&f, reference, operation, candidate),
                    Err(Error::InvalidReference)
                );
                assert_eq!(f.journal.guard_accesses_for_test_v1(), 0);
                assert_eq!(snapshot(&f.journal), before);
            }
        }
    }
}

#[test]
fn shared_status_preserves_ordered_errors_and_range_boundaries() {
    for fault in 0..13 {
        let (mut f, reference) = setup(8, "pending");
        let (error, lookups) = match fault {
            0 => {
                f.request.read.allocation.slot = usize::MAX;
                f.request.read.device.local += 1;
                (Some(Error::InvalidAllocationReference), 1)
            }
            1 => {
                f.journal.stable.guard_break_backlink_for_test_v1(f.source);
                f.request.read.device.local += 1;
                (Some(Error::InvalidState), 2)
            }
            2 => {
                f.request.read.device.local += 1;
                f.request.read.byte_extent += 1;
                (Some(Error::AllocationDeviceMismatch), 2)
            }
            3 => {
                f.request.read.byte_extent += 1;
                f.request.read.byte_len = 0;
                (Some(Error::AllocationExtentMismatch), 2)
            }
            4 => {
                f.request.read.byte_len = 0;
                f.request.producer.key.context_generation += 1;
                (Some(Error::InvalidExtent), 2)
            }
            5 => {
                f.request.read.byte_offset = u64::MAX;
                f.request.read.byte_len = 1;
                (Some(Error::InvalidExtent), 2)
            }
            6 => {
                f.request.read.byte_offset = 64;
                f.request.read.byte_len = 1;
                (Some(Error::InvalidExtent), 2)
            }
            7 => {
                f.request.read.byte_offset = 63;
                f.request.read.byte_len = 1;
                (None, 3)
            }
            8 => {
                f.request.producer.key.context_generation += 1;
                (Some(Error::InvalidState), 2)
            }
            9 => {
                f.request.producer.key.kind = ContextWriterKindV1::Synchronous;
                (Some(Error::InvalidState), 2)
            }
            10 => {
                f.request.read.content_lineage = f.request.read.attempt_epoch;
                (Some(Error::InvalidState), 2)
            }
            11 => {
                f.request.read.attempt_epoch += 1;
                (Some(Error::InvalidState), 2)
            }
            12 => {
                f.journal
                    .stable
                    .query_replace_writer_for_test_v1(f.producer, None);
                f.request.producer.key.local += 1;
                (Some(Error::InvalidState), 2)
            }
            _ => unreachable!(),
        };
        f.journal.reservations[reference.slot]
            .as_mut()
            .unwrap()
            .request = f.request;
        let before = snapshot(&f.journal);
        for candidate in [false, true] {
            let result = run(&f, reference, "status", candidate);
            assert_eq!(
                result,
                error.map_or(Ok(Outcome::Status(Status::Pending)), Err)
            );
            assert_eq!(f.journal.guard_accesses_for_test_v1(), lookups);
            assert_eq!(snapshot(&f.journal), before);
        }
    }
}

#[test]
fn shared_status_ignores_stale_producer_slots_for_resolved_data() {
    for scenario in ["success", "no_effect"] {
        let (mut f, reference) = setup(8, scenario);
        f.request.producer.slot = usize::MAX;
        f.journal.reservations[reference.slot]
            .as_mut()
            .unwrap()
            .request = f.request;
        for operation in OPERATIONS {
            qualify(&f, reference, scenario, operation);
        }
    }
    for scenario in ["pending", "success"] {
        let (mut f, reference) = setup(8, scenario);
        f.request.read.device.context_generation += 1;
        f.journal.reservations[reference.slot]
            .as_mut()
            .unwrap()
            .request = f.request;
        let before = snapshot(&f.journal);
        let storage = f.journal.guard_owner_storage_v1();
        for operation in OPERATIONS {
            for candidate in [false, true] {
                assert_eq!(
                    run(&f, reference, operation, candidate),
                    Err(Error::AllocationDeviceMismatch)
                );
                assert_eq!(
                    f.journal.guard_accesses_for_test_v1(),
                    if scenario == "pending" { 2 } else { 1 }
                );
                assert_eq!(snapshot(&f.journal), before);
                assert_eq!(f.journal.guard_owner_storage_v1(), storage);
            }
        }
    }
}

#[test]
fn shared_status_distinguishes_missing_reserved_and_stale_pending_writer() {
    for fault in 0..5 {
        let (mut f, reference) = setup(8, "pending");
        let mut writer = f.producer;
        let replacement = match fault {
            0 => None,
            1 => Some(ContextWriterStateV1::Reserved),
            2 => {
                writer.key.context_generation += 1;
                Some(ContextWriterStateV1::Pending { member_count: 1 })
            }
            3 => {
                writer.key.local += 1;
                Some(ContextWriterStateV1::Pending { member_count: 1 })
            }
            4 => {
                writer.key.kind = ContextWriterKindV1::Synchronous;
                Some(ContextWriterStateV1::Pending { member_count: 1 })
            }
            _ => unreachable!(),
        };
        f.journal
            .stable
            .query_replace_writer_for_test_v1(writer, replacement);
        let before = snapshot(&f.journal);
        for operation in OPERATIONS {
            for candidate in [false, true] {
                assert_eq!(
                    run(&f, reference, operation, candidate),
                    Err(if fault == 1 {
                        Error::InvalidState
                    } else {
                        Error::InvalidReference
                    })
                );
                assert_eq!(f.journal.guard_accesses_for_test_v1(), 3);
                assert_eq!(snapshot(&f.journal), before);
            }
        }
    }
}
