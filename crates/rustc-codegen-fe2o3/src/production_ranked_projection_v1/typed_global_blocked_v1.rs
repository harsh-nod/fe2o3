#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectedGlobalAccessShapeV1 {
    Read,
    Store,
    BlockedStore {
        witness: SemanticTypeIdV1,
        component: SemanticTypeIdV1,
        lanes_per_block: u64,
        elements_per_lane: u64,
    },
}

impl ProjectedGlobalAccessShapeV1 {
    const fn arity(self) -> usize {
        match self {
            Self::Read => 2,
            Self::Store => 3,
            Self::BlockedStore { .. } => 4,
        }
    }

    const fn value_ordinal(self) -> Option<usize> {
        match self {
            Self::Read => None,
            Self::Store => Some(2),
            Self::BlockedStore { .. } => Some(3),
        }
    }

    fn value_operand(self, call: &SemanticDirectCallV1) -> Option<&SemanticOperandV1> {
        call.arguments().get(self.value_ordinal()?)
    }

    fn arguments_match(self, call: &SemanticDirectCallV1, element: SemanticTypeIdV1) -> bool {
        call.arguments().len() == self.arity()
            && self.value_ordinal().is_none_or(|ordinal| {
                call.arguments().get(ordinal).map(SemanticOperandV1::ty) == Some(element)
            })
            && match self {
                Self::BlockedStore { component, .. } => {
                    call.arguments().get(2).map(SemanticOperandV1::ty) == Some(component)
                }
                Self::Read | Self::Store => true,
            }
    }

    fn witness_borrow_matches(
        self,
        types: &[SemanticTypeDeclV1],
        call: &SemanticDirectCallV1,
    ) -> bool {
        match self {
            Self::BlockedStore { witness, .. } => call.arguments().get(1).is_some_and(|operand| {
                is_exact_shared_reference_to_v1(types, operand.ty(), witness)
            }),
            Self::Read | Self::Store => true,
        }
    }

    fn source_inputs_match(
        self,
        call: &SemanticDirectCallV1,
        inputs: &[SemanticTypeIdV1],
        ownership: &[SemanticSourceArgumentOwnershipV1],
    ) -> bool {
        match self {
            Self::BlockedStore { .. } => {
                use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow, UniqueBorrow};
                ownership == [UniqueBorrow, SharedBorrow, ByValue, ByValue]
                    && inputs.len() == call.arguments().len()
                    && inputs
                        .iter()
                        .zip(call.arguments())
                        .all(|(ty, operand)| *ty == operand.ty())
            }
            Self::Read | Self::Store => true,
        }
    }
}

fn supported_global_disjoint_binding_v1(contract: SemanticCapabilityMemoryContractV1) -> bool {
    let (Some(space), SemanticCapabilityMemoryAliasingV1::Disjoint(mapping)) =
        (contract.index_space_type(), contract.aliasing())
    else {
        return false;
    };
    let supported = match mapping {
        SemanticDisjointIndexSpaceV1::Index1d => true,
        SemanticDisjointIndexSpaceV1::BlockedIndex1d {
            lanes_per_block,
            elements_per_lane,
        } => {
            lanes_per_block != 0
                && elements_per_lane != 0
                && lanes_per_block.checked_mul(elements_per_lane).is_some()
        }
        _ => false,
    };
    supported
        && contract == SemanticCapabilityMemoryContractV1::global_disjoint_write(space, mapping)
}

#[allow(clippy::too_many_arguments)]
fn project_global_blocked_literal_index_v1(
    types: &[SemanticTypeDeclV1],
    call: &SemanticDirectCallV1,
    contract: ProjectedGlobalAccessContractV1,
    projected: Option<ProjectedDisjointIndexV1>,
    launch_upper_bound: Option<u64>,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
    let ProjectedGlobalAccessShapeV1::BlockedStore {
        lanes_per_block,
        elements_per_lane,
        ..
    } = contract.shape
    else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "typed blocked store lacks its closed access shape",
        ));
    };
    let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
        lanes_per_block,
        elements_per_lane,
    };
    let projected = projected
        .filter(|index| index.mapping == expected && index.availability.is_some())
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "typed blocked store lacks its available checked-block origin",
        ))?;
    if contract.contract.aliasing() != SemanticCapabilityMemoryAliasingV1::Disjoint(expected)
        || !supported_global_disjoint_binding_v1(contract.contract)
        || !contract.shape.arguments_match(call, contract.element)
        || !contract.shape.witness_borrow_matches(types, call)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "typed blocked store changed its mapping, witness borrow, or component contract",
        ));
    }
    if !blocked_mapping_fits_launch_v1(launch_upper_bound, lanes_per_block, elements_per_lane) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "typed blocked store requires an authenticated finite nonoverflowing launch",
        ));
    }
    // This slice models actual literal operands, never an opaque runtime index.
    let component = match call.arguments().get(2) {
        Some(operand @ SemanticOperandV1::Constant(_)) => constant_operand_value(operand, &[]),
        _ => None,
    }
    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
        "typed blocked store requires an exact literal component",
    ))?;
    if component >= elements_per_lane {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a blocked component is outside the authenticated elements-per-lane bound",
        ));
    }
    project_literal_blocked_index_v1(
        projected.value,
        lanes_per_block,
        elements_per_lane,
        component,
        operations,
        next_value,
        ranked_ir,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_literal_blocked_index_v1(
    raw: ProductionRankedValueV1,
    lanes_per_block: u64,
    elements_per_lane: u64,
    component: u64,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
    let dimensions = lanes_per_block
        .checked_mul(elements_per_lane)
        .filter(|size| *size != 0)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "blocked dimensions are zero or overflow u64",
        ))?;
    if component >= elements_per_lane {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a blocked component is outside the authenticated elements-per-lane bound",
        ));
    }
    let (base, offset) = if lanes_per_block == 1 {
        let elements =
            push_projected_index_constant_v1(elements_per_lane, operations, next_value, ranked_ir)?;
        (
            push_projected_index_binary_v1(
                IndexBinaryKindAttr::Multiply,
                raw,
                elements,
                operations,
                next_value,
                ranked_ir,
            )?,
            component,
        )
    } else {
        let lanes =
            push_projected_index_constant_v1(lanes_per_block, operations, next_value, ranked_ir)?;
        let elements =
            push_projected_index_constant_v1(dimensions, operations, next_value, ranked_ir)?;
        let block = push_projected_index_binary_v1(
            IndexBinaryKindAttr::Divide,
            raw,
            lanes,
            operations,
            next_value,
            ranked_ir,
        )?;
        let lane = push_projected_index_binary_v1(
            IndexBinaryKindAttr::Remainder,
            raw,
            lanes,
            operations,
            next_value,
            ranked_ir,
        )?;
        let block_base = push_projected_index_binary_v1(
            IndexBinaryKindAttr::Multiply,
            block,
            elements,
            operations,
            next_value,
            ranked_ir,
        )?;
        let lane_base = push_projected_index_binary_v1(
            IndexBinaryKindAttr::Add,
            block_base,
            lane,
            operations,
            next_value,
            ranked_ir,
        )?;
        let offset = component.checked_mul(lanes_per_block).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a blocked component offset overflows u64",
            ),
        )?;
        (lane_base, offset)
    };
    let offset = push_projected_index_constant_v1(offset, operations, next_value, ranked_ir)?;
    push_projected_index_binary_v1(
        IndexBinaryKindAttr::Add,
        base,
        offset,
        operations,
        next_value,
        ranked_ir,
    )
}
