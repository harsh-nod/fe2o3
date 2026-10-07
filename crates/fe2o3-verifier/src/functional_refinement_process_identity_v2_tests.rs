#[test]
fn closed_fill_policy_binds_limits_command_and_distinct_execution_identity() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as P;
    let closed = P::ClosedConditionalFillV1;
    assert_eq!(closed.max_total(), 12);
    assert_eq!(closed.max_live(), 2);
    assert_eq!(closed.verifier_thread_stack_bytes(), 32 * 1024 * 1024);
    assert_eq!(closed.extra_verifier_arguments(), ["-V", "no-bv-simplify"]);
    assert_eq!(closed.canonical_bytes(), b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/CLOSED-CONDITIONAL-FILL/V1\0pinned-verus-b677dd5;max-total=12;max-live=2;num-threads=1;no-bv-simplify;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal;verifier-thread-stack-max=33554432;other-thread-stack-max=33554432;process-stack-max=33554432");
    let source = CanonicalGeneratedVerusProofInputV3::new(
        b"verus! { proof fn check() { assert(true); } }\n".to_vec(),
    )
    .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(15)).unwrap();
    let mut observed = output(0, b"verification results:: 1 verified, 0 errors\n", b"");
    observed.policy = closed;
    let closed_execution = execution_identity_for_runtime([0x3a; 32], &source, binding, &observed);
    for other in [
        P::LegacySingleSolverV1,
        P::PinnedSingleThreadContextsV2,
        P::PinnedSingleThreadContextsV3,
    ] {
        assert!(other.extra_verifier_arguments().is_empty());
        assert_ne!(closed.canonical_bytes(), other.canonical_bytes());
        for (domain, configuration) in [
            (
                VERUS_CONFIGURATION_DOMAIN,
                b"sealed-generated-source-fd;fixed-env".as_slice(),
            ),
            (
                SOLVER_CONFIGURATION_DOMAIN,
                b"rust_verify-managed-z3;fixed-env".as_slice(),
            ),
        ] {
            assert_ne!(
                process_configuration_digest(domain, configuration, closed),
                process_configuration_digest(domain, configuration, other)
            );
        }
        observed.policy = other;
        assert_ne!(
            closed_execution,
            execution_identity_for_runtime([0x3a; 32], &source, binding, &observed)
        );
    }
}

#[test]
fn closed_fill_boundary_refuses_reciprocal_process_policy_substitutions() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as P;
    use FunctionalRefinementBoundaryV2 as B;
    for policy in [
        P::ClosedConditionalFillV1,
        P::LegacySingleSolverV1,
        P::PinnedSingleThreadContextsV2,
        P::PinnedSingleThreadContextsV3,
    ] {
        for boundary in [
            B::SafeReferenceMirToKernelMir,
            B::SafeReferenceMirToLivePliron,
            B::SafeReferenceMirToLivePlironConditionalCoverage,
            B::SemanticMirToGfx942FillDispatchConditional,
            B::FinalKernelIrToGfx942FillDispatchConditional,
        ] {
            assert_eq!(
                require_composition_process_policy(policy, boundary).is_ok(),
                (policy == P::ClosedConditionalFillV1)
                    == matches!(
                        boundary,
                        B::SemanticMirToGfx942FillDispatchConditional
                            | B::FinalKernelIrToGfx942FillDispatchConditional
                    )
            );
        }
    }
}

#[test]
fn context_policy_preserves_legacy_config_hashes_and_versions_both_configurations() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2::{
        LegacySingleSolverV1, PinnedSingleThreadContextsV2,
    };
    for (domain, configuration) in [
        (
            b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-RUST-VERIFY-CONFIG/V2\0".as_slice(),
            b"sealed-generated-source-fd;fixed-env".as_slice(),
        ),
        (
            b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-Z3-CONFIG/V2\0".as_slice(),
            b"rust_verify-managed-z3;fixed-env".as_slice(),
        ),
    ] {
        let mut old = Sha256::new();
        for bytes in [domain, configuration] {
            old.update((bytes.len() as u64).to_le_bytes());
            old.update(bytes);
        }
        let old = DigestV1::from_untrusted_bytes(old.finalize().into());
        assert_eq!(
            process_configuration_digest(domain, configuration, LegacySingleSolverV1),
            old
        );
        assert_ne!(
            process_configuration_digest(domain, configuration, PinnedSingleThreadContextsV2),
            old
        );
    }
    assert_eq!(PinnedSingleThreadContextsV2.max_total(), 4096);
    assert_eq!(PinnedSingleThreadContextsV2.max_live(), 2);
    assert_eq!(PinnedSingleThreadContextsV2.canonical_bytes(), b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal");
}

#[test]
fn context_policy_preserves_exact_legacy_execution_transcript_and_versions_new_outputs() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2;
    let source = CanonicalGeneratedVerusProofInputV3::new(
        b"verus! { proof fn check() { assert(true); } }\n".to_vec(),
    )
    .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(15)).unwrap();
    let mut observed = output(0, b"verification results:: 1 verified, 0 errors\n", b"");
    let runtime_identity = [0x3a; 32];
    let mut legacy = Sha256::new();
    legacy.update(b"FE2O3/FUNCTIONAL-REFINEMENT/VERUS-EXECUTION/V2\0");
    let mut blob = |bytes: &[u8]| {
        legacy.update((bytes.len() as u64).to_le_bytes());
        legacy.update(bytes);
    };
    blob(&runtime_identity);
    blob(&source.identity().as_bytes());
    blob(source.source());
    for value in [
        binding.safe_reference_identity(),
        binding.safe_reference_source_hash(),
        binding.safe_reference_mir_hash(),
        binding.kernel_subject_identity(),
        binding.kernel_mir_hash(),
        binding.normalized_obligation_effect_ir_hash(),
    ] {
        blob(value.as_bytes());
    }
    legacy.update(0_i32.to_le_bytes());
    legacy.update(0_i32.to_le_bytes());
    for bytes in [&observed.stdout, &observed.stderr] {
        legacy.update((bytes.len() as u64).to_le_bytes());
        legacy.update(bytes);
    }
    let legacy = DigestV1::from_untrusted_bytes(legacy.finalize().into());
    assert_eq!(
        execution_identity_for_runtime(runtime_identity, &source, binding, &observed),
        legacy
    );
    observed.policy = PinnedSingleThreadContextsV2;
    assert_ne!(
        execution_identity_for_runtime(runtime_identity, &source, binding, &observed),
        legacy
    );
}

#[test]
fn legacy_receipt_import_policy_refuses_context_policy_even_with_same_tools_and_signature() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2::{
        ClosedConditionalFillV1, LegacySingleSolverV1, PinnedSingleThreadContextsV2,
        PinnedSingleThreadContextsV3,
    };
    let toolchain = |policy| {
        VerusToolchainIdentityV2::new(
            DigestV1::from_untrusted_bytes(VERUS_EXECUTABLE_SHA256),
            process_configuration_digest(
                VERUS_CONFIGURATION_DOMAIN,
                b"sealed-generated-source-fd;fixed-env",
                policy,
            ),
            DigestV1::from_untrusted_bytes(SOLVER_EXECUTABLE_SHA256),
            process_configuration_digest(
                SOLVER_CONFIGURATION_DOMAIN,
                b"rust_verify-managed-z3;fixed-env",
                policy,
            ),
            digest(14),
        )
        .unwrap()
    };
    // This public fixture key tests receipt association, not protected execution.
    let signing = SigningKey::from_bytes(&[0x69; 32]);
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(15)).unwrap();
    for accepted in [
        ClosedConditionalFillV1,
        LegacySingleSolverV1,
        PinnedSingleThreadContextsV2,
        PinnedSingleThreadContextsV3,
    ] {
        let policy = FunctionalRefinementImportPolicyV2::new(
            signing.verifying_key().to_bytes(),
            toolchain(accepted),
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        for observed in [
            ClosedConditionalFillV1,
            LegacySingleSolverV1,
            PinnedSingleThreadContextsV2,
            PinnedSingleThreadContextsV3,
        ] {
            let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
                policy.signer_identity(),
                binding,
                toolchain(observed),
                digest(16),
                FunctionalRefinementResultV2::Proved,
                FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
            )
            .unwrap();
            let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
            let wire = unsigned.attach_signature(signature);
            let result = FunctionalRefinementReceiptImporterV2::new(policy.clone(), 1)
                .unwrap()
                .import(FunctionalRefinementImportExpectationV2::new(binding), &wire);
            if accepted == observed {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(FunctionalRefinementImportErrorV2::WrongToolchain)
                ));
            }
        }
    }
}

#[test]
fn interpreter_stack_policy_preserves_v2_config_transcripts_and_versions_v3_exactly() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as Policy;
    let v2 = b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal";
    let v3 = b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V3\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal;verifier-thread-stack-max=1073741824;other-thread-stack-max=33554432;process-stack-max=33554432";
    assert_eq!(Policy::PinnedSingleThreadContextsV2.canonical_bytes(), v2);
    assert_eq!(Policy::PinnedSingleThreadContextsV3.canonical_bytes(), v3);
    for (domain, configuration) in [
        (
            b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-RUST-VERIFY-CONFIG/V2\0".as_slice(),
            b"sealed-generated-source-fd;fixed-env".as_slice(),
        ),
        (
            b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-Z3-CONFIG/V2\0".as_slice(),
            b"rust_verify-managed-z3;fixed-env".as_slice(),
        ),
    ] {
        let mut identities = Vec::new();
        for (policy, outer, bytes) in [
            (
                Policy::PinnedSingleThreadContextsV2,
                b"FE2O3/FUNCTIONAL-REFINEMENT/PROCESS-POLICY-CONFIG/V3\0".as_slice(),
                v2.as_slice(),
            ),
            (
                Policy::PinnedSingleThreadContextsV3,
                b"FE2O3/FUNCTIONAL-REFINEMENT/PROCESS-POLICY-CONFIG/V4\0".as_slice(),
                v3.as_slice(),
            ),
        ] {
            let mut transcript = Vec::new();
            for value in [outer, domain, configuration, bytes] {
                transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
                transcript.extend_from_slice(value);
            }
            let expected = DigestV1::from_untrusted_bytes(Sha256::digest(transcript).into());
            assert_eq!(
                process_configuration_digest(domain, configuration, policy),
                expected
            );
            assert_ne!(
                expected,
                process_configuration_digest(domain, configuration, Policy::LegacySingleSolverV1)
            );
            identities.push(expected);
        }
        assert_ne!(identities[0], identities[1]);
    }
    assert_eq!(Policy::PinnedSingleThreadContextsV3.max_total(), 4096);
    assert_eq!(Policy::PinnedSingleThreadContextsV3.max_live(), 2);
    assert_eq!(
        Policy::PinnedSingleThreadContextsV3.verifier_thread_stack_bytes(),
        1_073_741_824
    );
}

#[test]
fn interpreter_stack_policy_preserves_v2_execution_transcript_and_versions_v3_exactly() {
    use crate::retained_functional_refinement_runtime_v1::GeneratedProofProcessPolicyV2 as Policy;
    let source = CanonicalGeneratedVerusProofInputV3::new(
        b"verus! { proof fn check() { assert(true); } }\n".to_vec(),
    )
    .unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(15)).unwrap();
    let mut observed = output(0, b"verification results:: 1 verified, 0 errors\n", b"");
    let runtime_identity = [0x3a; 32];
    let mut identities = Vec::new();
    for (policy, domain, bytes) in [
        (Policy::PinnedSingleThreadContextsV2, b"FE2O3/FUNCTIONAL-REFINEMENT/VERUS-EXECUTION/V3\0".as_slice(), b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V2\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal".as_slice()),
        (Policy::PinnedSingleThreadContextsV3, b"FE2O3/FUNCTIONAL-REFINEMENT/VERUS-EXECUTION/V4\0".as_slice(), b"FE2O3/GENERATED-PROOF/PROCESS-POLICY/V3\0pinned-verus-b677dd5;max-total=4096;max-live=2;num-threads=1;direct-verifier-children;exact-retained-exec-fd-maps;authenticated-terminal;verifier-thread-stack-max=1073741824;other-thread-stack-max=33554432;process-stack-max=33554432".as_slice()),
    ] {
        let mut transcript = domain.to_vec();
        let mut append = |value: &[u8]| {
            transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
            transcript.extend_from_slice(value);
        };
        append(bytes);
        append(&runtime_identity);
        append(&source.identity().as_bytes());
        append(source.source());
        for value in [binding.safe_reference_identity(), binding.safe_reference_source_hash(), binding.safe_reference_mir_hash(), binding.kernel_subject_identity(), binding.kernel_mir_hash(), binding.normalized_obligation_effect_ir_hash()] {
            append(value.as_bytes());
        }
        transcript.extend_from_slice(&[0; 8]);
        for value in [&observed.stdout, &observed.stderr] {
            transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
            transcript.extend_from_slice(value);
        }
        let expected = DigestV1::from_untrusted_bytes(Sha256::digest(transcript).into());
        observed.policy = policy;
        assert_eq!(execution_identity_for_runtime(runtime_identity, &source, binding, &observed), expected);
        identities.push(expected);
    }
    observed.policy = Policy::LegacySingleSolverV1;
    identities.push(execution_identity_for_runtime(
        runtime_identity,
        &source,
        binding,
        &observed,
    ));
    for i in 0..identities.len() {
        for j in 0..identities.len() {
            assert_eq!(identities[i] == identities[j], i == j);
        }
    }
}
