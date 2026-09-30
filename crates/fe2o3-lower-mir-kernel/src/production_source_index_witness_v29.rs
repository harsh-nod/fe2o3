// A checked shared borrow can carry an index's scalar value, but cannot issue
// index authority. This receipt keeps its original recipe across SSA transport.
#[cfg(test)]
type SourceIndexBorrowObserverV29 =
    fn(&mut SemanticFunctionLoweringV1<'_, '_>, SemanticBlockIdV1, Option<u32>, usize);

#[cfg(test)]
type SourceIndexReaderObserverV29 = fn(
    &SourceReferenceEmissionV29<'_, '_>,
    &SemanticSourceReferenceBindingV29,
    (SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool),
    &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SOURCE_INDEX_BORROW_OBSERVER_V29: std::cell::Cell<Option<SourceIndexBorrowObserverV29>> = const { std::cell::Cell::new(None) };
    static SOURCE_INDEX_READER_OBSERVER_V29: std::cell::Cell<Option<SourceIndexReaderObserverV29>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIndexWitnessBorrowV29 {
    site: SourceReferenceSiteV29,
    ty: SemanticTypeIdV1,
    origin: usize,
    index_space: SemanticDisjointIndexSpaceV1,
    disjoint: bool,
}

fn source_index_witness_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source index reader differs from its original witness loan")
}

// Representation only: a scalar-shaped Rust wrapper is not sufficient. The
// original compiler callable roster must declare an index-witness producer.
fn source_index_witness_type_v29(
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    for callable in callables {
        budget.charge_work(3)?;
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        if direct_index_witness_contract_v1(operation, ty).is_some()
            || matches!(operation,
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { output_witness, .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { output_witness, .. }
                    if *output_witness == ty)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

// Legacy origin analysis treats every address-taking operation as a possible
// mutation. C1 can discharge exactly an original immutable whole-witness loan;
// this does not clear invalidation caused by any other operation on the local.
fn source_shared_index_borrow_preserves_origin_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    site: SourceReferenceSiteV29,
    statement: &SemanticStatementKindV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    let plan = references.plan;
    source_reference_owned_prepay_v29::<bool>(plan, budget)?;
    source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<usize>>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 8)?;
    let original = plan
        .instances
        .instance(site.instance)
        .ok_or_else(source_index_witness_error_v29)?
        .declaration();
    let actual = site
        .statement
        .and_then(|index| {
            original
                .blocks()
                .get(site.block.index() as usize)
                .and_then(|block| block.statements().get(index))
        })
        .ok_or_else(source_index_witness_error_v29)?;
    if !std::ptr::eq(actual.kind(), statement) {
        return Err(source_index_witness_error_v29());
    }
    let SemanticStatementKindV1::Assign(assignment) = statement else {
        return Ok(false);
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return Ok(false);
    };
    let source = plan.instances.owner().source_semantic();
    if !place.projections().is_empty()
        || original
            .locals()
            .get(place.local().index() as usize)
            .is_none_or(|local| local.ty() != place.ty())
        || !matches!(source.types().get(assignment.value().result_type().index() as usize)
            .map(|ty| ty.shape()), Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.pointee() == place.ty())
        || !source_index_witness_type_v29(source.callables(), place.ty(), budget)?
    {
        return Ok(false);
    }
    let Some(index) = references.loan_at(site, budget)? else {
        return Ok(false);
    };
    let loan = plan
        .loans
        .get(index)
        .ok_or_else(source_index_witness_error_v29)?;
    let origin = plan
        .origins
        .get(loan.origin)
        .ok_or_else(source_index_witness_error_v29)?;
    if loan.kind != SemanticBorrowKindV1::Shared
        || loan.source_type != assignment.value().result_type()
        || loan.parent.is_some()
        || loan.representation != SourceReferenceRepresentationV29::StableReferent
        || origin.instance != site.instance
        || origin.local != place.local()
        || origin.ty != place.ty()
        || !origin.projections.is_empty()
    {
        return Err(source_index_witness_error_v29());
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn source_reference_index_witness_values_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    carriers: &ExecutionCfgCarriersV29,
    place: &SemanticPlaceV1,
    loan: usize,
    referent: &SemanticValueBindingV1,
    values: &mut Vec<ValueDef>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceIndexWitnessBorrowV29>, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    let references = cursor
        .references
        .ok_or_else(source_index_witness_error_v29)?;
    references.check(budget)?;
    source_reference_owned_prepay_v29::<Option<SourceIndexWitnessBorrowV29>>(
        references.plan,
        budget,
    )?;
    let Some(carrier) = carriers.lookup(place.local().index(), budget)? else {
        return Ok(None);
    };
    let SemanticPromotedBindingV1::IndexWitness {
        index_space,
        disjoint,
        ..
    } = carrier.binding
    else {
        return Ok(None);
    };
    carriers.check_owner(cursor, budget)?;
    let plan = references.plan;
    budget.source_reference_charge_v29(plan, 14)?;
    let record = plan
        .loans
        .get(loan)
        .ok_or_else(source_index_witness_error_v29)?;
    let origin = plan
        .origins
        .get(record.origin)
        .ok_or_else(source_index_witness_error_v29)?;
    if record.kind != SemanticBorrowKindV1::Shared
        || record.site.instance != cursor.instance
        || record.representation != SourceReferenceRepresentationV29::StableReferent
        || origin.instance != cursor.instance
        || origin.local != place.local()
        || origin.ty != carrier.source_type
        || place.ty() != carrier.source_type
        || !place.projections().is_empty()
        || !origin.projections.is_empty()
        || !values.is_empty()
    {
        return Err(source_index_witness_error_v29());
    }
    // This validates the actual binding's complete provenance, including its
    // option-edge availability, against the recipe from the original source.
    source_reference_owned_prepay_v29::<ExecutionCfgCarrierValuesV29<'_, '_>>(plan, budget)?;
    source_reference_owned_prepay_v29::<()>(plan, budget)?;
    execution_cfg_carrier_values_v29(carrier, referent, values, budget)?;
    if values.len() != 1 || values[0].ty != Type::INDEX {
        return Err(source_index_witness_error_v29());
    }
    Ok(Some(SourceIndexWitnessBorrowV29 {
        site: record.site,
        ty: origin.ty,
        origin: record.origin,
        index_space,
        disjoint,
    }))
}

fn source_index_reader_contract_v29(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool), ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let Some(SemanticTerminatorKindV1::Call(original)) = function
        .blocks()
        .get(block.index() as usize)
        .map(|block| block.terminator().kind())
    else {
        return Err(source_index_witness_error_v29());
    };
    if !std::ptr::eq(original, call) || call.arguments().len() != 1 {
        return Err(source_index_witness_error_v29());
    }
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation, binding, ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Err(source_index_witness_error_v29());
    };
    if binding.abi().source_input_types() != [call.arguments()[0].ty()] {
        return Err(source_index_witness_error_v29());
    }
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness,
            raw_index,
        } if binding.abi().source_output_type() == *raw_index
            && call
                .destination()
                .map(|destination| destination.place().ty())
                == Some(*raw_index) =>
        {
            Ok((*index_witness, SemanticDisjointIndexSpaceV1::Index1d, false))
        }
        SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
            index_witness,
            index_space,
            raw_index,
        } if binding.abi().source_output_type() == *raw_index
            && call
                .destination()
                .map(|destination| destination.place().ty())
                == Some(*raw_index) =>
        {
            Ok((*index_witness, *index_space, true))
        }
        _ => Err(source_index_witness_error_v29()),
    }
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn index_reader_value_v29(
        &self,
        binding: &SemanticSourceReferenceBindingV29,
        source_type: SemanticTypeIdV1,
        expected: (SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool),
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let plan = self.plan;
        source_reference_owned_prepay_v29::<SourceIndexWitnessBorrowV29>(plan, budget)?;
        source_reference_validate_binding_v29(plan, binding, budget)?;
        budget.source_reference_charge_v29(plan, 18)?;
        let SourceReferenceBindingOriginV29::SingleLoan(loan) = binding.origin else {
            return Err(source_index_witness_error_v29());
        };
        let record = plan
            .loans
            .get(loan)
            .ok_or_else(source_index_witness_error_v29)?;
        let proof = self
            .index_witnesses
            .get(loan)
            .and_then(std::cell::Cell::get)
            .ok_or_else(source_index_witness_error_v29)?;
        let types = plan.instances.owner().source_semantic().types();
        if record.kind != SemanticBorrowKindV1::Shared
            || record.representation != SourceReferenceRepresentationV29::StableReferent
            || proof.site != record.site
            || proof.origin != record.origin
            || (proof.ty, proof.index_space, proof.disjoint) != expected
            || plan.origins.get(record.origin).map(|origin| origin.ty) != Some(proof.ty)
            || !self.claimed.get(loan).is_some_and(std::cell::Cell::get)
            || binding.source_type != source_type
            || record.source_type != source_type
            || !matches!(types.get(source_type.index() as usize).map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Pointer(pointer))
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.pointee() == proof.ty)
            || binding.values.len() != 1
            || binding.values[0].ty != Type::INDEX
        {
            return Err(source_index_witness_error_v29());
        }
        Ok(binding.values[0].id)
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn lower_source_index_reader_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return Ok(None);
        }
        self.with_scoped_payload_header_v29(
            argument_sum_v1(&[
                source_reference_emission_headers_v29::<Option<SemanticValueBindingV1>>()?,
                source_reference_emission_headers_v29::<(
                    SemanticTypeIdV1,
                    SemanticDisjointIndexSpaceV1,
                    bool,
                )>()?,
                source_reference_emission_headers_v29::<ValueId>()?,
            ])?,
            |this| {
                let expected = this.with_emission_budget_v1(|this, budget| {
                    let cursor = this
                        .execution
                        .as_ref()
                        .ok_or_else(source_index_witness_error_v29)?;
                    let references = cursor
                        .references
                        .ok_or_else(source_index_witness_error_v29)?;
                    references.check(budget)?;
                    budget.source_reference_charge_v29(references.plan, 4)?;
                    let original = references
                        .plan
                        .instances
                        .instance(cursor.instance)
                        .ok_or_else(source_index_witness_error_v29)?;
                    let source = references.plan.instances.owner().source_semantic();
                    if !std::ptr::eq(original.declaration(), this.function)
                        || !std::ptr::eq(source.types(), this.types)
                        || !std::ptr::eq(source.callables(), this.callables)
                    {
                        return Err(source_index_witness_error_v29());
                    }
                    source_index_reader_contract_v29(
                        this.function,
                        this.callables,
                        block,
                        call,
                        budget,
                    )
                })?;
                let operand = &call.arguments()[0];
                let value = this.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(0)),
                    operand,
                    operations,
                )?;
                let SemanticValueBindingV1::SourceReference(binding) = value else {
                    return Err(source_index_witness_error_v29());
                };
                let id = this.with_emission_budget_v1(|this, budget| {
                    let references = this
                        .execution
                        .as_ref()
                        .and_then(|cursor| cursor.references)
                        .ok_or_else(source_index_witness_error_v29)?;
                    #[cfg(test)]
                    if let Some(observer) = SOURCE_INDEX_READER_OBSERVER_V29.get() {
                        observer(references, &binding, expected, budget)?;
                    }
                    references.index_reader_value_v29(&binding, operand.ty(), expected, budget)
                })?;
                Ok(Some(SemanticValueBindingV1::Value {
                    id,
                    ty: Type::INDEX,
                }))
            },
        )
    }
}
