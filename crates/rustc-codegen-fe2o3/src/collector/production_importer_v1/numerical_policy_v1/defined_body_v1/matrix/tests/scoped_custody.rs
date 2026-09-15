//! Real source/SSA custody and consumed ranked terminal checks, not memory,
//! functional, KIR or device-execution qualification.
use super::*;

#[path = "scoped_query_map.rs"]
mod query_map;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextLoweringInputV1, ProductionScopedMatrixUseRelationV1,
};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

pub(super) fn source() -> String {
    let mut source = include_str!("terminal_source.rs").to_owned();
    for name in ["rows", "columns", "stride"] {
        let parameter = format!("    {name}: usize,");
        assert_eq!(source.matches(&parameter).count(), 1);
        source = source.replacen(&parameter, &format!("    {name}: u32,"), 1);
    }
    let opening = ") -> KernelResult {\n";
    assert_eq!(source.matches(opening).count(), 1);
    source.replacen(
        opening,
        ") -> KernelResult {\n    let rows = rows as usize;\n    let columns = columns as usize;\n    let stride = stride as usize;\n",
        1,
    )
}

fn owner(mir: &AdmittedInertSemanticMirV1) -> ProductionSemanticSsaOwnerV1 {
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn check(
    imported: &crate::collector::production_importer_v1::ConstructedProductionSemanticMirV1,
    typed: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
        typed,
        &imported.semantic_mir,
    )
    .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed,
        &imported.semantic_mir,
    )
    .unwrap();
    assert_eq!(typed.len(), 1);
    let owner = owner(&imported.semantic_mir);
    let root = imported.semantic_mir.roots()[0];
    query_map::inspect_recorded(
        &owner,
        owner.execution_view_for_root(root).unwrap(),
        &[
            ("2fdcaf57fcf814cbc3127b00dfd729a83fce7758e481f168063b32ecbb6f68ee", 30, 103),
            ("23f493c7aac1a3a4289a5fe8f48b78ea5ab42c4500eba207f527f63f5138e8d3", 30, 103),
        ],
        &mut std::io::stderr().lock(),
    ).unwrap();
    let launch = typed[0].source_launch().unwrap();
    let contexts = &imported.kernel_contexts;
    let entry = contexts
        .checked_ranked_entry(&owner, root, launch, 1_048_576)
        .unwrap();
    let relation = contexts
        .checked_ranked_matrix(
            &owner,
            root,
            launch,
            entry.as_ref().map(|(e, _)| e),
            1_048_576,
        )
        .unwrap()
        .expect("real scoped constructor roster");
    let view = owner.execution_view_for_root(root).unwrap();
    let body = view.body();
    let mut consumed = Vec::new();
    let mut matrix_roots = BTreeSet::new();
    let mut lane_roots = BTreeSet::new();
    let mut counts = [0usize; 3];
    for (block, data) in body.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
            continue;
        };
        let Some(row) = relation.use_at(body, block as u32, call).unwrap() else {
            continue;
        };
        assert_eq!(
            relation.use_at(body, block as u32, call).unwrap(),
            Some(row),
            "fixed-point queries are read-only and do not consume issuance"
        );
        lane_roots.insert(row.lane_identity());
        match imported.semantic_mir.callables()[call.callee().index() as usize] {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { .. },
                ..
            } => {
                matrix_roots.insert(row.matrix_identity().expect("policy-bound consumer"));
                counts[0] += 1;
            }
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. },
                ..
            } => {
                assert!(row.matrix_identity().is_none());
                counts[1] += 1;
            }
            _ => {
                assert_eq!(row.operand(), 1);
                assert!(row.matrix_identity().is_none());
                counts[2] += 1;
            }
        }
        let mut args = call.arguments().to_vec();
        args.remove(row.operand());
        let missing = SemanticDirectCallV1::new_callable(
            call.callee(),
            args,
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        assert!(relation.use_at(body, block as u32, &missing).is_err());
        let replaced = SemanticDirectCallV1::new_callable(
            call.callee(),
            call.arguments().iter().rev().cloned().collect(),
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        if replaced != *call {
            assert!(relation.use_at(body, block as u32, &replaced).is_err());
        }
        assert!(relation.use_at(&body.clone(), block as u32, call).is_err());
        consumed.push(block as u32);
    }
    assert_eq!(
        counts,
        [3, 3, 6],
        "real homogeneous and mixed typed matrix consumer roster"
    );
    assert_eq!(
        matrix_roots.len(),
        1,
        "one actual narrowed owner, three legal shared uses"
    );
    assert_eq!(
        lane_roots.len(),
        1,
        "every typed producer names the same subgroup/epoch"
    );
    relation.check_consumed(body, &consumed).unwrap();
    assert!(relation.check_consumed(body, &consumed[1..]).is_err());
    let mut duplicated = consumed.clone();
    duplicated.insert(0, consumed[0]);
    assert!(relation.check_consumed(body, &duplicated).is_err());
    let mut reversed = consumed.clone();
    reversed.reverse();
    assert!(relation.check_consumed(body, &reversed).is_err());
    let foreign_owner = owner_from_same_source(&imported.semantic_mir);
    assert!(
        relation
            .check_consumed(
                foreign_owner.execution_view_for_root(root).unwrap().body(),
                &consumed
            )
            .is_err()
    );

    // Mutate the inert frontend axes while retaining the real source graph.
    let source = contexts
        .roots
        .iter()
        .find(|r| r.selected_root == root)
        .unwrap();
    let exact = [
        contexts.frontend_unit_identity,
        source.kernel_marker_identity,
        contexts.target_brand_identity,
        source.launch_brand_identity,
        source.issuance_identity,
    ];
    for axis in 0..exact.len() {
        for zero in [false, true] {
            let mut identities = exact;
            identities[axis] = if zero {
                [0; 32]
            } else {
                let mut v = identities[axis];
                v[31] ^= 1;
                v
            };
            let mut input = ProductionKernelContextLoweringInputV1::new(
                root,
                identities[0],
                identities[1],
                identities[2],
                identities[3],
                identities[4],
            );
            if let Some(transfer) = source.entry_transfer {
                input = input.with_entry_transfer(transfer);
            }
            assert!(
                ProductionScopedMatrixUseRelationV1::checked_source_uses(
                    &owner,
                    &input,
                    entry.as_ref().map(|(e, _)| e),
                    1_048_576
                )
                .is_err(),
                "frontend axis {axis}"
            );
        }
    }
    assert!(
        contexts
            .checked_ranked_matrix(&owner, root, launch, entry.as_ref().map(|(e, _)| e), 1)
            .is_err(),
        "no per-consumer graph or budget reset"
    );
    crate::production_ranked_projection_v1::scoped_matrix_consumer_regression_v1(
        &owner, root, &relation,
    );
    super::scoped_owner_negatives::check(imported);
}

fn owner_from_same_source(mir: &AdmittedInertSemanticMirV1) -> ProductionSemanticSsaOwnerV1 {
    owner(mir)
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_matrix_source_and_ranked_gfx942() {
    run(
        "gfx942",
        "scoped_custody::scoped_matrix_source_and_ranked_gfx942",
        Fixture::ScopedCustody,
    );
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; invokes rustc only, never Cargo"]
fn scoped_matrix_source_and_ranked_gfx950() {
    run(
        "gfx950",
        "scoped_custody::scoped_matrix_source_and_ranked_gfx950",
        Fixture::ScopedCustody,
    );
}
