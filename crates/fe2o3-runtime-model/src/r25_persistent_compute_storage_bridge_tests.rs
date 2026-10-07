use super::*;

const STORAGE_BYTES: u64 = 4096;

fn identity(generation: u64) -> R25PersistentStorageIdentityV1 {
    R25PersistentStorageIdentityV1 {
        device_id: 25,
        vm_id: 31,
        allocation_id: 41,
        storage_generation: generation,
    }
}

fn full_range() -> R25FullStorageRangeV1 {
    R25FullStorageRangeV1 {
        logical_offset: 0,
        logical_bytes: STORAGE_BYTES,
        physical_offset: 0,
        physical_bytes: STORAGE_BYTES,
    }
}

fn effects(reads_storage: bool, writes_storage: bool) -> R25ComputeEffectsV1 {
    R25ComputeEffectsV1 {
        reads_storage,
        writes_storage,
    }
}

fn request(
    storage: R25PersistentStorageIdentityV1,
    frontier: u64,
    effects: R25ComputeEffectsV1,
) -> R25PrepareComputeRequestV1 {
    R25PrepareComputeRequestV1 {
        expected_storage: storage,
        expected_frontier_generation: frontier,
        range: full_range(),
        effects,
    }
}

fn model() -> R25PersistentComputeStorageBridgeModelV1 {
    R25PersistentComputeStorageBridgeModelV1::new_full_h2d_ready_model_only(
        identity(7),
        STORAGE_BYTES,
    )
    .unwrap()
}

fn complete(
    model: &mut R25PersistentComputeStorageBridgeModelV1,
    key: R25StorageBridgeKeyV1,
    authorization: R25DerivedStorageAuthorizationV1,
) {
    model
        .publish_model_only(key, R25PublishDispositionV1::Published)
        .unwrap();
    model
        .observe_completion_model_only(
            key,
            R25CompletionDispositionV1::Completed(R25CompletionObservationV1 {
                key,
                range: full_range(),
                authorization,
            }),
        )
        .unwrap();
}

#[test]
fn exact_full_h2d_compute_restore_device_chain_retains_storage_identity() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, true)))
        .unwrap();
    assert_eq!(
        model.snapshot().phase,
        R25StorageBridgePhaseV1::PreparedCompute
    );
    assert_eq!(model.snapshot().storage, storage);
    model
        .publish_model_only(key, R25PublishDispositionV1::Published)
        .unwrap();
    assert_eq!(model.snapshot().phase, R25StorageBridgePhaseV1::Published);
    model
        .observe_completion_model_only(
            key,
            R25CompletionDispositionV1::Completed(R25CompletionObservationV1 {
                key,
                range: full_range(),
                authorization: R25DerivedStorageAuthorizationV1::ReadWrite,
            }),
        )
        .unwrap();
    assert_eq!(model.snapshot().phase, R25StorageBridgePhaseV1::Completed);
    model
        .restore_model_only(key, R25RestoreDispositionV1::Restored)
        .unwrap();
    assert_eq!(model.snapshot().phase, R25StorageBridgePhaseV1::Restored);
    model.retire_exact_frontier_model_only(key).unwrap();
    let final_state = model.snapshot();
    assert_eq!(final_state.phase, R25StorageBridgePhaseV1::Device);
    assert_eq!(final_state.storage, storage);
    assert_eq!(final_state.retired_frontier_generation, 1);
    assert_eq!(final_state.generic_materialization_count, 0);
    model.validate_global_invariants().unwrap();
}

#[test]
fn authorization_is_derived_from_effects() {
    assert_eq!(
        effects(true, false).derived_authorization(),
        Some(R25DerivedStorageAuthorizationV1::Read)
    );
    assert_eq!(
        effects(false, true).derived_authorization(),
        Some(R25DerivedStorageAuthorizationV1::Write)
    );
    assert_eq!(
        effects(true, true).derived_authorization(),
        Some(R25DerivedStorageAuthorizationV1::ReadWrite)
    );
    assert_eq!(effects(false, false).derived_authorization(), None);
}

#[test]
fn reads_require_initialized_storage_but_full_writes_do_not() {
    let storage = identity(7);
    let mut model = R25PersistentComputeStorageBridgeModelV1::new_quiescent_device_model_only(
        storage,
        STORAGE_BYTES,
        false,
    )
    .unwrap();
    let before = model.snapshot();
    assert_eq!(
        model.prepare_compute_model_only(request(storage, 0, effects(true, false))),
        Err(R25PersistentComputeStorageBridgeErrorV1::ReadRequiresInitialization)
    );
    assert_eq!(model.snapshot(), before);
    assert!(
        model
            .prepare_compute_model_only(request(storage, 0, effects(false, true)))
            .is_ok()
    );
}

#[test]
fn invalid_extent_and_empty_effects_fail_atomically() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let before = model.snapshot();
    let mut invalid_range = request(storage, 0, effects(true, false));
    invalid_range.range.logical_bytes -= 1;
    assert_eq!(
        model.prepare_compute_model_only(invalid_range),
        Err(R25PersistentComputeStorageBridgeErrorV1::InvalidRange)
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(
        model.prepare_compute_model_only(request(storage, 0, effects(false, false))),
        Err(R25PersistentComputeStorageBridgeErrorV1::InvalidEffects)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn storage_and_frontier_substitution_fail_atomically() {
    let mut model = model();
    let before = model.snapshot();
    assert_eq!(
        model.prepare_compute_model_only(request(identity(8), 0, effects(true, false))),
        Err(R25PersistentComputeStorageBridgeErrorV1::StorageSubstitution)
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(
        model.prepare_compute_model_only(request(identity(7), 1, effects(true, false))),
        Err(R25PersistentComputeStorageBridgeErrorV1::StaleGeneration)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn retryable_publish_is_exactly_no_effect() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    let prepared = model.snapshot();
    assert_eq!(
        model.publish_model_only(key, R25PublishDispositionV1::RetryableNoEffect),
        Err(R25PersistentComputeStorageBridgeErrorV1::Retryable)
    );
    assert_eq!(model.snapshot(), prepared);
}

#[test]
fn pending_completion_retains_exact_published_state() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    model
        .publish_model_only(key, R25PublishDispositionV1::Published)
        .unwrap();
    let published = model.snapshot();
    model
        .observe_completion_model_only(key, R25CompletionDispositionV1::Pending)
        .unwrap();
    assert_eq!(model.snapshot(), published);
}

#[test]
fn completion_metadata_substitution_quarantines() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    model
        .publish_model_only(key, R25PublishDispositionV1::Published)
        .unwrap();
    let mut range = full_range();
    range.logical_bytes -= 1;
    assert_eq!(
        model.observe_completion_model_only(
            key,
            R25CompletionDispositionV1::Completed(R25CompletionObservationV1 {
                key,
                range,
                authorization: R25DerivedStorageAuthorizationV1::Read,
            })
        ),
        Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
    );
    assert_eq!(model.snapshot().phase, R25StorageBridgePhaseV1::Quarantined);
    model.validate_global_invariants().unwrap();
}

#[test]
fn ambiguous_publication_and_post_retention_faults_quarantine() {
    for disposition in [
        R25PublishDispositionV1::AmbiguousFailure,
        R25PublishDispositionV1::PostRetentionFault,
    ] {
        let mut model = model();
        let storage = model.snapshot().storage;
        let key = model
            .prepare_compute_model_only(request(storage, 0, effects(true, false)))
            .unwrap();
        assert_eq!(
            model.publish_model_only(key, disposition),
            Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
        );
        assert_eq!(model.snapshot().phase, R25StorageBridgePhaseV1::Quarantined);
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn terminal_quarantine_is_absorbing() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    let _ = model.publish_model_only(key, R25PublishDispositionV1::AmbiguousFailure);
    let quarantined = model.snapshot();
    assert_eq!(
        model.publish_model_only(key, R25PublishDispositionV1::Published),
        Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
    );
    assert_eq!(model.snapshot(), quarantined);
    assert_eq!(
        model.restore_model_only(key, R25RestoreDispositionV1::Restored),
        Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
    );
    assert_eq!(model.snapshot(), quarantined);
}

#[test]
fn restore_requires_authenticated_completion() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    model
        .publish_model_only(key, R25PublishDispositionV1::Published)
        .unwrap();
    let published = model.snapshot();
    assert_eq!(
        model.restore_model_only(key, R25RestoreDispositionV1::Restored),
        Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase)
    );
    assert_eq!(model.snapshot(), published);
}

#[test]
fn retryable_restore_is_exactly_no_effect() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    complete(&mut model, key, R25DerivedStorageAuthorizationV1::Read);
    let completed = model.snapshot();
    assert_eq!(
        model.restore_model_only(key, R25RestoreDispositionV1::RetryableNoEffect),
        Err(R25PersistentComputeStorageBridgeErrorV1::Retryable)
    );
    assert_eq!(model.snapshot(), completed);
}

#[test]
fn exact_frontier_is_required_for_retirement() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    complete(&mut model, key, R25DerivedStorageAuthorizationV1::Read);
    model
        .restore_model_only(key, R25RestoreDispositionV1::Restored)
        .unwrap();
    let restored = model.snapshot();
    let stale = R25StorageBridgeKeyV1 {
        operation_generation: key.operation_generation + 1,
        ..key
    };
    assert_eq!(
        model.retire_exact_frontier_model_only(stale),
        Err(R25PersistentComputeStorageBridgeErrorV1::StaleGeneration)
    );
    assert_eq!(model.snapshot(), restored);
    model.retire_exact_frontier_model_only(key).unwrap();
}

#[test]
fn retired_generation_rejects_aba_and_next_operation_advances() {
    let mut model = model();
    let storage = model.snapshot().storage;
    let key1 = model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    complete(&mut model, key1, R25DerivedStorageAuthorizationV1::Read);
    model
        .restore_model_only(key1, R25RestoreDispositionV1::Restored)
        .unwrap();
    model.retire_exact_frontier_model_only(key1).unwrap();
    let after_retirement = model.snapshot();
    assert_eq!(
        model.prepare_compute_model_only(request(storage, 0, effects(true, false))),
        Err(R25PersistentComputeStorageBridgeErrorV1::StaleGeneration)
    );
    assert_eq!(model.snapshot(), after_retirement);
    let key2 = model
        .prepare_compute_model_only(request(storage, 1, effects(true, false)))
        .unwrap();
    assert_eq!(key2.operation_generation, 2);
}

#[test]
fn fast_path_never_materializes_or_falls_back() {
    let mut model = model();
    let storage = model.snapshot().storage;
    model
        .prepare_compute_model_only(request(storage, 0, effects(true, false)))
        .unwrap();
    let selected = model.snapshot();
    assert_eq!(
        model.attempt_generic_materialization_model_only(),
        Err(R25PersistentComputeStorageBridgeErrorV1::FastPathFallbackForbidden)
    );
    assert_eq!(model.snapshot(), selected);
    assert_eq!(model.snapshot().generic_materialization_count, 0);
}

#[test]
fn write_completion_initializes_storage_at_exact_retirement() {
    let storage = identity(7);
    let mut model = R25PersistentComputeStorageBridgeModelV1::new_quiescent_device_model_only(
        storage,
        STORAGE_BYTES,
        false,
    )
    .unwrap();
    let key = model
        .prepare_compute_model_only(request(storage, 0, effects(false, true)))
        .unwrap();
    assert!(!model.snapshot().initialized);
    complete(&mut model, key, R25DerivedStorageAuthorizationV1::Write);
    model
        .restore_model_only(key, R25RestoreDispositionV1::Restored)
        .unwrap();
    assert!(!model.snapshot().initialized);
    model.retire_exact_frontier_model_only(key).unwrap();
    assert!(model.snapshot().initialized);
}

#[test]
fn constructor_bounds_are_checked() {
    assert!(matches!(
        R25PersistentComputeStorageBridgeModelV1::new_full_h2d_ready_model_only(identity(0), 1),
        Err(R25PersistentComputeStorageBridgeErrorV1::InvalidStorage)
    ));
    assert!(matches!(
        R25PersistentComputeStorageBridgeModelV1::new_full_h2d_ready_model_only(identity(1), 0),
        Err(R25PersistentComputeStorageBridgeErrorV1::InvalidStorage)
    ));
    assert!(matches!(
        R25PersistentComputeStorageBridgeModelV1::new_full_h2d_ready_model_only(
            identity(1),
            R25_MAX_STORAGE_BYTES_V1 + 1,
        ),
        Err(R25PersistentComputeStorageBridgeErrorV1::InvalidStorage)
    ));
}
