use super::*;
use crate::authorized_execution::tests::{
    TestAuthorityV1, source_authority_for_device, source_projection,
};
use crate::generated_source::GeneratedHostRosterV1;
use crate::{GeneratedGfx942PersistentStorageV1, RuntimeGfx942GeneratedSourceMutV1};

struct Roster {
    device: RuntimeDeviceIdV1,
    uid: u64,
    hold: ContextUnpublishedHoldV1,
    hsaco: Vec<u8>,
    storage: GeneratedGfx942PersistentStorageV1,
    authority: TestAuthorityV1,
    roster: GeneratedHostRosterV1,
}

struct Fixture {
    context: RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>,
    rosters: [Roster; 2],
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    ids: Vec<RuntimeAllocationIdV1>,
    handles: HashSet<u64>,
    next: u64,
    generated: [Option<u64>; 2],
    usage: [crate::RuntimeResourceCreditUsageV1; 2],
    journal: RuntimeContextJournalUsageV1,
}

impl Fixture {
    fn new(second_limits: (u64, usize), capacity: usize) -> Self {
        let mut context = RuntimeContextV1::open_with_version_journal_v1(
            KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
            capacity,
            2,
        )
        .unwrap();
        let rosters = [(7, (60, 3)), (8, second_limits)].map(|(uid, (bytes, records))| {
            let device = context
                .devices()
                .iter()
                .find(|record| record.backend_device == uid)
                .unwrap()
                .id();
            context
                .configure_allocation_admission_v1(device, bytes, records)
                .unwrap();
            let stream = context.create_stream(device).unwrap();
            let hold = context.hold_unpublished_stream_v1(stream).unwrap();
            let (hsaco, projection) = source_projection();
            let authority = source_authority_for_device(&projection, uid);
            let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
            Roster {
                device,
                uid,
                hold,
                hsaco,
                storage: projection.into_generated_storage_v1(),
                authority,
                roster,
            }
        });
        Self { context, rosters }
    }

    fn install(&mut self, index: usize) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let roster = &mut self.rosters[index];
        let mut source = RuntimeGfx942GeneratedSourceMutV1::new(
            &mut roster.storage,
            &roster.hsaco,
            &roster.authority,
        );
        // Exercise the real Context ledger with descriptive model admissions only.
        // This is not a successful native DATA or protected Worker preparation.
        self.context.install_generated_shells_v1(
            roster.device,
            admission_for_device(1, 1, roster.uid).1,
            &roster.hold,
            &mut source,
            &roster.roster,
        )
    }

    fn snapshot(&self) -> Snapshot {
        let mut ids = self.context.allocations.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        Snapshot {
            ids,
            handles: self.context.backend_allocations.clone(),
            next: self.context.next_identity,
            generated: core::array::from_fn(|i| {
                self.context.streams[&self.rosters[i].hold.stream()].generated
            }),
            usage: core::array::from_fn(|i| {
                self.context
                    .allocation_admission_usage_v1(self.rosters[i].device)
                    .unwrap()
                    .unwrap()
            }),
            journal: self.context.version_journal_usage_v1().unwrap(),
        }
    }

    fn retire(&mut self, index: usize) {
        self.context
            .retire_gfx942_adoption_v1(&self.rosters[index].hold)
            .unwrap();
    }

    fn finish(mut self) {
        for roster in &self.rosters {
            self.context
                .release_unpublished_hold_v1(&roster.hold)
                .unwrap();
        }
        assert!(self.context.cleanup().is_complete());
        self.context.shutdown_owned_backend_v1().unwrap();
    }
}

#[test]
fn generated_multi_context_journals_and_refunds_each_exact_device_independently() {
    let mut fixture = Fixture::new((60, 3), 6);
    let initial = fixture.snapshot();
    let pointers: [_; 2] = core::array::from_fn(|i| {
        fixture.rosters[i]
            .storage
            .buffers()
            .iter()
            .map(|buffer| buffer.bytes().as_ptr())
            .collect::<Vec<_>>()
    });
    for index in 0..2 {
        let next = fixture.context.next_identity;
        fixture.install(index).unwrap();
        assert_eq!(fixture.context.next_identity, next + 3);
    }
    let installed = fixture.snapshot();
    assert_eq!(installed.ids.len(), 6);
    assert_eq!(installed.handles.len(), 6);
    assert_eq!(installed.journal.allocation_records, 6);
    assert_eq!(installed.journal.provisional_records, 0);
    for usage in installed.usage {
        assert_eq!(
            usage.used,
            crate::RuntimeResourceVectorV1::ZERO
                .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 60)
                .with(crate::RuntimeResourceKindV1::AllocationRecords, 3)
        );
        assert_eq!(usage.retained_records, 3);
        assert_eq!(usage.reserved_records, 0);
        assert_eq!(usage.quarantined_records, 0);
    }
    let first = fixture
        .context
        .generated_plan_for_hold_v1(&fixture.rosters[0].hold)
        .unwrap();
    fixture.retire(1);
    let after = fixture.snapshot();
    assert_eq!(after.ids.len(), 3);
    assert_eq!(after.journal.allocation_records, 3);
    assert_eq!(after.usage[0], installed.usage[0]);
    assert_eq!(after.usage[1], initial.usage[1]);
    assert_eq!(
        fixture
            .context
            .generated_plan_for_hold_v1(&fixture.rosters[0].hold)
            .unwrap(),
        first
    );
    fixture.retire(0);
    let after = fixture.snapshot();
    assert_eq!(after.usage, initial.usage);
    assert_eq!(after.journal.allocation_records, 0);
    assert!(after.ids.is_empty() && after.handles.is_empty());
    for (index, expected) in pointers.iter().enumerate() {
        assert_eq!(
            &fixture.rosters[index]
                .storage
                .buffers()
                .iter()
                .map(|buffer| buffer.bytes().as_ptr())
                .collect::<Vec<_>>(),
            expected
        );
    }
    fixture.finish();
}

#[test]
fn generated_multi_context_second_roster_capacity_failure_preserves_first_and_identity() {
    for (limits, capacity) in [((59, 3), 6), ((60, 2), 6), ((60, 3), 5)] {
        let mut fixture = Fixture::new(limits, capacity);
        fixture.install(0).unwrap();
        let before = fixture.snapshot();
        assert!(matches!(
            fixture.install(1),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
        assert!(fixture.rosters[1].storage.control_available());
        assert_eq!(fixture.snapshot(), before);
        assert!(
            fixture
                .context
                .generated_plan_for_hold_v1(&fixture.rosters[0].hold)
                .is_ok()
        );
        fixture.retire(0);
        fixture.finish();
    }
}

#[test]
fn generated_multi_context_equal_size_cross_device_credits_cannot_substitute() {
    let mut fixture = Fixture::new((60, 3), 6);
    fixture.install(0).unwrap();
    fixture.install(1).unwrap();
    let plans: [_; 2] = core::array::from_fn(|i| {
        fixture
            .context
            .generated_plan_for_hold_v1(&fixture.rosters[i].hold)
            .unwrap()
    });
    let [first, second] = plans.map(|plan| plan.members[0].unwrap());
    assert_eq!(first.description.byte_len, second.description.byte_len);
    let before = fixture.snapshot();
    fixture
        .context
        .allocation_admission
        .swap_retained_for_test_v1(first.logical, second.logical);
    for roster in &fixture.rosters {
        assert!(matches!(
            fixture.context.generated_plan_for_hold_v1(&roster.hold),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidBackendDescription
            ))
        ));
    }
    assert_eq!(fixture.snapshot(), before);
    fixture
        .context
        .allocation_admission
        .swap_retained_for_test_v1(first.logical, second.logical);
    fixture.retire(0);
    fixture.retire(1);
    fixture.finish();
}

#[test]
fn generated_multi_context_bad_later_member_keeps_both_rosters_and_quarantines_all_credits() {
    let mut fixture = Fixture::new((60, 3), 6);
    fixture.install(0).unwrap();
    fixture.install(1).unwrap();
    let plan = fixture
        .context
        .generated_plan_for_hold_v1(&fixture.rosters[1].hold)
        .unwrap();
    fixture
        .context
        .allocations
        .get_mut(&plan.members[2].unwrap().logical)
        .unwrap()
        .journal = None;
    let mut expected = fixture.snapshot();
    for usage in &mut expected.usage {
        usage.retained_records = 0;
        usage.quarantined_records = 3;
    }
    assert!(
        fixture
            .context
            .retire_gfx942_adoption_v1(&fixture.rosters[1].hold)
            .is_err()
    );
    assert!(fixture.context.is_terminal());
    assert_eq!(fixture.snapshot(), expected);
    assert!(!fixture.context.cleanup().is_complete());
    // A terminal backend retains custody through process exit, never destructive Drop.
    core::mem::forget(fixture.context);
}
