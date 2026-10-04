//! Codec-only negatives do not construct an executed owner or impersonate a runtime.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn expected(key: [u8; 32]) -> Expected {
    Expected {
        binding: Binding {
            policy: 11,
            source: [[1; 32], [2; 32]],
            graphs: [
                ([3; 32], 120),
                ([4; 32], 100),
                ([5; 32], 100),
                ([12; 32], 90),
            ],
            statement: [6; 32],
            generated: [7; 32],
            witness: [8; 32],
            context: [13; 32],
            witness_bytes: 1252,
            rounds: 3,
            census: [2, 5, 30, 2, 2, 4],
        },
        runtime: [9; 32],
        toolchain: [[10; 32]; 5],
        execution: [11; 32],
        key,
    }
}
fn sign(value: &Expected, key: &SigningKey) -> [u8; WIRE] {
    let unsigned = value.unsigned();
    let mut wire = [0; WIRE];
    wire[..UNSIGNED].copy_from_slice(&unsigned);
    wire[UNSIGNED..].copy_from_slice(&key.sign(&unsigned).to_bytes());
    wire
}

#[test]
fn typed_source_execution_identity_preserves_legacy_transcript_and_binds_process_policy() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as Policy;

    let old_domain = b"FE2O3/V18/POLICY11/TYPED-SOURCE-TAIL/EXECUTION/V50\0";
    let new_domain = b"FE2O3/V18/POLICY11/TYPED-SOURCE-TAIL/EXECUTION/V66\0";
    let policy = b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal";
    assert_eq!(
        Policy::PinnedSingleThreadContextsV2.canonical_bytes(),
        policy
    );
    let stdout = b"verification results:: 1 verified, 0 errors\n";
    let runtime = [9; 32];
    let generated = [7; 32];
    let statement = [6; 32];
    let fields: [&[u8]; 5] = [&runtime, &generated, &statement, stdout, b""];
    let actual = |policy| -> [u8; 32] {
        let mut digest = Sha256::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        bind_execution_policy(&mut digest, policy, &mut budget).unwrap();
        for bytes in fields {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest.update(0i32.to_le_bytes());
        digest.update(0i32.to_le_bytes());
        digest.finalize().into()
    };

    // Reconstruct the historical byte transcript without the production policy helper.
    let mut old = old_domain.to_vec();
    for bytes in [&runtime, &generated, &statement] {
        old.extend_from_slice(&32u64.to_le_bytes());
        old.extend_from_slice(bytes);
    }
    old.extend_from_slice(&(stdout.len() as u64).to_le_bytes());
    old.extend_from_slice(stdout);
    old.extend_from_slice(&0u64.to_le_bytes());
    old.extend_from_slice(&[0; 8]);
    let legacy: [u8; 32] = Sha256::digest(&old).into();
    assert_eq!(actual(Policy::LegacySingleSolverV1), legacy);

    let mut contexts = new_domain.to_vec();
    contexts.extend_from_slice(&(policy.len() as u64).to_le_bytes());
    contexts.extend_from_slice(policy);
    contexts.extend_from_slice(&old[old_domain.len()..]);
    let contexts: [u8; 32] = Sha256::digest(&contexts).into();
    assert_eq!(actual(Policy::PinnedSingleThreadContextsV2), contexts);
    assert_ne!(legacy, contexts);

    // Inert codec fixtures isolate this coordinate; they do not execute a proof.
    let key = SigningKey::from_bytes(&[45; 32]);
    let mut legacy_expected = expected(key.verifying_key().to_bytes());
    legacy_expected.execution = legacy;
    let mut contexts_expected = legacy_expected;
    contexts_expected.execution = contexts;
    let legacy_wire = sign(&legacy_expected, &key);
    let contexts_wire = sign(&contexts_expected, &key);
    import(&legacy_expected, &legacy_wire).unwrap();
    import(&contexts_expected, &contexts_wire).unwrap();
    assert!(import(&legacy_expected, &contexts_wire).is_err());
    assert!(import(&contexts_expected, &legacy_wire).is_err());
}

#[test]
fn typed_source_execution_policy_hash_has_exact_incremental_work_and_sticky_refusal() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as Policy;

    let policy = b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal";
    let expected_work = 8 + policy.len();
    assert_eq!(EXECUTION_DOMAIN.len(), EXECUTION_DOMAIN_V66.len());
    for limit in [expected_work, expected_work - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        let mut digest = Sha256::new();
        let before: [u8; 32] = digest.clone().finalize().into();
        let result = bind_execution_policy(
            &mut digest,
            Policy::PinnedSingleThreadContextsV2,
            &mut budget,
        );
        if limit == expected_work {
            result.unwrap();
            assert_eq!(budget.work(), expected_work);
            assert!(budget.check_prior_denials_v1().is_ok());
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(error)))
                if error.limit() == limit && error.actual() == expected_work));
            assert_eq!(budget.failed_work(), Some(expected_work));
            assert_eq!(<[u8; 32]>::from(digest.clone().finalize()), before);
            // A cheaper later attempt cannot erase the recorded original refusal.
            assert!(budget.charge_work(0).is_ok());
            let retry = bind_execution_policy(
                &mut digest,
                Policy::PinnedSingleThreadContextsV2,
                &mut budget,
            );
            assert!(matches!(retry, Err(Error::Resource(Resource::Work(error)))
                if error.limit() == limit && error.actual() == expected_work));
            assert_eq!(budget.failed_work(), Some(expected_work));
            assert_eq!(<[u8; 32]>::from(digest.finalize()), before);
        }
        assert_eq!(budget.storage(), 31);
    }
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    bind_execution_policy(
        &mut Sha256::new(),
        Policy::LegacySingleSolverV1,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_work(), None);
}

#[test]
fn typed_source_receipt_rejects_every_modified_and_resigned_owner_statement_and_runtime_byte() {
    let key = SigningKey::from_bytes(&[41; 32]);
    let expected = expected(key.verifying_key().to_bytes());
    let wire = sign(&expected, &key);
    import(&expected, &wire).unwrap();
    for index in 0..WIRE {
        let mut changed = wire;
        changed[index] ^= 1;
        assert!(
            import(&expected, &changed).is_err(),
            "modified byte {index}"
        );
        if index < UNSIGNED {
            let signed = key.sign(&changed[..UNSIGNED]).to_bytes();
            changed[UNSIGNED..].copy_from_slice(&signed);
            assert!(
                import(&expected, &changed).is_err(),
                "resigned byte {index}"
            );
        }
    }
    assert!(import(&expected, &wire[..WIRE - 1]).is_err());
    let mut trailing = wire.to_vec();
    trailing.push(0);
    assert!(import(&expected, &trailing).is_err());
    let foreign = SigningKey::from_bytes(&[42; 32]);
    assert!(import(&expected, &sign(&expected, &foreign)).is_err());
}

#[test]
fn typed_source_receipt_refuses_historical_policy_empty_or_unbounded_scope_even_when_signed() {
    let key = SigningKey::from_bytes(&[43; 32]);
    let original = expected(key.verifying_key().to_bytes());
    for axis in 0..9 {
        let mut changed = original;
        match axis {
            0 => changed.binding.policy = 10,
            1 => changed.binding.rounds = 0,
            2 => changed.binding.rounds = 33,
            3 => changed.binding.witness_bytes = 0,
            4 => changed.binding.graphs[0].1 = 0,
            5 => changed.binding.graphs[1].1 = 0,
            6 => changed.binding.graphs[2].1 = 0,
            7 => changed.binding.graphs[3].1 = 0,
            _ => changed.binding.census[0] = 0,
        }
        assert!(
            import(&changed, &sign(&changed, &key)).is_err(),
            "axis {axis}"
        );
    }
    let mut old_boundary = sign(&original, &key);
    old_boundary[MAGIC.len() + 2] = 2;
    let signed = key.sign(&old_boundary[..UNSIGNED]).to_bytes();
    old_boundary[UNSIGNED..].copy_from_slice(&signed);
    assert!(import(&original, &old_boundary).is_err());
}

#[test]
fn typed_source_receipt_import_has_exact_and_one_short_independent_work_bound() {
    let key = SigningKey::from_bytes(&[44; 32]);
    let value = expected(key.verifying_key().to_bytes());
    let wire = sign(&value, &key);
    let expected_work = WIRE + UNSIGNED;
    for limit in [expected_work, expected_work - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = import_metered(&value, &wire, &mut budget);
        if limit == expected_work {
            result.unwrap();
            assert_eq!(budget.work(), expected_work);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(error)))
                if error.limit() == limit && error.actual() == expected_work));
        }
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn typed_source_execution_preparation_and_executed_receipt_are_distinct_nominal_owners() {
    use std::any::TypeId;
    assert_ne!(
        TypeId::of::<
            PreparedTypedSourceTailExecutionV50<
                'static,
                'static,
                'static,
                'static,
                'static,
                'static,
            >,
        >(),
        TypeId::of::<
            ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>,
        >(),
    );
    assert_ne!(
        TypeId::of::<
            ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>,
        >(),
        TypeId::of::<crate::ExecutedMixedPureCseCfgRefinementV27<'static, 'static, 'static>>(),
    );
    assert!(headers().unwrap() >= 2 * OUTPUT_LIMIT + 2 * UNSIGNED + WIRE);
    assert_ne!(
        TypeId::of::<
            ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>,
        >(),
        TypeId::of::<
            crate::ExecutedMixedComposedRefinementV29<
                'static,
                'static,
                'static,
                'static,
                'static,
                'static,
            >,
        >(),
    );
    assert_ne!(
        TypeId::of::<PreparedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static>>(),
        TypeId::of::<crate::PreparedOriginalSemanticMirRefinementV36<'static, 'static>>(),
    );
}

#[test]
fn typed_source_execution_frames_include_preparation_results_and_typed_captures() {
    type Prepared =
        PreparedTypedSourceTailExecutionV50<'static, 'static, 'static, 'static, 'static, 'static>;
    type Executed =
        ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>;
    let captures = 2 * (2 * size_of::<&()>() + size_of::<usize>())
        + 2 * 3 * size_of::<&()>()
        + 2 * align_of::<&()>();
    let expected = size_of::<Prepared>()
        + size_of::<Executed>()
        + size_of::<Binding>()
        + size_of::<Expected>()
        + size_of::<FunctionalRefinementAttemptV1>()
        + size_of::<FunctionalRefinementRuntimeProcessOutputV1>()
        + size_of::<VerusToolchainIdentityV2>()
        + size_of::<SigningKey>()
        + size_of::<VerifyingKey>()
        + 2 * size_of::<Signature>()
        + size_of::<Sha256>()
        + 2 * size_of::<Instant>()
        + size_of::<Duration>()
        + 5 * size_of::<&[u8]>()
        + 8 * size_of::<usize>()
        + 4 * size_of::<super::super::Identity>()
        + 6 * size_of::<usize>()
        + 2 * UNSIGNED
        + WIRE
        + 4 * OUTPUT_LIMIT
        + size_of::<Result<Binding>>()
        + size_of::<std::thread::Result<Result<Binding>>>()
        + size_of::<Result<(Expected, [u8; WIRE])>>()
        + size_of::<std::thread::Result<Result<(Expected, [u8; WIRE])>>>()
        + size_of::<Result<Executed>>()
        + size_of::<Result<Prepared>>()
        + 3 * size_of::<Result<()>>()
        + captures;
    assert_eq!(headers().unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn typed_source_execution_missing_runtime_is_an_explicit_error_without_a_receipt() {
    // A child of an ordinary device file cannot be an admitted runtime root.
    // This exercises admission only and never starts a verifier or solver.
    let result = Runtime::open("/dev/null/fe2o3-typed-source-v50-unavailable").map_err(runtime);
    assert!(matches!(
        result,
        Err(Error::Generation(ProofError::Runtime(_)))
    ));
}
