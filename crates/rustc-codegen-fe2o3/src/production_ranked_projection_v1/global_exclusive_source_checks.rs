//! Actual imported closure custody through the existing capability pass. These
//! checks stop before index, race, final reference, and functional refinement.
use super::*;

fn project(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    entry: &context_entry_v1::Entry<'_>,
) -> Result<ProjectedCapabilityEffectsV1, ProductionRankedProjectionErrorV1> {
    // UniqueSliceSource requires an already replay-checked owner.
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
    let binding = semantic.functions()[root.index() as usize]
        .kernel_entry()
        .unwrap()
        .kernel_binding_identity();
    let unique = unique_slice_source_v1::UniqueSliceSourceV1::for_root(owner, root, binding)
        .expect("original physical root supplies the independently authenticated primitive slice source");
    let allocations = local_allocation_contracts_with_source_v1(
        semantic.types(),
        function,
        &provenance.allocation_origins,
        Some(&unique),
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
    let effects = project(owner, root, entry)
        .expect("original mixed capture must retain both input and exclusive output receiver");
    let semantic = owner.source_semantic();
    let function = owner.execution_view_for_root(root).unwrap().body();
    let mut input = None;
    let mut output = None;
    let mut operations = 0;
    for (block, basic_block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = basic_block.terminator().kind() else {
            continue;
        };
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = &semantic.callables()[call.callee().index() as usize]
        else {
            continue;
        };
        let (element, contract, provenance, source, mutable) = match *operation {
            SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
                element,
                contract,
                provenance,
                source_identity,
                ..
            } => (element, contract, provenance, source_identity, false),
            SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveStore {
                element,
                contract,
                provenance,
                source_identity,
                ..
            } => (element, contract, provenance, source_identity, true),
            _ => continue,
        };
        let view = effects.global_views[block]
            .expect("every original load/store, not only observed successful effects, has its exact receiver");
        assert_eq!(binding.identity(), source);
        assert_eq!(provenance.root(), root);
        assert_eq!(view.provenance, provenance);
        assert_eq!(view.contract, contract);
        assert_eq!(view.element, element);
        assert_eq!(
            view.contract,
            if mutable {
                SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
            } else {
                SemanticCapabilityMemoryContractV1::global_read_only()
            }
        );
        assert_eq!(
            view.borrow,
            Some(if mutable {
                SemanticBorrowKindV1::Mutable
            } else {
                SemanticBorrowKindV1::Shared
            })
        );
        assert_eq!(view.allocation.writable, mutable);
        assert_ne!(view.allocation.allocation_origin, 0);
        assert_ne!(view.allocation.noalias_class, 0);
        let selected = if mutable { &mut output } else { &mut input };
        assert!(
            selected.replace(view).is_none(),
            "one original access per carrier"
        );
        operations += 1;
    }
    assert_eq!(operations, 2, "the original load and store remain present");
    let input = input.unwrap();
    let output = output.unwrap();
    assert_eq!(input.provenance, output.provenance);
    assert_ne!(
        input.allocation.allocation_origin,
        output.allocation.allocation_origin
    );
    assert_ne!(
        input.allocation.noalias_class,
        output.allocation.noalias_class
    );

    let mixed_source = function
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .any(|statement| {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return false;
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return false;
            };
            aggregate.operands().iter().any(|operand| {
                is_exact_reference_to_v1(
                    semantic.types(),
                    operand.ty(),
                    input.view,
                    SemanticMutabilityV1::Immutable,
                )
            }) && aggregate.operands().iter().any(|operand| {
                is_exact_reference_to_v1(
                    semantic.types(),
                    operand.ty(),
                    output.view,
                    SemanticMutabilityV1::Mutable,
                )
            })
        });
    assert!(
        mixed_source,
        "fixture must retain the actual shared-plus-mutable source aggregate"
    );

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
