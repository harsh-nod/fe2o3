/// Interpretation of one physical parameter from a borrowed aggregate field.
/// This describes emission, not a proof of its lifetime or aliasing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirBorrowedParameterTransportV1 {
    /// The parameter addresses persistent storage for the source scalar field.
    ScalarAddress,
    /// The parameter preserves the reference stored in the source field.
    ReferenceValue,
    /// The parameter preserves the whole slice stored in the source field.
    SliceValue,
}

/// A borrowed-field emission row, separate from by-value component bindings.
/// The reference ABI and physical signature are rechecked by argument views;
/// source ownership, initialization and use closure require independent replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticKirBorrowedParameterBindingV1 {
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_local: SemanticLocalIdV1,
    reference_type: SemanticTypeIdV1,
    semantic_component_type: SemanticTypeIdV1,
    projection: Box<[u32]>,
    transport: SemanticKirBorrowedParameterTransportV1,
    kernel_ir_value: ValueId,
}

impl SemanticKirBorrowedParameterBindingV1 {
    /// Root qualifying this source-to-physical association.
    pub const fn correspondence_owner(&self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }

    /// Source function containing the reference parameter.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Original reference local, not a replacement scalar local.
    pub const fn semantic_local(&self) -> SemanticLocalIdV1 {
        self.semantic_local
    }

    /// Original reference type retaining source access and referent identity.
    pub const fn reference_type(&self) -> SemanticTypeIdV1 {
        self.reference_type
    }

    /// Type of the field in the source referent.
    pub const fn semantic_component_type(&self) -> SemanticTypeIdV1 {
        self.semantic_component_type
    }

    /// Field indices relative to the dereferenced aggregate, never byte offsets.
    pub fn projection(&self) -> &[u32] {
        &self.projection
    }

    /// Whether the physical value is a field address or a stored carrier.
    pub const fn transport(&self) -> SemanticKirBorrowedParameterTransportV1 {
        self.transport
    }

    /// Actual parameter value in the associated physical function.
    pub const fn kernel_ir_value(&self) -> ValueId {
        self.kernel_ir_value
    }
}

fn borrowed_parameter_transport_v1(
    leaf: &BorrowedAggregateLeafV1,
) -> SemanticKirBorrowedParameterTransportV1 {
    match leaf.transport {
        BorrowedAggregateLeafTransportV1::ScalarSlot { .. } => {
            SemanticKirBorrowedParameterTransportV1::ScalarAddress
        }
        BorrowedAggregateLeafTransportV1::InvariantReference { .. } => {
            SemanticKirBorrowedParameterTransportV1::ReferenceValue
        }
        BorrowedAggregateLeafTransportV1::InvariantSlice { .. } => {
            SemanticKirBorrowedParameterTransportV1::SliceValue
        }
    }
}

fn check_borrowed_argument_parameters_v1(
    shape: &BorrowedAggregateShapeV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    target: &Function,
    physical: &mut [IndexedArgumentTraceV1<'_>],
    slot: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let body = target.body.as_ref().ok_or_else(mismatch)?;
    for leaf in &shape.leaves {
        budget.charge_work(argument_sum_v1(&[40, leaf.path.fields.len()])?)?;
        let value = *body.parameters.get(*slot).ok_or_else(mismatch)?;
        if target.signature.parameters.get(*slot)
            != Some(&leaf.parameter_type(shape.access, budget)?)
        {
            return Err(mismatch());
        }
        let index = physical
            .binary_search_by_key(&value, |row| row.trace.value())
            .map_err(|_| mismatch())?;
        let row = &mut physical[index];
        let PhysicalArgumentTraceV1::Borrowed(binding) = row.trace else {
            return Err(mismatch());
        };
        if row.used
            || binding.semantic_local != mapped.local()
            || binding.reference_type != shape.reference_type
            || binding.semantic_component_type != leaf.semantic_type
            || binding.projection.as_ref() != leaf.path.fields.as_ref()
            || binding.transport != borrowed_parameter_transport_v1(leaf)
        {
            return Err(mismatch());
        }
        row.used = true;
        *slot = slot.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    Ok(())
}

struct BorrowedArgumentFrameV1 {
    ty: SemanticTypeIdV1,
    next: u64,
    path_length: usize,
    first: usize,
}

fn visit_borrowed_argument_structure_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    parameters: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    visit: &mut impl FnMut(
        ParameterStructureNodeV1<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(4)?;
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(reference.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(mismatch());
    };
    let (mut frames, mut path) = (Vec::new(), Vec::new());
    let (mut frame_capacity, mut path_capacity) = (0, 0);
    argument_scratch_push_v1(
        &mut path,
        &mut path_capacity,
        ProductionArgumentProjectionV1::Dereference,
        budget,
    )?;
    argument_scratch_push_v1(
        &mut frames,
        &mut frame_capacity,
        BorrowedArgumentFrameV1 {
            ty: pointer.pointee(),
            next: 0,
            path_length: 1,
            first: 0,
        },
        budget,
    )?;
    let (mut nodes, mut slot) = (1_usize, 0_usize);
    while let Some(frame) = frames.last_mut() {
        budget.charge_work(6)?;
        let declaration = types.get(frame.ty.index() as usize).ok_or_else(mismatch)?;
        if let Some((ty, projection)) = atomic_argument_child_v1(declaration.shape(), frame.next)? {
            nodes = nodes.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            if nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(mismatch());
            }
            frame.next = frame
                .next
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            argument_scratch_push_v1(&mut path, &mut path_capacity, projection, budget)?;
            argument_scratch_push_v1(
                &mut frames,
                &mut frame_capacity,
                BorrowedArgumentFrameV1 {
                    ty,
                    next: 0,
                    path_length: path.len(),
                    first: slot,
                },
                budget,
            )?;
        } else {
            let frame = frames.pop().ok_or_else(mismatch)?;
            let represented_leaf = matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(_)
                    | SemanticTypeShapeV1::ValidityScalar(_)
                    | SemanticTypeShapeV1::Pointer(_)
            );
            if represented_leaf {
                slot = slot.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                if slot > parameters {
                    return Err(mismatch());
                }
            }
            visit(
                ParameterStructureNodeV1 {
                    ty: frame.ty,
                    path: &path[..frame.path_length],
                    physical: frame.first..slot,
                    represented_leaf,
                },
                budget,
            )?;
            if let Some(parent) = frames.last() {
                path.truncate(parent.path_length);
            }
        }
    }
    if slot != parameters {
        return Err(mismatch());
    }
    visit(
        ParameterStructureNodeV1 {
            ty: reference,
            path: &[],
            physical: 0..slot,
            represented_leaf: false,
        },
        budget,
    )
}

#[cfg(test)]
#[path = "production_borrowed_parameter_v1_tests.rs"]
mod borrowed_parameter_tests_v1;
