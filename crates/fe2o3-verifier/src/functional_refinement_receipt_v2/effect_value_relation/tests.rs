use super::*;
use dialect_kernel::{AccessKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_functional_proof::SafeReferenceKindV2;
use fe2o3_pliron::*;

fn local(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id))
}

fn subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes([1; 32]),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes([2; 32]),
        DigestV1::from_untrusted_bytes([3; 32]),
        DigestV1::from_untrusted_bytes([4; 32]),
    )
    .unwrap()
}

fn effect() -> ProductionEffectRefinementContractV2 {
    ProductionEffectRefinementContractV2::new(
        73,
        ProductionGpuWriteSiteV2::new(0, 8),
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        local(0),
        vec![local(1)],
        vec![local(5)],
        vec![local(5)],
        local(4),
        local(4),
        local(4),
        local(4),
        local(2),
        local(3),
    )
    .unwrap()
}

fn numerical(
    actual: u32,
    reference: u32,
    domain: u32,
    pre: u32,
) -> ProductionNumericalRefinementContractV2 {
    ProductionNumericalRefinementContractV2::new(
        74,
        local(actual),
        local(reference),
        local(domain),
        local(pre),
        0.25_f64.to_bits(),
        0,
    )
    .unwrap()
}

fn kernel(
    claim: ProductionNumericalRefinementContractV2,
    with_numerical: bool,
) -> ProductionRankedKernelV1 {
    let expression = |id, scalar, bits| ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(id),
        expression: ProductionSemanticExpressionV2::Constant { scalar, bits },
        numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
    };
    let mut ops = vec![
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [1; 3],
            workgroup_extents: [1; 3],
            subgroup_size: 1,
            full_physical_workgroups: true,
        },
        ProductionRankedOperationV1::View {
            result: ProductionRankedValueIdV1::new(0),
            element_width: 32,
            writable: true,
            shape: vec![1],
            dynamic_extents: vec![],
            allocation_origin: 1,
            noalias_class: 1,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(1),
            value: 0,
        },
        expression(
            2,
            ProductionSemanticScalarTypeV2::Float { bits: 32 },
            1.125_f32.to_bits() as u64,
        ),
        expression(
            3,
            ProductionSemanticScalarTypeV2::Float { bits: 32 },
            1_f32.to_bits() as u64,
        ),
        expression(4, ProductionSemanticScalarTypeV2::Bool, 1),
        expression(
            5,
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            },
            0,
        ),
        ProductionRankedOperationV1::OwnershipContract {
            view: local(0),
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        },
        ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view: local(0),
            indices: vec![local(1)],
            value: local(2),
        },
        ProductionRankedOperationV1::RequestEffectRefinement {
            contract: effect(),
            subjects: subjects(),
        },
    ];
    if with_numerical {
        ops.push(ProductionRankedOperationV1::RequestNumericalRefinement {
            contract: claim,
            subjects: subjects(),
        });
    }
    ProductionRankedKernelV1::new(
        "effect_value_relation",
        0,
        vec![ProductionRankedBlockV1::new(
            ops,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn mode() -> EffectValueRelation {
    EffectValueRelation::NumericalRequest {
        block: 0,
        operation: 10,
    }
}
fn kind(
    result: Result<Vec<Pair>, FunctionalRefinementVerusExecutionErrorV2>,
) -> FunctionalRefinementVerusExecutionErrorKindV2 {
    result.unwrap_err().kind()
}
fn claim_required() -> FunctionalRefinementVerusExecutionErrorKindV2 {
    FunctionalRefinementVerusExecutionErrorKindV2::ClaimSpecificNumericalProofRequired
}
fn invalid() -> FunctionalRefinementVerusExecutionErrorKindV2 {
    FunctionalRefinementVerusExecutionErrorKindV2::InvalidRankedProofRecipe
}

fn replay(
    kernel: &ProductionRankedKernelV1,
    effect: &ProductionEffectRefinementContractV2,
    subjects: FunctionalRefinementSubjectsV2,
    relation: EffectValueRelation,
    lemma: &str,
) -> Result<String, FunctionalRefinementVerusExecutionErrorV2> {
    let (program, pairs) = replay_program(kernel, effect, subjects, relation)?;
    program.render_lemma(&pairs, lemma)
}

#[test]
fn exact_preserves_all_ordered_pairs_including_unequal_value_roots() {
    for with_numerical in [false, true] {
        let k = kernel(numerical(2, 3, 4, 4), with_numerical);
        assert_eq!(
            pairs(&k, Some(&effect()), subjects(), EffectValueRelation::Exact).unwrap(),
            vec![
                (local(5), local(5)),
                (local(4), local(4)),
                (local(4), local(4)),
                (local(2), local(3))
            ]
        );
    }
}

#[test]
fn exact_replay_still_renders_location_domain_precondition_and_value_equalities() {
    let k = kernel(numerical(2, 3, 4, 4), false);
    let replay = replay(
        &k,
        &effect(),
        subjects(),
        EffectValueRelation::Exact,
        "replay_probe",
    )
    .unwrap();
    assert!(replay.contains("assert(v5 == v5)"));
    assert_eq!(replay.matches("assert(v4 == v4)").count(), 2);
    assert!(replay.contains("assert(v2 == v3)"));
    assert!(replay.contains("fe2o3_ieee_operator_congruence_v2"));
}

#[test]
fn coherent_nonzero_numerical_link_cannot_generate_pairs_or_replay() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    assert_eq!(
        kind(pairs(&k, Some(&effect()), subjects(), mode())),
        claim_required()
    );
    let error = replay(&k, &effect(), subjects(), mode(), "no_numerical_replay")
        .err()
        .unwrap();
    assert_eq!(error.kind(), claim_required());
    assert!(!error.to_string().contains("violation"));
    assert!(matches!(
        &k.blocks()[0].operations()[9],
        ProductionRankedOperationV1::RequestEffectRefinement { .. }
    ));
    assert!(matches!(
        &k.blocks()[0].operations()[10],
        ProductionRankedOperationV1::RequestNumericalRefinement { .. }
    ));
}

#[test]
fn numerical_without_effect_is_still_an_unproved_request() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    assert_eq!(kind(pairs(&k, None, subjects(), mode())), claim_required());
    assert_eq!(
        kind(pairs(&k, None, subjects(), EffectValueRelation::Exact)),
        invalid()
    );
    assert_eq!(
        super::super::generate_ranked_functional_refinement_proof_v2(&k, 0, 10, subjects())
            .unwrap_err()
            .kind(),
        claim_required()
    );
}

#[test]
fn missing_unknown_and_effect_self_site_do_not_resolve() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    for m in [
        EffectValueRelation::NumericalRequest {
            block: usize::MAX,
            operation: 0,
        },
        EffectValueRelation::NumericalRequest {
            block: 0,
            operation: usize::MAX,
        },
        EffectValueRelation::NumericalRequest {
            block: 0,
            operation: 9,
        },
    ] {
        assert_eq!(kind(pairs(&k, Some(&effect()), subjects(), m)), invalid());
        assert_eq!(
            replay(&k, &effect(), subjects(), m, "missing")
                .err()
                .unwrap()
                .kind(),
            invalid()
        );
    }
}

#[test]
fn foreign_registered_subjects_cannot_resolve() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    let other = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes([9; 32]),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes([2; 32]),
        DigestV1::from_untrusted_bytes([3; 32]),
        DigestV1::from_untrusted_bytes([4; 32]),
    )
    .unwrap();
    assert_eq!(kind(pairs(&k, Some(&effect()), other, mode())), invalid());
    assert_eq!(
        replay(&k, &effect(), other, mode(), "foreign")
            .err()
            .unwrap()
            .kind(),
        invalid()
    );
}

#[test]
fn self_substituted_value_roots_cannot_resolve() {
    for n in [numerical(2, 2, 4, 4), numerical(3, 3, 4, 4)] {
        let k = kernel(n, true);
        assert_eq!(
            kind(pairs(&k, Some(&effect()), subjects(), mode())),
            invalid()
        );
        assert_eq!(
            replay(&k, &effect(), subjects(), mode(), "self_substitution")
                .err()
                .unwrap()
                .kind(),
            invalid()
        );
    }
}

fn changed_effect(
    write: ProductionGpuWriteSiteV2,
    view: ProductionRankedValueV1,
    index: ProductionRankedValueV1,
    gd: ProductionRankedValueV1,
    rd: ProductionRankedValueV1,
    gp: ProductionRankedValueV1,
    rp: ProductionRankedValueV1,
    coordinate: ProductionRankedValueV1,
) -> ProductionEffectRefinementContractV2 {
    ProductionEffectRefinementContractV2::new(
        73,
        write,
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        view,
        vec![index],
        vec![coordinate],
        vec![coordinate],
        gd,
        rd,
        gp,
        rp,
        local(2),
        local(3),
    )
    .unwrap()
}

#[test]
fn wrong_store_view_index_guard_or_precondition_cannot_replay() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    for which in 0..7 {
        let mut fields = [local(0), local(1), local(4), local(4), local(4), local(4)];
        let mut write = ProductionGpuWriteSiteV2::new(0, 8);
        if which == 6 {
            write = ProductionGpuWriteSiteV2::new(0, 9);
        } else {
            fields[which] = local(5);
        }
        let e = changed_effect(
            write,
            fields[0],
            fields[1],
            fields[2],
            fields[3],
            fields[4],
            fields[5],
            local(5),
        );
        assert_eq!(
            replay(&k, &e, subjects(), mode(), "wrong_store_or_domain")
                .err()
                .unwrap()
                .kind(),
            invalid()
        );
    }
}

#[test]
fn numerical_replay_rejects_before_building_even_a_bad_exact_formula() {
    let k = kernel(numerical(2, 3, 4, 4), true);
    let e = changed_effect(
        ProductionGpuWriteSiteV2::new(0, 8),
        local(0),
        local(1),
        local(4),
        local(4),
        local(4),
        local(4),
        ProductionRankedValueV1::Argument(0),
    );
    assert_eq!(
        replay(&k, &e, subjects(), EffectValueRelation::Exact, "bad_exact")
            .err()
            .unwrap()
            .kind(),
        invalid()
    );
    assert_eq!(
        replay(&k, &e, subjects(), mode(), "no_build")
            .err()
            .unwrap()
            .kind(),
        claim_required()
    );
}
