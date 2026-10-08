//! Original rooted orchestration with synthetic completion, not GPU overlap.
use super::*;

fn counts<const N: usize>() -> [usize; N] {
    std::array::from_fn(|index| 1 + 64 * index)
}
fn grids<const N: usize>() -> [u32; N] {
    std::array::from_fn(|index| 64 * (index as u32 + 1))
}

fn exercise<const N: usize>() {
    let captured = native_fill_cohort_cases::payload();
    let (root, trace, account, pointer) =
        setup_resident_registry(captured.exact_payload_bytes(), counts::<N>(), grids::<N>());
    let charged = account.usage();
    assert_eq!(charged.retained_records, N + 2);
    let (mut root, result) = run_work(root, |root, entry| {
        root.construct_native_fill_registry(entry, 4096)
    });
    assert!(result.is_ok());
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    let completed = root.completed.as_mut().unwrap();
    let queue = completed.engine.resources[0].key;
    let common = completed.dispatch.take().unwrap();
    let ((), common, poisoned, _) =
        ComputeAqlQueueSessionV1::with_ordinary_binding_session_v1(queue, common, |session| {
            let registry = &mut root.preparation.2;
            let mut receipts = std::array::from_fn::<_, N, _>(|index| {
                Some(
                    session
                        .submit_registry_binding_for_test(
                            &mut registry.recipes[index],
                            |_, packets| {
                                assert_eq!(packets.packet_count(), 1);
                                Ok(index as u64)
                            },
                        )
                        .unwrap(),
                )
            });
            assert!(receipts.iter().all(Option::is_some));
            assert!(!registry.settled());
            for index in (0..N).rev() {
                let complete = session
                    .complete_registry_binding_for_test(
                        &mut registry.recipes[index],
                        receipts[index].take().unwrap(),
                    )
                    .unwrap();
                session
                    .recycle_registry_binding_for_test(&mut registry.recipes[index], complete)
                    .unwrap();
                assert_eq!(registry.settled(), index == 0);
                assert!(
                    session
                        .submit_registry_binding_for_test(
                            &mut registry.recipes[index],
                            |_, _| panic!("single-use original replay"),
                        )
                        .is_err()
                );
                assert_eq!(account.usage(), charged);
            }
        });
    root.completed.as_mut().unwrap().dispatch = Some(common);
    assert!(!poisoned);
    assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
    assert_eq!(account.usage(), charged);
}

#[test]
fn resident_registry_two_and_sixteen_originals_share_the_actual_submission_core() {
    exercise::<2>();
    exercise::<16>();
}

#[test]
fn resident_registry_sixteen_preparation_errors_and_panics_keep_all_originals() {
    for stage in [
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Complete,
    ] {
        for panics in [false, true] {
            let captured = native_fill_cohort_cases::payload();
            let (mut root, trace, account, pointer) = setup_resident_registry(
                captured.exact_payload_bytes(),
                counts::<16>(),
                grids::<16>(),
            );
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
fn resident_registry_sixteen_currentness_create_refusal_and_unwind_keep_common_root() {
    for mode in 0..7 {
        let captured = native_fill_cohort_cases::payload();
        let (root, trace, account, pointer) = setup_resident_registry(
            captured.exact_payload_bytes(),
            counts::<16>(),
            grids::<16>(),
        );
        let charged = account.usage();
        if mode < 2 {
            trace.borrow_mut().fault = Some(("currentness", 3, mode == 1));
        } else {
            trace.borrow_mut().create = mode - 1;
        }
        let (root, result) = run_work(root, |root, entry| {
            root.construct_native_fill_registry(entry, 4096)
        });
        assert!(result.is_err());
        assert_originals(&root, &trace, pointer, captured.exact_payload_bytes());
        assert_eq!(account.usage(), charged);
        if mode >= 2 {
            assert!(trace.borrow().poison);
            assert_eq!(trace.borrow().cleanup, 0);
        }
    }
}
