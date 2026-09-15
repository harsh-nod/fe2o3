#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateRetainedArrayFactsV1 {
    element_type: SemanticTypeIdV1,
    element: PrivateRetainedSlotFactsV1,
    length: u64,
}

fn private_retained_array_facts_v1<W: PrivateArrayChargeV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    max_elements: usize,
    work: &mut W,
) -> Result<Option<PrivateRetainedArrayFactsV1>, W::Error> {
    work.charge_private_array_work(1)?;
    let Some(declaration) = types.get(ty.index() as usize) else {
        return Ok(None);
    };
    work.charge_private_array_work(1)?;
    let SemanticTypeShapeV1::Array { element, length } = declaration.shape() else {
        return Ok(None);
    };
    let Some(element_facts) = private_retained_slot_facts_v1(types, *element, work)? else {
        return Ok(None);
    };
    let layout = declaration.layout();
    work.charge_private_array_work(1)?;
    let SemanticFieldsShapeV1::Array {
        stride_bytes,
        count,
    } = layout.fields()
    else {
        return Ok(None);
    };
    work.charge_private_array_work(1)?;
    let Some(size) = stride_bytes.checked_mul(*length) else {
        return Ok(None);
    };
    // Length, field count, stride, total size, alignment, inhabitedness, i64 extent.
    work.charge_private_array_work(7)?;
    if *length == 0
        || *count != *length
        || element_facts.size != *stride_bytes
        || layout.size_bytes() != Some(size)
        || layout.alignment_bytes() != u64::from(element_facts.alignment)
        || layout.is_uninhabited()
        || size > i64::MAX as u64
    {
        return Ok(None);
    }
    work.charge_private_array_work(1)?;
    let Ok(elements) = usize::try_from(*length) else {
        return Ok(None);
    };
    work.charge_private_array_work(1)?;
    if elements > max_elements {
        return Ok(None);
    }
    Ok(Some(PrivateRetainedArrayFactsV1 {
        element_type: *element,
        element: element_facts,
        length: *length,
    }))
}

#[derive(Clone, Copy)]
struct PrivateArraySourceAccessV1<'s> {
    place: &'s SemanticPlaceV1,
    access: PrivateArrayAccessV1,
    volatility: SemanticVolatilityV1,
    atomic: Option<fe2o3_mir_model::semantic_mir_v1::SemanticAtomicAccessV1>,
}

fn private_array_operand_place_v1<'s, W: PrivateArrayChargeV1>(
    operand: &'s SemanticOperandV1,
    work: &mut W,
) -> Result<Option<&'s SemanticPlaceV1>, W::Error> {
    work.charge_private_array_work(1)?;
    Ok(match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        SemanticOperandV1::Constant(_) => None,
    })
}

fn private_array_rvalue_operand_v1<'s, W: PrivateArrayChargeV1>(
    rvalue: &'s SemanticRvalueKindV1,
    ordinal: u32,
    work: &mut W,
) -> Result<Option<&'s SemanticOperandV1>, W::Error> {
    work.charge_private_array_work(1)?;
    match rvalue {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => {
            work.charge_private_array_work(1)?;
            Ok((ordinal == 0).then_some(operand))
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            work.charge_private_array_work(1)?;
            Ok(match ordinal {
                0 => Some(left),
                1 => Some(right),
                _ => None,
            })
        }
        SemanticRvalueKindV1::CheckedBinary(checked) => {
            work.charge_private_array_work(1)?;
            Ok(match ordinal {
                0 => Some(checked.left()),
                1 => Some(checked.right()),
                _ => None,
            })
        }
        SemanticRvalueKindV1::UncheckedBinary(unchecked) => {
            work.charge_private_array_work(1)?;
            Ok(match ordinal {
                0 => Some(unchecked.left()),
                1 => Some(unchecked.right()),
                _ => None,
            })
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            work.charge_private_array_work(1)?;
            Ok(aggregate.operands().get(ordinal as usize))
        }
        _ => Ok(None),
    }
}

fn private_array_ordinary_source_access_v1(
    place: &SemanticPlaceV1,
    access: PrivateArrayAccessV1,
) -> PrivateArraySourceAccessV1<'_> {
    PrivateArraySourceAccessV1 {
        place,
        access,
        volatility: SemanticVolatilityV1::NonVolatile,
        atomic: None,
    }
}

fn private_array_source_access_v1<'s, W: PrivateArrayChargeV1>(
    statement: &'s SemanticStatementKindV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    work: &mut W,
) -> Result<Option<PrivateArraySourceAccessV1<'s>>, W::Error> {
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

    work.charge_private_array_work(1)?;
    match (statement, role) {
        (SemanticStatementKindV1::Assign(assignment), Role::Destination) => {
            work.charge_private_array_work(1)?;
            Ok(Some(private_array_ordinary_source_access_v1(
                assignment.destination(),
                PrivateArrayAccessV1::Write,
            )))
        }
        (SemanticStatementKindV1::Assign(assignment), Role::RvalueOperand(ordinal)) => {
            let Some(operand) =
                private_array_rvalue_operand_v1(assignment.value().kind(), ordinal, work)?
            else {
                return Ok(None);
            };
            Ok(private_array_operand_place_v1(operand, work)?.map(|place| {
                private_array_ordinary_source_access_v1(place, PrivateArrayAccessV1::Read)
            }))
        }
        (SemanticStatementKindV1::Assign(assignment), Role::RvaluePlace) => {
            work.charge_private_array_work(1)?;
            let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
                return Ok(None);
            };
            work.charge_private_array_work(3)?;
            Ok(Some(PrivateArraySourceAccessV1 {
                place: load.source(),
                access: PrivateArrayAccessV1::Read,
                volatility: load.volatility(),
                atomic: load.atomic(),
            }))
        }
        (SemanticStatementKindV1::Store(store), Role::StoreValue) => Ok(
            private_array_operand_place_v1(store.value(), work)?.map(|place| {
                private_array_ordinary_source_access_v1(place, PrivateArrayAccessV1::Read)
            }),
        ),
        (SemanticStatementKindV1::Store(store), Role::StoreDestination) => {
            work.charge_private_array_work(3)?;
            Ok(Some(PrivateArraySourceAccessV1 {
                place: store.destination(),
                access: PrivateArrayAccessV1::Write,
                volatility: store.volatility(),
                atomic: store.atomic(),
            }))
        }
        _ => Ok(None),
    }
}

fn private_array_operation_v1<'m, W: PrivateArrayChargeV1>(
    body: &'m FunctionBody,
    location: PrivateArrayPhysicalLocationV1,
    work: &mut W,
) -> Result<Option<&'m Operation>, W::Error> {
    work.charge_private_array_work(1)?;
    let Some(block) = body.blocks.get(location.block_ordinal) else {
        return Ok(None);
    };
    work.charge_private_array_work(1)?;
    if block.id != location.block {
        return Ok(None);
    }
    work.charge_private_array_work(1)?;
    Ok(block.operations.get(location.operation))
}

enum PrivateArrayRelationErrorV1<E> {
    Work(E),
    InvalidSource(&'static str),
    Incomplete(&'static str),
    Mismatch(&'static str),
}

impl<E> From<E> for PrivateArrayRelationErrorV1<E> {
    fn from(error: E) -> Self {
        Self::Work(error)
    }
}

fn private_array_unsigned_operation_v1<W: PrivateArrayChargeV1>(
    body: &FunctionBody,
    location: PrivateArrayPhysicalLocationV1,
    value: ValueId,
    scalar: ScalarType,
    work: &mut W,
) -> Result<u64, PrivateArrayRelationErrorV1<W::Error>> {
    use PrivateArrayRelationErrorV1::Mismatch;
    let operation = private_array_operation_v1(body, location, work)?
        .ok_or(Mismatch("unsigned definition coordinate is absent"))?;
    work.charge_private_array_work(3)?;
    let [result] = operation.results.as_slice() else {
        return Err(Mismatch("unsigned definition result count changed"));
    };
    if result.id != value || result.ty != Type::Scalar(scalar) {
        return Err(Mismatch("unsigned definition identity or type changed"));
    }
    work.charge_private_array_work(1)?;
    let value = match (&operation.kind, scalar) {
        (OperationKind::Constant(Constant::U8(value)), ScalarType::U8) => u64::from(*value),
        (OperationKind::Constant(Constant::U16(value)), ScalarType::U16) => u64::from(*value),
        (OperationKind::Constant(Constant::U32(value)), ScalarType::U32) => u64::from(*value),
        (OperationKind::Constant(Constant::U64(value)), ScalarType::U64) => *value,
        (OperationKind::Constant(Constant::Index(value)), ScalarType::Index) => *value,
        _ => {
            return Err(Mismatch(
                "unsigned definition is not the exact typed constant",
            ));
        }
    };
    Ok(value)
}

fn private_array_pointer_matches_v1<W: PrivateArrayChargeV1>(
    actual: &Type,
    element: PrivateRetainedElementFactsV1,
    work: &mut W,
) -> Result<bool, W::Error> {
    work.charge_private_array_work(1)?;
    let Type::Pointer(pointer) = actual else {
        return Ok(false);
    };
    work.charge_private_array_work(2)?;
    if pointer.address_space != AddressSpace::Private || pointer.access != AccessMode::ReadWrite {
        return Ok(false);
    }
    element.matches_borrowed(pointer.pointee.as_ref(), work)
}

fn private_array_slot_facts_equal_v1<W: PrivateArrayChargeV1>(
    left: PrivateRetainedSlotFactsV1,
    right: PrivateRetainedSlotFactsV1,
    work: &mut W,
) -> Result<bool, W::Error> {
    work.charge_private_array_work(2)?;
    if left.size != right.size || left.alignment != right.alignment {
        return Ok(false);
    }
    work.charge_private_array_work(1)?;
    Ok(match (left.element, right.element) {
        (PrivateRetainedElementFactsV1::Scalar(a), PrivateRetainedElementFactsV1::Scalar(b)) => {
            work.charge_private_array_work(1)?;
            a == b
        }
        (
            PrivateRetainedElementFactsV1::ThinPointer {
                element: a,
                space: a_space,
                access: a_access,
            },
            PrivateRetainedElementFactsV1::ThinPointer {
                element: b,
                space: b_space,
                access: b_access,
            },
        ) => {
            work.charge_private_array_work(3)?;
            a == b && a_space == b_space && a_access == b_access
        }
        _ => false,
    })
}

fn private_array_exact_relation_v1<W: PrivateArrayChargeV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    body: &FunctionBody,
    owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    slot: &PrivateArraySlotV1,
    effect: &PrivateArrayEffectV1,
    max_elements: usize,
    work: &mut W,
) -> Result<u64, PrivateArrayRelationErrorV1<W::Error>> {
    use PrivateArrayRelationErrorV1::{Incomplete, InvalidSource, Mismatch};
    work.charge_private_array_work(5)?;
    if slot.owner != owner
        || effect.owner != owner
        || slot.function != semantic_function
        || effect.function != semantic_function
        || slot.local != effect.local
    {
        return Err(Mismatch("private array owner, function, or local changed"));
    }
    // Local lookup/type comparison, source block lookup, and statement lookup.
    work.charge_private_array_work(4)?;
    let local = function
        .locals()
        .get(slot.local as usize)
        .ok_or(InvalidSource("array local is absent"))?;
    if local.ty() != slot.semantic_type {
        return Err(Mismatch("array source type changed"));
    }
    let source = function
        .blocks()
        .get(effect.semantic_block as usize)
        .and_then(|block| block.statements().get(effect.semantic_statement as usize))
        .ok_or(InvalidSource("array statement is absent"))?;
    let access = private_array_source_access_v1(source.kind(), effect.role, work)?
        .ok_or(InvalidSource("array operand role is not an actual access"))?;
    work.charge_private_array_work(5)?;
    if access.place.local().index() != slot.local
        || access.place.ty() != effect.semantic_type
        || access.access != effect.access
    {
        return Err(Mismatch("array source occurrence changed"));
    }
    if access.volatility != SemanticVolatilityV1::NonVolatile || access.atomic.is_some() {
        return Err(Incomplete(
            "volatile or atomic private array relation is not supported",
        ));
    }
    let facts = private_retained_array_facts_v1(types, slot.semantic_type, max_elements, work)?
        .ok_or(Incomplete(
            "private array has no supported exact fixed layout",
        ))?;
    // Three scalar checks and the following single-projection shape check.
    work.charge_private_array_work(4)?;
    if facts.element_type != slot.element_type
        || facts.length != slot.length
        || effect.semantic_type != slot.element_type
        || !private_array_slot_facts_equal_v1(facts.element, slot.element_facts, work)?
    {
        return Err(Mismatch("private array layout facts changed"));
    }
    let [projection] = access.place.projections() else {
        return Err(Incomplete(
            "private array requires one exact index projection",
        ));
    };
    work.charge_private_array_work(1)?;
    if projection.result_type() != slot.element_type {
        return Err(Mismatch("private array projection element type changed"));
    }

    work.charge_private_array_work(6)?;
    if slot.count_location.block_ordinal != 0
        || slot.alloca_location.block_ordinal != 0
        || slot.count_location.block != slot.alloca_location.block
        || slot.alloca_location.block != BlockId(function.entry().index())
        || slot.count_location.operation.checked_add(1) != Some(slot.alloca_location.operation)
    {
        return Err(Mismatch("private counted allocation prologue changed"));
    }
    let count = private_array_unsigned_operation_v1(
        body,
        slot.count_location,
        slot.count,
        ScalarType::Index,
        work,
    )?;
    work.charge_private_array_work(1)?;
    if count != slot.length {
        return Err(Mismatch("private array counted extent changed"));
    }
    let allocation = private_array_operation_v1(body, slot.alloca_location, work)?
        .ok_or(Mismatch("private counted allocation is absent"))?;
    work.charge_private_array_work(2)?;
    let [pointer] = allocation.results.as_slice() else {
        return Err(Mismatch("private counted allocation result count changed"));
    };
    let OperationKind::Alloca {
        element,
        count,
        address_space,
        alignment,
    } = &allocation.kind
    else {
        return Err(Mismatch("private array allocation is not a counted Alloca"));
    };
    work.charge_private_array_work(4)?;
    if pointer.id != slot.pointer
        || *count != Some(slot.count)
        || *address_space != AddressSpace::Private
        || *alignment != slot.element_facts.alignment
    {
        return Err(Mismatch(
            "private counted allocation identity or access changed",
        ));
    }
    if !slot.element_facts.element.matches_borrowed(element, work)?
        || !private_array_pointer_matches_v1(&pointer.ty, slot.element_facts.element, work)?
    {
        return Err(Mismatch("private counted allocation element type changed"));
    }

    let expected_index = match (projection.kind(), effect.original_index) {
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            },
            PrivateArrayIndexV1::ConstantIndex {
                offset: recorded_offset,
                min_length,
                from_end: recorded_from_end,
            },
        ) => {
            work.charge_private_array_work(5)?;
            if offset != recorded_offset
                || minimum_length != min_length
                || from_end != recorded_from_end
                || minimum_length > slot.length
            {
                return Err(Mismatch("private array constant projection changed"));
            }
            if from_end {
                slot.length
                    .checked_sub(offset)
                    .ok_or(Mismatch("private array reverse index underflow"))?
            } else {
                offset
            }
        }
        (
            SemanticProjectionKindV1::Index(local),
            PrivateArrayIndexV1::Local {
                local: recorded_local,
                semantic_type,
                original,
                physical_type,
                direct_definition,
            },
        ) => {
            work.charge_private_array_work(3)?;
            if local.index() != recorded_local {
                return Err(Mismatch("private array original index local changed"));
            }
            let declaration = function
                .locals()
                .get(recorded_local as usize)
                .ok_or(InvalidSource("private array index local is absent"))?;
            if declaration.ty() != semantic_type {
                return Err(Mismatch("private array original index type changed"));
            }
            work.charge_private_array_work(1)?;
            let declaration = types
                .get(semantic_type.index() as usize)
                .ok_or(InvalidSource("private array index type is absent"))?;
            let source_scalar = private_scalar_declaration_fact_v1(declaration, work)?.ok_or(
                Incomplete("private array index is not one supported scalar"),
            )?;
            work.charge_private_array_work(1)?;
            if source_scalar != physical_type {
                return Err(Incomplete(
                    "private array index representation requires additional transport proof",
                ));
            }
            let location = direct_definition.ok_or(Incomplete(
                "private array index lacks an actual direct unsigned definition",
            ))?;
            private_array_unsigned_operation_v1(body, location, original, physical_type, work)?
        }
        _ => return Err(Mismatch("private array original index projection changed")),
    };
    work.charge_private_array_work(1)?;
    if expected_index >= slot.length {
        return Err(Mismatch(
            "private array exact source index exceeds its extent",
        ));
    }
    let offset_location = effect
        .offset_location
        .ok_or(Incomplete("private array offset is not an actual constant"))?;
    let offset = private_array_unsigned_operation_v1(
        body,
        offset_location,
        effect.offset,
        ScalarType::Index,
        work,
    )?;
    work.charge_private_array_work(1)?;
    if offset != expected_index {
        return Err(Mismatch(
            "private array physical offset differs from its original source index",
        ));
    }
    work.charge_private_array_work(12)?;
    if effect.memory_location.block != BlockId(effect.semantic_block)
        || effect.gep_location.block != effect.memory_location.block
        || effect.gep_location.block_ordinal != effect.memory_location.block_ordinal
        || offset_location.block != effect.memory_location.block
        || offset_location.block_ordinal != effect.memory_location.block_ordinal
        || offset_location.operation < effect.source_first_operation
        || offset_location.operation >= effect.gep_location.operation
        || effect.gep_location.operation.checked_add(1) != Some(effect.memory_location.operation)
        || effect.memory_location.operation >= effect.source_end_operation
    {
        return Err(Mismatch(
            "private array effect is outside its exact source operation span",
        ));
    }
    let gep = private_array_operation_v1(body, effect.gep_location, work)?
        .ok_or(Mismatch("private array GEP is absent"))?;
    // Result shape/id, GEP kind, and the base/offset scalar identities.
    work.charge_private_array_work(5)?;
    let [result] = gep.results.as_slice() else {
        return Err(Mismatch("private array GEP result count changed"));
    };
    if result.id != effect.gep
        || !matches!(gep.kind, OperationKind::GetElementPointer { base, offset } if base == slot.pointer && offset == effect.offset)
    {
        return Err(Mismatch("private array GEP allocation or index changed"));
    }
    if !private_array_pointer_matches_v1(&result.ty, slot.element_facts.element, work)? {
        return Err(Mismatch("private array GEP type changed"));
    }
    let memory = private_array_operation_v1(body, effect.memory_location, work)?
        .ok_or(Mismatch("private array memory operation is absent"))?;
    work.charge_private_array_work(1)?;
    let access = match (&memory.kind, effect.access) {
        (OperationKind::Load { pointer, access }, PrivateArrayAccessV1::Read) => {
            work.charge_private_array_work(2)?;
            let [result] = memory.results.as_slice() else {
                return Err(Mismatch("private array load result count changed"));
            };
            if *pointer != effect.gep
                || !slot
                    .element_facts
                    .element
                    .matches_borrowed(&result.ty, work)?
            {
                return Err(Mismatch("private array load pointer or element changed"));
            }
            access
        }
        (
            OperationKind::Store {
                pointer, access, ..
            },
            PrivateArrayAccessV1::Write,
        ) => {
            work.charge_private_array_work(2)?;
            if *pointer != effect.gep || !memory.results.is_empty() {
                return Err(Mismatch("private array store pointer or result changed"));
            }
            access
        }
        _ => return Err(Mismatch("private array memory effect kind changed")),
    };
    work.charge_private_array_work(3)?;
    if access.address_space != AddressSpace::Private
        || access.alignment != slot.element_facts.alignment
        || access.volatile
    {
        return Err(Mismatch("private array memory access contract changed"));
    }
    Ok(expected_index)
}

fn private_array_query_error_v1(
    error: PrivateArrayRelationErrorV1<SemanticKirPrivateArrayQueryErrorV1>,
) -> SemanticKirPrivateArrayQueryErrorV1 {
    match error {
        PrivateArrayRelationErrorV1::Work(error) => error,
        PrivateArrayRelationErrorV1::InvalidSource(detail) => {
            SemanticKirPrivateArrayQueryErrorV1::InvalidSource(detail)
        }
        PrivateArrayRelationErrorV1::Incomplete(detail) => {
            SemanticKirPrivateArrayQueryErrorV1::Incomplete(detail)
        }
        PrivateArrayRelationErrorV1::Mismatch(detail) => {
            SemanticKirPrivateArrayQueryErrorV1::Mismatch(detail)
        }
    }
}

fn private_array_equal_bytes_v1<W: PrivateArrayChargeV1>(
    left: &[u8],
    right: &[u8],
    work: &mut W,
) -> Result<bool, W::Error> {
    work.charge_private_array_work(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        work.charge_private_array_work(1)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn private_array_instance_v1<'r, W: PrivateArrayChargeV1>(
    rows: &'r PrivateArrayCorrespondenceV1,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    work: &mut W,
) -> Result<Option<&'r PrivateArrayInstanceV1>, W::Error> {
    let key = [owner.index() as usize, function.index() as usize];
    let index = private_array_binary_search_v1(
        &rows.instances,
        |row| [row.owner.index() as usize, row.function.index() as usize],
        key,
        work,
    )?;
    match index {
        Ok(index) => {
            work.charge_private_array_work(1)?;
            Ok(rows.instances.get(index))
        }
        Err(_) => Ok(None),
    }
}

fn private_array_absent_query_v1(
    promoted: bool,
    requires_slot: bool,
    projection: SemanticProjectionKindV1,
    length: u64,
    work: &mut PrivateArrayQueryWorkV1<'_, '_>,
) -> Result<bool, SemanticKirPrivateArrayQueryErrorV1> {
    use SemanticKirPrivateArrayQueryErrorV1::{Incomplete, InvalidSource, Mismatch};
    work.charge_private_array_work(1)?;
    if requires_slot {
        return Err(Mismatch(
            "required retained array instance or slot is absent",
        ));
    }
    work.charge_private_array_work(2)?;
    let SemanticProjectionKindV1::ConstantIndex {
        offset,
        minimum_length,
        from_end,
    } = projection
    else {
        return Err(Incomplete(
            "unretained array index lacks a bounded constant-index witness",
        ));
    };
    if !promoted {
        return Err(Incomplete(
            "unretained array selection is not locally established",
        ));
    }
    work.charge_private_array_work(1)?;
    let index = if from_end {
        work.charge_private_array_work(1)?;
        length
            .checked_sub(offset)
            .ok_or(InvalidSource("unretained reverse array index underflows"))?
    } else {
        offset
    };
    work.charge_private_array_work(2)?;
    if index >= length || minimum_length > length {
        return Err(InvalidSource(
            "unretained array index exceeds its exact extent",
        ));
    }
    Ok(false)
}

impl ProductionPreRankedKirOwnerV1 {
    /// Checks one original indexed private-array access against this owner's
    /// actual counted allocation, source index, GEP and ordinary memory effect.
    ///
    /// `false` means this supported source occurrence has no retained fixed-array
    /// slot. A retained but unsupported or missing relation is an error. Only
    /// constant indices are supported; the result grants no source-value,
    /// functional-refinement, artifact or launch authority. The caller must keep
    /// this owner's graph and assertion-origin payload reserved in `budget`.
    /// This allocation-free query uses the constructor-selected body; it does
    /// not replay lowering or invoke a new source planner.
    pub fn has_materialized_private_array_access(
        &self,
        selected_root: SemanticFunctionIdV1,
        expected_body: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<bool, SemanticKirPrivateArrayQueryErrorV1> {
        self.private_array_constant_index_v1(selected_root, expected_body, site, role, budget)
            .map(|index| index.is_some())
    }

    /// Returns the exact constant index for one checked, retained private-array
    /// occurrence. This performs the same source/allocation/index/GEP/effect
    /// relation as `has_materialized_private_array_access` on this owner.
    ///
    /// `None` has exactly that query's supported-unretained meaning. Unsupported,
    /// absent, or mismatched retained relations remain errors. The returned
    /// integer is inert: it grants no source-value, refinement, artifact or
    /// launch authority. It neither installs a row nor changes the source plan.
    /// Keep the same graph and assertion-origin storage floor reserved. One
    /// fixed extraction work unit precedes the unchanged allocation-free query.
    pub fn materialized_private_array_constant_index(
        &self,
        selected_root: SemanticFunctionIdV1,
        expected_body: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<u64>, SemanticKirPrivateArrayQueryErrorV1> {
        budget
            .charge_work(1)
            .map_err(SemanticKirPrivateArrayQueryErrorV1::Resource)?;
        self.private_array_constant_index_v1(selected_root, expected_body, site, role, budget)
    }

    fn private_array_constant_index_v1(
        &self,
        selected_root: SemanticFunctionIdV1,
        expected_body: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<u64>, SemanticKirPrivateArrayQueryErrorV1> {
        use SemanticKirPrivateArrayQueryErrorV1::{Incomplete, InvalidSource, Mismatch, Resource};
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as ResourceError;
        budget.charge_work(3).map_err(Resource)?;
        let floor = self
            .executable_storage
            .retained_storage()
            .checked_add(self.assert_origins.storage.payload_storage())
            .ok_or(Resource(ResourceError::Arithmetic))?;
        if budget.storage() < floor {
            return Err(Resource(ResourceError::Accounting));
        }
        let mut work = PrivateArrayQueryWorkV1 { budget };
        let root = private_array_binary_search_v1(
            &self.launch_roots,
            |row| [row.selected_root.index() as usize],
            [selected_root.index() as usize],
            &mut work,
        )?
        .map_err(|_| InvalidSource("selected root is absent from this owner"))?;
        work.charge_private_array_work(5)?;
        let entry = self
            .correspondence
            .lowered_functions
            .get(root)
            .ok_or(Mismatch("selected root entry row is absent"))?;
        if entry.correspondence_owner != selected_root
            || entry.role != SemanticKirFunctionRoleV1::KernelEntry
        {
            return Err(Mismatch("selected root entry prefix changed"));
        }
        work.charge_private_array_work(1)?;
        if entry.semantic_function != expected_body {
            return Err(InvalidSource(
                "requested body differs from the constructor-selected entry",
            ));
        }
        let module = self.executable.module();
        let kernel = module
            .kernels
            .get(root)
            .ok_or(Mismatch("selected root physical kernel is absent"))?;
        let lowered = module
            .functions
            .get(root)
            .ok_or(Mismatch("selected root physical function is absent"))?;
        if !private_array_equal_bytes_v1(
            entry.kernel_ir_function.as_str().as_bytes(),
            lowered.id.as_str().as_bytes(),
            &mut work,
        )? || !private_array_equal_bytes_v1(
            kernel.entry.as_str().as_bytes(),
            lowered.id.as_str().as_bytes(),
            &mut work,
        )? {
            return Err(Mismatch("selected root physical function identity changed"));
        }
        work.charge_private_array_work(3)?;
        let body = lowered
            .body
            .as_ref()
            .ok_or(Mismatch("selected root physical body is absent"))?;
        let semantic = self.semantic_ssa.source_semantic();
        let function = semantic
            .functions()
            .get(entry.semantic_function.index() as usize)
            .ok_or(Mismatch("selected source body is absent"))?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(Incomplete(
                "private array terminator effects require a separate relation",
            ));
        };
        work.charge_private_array_work(2)?;
        let source = function
            .blocks()
            .get(block.get() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(InvalidSource("private array source coordinate is absent"))?;
        let access = private_array_source_access_v1(source.kind(), role, &mut work)?.ok_or(
            InvalidSource("private array source role is not an actual place access"),
        )?;
        work.charge_private_array_work(4)?;
        if access.volatility != SemanticVolatilityV1::NonVolatile || access.atomic.is_some() {
            return Err(Incomplete(
                "volatile or atomic private array relation is not supported",
            ));
        }
        let [projection] = access.place.projections() else {
            return Err(Incomplete(
                "private array requires one exact index projection",
            ));
        };
        if !matches!(
            projection.kind(),
            SemanticProjectionKindV1::Index(_) | SemanticProjectionKindV1::ConstantIndex { .. }
        ) {
            return Err(Incomplete(
                "private array projection is not an element index",
            ));
        }
        work.charge_private_array_work(1)?;
        let local = function
            .locals()
            .get(access.place.local().index() as usize)
            .ok_or(InvalidSource("private array local is absent"))?;
        let facts = private_retained_array_facts_v1(
            semantic.types(),
            local.ty(),
            self.limits.max_operations,
            &mut work,
        )?
        .ok_or(Incomplete(
            "source place has no supported fixed-array layout",
        ))?;
        work.charge_private_array_work(2)?;
        if access.place.ty() != facts.element_type || projection.result_type() != facts.element_type
        {
            return Err(InvalidSource(
                "private array projection element type differs",
            ));
        }
        work.charge_private_array_work(2)?;
        let plan = self
            .semantic_ssa
            .plan_for_function(entry.semantic_function)
            .ok_or(Mismatch("selected source SSA plan is absent"))?;
        let variable = access.place.local().index() as usize;
        // Both sealed lists are unique, ascending filters of dense source variable order.
        // Reuse those constructor invariants without a query-time scan or new planner.
        let promoted = private_array_binary_search_v1(
            plan.plan().promoted_variables(),
            |value| [value.get() as usize],
            [variable],
            &mut work,
        )?
        .is_ok();
        let cross_edge = private_array_binary_search_v1(
            plan.retained_cross_edge_variables(),
            |value| [value.get() as usize],
            [variable],
            &mut work,
        )?
        .is_ok();
        work.charge_private_array_work(1)?;
        // One role classification; the two booleans are already computed membership facts.
        let requires_slot = !promoted
            && (cross_edge
                || matches!(
                    role,
                    fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
                        | fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::StoreDestination
                        | fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvaluePlace
                ));
        let rows = &self.correspondence.private_arrays;
        let Some(instance) =
            private_array_instance_v1(rows, selected_root, entry.semantic_function, &mut work)?
        else {
            return private_array_absent_query_v1(
                promoted,
                requires_slot,
                projection.kind(),
                facts.length,
                &mut work,
            )
            .map(|_| None);
        };
        work.charge_private_array_work(4)?;
        if instance.lowered_function_ordinal != root || instance.module_function_ordinal != root {
            return Err(Mismatch(
                "private array selected instance coordinates changed",
            ));
        }
        let slots = rows
            .slots
            .get(instance.slot_start..instance.slot_end)
            .ok_or(Mismatch("private array instance slot range changed"))?;
        let effects = rows
            .effects
            .get(instance.effect_start..instance.effect_end)
            .ok_or(Mismatch("private array instance effect range changed"))?;
        let local = access.place.local().index();
        let Ok(slot_index) = private_array_binary_search_v1(
            slots,
            |row| [row.local as usize],
            [local as usize],
            &mut work,
        )?
        else {
            return private_array_absent_query_v1(
                promoted,
                requires_slot,
                projection.kind(),
                facts.length,
                &mut work,
            )
            .map(|_| None);
        };
        work.charge_private_array_work(1)?;
        if promoted {
            return Err(Mismatch(
                "retained array slot contradicts sealed SSA promotion",
            ));
        }
        work.charge_private_array_work(2)?;
        let slot = &slots[slot_index];
        let role_key = private_array_role_key_v1(role).ok_or(InvalidSource(
            "private array operand role has no supported order",
        ))?;
        let key = [
            block.get() as usize,
            statement as usize,
            role_key.0 as usize,
            role_key.1 as usize,
        ];
        let effect_index = private_array_binary_search_v1(
            effects,
            |row| {
                let role = private_array_role_key_v1(row.role).unwrap_or((u8::MAX, u32::MAX));
                [
                    row.semantic_block as usize,
                    row.semantic_statement as usize,
                    role.0 as usize,
                    role.1 as usize,
                ]
            },
            key,
            &mut work,
        )?
        .map_err(|_| {
            Incomplete("retained private array occurrence has no recorded ordinary effect")
        })?;
        work.charge_private_array_work(1)?;
        let index = private_array_exact_relation_v1(
            semantic.types(),
            function,
            body,
            selected_root,
            entry.semantic_function,
            slot,
            &effects[effect_index],
            self.limits.max_operations,
            &mut work,
        )
        .map_err(private_array_query_error_v1)?;
        Ok(Some(index))
    }
}
