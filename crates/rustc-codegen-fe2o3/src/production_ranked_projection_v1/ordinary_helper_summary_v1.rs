// This branch describes source value transport only. It never authorizes
// deterministic call results, pointee effects, or an executable call omission.
fn ordinary_helper_transport_query_v1(
    work: &mut usize,
    query: impl FnOnce(
        &mut dyn FnMut(usize) -> Result<(), fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1>,
    ) -> Result<(), fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1>,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    use fe2o3_lower_mir_kernel::{
        ProductionSemanticKirErrorV1 as Error, ProductionSemanticKirResourceV1 as Resource,
    };
    let mut original_failure = None;
    let result = query(&mut |amount| {
        if original_failure.is_some() {
            return Err(Error::ResourceLimit {
                resource: Resource::AnalysisWork,
                actual: usize::MAX,
                limit: MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1,
            });
        }
        let actual = work.checked_add(amount).unwrap_or(usize::MAX);
        charge_defined_callable_summary_work_v1(work, amount).map_err(|error| {
            original_failure = Some(error);
            Error::ResourceLimit {
                resource: Resource::AnalysisWork,
                actual,
                limit: MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1,
            }
        })
    });
    if let Some(error) = original_failure {
        return Err(error);
    }
    match result {
        Ok(()) => Ok(true),
        Err(Error::Unsupported { .. } | Error::ScalarTypeUnavailable { .. }) => Ok(false),
        Err(error) => Err(ProductionRankedProjectionErrorV1::StructuralValidation(
            error,
        )),
    }
}

struct OrdinaryHelperSummaryV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    function_count: usize,
    callables: &'a [SemanticCallableDeclV1],
}

impl OrdinaryHelperSummaryV1<'_> {
    fn place(
        &self,
        place: &SemanticPlaceV1,
        destination: bool,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1 + place.projections().len())?;
        let Some(local) = self.function.locals().get(place.local().index() as usize) else {
            return Ok(false);
        };
        if destination && !place.projections().is_empty() {
            return Ok(false);
        }
        let mut current = local.ty();
        for projection in place.projections() {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Ok(false);
            };
            let Some(SemanticTypeShapeV1::Tuple(fields)) = self
                .types
                .get(current.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(false);
            };
            let Some(next) = fields.fields().get(field as usize).copied() else {
                return Ok(false);
            };
            if projection.result_type() != next {
                return Ok(false);
            }
            current = next;
        }
        Ok(current == place.ty())
    }

    fn operand(
        &self,
        operand: &SemanticOperandV1,
        scalar_only: bool,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        if scalar_only && !scalar_defined_callable_type_v1(self.types, operand.ty()) {
            return Ok(false);
        }
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(place, false, work)
            }
            SemanticOperandV1::Constant(constant) => {
                Ok(scalar_defined_callable_type_v1(self.types, constant.ty())
                    && matches!(
                        constant.value(),
                        SemanticConstantValueV1::Scalar(_) | SemanticConstantValueV1::ZeroSized
                    ))
            }
        }
    }

    fn rvalue(
        &self,
        value: &SemanticRvalueV1,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        let scalar = scalar_defined_callable_type_v1(self.types, value.result_type());
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                Ok(operand.ty() == value.result_type() && self.operand(operand, false, work)?)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                charge_defined_callable_summary_work_v1(work, 1 + aggregate.operands().len())?;
                match (
                    aggregate.kind(),
                    self.types
                        .get(value.result_type().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                ) {
                    (
                        SemanticAggregateKindV1::Array,
                        Some(SemanticTypeShapeV1::Array { element, length }),
                    ) => {
                        if usize::try_from(*length).ok() != Some(aggregate.operands().len()) {
                            return Ok(false);
                        }
                        for operand in aggregate.operands() {
                            if operand.ty() != *element || !self.operand(operand, false, work)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    (SemanticAggregateKindV1::Tuple, Some(SemanticTypeShapeV1::Tuple(fields))) => {
                        if fields.fields().len() != aggregate.operands().len() {
                            return Ok(false);
                        }
                        for (operand, ty) in aggregate.operands().iter().zip(fields.fields()) {
                            if operand.ty() != *ty || !self.operand(operand, false, work)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer | SemanticCastKindV1::Float,
                operand,
            } => Ok(scalar && self.operand(operand, true, work)?),
            SemanticRvalueKindV1::Binary { left, right, .. } => {
                Ok(scalar && self.operand(left, true, work)? && self.operand(right, true, work)?)
            }
            SemanticRvalueKindV1::CheckedBinary(checked) => {
                Ok(
                    checked_scalar_carrier_type_v1(self.types, value.result_type()).is_some_and(
                        |(result, _)| {
                            result == checked.left().ty() && result == checked.right().ty()
                        },
                    ) && self.operand(checked.left(), true, work)?
                        && self.operand(checked.right(), true, work)?,
                )
            }
            SemanticRvalueKindV1::UncheckedBinary(unchecked) => Ok(scalar
                && self.operand(unchecked.left(), true, work)?
                && self.operand(unchecked.right(), true, work)?),
            SemanticRvalueKindV1::Cast { .. }
            | SemanticRvalueKindV1::Borrow { .. }
            | SemanticRvalueKindV1::AddressOf { .. }
            | SemanticRvalueKindV1::Length(_)
            | SemanticRvalueKindV1::Discriminant(_)
            | SemanticRvalueKindV1::Load(_) => Ok(false),
        }
    }

    fn statement(
        &self,
        statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => Ok(assignment.destination().ty()
                == assignment.value().result_type()
                && self.place(assignment.destination(), true, work)?
                && self.rvalue(assignment.value(), work)?),
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                Ok(self.function.locals().get(local.index() as usize).is_some())
            }
            SemanticStatementKindV1::Assume(condition) => self.operand(condition, true, work),
            SemanticStatementKindV1::Nop => Ok(true),
            SemanticStatementKindV1::Store(_)
            | SemanticStatementKindV1::AtomicRmw(_)
            | SemanticStatementKindV1::AtomicCompareExchange(_)
            | SemanticStatementKindV1::SetDiscriminant { .. }
            | SemanticStatementKindV1::Deinitialize(_) => Ok(false),
        }
    }

    fn call(
        &self,
        call: &SemanticDirectCallV1,
        callees: &mut Vec<usize>,
        call_edges: &mut usize,
        work: &mut usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        charge_defined_callable_summary_work_v1(work, 1 + call.arguments().len())?;
        let mut eligible = call.variadic_argument_abis().is_empty()
            && matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            );
        for argument in call.arguments() {
            eligible &= self.operand(argument, true, work)?;
        }
        if let Some(destination) = call.destination() {
            eligible &= self.place(destination.place(), true, work)?;
        }
        match self.callables.get(call.callee().index() as usize) {
            Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => Ok(
                scalar_defined_callable_intrinsic_eligibility_v1(*operation, eligible)
                    .empty_eligible,
            ),
            Some(SemanticCallableDeclV1::Defined { function }) => {
                let callee = function.index() as usize;
                if callee >= self.function_count {
                    return Ok(false);
                }
                *call_edges = call_edges.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "defined-callable effect-summary edge count overflowed",
                    ),
                )?;
                if *call_edges > MAX_DEFINED_CALLABLE_SUMMARY_EDGES_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "defined-callable effect-summary edge count exceeded its production limit",
                    ));
                }
                callees.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "defined-callable effect-summary edge storage cannot be reserved",
                    )
                })?;
                callees.push(callee);
                Ok(eligible)
            }
            _ => Ok(false),
        }
    }
}

fn ordinary_direct_defined_callable_summary_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_count: usize,
    callables: &[SemanticCallableDeclV1],
    call_edges: &mut usize,
    work: &mut usize,
) -> Result<Option<DefinedCallableDirectSummaryV1>, ProductionRankedProjectionErrorV1> {
    let abi = function.abi();
    let argument_work = abi
        .arguments()
        .len()
        .checked_mul(4)
        .and_then(|count| count.checked_add(1))
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "defined-callable effect-summary work overflowed",
        ))?;
    charge_defined_callable_summary_work_v1(work, argument_work)?;
    if abi.arguments().len() != abi.source_input_types().len()
        || abi.arguments().len() != abi.source_argument_ownership().len()
        || !abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .zip(abi.source_argument_ownership())
            .all(|((argument, ty), ownership)| {
                argument.role() == SemanticAbiArgumentRoleV1::Source
                    && argument.value().source_ty() == *ty
                    && *ownership == SemanticSourceArgumentOwnershipV1::ByValue
                    && scalar_direct_abi_value_v1(types, argument.value())
            })
    {
        return Ok(None);
    }
    if !ordinary_helper_transport_query_v1(work, |charge| {
        fe2o3_lower_mir_kernel::check_ordinary_helper_result_transport_v1(
            types,
            function,
            charge,
            MAX_DEFINED_CALLABLE_SUMMARY_WORK_V1,
        )
    })? {
        return Ok(None);
    }
    for local in function.locals() {
        charge_defined_callable_summary_work_v1(work, 1)?;
        if local.ty() != abi.source_output_type()
            && !scalar_or_checked_carrier_type_v1(types, local.ty())
            && !ordinary_helper_transport_query_v1(work, |charge| {
                fe2o3_lower_mir_kernel::check_ordinary_helper_value_type_v1(
                    types,
                    local.ty(),
                    charge,
                )
            })?
        {
            return Ok(None);
        }
    }
    // No new storage-observable cause is admitted: every destination is whole,
    // value projections are tuple fields, and all address/memory/drop effects
    // are rejected. Existing source SSA therefore retains ordinary phi values,
    // not private slots. The executable CFG and calls are not changed here.
    charge_defined_callable_summary_work_v1(work, function.blocks().len())?;
    let assert_proofs = if function.blocks().iter().any(|block| {
        matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Assert { .. }
        )
    }) {
        Some(SemanticAssertProofsV1::analyze_defined_callable_asserts_v1(
            types, function,
        )?)
    } else {
        None
    };
    let summary = OrdinaryHelperSummaryV1 {
        types,
        function,
        function_count,
        callables,
    };
    let mut empty_eligible = true;
    let mut callees = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        charge_defined_callable_summary_work_v1(work, 1)?;
        for statement in block.statements() {
            empty_eligible &= summary.statement(statement, work)?;
        }
        let eligible = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                summary.call(call, &mut callees, call_edges, work)?
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                charge_defined_callable_summary_work_v1(work, 1)?;
                summary.operand(discriminant, true, work)?
            }
            SemanticTerminatorKindV1::TailCall(_) => {
                charge_defined_callable_summary_work_v1(work, 1)?;
                false
            }
            terminator => {
                scalar_defined_callable_terminator_v1(
                    types,
                    function,
                    function_count,
                    callables,
                    assert_proofs
                        .as_deref()
                        .and_then(|proofs| proofs.get(block_index))
                        .copied()
                        .unwrap_or(false),
                    terminator,
                    &mut callees,
                    call_edges,
                    work,
                )?
                .empty_eligible
            }
        };
        empty_eligible &= eligible;
    }
    Ok(Some(DefinedCallableDirectSummaryV1 {
        empty_eligible,
        deterministic_scalar_eligible: false,
        callees,
    }))
}
