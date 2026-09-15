//! Exact guarded source-to-store correspondence, not a GridLeaderCurrent alias.
use super::*;
use fe2o3_pliron::ProductionGuardedGridSourceV1;

#[allow(clippy::too_many_arguments)]
pub(super) fn project<'a>(
    source: &ProductionGuardedGridSourceV1<'_>,
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    linear_launch_upper_bound: Option<u64>,
    dominance: &SemanticEnumPayloadDominanceV1,
    proofs: &mut Option<SemanticAssertProofsV1<'a>>,
    definitions: &[u8],
    escaped: &[bool],
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<ProjectedDisjointIndexV1, ProductionRankedProjectionErrorV1> {
    let mismatch = || {
        ProductionRankedProjectionErrorV1::Incomplete(
            "a GridExclusive store lacks exact guarded source and SSA correspondence",
        )
    };
    if !std::ptr::eq(source.function(), function)
        || !std::ptr::eq(source.types(), types)
        || !linear_launch_upper_bound.is_some_and(|bound| bound > 0)
    {
        return Err(mismatch());
    }
    if proofs.is_none() {
        *proofs = Some(SemanticAssertProofsV1::new(types, function)?);
    }
    let proof = proofs
        .as_mut()
        .expect("initialized existing index proof owner");
    let mut failure = None;
    let index = source.index_for_store(block, &mut |amount| match proof.charge(amount) {
        Ok(()) => true,
        Err(error) => {
            failure = Some(error);
            false
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    let index = index.map_err(|_| mismatch())?.ok_or_else(mismatch)?;
    if !index.belongs_to(source) || index.receipt().contract().provenance() != provenance {
        return Err(mismatch());
    }
    let availability = dominance
        .availability(index.receipt().option(), 1)
        .filter(|availability| dominance.allows(*availability, block))
        .ok_or_else(mismatch)?;
    // The exact constructor parameter has one definition in this expanded
    // frame. Do not collapse a versioned/reassigned scalar into a local slot.
    let raw = simple_operand_local(index.raw_operand()).ok_or_else(mismatch)?;
    let raw_index = raw.index() as usize;
    if definitions.get(raw_index).copied() != Some(1)
        || escaped.get(raw_index).copied() != Some(false)
    {
        return Err(mismatch());
    }
    proof.charge(8)?;
    let value = project_runtime_index_operand_v1(
        Some(index.raw_operand()),
        constants,
        stable_argument_origins,
        arguments,
        next_argument,
        operations,
        next_value,
    )?;

    // This predicate follows only from the checked original rank==0 issuer
    // relation above. It does not register or substitute a source intrinsic.
    reserve_operation(operations)?;
    let invocation = next_value_id(next_value)?;
    operations.push(ProductionRankedOperationV1::InvocationIndex {
        result: invocation,
        dimension: 0,
        launch_extent: 0,
    });
    reserve_operation(operations)?;
    let one = next_value_id(next_value)?;
    operations.push(ProductionRankedOperationV1::IndexConstant {
        result: one,
        value: 1,
    });
    Ok(ProjectedDisjointIndexV1 {
        value,
        mapping: SemanticDisjointIndexSpaceV1::GridExclusive,
        precondition: Some((
            ProductionRankedValueV1::Local(invocation),
            ProductionRankedValueV1::Local(one),
        )),
        availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
    })
}

#[cfg(test)]
#[path = "guarded_grid_store_v1/geometry_tests.rs"]
mod geometry_tests;

#[cfg(test)]
pub(crate) fn assert_actual_guarded_grid_store_projection(
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    source_launch: &LaunchContract,
) {
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(root).unwrap();
    let function = view.body();
    let types = owner.source_semantic().types();
    let source = owner.guarded_grid_source_for_root(root, function).unwrap();
    let inventory = assertion_definition_inventory(function).unwrap();
    let provenance = local_provenance_with_scalar_inventory_v1(
        types,
        function,
        &inventory.counts,
        &inventory.address_escaped,
    )
    .unwrap();
    let constants = constant_locals(function).unwrap();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    let mut proofs = None;
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 0;
    let mut operations = Vec::new();
    let mut next_value = 0;
    let mut stores = 0;
    for (i, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore {
                    contract,
                    provenance: expected,
                    ..
                },
            ..
        }) = owner
            .source_semantic()
            .callables()
            .get(call.callee().index() as usize)
        else {
            continue;
        };
        if contract.aliasing()
            != SemanticCapabilityMemoryAliasingV1::Disjoint(
                SemanticDisjointIndexSpaceV1::GridExclusive,
            )
        {
            continue;
        }
        // A live source owner must not turn a multidimensional or unknown
        // launch into the one-dimensional singleton predicate.
        for rejected in geometry_tests::rejected_launches() {
            let mut rejected_proofs = None;
            let before = (arguments.clone(), next_argument, operations.len(), next_value);
            let rejected_result = project(
                &source, types, function, SemanticBlockIdV1::from_index(i as u32), *expected,
                bounded_linear_launch_extent_v1(&rejected), &dominance, &mut rejected_proofs,
                &inventory.counts, &inventory.address_escaped, &constants,
                &provenance.stable_argument_origins, &mut arguments, &mut next_argument,
                &mut operations, &mut next_value,
            );
            assert!(matches!(rejected_result, Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a GridExclusive store lacks exact guarded source and SSA correspondence"
            ))), "nonlinear launch must reject at its geometry gate");
            assert!(rejected_proofs.is_none());
            assert_eq!((arguments.clone(), next_argument, operations.len(), next_value), before);
        }
        let result = project(
            &source,
            types,
            function,
            SemanticBlockIdV1::from_index(i as u32),
            *expected,
            bounded_linear_launch_extent_v1(source_launch),
            &dominance,
            &mut proofs,
            &inventory.counts,
            &inventory.address_escaped,
            &constants,
            &provenance.stable_argument_origins,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .expect("original guarded source must reach the actual ranked store consumer");
        assert_eq!(result.mapping, SemanticDisjointIndexSpaceV1::GridExclusive);
        assert!(matches!(
            result.availability,
            Some(CapabilityAvailabilityV1::EnumPayload(_))
        ));
        let Some((ProductionRankedValueV1::Local(invocation), ProductionRankedValueV1::Local(one))) =
            result.precondition
        else {
            panic!("exact singleton precondition retained");
        };
        assert!(operations.iter().any(|op| matches!(op,ProductionRankedOperationV1::InvocationIndex {result,dimension:0,..} if *result==invocation)));
        assert!(operations.iter().any(|op| matches!(op,ProductionRankedOperationV1::IndexConstant {result,value:1} if *result==one)));
        stores += 1;
    }
    assert_eq!(
        stores, 1,
        "the original fixture has exactly one GridExclusive store"
    );
}
