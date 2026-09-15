//! Tests the production capability pass on an actual source-imported SSA owner.
//! No state entries, Context relations, allocation IDs, or source proof are invented here.
use super::*;

fn project(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    entry: &context_entry_v1::Entry<'_>,
) -> Result<ProjectedCapabilityEffectsV1, ProductionRankedProjectionErrorV1> {
    owner.verify_replay().unwrap();
    let semantic = owner.source_semantic();
    let view = owner.execution_view_for_root(root).unwrap();
    assert!(std::ptr::eq(
        owner.execution_expansion().root(root).unwrap(),
        view
    ));
    assert_eq!(
        owner
            .execution_plan_for_root(root)
            .unwrap()
            .function_identity(),
        view.body().identity()
    );
    let function = view.body();
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, semantic.types()).unwrap();
    let inventory = assertion_definition_inventory(function)?;
    let provenance = local_provenance_with_scalar_inventory_v1(
        semantic.types(),
        function,
        &inventory.counts,
        &inventory.address_escaped,
    )?;
    let allocations = local_allocation_contracts_with_source_v1(
        semantic.types(),
        function,
        &provenance.allocation_origins,
        None,
    )?;
    let private = private_scalar_capture_v1::PrivateScalarReads::for_root(owner, root);
    project_authenticated_capabilities_with_transport_v1(
        semantic.types(),
        semantic.callables(),
        function,
        &dominance,
        &allocations,
        &constant_locals(function)?,
        Some(entry),
        private.as_ref(),
    )
}

pub(crate) fn check(
    owner: &ProductionSemanticSsaOwnerV1,
    foreign_owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    foreign_root: SemanticFunctionIdV1,
    entry: &context_entry_v1::Entry<'_>,
) {
    let effects =
        project(owner, root, entry).expect("actual multi-Global captures reach every bound read");
    let function = owner.execution_view_for_root(root).unwrap().body();
    let callables = owner.source_semantic().callables();
    let mut allocations = BTreeMap::<u64, usize>::new();
    let mut read_count = 0;
    for (block, view) in effects.global_views.iter().enumerate() {
        let Some(view) = view else {
            continue;
        };
        let SemanticTerminatorKindV1::Call(call) = function.blocks()[block].terminator().kind()
        else {
            panic!("read view must be bound to a call")
        };
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
                    provenance,
                    source_identity,
                    contract,
                    ..
                },
            ..
        } = &callables[call.callee().index() as usize]
        else {
            continue;
        };
        assert_eq!(binding.identity(), *source_identity);
        assert_eq!(provenance.root(), root);
        assert_eq!(view.provenance, *provenance);
        assert_eq!(view.contract, *contract);
        assert_eq!(
            view.contract,
            SemanticCapabilityMemoryContractV1::global_read_only()
        );
        assert_eq!(view.borrow, Some(SemanticBorrowKindV1::Shared));
        assert!(!view.allocation.writable);
        assert_ne!(view.allocation.allocation_origin, 0);
        *allocations
            .entry(view.allocation.allocation_origin)
            .or_default() += 1;
        read_count += 1;
    }
    assert_eq!(
        read_count, 4,
        "all four actual source loads retain their own receiver"
    );
    assert_eq!(
        allocations.len(),
        2,
        "same element type cannot merge distinct source parameters"
    );
    assert!(allocations.values().all(|count| *count == 2));

    // Identical canonical bytes in another owner are not the current retained body.
    assert_eq!(
        owner.source_semantic().canonical_encoding(),
        foreign_owner.source_semantic().canonical_encoding()
    );
    assert!(matches!(
        project(foreign_owner, root, entry),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "ranked Context entry SSA owner changed"
        ))
    ));
    assert!(matches!(
        project(owner, foreign_root, entry),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "ranked Context entry SSA owner changed"
        ))
    ));
}
