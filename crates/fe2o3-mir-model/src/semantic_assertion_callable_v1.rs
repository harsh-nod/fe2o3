// Representation-neutral callable closure and non-authority structural profiles.
pub const MAX_SEMANTIC_CALLABLE_FUNCTIONS_V1: usize = 4_096;
pub const MAX_SEMANTIC_CALLABLE_EDGES_V1: usize = 65_536;
pub const MAX_SEMANTIC_CALLABLE_WORK_V1: usize = 16_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCallableDecisionV1 {
    ExactEmptyDeterministicScalar,
    ExactEmptyOnly,
    Rejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DefinedCallableEmptyEffectDecisionV1 {
    Unknown,
    ExactEmptyDeterministicScalar,
    ExactEmptyOnly,
    Rejected,
}

pub struct SemanticDefinedCallableSummariesV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    decisions: Vec<DefinedCallableEmptyEffectDecisionV1>,
    resources: SemanticAssertionResourceObservationV1,
}
impl<'a> SemanticDefinedCallableSummariesV1<'a> {
    pub fn types(&self) -> &'a [SemanticTypeDeclV1] {
        self.types
    }
    pub fn functions(&self) -> &'a [SemanticFunctionDeclV1] {
        self.functions
    }
    pub fn callables(&self) -> &'a [SemanticCallableDeclV1] {
        self.callables
    }
    pub fn decision(&self, function: SemanticFunctionIdV1) -> Option<SemanticCallableDecisionV1> {
        Some(match self.decisions.get(function.index() as usize)? {
            DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar => {
                SemanticCallableDecisionV1::ExactEmptyDeterministicScalar
            }
            DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly => {
                SemanticCallableDecisionV1::ExactEmptyOnly
            }
            DefinedCallableEmptyEffectDecisionV1::Rejected
            | DefinedCallableEmptyEffectDecisionV1::Unknown => SemanticCallableDecisionV1::Rejected,
        })
    }
    pub const fn resources(&self) -> SemanticAssertionResourceObservationV1 {
        self.resources
    }
}
struct DefinedCallableDirectSummaryV1 {
    empty_eligible: bool,
    deterministic_scalar_eligible: bool,
    callees: Vec<usize>,
}
/// Structural classification only, never assertion-success or owner evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticCallableStructuralProfileV1 {
    empty_eligible: bool,
    deterministic_scalar_eligible: bool,
}
type DefinedCallableTerminatorEligibilityV1 = SemanticCallableStructuralProfileV1;
impl SemanticCallableStructuralProfileV1 {
    const fn shared(eligible: bool) -> Self {
        Self {
            empty_eligible: eligible,
            deterministic_scalar_eligible: eligible,
        }
    }
    pub const fn empty_eligible(self) -> bool {
        self.empty_eligible
    }
    pub const fn deterministic_scalar_eligible(self) -> bool {
        self.deterministic_scalar_eligible
    }
}
struct CallableWork<'m, M> {
    budget: Budget,
    meter: &'m mut M,
    logical: usize,
}
impl<M: SemanticAssertionMeterV1> CallableWork<'_, M> {
    fn paid(&mut self) -> Metered<'_, '_, M> {
        self.budget.metered(self.meter)
    }
}
impl<M: SemanticAssertionMeterV1> SemanticAssertionMeterV1 for CallableWork<'_, M> {
    type Error = ErrorFor<M>;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.paid().charge(amount)
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.paid().reserve(bytes)
    }
}
fn flatten_error<M: SemanticAssertionMeterV1>(
    error: SemanticAssertionMeteredErrorV1<ErrorFor<M>>,
) -> ErrorFor<M> {
    match error {
        SemanticAssertionMeteredErrorV1::Analysis(error) => error.into(),
        SemanticAssertionMeteredErrorV1::Meter(error) => error,
    }
}
fn charge_defined_callable_summary_work_v1<M: SemanticAssertionMeterV1>(
    work: &mut CallableWork<'_, M>,
    amount: usize,
) -> MR<(), M> {
    work.logical = work
        .logical
        .checked_add(amount)
        .ok_or(SemanticAssertionErrorV1::Arithmetic)?;
    work.paid().charge(amount)?;
    if work.logical > MAX_SEMANTIC_CALLABLE_WORK_V1 {
        return Err(SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary work exceeded its production limit",
        )
        .into());
    }
    Ok(())
}
fn zero_sized_defined_callable_type_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    work: &mut CallableWork<'_, M>,
) -> MR<bool, M> {
    fn visit<M: SemanticAssertionMeterV1>(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        state: &mut [u8],
        work: &mut CallableWork<'_, M>,
    ) -> MR<bool, M> {
        work.paid().charge(1)?;
        let index = ty.index() as usize;
        let Some(&status) = state.get(index) else {
            return Ok(false);
        };
        if status == 2 {
            return Ok(true);
        }
        if status == 1 {
            return Ok(false);
        }
        state[index] = 1;
        let declaration = &types[index];
        if declaration.layout().size_bytes() != Some(0) {
            return Ok(false);
        }
        let pointer_free = match declaration.shape() {
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Never => true,
            SemanticTypeShapeV1::Array { element, .. } => visit(types, *element, state, work)?,
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let mut result = true;
                for field in fields.fields() {
                    if !visit(types, *field, state, work)? {
                        result = false;
                        break;
                    }
                }
                result
            }
            SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_)
            | SemanticTypeShapeV1::Pointer(_)
            | SemanticTypeShapeV1::Slice { .. }
            | SemanticTypeShapeV1::Enum { .. }
            | SemanticTypeShapeV1::Union(_)
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Opaque => false,
        };
        state[index] = if pointer_free { 2 } else { 0 };
        Ok(pointer_free)
    }
    let mut state = work.paid().table(types.len(), 0_u8)?;
    visit(types, ty, &mut state, work)
}
fn zero_sized_direct_defined_callable_abi_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work: &mut CallableWork<'_, M>,
) -> MR<bool, M> {
    let abi = function.abi();
    work.paid().charge(1)?;
    if abi.c_variadic() || !abi.hidden_arguments().is_empty() {
        return Ok(false);
    }
    for argument in abi.arguments() {
        work.paid().charge(1)?;
        let value = argument.value();
        if !zero_sized_defined_callable_type_v1(types, value.source_ty(), work)?
            || value.adjusted().is_some()
            || value.pointee_override().is_some()
            || !matches!(value.mode(), SemanticAbiPassModeV1::Ignore)
        {
            return Ok(false);
        }
    }
    let value = abi.return_value();
    if !zero_sized_defined_callable_type_v1(types, value.source_ty(), work)?
        || value.adjusted().is_some()
        || value.pointee_override().is_some()
        || !matches!(value.mode(), SemanticAbiPassModeV1::Ignore)
    {
        return Ok(false);
    }
    for &ty in abi.source_input_types() {
        if !zero_sized_defined_callable_type_v1(types, ty, work)? {
            return Ok(false);
        }
    }
    zero_sized_defined_callable_type_v1(types, abi.source_output_type(), work)
}
fn scalar_direct_defined_callable_abi_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work: &mut CallableWork<'_, M>,
) -> MR<bool, M> {
    let abi = function.abi();
    work.paid()
        .charge(sum(abi.arguments().len(), abi.source_input_types().len())?)?;
    let scalar_abi = !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi
            .arguments()
            .iter()
            .all(|argument| scalar_direct_abi_value_v1(types, argument.value()))
        && scalar_direct_abi_value_v1(types, abi.return_value())
        && abi
            .source_input_types()
            .iter()
            .copied()
            .all(|ty| scalar_defined_callable_type_v1(types, ty))
        && scalar_defined_callable_type_v1(types, abi.source_output_type());
    let mut has_carrier = false;
    for &ty in abi.source_input_types() {
        if transparent_carrier(types, ty, work)?.is_some() {
            has_carrier = true;
            break;
        }
    }
    let mut carrier_abi = has_carrier
        && !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi.arguments().len() == abi.source_input_types().len()
        && abi.arguments().len() == abi.source_argument_ownership().len();
    if carrier_abi {
        for argument in abi.arguments() {
            work.paid().charge(1)?;
            if argument.role() != SemanticAbiArgumentRoleV1::Source {
                carrier_abi = false;
                break;
            }
        }
    }
    if carrier_abi {
        for ((argument, &ty), &ownership) in abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .zip(abi.source_argument_ownership())
        {
            work.paid().charge(1)?;
            let value = argument.value();
            let eligible = scalar_direct_abi_value_v1(types, value)
                || (ownership == SemanticSourceArgumentOwnershipV1::ByValue
                    && transparent_carrier(types, ty, work)?.is_some()
                    && value.source_ty() == ty
                    && value.adjusted().is_none()
                    && value.pointee_override().is_none()
                    && matches!(value.mode(), SemanticAbiPassModeV1::Direct(_)));
            if !eligible {
                carrier_abi = false;
                break;
            }
        }
    }
    carrier_abi &= scalar_direct_abi_value_v1(types, abi.return_value());
    if carrier_abi {
        for (&ty, &ownership) in abi
            .source_input_types()
            .iter()
            .zip(abi.source_argument_ownership())
        {
            work.paid().charge(1)?;
            if !(scalar_defined_callable_type_v1(types, ty)
                || (ownership == SemanticSourceArgumentOwnershipV1::ByValue
                    && transparent_carrier(types, ty, work)?.is_some()))
            {
                carrier_abi = false;
                break;
            }
        }
    }
    carrier_abi &= scalar_defined_callable_type_v1(types, abi.source_output_type());
    Ok(scalar_abi || carrier_abi)
}

impl<M: SemanticAssertionMeterV1> crate::semantic_scalar_carrier_v1::ScalarCarrierMeterV1
    for CallableWork<'_, M>
{
    type Error = ErrorFor<M>;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.paid().charge(amount)
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.paid().reserve(bytes)
    }
}
fn transparent_carrier<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    work: &mut CallableWork<'_, M>,
) -> MR<Option<SemanticTypeIdV1>, M> {
    use crate::semantic_scalar_carrier_v1::{
        ScalarCarrierErrorV1, exact_transparent_scalar_carrier_field_metered_v1,
    };
    exact_transparent_scalar_carrier_field_metered_v1(types, ty, work).map_err(
        |error| match error {
            ScalarCarrierErrorV1::Meter(error) => error,
            ScalarCarrierErrorV1::Arithmetic => SemanticAssertionErrorV1::Arithmetic.into(),
            ScalarCarrierErrorV1::Allocation => SemanticAssertionErrorV1::Allocation.into(),
        },
    )
}
/// Non-authority structural queries. Positive profile results never prove an
/// assertion, and no method accepts caller-provided assertion success.
pub struct SemanticCallableProfileQueriesV1<'a, 'm, M> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    work: CallableWork<'m, M>,
}
impl<'a, 'm, M: SemanticAssertionMeterV1> SemanticCallableProfileQueriesV1<'a, 'm, M> {
    pub fn new_metered(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
        limits: SemanticAssertionLimitsV1,
        meter: &'m mut M,
    ) -> MR<Self, M> {
        Ok(Self {
            types,
            function,
            work: CallableWork {
                budget: Budget::new(limits)?,
                meter,
                logical: 0,
            },
        })
    }
    pub fn logical_visits(&self) -> usize {
        self.work.logical
    }
    pub fn scalar_type(&mut self, ty: SemanticTypeIdV1) -> MR<bool, M> {
        self.work.paid().charge(1)?;
        Ok(scalar_defined_callable_type_v1(self.types, ty))
    }
    pub fn checked_carrier_type(
        &mut self,
        ty: SemanticTypeIdV1,
    ) -> MR<Option<(SemanticTypeIdV1, SemanticTypeIdV1)>, M> {
        self.work.paid().charge(3)?;
        Ok(checked_scalar_carrier_type_v1(self.types, ty))
    }
    pub fn zero_sized_type(&mut self, ty: SemanticTypeIdV1) -> MR<bool, M> {
        zero_sized_defined_callable_type_v1(self.types, ty, &mut self.work)
    }
    pub fn scalar_abi(&mut self) -> MR<bool, M> {
        scalar_direct_defined_callable_abi_v1(self.types, self.function, &mut self.work)
    }
    pub fn scalar_place(&mut self, place: &SemanticPlaceV1) -> MR<bool, M> {
        scalar_defined_callable_place_v1(self.types, self.function, place, &mut self.work)
    }
    pub fn scalar_operand(&mut self, operand: &SemanticOperandV1) -> MR<bool, M> {
        scalar_defined_callable_operand_v1(self.types, self.function, operand, &mut self.work)
    }
    pub fn scalar_destination(&mut self, place: &SemanticPlaceV1) -> MR<bool, M> {
        scalar_defined_callable_destination_v1(self.types, self.function, place, &mut self.work)
    }
    pub fn carrier_destination(&mut self, place: &SemanticPlaceV1) -> MR<bool, M> {
        scalar_or_checked_carrier_destination_v1(self.types, self.function, place, &mut self.work)
    }
    pub fn scalar_rvalue(&mut self, value: &SemanticRvalueV1) -> MR<bool, M> {
        scalar_defined_callable_rvalue_v1(self.types, self.function, value, &mut self.work)
    }
    pub fn terminator_profile(
        &mut self,
        function_count: usize,
        callables: &[SemanticCallableDeclV1],
        terminator: &SemanticTerminatorKindV1,
    ) -> MR<SemanticCallableStructuralProfileV1, M> {
        let mut callees = Vec::new();
        scalar_defined_callable_terminator_v1(
            self.types,
            self.function,
            function_count,
            callables,
            terminator,
            &mut callees,
            &mut 0,
            &mut self.work,
        )
    }
}

pub fn semantic_zero_sized_type_profile_metered_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> MR<bool, M> {
    zero_sized_defined_callable_type_v1(
        types,
        ty,
        &mut CallableWork {
            budget: Budget::new(limits)?,
            meter,
            logical: 0,
        },
    )
}
impl<'a> SemanticDefinedCallableSummariesV1<'a> {
    pub fn new_metered<M: SemanticAssertionMeterV1>(
        types: &'a [SemanticTypeDeclV1],
        functions: &'a [SemanticFunctionDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        limits: SemanticAssertionLimitsV1,
        meter: &mut M,
    ) -> MR<Self, M> {
        semantic_defined_callable_summaries_metered_v1(types, functions, callables, limits, meter)
    }
}

fn direct_defined_callable_summary_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_count: usize,
    callables: &[SemanticCallableDeclV1],
    call_edges: &mut usize,
    work: &mut CallableWork<'_, M>,
) -> Result<DefinedCallableDirectSummaryV1, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, function.locals().len())?)?;
    let mut scalar_base_eligible = scalar_direct_defined_callable_abi_v1(types, function, work)?;
    if scalar_base_eligible {
        for local in function.locals() {
            work.paid().charge(1)?;
            if !(scalar_or_checked_carrier_type_v1(types, local.ty())
                || transparent_carrier(types, local.ty(), work)?.is_some())
            {
                scalar_base_eligible = false;
                break;
            }
        }
    }
    let mut zero_sized_base_eligible =
        zero_sized_direct_defined_callable_abi_v1(types, function, work)?;
    if zero_sized_base_eligible {
        for local in function.locals() {
            if !zero_sized_defined_callable_type_v1(types, local.ty(), work)? {
                zero_sized_base_eligible = false;
                break;
            }
        }
    }
    work.paid().charge(function.blocks().len())?;
    let base_eligible = scalar_base_eligible || zero_sized_base_eligible;
    let mut empty_eligible = base_eligible;
    let mut deterministic_scalar_eligible = scalar_base_eligible;
    let assert_proofs = if scalar_base_eligible
        && function.blocks().iter().any(|block| {
            matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            )
        }) {
        let mut analysis = SemanticAssertionAnalysisV1::new_metered(
            types,
            function,
            SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX),
            work,
        )
        .map_err(flatten_error::<M>)?;
        Some(
            analysis
                .legacy_recipe_assertions_v1(true, work)
                .map_err(flatten_error::<M>)?,
        )
    } else {
        None
    };
    let mut callees = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        charge_defined_callable_summary_work_v1(work, 1)?;
        for statement in block.statements() {
            let statement_eligible = if scalar_base_eligible {
                scalar_defined_callable_statement_v1(types, function, statement, work)?
            } else {
                zero_sized_defined_callable_statement_v1(types, function, statement, work)?
            };
            empty_eligible &= statement_eligible;
            deterministic_scalar_eligible &=
                statement_eligible && deterministic_scalar_defined_callable_statement_v1(statement);
        }
        let assertion_ok = !matches!(
            block.terminator().kind(),
            SemanticTerminatorKindV1::Assert { .. }
        ) || assert_proofs
            .as_deref()
            .and_then(|proved| proved.get(block_index))
            == Some(&SemanticAssertionLegacyDispositionV1::Proved);
        let terminator_eligibility = if scalar_base_eligible && assertion_ok {
            scalar_defined_callable_terminator_v1(
                types,
                function,
                function_count,
                callables,
                block.terminator().kind(),
                &mut callees,
                call_edges,
                work,
            )?
        } else {
            charge_defined_callable_summary_work_v1(work, 1)?;
            DefinedCallableTerminatorEligibilityV1::shared(matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Goto(_)
                    | SemanticTerminatorKindV1::Return
                    | SemanticTerminatorKindV1::Unreachable
            ))
        };
        empty_eligible &= terminator_eligibility.empty_eligible;
        deterministic_scalar_eligible &= terminator_eligibility.deterministic_scalar_eligible;
    }
    Ok(DefinedCallableDirectSummaryV1 {
        empty_eligible,
        deterministic_scalar_eligible,
        callees,
    })
}

pub fn semantic_defined_callable_summaries_metered_v1<'a, M: SemanticAssertionMeterV1>(
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    limits: SemanticAssertionLimitsV1,
    meter: &mut M,
) -> Result<SemanticDefinedCallableSummariesV1<'a>, ErrorFor<M>> {
    if functions.len() > MAX_SEMANTIC_CALLABLE_FUNCTIONS_V1 {
        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary function count exceeded its production limit",
        )));
    }
    let mut work = CallableWork {
        budget: Budget::new(limits)?,
        meter,
        logical: 0,
    };
    work.paid()
        .reserve(size_of::<SemanticDefinedCallableSummariesV1<'a>>())?;
    let mut direct = Vec::new();
    work.paid().grow(&mut direct, functions.len())?;
    let mut call_edges = 0_usize;
    for function in functions {
        direct.push(direct_defined_callable_summary_v1(
            types,
            function,
            functions.len(),
            callables,
            &mut call_edges,
            &mut work,
        )?);
    }
    let mut callers = Vec::new();
    work.paid().grow(&mut callers, functions.len())?;
    work.paid().charge(functions.len())?;
    callers.resize_with(functions.len(), Vec::new);
    for (caller, summary) in direct.iter().enumerate() {
        work.paid().charge(sum(summary.callees.len(), 1)?)?;
        for &callee in &summary.callees {
            let Some(reverse) = callers.get_mut(callee) else {
                return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                    "defined-callable effect summary references a missing function",
                )));
            };
            work.paid().grow(reverse, 1)?;
            reverse.push(caller);
        }
    }

    let mut decisions = work.paid().table(
        functions.len(),
        DefinedCallableEmptyEffectDecisionV1::Unknown,
    )?;
    let mut remaining = work.paid().backing(functions.len())?;
    work.paid().charge(direct.len())?;
    remaining.extend(direct.iter().map(|summary| summary.callees.len()));
    let mut all_callees_deterministic = work.paid().table(functions.len(), true)?;
    let mut pending = work.paid().deque(functions.len())?;
    for (function, summary) in direct.iter().enumerate() {
        work.paid().charge(1)?;
        if !summary.empty_eligible {
            decisions[function] = DefinedCallableEmptyEffectDecisionV1::Rejected;
            pending.push_back(function);
        } else if remaining[function] == 0 {
            decisions[function] = if summary.deterministic_scalar_eligible {
                DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar
            } else {
                DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
            };
            pending.push_back(function);
        }
    }
    while let Some(callee) = pending.pop_front() {
        charge_defined_callable_summary_work_v1(&mut work, sum(1, callers[callee].len())?)?;
        for &caller in &callers[callee] {
            if decisions[caller] != DefinedCallableEmptyEffectDecisionV1::Unknown {
                continue;
            }
            remaining[caller] = remaining[caller].checked_sub(1).ok_or(analysis_error::<M>(
                SemanticAssertionErrorV1::Unsupported(
                    "defined-callable effect-summary dependency accounting underflowed",
                ),
            ))?;
            if decisions[callee] == DefinedCallableEmptyEffectDecisionV1::Rejected {
                decisions[caller] = DefinedCallableEmptyEffectDecisionV1::Rejected;
                pending.push_back(caller);
            } else {
                all_callees_deterministic[caller] &= decisions[callee]
                    == DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar;
                if remaining[caller] == 0 {
                    decisions[caller] = if direct[caller].deterministic_scalar_eligible
                        && all_callees_deterministic[caller]
                    {
                        DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar
                    } else {
                        DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
                    };
                    pending.push_back(caller);
                }
            }
        }
    }
    work.paid().charge(decisions.len())?;
    for decision in &mut decisions {
        if *decision == DefinedCallableEmptyEffectDecisionV1::Unknown {
            // The unresolved subgraph consists of recursive cycles and callers
            // that depend on them. Recursion has no production call-stack proof.
            *decision = DefinedCallableEmptyEffectDecisionV1::Rejected;
        }
    }
    Ok(SemanticDefinedCallableSummariesV1 {
        types,
        functions,
        callables,
        decisions,
        resources: work.budget.observation(),
    })
}
