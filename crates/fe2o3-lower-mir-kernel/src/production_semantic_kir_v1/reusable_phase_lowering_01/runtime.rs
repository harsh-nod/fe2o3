//! Actual source-site emission. The cursor owns linear KIR results; original
//! SSA definitions are never overwritten when End restores an owner/allocation.
use super::borrow_sites::OriginalBorrow;
use super::descriptors::{Description, Descriptions};
use super::schedule::ActionSchedule;
use super::*;
use fe2o3_kernel_ir::{ExecutionCapabilityProvenanceV1, ReusablePhaseOpV1};
use fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 as Site;

pub(in super::super) struct Runtime<'a> {
    pub(super) checked: CheckedRows<'a>,
    schedule: ActionSchedule,
    descriptions: Descriptions,
    state: State,
    borrows: Vec<OriginalBorrow>,
    borrowed: Vec<bool>,
    constructors: super::constructors::Constructors,
    relay_variables: Vec<u32>,
}

struct State {
    phases: Vec<PhaseState>,
    restored: Vec<(SsaValueV1, SemanticValueBindingV1)>,
}

struct PhaseState {
    owner_loan: Option<SemanticValueBindingV1>,
    completion: Option<SemanticValueBindingV1>,
    leases: Vec<LeaseState>,
    started: bool,
    ended: bool,
}

struct LeaseState {
    loan: Option<SemanticValueBindingV1>,
    value: Option<SemanticValueBindingV1>,
    closed: bool,
}

impl<'a> Runtime<'a> {
    pub(in super::super) fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &'a PhaseEmissionInputV1,
        work: &mut usize,
    ) -> PhaseResult<Self> {
        let checked = CheckedRows::new(owner, input, work)?;
        spend(work, std::mem::size_of::<Option<FinishEpoch>>())?;
        let descriptions = Descriptions::new(&checked, work)?;
        let schedule = ActionSchedule::new(&checked, work)?;
        let borrows = super::borrow_sites::collect(&checked, work)?;
        let mut borrowed = reserve(borrows.len(), work)?;
        borrowed.resize(borrows.len(), false);
        let constructors = super::constructors::Constructors::new(&checked, work)?;
        let mut relay_variables = reserve(input.phases.len(), work)?;
        for row in &input.rows {
            spend(work, 1)?;
            if row.action != PhaseEmissionActionV1::RelayClosure {
                continue;
            }
            let variable = row.boundary.variable.get();
            if (variable as usize) < checked.query.function().locals().len() {
                return Err(rejected(
                    "phase relay unexpectedly aliases an original Rust local",
                ));
            }
            let mut slot = 0;
            while slot < relay_variables.len() && relay_variables[slot] < variable {
                spend(work, 1)?;
                slot += 1;
            }
            spend(work, relay_variables.len() - slot + 1)?;
            if relay_variables.get(slot) == Some(&variable) {
                return Err(rejected("phase relay variable was reused"));
            }
            relay_variables.insert(slot, variable);
        }
        if relay_variables.len() != input.phases.len() {
            return Err(rejected("phase relay inventory is not complete"));
        }
        let mut phases = reserve(input.phases.len(), work)?;
        let mut capacity = 0usize;
        for phase in &input.phases {
            spend(work, 1)?;
            capacity = capacity
                .checked_add(phase.leases.len() + 1)
                .ok_or_else(|| rejected("phase restored-value roster overflow"))?;
            let mut leases = reserve(phase.leases.len(), work)?;
            for _ in &phase.leases {
                leases.push(LeaseState {
                    loan: None,
                    value: None,
                    closed: false,
                });
            }
            phases.push(PhaseState {
                owner_loan: None,
                completion: None,
                leases,
                started: false,
                ended: false,
            });
        }
        Ok(Self {
            checked,
            schedule,
            descriptions,
            state: State {
                phases,
                restored: reserve(capacity, work)?,
            },
            borrows,
            borrowed,
            constructors,
            relay_variables,
        })
    }

    /// Only these exact synthetic relay variables leave ordinary local SSA.
    /// They have no Rust local or block parameter and are consumed by this cursor.
    pub(in super::super) fn is_relay(&self, variable: u32, work: &mut usize) -> PhaseResult<bool> {
        let (mut lo, mut hi) = (0, self.relay_variables.len());
        while lo < hi {
            spend(work, 1)?;
            let middle = lo + (hi - lo) / 2;
            match self.relay_variables[middle].cmp(&variable) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => return Ok(true),
            }
        }
        Ok(false)
    }

    pub(in super::super) fn attach(
        &self,
        function: &SemanticFunctionDeclV1,
        plan: &ProductionSemanticSsaFunctionPlanV1,
    ) -> PhaseResult<()> {
        if !std::ptr::eq(function, self.checked.query.function())
            || !std::ptr::eq(plan, self.checked.query.plan())
        {
            return Err(rejected(
                "phase emitter was attached to a different body or SSA plan",
            ));
        }
        Ok(())
    }

    pub(in super::super) fn before_statement(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        statement: u32,
        source: &SemanticStatementKindV1,
        operations: &mut Vec<Operation>,
        work: &mut usize,
    ) -> PhaseResult<bool> {
        let site = Site::new(block, Some(statement));
        let mut handled = false;
        for (index, borrow) in self.borrows.iter().enumerate() {
            spend(work, 1)?;
            if borrow.site != site {
                continue;
            }
            if self.borrowed[index]
                || handled
                || !matches!(source, SemanticStatementKindV1::Assign(actual) if actual == &borrow.assignment)
            {
                return Err(rejected(
                    "phase original borrow changed or was consumed twice",
                ));
            }
            let mut restored = None;
            for (original, value) in &self.state.restored {
                spend(work, 1)?;
                if *original == borrow.source {
                    if restored.replace(value).is_some() {
                        return Err(rejected("phase restored source value is ambiguous"));
                    }
                }
            }
            let value = match restored {
                Some(value) => clone_value(value, work)?,
                None => ssa_value(lowering, borrow.source, work)?,
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = borrow.assignment.value().kind()
            else {
                return Err(rejected("phase checked borrow changed kind"));
            };
            let SemanticValueBindingV1::Value {
                ty: Type::ExecutionCapability(cap),
                ..
            } = &value
            else {
                return Err(rejected(
                    "phase borrow has no actual execution-capability producer",
                ));
            };
            if cap.source_type != execution_type_identity_v1(lowering.types, place.ty())? {
                return Err(rejected(
                    "phase restored borrow changed its exact source type",
                ));
            }
            bind_exact(
                lowering,
                site,
                borrow.assignment.destination(),
                borrow.reference,
                value,
            )?;
            self.borrowed[index] = true;
            handled = true;
        }
        if self
            .constructors
            .defer(&self.checked, lowering, site, source, work)?
        {
            if handled {
                return Err(rejected("phase constructor overlaps a source borrow"));
            }
            handled = true;
        }
        let state = &mut self.state;
        let checked = &self.checked;
        let descriptions = &self.descriptions;
        self.schedule.consume_site(site, work, |index, row, work| {
            if matches!(
                row.action,
                PhaseEmissionActionV1::OwnerConvert
                    | PhaseEmissionActionV1::Begin
                    | PhaseEmissionActionV1::Bind { .. }
                    | PhaseEmissionActionV1::Seal
                    | PhaseEmissionActionV1::RelayClosure
            ) {
                if handled {
                    return Err(rejected("phase result site has multiple source actions"));
                }
                handled = true;
            }
            state.emit(
                checked,
                descriptions.get(index)?,
                row,
                lowering,
                operations,
                work,
            )
        })?;
        Ok(handled)
    }

    pub(in super::super) fn before_terminator(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        work: &mut usize,
    ) -> PhaseResult<()> {
        if lowering.phase_finish_epoch.is_some() {
            return Err(rejected("phase Finish epoch was not consumed at its original call"));
        }
        let state = &mut self.state;
        let checked = &self.checked;
        let descriptions = &self.descriptions;
        self.schedule
            .consume_site(Site::new(block, None), work, |index, row, work| {
                if !matches!(
                    row.action,
                    PhaseEmissionActionV1::RelayDrop | PhaseEmissionActionV1::End
                ) {
                    return Err(rejected(
                        "phase source action moved to an unsupported terminator",
                    ));
                }
                state.emit(
                    checked,
                    descriptions.get(index)?,
                    row,
                    lowering,
                    operations,
                    work,
                )
            })?;
        if let Some((index, epoch)) = self.descriptions.finish_at(block, work)? {
            let phase = &self.state.phases[index];
            if !phase.started || phase.ended || phase.owner_loan.is_none() {
                return Err(rejected("phase Finish epoch is outside its original active phase"));
            }
            lowering.phase_finish_epoch = Some(epoch);
        }
        Ok(())
    }

    pub(in super::super) fn complete(&self, work: &mut usize) -> PhaseResult<()> {
        self.schedule.complete(work)?;
        self.constructors.complete(work)?;
        for borrowed in &self.borrowed {
            spend(work, 1)?;
            if !borrowed {
                return Err(rejected(
                    "phase emission omitted an original owner/storage borrow",
                ));
            }
        }
        self.state.complete(work)
    }
}

impl State {
    fn complete(&self, work: &mut usize) -> PhaseResult<()> {
        for phase in &self.phases {
            spend(work, 1)?;
            if !phase.started
                || !phase.ended
                || phase.owner_loan.is_some()
                || phase.completion.is_some()
            {
                return Err(rejected("phase emission left an unconsumed linear result"));
            }
            for lease in &phase.leases {
                spend(work, 1)?;
                if !lease.closed || lease.loan.is_some() || lease.value.is_some() {
                    return Err(rejected("phase emission left an unconsumed linear result"));
                }
            }
        }
        Ok(())
    }

    fn emit(
        &mut self,
        checked: &CheckedRows<'_>,
        description: &Description,
        row: PhaseEmissionRowV1,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        operations: &mut Vec<Operation>,
        work: &mut usize,
    ) -> PhaseResult<()> {
        use PhaseEmissionActionV1 as Action;
        let input = &checked.input.phases[row.phase];
        let state = &mut self.phases[row.phase];
        let site = row.boundary.site;
        let mut operands = reserve(description.operation.arity().0, work)?;
        let take = |value: &mut Option<SemanticValueBindingV1>| {
            value
                .take()
                .ok_or_else(|| rejected("phase action has no current linear producer"))
        };
        match row.action {
            Action::OwnerConvert => {
                operands.push(ssa_value(lowering, input.owner_workgroup, work)?)
            }
            Action::Begin => {
                if state.started || state.ended {
                    return Err(rejected("phase Begin was already consumed"));
                }
                operands.push(ssa_value(lowering, input.owner_reference, work)?);
            }
            Action::Bind { lease } => {
                if !state.started
                    || state.ended
                    || state.leases[lease].loan.is_some()
                    || state.leases[lease].closed
                {
                    return Err(rejected(
                        "phase Bind is outside its active once-only lifetime",
                    ));
                }
                operands.push(take(&mut state.owner_loan)?);
                operands.push(ssa_value(
                    lowering,
                    input.leases[lease].phase_reference,
                    work,
                )?);
                operands.push(ssa_value(
                    lowering,
                    input.leases[lease].storage_reference,
                    work,
                )?);
            }
            Action::Seal => {
                let result = result_record(checked, input.finish, work)?;
                let fe2o3_pliron::ProductionSemanticPhaseResultInputsV1::Finish { barrier_output } =
                    result.inputs()
                else {
                    return Err(rejected("phase Seal lost its actual barrier result"));
                };
                let barrier = event_value(
                    checked,
                    site,
                    barrier_output.index(),
                    PhaseBoundaryKindV1::Use,
                    work,
                )?;
                operands.push(take(&mut state.owner_loan)?);
                operands.push(ssa_value(lowering, barrier, work)?);
            }
            Action::RelayClosure => operands.push(ssa_value(lowering, input.completion, work)?),
            Action::RelayDrop => {
                operands.push(take(&mut state.owner_loan)?);
                operands.push(take(&mut state.completion)?);
            }
            Action::CloseStorage { lease } => {
                operands.push(take(&mut state.leases[lease].loan)?);
                operands.push(take(&mut state.leases[lease].value)?);
            }
            Action::End => {
                if state.ended {
                    return Err(rejected("phase End was already consumed"));
                }
                operands.push(take(&mut state.owner_loan)?);
                operands.push(take(&mut state.completion)?);
                for lease in &mut state.leases {
                    spend(work, 1)?;
                    if !lease.closed {
                        return Err(rejected("phase End omitted a source lease close"));
                    }
                    operands.push(take(&mut lease.loan)?);
                }
            }
        }
        let mut results =
            emit_operation(lowering, operations, description, row, &operands, work)?.into_iter();
        let next = |results: &mut std::vec::IntoIter<SemanticValueBindingV1>| {
            results
                .next()
                .ok_or_else(|| rejected("phase emitter omitted a checked operation result"))
        };
        match row.action {
            Action::OwnerConvert => bind_result(
                checked,
                lowering,
                input.owner,
                row,
                next(&mut results)?,
                false,
                work,
            )?,
            Action::Begin => {
                bind_result(
                    checked,
                    lowering,
                    input.issue,
                    row,
                    next(&mut results)?,
                    false,
                    work,
                )?;
                state.owner_loan = Some(next(&mut results)?);
                state.started = true;
            }
            Action::Bind { lease } => {
                state.owner_loan = Some(next(&mut results)?);
                let value = next(&mut results)?;
                state.leases[lease].value = Some(clone_value(&value, work)?);
                bind_result(
                    checked,
                    lowering,
                    input.leases[lease].bind,
                    row,
                    value,
                    true,
                    work,
                )?;
                state.leases[lease].loan = Some(next(&mut results)?);
            }
            Action::Seal => {
                state.owner_loan = Some(next(&mut results)?);
                bind_result(
                    checked,
                    lowering,
                    input.finish,
                    row,
                    next(&mut results)?,
                    true,
                    work,
                )?;
            }
            Action::RelayClosure => {
                if state.completion.replace(next(&mut results)?).is_some() {
                    return Err(rejected("phase closure relayed its completion twice"));
                }
                lower_relay_pack(checked, lowering, row, operations, work)?;
            }
            Action::RelayDrop => {
                state.owner_loan = Some(next(&mut results)?);
                state.completion = Some(next(&mut results)?);
            }
            Action::CloseStorage { lease } => {
                if state.leases[lease].closed {
                    return Err(rejected("phase lease closed twice"));
                }
                state.leases[lease].loan = Some(next(&mut results)?);
                state.leases[lease].closed = true;
            }
            Action::End => {
                state.ended = true;
                restore(
                    &mut self.restored,
                    input.converted_owner,
                    next(&mut results)?,
                    work,
                )?;
                for lease in &input.leases {
                    restore(
                        &mut self.restored,
                        lease.allocation,
                        next(&mut results)?,
                        work,
                    )?;
                }
            }
        }
        if results.next().is_some() {
            return Err(rejected("phase emitter added an unexpected result"));
        }
        Ok(())
    }
}

fn restore(
    restored: &mut Vec<(SsaValueV1, SemanticValueBindingV1)>,
    original: SsaValueV1,
    value: SemanticValueBindingV1,
    work: &mut usize,
) -> PhaseResult<()> {
    for (source, current) in restored.iter_mut() {
        spend(work, 1)?;
        if *source == original {
            *current = value;
            return Ok(());
        }
    }
    if restored.len() == restored.capacity() {
        return Err(rejected("phase restored-value roster exceeded its bound"));
    }
    restored.push((original, value));
    Ok(())
}

pub(super) fn clone_value(
    value: &SemanticValueBindingV1,
    work: &mut usize,
) -> PhaseResult<SemanticValueBindingV1> {
    let SemanticValueBindingV1::Value { id, ty } = value else {
        return Err(rejected("phase operand is not a real KIR value"));
    };
    clone_typed(*id, ty, work)
}

fn clone_typed(id: ValueId, ty: &Type, work: &mut usize) -> PhaseResult<SemanticValueBindingV1> {
    let root = match ty {
        Type::ExecutionCapability(cap) => cap.provenance.root.as_str(),
        Type::ReusablePhaseToken(token) => token.provenance.root.as_str(),
        _ => {
            return Err(rejected(
                "phase operand changed its exact capability/token type",
            ));
        }
    };
    spend(
        work,
        std::mem::size_of::<SemanticValueBindingV1>()
            .checked_add(root.len())
            .ok_or_else(|| rejected("phase operand copy accounting overflow"))?,
    )?;
    Ok(SemanticValueBindingV1::Value { id, ty: ty.clone() })
}

fn ssa_value(
    lowering: &SemanticFunctionLoweringV1<'_>,
    value: SsaValueV1,
    work: &mut usize,
) -> PhaseResult<SemanticValueBindingV1> {
    spend(work, 1)?;
    clone_value(
        lowering
            .semantic_ssa_bindings
            .get(&value)
            .ok_or_else(|| rejected("phase source operand has no dominating emitted SSA value"))?,
        work,
    )
}

pub(super) fn event_value(
    checked: &CheckedRows<'_>,
    site: Site,
    local: u32,
    kind: PhaseBoundaryKindV1,
    work: &mut usize,
) -> PhaseResult<SsaValueV1> {
    let events = checked
        .query
        .plan()
        .plan()
        .resolved_events(SsaBlockIdV1::new(site.block().index()))
        .ok_or_else(|| rejected("phase source event block is unreachable"))?;
    let mut found = None;
    for (event, resolved) in events {
        spend(work, 1)?;
        let candidate = match (kind, *resolved) {
            (PhaseBoundaryKindV1::Define, SsaResolvedEventV1::Define { variable, value })
            | (PhaseBoundaryKindV1::Use, SsaResolvedEventV1::Use { variable, value })
                if variable.get() == local =>
            {
                Some(value)
            }
            _ => None,
        };
        let Some(value) = candidate else {
            continue;
        };
        let actual = checked
            .query
            .event_site(SsaBlockIdV1::new(site.block().index()), *event, &mut || {
                spend(work, 1).is_ok()
            })
            .map_err(|_| rejected("phase source event lost its original site"))?;
        if actual == site && found.replace(value).is_some() {
            return Err(rejected("phase source site has an ambiguous value event"));
        }
    }
    found.ok_or_else(|| rejected("phase source site has no exact requested value event"))
}

pub(super) fn bind_exact(
    lowering: &mut SemanticFunctionLoweringV1<'_>,
    site: Site,
    destination: &SemanticPlaceV1,
    expected: SsaValueV1,
    value: SemanticValueBindingV1,
) -> PhaseResult<()> {
    if lowering
        .pending_semantic_ssa_definitions
        .get(&(site.block().index(), destination.local().index()))
        .and_then(VecDeque::front)
        != Some(&expected)
    {
        return Err(rejected(
            "phase result did not consume its original next SSA definition",
        ));
    }
    lowering.bind_destination(site.block(), site.statement(), destination, value)
}

fn result_record<'a>(
    checked: &'a CheckedRows<'_>,
    instance: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PhaseResult<&'a fe2o3_pliron::ProductionSemanticPhaseResultV1> {
    let mut found = None;
    for result in checked.query.plan().defined_reusable_phase_results() {
        spend(work, 1)?;
        if result.callee() == instance && found.replace(result).is_some() {
            return Err(rejected("phase result has two retained producer records"));
        }
    }
    found.ok_or_else(|| rejected("phase result has no retained producer record"))
}

fn bind_result(
    checked: &CheckedRows<'_>,
    lowering: &mut SemanticFunctionLoweringV1<'_>,
    instance: SemanticCallInstanceIdV1,
    row: PhaseEmissionRowV1,
    value: SemanticValueBindingV1,
    synthetic_return: bool,
    work: &mut usize,
) -> PhaseResult<()> {
    let site = row.boundary.site;
    let binding = checked.binding(instance, work)?;
    if synthetic_return {
        let result = result_record(checked, instance, work)?;
        if result.block() != site.block()
            || Some(result.statement()) != site.statement()
            || result.return_local() != binding.callee_return()
        {
            return Err(rejected(
                "phase result changed its exact return-transfer producer",
            ));
        }
        let defined = event_value(
            checked,
            site,
            binding.callee_return().index(),
            PhaseBoundaryKindV1::Define,
            work,
        )?;
        let place = SemanticPlaceV1::new(
            binding.callee_return(),
            Vec::new(),
            binding.destination().ty(),
        )
        .map_err(|_| rejected("phase result has an invalid original return place"))?;
        bind_exact(lowering, site, &place, defined, clone_value(&value, work)?)?;
    }
    bind_exact(
        lowering,
        site,
        binding.destination(),
        row.boundary.value,
        value,
    )
}

fn lower_relay_pack(
    checked: &CheckedRows<'_>,
    lowering: &mut SemanticFunctionLoweringV1<'_>,
    row: PhaseEmissionRowV1,
    operations: &mut Vec<Operation>,
    work: &mut usize,
) -> PhaseResult<()> {
    let site = row.boundary.site;
    let statement = site
        .statement()
        .ok_or_else(|| rejected("phase relay pack is not a source statement"))?;
    let source = &checked.query.function().blocks()[site.block().index() as usize].statements()
        [statement as usize];
    let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
        return Err(rejected("phase relay pack changed its source assignment"));
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(rejected(
            "phase relay pack is not the checked completion/result pair",
        ));
    };
    if aggregate.operands().len() != 2 || !assignment.destination().projections().is_empty() {
        return Err(rejected(
            "phase relay pack changed its exact two-field result",
        ));
    }
    // The linear completion has just been consumed by RelayClosure. Only the
    // independent ordinary result continues through the original Rust pair.
    let result = lowering.lower_operand(
        site.block(),
        site.statement(),
        &aggregate.operands()[1],
        operations,
    )?;
    let mut fields = reserve(2, work)?;
    fields.push(SemanticValueBindingV1::Unit);
    fields.push(result);
    let defined = event_value(
        checked,
        site,
        assignment.destination().local().index(),
        PhaseBoundaryKindV1::Define,
        work,
    )?;
    bind_exact(
        lowering,
        site,
        assignment.destination(),
        defined,
        SemanticValueBindingV1::Aggregate(fields),
    )
}

fn emit_operation(
    lowering: &mut SemanticFunctionLoweringV1<'_>,
    operations: &mut Vec<Operation>,
    description: &Description,
    row: PhaseEmissionRowV1,
    operands: &[SemanticValueBindingV1],
    work: &mut usize,
) -> PhaseResult<Vec<SemanticValueBindingV1>> {
    let context = lowering
        .kernel_context
        .ok_or_else(|| rejected("phase emission has no exact root Context"))?;
    let source = description.provenance;
    let provenance = ExecutionCapabilityProvenanceV1 {
        root: context.context_type.root().clone(),
        kernel_binding: *source.kernel_binding().as_bytes(),
        frontend_unit: *source.frontend_unit().as_bytes(),
        kernel_marker: *source.kernel_marker().as_bytes(),
        target_brand: *source.target_brand().as_bytes(),
        launch_brand: *source.launch_brand().as_bytes(),
        issuance: *source.issuance().as_bytes(),
    };
    spend(
        work,
        provenance.root.as_str().len() + std::mem::size_of::<ReusablePhaseOpV1>(),
    )?;
    // The shared checked emitter allocates transient result-type/result lists.
    // Charge their bounded arity before invoking it; no extra allowance or
    // claim about the allocator's physical bookkeeping is introduced here.
    let result_work = std::mem::size_of::<Type>()
        .checked_add(provenance.root.as_str().len())
        .and_then(|per_result| per_result.checked_mul(description.operation.arity().1))
        .and_then(|all| all.checked_mul(4))
        .ok_or_else(|| rejected("phase checked-emitter work accounting overflow"))?;
    spend(work, result_work)?;
    let mut ids = reserve(operands.len(), work)?;
    let mut inputs = reserve(operands.len(), work)?;
    for operand in operands {
        spend(work, 1)?;
        let SemanticValueBindingV1::Value { id, ty } = operand else {
            return Err(rejected("phase emission requires actual typed KIR inputs"));
        };
        ids.push(*id);
        inputs.push((*id, ty));
    }
    let contract = ReusablePhaseOpV1 {
        operands: ids,
        operation: description.operation.clone(),
        provenance,
        source: description.source.clone(),
        obligations: ExecutionSafetyObligationsV1::from_bits(
            description.operation.required_obligations(),
        ),
    };
    let (operation, next) = contract
        .checked_operation(&inputs, ValueId(lowering.next_value))
        .ok_or_else(|| {
            rejection_diagnostic::report(
                &row,
                &description.operation,
                &description.source,
                &description.provenance,
                context.context_type.root().as_str(),
                &inputs,
                ValueId(lowering.next_value),
            );
            rejected("phase operation rejected its actual input producer types")
        })?;
    let mut outputs = reserve(operation.results.len(), work)?;
    for result in &operation.results {
        outputs.push(clone_typed(result.id, &result.ty, work)?);
    }
    lowering.push_operation(operations, || operation)?;
    lowering.next_value = next.0;
    Ok(outputs)
}

#[path = "rejection_diagnostic.rs"]
mod rejection_diagnostic;

#[cfg(test)]
#[path = "completion_work_tests.rs"]
mod completion_work_tests;
