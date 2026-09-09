use alloc::vec::Vec;

use super::*;

fn lane() -> R60LaneIdentityV1 {
    R60LaneIdentityV1 {
        device_id: 60,
        device_generation: 7,
        lane_id: 3,
        lane_generation: 11,
    }
}

fn recipe(seed: u8) -> R60RecipeStorageFingerprintV1 {
    R60RecipeStorageFingerprintV1 {
        kernel_id: u64::from(seed) + 1,
        module_sha256: [seed; 32],
        dispatch_shape_sha256: [seed.wrapping_add(1); 32],
        storage_sha256: [seed.wrapping_add(2); 32],
        bindings_sha256: [seed.wrapping_add(3); 32],
    }
}

fn pipeline_recipe() -> R60RecipeStorageFingerprintV1 {
    recipe(60)
}

fn model() -> R60OrdinaryFixedDispatchPipelineModelV1 {
    R60OrdinaryFixedDispatchPipelineModelV1::new_model_only(lane()).unwrap()
}

fn enqueue(
    model: &mut R60OrdinaryFixedDispatchPipelineModelV1,
    submission: u64,
    predecessor: Option<R60EpochIdentityV1>,
    dependencies: &[R60EpochIdentityV1],
) -> R60EpochIdentityV1 {
    enqueue_with_recipe(
        model,
        submission,
        predecessor,
        dependencies,
        pipeline_recipe(),
    )
}

fn enqueue_with_recipe(
    model: &mut R60OrdinaryFixedDispatchPipelineModelV1,
    submission: u64,
    predecessor: Option<R60EpochIdentityV1>,
    dependencies: &[R60EpochIdentityV1],
    recipe_storage: R60RecipeStorageFingerprintV1,
) -> R60EpochIdentityV1 {
    model
        .enqueue_model_only(
            submission,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            predecessor,
            dependencies,
            recipe_storage,
        )
        .unwrap()
}

fn prepare(model: &mut R60OrdinaryFixedDispatchPipelineModelV1, identity: R60EpochIdentityV1) {
    model
        .prepare_model_only(identity, pipeline_recipe())
        .unwrap();
}

fn publish(model: &mut R60OrdinaryFixedDispatchPipelineModelV1, identity: R60EpochIdentityV1) {
    let ordering_witness = match model.ordered_predecessor_model_only(identity).unwrap() {
        Some(predecessor) => R60PublicationOrderingWitnessV1::WaitForPrior(predecessor),
        None => R60PublicationOrderingWitnessV1::NoPredecessor,
    };
    assert_eq!(
        model
            .publish_model_only(
                identity,
                pipeline_recipe(),
                ordering_witness,
                R60PublicationScriptV1::Complete,
            )
            .unwrap(),
        R60PublicationOutcomeV1::Published
    );
}

fn complete(
    model: &mut R60OrdinaryFixedDispatchPipelineModelV1,
    identity: R60EpochIdentityV1,
    status: R60TerminalStatusV1,
) {
    assert_eq!(
        model
            .observe_completion_model_only(
                identity,
                identity,
                pipeline_recipe(),
                R60CompletionObservationV1::Exact(status),
            )
            .unwrap(),
        R60CompletionOutcomeV1::Completed
    );
}

fn retire(model: &mut R60OrdinaryFixedDispatchPipelineModelV1, identity: R60EpochIdentityV1) {
    assert_eq!(
        model
            .retire_physical_model_only(
                identity,
                identity,
                pipeline_recipe(),
                R60RetirementObservationV1::Exact,
            )
            .unwrap(),
        R60RetirementOutcomeV1::PhysicallyRetired
    );
}

#[test]
fn n1_and_n3_are_rejected_before_epoch_or_custody_mutation() {
    for class in [
        R60ExecutionClassV1::PersistentN1,
        R60ExecutionClassV1::ThreeBindingPersistentN3,
    ] {
        let mut model = model();
        let before = model.clone();
        assert_eq!(
            model.enqueue_model_only(1, class, None, &[], recipe(1)),
            Err(R60PipelineModelErrorV1::UnsupportedExecutionClass)
        );
        assert_eq!(model, before);
    }
}

#[test]
fn exact_recipe_and_storage_are_immutable_across_every_success_phase() {
    let mut model = model();
    let identity = enqueue(&mut model, 1, None, &[]);
    let exact = pipeline_recipe();
    assert_eq!(model.recipe_storage_model_only(identity), Some(exact));
    prepare(&mut model, identity);
    publish(&mut model, identity);
    complete(&mut model, identity, R60TerminalStatusV1::Succeeded);
    retire(&mut model, identity);
    assert_eq!(model.commit_contiguous_model_only().unwrap(), [identity]);
    assert_eq!(
        model.phase_model_only(identity),
        Some(R60PipelinePhaseV1::HostCommitted)
    );
    assert_eq!(model.recipe_storage_model_only(identity), Some(exact));
    model.validate_global_invariants().unwrap();
}

#[test]
fn recipe_or_storage_substitution_is_exactly_no_effect() {
    let mut model = model();
    let identity = enqueue(&mut model, 1, None, &[]);
    let before = model.clone();
    assert_eq!(
        model.prepare_model_only(identity, recipe(61)),
        Err(R60PipelineModelErrorV1::RecipeOrStorageMismatch)
    );
    assert_eq!(model, before);
    prepare(&mut model, identity);
    let before = model.clone();
    assert_eq!(
        model.publish_model_only(
            identity,
            recipe(61),
            R60PublicationOrderingWitnessV1::NoPredecessor,
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::RecipeOrStorageMismatch)
    );
    assert_eq!(model, before);
}

#[test]
fn ordered_failure_is_completion_only_but_explicit_failure_blocks_successor() {
    let mut ordered = model();
    let predecessor = enqueue(&mut ordered, 1, None, &[]);
    let successor = enqueue(&mut ordered, 2, Some(predecessor), &[]);
    prepare(&mut ordered, predecessor);
    publish(&mut ordered, predecessor);
    complete(
        &mut ordered,
        predecessor,
        R60TerminalStatusV1::Failed { code: -7 },
    );
    prepare(&mut ordered, successor);
    publish(&mut ordered, successor);

    let mut explicit = R60OrdinaryFixedDispatchPipelineModelV1::new_model_only(lane()).unwrap();
    let predecessor = enqueue(&mut explicit, 1, None, &[]);
    let successor = enqueue(&mut explicit, 2, Some(predecessor), &[predecessor]);
    prepare(&mut explicit, predecessor);
    publish(&mut explicit, predecessor);
    complete(
        &mut explicit,
        predecessor,
        R60TerminalStatusV1::Failed { code: -7 },
    );
    prepare(&mut explicit, successor);
    let before = explicit.clone();
    assert_eq!(
        explicit.publish_model_only(
            successor,
            pipeline_recipe(),
            R60PublicationOrderingWitnessV1::WaitForPrior(predecessor),
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::ExplicitDependencyNotSucceeded)
    );
    assert_eq!(explicit, before);
}

#[test]
fn three_epochs_are_published_before_any_predecessor_completes() {
    let mut model = model();
    let first = enqueue(&mut model, 1, None, &[]);
    let second = enqueue(&mut model, 2, Some(first), &[]);
    let third = enqueue(&mut model, 3, Some(second), &[]);
    for identity in [first, second, third] {
        prepare(&mut model, identity);
        publish(&mut model, identity);
    }
    for (identity, witness) in [
        (first, R60PublicationOrderingWitnessV1::NoPredecessor),
        (second, R60PublicationOrderingWitnessV1::WaitForPrior(first)),
        (third, R60PublicationOrderingWitnessV1::WaitForPrior(second)),
    ] {
        assert_eq!(
            model.phase_model_only(identity),
            Some(R60PipelinePhaseV1::Published)
        );
        assert_eq!(
            model.publication_ordering_witness_model_only(identity),
            Some(Some(witness))
        );
    }
    assert_eq!(model.snapshot_model_only().native_publications, 3);
    model.validate_global_invariants().unwrap();
}

#[test]
fn wait_for_prior_accepts_every_live_chain_phase() {
    for predecessor_phase in [
        R60PipelinePhaseV1::Published,
        R60PipelinePhaseV1::Completed,
        R60PipelinePhaseV1::PhysicallyRetired,
    ] {
        let mut model = model();
        let predecessor = enqueue(&mut model, 1, None, &[]);
        prepare(&mut model, predecessor);
        publish(&mut model, predecessor);
        if predecessor_phase != R60PipelinePhaseV1::Published {
            complete(
                &mut model,
                predecessor,
                R60TerminalStatusV1::Failed { code: -7 },
            );
        }
        if predecessor_phase == R60PipelinePhaseV1::PhysicallyRetired {
            retire(&mut model, predecessor);
        }
        let successor = enqueue(&mut model, 2, Some(predecessor), &[]);
        prepare(&mut model, successor);
        publish(&mut model, successor);
        assert_eq!(model.predecessor_retain_count_model_only(predecessor), 1);
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn early_chain_requires_exact_wait_binding_and_recipe_storage() {
    let mut model = model();
    let predecessor = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, predecessor);
    publish(&mut model, predecessor);
    let successor = enqueue_with_recipe(&mut model, 2, Some(predecessor), &[], recipe(61));
    model.prepare_model_only(successor, recipe(61)).unwrap();

    let before = model.clone();
    assert_eq!(
        model.publish_model_only(
            successor,
            recipe(61),
            R60PublicationOrderingWitnessV1::NoPredecessor,
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::OrderedWaitMismatch)
    );
    assert_eq!(model, before);
    assert_eq!(
        model.publish_model_only(
            successor,
            recipe(61),
            R60PublicationOrderingWitnessV1::WaitForPrior(predecessor),
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::OrderedPredecessorNotPublicationCapable)
    );
    assert_eq!(model, before);
    complete(&mut model, predecessor, R60TerminalStatusV1::Succeeded);
    let before = model.clone();
    assert_eq!(
        model.publish_model_only(
            successor,
            recipe(61),
            R60PublicationOrderingWitnessV1::CompletionObserved(predecessor),
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::OrderedPredecessorNotPublicationCapable)
    );
    assert_eq!(model, before);
}

#[test]
fn committed_predecessor_is_completion_authority_but_not_a_chain_anchor() {
    let mut model = model();
    let predecessor = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, predecessor);
    publish(&mut model, predecessor);
    complete(
        &mut model,
        predecessor,
        R60TerminalStatusV1::Failed { code: -7 },
    );
    retire(&mut model, predecessor);
    assert_eq!(model.commit_contiguous_model_only().unwrap(), [predecessor]);

    let successor = enqueue(&mut model, 2, Some(predecessor), &[]);
    prepare(&mut model, successor);
    let before = model.clone();
    assert_eq!(
        model.publish_model_only(
            successor,
            pipeline_recipe(),
            R60PublicationOrderingWitnessV1::WaitForPrior(predecessor),
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::OrderedPredecessorNotPublicationCapable)
    );
    assert_eq!(model, before);
    assert_eq!(model.predecessor_retain_count_model_only(predecessor), 1);
    assert_eq!(
        model
            .publish_model_only(
                successor,
                pipeline_recipe(),
                R60PublicationOrderingWitnessV1::CompletionObserved(predecessor),
                R60PublicationScriptV1::Complete,
            )
            .unwrap(),
        R60PublicationOutcomeV1::Published
    );
    assert_eq!(model.predecessor_retain_count_model_only(predecessor), 0);
    model.validate_global_invariants().unwrap();

    let mut explicit = R60OrdinaryFixedDispatchPipelineModelV1::new_model_only(lane()).unwrap();
    let predecessor = enqueue(&mut explicit, 1, None, &[]);
    prepare(&mut explicit, predecessor);
    publish(&mut explicit, predecessor);
    complete(
        &mut explicit,
        predecessor,
        R60TerminalStatusV1::Failed { code: -7 },
    );
    retire(&mut explicit, predecessor);
    explicit.commit_contiguous_model_only().unwrap();
    let successor = enqueue(&mut explicit, 2, Some(predecessor), &[predecessor]);
    prepare(&mut explicit, successor);
    let before = explicit.clone();
    assert_eq!(
        explicit.publish_model_only(
            successor,
            pipeline_recipe(),
            R60PublicationOrderingWitnessV1::CompletionObserved(predecessor),
            R60PublicationScriptV1::Complete,
        ),
        Err(R60PipelineModelErrorV1::ExplicitDependencyNotSucceeded)
    );
    assert_eq!(explicit, before);
}

#[test]
fn retryable_publication_retains_prepared_recipe_and_is_native_effect_free() {
    let mut model = model();
    let identity = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, identity);
    let before = model.clone();
    assert_eq!(
        model
            .publish_model_only(
                identity,
                pipeline_recipe(),
                R60PublicationOrderingWitnessV1::NoPredecessor,
                R60PublicationScriptV1::RetryableNoEffect,
            )
            .unwrap(),
        R60PublicationOutcomeV1::Retryable
    );
    assert_eq!(model, before);
    assert_eq!(
        model.phase_model_only(identity),
        Some(R60PipelinePhaseV1::Prepared)
    );
    assert_eq!(model.owns_custody_model_only(identity), Some(true));
    publish(&mut model, identity);
}

#[test]
fn completion_and_retirement_retry_do_not_expose_host_state() {
    let mut model = model();
    let identity = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, identity);
    publish(&mut model, identity);
    assert_eq!(
        model
            .observe_completion_model_only(
                identity,
                identity,
                pipeline_recipe(),
                R60CompletionObservationV1::Pending,
            )
            .unwrap(),
        R60CompletionOutcomeV1::Pending
    );
    assert_eq!(model.host_observation_model_only(identity), None);
    complete(&mut model, identity, R60TerminalStatusV1::Succeeded);
    let before = model.clone();
    assert_eq!(
        model
            .retire_physical_model_only(
                identity,
                identity,
                pipeline_recipe(),
                R60RetirementObservationV1::RetryableRestoredExact,
            )
            .unwrap(),
        R60RetirementOutcomeV1::Retryable
    );
    assert_eq!(model, before);
    assert_eq!(model.host_observation_model_only(identity), None);
}

#[test]
fn out_of_order_physical_retirement_is_hidden_until_contiguous_commit() {
    let mut model = model();
    let first = enqueue(&mut model, 1, None, &[]);
    let second = enqueue(&mut model, 2, Some(first), &[]);
    let third = enqueue(&mut model, 3, Some(second), &[]);
    for identity in [first, second, third] {
        prepare(&mut model, identity);
        publish(&mut model, identity);
        complete(&mut model, identity, R60TerminalStatusV1::Succeeded);
    }
    retire(&mut model, third);
    retire(&mut model, second);
    assert!(model.commit_contiguous_model_only().unwrap().is_empty());
    for identity in [first, second, third] {
        assert_eq!(model.host_observation_model_only(identity), None);
        assert_eq!(model.owns_custody_model_only(identity), Some(true));
    }
    retire(&mut model, first);
    assert_eq!(
        model.commit_contiguous_model_only().unwrap(),
        [first, second, third]
    );
    for identity in [first, second, third] {
        assert_eq!(
            model.host_observation_model_only(identity),
            Some(R60HostObservationV1 {
                status: R60TerminalStatusV1::Succeeded,
                committed_effects: 1,
                profile_visible: true,
                custody_released: true,
            })
        );
    }
    assert!(model.commit_contiguous_model_only().unwrap().is_empty());
    model.validate_global_invariants().unwrap();
}

#[test]
fn deferred_ordered_predecessor_retain_releases_only_at_successor_commit() {
    let mut model = model();
    let predecessor = enqueue(&mut model, 1, None, &[]);
    let successor = enqueue(&mut model, 2, Some(predecessor), &[]);
    assert_eq!(model.predecessor_retain_count_model_only(predecessor), 1);
    for identity in [predecessor, successor] {
        prepare(&mut model, identity);
        publish(&mut model, identity);
        complete(&mut model, identity, R60TerminalStatusV1::Succeeded);
    }
    retire(&mut model, predecessor);
    assert_eq!(model.commit_contiguous_model_only().unwrap(), [predecessor]);
    assert_eq!(model.predecessor_retain_count_model_only(predecessor), 1);
    retire(&mut model, successor);
    assert_eq!(model.commit_contiguous_model_only().unwrap(), [successor]);
    assert_eq!(model.predecessor_retain_count_model_only(predecessor), 0);
}

#[test]
fn cancellation_is_tail_only_and_publication_is_too_late() {
    let mut model = model();
    let first = enqueue(&mut model, 1, None, &[]);
    let second = enqueue(&mut model, 2, Some(first), &[]);
    assert_eq!(
        model.cancel_tail_model_only(first),
        Err(R60PipelineModelErrorV1::NotLaneTail)
    );
    assert_eq!(
        model.cancel_tail_model_only(second).unwrap(),
        R60CancellationV1::Cancelled
    );
    assert_eq!(model.predecessor_retain_count_model_only(first), 0);
    let prepared_tail = enqueue(&mut model, 3, Some(first), &[]);
    prepare(&mut model, prepared_tail);
    assert_eq!(
        model.cancel_tail_model_only(prepared_tail).unwrap(),
        R60CancellationV1::Cancelled
    );
    assert_eq!(model.predecessor_retain_count_model_only(first), 0);
    prepare(&mut model, first);
    publish(&mut model, first);
    assert_eq!(
        model.cancel_tail_model_only(first).unwrap(),
        R60CancellationV1::TooLate
    );
    assert_eq!(
        model.phase_model_only(first),
        Some(R60PipelinePhaseV1::Published)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn substituted_completion_or_failed_restore_quarantines_the_whole_lane() {
    let mut completion = model();
    let first = enqueue(&mut completion, 1, None, &[]);
    let second = enqueue(&mut completion, 2, Some(first), &[]);
    for identity in [first, second] {
        prepare(&mut completion, identity);
    }
    publish(&mut completion, first);
    assert_eq!(
        completion
            .observe_completion_model_only(
                first,
                second,
                pipeline_recipe(),
                R60CompletionObservationV1::Exact(R60TerminalStatusV1::Succeeded),
            )
            .unwrap(),
        R60CompletionOutcomeV1::Quarantined
    );
    for identity in [first, second] {
        assert_eq!(
            completion.phase_model_only(identity),
            Some(R60PipelinePhaseV1::Quarantined)
        );
        assert_eq!(completion.owns_custody_model_only(identity), Some(true));
        assert_eq!(completion.host_observation_model_only(identity), None);
    }
    assert!(!completion.current_model_only());
    completion.validate_global_invariants().unwrap();

    let mut restore = model();
    let identity = enqueue(&mut restore, 1, None, &[]);
    prepare(&mut restore, identity);
    publish(&mut restore, identity);
    complete(&mut restore, identity, R60TerminalStatusV1::Succeeded);
    assert_eq!(
        restore
            .retire_physical_model_only(
                identity,
                identity,
                pipeline_recipe(),
                R60RetirementObservationV1::RestoreFailed,
            )
            .unwrap(),
        R60RetirementOutcomeV1::Quarantined
    );
    assert_eq!(
        restore.phase_model_only(identity),
        Some(R60PipelinePhaseV1::Quarantined)
    );
    assert_eq!(restore.owns_custody_model_only(identity), Some(true));
    assert!(!restore.current_model_only());
}

#[test]
fn fixed_roster_admits_exactly_sixty_four_epochs() {
    let mut model = model();
    let mut predecessor = None;
    let mut identities = Vec::new();
    for submission in 1..=R60_MAX_EPOCHS_PER_LANE_V1 as u64 {
        let identity = enqueue(&mut model, submission, predecessor, &[]);
        prepare(&mut model, identity);
        publish(&mut model, identity);
        predecessor = Some(identity);
        identities.push(identity);
    }
    assert_eq!(
        model.live_epoch_count_model_only(),
        R60_MAX_EPOCHS_PER_LANE_V1
    );
    let before = model.clone();
    assert_eq!(
        model.enqueue_model_only(
            65,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            predecessor,
            &[],
            pipeline_recipe(),
        ),
        Err(R60PipelineModelErrorV1::CapacityExceeded)
    );
    assert_eq!(model, before);
    assert_eq!(identities[0].slot(), 0);
    assert_eq!(identities[63].slot(), 63);
    assert_eq!(model.snapshot_model_only().native_publications, 64);
    assert!(identities.iter().all(|identity| {
        model.phase_model_only(*identity) == Some(R60PipelinePhaseV1::Published)
    }));
    model.validate_global_invariants().unwrap();
}

#[test]
fn explicit_dependency_admission_is_bounded_exact_and_failure_atomic() {
    let mut exact = model();
    let mut predecessor = None;
    let mut dependencies = Vec::new();
    for submission in 1..=R60_MAX_EXPLICIT_SUCCESS_DEPENDENCIES_V1 as u64 {
        let identity = enqueue(&mut exact, submission, predecessor, &[]);
        predecessor = Some(identity);
        dependencies.push(identity);
    }
    let admitted = enqueue(&mut exact, 33, predecessor, &dependencies);
    assert_eq!(
        exact.explicit_success_dependencies_model_only(admitted),
        Some(dependencies.as_slice())
    );

    let mut overflow = model();
    let mut predecessor = None;
    let mut dependencies = Vec::new();
    for submission in 1..=33 {
        let identity = enqueue(&mut overflow, submission, predecessor, &[]);
        predecessor = Some(identity);
        dependencies.push(identity);
    }
    let before = overflow.clone();
    assert_eq!(
        overflow.enqueue_model_only(
            34,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            predecessor,
            &dependencies,
            pipeline_recipe(),
        ),
        Err(R60PipelineModelErrorV1::CapacityExceeded)
    );
    assert_eq!(overflow, before);

    let mut hostile = model();
    let local = enqueue(&mut hostile, 1, None, &[]);
    let before = hostile.clone();
    assert_eq!(
        hostile.enqueue_model_only(
            2,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            Some(local),
            &[local, local],
            pipeline_recipe(),
        ),
        Err(R60PipelineModelErrorV1::InvalidIdentity)
    );
    assert_eq!(hostile, before);

    let mut foreign_lane = lane();
    foreign_lane.lane_generation += 1;
    let mut foreign =
        R60OrdinaryFixedDispatchPipelineModelV1::new_model_only(foreign_lane).unwrap();
    let foreign_identity = enqueue(&mut foreign, 9, None, &[]);
    assert_eq!(
        hostile.enqueue_model_only(
            2,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            Some(local),
            &[foreign_identity],
            pipeline_recipe(),
        ),
        Err(R60PipelineModelErrorV1::InvalidIdentity)
    );
    assert_eq!(hostile, before);
}

#[test]
fn slot_generation_reuse_rejects_aba_identity() {
    let mut model = model();
    let old = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, old);
    publish(&mut model, old);
    complete(&mut model, old, R60TerminalStatusV1::Succeeded);
    retire(&mut model, old);
    model.commit_contiguous_model_only().unwrap();
    let fresh = enqueue(&mut model, 2, Some(old), &[]);
    assert_eq!(fresh.slot(), old.slot());
    assert!(fresh.slot_generation() > old.slot_generation());
    let before = model.clone();
    assert_eq!(
        model.prepare_model_only(old, pipeline_recipe()),
        Err(R60PipelineModelErrorV1::StaleEpoch)
    );
    assert_eq!(model, before);
}

#[test]
fn slot_and_logical_epoch_exhaustion_never_wrap() {
    let mut slots = model();
    slots.exhaust_vacant_slot_generations_for_test_v1();
    let before = slots.clone();
    assert_eq!(
        slots.enqueue_model_only(
            1,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            None,
            &[],
            recipe(1),
        ),
        Err(R60PipelineModelErrorV1::CapacityExceeded)
    );
    assert_eq!(slots, before);

    let mut epochs = model();
    epochs.exhaust_logical_epochs_for_test_v1();
    let before = epochs.clone();
    assert_eq!(
        epochs.enqueue_model_only(
            1,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            None,
            &[],
            recipe(1),
        ),
        Err(R60PipelineModelErrorV1::CapacityExceeded)
    );
    assert_eq!(epochs, before);
}

#[test]
fn host_commit_counter_overflow_is_failure_atomic() {
    let mut model = model();
    let identity = enqueue(&mut model, 1, None, &[]);
    prepare(&mut model, identity);
    publish(&mut model, identity);
    complete(&mut model, identity, R60TerminalStatusV1::Succeeded);
    retire(&mut model, identity);
    model.exhaust_host_commits_for_test_v1();
    let before = model.clone();
    assert_eq!(
        model.commit_contiguous_model_only(),
        Err(R60PipelineModelErrorV1::CapacityExceeded)
    );
    assert_eq!(model, before);
    assert_eq!(
        model.phase_model_only(identity),
        Some(R60PipelinePhaseV1::PhysicallyRetired)
    );
    assert_eq!(model.host_observation_model_only(identity), None);
    assert_eq!(model.owns_custody_model_only(identity), Some(true));
}

#[test]
fn currentness_loss_is_lane_wide_absorbing_and_custody_preserving() {
    let mut model = model();
    let first = enqueue(&mut model, 1, None, &[]);
    let second = enqueue(&mut model, 2, Some(first), &[]);
    prepare(&mut model, first);
    publish(&mut model, first);
    model.lose_currentness_model_only();
    assert!(!model.current_model_only());
    assert!(model.quarantined_model_only());
    for identity in [first, second] {
        assert_eq!(
            model.phase_model_only(identity),
            Some(R60PipelinePhaseV1::Quarantined)
        );
        assert_eq!(model.owns_custody_model_only(identity), Some(true));
    }
    assert_eq!(
        model.enqueue_model_only(
            3,
            R60ExecutionClassV1::OrdinaryFixedDispatch,
            Some(second),
            &[],
            recipe(3),
        ),
        Err(R60PipelineModelErrorV1::NotCurrent)
    );
    assert_eq!(
        model.commit_contiguous_model_only(),
        Err(R60PipelineModelErrorV1::NotCurrent)
    );
    model.validate_global_invariants().unwrap();
}
