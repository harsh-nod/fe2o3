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
