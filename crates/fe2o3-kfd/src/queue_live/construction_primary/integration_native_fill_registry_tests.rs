//! Actual rooted preparation and publication orchestration with CPU adapters.

use super::*;
use crate::queue::dispatch_binding::{
    Gfx942NativeFillCohortMemberV1, Gfx942NativeFillRegistryInputsV1,
    Gfx942NativeFillRegistryStorageV1,
};
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

type RegistryRoot<'a> = Root<(
    Vec<ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<4>,
    Gfx942NativeFillRegistryStorageV1,
)>;

fn setup_registry(
    bytes: &[u8],
) -> (
    Box<RegistryRoot<'_>>,
    Rc<RefCell<Trace>>,
    ResourceCreditAccountV1,
    usize,
) {
    let (mut memory, trace) = setup_memory();
    let members = std::array::from_fn(|index| {
        let count = [1, 37, 65, 129][index];
        let grid = [64, 64, 128, 192][index];
        let token = memory
            .allocate::<HostVisibleCoherentGttV1>(count * 4)
            .unwrap();
        Gfx942NativeFillCohortMemberV1::new(
            native_fill_cohort_cases::program(bytes),
            native_fill_cohort_cases::packet(count as u64, grid, 0, 0, true),
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory.map(token).unwrap()),
        )
    });
    let inputs = Gfx942NativeFillRegistryInputsV1::admit(members).unwrap();
    let crate::queue::dispatch_binding::Gfx942NativeFillCohortV1 {
        programs,
        packets,
        data,
    } = inputs.cohort;
    let custody = FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data);
    trace.borrow_mut().initial_data = Some(memory.observation());
    trace.borrow_mut().initial_preparation = Some(
        custody
            .primary_snapshot_v1()
            .with_expected_write_only_ranges_v1(&[(0, 4), (0, 148), (0, 260), (0, 516)]),
    );
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1 << 20),
        6,
    )
    .unwrap();
    let storage = Gfx942NativeFillRegistryStorageV1::preallocate(account.clone()).unwrap();
    let root = Root::new_with(memory, (programs, custody, storage));
    let pointer = &*root as *const RegistryRoot<'_> as usize;
    (root, trace, account, pointer)
}

fn assert_originals(
    root: &RegistryRoot<'_>,
    trace: &Rc<RefCell<Trace>>,
    pointer: usize,
    bytes: &[u8],
) {
    assert_common(root, pointer, trace);
    assert_platform(root, None);
    assert_eq!(root.preparation.0.len(), 4);
    for program in &root.preparation.0 {
        assert_eq!(program.envelope().bytes().as_ptr(), bytes.as_ptr());
        assert_eq!(program.envelope().bytes().len(), bytes.len());
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
        assert_eq!(identities.len(), 9);
        assert_partition(root, identities);
    }
}

#[test]
fn native_fill_registry_primary_retains_four_original_partitions_and_prepaid_tables() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
    let charged = account.usage();
    assert_eq!(charged.retained_records, 6);
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_registry(entry, 4096)
    });
    assert!(result.is_ok());
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
    assert!(root.preparation.2.settled());
    assert!(!trace.borrow().poison);
}

#[test]
fn native_fill_registry_primary_preentry_refusal_preserves_all_original_storage() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
    let charged = account.usage();
    let before = memory(&root).observation();
    let (root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_registry(entry, 64)
    });
    assert!(result.is_err());
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
    assert_eq!(memory(&root).observation(), before);
    assert_eq!(trace.borrow().calls, ["retain-root"]);
}

#[test]
fn native_fill_registry_primary_partial_errors_and_panics_keep_original_accounts() {
    for stage in [
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Complete,
    ] {
        for panics in [false, true] {
            let captured = native_fill_cohort_cases::payload();
            let (mut root, trace, account, pointer) =
                setup_registry(captured.exact_payload_bytes());
            let charged = account.usage();
            root.preparation.1.primary_inject_stage_v1(stage, panics);
            let (root, result) = run_work(root, |root, entry| {
                root.construct_native_fill_registry(entry, 4096)
            });
            assert!(result.is_err());
            root.preparation.1.primary_assert_failed_stage_v1(stage);
            assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
            assert_eq!(account.usage(), charged);
            assert!(!trace.borrow().calls.contains(&"allocate-ring"));
        }
    }
}

#[test]
fn native_fill_registry_primary_currentness_and_uncertain_create_keep_common_root() {
    for panics in [false, true] {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().fault = Some(("currentness", 3, panics));
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_registry(entry, 4096)
        });
        assert!(result.is_err());
        assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
    }
    for mode in 1..=5 {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
        let charged = account.usage();
        trace.borrow_mut().create = mode;
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_registry(entry, 4096)
        });
        assert!(result.is_err());
        assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
        assert!(trace.borrow().poison);
        assert_eq!(trace.borrow().cleanup, 0);
    }
}

#[test]
fn native_fill_registry_four_original_publications_recycle_independently_in_cpu_adapter() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
    let (mut root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_registry(entry, 4096)
    });
    assert!(result.is_ok());
    let charged = account.usage();
    let completed = root.completed.as_mut().unwrap();
    let queue = completed.engine.resources[0].key;
    let common = completed.dispatch.take().unwrap();
    let ((), common, poisoned, _) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, common, |session| {
            let registry = &mut root.preparation.2;
            let mut receipts = std::array::from_fn::<_, 4, _>(|index| {
                Some(
                    session
                        .submit_registry_binding_for_test(&mut registry.recipes[index], |_, _| {
                            Ok(index as u64)
                        })
                        .unwrap(),
                )
            });
            assert!(!registry.settled());
            for index in [3, 0, 2, 1] {
                let completed = session
                    .complete_registry_binding_for_test(
                        &mut registry.recipes[index],
                        receipts[index].take().unwrap(),
                    )
                    .unwrap();
                session
                    .recycle_registry_binding_for_test(&mut registry.recipes[index], completed)
                    .unwrap();
                assert!(
                    session
                        .submit_registry_binding_for_test(
                            &mut registry.recipes[index],
                            |_, _| panic!("accepted original cannot be submitted twice")
                        )
                        .is_err()
                );
            }
            assert!(registry.settled());
        });
    root.completed.as_mut().unwrap().dispatch = Some(common);
    assert!(!poisoned);
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
}

#[test]
fn native_fill_registry_retry_before_publication_preserves_every_original_for_exact_retry() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) = setup_registry(captured.exact_payload_bytes());
    let (mut root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_registry(entry, 4096)
    });
    assert!(result.is_ok());
    let charged = account.usage();
    let completed = root.completed.as_mut().unwrap();
    let queue = completed.engine.resources[0].key;
    let common = completed.dispatch.take().unwrap();
    let ((), common, poisoned, completion) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, common, |session| {
            let registry = &mut root.preparation.2;
            let result =
                session.submit_registry_binding_for_test(&mut registry.recipes[3], |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                        NativeAqlSubmissionErrorV1::Ring(fe2o3_aql::AqlRingReservationError::Full),
                    ))
                });
            assert!(matches!(
                result,
                Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_))
            ));
            assert!(registry.settled());
            let actual = session
                .submit_registry_binding_for_test(&mut registry.recipes[3], |_, _| Ok(7))
                .unwrap();
            assert!(!registry.settled());
            let completed = session
                .complete_registry_binding_for_test(&mut registry.recipes[3], actual)
                .unwrap();
            session
                .recycle_registry_binding_for_test(&mut registry.recipes[3], completed)
                .unwrap();
            assert!(registry.settled());
        });
    root.completed.as_mut().unwrap().dispatch = Some(common);
    assert!(!poisoned);
    assert_ne!(completion[0], completion[1]);
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
}
