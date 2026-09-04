#[derive(Clone, Debug, Default)]
struct SemanticControlFlowSsaPlanV1 {
    compiler_issued_bindings: BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    implicit_entry_locals: BTreeSet<u32>,
    ssa_value_locals: BTreeSet<u32>,
    promoted: BTreeMap<u32, SemanticPromotedLocalV1>,
    live_in: BTreeMap<u32, Vec<u32>>,
    block_entry_values: BTreeMap<(u32, u32), SsaValueV1>,
    entry_definitions: BTreeMap<u32, SsaValueV1>,
    definition_values: BTreeMap<(u32, u32), Vec<SsaValueV1>>,
    edge_definitions: BTreeMap<(u32, u32), Vec<SsaArgumentV1>>,
    edge_arguments: BTreeMap<(u32, u32), Vec<SsaArgumentV1>>,
    retained_local_slots: BTreeMap<u32, SemanticRetainedLocalSlotPlanV1>,
    retained_initialized_at_entry: BTreeMap<u32, BTreeSet<u32>>,
}

#[derive(Clone, Debug)]
struct SemanticRetainedLocalSlotPlanV1 {
    semantic_type: SemanticTypeIdV1,
    kernel_type: Type,
    alignment: u32,
}

#[derive(Clone, Copy, Debug)]
struct SemanticRetainedInitializationBudgetV1 {
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
    peak_storage: usize,
}

impl SemanticRetainedInitializationBudgetV1 {
    const fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work_limit,
            storage_limit,
            work: 0,
            storage: 0,
            peak_storage: 0,
        }
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work =
            self.work
                .checked_add(amount)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: usize::MAX,
                    limit: self.work_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisWork,
            self.work,
            self.work_limit,
        )
    }

    fn replace_storage(
        &mut self,
        previous: usize,
        next: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let storage = self.storage.checked_sub(previous).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "retained-local initialization storage accounting underflow",
            )
        })?;
        let storage =
            storage
                .checked_add(next)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                    actual: usize::MAX,
                    limit: self.storage_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            storage,
            self.storage_limit,
        )?;
        self.storage = storage;
        self.peak_storage = self.peak_storage.max(storage);
        Ok(())
    }

    fn charge_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.replace_storage(0, amount)
    }

    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.replace_storage(amount, 0)
    }
}

fn retained_local_slot_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<(Type, u32)> {
    let declaration = types.get(ty.index() as usize)?;
    let layout = declaration.layout();
    if layout.is_uninhabited() {
        return None;
    }
    let (kernel_type, expected_size) = match declaration.shape() {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            let kernel_type = lower_scalar_type(types, ty).ok()?;
            let scalar = kernel_type.as_scalar()?;
            let size = match scalar {
                ScalarType::Bool => 1,
                ScalarType::Index => return None,
                scalar => u64::from(scalar.bit_width()? / 8),
            };
            (kernel_type, size)
        }
        SemanticTypeShapeV1::Pointer(pointer)
            if pointer.metadata() == SemanticPointerMetadataV1::None
                && matches!(pointer.pointer_width_bits(), 32 | 64) =>
        {
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            let kernel_type = Type::pointer(
                lower_memory_element_type(types, pointer.pointee()).ok()?,
                lower_address_space(pointer.address_space()).ok()?,
                access,
            );
            (kernel_type, u64::from(pointer.pointer_width_bits() / 8))
        }
        _ => return None,
    };
    let alignment = u32::try_from(layout.alignment_bytes()).ok()?;
    if layout.size_bytes() != Some(expected_size)
        || alignment == 0
        || !alignment.is_power_of_two()
        || u64::from(alignment) > expected_size
        || !kernel_type.is_storable()
    {
        return None;
    }
    Some((kernel_type, alignment))
}

fn private_slot_candidate_locals_v1(
    function: &SemanticFunctionDeclV1,
    promoted: &BTreeSet<u32>,
    retained_cross_edge: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let mut candidates = retained_cross_edge
        .difference(promoted)
        .copied()
        .collect::<BTreeSet<_>>();
    let mut add_root = |place: &SemanticPlaceV1| {
        // The shared adapter marks an address-taking borrow promotable only when
        // it proved one exact compiler-intrinsic consumer. Such a borrow
        // transports the semantic value/capability, not a Rust stack address.
        let local = place.local().index();
        if !promoted.contains(&local)
            && !matches!(
                place
                    .projections()
                    .first()
                    .map(|projection| projection.kind()),
                Some(SemanticProjectionKindV1::Dereference),
            )
        {
            candidates.insert(local);
        }
    };
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        add_root(assignment.destination());
                    }
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } => add_root(place),
                        SemanticRvalueKindV1::Load(load) => add_root(load.source()),
                        _ => {}
                    }
                }
                SemanticStatementKindV1::Store(store) => add_root(store.destination()),
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    add_root(operation.address());
                    if !operation.destination().projections().is_empty() {
                        add_root(operation.destination());
                    }
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    add_root(operation.address());
                    if !operation.destination().projections().is_empty() {
                        add_root(operation.destination());
                    }
                }
                SemanticStatementKindV1::SetDiscriminant { .. }
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && !destination.place().projections().is_empty()
        {
            add_root(destination.place());
        }
    }
    candidates
}

fn kill_moved_retained_operand_v1(
    operand: &SemanticOperandV1,
    retained: &BTreeSet<u32>,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if let SemanticOperandV1::Move(place) = operand
        && place.projections().is_empty()
        && retained.contains(&place.local().index())
        && initialized.remove(&place.local().index())
    {
        budget.release_storage(1)?;
    }
    Ok(())
}

fn insert_retained_initialization_v1(
    local: u32,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !initialized.contains(&local) {
        budget.charge_storage(1)?;
        initialized.insert(local);
    }
    Ok(())
}

fn remove_retained_initialization_v1(
    local: u32,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if initialized.remove(&local) {
        budget.release_storage(1)?;
    }
    Ok(())
}

fn apply_retained_rvalue_effects_v1(
    value: &SemanticRvalueKindV1,
    retained: &BTreeSet<u32>,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    match value {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => {
            kill_moved_retained_operand_v1(operand, retained, initialized, budget)?;
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            kill_moved_retained_operand_v1(left, retained, initialized, budget)?;
            kill_moved_retained_operand_v1(right, retained, initialized, budget)?;
        }
        SemanticRvalueKindV1::CheckedBinary(operation) => {
            kill_moved_retained_operand_v1(operation.left(), retained, initialized, budget)?;
            kill_moved_retained_operand_v1(operation.right(), retained, initialized, budget)?;
        }
        SemanticRvalueKindV1::UncheckedBinary(operation) => {
            kill_moved_retained_operand_v1(operation.left(), retained, initialized, budget)?;
            kill_moved_retained_operand_v1(operation.right(), retained, initialized, budget)?;
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                kill_moved_retained_operand_v1(operand, retained, initialized, budget)?;
            }
        }
        SemanticRvalueKindV1::Borrow { .. }
        | SemanticRvalueKindV1::AddressOf { .. }
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_)
        | SemanticRvalueKindV1::Load(_) => {}
    }
    Ok(())
}

fn apply_retained_statement_effects_v1(
    statement: &SemanticStatementKindV1,
    retained: &BTreeSet<u32>,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            apply_retained_rvalue_effects_v1(
                assignment.value().kind(),
                retained,
                initialized,
                budget,
            )?;
            if assignment.destination().projections().is_empty()
                && retained.contains(&assignment.destination().local().index())
            {
                insert_retained_initialization_v1(
                    assignment.destination().local().index(),
                    initialized,
                    budget,
                )?;
            }
        }
        SemanticStatementKindV1::Store(store) => {
            kill_moved_retained_operand_v1(store.value(), retained, initialized, budget)?;
            if store.destination().projections().is_empty()
                && retained.contains(&store.destination().local().index())
            {
                insert_retained_initialization_v1(
                    store.destination().local().index(),
                    initialized,
                    budget,
                )?;
            }
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            kill_moved_retained_operand_v1(operation.value(), retained, initialized, budget)?;
            if operation.destination().projections().is_empty()
                && retained.contains(&operation.destination().local().index())
            {
                insert_retained_initialization_v1(
                    operation.destination().local().index(),
                    initialized,
                    budget,
                )?;
            }
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            kill_moved_retained_operand_v1(operation.expected(), retained, initialized, budget)?;
            kill_moved_retained_operand_v1(operation.replacement(), retained, initialized, budget)?;
            if operation.destination().projections().is_empty()
                && retained.contains(&operation.destination().local().index())
            {
                insert_retained_initialization_v1(
                    operation.destination().local().index(),
                    initialized,
                    budget,
                )?;
            }
        }
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            remove_retained_initialization_v1(local.index(), initialized, budget)?;
        }
        SemanticStatementKindV1::Deinitialize(place) => {
            if place.projections().is_empty() {
                remove_retained_initialization_v1(
                    place.local().index(),
                    initialized,
                    budget,
                )?;
            }
        }
        SemanticStatementKindV1::Assume(condition) => {
            kill_moved_retained_operand_v1(condition, retained, initialized, budget)?;
        }
        SemanticStatementKindV1::SetDiscriminant { .. } | SemanticStatementKindV1::Nop => {}
    }
    Ok(())
}

fn apply_retained_terminator_move_effects_v1(
    terminator: &SemanticTerminatorKindV1,
    retained: &BTreeSet<u32>,
    initialized: &mut BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let mut apply = |operand: &SemanticOperandV1| {
        kill_moved_retained_operand_v1(operand, retained, initialized, budget)
    };
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => apply(discriminant)?,
        SemanticTerminatorKindV1::Call(call) => {
            for argument in call.arguments() {
                apply(argument)?;
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                apply(argument)?;
            }
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            apply(condition)?;
            match message {
                SemanticAssertMessageV1::BoundsCheck { length, index } => {
                    apply(length)?;
                    apply(index)?;
                }
                SemanticAssertMessageV1::Overflow { left, right, .. } => {
                    apply(left)?;
                    apply(right)?;
                }
                SemanticAssertMessageV1::DivisionByZero(operand)
                | SemanticAssertMessageV1::RemainderByZero(operand) => apply(operand)?,
                SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment,
                    found_alignment,
                } => {
                    apply(required_alignment)?;
                    apply(found_alignment)?;
                }
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {}
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Drop { .. }
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

fn retained_local_initialization_entries_v1(
    function: &SemanticFunctionDeclV1,
    retained_local_slots: &BTreeMap<u32, SemanticRetainedLocalSlotPlanV1>,
    reachable: &BTreeSet<u32>,
    max_analysis_work: usize,
    max_analysis_storage: usize,
) -> Result<BTreeMap<u32, BTreeSet<u32>>, ProductionSemanticKirErrorV1> {
    let mut budget =
        SemanticRetainedInitializationBudgetV1::new(max_analysis_work, max_analysis_storage);
    retained_local_initialization_entries_with_budget_v1(
        function,
        retained_local_slots,
        reachable,
        &mut budget,
    )
}

fn retained_local_initialization_entries_with_budget_v1(
    function: &SemanticFunctionDeclV1,
    retained_local_slots: &BTreeMap<u32, SemanticRetainedLocalSlotPlanV1>,
    reachable: &BTreeSet<u32>,
    budget: &mut SemanticRetainedInitializationBudgetV1,
) -> Result<BTreeMap<u32, BTreeSet<u32>>, ProductionSemanticKirErrorV1> {
    budget.charge_work(retained_local_slots.len())?;
    budget.charge_storage(retained_local_slots.len())?;
    let retained = retained_local_slots
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if retained.is_empty() {
        return Ok(BTreeMap::new());
    }
    let entry = function.entry().index();
    budget.charge_work(function.locals().len())?;
    let mut entry_initialized = BTreeSet::new();
    for (local, declaration) in function.locals().iter().enumerate() {
        let local = local as u32;
        if retained.contains(&local)
            && matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
        {
            insert_retained_initialization_v1(local, &mut entry_initialized, budget)?;
        }
    }
    budget.charge_storage(1)?;
    let mut initialized_at_entry = BTreeMap::from([(entry, entry_initialized)]);
    budget.charge_storage(2)?;
    let mut worklist = VecDeque::from([entry]);
    let mut queued = BTreeSet::from([entry]);
    while let Some(source) = worklist.pop_front() {
        budget.release_storage(1)?;
        if !queued.remove(&source) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        budget.release_storage(1)?;
        budget.charge_work(1)?;
        let block = function
            .blocks()
            .get(source as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let outgoing_source = initialized_at_entry
            .get(&source)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_storage(outgoing_source.len())?;
        let mut outgoing = outgoing_source.clone();
        budget.charge_work(outgoing.len())?;
        for statement in block.statements() {
            apply_retained_statement_effects_v1(
                statement.kind(),
                &retained,
                &mut outgoing,
                budget,
            )?;
        }
        apply_retained_terminator_move_effects_v1(
            block.terminator().kind(),
            &retained,
            &mut outgoing,
            budget,
        )?;
        block.terminator().kind().try_for_each_edge(|edge| {
            budget.charge_work(1usize.saturating_add(outgoing.len()))?;
            let target = edge.target().index();
            if !reachable.contains(&target) {
                return Ok(());
            }
            if target == entry {
                return Err(unsupported(
                    0,
                    Some(source),
                    None,
                    "retained-local slots require a synthetic preheader for a cyclic entry",
                ));
            }
            budget.charge_storage(outgoing.len())?;
            let mut edge_state = outgoing.clone();
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.edge() == edge
                && destination.place().projections().is_empty()
                && retained.contains(&destination.place().local().index())
            {
                insert_retained_initialization_v1(
                    destination.place().local().index(),
                    &mut edge_state,
                    budget,
                )?;
            }
            let changed = match initialized_at_entry.get_mut(&target) {
                Some(current) => {
                    budget.charge_work(
                        1usize
                            .saturating_add(current.len())
                            .saturating_add(edge_state.len()),
                    )?;
                    let next_len = current.intersection(&edge_state).count();
                    budget.charge_storage(next_len)?;
                    let next = current
                        .intersection(&edge_state)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    if *current == next {
                        budget.release_storage(next_len)?;
                        false
                    } else {
                        let previous_len = current.len();
                        *current = next;
                        budget.release_storage(previous_len)?;
                        true
                    }
                }
                None => {
                    budget.charge_storage(1)?;
                    initialized_at_entry.insert(target, edge_state);
                    if !queued.contains(&target) {
                        budget.charge_storage(2)?;
                        queued.insert(target);
                        worklist.push_back(target);
                    }
                    return Ok(());
                }
            };
            budget.release_storage(edge_state.len())?;
            if changed && !queued.contains(&target) {
                budget.charge_storage(2)?;
                queued.insert(target);
                worklist.push_back(target);
            }
            Ok(())
        })?;
        budget.release_storage(outgoing.len())?;
    }
    budget.charge_work(initialized_at_entry.len().saturating_add(reachable.len()))?;
    if initialized_at_entry.len() != reachable.len()
        || !initialized_at_entry
            .keys()
            .all(|block| reachable.contains(block))
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    budget.release_storage(retained.len())?;
    Ok(initialized_at_entry)
}

fn first_retained_local_cause_v1(
    function: &SemanticFunctionDeclV1,
    local: u32,
    callables: &[SemanticCallableDeclV1],
    compiler_issued_bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
) -> &'static str {
    let place_matches = |place: &SemanticPlaceV1| {
        place.local().index() == local
            && !matches!(
                place
                    .projections()
                    .first()
                    .map(|projection| projection.kind()),
                Some(SemanticProjectionKindV1::Dereference),
            )
    };
    let transparent_borrow = |place: &SemanticPlaceV1| {
        place.projections().is_empty()
            && matches!(
                compiler_issued_bindings.get(&place.ty()),
                Some(
                    SemanticPromotedBindingV1::MathContext
                        | SemanticPromotedBindingV1::CollectiveContext
                        | SemanticPromotedBindingV1::MatrixContext
                        | SemanticPromotedBindingV1::WaveLane { .. }
                )
            )
    };
    let rvalue_cause = |value: &SemanticRvalueKindV1| match value {
        SemanticRvalueKindV1::Borrow { place, .. }
            if place_matches(place) && !transparent_borrow(place) =>
        {
            Some(first_borrow_consumer_cause_v1(function, local, callables))
        }
        SemanticRvalueKindV1::AddressOf { place, .. } if place_matches(place) => Some("address-of"),
        SemanticRvalueKindV1::Load(load) if place_matches(load.source()) => Some("local load"),
        _ => None,
    };

    for block in function.blocks() {
        for statement in block.statements() {
            let cause = match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty()
                        && place_matches(assignment.destination())
                    {
                        Some("projected assignment")
                    } else {
                        rvalue_cause(assignment.value().kind())
                    }
                }
                SemanticStatementKindV1::Store(store) if place_matches(store.destination()) => {
                    Some("local store")
                }
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    if !operation.destination().projections().is_empty()
                        && place_matches(operation.destination())
                    {
                        Some("projected atomic result")
                    } else if place_matches(operation.address()) {
                        Some("atomic address")
                    } else {
                        None
                    }
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    if !operation.destination().projections().is_empty()
                        && place_matches(operation.destination())
                    {
                        Some("projected atomic result")
                    } else if place_matches(operation.address()) {
                        Some("atomic address")
                    } else {
                        None
                    }
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. } if place_matches(place) => {
                    Some("set discriminant")
                }
                SemanticStatementKindV1::Deinitialize(place) if place_matches(place) => {
                    Some("deinitialize")
                }
                _ => None,
            };
            if let Some(cause) = cause {
                return cause;
            }
        }
        let cause = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => call
                .destination()
                .filter(|destination| {
                    !destination.place().projections().is_empty()
                        && place_matches(destination.place())
                })
                .map(|_| "projected call result"),
            SemanticTerminatorKindV1::Drop { place, .. } if place_matches(place) => Some("drop"),
            _ => None,
        };
        if let Some(cause) = cause {
            return cause;
        }
    }
    "retained storage"
}

fn first_borrow_consumer_cause_v1(
    function: &SemanticFunctionDeclV1,
    source_local: u32,
    callables: &[SemanticCallableDeclV1],
) -> &'static str {
    let mut borrowed_locals = BTreeSet::new();
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                continue;
            };
            if place.local().index() == source_local
                && place.projections().is_empty()
                && assignment.destination().projections().is_empty()
            {
                borrowed_locals.insert(assignment.destination().local().index());
            }
        }
    }
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let consumes_borrow = call.arguments().iter().any(|operand| {
            matches!(
                operand,
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                    if place.projections().is_empty()
                        && borrowed_locals.contains(&place.local().index())
            )
        });
        if !consumes_borrow {
            continue;
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            return "borrow passed to non-intrinsic call";
        };
        return match operation {
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. } => {
                "borrow for workgroup pipeline creation"
            }
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent { .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { .. } => {
                "borrow for workgroup pipeline operation"
            }
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. }
            | SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 { .. }
            | SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 { .. } => {
                "borrow for matrix load"
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. } => {
                "borrow for disjoint-slice operation"
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. } => {
                "borrow for index-witness operation"
            }
            SemanticCompilerIntrinsicOperationV1::MathF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum { .. }
            | SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum { .. }
            | SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum { .. }
            | SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { .. } => {
                "borrow for compiler capability operation"
            }
            _ => "borrow for compiler intrinsic",
        };
    }
    "borrow with no authenticated consumer"
}

fn option_component_witness_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    witness_type: SemanticTypeIdV1,
) -> Option<SemanticDisjointIndexSpaceV1> {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
            output_block,
            output_space,
            ..
        } if *output_block == witness_type => Some(*output_space),
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
            output_tile,
            output_space,
            ..
        } if *output_tile == witness_type => Some(*output_space),
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
            output_stripe,
            output_space,
            ..
        } if *output_stripe == witness_type => Some(*output_space),
        _ => None,
    }
}

fn direct_index_witness_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    witness_type: SemanticTypeIdV1,
) -> Option<SemanticPromotedBindingV1> {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { index_witness, .. }
            if *index_witness == witness_type =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                disjoint: false,
                availability: None,
            })
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
            output_witness,
            index_space,
            ..
        } if *output_witness == witness_type => Some(SemanticPromotedBindingV1::IndexWitness {
            index_space: *index_space,
            disjoint: true,
            availability: None,
        }),
        _ => None,
    }
}

fn option_capability_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    payload_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    let availability = SemanticCapabilityAvailabilityV1::Option(availability);
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
            output_witness,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space,
            offset,
            ..
        } if *output_witness == payload_type
            && *output_space
                == SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: *offset } =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: *output_space,
                disjoint: false,
                availability: Some(availability),
            })
        }
        SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
            output_witness,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space,
            offset,
            ..
        } if *output_witness == payload_type
            && *output_space
                == SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: *offset } =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: *output_space,
                disjoint: true,
                availability: Some(availability),
            })
        }
        SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader }
            if *grid_leader == payload_type =>
        {
            Some(SemanticPromotedBindingV1::GridLeader { availability })
        }
        _ => None,
    }
}

fn option_payload_type_v1(
    types: &[SemanticTypeDeclV1],
    option_type: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Enum { variants, .. } =
        types.get(option_type.index() as usize)?.shape()
    else {
        return None;
    };
    let [none, some] = &**variants else {
        return None;
    };
    if none.discriminant() != 0
        || some.discriminant() != 1
        || none.is_uninhabited()
        || some.is_uninhabited()
        || !none.fields().fields().is_empty()
    {
        return None;
    }
    let [payload] = some.fields().fields() else {
        return None;
    };
    Some(*payload)
}

fn exact_option_payload_projection_v1(
    types: &[SemanticTypeDeclV1],
    option_type: SemanticTypeIdV1,
    projections: &[SemanticProjectionV1],
    result_type: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let [downcast, field] = projections else {
        return None;
    };
    if downcast.kind() != SemanticProjectionKindV1::Downcast(1)
        || field.kind() != SemanticProjectionKindV1::Field(0)
    {
        return None;
    }
    let payload_type = option_payload_type_v1(types, option_type)?;
    // rustc's `PlaceTy` retains the enum type across a downcast and records the
    // active variant separately. Requiring a synthetic aggregate type here
    // rejects the canonical `((_option as Some).0)` projection produced by MIR.
    if downcast.result_type() != option_type
        || field.result_type() != payload_type
        || result_type != payload_type
    {
        return None;
    }
    Some(payload_type)
}

fn optionalized_capability_binding_v1(
    binding: SemanticPromotedBindingV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    match binding {
        SemanticPromotedBindingV1::IndexWitness {
            index_space,
            disjoint,
            availability: Some(SemanticCapabilityAvailabilityV1::Option(actual_availability)),
            ..
        } if actual_availability == availability => {
            Some(SemanticPromotedBindingV1::OptionIndexWitness {
                index_space,
                disjoint,
                availability,
            })
        }
        SemanticPromotedBindingV1::GridLeader {
            availability: SemanticCapabilityAvailabilityV1::Option(actual_availability),
        } if actual_availability == availability => {
            Some(SemanticPromotedBindingV1::OptionGridLeader { availability })
        }
        _ => None,
    }
}

fn optional_pointer_binding_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    operation: &SemanticCompilerIntrinsicOperationV1,
    payload_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    let (disjoint_slice, element) = match operation {
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
            disjoint_slice,
            element,
            ..
        } => (*disjoint_slice, *element),
        _ => return None,
    };
    let (authenticated_element, _, access) = disjoint_slice_descriptor(callables, disjoint_slice)?;
    if authenticated_element != element {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(payload_type.index() as usize)?.shape()
    else {
        return None;
    };
    if pointer.pointee() != element
        || pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return None;
    }
    let Type::Scalar(element) = lower_scalar_type(types, element).ok()? else {
        return None;
    };
    Some(SemanticPromotedBindingV1::OptionPointer {
        element,
        address_space: AddressSpace::Global,
        access,
        availability,
    })
}

#[derive(Clone, Copy)]
enum SemanticCapabilityDefinitionV1<'a> {
    Assignment(&'a SemanticRvalueKindV1),
    Call(&'a SemanticDirectCallV1),
}

#[derive(Clone, Copy)]
enum SemanticTransparentDefinitionV1 {
    Borrow {
        source: u32,
        referent_type: SemanticTypeIdV1,
    },
    ValueAlias {
        source: u32,
        source_type: SemanticTypeIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SemanticCapabilityOriginNodeV1 {
    Local(u32, u32),
    EnumPayload(u32, u32, u32, u32),
}

fn merge_capability_origin_v1(
    binding: &mut Option<SemanticPromotedBindingV1>,
    candidate: SemanticPromotedBindingV1,
) -> bool {
    if binding.is_some_and(|existing| existing != candidate) {
        return false;
    }
    *binding = Some(candidate);
    true
}

struct SemanticCapabilityOriginResolverV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    option_dominance: &'a SemanticOptionDominanceV1,
    certified_locals: &'a BTreeSet<u32>,
    definitions: BTreeMap<u32, Vec<SemanticCapabilityDefinitionV1<'a>>>,
    invalidated_locals: BTreeSet<u32>,
    memo: BTreeMap<SemanticCapabilityOriginNodeV1, Option<SemanticPromotedBindingV1>>,
    visiting: BTreeSet<SemanticCapabilityOriginNodeV1>,
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
    peak_storage: usize,
}

impl<'a> SemanticCapabilityOriginResolverV1<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        option_dominance: &'a SemanticOptionDominanceV1,
        certified_locals: &'a BTreeSet<u32>,
        max_analysis_work: usize,
        max_analysis_storage: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut resolver = Self {
            types,
            callables,
            function,
            option_dominance,
            certified_locals,
            definitions: BTreeMap::new(),
            invalidated_locals: BTreeSet::new(),
            memo: BTreeMap::new(),
            visiting: BTreeSet::new(),
            work_limit: max_analysis_work,
            storage_limit: max_analysis_storage,
            work: 0,
            storage: 0,
            peak_storage: 0,
        };
        for block in function.blocks() {
            resolver.charge_work(1)?;
            for statement in block.statements() {
                resolver.charge_work(1)?;
                resolver.index_statement(statement.kind())?;
            }
            resolver.charge_work(1)?;
            resolver.index_terminator(block.terminator().kind())?;
        }
        Ok(resolver)
    }

    fn push_definition(
        &mut self,
        local: u32,
        definition: SemanticCapabilityDefinitionV1<'a>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let amount = 1usize
            .checked_add(usize::from(!self.definitions.contains_key(&local)))
            .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: usize::MAX,
                limit: self.storage_limit,
            })?;
        self.charge_storage(amount)?;
        self.definitions.entry(local).or_default().push(definition);
        Ok(())
    }

    fn invalidate_local(&mut self, local: u32) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.invalidated_locals.contains(&local) {
            self.charge_storage(1)?;
            self.invalidated_locals.insert(local);
        }
        Ok(())
    }

    fn index_statement(
        &mut self,
        statement: &'a SemanticStatementKindV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                let local = assignment.destination().local().index();
                if assignment.destination().projections().is_empty() {
                    self.push_definition(
                        local,
                        SemanticCapabilityDefinitionV1::Assignment(assignment.value().kind()),
                    )?;
                } else {
                    self.invalidate_local(local)?;
                }
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } => {
                        self.invalidate_local(place.local().index())?;
                    }
                    _ => {}
                }
            }
            SemanticStatementKindV1::Store(store) => {
                self.invalidate_local(store.destination().local().index())?;
            }
            SemanticStatementKindV1::AtomicRmw(operation) => {
                self.invalidate_local(operation.destination().local().index())?;
                self.invalidate_local(operation.address().local().index())?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                self.invalidate_local(operation.destination().local().index())?;
                self.invalidate_local(operation.address().local().index())?;
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => {
                self.invalidate_local(place.local().index())?;
            }
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Assume(_)
            | SemanticStatementKindV1::Nop => {}
        }
        Ok(())
    }

    fn index_terminator(
        &mut self,
        terminator: &'a SemanticTerminatorKindV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination() {
                    let local = destination.place().local().index();
                    if destination.place().projections().is_empty() {
                        self.push_definition(local, SemanticCapabilityDefinitionV1::Call(call))?;
                    } else {
                        self.invalidate_local(local)?;
                    }
                }
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                self.invalidate_local(place.local().index())?;
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
        Ok(())
    }

    fn resolve(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let ty = self
            .function
            .locals()
            .get(local.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty();
        self.resolve_local(local, ty)
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work =
            self.work
                .checked_add(amount)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: usize::MAX,
                    limit: self.work_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisWork,
            self.work,
            self.work_limit,
        )
    }

    fn charge_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        let next = self.storage.checked_add(amount).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: usize::MAX,
                limit: self.storage_limit,
            },
        )?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            next,
            self.storage_limit,
        )?;
        self.storage = next;
        self.peak_storage = self.peak_storage.max(next);
        Ok(())
    }

    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.storage = self.storage.checked_sub(amount).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "capability SSA provenance storage accounting underflow",
            )
        })?;
        Ok(())
    }

    fn definition_count(&self, local: SemanticLocalIdV1) -> usize {
        self.definitions.get(&local.index()).map_or(0, Vec::len)
    }

    fn definition(
        &self,
        local: SemanticLocalIdV1,
        index: usize,
    ) -> Option<SemanticCapabilityDefinitionV1<'a>> {
        self.definitions
            .get(&local.index())
            .and_then(|definitions| definitions.get(index))
            .copied()
    }

    fn resolve_local(
        &mut self,
        local: SemanticLocalIdV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let node = SemanticCapabilityOriginNodeV1::Local(local.index(), expected_type.index());
        self.resolve_node(node, |resolver| {
            if !resolver.certified_locals.contains(&local.index())
                || resolver.invalidated_locals.contains(&local.index())
            {
                return Ok(None);
            }
            let Some(declaration) = resolver.function.locals().get(local.index() as usize) else {
                return Ok(None);
            };
            if declaration.ty() != expected_type {
                return Ok(None);
            }
            let definition_count = resolver.definition_count(local);
            if definition_count == 0 {
                return Ok(None);
            }
            let mut binding = None;
            for index in 0..definition_count {
                resolver.charge_work(1)?;
                let Some(definition) = resolver.definition(local, index) else {
                    return Ok(None);
                };
                let candidate = match definition {
                    SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                        operand,
                    )) => resolver.resolve_operand(operand, expected_type)?,
                    SemanticCapabilityDefinitionV1::Assignment(_) => None,
                    SemanticCapabilityDefinitionV1::Call(call) => {
                        resolver.resolve_direct_call(local, expected_type, call)
                    }
                };
                let Some(candidate) = candidate else {
                    return Ok(None);
                };
                if !merge_capability_origin_v1(&mut binding, candidate) {
                    return Ok(None);
                }
            }
            Ok(binding)
        })
    }

    fn resolve_operand(
        &mut self,
        operand: &SemanticOperandV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let place = match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
            SemanticOperandV1::Constant(_) => return Ok(None),
        };
        self.resolve_place(place, expected_type)
    }

    fn resolve_place(
        &mut self,
        place: &SemanticPlaceV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        if place.ty() != expected_type {
            return Ok(None);
        }
        match place.projections() {
            [] => self.resolve_local(place.local(), expected_type),
            [downcast, field] => {
                let SemanticProjectionKindV1::Downcast(variant) = downcast.kind() else {
                    return Ok(None);
                };
                let SemanticProjectionKindV1::Field(field) = field.kind() else {
                    return Ok(None);
                };
                self.resolve_enum_payload(place.local(), variant, field, expected_type)
            }
            _ => Ok(None),
        }
    }

    fn resolve_enum_payload(
        &mut self,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let node = SemanticCapabilityOriginNodeV1::EnumPayload(
            local.index(),
            variant,
            field,
            expected_type.index(),
        );
        self.resolve_node(node, |resolver| {
            if !resolver.certified_locals.contains(&local.index())
                || resolver.invalidated_locals.contains(&local.index())
            {
                return Ok(None);
            }
            let Some(enum_type) = resolver
                .function
                .locals()
                .get(local.index() as usize)
                .map(|declaration| declaration.ty())
            else {
                return Ok(None);
            };
            let Some(SemanticTypeShapeV1::Enum { variants, .. }) = resolver
                .types
                .get(enum_type.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(None);
            };
            if variants
                .get(variant as usize)
                .and_then(|variant| variant.fields().fields().get(field as usize))
                != Some(&expected_type)
            {
                return Ok(None);
            }
            let Some(expected_arity) = variants
                .get(variant as usize)
                .map(|variant| variant.fields().fields().len())
            else {
                return Ok(None);
            };
            let definition_count = resolver.definition_count(local);
            if definition_count == 0 {
                return Ok(None);
            }
            let mut binding = None;
            for index in 0..definition_count {
                resolver.charge_work(1)?;
                let Some(definition) = resolver.definition(local, index) else {
                    return Ok(None);
                };
                let candidate = match definition {
                    SemanticCapabilityDefinitionV1::Assignment(
                        SemanticRvalueKindV1::Aggregate(aggregate),
                    ) => match aggregate.kind() {
                        SemanticAggregateKindV1::EnumVariant(actual)
                            if *actual == variant
                                && aggregate.operands().len() == expected_arity =>
                        {
                            let Some(operand) = aggregate.operands().get(field as usize) else {
                                return Ok(None);
                            };
                            resolver.resolve_operand(operand, expected_type)?
                        }
                        SemanticAggregateKindV1::EnumVariant(_) => continue,
                        SemanticAggregateKindV1::Array
                        | SemanticAggregateKindV1::Tuple
                        | SemanticAggregateKindV1::Aggregate => None,
                    },
                    SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                        operand,
                    )) => {
                        let place = match operand {
                            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                                if place.ty() == enum_type && place.projections().is_empty() =>
                            {
                                place
                            }
                            SemanticOperandV1::Copy(_)
                            | SemanticOperandV1::Move(_)
                            | SemanticOperandV1::Constant(_) => return Ok(None),
                        };
                        resolver.resolve_enum_payload(
                            place.local(),
                            variant,
                            field,
                            expected_type,
                        )?
                    }
                    SemanticCapabilityDefinitionV1::Assignment(_) => None,
                    SemanticCapabilityDefinitionV1::Call(call) => resolver
                        .resolve_option_payload_call(
                            local,
                            enum_type,
                            variant,
                            field,
                            expected_type,
                            call,
                        ),
                };
                let Some(candidate) = candidate else {
                    return Ok(None);
                };
                if !merge_capability_origin_v1(&mut binding, candidate) {
                    return Ok(None);
                }
            }
            Ok(binding)
        })
    }

    fn resolve_option_payload_call(
        &self,
        local: SemanticLocalIdV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        payload_type: SemanticTypeIdV1,
        call: &SemanticDirectCallV1,
    ) -> Option<SemanticPromotedBindingV1> {
        if variant != 1
            || field != 0
            || option_payload_type_v1(self.types, enum_type) != Some(payload_type)
        {
            return None;
        }
        let availability = self.option_dominance.availability(local)?;
        let operation = self.compiler_intrinsic(call)?;
        option_capability_contract_v1(operation, payload_type, availability).or_else(|| {
            option_component_witness_contract_v1(operation, payload_type).map(|index_space| {
                SemanticPromotedBindingV1::ComponentWitness {
                    index_space,
                    availability: SemanticCapabilityAvailabilityV1::Option(availability),
                }
            })
        })
    }

    fn resolve_direct_call(
        &self,
        local: SemanticLocalIdV1,
        result_type: SemanticTypeIdV1,
        call: &SemanticDirectCallV1,
    ) -> Option<SemanticPromotedBindingV1> {
        let operation = self.compiler_intrinsic(call)?;
        if let (Some(availability), Some(payload_type)) = (
            self.option_dominance.availability(local),
            option_payload_type_v1(self.types, result_type),
        ) {
            return option_capability_contract_v1(operation, payload_type, availability)
                .and_then(|binding| optionalized_capability_binding_v1(binding, availability))
                .or_else(|| {
                    option_component_witness_contract_v1(operation, payload_type).map(
                        |index_space| SemanticPromotedBindingV1::OptionComponentWitness {
                            index_space,
                            availability,
                        },
                    )
                })
                .or_else(|| {
                    optional_pointer_binding_v1(
                        self.types,
                        self.callables,
                        operation,
                        payload_type,
                        availability,
                    )
                });
        }
        direct_index_witness_contract_v1(operation, result_type)
    }

    fn transparent_definition(
        &self,
        local: SemanticLocalIdV1,
    ) -> Option<SemanticTransparentDefinitionV1> {
        if self.invalidated_locals.contains(&local.index()) || self.definition_count(local) != 1 {
            return None;
        }
        match self.definition(local, 0)? {
            SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Borrow {
                place,
                ..
            }) if place.projections().is_empty() => Some(SemanticTransparentDefinitionV1::Borrow {
                source: place.local().index(),
                referent_type: place.ty(),
            }),
            SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            )) if place.projections().is_empty() => {
                Some(SemanticTransparentDefinitionV1::ValueAlias {
                    source: place.local().index(),
                    source_type: place.ty(),
                })
            }
            SemanticCapabilityDefinitionV1::Assignment(_)
            | SemanticCapabilityDefinitionV1::Call(_) => None,
        }
    }

    fn compiler_intrinsic(
        &self,
        call: &SemanticDirectCallV1,
    ) -> Option<&SemanticCompilerIntrinsicOperationV1> {
        match self.callables.get(call.callee().index() as usize)? {
            SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => Some(operation),
            SemanticCallableDeclV1::Defined { .. }
            | SemanticCallableDeclV1::DeviceFfiImport { .. } => None,
        }
    }

    fn resolve_node(
        &mut self,
        node: SemanticCapabilityOriginNodeV1,
        resolve: impl FnOnce(
            &mut Self,
        )
            -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1>,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        self.charge_work(1)?;
        if let Some(cached) = self.memo.get(&node) {
            return Ok(*cached);
        }
        if self.visiting.contains(&node) {
            return Ok(None);
        }
        self.charge_storage(1)?;
        self.visiting.insert(node);
        let result = resolve(self);
        self.visiting.remove(&node);
        self.release_storage(1)?;
        let result = result?;
        self.charge_storage(1)?;
        self.memo.insert(node, result);
        Ok(result)
    }
}

#[cfg(test)]
fn promoted_capability_binding_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    option_dominance: &SemanticOptionDominanceV1,
    certified_locals: &BTreeSet<u32>,
    local: u32,
) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
    if function.locals().get(local as usize).is_none() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    SemanticCapabilityOriginResolverV1::new(
        types,
        callables,
        function,
        option_dominance,
        certified_locals,
        usize::MAX,
        usize::MAX,
    )?
    .resolve(SemanticLocalIdV1::from_index(local))
}

fn promoted_transport_descriptor_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    local: u32,
    compiler_issued_bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    shared_promoted: &BTreeSet<u32>,
    capability_origins: &mut SemanticCapabilityOriginResolverV1<'_>,
    direct_parameters: &BTreeMap<u32, Type>,
) -> Result<(SemanticTypeIdV1, SemanticPromotedTransportV1), ProductionSemanticKirErrorV1> {
    let mut current = local;
    let mut visited = BTreeSet::new();
    loop {
        capability_origins.charge_work(1)?;
        if !visited.insert(current) || visited.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "transparent-borrow SSA transport is cyclic or too deep",
            ));
        }
        let declaration = function
            .locals()
            .get(current as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if let Some(binding) = compiler_issued_bindings.get(&declaration.ty()).copied() {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(binding),
            ));
        }
        if let Some(binding) = capability_origins.resolve(SemanticLocalIdV1::from_index(current))? {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(binding),
            ));
        }
        if matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
            && direct_parameters.contains_key(&current)
        {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::DirectParameter {
                    parameter_local: current,
                },
            ));
        }

        let Some(definition) =
            capability_origins.transparent_definition(SemanticLocalIdV1::from_index(current))
        else {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
            ));
        };
        let source = match definition {
            SemanticTransparentDefinitionV1::Borrow { source, .. }
            | SemanticTransparentDefinitionV1::ValueAlias { source, .. } => source,
        };
        if !shared_promoted.contains(&source) {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
            ));
        }
        match definition {
            SemanticTransparentDefinitionV1::Borrow { referent_type, .. } => {
                let Some(reference) =
                    types
                        .get(declaration.ty().index() as usize)
                        .and_then(|declaration| match declaration.shape() {
                            SemanticTypeShapeV1::Pointer(pointer) => Some(pointer),
                            _ => None,
                        })
                else {
                    return Ok((
                        declaration.ty(),
                        SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
                    ));
                };
                if reference.pointee() != referent_type {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "transparent-borrow SSA transport pointee type changed",
                    ));
                }
            }
            SemanticTransparentDefinitionV1::ValueAlias { source_type, .. } => {
                if declaration.ty() != source_type {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "transparent value-alias SSA transport type changed",
                    ));
                }
            }
        }
        current = source;
    }
}

impl SemanticControlFlowSsaPlanV1 {
    fn analyze(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
        option_dominance: &SemanticOptionDominanceV1,
        direct_parameters: &BTreeMap<u32, Type>,
        max_analysis_work: usize,
        max_analysis_storage: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if semantic_ssa.function() != semantic_function
            || semantic_ssa.function_identity() != function.identity()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let compiler_issued_bindings =
            compiler_issued_ssa_bindings_v1(types, callables, function, semantic_function)?;
        let shared = semantic_ssa.plan();
        let retained_cross_edge = semantic_ssa
            .retained_cross_edge_variables()
            .iter()
            .map(|local| local.get())
            .collect::<BTreeSet<_>>();
        let shared_promoted = shared
            .promoted_variables()
            .iter()
            .map(|variable| variable.get())
            .collect::<BTreeSet<_>>();
        let private_slot_candidates =
            private_slot_candidate_locals_v1(function, &shared_promoted, &retained_cross_edge);
        let mut retained_local_slots = BTreeMap::new();
        let mut unsupported_retained_locals = Vec::new();
        for local in private_slot_candidates {
            let declaration = function
                .locals()
                .get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let Some((kernel_type, alignment)) =
                retained_local_slot_type_v1(types, declaration.ty())
            else {
                unsupported_retained_locals.push((local, declaration.ty().index()));
                continue;
            };
            retained_local_slots.insert(
                local,
                SemanticRetainedLocalSlotPlanV1 {
                    semantic_type: declaration.ty(),
                    kernel_type,
                    alignment,
                },
            );
        }
        if !unsupported_retained_locals.is_empty() {
            const MAX_RETAINED_LOCAL_DIAGNOSTICS_V1: usize = 32;
            let retained_count = unsupported_retained_locals.len();
            unsupported_retained_locals.truncate(MAX_RETAINED_LOCAL_DIAGNOSTICS_V1);
            let unsupported_retained_locals = unsupported_retained_locals
                .into_iter()
                .map(|(local, ty)| {
                    (
                        local,
                        ty,
                        first_retained_local_cause_v1(
                            function,
                            local,
                            callables,
                            &compiler_issued_bindings,
                        ),
                    )
                })
                .collect();
            return Err(ProductionSemanticKirErrorV1::RetainedLocalStorage {
                function: semantic_function.index(),
                retained_locals: unsupported_retained_locals,
                retained_count,
            });
        }
        let implicit_entry_locals = semantic_ssa
            .implicit_entry_variables()
            .iter()
            .map(|variable| variable.get())
            .collect::<BTreeSet<_>>();
        if implicit_entry_locals.len() != semantic_ssa.implicit_entry_variables().len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for local in &implicit_entry_locals {
            let declaration = function
                .locals()
                .get(*local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !shared_promoted.contains(local)
                || declaration.role() != SemanticLocalRoleV1::Temporary
                || !authenticated_ambient_workgroup_lds_scope_zst_v1(
                    types,
                    callables,
                    declaration.ty(),
                )
                || compiler_issued_bindings.get(&declaration.ty())
                    != Some(&SemanticPromotedBindingV1::WorkgroupLdsScope)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        let entry = function.entry().index();
        if shared
            .transport_variables(SsaBlockIdV1::new(entry))
            .is_some_and(|variables| !variables.is_empty())
        {
            return Err(unsupported(
                semantic_function.index(),
                Some(entry),
                None,
                "cyclic entry SSA requires a synthetic Kernel IR preheader",
            ));
        }
        let mut transported = BTreeSet::new();
        for block in shared.reverse_postorder() {
            if block.get() == entry {
                continue;
            }
            let variables = shared
                .transport_variables(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            for variable in variables {
                if !shared_promoted.contains(&variable.get()) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                transported.insert(variable.get());
            }
        }
        let mut capability_origins = SemanticCapabilityOriginResolverV1::new(
            types,
            callables,
            function,
            option_dominance,
            &shared_promoted,
            max_analysis_work,
            max_analysis_storage,
        )?;
        let mut promoted = BTreeMap::new();
        for local in transported {
            let declaration = function
                .locals()
                .get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let (transport_semantic_type, binding) = promoted_transport_descriptor_v1(
                types,
                function,
                local,
                &compiler_issued_bindings,
                &shared_promoted,
                &mut capability_origins,
                direct_parameters,
            )?;
            let kernel_types =
                binding.transport_types(types, transport_semantic_type, direct_parameters)?;
            if kernel_types.is_empty()
                && !matches!(
                    binding,
                    SemanticPromotedTransportV1::Semantic(
                        SemanticPromotedBindingV1::MathContext
                            | SemanticPromotedBindingV1::CollectiveContext
                            | SemanticPromotedBindingV1::WorkgroupLdsScope
                            | SemanticPromotedBindingV1::MatrixContext
                            | SemanticPromotedBindingV1::GridLeader { .. }
                    )
                )
            {
                return Err(unsupported(
                    semantic_function.index(),
                    None,
                    None,
                    "SSA transport produced no Kernel IR block-parameter types",
                ));
            }
            promoted.insert(
                local,
                SemanticPromotedLocalV1 {
                    semantic_type: declaration.ty(),
                    transport_semantic_type,
                    transport: binding,
                    kernel_types: kernel_types.into_boxed_slice(),
                },
            );
        }
        let mut live_in = BTreeMap::new();
        for block in 0..function.blocks().len() as u32 {
            let block_id = SsaBlockIdV1::new(block);
            let locals = if block == entry || !shared.is_reachable(block_id) {
                Vec::new()
            } else {
                shared
                    .transport_variables(block_id)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .map(|variable| variable.get())
                    .filter(|local| promoted.contains_key(local))
                    .collect()
            };
            live_in.insert(block, locals);
        }
        let mut edge_arguments = BTreeMap::new();
        let mut edge_definitions = BTreeMap::new();
        for block in shared.reverse_postorder() {
            let source = function
                .blocks()
                .get(block.get() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            for ordinal in 0..source.terminator().kind().edge_count() {
                let edge = fe2o3_mir_model::SsaEdgeIdV1::new(*block, ordinal as u32);
                let arguments = shared
                    .edge_arguments(edge)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .copied()
                    .filter(|argument| promoted.contains_key(&argument.variable().get()))
                    .collect();
                let definitions = shared
                    .edge_definitions(edge)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .copied()
                    .collect();
                edge_definitions.insert((block.get(), ordinal as u32), definitions);
                edge_arguments.insert((block.get(), ordinal as u32), arguments);
            }
        }

        let entry_definitions = shared
            .entry_definitions()
            .iter()
            .copied()
            .map(|definition| (definition.variable().get(), definition.value()))
            .collect();
        let mut block_entry_values = BTreeMap::new();
        let mut definition_values = BTreeMap::<(u32, u32), Vec<SsaValueV1>>::new();
        for block in shared.reverse_postorder() {
            let mut seen = BTreeSet::new();
            for (_, event) in shared
                .resolved_events(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            {
                let (variable, entry_value, definition) = match *event {
                    SsaResolvedEventV1::Use { variable, value } => (variable, Some(value), None),
                    SsaResolvedEventV1::Define { variable, value } => (variable, None, Some(value)),
                    SsaResolvedEventV1::Kill { variable, previous } => (variable, previous, None),
                };
                let local = variable.get();
                if !shared_promoted.contains(&local) {
                    continue;
                }
                if seen.insert(local)
                    && let Some(value) = entry_value
                {
                    block_entry_values.insert((block.get(), local), value);
                }
                if let Some(value) = definition {
                    definition_values
                        .entry((block.get(), local))
                        .or_default()
                        .push(value);
                }
            }
        }
        let reachable = shared
            .reverse_postorder()
            .iter()
            .map(|block| block.get())
            .collect::<BTreeSet<_>>();
        let retained_initialized_at_entry = retained_local_initialization_entries_v1(
            function,
            &retained_local_slots,
            &reachable,
            max_analysis_work,
            max_analysis_storage,
        )?;
        Ok(Self {
            compiler_issued_bindings,
            implicit_entry_locals,
            ssa_value_locals: shared_promoted,
            promoted,
            live_in,
            block_entry_values,
            entry_definitions,
            definition_values,
            edge_definitions,
            edge_arguments,
            retained_local_slots,
            retained_initialized_at_entry,
        })
    }

    fn live_in(&self, block: u32) -> &[u32] {
        self.live_in.get(&block).map_or(&[], Vec::as_slice)
    }

    fn entry_value(
        &self,
        function: &SemanticFunctionDeclV1,
        block: u32,
        local: u32,
    ) -> Option<SsaValueV1> {
        if block == function.entry().index() {
            return self.entry_definitions.get(&local).copied();
        }
        if self.live_in(block).contains(&local) {
            return Some(SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(block),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
            });
        }
        self.block_entry_values.get(&(block, local)).copied()
    }
}
