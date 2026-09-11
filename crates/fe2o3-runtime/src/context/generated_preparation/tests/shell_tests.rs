use super::*;
use crate::authorized_execution::tests::{TestAuthorityV1, source_authority, source_projection};
use crate::generated_source::GeneratedHostRosterV1;
use crate::{GeneratedGfx942PersistentStorageV1, RuntimeGfx942GeneratedSourceMutV1};

struct Fixture {
    context: RuntimeContextV1<KfdRuntimeBackendV1>,
    device: RuntimeDeviceIdV1,
    hold: ContextUnpublishedHoldV1,
    hsaco: Vec<u8>,
    storage: GeneratedGfx942PersistentStorageV1,
    authority: TestAuthorityV1,
    roster: GeneratedHostRosterV1,
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    ids: Vec<RuntimeAllocationIdV1>,
    handles: HashSet<u64>,
    next: u64,
    generated: Option<u64>,
    usage: Option<crate::RuntimeResourceCreditUsageV1>,
}

impl Fixture {
    fn new(limits: Option<(u64, usize)>) -> Self {
        let mut context = context();
        let device = context.devices()[0].id();
        if let Some((bytes, records)) = limits {
            context
                .configure_allocation_admission_v1(device, bytes, records)
                .unwrap();
        }
        let stream = context.create_stream(device).unwrap();
        let hold = context.hold_unpublished_stream_v1(stream).unwrap();
        let (hsaco, projection) = source_projection();
        let authority = source_authority(&projection);
        let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
        Self {
            context,
            device,
            hold,
            hsaco,
            storage: projection.into_generated_storage_v1(),
            authority,
            roster,
        }
    }

    fn install(&mut self) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let mut source =
            RuntimeGfx942GeneratedSourceMutV1::new(&mut self.storage, &self.hsaco, &self.authority);
        self.context.install_generated_shells_v1(
            self.device,
            admission(1, 1).1,
            &self.hold,
            &mut source,
            &self.roster,
        )
    }

    fn snapshot(&self) -> Snapshot {
        let mut ids = self.context.allocations.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        Snapshot {
            ids,
            handles: self.context.backend_allocations.clone(),
            next: self.context.next_identity,
            generated: self.context.streams[&self.hold.stream()].generated,
            usage: self
                .context
                .allocation_admission_usage_v1(self.device)
                .unwrap(),
        }
    }

    fn retire(&mut self) {
        self.context.retire_generated_shells_v1(&self.hold).unwrap();
        self.context
            .release_unpublished_hold_v1(&self.hold)
            .unwrap();
    }
}

#[test]
fn shell_registration_retains_exact_roster_and_refunds_only_after_retirement() {
    for configured in [false, true] {
        let mut fixture = Fixture::new(configured.then_some((60, 3)));
        let initial = fixture.snapshot();
        let pointers = fixture
            .storage
            .buffers()
            .iter()
            .map(|buffer| buffer.bytes().as_ptr())
            .collect::<Vec<_>>();
        fixture.install().unwrap();
        assert!(!fixture.storage.control_available());
        assert_eq!(fixture.context.allocations.len(), 3);
        let after = fixture.snapshot();
        assert_eq!(after.next, initial.next + 3);
        let lengths = after
            .ids
            .iter()
            .map(|id| fixture.context.allocations[id].byte_len)
            .collect::<Vec<_>>();
        assert_eq!(lengths, [16, 20, 24]);
        if let Some(usage) = after.usage {
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
        assert!(
            fixture
                .context
                .release_unpublished_hold_v1(&fixture.hold)
                .is_err()
        );
        assert!(fixture.install().is_err());
        assert_eq!(fixture.snapshot(), after);
        fixture
            .context
            .retire_generated_shells_v1(&fixture.hold)
            .unwrap();
        assert_eq!(
            fixture
                .context
                .allocation_admission_usage_v1(fixture.device)
                .unwrap(),
            initial.usage
        );
        assert!(fixture.context.allocations.is_empty());
        assert!(fixture.context.backend_allocations.is_empty());
        assert_eq!(fixture.context.next_identity, after.next);
        assert!(
            fixture
                .context
                .require_stream_unheld_v1(fixture.hold.stream())
                .is_err()
        );
        assert!(
            fixture
                .context
                .retire_generated_shells_v1(&fixture.hold)
                .is_err()
        );
        fixture
            .context
            .release_unpublished_hold_v1(&fixture.hold)
            .unwrap();
        assert!(
            fixture
                .context
                .release_unpublished_hold_v1(&fixture.hold)
                .is_err()
        );
        assert_eq!(
            fixture
                .storage
                .buffers()
                .iter()
                .map(|buffer| buffer.bytes().as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
        assert!(!fixture.storage.control_available());
    }
}

#[test]
fn shell_credit_exhaustion_rejects_the_whole_roster_without_transfer() {
    for limits in [(59, 3), (60, 2)] {
        let mut fixture = Fixture::new(Some(limits));
        let before = fixture.snapshot();
        assert!(matches!(
            fixture.install(),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
        assert_eq!(fixture.snapshot(), before);
        assert!(fixture.storage.control_available());
        fixture
            .context
            .release_unpublished_hold_v1(&fixture.hold)
            .unwrap();
    }
}

#[test]
fn shell_context_identity_exhaustion_is_atomic_and_adjacent_range_succeeds() {
    let mut fixture = Fixture::new(Some((60, 3)));
    fixture.context.next_identity = u64::MAX - 2;
    let before = fixture.snapshot();
    assert!(fixture.install().is_err());
    assert_eq!(fixture.snapshot(), before);
    assert!(fixture.storage.control_available());
    fixture.context.next_identity = u64::MAX - 3;
    fixture.install().unwrap();
    assert_eq!(fixture.context.next_identity, u64::MAX);
    fixture.retire();
}

#[test]
fn shell_retirement_preserves_unrelated_ordinary_bytes_and_credits() {
    let mut fixture = Fixture::new(Some((64, 4)));
    let ordinary = fixture
        .context
        .allocate(fixture.device, RuntimeMemoryKindV1::HostVisible, 4, 4)
        .unwrap();
    fixture
        .context
        .write_allocation(ordinary, 0, &[1, 2, 3, 4])
        .unwrap();
    let before = fixture.snapshot();
    fixture.install().unwrap();
    for id in fixture
        .snapshot()
        .ids
        .into_iter()
        .filter(|id| *id != ordinary)
    {
        let usage = fixture
            .context
            .allocation_admission_usage_v1(fixture.device)
            .unwrap();
        let mut destination = [0xa5; 4];
        assert!(
            fixture
                .context
                .read_allocation(id, 0, &mut destination)
                .is_err()
        );
        assert_eq!(destination, [0xa5; 4]);
        assert!(fixture.context.write_allocation(id, 0, &[9; 4]).is_err());
        assert!(fixture.context.release_allocation(id).is_err());
        assert_eq!(
            fixture
                .context
                .allocation_admission_usage_v1(fixture.device)
                .unwrap(),
            usage
        );
    }
    fixture.retire();
    assert_eq!(
        fixture
            .context
            .allocation_admission_usage_v1(fixture.device)
            .unwrap(),
        before.usage
    );
    assert_eq!(fixture.context.allocations.len(), 1);
    let mut bytes = [0; 4];
    fixture
        .context
        .read_allocation(ordinary, 0, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
    fixture.context.release_allocation(ordinary).unwrap();
    assert_eq!(
        fixture
            .context
            .allocation_admission_usage_v1(fixture.device)
            .unwrap()
            .unwrap()
            .used,
        crate::RuntimeResourceVectorV1::ZERO
    );
}

#[test]
fn shell_source_substitution_preserves_both_control_owners() {
    let mut fixture = Fixture::new(Some((60, 3)));
    let (_, substitute) = source_projection();
    assert_eq!(
        substitute.dispatch_contract_sha256(),
        fixture.storage.dispatch_contract_sha256()
    );
    let original = std::mem::replace(&mut fixture.storage, substitute.into_generated_storage_v1());
    let before = fixture.snapshot();
    assert!(fixture.install().is_err());
    assert_eq!(fixture.snapshot(), before);
    assert!(fixture.storage.control_available());
    assert!(original.control_available());
    fixture.storage = original;
    fixture.install().unwrap();
    fixture.retire();
}

#[test]
fn shell_foreign_and_stale_holds_cannot_change_current_ownership() {
    let mut fixture = Fixture::new(Some((60, 3)));
    let foreign = Fixture::new(None);
    let before = fixture.snapshot();
    let mut source = RuntimeGfx942GeneratedSourceMutV1::new(
        &mut fixture.storage,
        &fixture.hsaco,
        &fixture.authority,
    );
    assert!(
        fixture
            .context
            .install_generated_shells_v1(
                fixture.device,
                admission(1, 1).1,
                &foreign.hold,
                &mut source,
                &fixture.roster
            )
            .is_err()
    );
    assert_eq!(fixture.snapshot(), before);
    fixture
        .context
        .release_unpublished_hold_v1(&fixture.hold)
        .unwrap();
    let fresh = fixture
        .context
        .hold_unpublished_stream_v1(fixture.hold.stream())
        .unwrap();
    let old = std::mem::replace(&mut fixture.hold, fresh);
    fixture.install().unwrap();
    let current = fixture.snapshot();
    assert!(fixture.context.retire_generated_shells_v1(&old).is_err());
    assert!(fixture.context.release_unpublished_hold_v1(&old).is_err());
    assert_eq!(fixture.snapshot(), current);
    fixture.retire();
}

#[test]
fn shell_batch_drop_returns_all_unissued_reservations() {
    let mut fixture = Fixture::new(Some((60, 3)));
    let before = fixture.snapshot();
    let batch = fixture
        .context
        .allocation_admission
        .prepare_roster(fixture.device, &[16, 20, 24])
        .unwrap()
        .unwrap();
    let usage = fixture
        .context
        .allocation_admission_usage_v1(fixture.device)
        .unwrap()
        .unwrap();
    assert_eq!(usage.reserved_records, 3);
    assert_eq!(usage.retained_records, 0);
    assert!(fixture.context.allocations.is_empty());
    drop(batch);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn shell_late_context_roster_mismatch_preserves_control_and_all_credits() {
    for missing_handle in [false, true] {
        let mut fixture = Fixture::new(Some((60, 3)));
        fixture.install().unwrap();
        let installed = fixture.snapshot();
        let last = *installed.ids.last().unwrap();
        let record = fixture.context.allocations[&last];
        if missing_handle {
            fixture
                .context
                .backend_allocations
                .remove(&record.backend_allocation);
        } else {
            fixture.context.allocations.get_mut(&last).unwrap().byte_len += 1;
        }
        let malformed = fixture.snapshot();
        assert!(
            fixture
                .context
                .retire_generated_shells_v1(&fixture.hold)
                .is_err()
        );
        assert_eq!(fixture.snapshot(), malformed);
        assert!(
            fixture
                .context
                .backend
                .generated_shell_plan_v1(
                    installed.generated.unwrap(),
                    fixture.context.context_generation,
                    fixture.hold.stream(),
                    fixture.hold.identity()
                )
                .is_some()
        );
        fixture.context.allocations.insert(last, record);
        fixture
            .context
            .backend_allocations
            .insert(record.backend_allocation);
        assert_eq!(fixture.snapshot(), installed);
        fixture.retire();
    }
}

#[test]
fn shell_pure_ingress_rejects_before_mutable_carrier_callback() {
    struct Carrier;
    impl crate::RuntimeGfx942GeneratedCarrierV1 for Carrier {
        type CurrentnessError = ();
        type Readback = ();
        fn source(&self) -> crate::RuntimeGfx942GeneratedSourceV1<'_, ()> {
            panic!("unexpected source callback")
        }
        fn source_mut(&mut self) -> Option<crate::RuntimeGfx942GeneratedSourceMutV1<'_, ()>> {
            panic!("unexpected mutable source callback")
        }
        fn prepare_readback(&self) -> Result<(), crate::RuntimeGfx942ReadbackErrorV1> {
            panic!("unexpected readback callback")
        }
        fn install_readback(&mut self, _: ()) {
            panic!("unexpected install callback")
        }
    }
    for already_registered in [false, true] {
        let mut fixture = Fixture::new(None);
        let mut prepared = bound(&fixture.context, Carrier);
        if already_registered {
            fixture
                .context
                .streams
                .get_mut(&fixture.hold.stream())
                .unwrap()
                .generated = Some(99);
        } else {
            prepared.binding.device =
                RuntimeDeviceIdV1::new(fixture.context.context_generation, u64::MAX);
        }
        assert!(matches!(
            fixture.context.register_gfx942_generated_shells_v1(
                &mut prepared,
                &fixture.hold,
                &fixture.roster
            ),
            Err(crate::RuntimeGfx942GeneratedReservationErrorV1::Context(
                RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)
            ))
        ));
        assert!(!fixture.context.terminal);
        assert!(fixture.storage.control_available());
    }
}
