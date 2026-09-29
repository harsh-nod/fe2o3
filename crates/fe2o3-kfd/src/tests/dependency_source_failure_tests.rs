//! Actual failure join and CPU-prepared native resources, not GPU execution.
use super::super::fixed_dispatch::{
    DependencySourceRecipeV1, NativeDependencySourceRecipeV1, dependency_source_failure_body,
    dependency_source_recipe_cancel_call,
};
use super::*;
use crate::queue::dispatch_binding::{
    TestOnlyMultiInflightDispatchOwnerV1, actual_persistent_control_test_program,
    control_release::{ReturningControlCleanupCustodyV1, ReturningControlModeV1},
    prepare_public_fixed_dispatch_resources_in_place,
};
use crate::shared_memory::PreparationMemoryFixtureV1;

fn settle<const N: usize>(
    session: &mut ComputeAqlQueueSessionV1,
    recipe: &mut impl DependencySourceRecipeV1<N>,
    identity: DispatchEpochIdentityV1,
    failure: FixedDispatchSubmissionFailureV1,
) -> FixedDispatchSubmissionFailureV1 {
    macro_rules! rust_expr {
        ($body:expr) => {
            $body
        };
    }
    dependency_source_failure_body!(rust_expr, session, recipe, identity, failure)
}

fn marker(kind: u8) -> (FixedDispatchSubmissionFailureV1, (*const u8, usize, usize)) {
    let mut text = String::with_capacity(137);
    text.push_str("exact retained source failure payload");
    let allocation = (text.as_ptr(), text.len(), text.capacity());
    let error = ComputeAqlQueueSessionErrorV1::Doorbell(text);
    let failure = match kind {
        0 => FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error),
        1 => FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error),
        2 => FixedDispatchSubmissionFailureV1::Terminal(error),
        _ => unreachable!(),
    };
    (failure, allocation)
}

fn assert_marker(
    out: FixedDispatchSubmissionFailureV1,
    retry: bool,
    allocation: (*const u8, usize, usize),
) {
    let error = match out {
        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error) if retry => error,
        FixedDispatchSubmissionFailureV1::Terminal(error) if !retry => error,
        other => panic!("wrong source failure class: {other:?}"),
    };
    let ComputeAqlQueueSessionErrorV1::Doorbell(text) = error else {
        panic!("substituted error payload")
    };
    assert_eq!((text.as_ptr(), text.len(), text.capacity()), allocation);
    assert_eq!(text, "exact retained source failure payload");
}

#[test]
fn native_source_failure_join_preserves_neighbors_resources_and_move_only_errors() {
    for auxiliary in [false, true] {
        for kind in 0..3 {
            let mut memory = PreparationMemoryFixtureV1::with_host_budget(true, 1 << 30, 1 << 20);
            const IMAGE: &[u8] = include_bytes!(
                "../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
            );
            let programs: Vec<_> = (1..=3)
                .map(|i| actual_persistent_control_test_program(IMAGE, [i; 32]))
                .collect();
            let packets = std::array::from_fn::<_, 3, _>(|i| {
                let mut bytes = [0; 16];
                bytes[8..].copy_from_slice(&1024_u64.to_le_bytes());
                Gfx942FixedDispatchPacketV1::new(
                    i,
                    fe2o3_aql::AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
                    0,
                    bytes.into(),
                    vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
                )
            });
            let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, memory.roster());
            prepare_public_fixed_dispatch_resources_in_place(
                &mut memory,
                &programs,
                &mut preparation,
            )
            .unwrap();
            let dispatch = preparation.take_completed().unwrap();
            let mut primary = test_queue_key(186, 1);
            primary.vm = memory.primary_vm();
            let mut other = test_queue_key(187, 1);
            other.vm = primary.vm;
            let mut session = persistent_compute_cancellation_test_session(primary, None, None);
            session
                .auxiliary_compute_lanes
                .push(AuxiliaryComputeLaneSlotV1 {
                    generation: 7,
                    state: Some(compute_lane_state_for_multi_inflight_test(other)),
                });
            let lane = if auxiliary {
                ComputeAqlQueueLaneV1 {
                    session: primary,
                    ordinal: 1,
                    generation: 7,
                }
            } else {
                session.primary_compute_lane_v1()
            };
            let before_primary = session.completion_owner.source_rollback_snapshot_for_test();
            let before_aux = session.auxiliary_compute_lanes[0]
                .state
                .as_ref()
                .unwrap()
                .completion_owner
                .source_rollback_snapshot_for_test();
            session
                .with_compute_lane_v1(lane, |selected| {
                    let session = &mut *selected.session;
                    assert!(session.dispatch.replace(dispatch).is_none());
                    let mut recipe = NativeDependencySourceRecipeV1;
                    let (_, left) = <NativeDependencySourceRecipeV1 as DependencySourceRecipeV1<
                        3,
                    >>::bind(&mut recipe, session)
                    .unwrap();
                    let (_, target) =
                        <NativeDependencySourceRecipeV1 as DependencySourceRecipeV1<3>>::bind(
                            &mut recipe,
                            session,
                        )
                        .unwrap();
                    let (_, right) = <NativeDependencySourceRecipeV1 as DependencySourceRecipeV1<
                        3,
                    >>::bind(&mut recipe, session)
                    .unwrap();
                    let owner = session.dispatch.as_ref().unwrap();
                    let mut expected = owner.source_failure_snapshot_v1();
                    let resources = owner.primary_fixture_identities_v1();
                    let memory_before = memory.observation();
                    session.dependency_owner.reserve_acceptance_epoch().unwrap();
                    let dependency_before = session.dependency_owner.custody_snapshot_for_test();
                    let (failure, allocation) = marker(kind);
                    let out = settle::<3>(session, &mut recipe, target, failure);
                    assert_marker(out, kind == 0, allocation);
                    if kind == 0 {
                        expected.expect_cancel_for_test(target);
                    }
                    let owner = session.dispatch.as_ref().unwrap();
                    assert_eq!(owner.source_failure_snapshot_v1(), expected);
                    assert_eq!(owner.primary_fixture_identities_v1(), resources);
                    assert_eq!(memory.observation(), memory_before);
                    assert_eq!(
                        session.dependency_owner.custody_snapshot_for_test(),
                        dependency_before
                    );
                    assert!(!session.terminal_poisoned);
                    // The raw failure join does not terminalize; dispose every real fixture authority.
                    for identity in [Some(left), (kind != 0).then_some(target), Some(right)]
                        .into_iter()
                        .flatten()
                    {
                        <NativeDependencySourceRecipeV1 as DependencySourceRecipeV1<3>>::cancel(
                            &mut recipe,
                            session,
                            identity,
                        )
                        .unwrap();
                    }
                    let mut cleanup = ReturningControlCleanupCustodyV1::new(
                        session.dispatch.take().unwrap(),
                        ReturningControlModeV1::Ordinary,
                    );
                    cleanup.release_ordinary_in_place(&mut memory).unwrap();
                    assert!(cleanup.is_complete());
                })
                .unwrap();
            assert_eq!(
                session.completion_owner.source_rollback_snapshot_for_test(),
                before_primary
            );
            assert_eq!(
                session.auxiliary_compute_lanes[0]
                    .state
                    .as_ref()
                    .unwrap()
                    .completion_owner
                    .source_rollback_snapshot_for_test(),
                before_aux
            );
            memory.primary_assert_all_released_v1();
        }
    }
}

#[test]
fn source_failure_without_native_dispatch_skips_nonretry_and_panics_on_retry() {
    for kind in 0..3 {
        let (mut session, _) = super::runtime_publication_tests::fixture(false);
        let mut logical = TestOnlyMultiInflightDispatchOwnerV1::new();
        let identity = logical
            .reserve_one(session.key, test_completion_template(session.key, 1))
            .unwrap();
        let completion = session.completion_owner.source_rollback_snapshot_for_test();
        let dependency = session.dependency_owner.custody_snapshot_for_test();
        let (failure, allocation) = marker(kind);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            settle::<3>(
                &mut session,
                &mut NativeDependencySourceRecipeV1,
                identity,
                failure,
            )
        }));
        if kind == 0 {
            let panic = result.unwrap_err();
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(message, "dependency source dispatch owner remains retained");
        } else {
            assert_marker(result.unwrap(), false, allocation);
        }
        assert!(session.dispatch.is_none());
        assert!(!session.terminal_poisoned);
        assert_eq!(
            session.completion_owner.source_rollback_snapshot_for_test(),
            completion
        );
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        logical.cancel(identity).unwrap();
    }
}

struct RefusingRecipe {
    calls: Vec<DispatchEpochIdentityV1>,
    poison: bool,
}
impl DependencySourceRecipeV1<3> for RefusingRecipe {
    fn bind(
        &mut self,
        _: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        ([CompletionPacketTemplateV1; 3], DispatchEpochIdentityV1),
        Gfx942DispatchBindingErrorV1,
    > {
        panic!("no bind at failure join")
    }
    fn mark_published(
        &mut self,
        _: &mut ComputeAqlQueueSessionV1,
        _: DispatchEpochIdentityV1,
        _: &Gfx942CompletionBatchV1<3>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        panic!("no publication at failure join")
    }
    fn cancel(
        &mut self,
        _: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.calls.push(identity);
        Err(if self.poison {
            Gfx942DispatchBindingErrorV1::Poisoned
        } else {
            Gfx942DispatchBindingErrorV1::ResourcePhase
        })
    }
}

#[test]
fn source_failure_normalizes_distinct_cancel_refusals_and_calls_exactly_once() {
    for poison in [false, true] {
        let (mut session, _) = super::runtime_publication_tests::fixture(false);
        let mut logical = TestOnlyMultiInflightDispatchOwnerV1::new();
        let identity = logical
            .reserve_one(session.key, test_completion_template(session.key, 1))
            .unwrap();
        let completion = session.completion_owner.source_rollback_snapshot_for_test();
        let dependency = session.dependency_owner.custody_snapshot_for_test();
        let mut recipe = RefusingRecipe {
            calls: Vec::new(),
            poison,
        };
        let out = settle::<3>(&mut session, &mut recipe, identity, marker(0).0);
        assert!(matches!(
            out,
            FixedDispatchSubmissionFailureV1::Terminal(
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
                )
            )
        ));
        assert_eq!(recipe.calls, [identity]);
        assert_eq!(
            session.completion_owner.source_rollback_snapshot_for_test(),
            completion
        );
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        assert!(!session.terminal_poisoned);
        logical.cancel(identity).unwrap();
    }
}
