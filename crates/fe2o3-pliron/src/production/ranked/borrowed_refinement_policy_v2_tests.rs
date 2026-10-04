use super::*;

fn policy() -> ProductionRefinementStagingPolicyV2 {
    let d = DigestV1::from_untrusted_bytes([7; 32]);
    ProductionRefinementStagingPolicyV2::new(
        [d, d],
        VerusToolchainIdentityV2::new(d, d, d, d, d).unwrap(),
    )
    .unwrap()
}
fn construction(unbound: bool) -> ProductionConstructionV1 {
    let operations = if unbound {
        let d = DigestV1::from_untrusted_bytes([7; 32]);
        let local = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        vec![
            ProductionRankedOperationV1::SemanticConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 1,
            },
            ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                actual: local,
                expected: local,
                subjects: fe2o3_functional_proof::FunctionalRefinementSubjectsV2::new(
                    fe2o3_functional_proof::SafeReferenceKindV2::Mir,
                    d,
                    DigestV1::ZERO,
                    d,
                    d,
                    d,
                )
                .unwrap(),
            },
        ]
    } else {
        vec![]
    };
    let kernel = ProductionRankedKernelV1::new(
        "borrowed_policy",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    ProductionConstructionV1::ranked_kernel("borrowed_policy", kernel).unwrap()
}

#[test]
fn borrowed_policy_empty_obligation_parity_retains_same_external_owner() {
    let accepted = policy();
    let before = &accepted as *const _;
    let borrowed = stage_ranked_kernel_with_borrowed_policy_checked_refinement_v2(
        construction(false),
        vec![],
        &accepted,
    )
    .unwrap();
    let owned = stage_ranked_kernel_with_policy_checked_refinement_v2(
        construction(false),
        vec![],
        policy(),
    )
    .unwrap();
    for staged in [borrowed, owned] {
        let ProductionConstructionKindV1::RankedKernel {
            kernel,
            policy_checked_refinement_staging,
            ..
        } = staged.kind
        else {
            panic!("kind changed");
        };
        assert!(policy_checked_refinement_staging.is_empty());
        assert!(kernel.blocks()[0].operations().is_empty());
    }
    assert_eq!(&accepted as *const _, before);
    assert!(accepted.accepts_signer(DigestV1::from_untrusted_bytes([7; 32])));
}

#[test]
fn borrowed_policy_wrong_kind_and_unbound_request_preserve_old_refusals() {
    for unbound in [false, true] {
        let make = || {
            if unbound {
                construction(true)
            } else {
                ProductionConstructionV1::builtin_module("not_ranked").unwrap()
            }
        };
        let accepted = policy();
        let borrowed = stage_ranked_kernel_with_borrowed_policy_checked_refinement_v2(
            make(),
            vec![],
            &accepted,
        )
        .err()
        .unwrap();
        let owned = stage_ranked_kernel_with_policy_checked_refinement_v2(make(), vec![], policy())
            .err()
            .unwrap();
        for error in [borrowed, owned] {
            assert!(matches!(
                (unbound, error),
                (
                    true,
                    ProductionFunctionalRefinementAdmissionErrorV2::UnboundRequest
                ) | (
                    false,
                    ProductionFunctionalRefinementAdmissionErrorV2::WrongConstructionKind
                )
            ));
        }
        assert!(accepted.accepts_signer(DigestV1::from_untrusted_bytes([7; 32])));
    }
}
