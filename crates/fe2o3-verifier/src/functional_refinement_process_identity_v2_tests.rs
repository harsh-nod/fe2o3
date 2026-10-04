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
        LegacySingleSolverV1, PinnedSingleThreadContextsV2,
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
    for accepted in [LegacySingleSolverV1, PinnedSingleThreadContextsV2] {
        let policy = FunctionalRefinementImportPolicyV2::new(
            signing.verifying_key().to_bytes(),
            toolchain(accepted),
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        for observed in [LegacySingleSolverV1, PinnedSingleThreadContextsV2] {
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
