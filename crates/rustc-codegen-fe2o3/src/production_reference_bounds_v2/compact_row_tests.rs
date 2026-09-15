use super::*;
use fe2o3_pliron::{
    ProductionRankedBlockV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
};

fn kernel(rank: usize) -> ProductionRankedKernelV1 {
    let ops = (0..2)
        .map(|id| ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(id),
            element_width: 32,
            writable: id == 1,
            shape: vec![256; rank],
            dynamic_extents: vec![],
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: id as u64 + 1,
            noalias_class: id as u64 + 1,
        })
        .collect();
    ProductionRankedKernelV1::new(
        "compact_row_bounds",
        0,
        vec![ProductionRankedBlockV1::new(
            ops,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

#[test]
fn compact_row_bounds_exact_accesses_and_no_extent_to_point_bootstrap() {
    let ir = crate::reference_effect_v1::compact_row_source_tests::fixture();
    let writes = crate::reference_effect_v1::compact_row_source_tests::writes(&ir);
    let outputs = [CompilerOwnedOutputDomainV2 {
        reference: &writes[0],
        ranked_view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
    }];
    let kernel = kernel(1);
    let definitions = definitions(&kernel).unwrap();
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    assert!(
        point_domains(&kernel, &outputs, &definitions, &mut work)
            .unwrap()
            .is_empty()
    );
    discharge_reference_bounds_with_budget_v2(&kernel, &ir, &outputs, &mut work).unwrap();
    let mut changed = ir.clone();
    changed.blocks[2].terminator =
        crate::reference_effect_v1::ReferenceTerminatorV1::Goto { target: 3 };
    assert!(
        discharge_reference_bounds_with_budget_v2(&kernel, &changed, &outputs, &mut work)
            .unwrap_err()
            .detail()
            .contains("preceding CPU predicate")
    );
}

#[test]
fn compact_row_bounds_wrong_rank_and_invocation_carrier_reject() {
    let mut ir = crate::reference_effect_v1::compact_row_source_tests::fixture();
    let writes = crate::reference_effect_v1::compact_row_source_tests::writes(&ir);
    let outputs = [CompilerOwnedOutputDomainV2 {
        reference: &writes[0],
        ranked_view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
    }];
    let error = discharge_reference_bounds_with_budget_v2(
        &kernel(2),
        &ir,
        &outputs,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap_err();
    assert_eq!(error.detail(), "compact row output requires a rank-1 view");
    ir.relations[2] = ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
        argument: 1,
        element: ReferenceScalarTypeV1::U32,
    };
    assert_eq!(
        discharge_reference_bounds_with_budget_v2(
            &kernel(1),
            &ir,
            &outputs,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap_err()
        .detail(),
        "compact row bounds require an exclusive primitive output relation"
    );
}
