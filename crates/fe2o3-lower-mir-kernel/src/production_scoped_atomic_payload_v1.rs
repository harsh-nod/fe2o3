// Original-source AtomicRmw payloads. This is not shared-reference/UnsafeCell
// admission, an atomic helper ABI policy, or an optimized-source effect proof.
// Pointer provenance still belongs to the original source address relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedAtomicEffectV1 {
    kind: AtomicKind,
    access: MemoryAccess,
    scope: SynchronizationScope,
    ordering: MemoryOrdering,
}

impl ScopedAtomicEffectV1 {
    fn from_operation(atomic: &Atomic) -> Option<Self> {
        if !matches!(
            atomic.kind,
            AtomicKind::Exchange
                | AtomicKind::Add
                | AtomicKind::Subtract
                | AtomicKind::Min
                | AtomicKind::Max
                | AtomicKind::BitAnd
                | AtomicKind::BitOr
                | AtomicKind::BitXor
        ) || atomic.value.is_none()
            || atomic.compare.is_some()
            || atomic.failure_ordering.is_some()
            || atomic.access.volatile
        {
            return None;
        }
        Some(Self {
            kind: atomic.kind,
            access: atomic.access,
            scope: atomic.scope,
            ordering: atomic.ordering,
        })
    }

    fn matches_source(
        self,
        source: &SemanticAtomicRmwV1,
        scalar: ScalarType,
        access: MemoryAccess,
    ) -> bool {
        lower_atomic_rmw_kind(source.operation(), scalar) == Some(self.kind)
            && lower_atomic_scope(source.access().scope()) == Some(self.scope)
            && lower_atomic_ordering(source.access().ordering()) == self.ordering
            && self.access == access
            && !self.access.volatile
    }
}

fn scoped_atomic_source_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    frame: Option<ScopedMemoryFrameV29>,
    source: ScopedMemoryStoreSourceV29,
    effect: ScopedAtomicEffectV1,
    result: &ValueDef,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<ExecutionSiteV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(20)?;
    let ScopedMemoryStoreSourceV29::Operand {
        site,
        role: ExecutionOperandV29::AtomicValue,
        ty,
        ..
    } = source
    else {
        return Err(scoped_memory_error_v29());
    };
    let Some(SemanticStatementKindV1::AtomicRmw(original)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(scoped_memory_error_v29());
    };
    let Type::Scalar(scalar) = &result.ty else {
        return Err(scoped_memory_error_v29());
    };
    if frame
        != Some(ScopedMemoryFrameV29::operand(
            site,
            Some(ExecutionOperandV29::AtomicAddress),
        ))
        || semantic_operand_type(original.value()) != ty
        || !scoped_payload_type_matches_v29(types, ty, &result.ty, budget)?
        || !scoped_payload_type_matches_v29(types, original.address().ty(), &result.ty, budget)?
        || !scoped_payload_type_matches_v29(types, original.destination().ty(), &result.ty, budget)?
        || !effect.matches_source(
            original,
            *scalar,
            memory_access_for_type(types, original.address().ty(), effect.access.address_space)?,
        )
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(site)
}

fn scoped_recorded_atomic_payload_v1(
    recorder: &ScopedMemoryRecorderV29,
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    atomic: &Atomic,
    results: &[ValueDef],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<ScopedMemoryPayloadV29>, ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    // Once a scoped recorder sees an Atomic, missing payload is a refusal, not
    // an unrecorded memory effect. Existing unscoped lowering is unchanged.
    let (value, source) = recorder.store_payload.ok_or_else(scoped_memory_error_v29)?;
    let effect =
        ScopedAtomicEffectV1::from_operation(atomic).ok_or_else(scoped_memory_error_v29)?;
    let [result] = results else {
        return Err(scoped_memory_error_v29());
    };
    if atomic.value != Some(value) {
        return Err(scoped_memory_error_v29());
    }
    scoped_atomic_source_v1(
        types,
        function,
        recorder.frame,
        source,
        effect,
        result,
        budget,
    )?;
    Ok(Some(ScopedMemoryPayloadV29::AtomicRmw {
        result: result.id,
        value,
        source,
        effect,
    }))
}

fn check_scoped_atomic_payload_v1(
    function: &SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    result: ValueId,
    value: ValueId,
    source: ScopedMemoryStoreSourceV29,
    effect: ScopedAtomicEffectV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let ScopedMemoryAnchorKindV29::Access { pointer, .. } = row.kind else {
        return Err(scoped_memory_error_v29());
    };
    let OperationKind::Atomic(actual) = &operation.kind else {
        return Err(scoped_memory_error_v29());
    };
    let [output] = operation.results.as_slice() else {
        return Err(scoped_memory_error_v29());
    };
    if actual.pointer != pointer
        || actual.value != Some(value)
        || output.id != result
        || ScopedAtomicEffectV1::from_operation(actual) != Some(effect)
    {
        return Err(scoped_memory_error_v29());
    }
    let site = scoped_atomic_source_v1(
        occurrences.owner().source_semantic().types(),
        function,
        row.source,
        source,
        effect,
        output,
        budget,
    )?;
    let ScopedMemoryStoreSourceV29::Operand { ty, source, .. } = source else {
        return Err(scoped_memory_error_v29());
    };
    match (
        source,
        scoped_source_operand_v29(function, site, ExecutionOperandV29::AtomicValue),
    ) {
        (ScopedMemoryOperandSourceV29::Constant, Some(SemanticOperandV1::Constant(original)))
            if original.ty() == ty =>
        {
            Ok(())
        }
        (
            ScopedMemoryOperandSourceV29::Place(
                capture @ ScopedMemoryOccurrenceV29::Promoted { .. },
            )
            | ScopedMemoryOperandSourceV29::Memory {
                occurrence: capture,
                ..
            },
            Some(SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original)),
        ) if original.ty() == ty => check_scoped_payload_occurrence_v29(
            occurrences,
            site,
            ExecutionOperandV29::AtomicValue,
            original,
            capture,
            budget,
        ),
        _ => Err(scoped_memory_error_v29()),
    }
}

// Replay the effect against the original AtomicRmw statement. Type/layout and
// address provenance were separately checked when the immutable payload was
// issued; this query must not reinterpret it as an ordinary load or store.
fn check_scoped_atomic_effect_v1(
    function: &SemanticFunctionDeclV1,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    result: ValueId,
    value: ValueId,
    source: ScopedMemoryStoreSourceV29,
    effect: ScopedAtomicEffectV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let ScopedMemoryStoreSourceV29::Operand {
        site,
        role: ExecutionOperandV29::AtomicValue,
        ty,
        ..
    } = source
    else {
        return Err(scoped_memory_error_v29());
    };
    let Some(SemanticStatementKindV1::AtomicRmw(original)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(scoped_memory_error_v29());
    };
    let ScopedMemoryAnchorKindV29::Access { pointer, .. } = row.kind else {
        return Err(scoped_memory_error_v29());
    };
    let OperationKind::Atomic(actual) = &operation.kind else {
        return Err(scoped_memory_error_v29());
    };
    let [output] = operation.results.as_slice() else {
        return Err(scoped_memory_error_v29());
    };
    let Type::Scalar(scalar) = &output.ty else {
        return Err(scoped_memory_error_v29());
    };
    if row.source
        != Some(ScopedMemoryFrameV29::operand(
            site,
            Some(ExecutionOperandV29::AtomicAddress),
        ))
        || semantic_operand_type(original.value()) != ty
        || actual.pointer != pointer
        || actual.value != Some(value)
        || output.id != result
        || ScopedAtomicEffectV1::from_operation(actual) != Some(effect)
        || !effect.matches_source(original, *scalar, effect.access)
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}
