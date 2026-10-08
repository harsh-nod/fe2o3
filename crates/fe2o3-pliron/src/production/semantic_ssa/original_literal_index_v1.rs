//! Original-owner literal-index query. No source planner, capture, or proof row is created.
use super::adapter::emission_v1::{
    self as emission, SemanticSsaEmissionObserverV1, SemanticSsaEmissionSiteV1 as ESite,
    SemanticSsaEventBufferV1, SemanticSsaEventRoleV1 as ERole, SemanticSsaOperandRoleV1 as ORole,
    SemanticSsaVisitV1,
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantV1, SemanticConstantValueV1, SemanticScalarTypeV1,
};
use fe2o3_mir_model::{SsaEdgeIdV1, SsaResolvedEventV1, SsaValueV1};

type QueryError = ProductionSemanticSsaOccurrenceErrorV1;
type QueryResult<T> = Result<T, QueryError>;

fn charge(budget: &mut Budget<'_>, n: usize) -> QueryResult<()> {
    budget.charge_work(n).map_err(QueryError::Resource)
}
fn mismatch(function: SemanticFunctionIdV1, block: Option<SsaBlockIdV1>) -> QueryError {
    QueryError::CaptureMismatch { function, block }
}
fn arithmetic() -> QueryError {
    QueryError::Resource(Resource::Arithmetic)
}

struct Counter(usize);
impl SemanticSsaEventBufferV1 for Counter {
    type Error = QueryError;
    fn event_count(&self) -> usize {
        self.0
    }
    fn push_event(&mut self, _: SsaEventV1) -> QueryResult<()> {
        // The observer prepays this increment before every push.
        self.0 = self.0.checked_add(1).ok_or_else(arithmetic)?;
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Selection {
    Index,
    Definition,
}
struct Observer<'a, 'b> {
    budget: &'a mut Budget<'b>,
    function: SemanticFunctionIdV1,
    block: SsaBlockIdV1,
    statement: usize,
    local: SsaVariableIdV1,
    selection: Selection,
    ordinal: Option<u32>,
}
impl SemanticSsaEmissionObserverV1 for Observer<'_, '_> {
    type Error = QueryError;
    fn visit(&mut self, _: SemanticSsaVisitV1, _: ESite) -> QueryResult<()> {
        charge(self.budget, 1)
    }
    fn event(
        &mut self,
        site: ESite,
        operand: ORole,
        role: ERole,
        ordinal: usize,
        event: SsaEventV1,
    ) -> QueryResult<()> {
        charge(self.budget, 10)?;
        let matches = match self.selection {
            Selection::Index => {
                operand == ORole::RvalueOperand(0)
                    && role == ERole::ProjectionIndexUse(0)
                    && event == SsaEventV1::Use(self.local)
            }
            Selection::Definition => {
                operand == ORole::Destination
                    && role == ERole::DestinationDefine
                    && event == SsaEventV1::Define(self.local)
            }
        };
        if site
            == (ESite::Statement {
                block: self.block.get() as usize,
                statement: self.statement,
            })
            && matches
        {
            if self.ordinal.is_some() {
                return Err(mismatch(self.function, Some(self.block)));
            }
            self.ordinal = Some(u32::try_from(ordinal).map_err(|_| arithmetic())?);
        }
        Ok(())
    }
    fn constant(
        &mut self,
        _: ESite,
        _: ORole,
        _: usize,
        _: &SemanticConstantV1,
    ) -> QueryResult<()> {
        charge(self.budget, 1)
    }
    fn successor(&mut self, _: usize, _: usize, _: SemanticControlFlowEdgeV1) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn edge_definition(
        &mut self,
        _: usize,
        _: usize,
        _: SemanticControlFlowEdgeV1,
        _: usize,
        _: SsaVariableIdV1,
    ) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn entry_definition(
        &mut self,
        _: usize,
        _: SsaVariableIdV1,
        _: emission::SemanticSsaEntryOriginV1,
    ) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn elided_borrow(&mut self, _: ESite) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn statement_elision_lookup(&mut self, _: ESite, _: usize) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn block_complete(&mut self, _: usize, _: usize, _: usize) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
    fn input_complete(&mut self, _: usize, _: usize) -> QueryResult<()> {
        Err(mismatch(self.function, Some(self.block)))
    }
}

fn exact_event(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    plan: &SsaConstructionPlanV1,
    block: SsaBlockIdV1,
    statement: u32,
    local: SsaVariableIdV1,
    selection: Selection,
    budget: &mut Budget<'_>,
) -> QueryResult<Option<SsaResolvedEventV1>> {
    charge(budget, 5)?;
    if !plan.is_reachable(block) {
        return Ok(None);
    }
    let Some(source) = function.blocks().get(block.get() as usize) else {
        return Ok(None);
    };
    let end = (statement as usize).checked_add(1).ok_or_else(arithmetic)?;
    let Some(prefix) = source.statements().get(..end) else {
        return Ok(None);
    };
    let mut counter = Counter(0);
    let mut observer = Observer {
        budget,
        function: function_id,
        block,
        statement: statement as usize,
        local,
        selection,
        ordinal: None,
    };
    for (ordinal, row) in prefix.iter().enumerate() {
        charge(observer.budget, 3)?;
        // The original adapter may elide an authenticated Borrow. Without a
        // retained elision row this query does not guess its event count.
        if matches!(row.kind(), SemanticStatementKindV1::Assign(a) if matches!(a.value().kind(), SemanticRvalueKindV1::Borrow { .. }))
        {
            return Ok(None);
        }
        emission::emit_statement_events_with_buffer_v1(
            row.kind(),
            false,
            ESite::Statement {
                block: block.get() as usize,
                statement: ordinal,
            },
            &mut counter,
            &mut observer,
        )
        .map_err(|error| match error {
            emission::SemanticSsaEmissionErrorV1::Observer(error)
            | emission::SemanticSsaEmissionErrorV1::Output(error) => error,
        })?;
    }
    charge(observer.budget, 1)?;
    let Some(ordinal) = observer.ordinal else {
        return Ok(None);
    };
    let rows = plan
        .resolved_events(block)
        .ok_or_else(|| mismatch(function_id, Some(block)))?;
    let mut result = None;
    let mut previous = None;
    for (event, value) in rows {
        charge(observer.budget, 5)?;
        if previous.is_some_and(|old| old >= *event) {
            return Err(mismatch(function_id, Some(block)));
        }
        previous = Some(*event);
        if *event == ordinal {
            if result.is_some() {
                return Err(mismatch(function_id, Some(block)));
            }
            result = Some(*value);
        }
    }
    Ok(result)
}

impl ProductionSemanticSsaOwnerV1 {
    /// Proves a narrow original unsigned literal index at one exact source use.
    ///
    /// This allocation-free query joins the existing adapter's event ordinal to
    /// this owner's sealed plan. It constructs no planner input, reruns no SSA
    /// planner, installs no capture, and grants no memory/refinement authority.
    /// Only RvalueOperand(0), one Index projection, a promoted unsigned temporary
    /// with one direct literal definition, and exact Definition uses are supported.
    /// Unsupported shapes return None. Caller retains its existing source/SSA
    /// storage reservation; every traversal uses the caller's unchanged work meter.
    pub fn original_unsigned_literal_index_v1(
        &self,
        function_id: SemanticFunctionIdV1,
        site: ProductionSemanticSsaOccurrenceSiteV1,
        role: ProductionSemanticSsaOperandRoleV1,
        expected_array: SemanticLocalIdV1,
        expected_index: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> QueryResult<Option<u64>> {
        charge(budget, 8)?;
        let ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } = site else {
            return Ok(None);
        };
        if role != ProductionSemanticSsaOperandRoleV1::RvalueOperand(0) {
            return Ok(None);
        }
        let semantic = self.source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(|| mismatch(function_id, Some(block)))?;
        let retained = self
            .plan_for_function(function_id)
            .ok_or_else(|| mismatch(function_id, Some(block)))?;
        charge(budget, 33)?;
        if retained.function_identity() != function.identity() {
            return Err(mismatch(function_id, Some(block)));
        }
        let plan = retained.plan();
        let Some(source) = function
            .blocks()
            .get(block.get() as usize)
            .and_then(|b| b.statements().get(statement as usize))
        else {
            return Ok(None);
        };
        let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
            return Ok(None);
        };
        let SemanticRvalueKindV1::Use(
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        ) = assignment.value().kind()
        else {
            return Ok(None);
        };
        charge(budget, 5)?;
        if place.local() != expected_array {
            return Ok(None);
        }
        let [projection] = place.projections() else {
            return Ok(None);
        };
        if projection.kind() != SemanticProjectionKindV1::Index(expected_index) {
            return Ok(None);
        }
        let index = function
            .locals()
            .get(expected_index.index() as usize)
            .ok_or_else(|| mismatch(function_id, Some(block)))?;
        charge(budget, 4)?;
        if index.role() != SemanticLocalRoleV1::Temporary {
            return Ok(None);
        }
        let declaration = semantic
            .types()
            .get(index.ty().index() as usize)
            .ok_or_else(|| mismatch(function_id, Some(block)))?;
        let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }) = declaration.shape()
        else {
            return Ok(None);
        };
        charge(budget, 3)?;
        if !matches!(*bits, 8 | 16 | 32 | 64)
            || declaration.layout().size_bytes() != Some(u64::from(*bits / 8))
        {
            return Ok(None);
        }
        let variable = SsaVariableIdV1::new(expected_index.index());
        let mut promoted = false;
        for candidate in plan.promoted_variables() {
            charge(budget, 2)?;
            if *candidate == variable {
                if promoted {
                    return Err(mismatch(function_id, Some(block)));
                }
                promoted = true;
            }
        }
        if !promoted {
            return Ok(None);
        }
        let Some(SsaResolvedEventV1::Use {
            variable: used,
            value: use_value @ SsaValueV1::Definition(_),
        }) = exact_event(
            function_id,
            function,
            plan,
            block,
            statement,
            variable,
            Selection::Index,
            budget,
        )?
        else {
            return Ok(None);
        };
        charge(budget, 1)?;
        if used != variable {
            return Err(mismatch(function_id, Some(block)));
        }

        // Locate the sole literal assignment in the original source, not a
        // diagnostic record, physical constant map, or separately supplied MIR.
        let mut literal = None;
        for (bi, source_block) in function.blocks().iter().enumerate() {
            charge(budget, 2)?;
            let bid = SsaBlockIdV1::new(u32::try_from(bi).map_err(|_| arithmetic())?);
            for (si, row) in source_block.statements().iter().enumerate() {
                charge(budget, 4)?;
                if let SemanticStatementKindV1::Assign(a) = row.kind() {
                    if let SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = a.value().kind()
                    {
                        if place.local() == expected_index {
                            return Ok(None);
                        }
                    }
                    if a.destination().local() != expected_index {
                        continue;
                    }
                    charge(budget, 9)?;
                    if literal.is_some()
                        || !plan.is_reachable(bid)
                        || !a.destination().projections().is_empty()
                        || a.destination().ty() != index.ty()
                        || a.value().result_type() != index.ty()
                    {
                        return Ok(None);
                    }
                    let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
                        a.value().kind()
                    else {
                        return Ok(None);
                    };
                    let SemanticConstantValueV1::Scalar(scalar) = constant.value() else {
                        return Ok(None);
                    };
                    if constant.ty() != index.ty() || u16::from(scalar.size_bytes()) * 8 != *bits {
                        return Ok(None);
                    }
                    let value = u64::try_from(scalar.bits()).map_err(|_| arithmetic())?;
                    let ordinal = u32::try_from(si).map_err(|_| arithmetic())?;
                    let Some(SsaResolvedEventV1::Define {
                        variable: defined,
                        value: definition,
                    }) = exact_event(
                        function_id,
                        function,
                        plan,
                        bid,
                        ordinal,
                        variable,
                        Selection::Definition,
                        budget,
                    )?
                    else {
                        return Ok(None);
                    };
                    charge(budget, 2)?;
                    if defined != variable || definition != use_value {
                        return Ok(None);
                    }
                    literal = Some(value);
                }
            }
        }
        let Some(literal) = literal else {
            return Ok(None);
        };
        for entry in plan.entry_definitions() {
            charge(budget, 2)?;
            if entry.variable() == variable {
                return Ok(None);
            }
        }
        let mut definitions = 0usize;
        for (bi, source_block) in function.blocks().iter().enumerate() {
            charge(budget, 3)?;
            let bid = SsaBlockIdV1::new(u32::try_from(bi).map_err(|_| arithmetic())?);
            if !plan.is_reachable(bid) {
                continue;
            }
            for merged in plan
                .merge_variables(bid)
                .ok_or_else(|| mismatch(function_id, Some(bid)))?
            {
                charge(budget, 2)?;
                if *merged == variable {
                    return Ok(None);
                }
            }
            for (_, event) in plan
                .resolved_events(bid)
                .ok_or_else(|| mismatch(function_id, Some(bid)))?
            {
                charge(budget, 5)?;
                match *event {
                    SsaResolvedEventV1::Use { variable: v, value } if v == variable => {
                        if value != use_value {
                            return Ok(None);
                        }
                    }
                    SsaResolvedEventV1::Define { variable: v, value } if v == variable => {
                        if value != use_value {
                            return Ok(None);
                        }
                        definitions = definitions.checked_add(1).ok_or_else(arithmetic)?;
                        if definitions != 1 {
                            return Ok(None);
                        }
                    }
                    SsaResolvedEventV1::Kill {
                        variable: v,
                        previous: Some(value),
                    } if v == variable => {
                        if value != use_value {
                            return Ok(None);
                        }
                    }
                    _ => {}
                }
            }
            for ordinal in 0..source_block.terminator().kind().edge_count() {
                charge(budget, 4)?;
                let edge = SsaEdgeIdV1::new(bid, u32::try_from(ordinal).map_err(|_| arithmetic())?);
                for definition in plan
                    .edge_definitions(edge)
                    .ok_or_else(|| mismatch(function_id, Some(bid)))?
                {
                    charge(budget, 2)?;
                    if definition.variable() == variable {
                        return Ok(None);
                    }
                }
            }
        }
        charge(budget, 1)?;
        Ok((definitions == 1).then_some(literal))
    }
}
