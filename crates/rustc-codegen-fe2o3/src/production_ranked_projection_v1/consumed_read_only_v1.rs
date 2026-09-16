// The nominal local view is not authority. The whole-function audit below
// authenticates its exclusive allocation before capability dataflow can use it.
include!("consumed_read_only_audit_v1.rs");
include!("consumed_read_only_custody_v1.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConsumedReadOnlyRootV1 {
    argument: u32,
    slice: SemanticTypeIdV1,
    view: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    conversion_block: usize,
}

fn consumed_read_only_origin_v1(
    call: &SemanticDirectCallV1,
    function: &SemanticFunctionDeclV1,
    root: u64,
    view_type: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    allocations: &[Option<AllocationContractV1>],
) -> Option<ProjectedReadViewV1> {
    let [operand @ (SemanticOperandV1::Move(source) | SemanticOperandV1::Copy(source))] =
        call.arguments()
    else {
        return None;
    };
    let destination = call.destination()?.place();
    if !source.projections().is_empty()
        || !destination.projections().is_empty()
        || destination.ty() != view_type
    {
        return None;
    }
    let allocation = allocations
        .get(source.local().index() as usize)
        .copied()
        .flatten()?;
    if !allocation.writable
        || allocation.singleton_object
        || allocation.noalias_class != allocation.allocation_origin.checked_add(1)?
    {
        return None;
    }
    let argument = u32::try_from(allocation.allocation_origin.checked_sub(1)?).ok()?;
    // Post-borrowck optimized MIR spells the genuine by-value constructor as
    // Copy of its original argument. This exception never admits copied owners.
    if matches!(operand, SemanticOperandV1::Copy(_))
        && (function
            .locals()
            .get(source.local().index() as usize)?
            .role()
            != SemanticLocalRoleV1::Argument(argument)
            || function.abi().source_input_types().get(argument as usize) != Some(&source.ty())
            || function
                .abi()
                .source_argument_ownership()
                .get(argument as usize)
                != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner))
    {
        return None;
    }
    Some(ProjectedReadViewV1 {
        root,
        element,
        allocation: AllocationContractV1 {
            writable: false,
            ..allocation
        },
        rows: ProjectedReadValueV1::Constant(1),
        columns: ProjectedReadValueV1::AllocationExtent(argument),
    })
}

fn transfer_consumed_read_only_use_v1(
    call: &SemanticDirectCallV1,
    operation: Option<&SemanticCompilerIntrinsicOperationV1>,
    state: &mut ProjectedCapabilityStateV1,
    constants: &[Option<u64>],
    require_authenticated_site: bool,
) -> Result<ProjectedCapabilityTerminatorEffectsV1, ProductionRankedProjectionErrorV1> {
    let origin = call.arguments().first().and_then(|operand| {
        match capability_known_origin_v1(state, operand)? {
            ProjectedCapabilityOriginV1::ConsumedReadOnly(view) => Some(view),
            _ => None,
        }
    });
    let read_view = match (operation, origin) {
        (
            Some(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
                element, ..
            }),
            Some(view),
        ) if call.arguments().len() == 3 && view.element == *element => {
            projected_read_value_v1(&call.arguments()[1], constants).map(|column| {
                ProjectedReadViewAccessV1 {
                    view,
                    row: ProjectedReadValueV1::Constant(0),
                    column,
                }
            })
        }
        _ => None,
    };
    let authenticated = match operation {
        Some(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { .. }) => {
            call.arguments().len() == 1 && origin.is_some()
        }
        Some(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr { .. }) => {
            read_view.is_some()
        }
        _ => false,
    };
    consume_capability_operands_v1(state, call.arguments());
    if let Some(destination) = call.destination() {
        invalidate_capability_place_v1(state, destination.place());
        if destination.place().projections().is_empty() {
            state.remove(&(destination.place().local().index() as usize));
        }
    }
    if !authenticated && require_authenticated_site {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a consumed readonly use lacks a live exact allocation capability",
        ));
    }
    Ok(ProjectedCapabilityTerminatorEffectsV1 {
        read_view,
        ..ProjectedCapabilityTerminatorEffectsV1::default()
    })
}

fn project_consumed_read_only_extent_v1(
    argument: u32,
    extent_arguments: &mut [Option<u32>],
    next_argument: &mut usize,
) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
    let slot = extent_arguments.get_mut(argument as usize).ok_or(
        ProductionRankedProjectionErrorV1::Incomplete(
            "a consumed readonly extent lacks its exact source argument",
        ),
    )?;
    let argument = match *slot {
        Some(argument) => argument,
        None => {
            if *next_argument >= HARD_MAX_PRODUCTION_RANKED_ARGUMENTS {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "consumed readonly extents exceed the ranked argument limit",
                ));
            }
            let argument = u32::try_from(*next_argument).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported("readonly extent index overflow")
            })?;
            *next_argument += 1;
            *slot = Some(argument);
            argument
        }
    };
    Ok(ProductionRankedValueV1::Argument(argument))
}

fn consumed_read_only_shared_reference_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable
                && pointer.metadata() == SemanticPointerMetadataV1::None
                && pointer.pointer_width_bits() == 64
                && pointer.address_space() == 0
                && pointer.pointee() == pointee)
}

fn consumed_read_only_alias_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    root: ConsumedReadOnlyRootV1,
) -> bool {
    ty == root.slice
        || ty == root.view
        || consumed_read_only_shared_reference_v1(types, ty, root.slice)
        || consumed_read_only_shared_reference_v1(types, ty, root.view)
}
