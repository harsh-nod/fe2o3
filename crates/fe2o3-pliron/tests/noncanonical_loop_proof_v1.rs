#![cfg(feature = "internal-proof-staging")]

use dialect_gpu::{AddressSpaceAttr, MemoryOrderAttr, MemoryScopeAttr};
use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, ImportedFunctionalRefinementProofV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_pliron::{
    ProductionNonCanonicalLoopClaimsV1, ProductionNonCanonicalLoopProofErrorV1,
    ProductionNonCanonicalLoopProofRequestV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionRefinementStagingPolicyV2,
    derive_noncanonical_loop_proof_request_v1, derive_noncanonical_loop_proof_requirement_v1,
    import_noncanonical_loop_proof_v1,
};
use fe2o3_proof_contracts::DigestV1;

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Cfg,
    Guard,
    Transition,
    CarriedValue,
    LoopOperation,
    Membership,
}

fn digest(tag: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([tag; 32])
}

fn local(identity: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity))
}

fn argument(block: u32, argument: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::BlockArgument { block, argument }
}

fn subjects() -> FunctionalRefinementSubjectsV2 {
    subjects_with_kernel_mir(4)
}

fn evidence_identity() -> DigestV1 {
    digest(5)
}

fn subjects_with_kernel_mir(kernel_mir: u8) -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(kernel_mir),
    )
    .unwrap()
}

fn claims(invariant: u8, variant: u8) -> ProductionNonCanonicalLoopClaimsV1 {
    ProductionNonCanonicalLoopClaimsV1::new(17, 1, digest(invariant), digest(variant)).unwrap()
}

fn kernel(mutation: Mutation) -> ProductionRankedKernelV1 {
    let entry = ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 0,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(1),
                value: 8,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(2),
                value: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(3),
                value: 7,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(4),
                value: 2,
            },
        ],
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            control_dependencies: vec![local(0)],
            first_arguments: vec![local(0), local(3)],
            second_arguments: vec![local(0), local(3)],
            first_block: 1,
            second_block: 5,
        },
    );
    let guard = if matches!(mutation, Mutation::Guard) {
        ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs: argument(1, 0),
            rhs: local(1),
            true_arguments: vec![argument(1, 0), argument(1, 1)],
            false_arguments: vec![],
            true_block: 2,
            false_block: 4,
        }
    } else {
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs: argument(1, 0),
            rhs: local(1),
            true_arguments: vec![argument(1, 0), argument(1, 1)],
            false_arguments: vec![],
            true_block: 2,
            false_block: 4,
        }
    };
    let header = ProductionRankedBlockV1::with_index_arguments(2, vec![], guard);
    let false_arguments = if matches!(mutation, Mutation::CarriedValue) {
        vec![argument(2, 1), argument(2, 0)]
    } else {
        vec![argument(2, 0), argument(2, 1)]
    };
    let body = ProductionRankedBlockV1::with_index_arguments(
        2,
        vec![ProductionRankedOperationV1::Fence {
            memory_scope: MemoryScopeAttr::Device,
            address_space: AddressSpaceAttr::Global,
            order: if matches!(mutation, Mutation::LoopOperation) {
                MemoryOrderAttr::SequentiallyConsistent
            } else {
                MemoryOrderAttr::AcquireRelease
            },
        }],
        ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs: argument(2, 0),
            rhs: local(1),
            true_arguments: vec![argument(2, 0), argument(2, 1)],
            false_arguments,
            true_block: 3,
            false_block: 5,
        },
    );
    let first_latch = ProductionRankedBlockV1::with_index_arguments(
        2,
        vec![],
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments: vec![argument(3, 0), argument(3, 1)],
            add_argument: 0,
            step: if matches!(mutation, Mutation::Transition) {
                local(4)
            } else {
                local(2)
            },
            target: if matches!(mutation, Mutation::Cfg) {
                2
            } else {
                1
            },
        },
    );
    let exit = ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return);
    let second_latch = ProductionRankedBlockV1::with_index_arguments(
        2,
        vec![],
        if matches!(mutation, Mutation::Membership) {
            ProductionRankedTerminatorV1::Branch { target: 4 }
        } else {
            ProductionRankedTerminatorV1::BranchArgsAddAt {
                arguments: vec![argument(5, 0), argument(5, 1)],
                add_argument: 0,
                step: local(2),
                target: 1,
            }
        },
    );
    ProductionRankedKernelV1::new(
        "noncanonical_loop_receipt",
        0,
        vec![entry, header, body, first_latch, exit, second_latch],
    )
    .unwrap()
}

fn loop_request(
    kernel: &ProductionRankedKernelV1,
    claims: ProductionNonCanonicalLoopClaimsV1,
) -> ProductionNonCanonicalLoopProofRequestV1 {
    derive_noncanonical_loop_proof_request_v1(kernel, claims, subjects(), evidence_identity())
        .unwrap()
}

fn imported(
    request: &ProductionNonCanonicalLoopProofRequestV1,
    boundary: FunctionalRefinementBoundaryV2,
    execution: u8,
) -> (
    ImportedFunctionalRefinementProofV2,
    ProductionRefinementStagingPolicyV2,
) {
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let import_policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let signer = import_policy.signer_identity();
    let binding = FunctionalRefinementBindingV2::from_subjects(
        request.subjects(),
        request.normalized_obligation(),
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        signer,
        binding,
        toolchain,
        digest(execution),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let wire = unsigned
        .clone()
        .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
    let mut importer = FunctionalRefinementReceiptImporterV2::new(import_policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let policy = ProductionRefinementStagingPolicyV2::new([signer], toolchain).unwrap();
    (imported, policy)
}

#[test]
fn exact_noncanonical_loop_request_and_import_are_bound_but_non_authoritative() {
    let kernel = kernel(Mutation::None);
    let request = loop_request(&kernel, claims(20, 21));
    assert_eq!(request.loop_blocks(), [1, 2, 3, 5]);
    assert_eq!(request.entry_edges(), [(0, 1), (0, 5)]);
    assert!(request.internal_edges().contains(&(3, 1)));
    assert!(request.internal_edges().contains(&(5, 1)));
    assert_eq!(request.backedges(), [(3, 1)]);
    assert_eq!(request.exit_edges(), [(1, 4)]);
    for identity in [
        request.membership_identity(),
        request.guard_identity(),
        request.transition_identity(),
        request.carried_values_identity(),
        request.operations_identity(),
        request.exact_ranked_graph_identity(),
        request.normalized_obligation(),
    ] {
        assert!(!identity.is_zero());
    }
    assert!(!request.grants_noncanonical_loop_authority());

    let (proof, policy) = imported(
        &request,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        30,
    );
    let checked = import_noncanonical_loop_proof_v1(&kernel, request, proof, &policy).unwrap();
    assert!(checked.signature_policy_and_exact_binding_checked());
    assert!(!checked.grants_noncanonical_loop_authority());
    assert!(!checked.composes_with_aggregate_functional_replay());
}

#[test]
fn exact_obligation_changes_for_every_live_cfg_component() {
    let baseline_kernel = kernel(Mutation::None);
    let baseline = loop_request(&baseline_kernel, claims(20, 21));
    for mutation in [
        Mutation::Cfg,
        Mutation::Guard,
        Mutation::Transition,
        Mutation::CarriedValue,
        Mutation::LoopOperation,
        Mutation::Membership,
    ] {
        let changed = loop_request(&kernel(mutation), claims(20, 21));
        assert_ne!(
            changed.normalized_obligation(),
            baseline.normalized_obligation()
        );
    }
    assert_ne!(
        loop_request(&kernel(Mutation::Guard), claims(20, 21)).guard_identity(),
        baseline.guard_identity()
    );
    assert_ne!(
        loop_request(&kernel(Mutation::Transition), claims(20, 21)).transition_identity(),
        baseline.transition_identity()
    );
    assert_ne!(
        loop_request(&kernel(Mutation::CarriedValue), claims(20, 21)).carried_values_identity(),
        baseline.carried_values_identity()
    );
    assert_ne!(
        loop_request(&kernel(Mutation::LoopOperation), claims(20, 21)).operations_identity(),
        baseline.operations_identity()
    );
    assert_ne!(
        loop_request(&kernel(Mutation::Membership), claims(20, 21)).loop_blocks(),
        baseline.loop_blocks()
    );
    let changed_subjects = [
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(40),
            DigestV1::ZERO,
            digest(2),
            digest(3),
            digest(4),
        )
        .unwrap(),
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(1),
            DigestV1::ZERO,
            digest(41),
            digest(3),
            digest(4),
        )
        .unwrap(),
        FunctionalRefinementSubjectsV2::new(
            SafeReferenceKindV2::Mir,
            digest(1),
            DigestV1::ZERO,
            digest(2),
            digest(42),
            digest(4),
        )
        .unwrap(),
        subjects_with_kernel_mir(43),
    ];
    for subjects in changed_subjects {
        let changed = derive_noncanonical_loop_proof_request_v1(
            &baseline_kernel,
            claims(20, 21),
            subjects,
            evidence_identity(),
        )
        .unwrap();
        assert_ne!(
            changed.normalized_obligation(),
            baseline.normalized_obligation()
        );
    }
    let changed_evidence = derive_noncanonical_loop_proof_request_v1(
        &baseline_kernel,
        claims(20, 21),
        subjects(),
        digest(44),
    )
    .unwrap();
    assert_ne!(
        changed_evidence.normalized_obligation(),
        baseline.normalized_obligation()
    );
}

#[test]
fn stale_cfg_claim_and_receipt_substitutions_fail_closed() {
    let baseline_kernel = kernel(Mutation::None);
    let baseline = loop_request(&baseline_kernel, claims(20, 21));
    let (proof, policy) = imported(
        &baseline,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        30,
    );
    assert!(matches!(
        import_noncanonical_loop_proof_v1(&kernel(Mutation::Transition), baseline, proof, &policy,),
        Err(ProductionNonCanonicalLoopProofErrorV1::StaleRequest)
    ));

    for changed_claims in [claims(22, 21), claims(20, 23)] {
        let baseline = loop_request(&baseline_kernel, claims(20, 21));
        let changed = loop_request(&baseline_kernel, changed_claims);
        let (proof, policy) = imported(
            &baseline,
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
            31,
        );
        assert!(matches!(
            import_noncanonical_loop_proof_v1(&baseline_kernel, changed, proof, &policy),
            Err(ProductionNonCanonicalLoopProofErrorV1::BindingMismatch(_))
        ));
    }

    let baseline = loop_request(&baseline_kernel, claims(20, 21));
    let swapped = loop_request(&baseline_kernel, claims(24, 25));
    let (proof, policy) = imported(
        &swapped,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        32,
    );
    assert!(matches!(
        import_noncanonical_loop_proof_v1(&baseline_kernel, baseline, proof, &policy),
        Err(ProductionNonCanonicalLoopProofErrorV1::BindingMismatch(_))
    ));

    let changed_header =
        ProductionNonCanonicalLoopClaimsV1::new(17, 2, digest(20), digest(21)).unwrap();
    assert_ne!(
        loop_request(&baseline_kernel, changed_header).normalized_obligation(),
        loop_request(&baseline_kernel, claims(20, 21)).normalized_obligation()
    );
}

#[test]
fn wrong_boundary_and_digest_only_claims_never_become_authority() {
    assert_eq!(
        ProductionNonCanonicalLoopClaimsV1::new(0, 1, digest(1), digest(2)),
        Err(ProductionNonCanonicalLoopProofErrorV1::InvalidClaim)
    );
    assert_eq!(
        ProductionNonCanonicalLoopClaimsV1::new(17, 1, DigestV1::ZERO, digest(2)),
        Err(ProductionNonCanonicalLoopProofErrorV1::InvalidClaim)
    );
    assert_eq!(
        ProductionNonCanonicalLoopClaimsV1::new(17, 1, digest(2), DigestV1::ZERO),
        Err(ProductionNonCanonicalLoopProofErrorV1::InvalidClaim)
    );
    let kernel = kernel(Mutation::None);
    assert_eq!(
        derive_noncanonical_loop_proof_requirement_v1(&kernel, 1, subjects(), DigestV1::ZERO,),
        Err(ProductionNonCanonicalLoopProofErrorV1::InvalidEvidenceIdentity)
    );
    let requirement =
        derive_noncanonical_loop_proof_requirement_v1(&kernel, 1, subjects(), evidence_identity())
            .unwrap();
    assert_eq!(
        requirement.bind_claims(
            ProductionNonCanonicalLoopClaimsV1::new(17, 2, digest(20), digest(21)).unwrap()
        ),
        Err(ProductionNonCanonicalLoopProofErrorV1::ClaimHeaderMismatch)
    );
    let request = loop_request(&kernel, claims(20, 21));
    assert!(!request.grants_noncanonical_loop_authority());
    let (proof, policy) = imported(
        &request,
        FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
        33,
    );
    assert!(matches!(
        import_noncanonical_loop_proof_v1(&kernel, request, proof, &policy),
        Err(ProductionNonCanonicalLoopProofErrorV1::WrongBoundary(_))
    ));

    let request = loop_request(&kernel, claims(20, 21));
    let (proof, _) = imported(
        &request,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        34,
    );
    let wrong_signer =
        ProductionRefinementStagingPolicyV2::new([digest(90)], proof.toolchain()).unwrap();
    assert!(matches!(
        import_noncanonical_loop_proof_v1(&kernel, request, proof, &wrong_signer),
        Err(ProductionNonCanonicalLoopProofErrorV1::WrongSigner(_))
    ));

    let request = loop_request(&kernel, claims(20, 21));
    let (proof, _) = imported(
        &request,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        35,
    );
    let wrong_toolchain =
        VerusToolchainIdentityV2::new(digest(50), digest(51), digest(52), digest(53), digest(54))
            .unwrap();
    let wrong_toolchain_policy =
        ProductionRefinementStagingPolicyV2::new([proof.signer_identity()], wrong_toolchain)
            .unwrap();
    assert!(matches!(
        import_noncanonical_loop_proof_v1(&kernel, request, proof, &wrong_toolchain_policy),
        Err(ProductionNonCanonicalLoopProofErrorV1::WrongToolchain(_))
    ));
}

#[test]
fn non_loop_and_invalid_headers_are_rejected_before_receipt_import() {
    let acyclic = ProductionRankedKernelV1::new(
        "acyclic_loop_claim",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    assert_eq!(
        derive_noncanonical_loop_proof_request_v1(
            &acyclic,
            claims(20, 21),
            subjects(),
            evidence_identity(),
        ),
        Err(ProductionNonCanonicalLoopProofErrorV1::InvalidHeader)
    );
    let claims = ProductionNonCanonicalLoopClaimsV1::new(17, 0, digest(20), digest(21)).unwrap();
    assert_eq!(
        derive_noncanonical_loop_proof_request_v1(
            &acyclic,
            claims,
            subjects(),
            evidence_identity(),
        ),
        Err(ProductionNonCanonicalLoopProofErrorV1::NotCyclic)
    );
}
