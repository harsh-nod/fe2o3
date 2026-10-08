//! Original primary construction with captured fill code and CPU native adapters.

use super::*;
use crate::queue::dispatch_binding::{Gfx942NativeFillCohortMemberV1, Gfx942NativeFillCohortV1};
use crate::shared_memory::{PreparationMemoryCallV1 as Call, PreparationNativeFaultV1 as Fault};
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
use fe2o3_hsaco::ArgumentAccess;
use fe2o3_kernel_analysis::PhysicalMachineEffectRequestV1;

type CohortRoot<'a> = Root<(
    Vec<ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<3>,
)>;

struct Original {
    root: usize,
    programs: usize,
    capacity: usize,
    bytes: usize,
    length: usize,
}

pub(super) fn payload() -> PhysicalMachineEffectRequestV1 {
    PhysicalMachineEffectRequestV1::decode_canonical(include_bytes!(
        "../../../../fe2o3-kernel-analysis/src/gfx942_fill_analysis_v1/fill.request"
    ))
    .unwrap()
}

pub(super) fn program(bytes: &[u8]) -> ValidatedKernelEnvelope<'_> {
    let kernel = validate(bytes, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("fill_write_only")
        .unwrap();
    let name = kernel.selected_kernel().explicit_arguments()[0]
        .name()
        .unwrap()
        .to_owned();
    kernel
        .reconcile_dispatch_abi(
            [0x51; 32],
            &[KernelGlobalBufferAbiV1::new(
                0,
                &name,
                0,
                4,
                ArgumentAccess::WriteOnly,
            )],
        )
        .unwrap()
}

pub(super) fn packet(
    count: u64,
    grid: u32,
    program_index: usize,
    data_index: usize,
    conditional: bool,
) -> Gfx942FixedDispatchPacketV1 {
    let mut kernarg = vec![0; 272];
    kernarg[8..16].copy_from_slice(&count.to_le_bytes());
    let packet = Gfx942FixedDispatchPacketV1::new(
        program_index,
        AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]).unwrap(),
        0,
        kernarg.into_boxed_slice(),
        Box::new([Gfx942DispatchBufferBindingV1::new(
            0,
            data_index,
            0,
            count * 4,
        )]),
    );
    if conditional {
        packet.require_conditional_fill_v1()
    } else {
        packet
    }
}

fn setup_cohort<'a>(
    bytes: &'a [u8],
    mutate: impl FnOnce(&mut Gfx942NativeFillCohortV1<'a, 3>),
) -> (Box<CohortRoot<'a>>, Rc<RefCell<Trace>>, Original) {
    let (mut memory, trace) = setup_memory();
    let members = std::array::from_fn(|index| {
        let count = 1 + index as u64 * 64;
        let packet = packet(count, (index as u32 + 1) * 64, 0, 0, true);
        let token = memory
            .allocate::<HostVisibleCoherentGttV1>(count as usize * 4)
            .unwrap();
        let data =
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap());
        Gfx942NativeFillCohortMemberV1::new(program(bytes), packet, data)
    });
    let mut cohort = Gfx942NativeFillCohortV1::admit(members).unwrap();
    mutate(&mut cohort);
    let Gfx942NativeFillCohortV1 {
        programs,
        packets,
        data,
    } = cohort;
    let custody = FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data);
    trace.borrow_mut().initial_data = Some(memory.observation());
    trace.borrow_mut().initial_preparation = Some(
        custody
            .primary_snapshot_v1()
            .with_expected_write_only_ranges_v1(&[(0, 4), (0, 260), (0, 516)]),
    );
    let root = Root::new_with(memory, (programs, custody));
    let original = Original {
        root: &*root as *const CohortRoot<'_> as usize,
        programs: root.preparation.0.as_ptr() as usize,
        capacity: root.preparation.0.capacity(),
        bytes: bytes.as_ptr() as usize,
        length: bytes.len(),
    };
    (root, trace, original)
}

fn assert_owners(root: &CohortRoot<'_>, original: &Original, trace: &Rc<RefCell<Trace>>) {
    assert_common(root, original.root, trace);
    assert_platform(root, None);
    assert_eq!(root.preparation.0.len(), 3);
    assert_eq!(root.preparation.0.as_ptr() as usize, original.programs);
    assert_eq!(root.preparation.0.capacity(), original.capacity);
    for program in &root.preparation.0 {
        assert_eq!(program.envelope().bytes().as_ptr() as usize, original.bytes);
        assert_eq!(program.envelope().bytes().len(), original.length);
    }
    let dispatch = root
        .dispatch
        .as_ref()
        .or_else(|| root.completed.as_ref().and_then(|c| c.dispatch.as_ref()));
    root.preparation.1.primary_assert_snapshot_v1(
        memory(root),
        trace.borrow().initial_preparation.as_ref().unwrap(),
        dispatch,
    );
    if let Some(dispatch) = dispatch {
        let identities = dispatch.primary_fixture_identities_v1();
        // Three original outputs, three independently retained code images and
        // one whole-cohort kernarg allocation, plus exact queue-resource owners.
        assert_eq!(identities.len(), 7);
        assert_partition(root, identities);
    } else {
        assert!(root.ring.is_none());
    }
}

#[test]
fn native_fill_cohort_primary_constructs_three_originals_without_singleton_fallback() {
    let captured = payload();
    let (root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
    let (mut root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_cohort(entry, 4096)
    });
    assert!(result.is_ok());
    assert_owners(&root, &original, &trace);
    assert!(!trace.borrow().poison);
    assert_eq!(
        trace
            .borrow()
            .calls
            .iter()
            .filter(|&&c| c == "create")
            .count(),
        1
    );
    let complete = root.completed.as_mut().unwrap();
    let key = complete.engine.resources[0].key;
    let dispatch = complete.dispatch.as_mut().unwrap();
    let before = dispatch.source_failure_snapshot_v1();
    assert!(dispatch.bind_templates::<1>(key).is_err());
    assert_eq!(dispatch.source_failure_snapshot_v1(), before);
    let (templates, epoch) = dispatch.bind_templates::<3>(key).unwrap();
    assert_eq!(templates.len(), 3);
    assert!(dispatch.bind_templates::<3>(key).is_err());
    dispatch.cancel_binding(epoch).unwrap();
    // This exercises original CPU model custody, not native queue publication.
    assert_owners(&root, &original, &trace);
}

#[test]
fn native_fill_cohort_primary_preentry_ring_and_capacity_refusals_retain_every_input() {
    for invalid_ring in [false, true] {
        let captured = payload();
        let (mut root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
        let account = if invalid_ring {
            None
        } else {
            let (capacity, account) = capacity_cases::capacity();
            root.dispatch_capacity = capacity;
            Some(account)
        };
        let before = memory(&root).observation();
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_cohort(entry, if invalid_ring { 64 } else { 4096 })
        });
        assert!(result.is_err());
        assert_owners(&root, &original, &trace);
        assert_eq!(memory(&root).observation(), before);
        assert_eq!(trace.borrow().calls, ["retain-root"]);
        assert!(!trace.borrow().poison);
        if let Some(account) = account {
            assert_eq!(
                account.usage().used,
                fe2o3_resource_accounting::ResourceVectorV1::ZERO
            );
            assert_eq!(account.usage().retained_records, 0);
        }
    }
}

#[test]
fn native_fill_cohort_primary_rechecks_member_bindings_before_native_preparation() {
    for case in 0..3 {
        let captured = payload();
        let (root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |cohort| {
            cohort.packets[2] = match case {
                0 => packet(129, 192, 0, 2, true),
                1 => packet(129, 192, 2, 2, false),
                2 => packet(130, 192, 2, 2, true),
                _ => unreachable!(),
            };
        });
        let before = memory(&root).observation();
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_cohort(entry, 4096)
        });
        assert!(result.is_err());
        assert_owners(&root, &original, &trace);
        assert_eq!(memory(&root).observation(), before);
        assert_eq!(
            trace.borrow().calls,
            ["plan-auxiliary-resources", "retain-root"]
        );
        assert!(!trace.borrow().poison);
    }
}

#[test]
fn native_fill_cohort_primary_partial_preparation_errors_and_panics_keep_exact_prefix() {
    let mut stages = vec![
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Capacity,
        PreparationStageV1::DataRetention,
        PreparationStageV1::KernargAllocate,
        PreparationStageV1::KernargMaterialize,
        PreparationStageV1::KernargMap,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::Commit,
        PreparationStageV1::Complete,
    ];
    for index in 0..3 {
        stages.extend([
            PreparationStageV1::CodeAllocate(index),
            PreparationStageV1::CodeMaterialize(index),
            PreparationStageV1::CodeSeal(index),
            PreparationStageV1::CodeMap(index),
            PreparationStageV1::CodeRetain(index),
            PreparationStageV1::CodeResolve(index),
            PreparationStageV1::PacketResolve(index),
        ]);
    }
    for stage in stages {
        for panics in [false, true] {
            let captured = payload();
            let (mut root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
            root.preparation.1.primary_inject_stage_v1(stage, panics);
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_cohort(entry, 4096)
            });
            let error = result.expect_err("injected preparation boundary must be exercised");
            if panics {
                assert_eq!(
                    error.downcast_ref::<(&str, PreparationStageV1)>(),
                    Some(&("dispatch preparation", stage))
                );
            } else {
                assert!(error.is::<ComputeAqlQueueSessionErrorV1>());
            }
            root.preparation.1.primary_assert_failed_stage_v1(stage);
            assert_owners(&root, &original, &trace);
            assert_eq!(
                trace.borrow().calls,
                ["plan-auxiliary-resources", "retain-root"]
            );
            assert!(!trace.borrow().poison);
        }
    }
}

#[test]
fn native_fill_cohort_primary_native_preparation_currentness_and_partial_maps_retain_custody() {
    for call in [
        Call::MapCode(0),
        Call::MapCode(1),
        Call::MapCode(2),
        Call::MapKernarg,
    ] {
        for fault in [
            Fault::Error("map_gpu"),
            Fault::Panic("map_gpu"),
            Fault::CurrentnessError(1),
            Fault::CurrentnessPanic(1),
            Fault::CurrentnessError(2),
            Fault::CurrentnessPanic(2),
            Fault::PartialMap(0, true),
            Fault::PartialMap(1, true),
            Fault::PartialMap(2, false),
        ] {
            let captured = payload();
            let (mut root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
            root.memory.as_mut().unwrap().fault = Some((call, fault));
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_cohort(entry, 4096)
            });
            let error = result.expect_err("injected native preparation fault must be exercised");
            match fault {
                Fault::Panic(operation) => assert_eq!(
                    error.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                ),
                Fault::CurrentnessPanic(_) => assert_eq!(
                    error.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                ),
                _ => assert!(error.is::<ComputeAqlQueueSessionErrorV1>()),
            }
            assert_owners(&root, &original, &trace);
            memory(&root).primary_assert_preparation_fault_v1(call, fault);
            assert!(!trace.borrow().calls.contains(&"allocate-ring"));
        }
    }
}

#[test]
fn native_fill_cohort_primary_queue_errors_panics_and_currentness_keep_all_three_outputs() {
    let mut cases = vec![
        ("plan-auxiliary-resources", 1),
        ("allocate-ring", 1),
        ("allocate-control", 1),
        ("event", 1),
        ("shadow-install", 1),
        ("foundation", 1),
        ("recover-outputs", 1),
        ("doorbell", 1),
        ("gate-finish", 1),
    ];
    cases.extend((1..=8).map(|index| ("currentness", index)));
    for (name, occurrence) in cases {
        for panics in [false, true] {
            let captured = payload();
            let (root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
            trace.borrow_mut().fault = Some((name, occurrence, panics));
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_cohort(entry, 4096)
            });
            let error = result.expect_err("injected queue boundary must be exercised");
            if panics {
                assert_eq!(
                    error.downcast_ref::<(&str, usize)>(),
                    Some(&(name, occurrence))
                );
            } else {
                assert!(error.is::<ComputeAqlQueueSessionErrorV1>());
            }
            assert_owners(&root, &original, &trace);
            assert_eq!(
                trace.borrow().poison,
                !matches!(name, "plan-auxiliary-resources" | "allocate-ring")
            );
        }
    }
}

#[test]
fn native_fill_cohort_primary_uncertain_create_keeps_original_queue_and_no_success() {
    for mode in 1..=5 {
        let captured = payload();
        let (root, trace, original) = setup_cohort(captured.exact_payload_bytes(), |_| {});
        trace.borrow_mut().create = mode;
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_cohort(entry, 4096)
        });
        assert!(result.is_err());
        assert_owners(&root, &original, &trace);
        let trace = trace.borrow();
        assert!(trace.poison);
        assert_eq!(trace.cleanup, 0);
        assert!(!trace.calls.contains(&"doorbell"));
        assert!(root.completed.is_none());
        assert!(
            root.engine.as_ref().unwrap().resources[0]
                .authority
                .is_some()
        );
    }
}
