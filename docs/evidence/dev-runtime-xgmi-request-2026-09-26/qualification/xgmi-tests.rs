use super::*;
use crate::{
    RuntimeAllocationDeviceAdmissionV1 as Entry, RuntimeContextV1, RuntimeResourceKindV1 as K,
    RuntimeResourceVectorV1 as V,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn pair() -> (Gfx942ComposedBackingRootV1, [Entry; 2]) {
    let root = Entry::qualification_root_v1();
    let entries = [
        Entry::qualification_entry_v1(&root, 19),
        Entry::qualification_entry_v1(&root, 7),
    ];
    (root, entries)
}

fn policy(entries: &[Entry; 2]) -> RequestPolicyV1 {
    RequestPolicyV1::from_bindings(
        [19, 7],
        [Some(entries[0].clone()), Some(entries[1].clone())],
    )
    .unwrap()
}

fn assert_clear(entry: &Entry) {
    let usage = entry.account().usage_v1();
    assert_eq!(usage.used, V::ZERO);
    assert_eq!(
        (
            usage.reserved_records,
            usage.retained_records,
            usage.quarantined_records
        ),
        (0, 0, 0)
    );
}

#[test]
fn qualification_xgmi_policy_preserves_order_and_rejects_partial_alias_and_swapped_rosters() {
    let (_root, entries) = pair();
    let valid = policy(&entries);
    let RuntimeAllocationAdmissionProfileV1::Required(roster) = valid.profile([19, 7]).unwrap()
    else {
        panic!("lost required profile");
    };
    assert_eq!(roster.len(), 2);
    for (actual, expected) in roster.iter().zip(&entries) {
        assert_eq!(actual.backend_device_v1(), expected.backend_device_v1());
        assert_eq!(actual.model(), expected.model());
        assert!(actual.account().shares_account_with_v1(expected.account()));
    }
    assert!(valid.profile([7, 19]).is_err());
    let alias = Entry::qualification_v1(7, entries[0].model(), entries[0].account().clone());
    for bindings in [
        [Some(entries[0].clone()), None],
        [None, Some(entries[1].clone())],
        [Some(entries[1].clone()), Some(entries[0].clone())],
        [Some(entries[0].clone()), Some(entries[0].clone())],
        [Some(entries[0].clone()), Some(alias)],
    ] {
        assert!(RequestPolicyV1::from_bindings([19, 7], bindings).is_err());
    }
}

#[test]
fn qualification_xgmi_witness_rejection_precedes_record_reservation_id_and_operation() {
    let (_root, entries) = pair();
    let valid = policy(&entries);
    let other_root = Entry::qualification_root_v1();
    let foreign = Entry::qualification_entry_v1(&other_root, 19);
    let wrong_generation = Entry::qualification_v1(
        19,
        Entry::qualification_model_v1(2),
        entries[0].account().clone(),
    );
    let context = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id();
    for (entry, selected, witness_bytes, requested_bytes, credit_bytes) in [
        (&entries[0], 7, 16, 16, 16),
        (&entries[1], 19, 16, 16, 16),
        (&foreign, 19, 16, 16, 16),
        (&wrong_generation, 19, 16, 16, 16),
        (&entries[0], 19, 17, 16, 17),
        (&entries[0], 19, 16, 16, 17),
    ] {
        let credit = entry.account().reserve_v1(credit_bytes).unwrap().retain();
        let before = entry.account().usage_v1();
        let mut terminal = false;
        let mut next = 1;
        let mut records = HashMap::<u64, Box<u64>>::new();
        let mut calls = 0;
        let witness = entry.qualification_witness_v1(device, &credit, witness_bytes);
        let result = valid
            .allocation_endpoint([19, 7], selected, requested_bytes, Some(witness))
            .and_then(|_| {
                xgmi_budget::allocate_record(
                    &mut terminal,
                    &mut next,
                    &mut records,
                    || {
                        calls += 1;
                        Ok::<_, &str>(Box::new(1))
                    },
                    |_| unreachable!(),
                )
            });
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))));
        assert_eq!(
            (next, records.len(), records.capacity(), calls, terminal),
            (1, 0, 0, 0, false)
        );
        assert_eq!(entry.account().usage_v1(), before);
        credit.release_after_rejection().unwrap();
    }
    for selected in [19, 7] {
        assert!(
            matches!(valid.allocation_endpoint([19, 7], selected, 16, None),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
        );
    }
    let credit = entries[0].account().reserve_v1(16).unwrap().retain();
    assert!(
        RequestPolicyV1::Legacy
            .allocation_endpoint(
                [19, 7],
                19,
                16,
                Some(entries[0].qualification_witness_v1(device, &credit, 16))
            )
            .is_err()
    );
    credit.release_after_rejection().unwrap();
}

#[test]
fn qualification_xgmi_invalid_roster_never_enters_acquisition() {
    let (root, entries) = pair();
    let before = root.usage_v1();
    let alias = Entry::qualification_v1(7, entries[0].model(), entries[0].account().clone());
    for bindings in [
        [Some(entries[1].clone()), Some(entries[0].clone())],
        [Some(entries[0].clone()), Some(entries[0].clone())],
        [Some(entries[0].clone()), None],
        [None, Some(entries[1].clone())],
        [Some(entries[0].clone()), Some(alias)],
    ] {
        let mut calls = 0;
        let result = xgmi_budget::bind_before_acquire(
            [19, 7],
            bindings,
            |keys, bindings| RequestPolicyV1::from_bindings(keys.map(|key| *key), bindings.clone()),
            |_, _| {
                calls += 1;
                Ok::<_, KfdRuntimeBackendErrorV1>(())
            },
        );
        assert!(result.is_err());
        assert_eq!(calls, 0);
        assert_eq!(root.usage_v1(), before);
        for entry in &entries {
            assert_clear(entry);
        }
    }
}

#[test]
fn qualification_xgmi_selected_session_health_is_not_silent_roster_truncation() {
    let (_root, entries) = pair();
    let valid = policy(&entries);
    let context = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    let device = context.devices()[0].id();
    let retained = entries[1].account().reserve_v1(16).unwrap().retain();
    entries[1]
        .account()
        .reserve_v1(1)
        .unwrap()
        .retain()
        .quarantine();
    assert!(valid.profile([19, 7]).is_err());
    assert!(
        RequestPolicyV1::from_bindings(
            [19, 7],
            [Some(entries[0].clone()), Some(entries[1].clone())]
        )
        .is_err()
    );
    assert!(
        valid
            .allocation_endpoint(
                [19, 7],
                7,
                16,
                Some(entries[1].qualification_witness_v1(device, &retained, 16))
            )
            .is_err()
    );
    retained.release_after_rejection().unwrap();
    let healthy = entries[0].account().reserve_v1(16).unwrap().retain();
    assert_eq!(
        valid
            .allocation_endpoint(
                [19, 7],
                19,
                16,
                Some(entries[0].qualification_witness_v1(device, &healthy, 16))
            )
            .unwrap(),
        0
    );
    healthy.release_after_rejection().unwrap();
    assert_clear(&entries[0]);
    assert_eq!(entries[1].account().usage_v1().quarantined_records, 1);
}

// A CPU policy/transaction adapter, not a native XGMI backend or session.
// Context executes the production policy and record transaction with Box owners.
struct PolicyBackend {
    policy: RequestPolicyV1,
    descriptions: [BackendDeviceDescriptionV1; 2],
    records: HashMap<u64, Box<(usize, u64)>>,
    next: u64,
    terminal: bool,
    outcome: u8,
    calls: usize,
}

impl PolicyBackend {
    fn new(policy: RequestPolicyV1) -> Self {
        let mut first = KfdRuntimeBackendV1::mock().description.clone();
        first.capabilities = crate::RuntimeCapabilitiesV1 {
            device_memory: true,
            multi_device: true,
            ..crate::RuntimeCapabilitiesV1::default()
        };
        let second = first.clone();
        first.backend_device = 19;
        Self {
            policy,
            descriptions: [first, second],
            records: HashMap::new(),
            next: 1,
            terminal: false,
            outcome: 0,
            calls: 0,
        }
    }
    fn allocate(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
        witness: Option<RuntimeAllocationRequestWitnessV1<'_>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let index = self
            .policy
            .allocation_endpoint([19, 7], device, bytes, witness)?;
        if kind != RuntimeMemoryKindV1::DeviceLocal || bytes == 0 || !alignment.is_power_of_two() {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "fixture geometry",
            ));
        }
        xgmi_budget::allocate_record(
            &mut self.terminal,
            &mut self.next,
            &mut self.records,
            || {
                self.calls += 1;
                match self.outcome {
                    0 => Ok(Box::new((index, bytes))),
                    1 => Err("capacity"),
                    2 => Err("terminal"),
                    3 => panic!("operation panic"),
                    _ => unreachable!(),
                }
            },
            |error| {
                if *error == "capacity" {
                    fe2o3_kfd::Gfx942XgmiAllocationDispositionV1::RejectedCapacity
                } else {
                    fe2o3_kfd::Gfx942XgmiAllocationDispositionV1::ProcessTeardown
                }
            },
        )
    }
}

macro_rules! unsupported {
    ($name:ident($($arg:ident: $ty:ty),*) -> $result:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> Result<$result, RuntimeBackendFailureV1<Self::Error>> {
            $(let _ = $arg;)*
            Err(KfdRuntimeBackendV1::rejected(KfdRuntimeBackendErrorKindV1::Unsupported, "fixture operation"))
        }
    };
}

impl RuntimeBackendV1 for PolicyBackend {
    type Error = KfdRuntimeBackendErrorV1;
    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        Ok(self.descriptions.to_vec())
    }
    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<RuntimeAllocationAdmissionProfileV1, RuntimeBackendFailureV1<Self::Error>> {
        self.policy.profile([19, 7])
    }
    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.allocate(device, kind, bytes, alignment, None)
    }
    fn allocate_with_request_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        bytes: u64,
        alignment: u64,
        witness: RuntimeAllocationRequestWitnessV1<'_>,
    ) -> crate::RuntimeRequestAllocationResultV1<Self::Error> {
        crate::RuntimeRequestAllocationResultV1::Outcome(
            self.allocate(device, kind, bytes, alignment, Some(witness))
                .map(RuntimeBackendAllocationOutcomeV1::Allocated),
        )
    }
    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        assert!(!self.terminal);
        assert!(self.records.remove(&allocation).is_some());
        Ok(())
    }
    unsupported!(create_stream_v1(device: u64) -> u64);
    unsupported!(destroy_stream_v1(stream: u64) -> ());
    unsupported!(write_allocation_v1(allocation: u64, offset: u64, bytes: &[u8]) -> ());
    unsupported!(read_allocation_v1(allocation: u64, offset: u64, bytes: &mut [u8]) -> ());
    unsupported!(load_module_v1(device: u64, image: &[u8]) -> u64);
    unsupported!(unload_module_v1(module: u64) -> ());
    unsupported!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64);
    unsupported!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    unsupported!(poll_v1(submission: u64) -> BackendPollV1);
    unsupported!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
    unsupported!(release_submission_v1(submission: u64) -> ());
    unsupported!(record_event_v1(stream: u64, submission: u64) -> u64);
    unsupported!(release_event_v1(event: u64) -> ());
    unsupported!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64);
}

#[test]
fn qualification_xgmi_context_adapter_refunds_classified_capacity_and_releases_exact_requests() {
    let (root, entries) = pair();
    let baseline = root.usage_v1();
    let mut backend = PolicyBackend::new(policy(&entries));
    backend.outcome = 1;
    let mut context = RuntimeContextV1::open(backend).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for (kind, bytes, alignment) in [
        (RuntimeMemoryKindV1::HostVisible, 16, 8),
        (RuntimeMemoryKindV1::DeviceLocal, 0, 8),
        (RuntimeMemoryKindV1::DeviceLocal, 16, 0),
        (RuntimeMemoryKindV1::DeviceLocal, 16, 3),
    ] {
        assert!(
            context
                .allocate(devices[0], kind, bytes, alignment)
                .is_err()
        );
        assert_eq!(context.backend_mut_for_test_v1().calls, 0);
        assert_eq!(context.backend_mut_for_test_v1().next, 1);
        assert_eq!(root.usage_v1(), baseline);
    }
    assert!(
        matches!(context.allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 16, 8),
        Err(crate::RuntimeErrorV1::BackendRejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(context.backend_mut_for_test_v1().next, 2);
    assert!(!context.is_terminal());
    for entry in &entries {
        assert_clear(entry);
    }
    assert_eq!(root.usage_v1(), baseline);
    context.backend_mut_for_test_v1().outcome = 0;
    let left = context
        .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    let right = context
        .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 24, 8)
        .unwrap();
    assert_eq!(
        entries[0].account().usage_v1().used,
        V::ZERO
            .with(K::RequestedAllocationBytes, 16)
            .with(K::AllocationRecords, 1)
    );
    assert_eq!(
        entries[1].account().usage_v1().used,
        V::ZERO
            .with(K::RequestedAllocationBytes, 24)
            .with(K::AllocationRecords, 1)
    );
    assert_eq!(*context.backend_mut_for_test_v1().records[&2], (0, 16));
    assert_eq!(*context.backend_mut_for_test_v1().records[&3], (1, 24));
    context.release_allocation(left).unwrap();
    assert_clear(&entries[0]);
    assert_eq!(entries[1].account().usage_v1().retained_records, 1);
    context.release_allocation(right).unwrap();
    let mut backend = context.shutdown().unwrap();
    assert!(backend.records.is_empty());
    for entry in &entries {
        assert_clear(entry);
    }
    assert_eq!(root.usage_v1(), baseline);
    assert!(
        backend
            .allocate_v1(19, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .is_err()
    );
    assert!(matches!(
        backend.allocation_admission_profile_v1().unwrap(),
        RuntimeAllocationAdmissionProfileV1::Required(_)
    ));
}

#[test]
fn qualification_xgmi_context_adapter_terminal_and_panic_quarantine_only_attempted_request() {
    for outcome in [2, 3] {
        let observe = {
            let (root, entries) = pair();
            let mut backend = PolicyBackend::new(policy(&entries));
            backend.outcome = outcome;
            let mut context = RuntimeContextV1::open(backend).unwrap();
            let device = context.devices()[1].id();
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 17, 1)
            }));
            if outcome == 2 {
                assert!(matches!(
                    result,
                    Ok(Err(crate::RuntimeErrorV1::BackendTerminal(_)))
                ));
            } else {
                assert!(result.is_err());
            }
            assert!(context.is_terminal());
            assert!(context.backend_mut_for_test_v1().terminal);
            assert!(context.backend_mut_for_test_v1().records.is_empty());
            assert_eq!(context.backend_mut_for_test_v1().calls, 1);
            assert_clear(&entries[0]);
            assert_eq!(entries[1].account().usage_v1().quarantined_records, 1);
            assert_eq!(
                entries[1].account().usage_v1().used,
                V::ZERO
                    .with(K::RequestedAllocationBytes, 17)
                    .with(K::AllocationRecords, 1)
            );
            drop(context);
            root.qualification_observer_v1()
        };
        let usage = observe().expect("quarantine retains exact root without ordinary owners");
        assert_eq!(usage.quarantined_records, 1);
        assert_eq!(usage.used.get(K::RequestedAllocationBytes), 17);
    }
}

#[test]
fn qualification_xgmi_policy_root_lifetime_survives_context_shutdown_until_backend_drop() {
    let (mut context, observe) = {
        let (root, entries) = pair();
        (
            RuntimeContextV1::open(PolicyBackend::new(policy(&entries))).unwrap(),
            root.qualification_observer_v1(),
        )
    };
    let device = context.devices()[0].id();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    assert_eq!(observe().unwrap().used.get(K::RequestedAllocationBytes), 16);
    context.release_allocation(allocation).unwrap();
    let backend = context.shutdown().unwrap();
    assert!(observe().is_some());
    drop(backend);
    assert!(observe().is_none());
}
