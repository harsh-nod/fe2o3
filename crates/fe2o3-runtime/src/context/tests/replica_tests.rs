#![cfg(test)]

use super::*;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

mod faults;
mod graph;
mod retirement;
mod selection;

type Context = RuntimeContextV1<MockBackend>;

fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        records,
    )
    .unwrap()
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 96,
    }
}

struct Fixture {
    context: Context,
    account: ResourceCreditAccountV1,
    streams: [RuntimeStreamIdV1; 3],
    allocations: [RuntimeAllocationIdV1; 3],
}

impl Fixture {
    fn new(capacity: usize) -> Self {
        let mut context = Context::open_with_version_journal_v1(
            MockBackend {
                next: 100,
                third_device: true,
                deferred_copies: true,
                ..MockBackend::default()
            },
            32,
            32,
        )
        .unwrap();
        let account = account(
            RuntimeReplicaStorageV1::required_payload_bytes_v1(capacity).unwrap(),
            1,
        );
        context
            .configure_replica_registry_v1(
                RuntimeReplicaStorageV1::preallocate(&account, capacity).unwrap(),
            )
            .unwrap();
        let devices = std::array::from_fn::<_, 3, _>(|i| context.devices()[i].id());
        let streams = devices.map(|d| context.create_stream(d).unwrap());
        let allocations = devices.map(|d| {
            context
                .allocate(d, RuntimeMemoryKindV1::HostVisible, 96, 16)
                .unwrap()
        });
        for (index, allocation) in allocations.iter().enumerate() {
            context
                .write_allocation(*allocation, 0, &[0x31 + index as u8; 96])
                .unwrap();
        }
        Self {
            context,
            account,
            streams,
            allocations,
        }
    }

    fn start(&mut self, source: usize, destination: usize) -> RuntimeTrackedReplicaCopyV1 {
        self.context
            .submit_tracked_replica_copy_v1(
                self.streams[destination],
                region(self.allocations[source], RuntimeAccessV1::Read),
                region(self.allocations[destination], RuntimeAccessV1::Write),
            )
            .unwrap()
    }

    fn observe(&mut self, copy: &mut RuntimeTrackedReplicaCopyV1) {
        for _ in 0..4 {
            if self.context.poll_tracked_replica_copy_v1(copy).unwrap() == RuntimePollV1::Succeeded
            {
                return;
            }
        }
        panic!("bounded mock completion");
    }

    fn settle(&mut self, mut copy: RuntimeTrackedReplicaCopyV1) -> RuntimeReplicaReferenceV1 {
        self.observe(&mut copy);
        match self.context.retire_tracked_replica_copy_v1(copy).unwrap() {
            RuntimeReplicaCopySettlementV1::Current(reference) => reference,
            other => panic!("unexpected settlement: {other:?}"),
        }
    }

    fn read(&mut self, index: usize) -> [u8; 96] {
        let mut bytes = [0; 96];
        self.context
            .read_allocation(self.allocations[index], 0, &mut bytes)
            .unwrap();
        bytes
    }

    fn finish(mut self) {
        assert!(self.context.cleanup().is_complete());
        let charged = self.account.usage();
        assert_eq!(charged.retained_records, 1);
        drop(self.context);
        assert_eq!(self.account.usage().retained_records, 0);
        assert_eq!(self.account.usage().used, ResourceVectorV1::ZERO);
    }
}

#[test]
fn replica_requires_original_success_and_actual_release_before_current_fact() {
    let mut f = Fixture::new(4);
    let copy = f.start(0, 1);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 1);
    let refused = f.context.retire_tracked_replica_copy_v1(copy).unwrap_err();
    assert!(matches!(
        refused.error,
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    let mut copy = refused.copy;
    f.observe(&mut copy);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 0);
    let before = f.account.usage();
    f.context.backend.release_submission_failure = MockMemoryFailure::Rejected;
    let refused = f.context.retire_tracked_replica_copy_v1(copy).unwrap_err();
    assert!(matches!(refused.error, RuntimeErrorV1::BackendRejected(_)));
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 1);
    assert_eq!(f.context.submissions.len(), 1);
    assert_eq!(f.account.usage(), before);
    let reference = match f
        .context
        .retire_tracked_replica_copy_v1(refused.copy)
        .unwrap()
    {
        RuntimeReplicaCopySettlementV1::Current(reference) => reference,
        other => panic!("unexpected: {other:?}"),
    };
    f.context.validate_replica_v1(reference).unwrap();
    assert_eq!(
        f.context.replica_registry_usage_v1().unwrap(),
        RuntimeReplicaUsageV1 {
            capacity: 4,
            pending: 0,
            settled: 1
        }
    );
    assert!(f.context.submissions.is_empty());
    assert_eq!(f.read(0), [0x31; 96]);
    assert_eq!(f.read(1), [0x31; 96]);
    assert_eq!(f.read(2), [0x33; 96]);
    f.finish();
}

#[test]
fn replica_source_and_destination_writes_invalidate_even_identical_bytes() {
    for changed in [0, 1] {
        let mut f = Fixture::new(2);
        let copy = f.start(0, 1);
        let reference = f.settle(copy);
        f.context
            .write_allocation(f.allocations[changed], 0, &[0x31; 96])
            .unwrap();
        assert!(f.context.validate_replica_v1(reference).is_err());
        f.context.forget_replica_v1(reference).unwrap();
        assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 0);
        f.finish();
    }
}

#[test]
fn replica_original_context_allocation_and_slot_incarnations_are_not_interchangeable() {
    let mut f = Fixture::new(1);
    let mut foreign = Fixture::new(1);
    let copy = f.start(0, 1);
    let old = f.settle(copy);
    assert!(foreign.context.validate_replica_v1(old).is_err());
    assert!(foreign.context.forget_replica_v1(old).is_err());
    f.context.forget_replica_v1(old).unwrap();
    let copy = f.start(0, 1);
    let new = f.settle(copy);
    assert_ne!(old, new);
    assert!(f.context.validate_replica_v1(old).is_err());
    assert!(f.context.forget_replica_v1(old).is_err());
    f.context.validate_replica_v1(new).unwrap();
    f.context.release_allocation(f.allocations[1]).unwrap();
    f.allocations[1] = f
        .context
        .allocate(
            f.context.devices()[1].id(),
            RuntimeMemoryKindV1::HostVisible,
            96,
            16,
        )
        .unwrap();
    assert!(f.context.validate_replica_v1(new).is_err());
    f.context.forget_replica_v1(new).unwrap();
    f.context
        .write_allocation(f.allocations[1], 0, &[0x32; 96])
        .unwrap();
    f.context.exhaust_replica_slot_for_test_v1();
    let calls = f.context.backend.copy_call_count;
    assert!(matches!(
        f.context.submit_tracked_replica_copy_v1(
            f.streams[1],
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write)
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, calls);
    f.finish();
    foreign.finish();
}

#[test]
fn replica_changed_after_successful_poll_settles_without_current_fact() {
    let mut f = Fixture::new(2);
    let mut copy = f.start(0, 1);
    f.observe(&mut copy);
    f.context
        .write_allocation(f.allocations[0], 0, &[0xee; 96])
        .unwrap();
    assert_eq!(
        f.context.retire_tracked_replica_copy_v1(copy).unwrap(),
        RuntimeReplicaCopySettlementV1::SettledWithoutCurrentReplica(
            RuntimeCompletionStatusV1::Succeeded
        )
    );
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 0);
    assert_eq!(f.read(1), [0x31; 96]);
    f.finish();
}

#[test]
fn replica_metadata_capacity_and_original_account_refusals_precede_copy_entry() {
    for capacity in [0, MAX_RUNTIME_REPLICA_RECORDS_V1 + 1, usize::MAX] {
        let account = account(1 << 20, 1);
        assert!(RuntimeReplicaStorageV1::preallocate(&account, capacity).is_err());
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }
    let payload = RuntimeReplicaStorageV1::required_payload_bytes_v1(4).unwrap();
    let maximum =
        RuntimeReplicaStorageV1::required_payload_bytes_v1(MAX_RUNTIME_REPLICA_RECORDS_V1).unwrap();
    assert!(maximum <= MAX_RUNTIME_REPLICA_STORAGE_BYTES_V1);
    let maximum_account = account(maximum, 1);
    let maximum_storage =
        RuntimeReplicaStorageV1::preallocate(&maximum_account, MAX_RUNTIME_REPLICA_RECORDS_V1)
            .unwrap();
    assert_eq!(
        maximum_account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        maximum
    );
    drop(maximum_storage);
    assert_eq!(maximum_account.usage().used, ResourceVectorV1::ZERO);
    let denied = account(payload - 1, 1);
    assert!(RuntimeReplicaStorageV1::preallocate(&denied, 4).is_err());
    assert_eq!(denied.usage().retained_records, 0);
    let exact = account(payload, 1);
    let storage = RuntimeReplicaStorageV1::preallocate(&exact, 4).unwrap();
    assert_eq!(
        exact.usage().used.get(ResourceKindV1::ControlResidentBytes),
        payload
    );
    let mut no_journal = Context::open(MockBackend::default()).unwrap();
    let failed = no_journal
        .configure_replica_registry_v1(storage)
        .unwrap_err();
    assert_eq!(exact.usage().retained_records, 1);
    drop(failed);
    assert_eq!(exact.usage().used, ResourceVectorV1::ZERO);
    assert!(no_journal.cleanup().is_complete());
    let mut f = Fixture::new(1);
    let copy = f.start(0, 1);
    let calls = f.context.backend.copy_call_count;
    assert!(matches!(
        f.context.submit_tracked_replica_copy_v1(
            f.streams[2],
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[2], RuntimeAccessV1::Write)
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, calls);
    f.settle(copy);
    f.finish();
}

#[test]
fn replica_whole_copy_bounds_refuse_before_metadata_or_native_mutation() {
    let mut f = Fixture::new(4);
    for case in 0..6 {
        let mut source = region(f.allocations[0], RuntimeAccessV1::Read);
        let mut destination = region(f.allocations[1], RuntimeAccessV1::Write);
        match case {
            0 => source.access = RuntimeAccessV1::ReadWrite,
            1 => destination.access = RuntimeAccessV1::Read,
            2 => source.byte_offset = 1,
            3 => {
                source.byte_len = 95;
                destination.byte_len = 95;
            }
            4 => destination.allocation = source.allocation,
            5 => destination.byte_len = u64::MAX,
            _ => unreachable!(),
        }
        let before = f.account.usage();
        assert!(
            f.context
                .submit_tracked_replica_copy_v1(f.streams[1], source, destination)
                .is_err()
        );
        assert_eq!(f.account.usage(), before);
        assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 0);
        assert_eq!(f.context.backend.copy_call_count, 0);
    }
    f.finish();
}
