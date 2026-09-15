// Both adapters use the same queue order and single-origin rule. The legacy
// adapter keeps its existing one-step-per-pop budget and owned scratch policy.
trait ExternalAllocationWorklistV1 {
    type Error;

    fn pop(&mut self) -> Result<Option<ValueId>, Self::Error>;
    fn first_visit(&mut self, value: ValueId) -> Result<bool, Self::Error>;
    fn expand(&mut self, value: ValueId) -> Result<ExternalAllocationStepV1, Self::Error>;
}

enum ExternalAllocationStepV1 {
    Parameter(u32),
    Expanded,
    Unsupported,
}

fn external_allocation_worklist_v1<W: ExternalAllocationWorklistV1>(
    worklist: &mut W,
) -> Result<Option<u32>, W::Error> {
    let mut origin = None;
    while let Some(value) = worklist.pop()? {
        if !worklist.first_visit(value)? {
            continue;
        }
        match worklist.expand(value)? {
            ExternalAllocationStepV1::Parameter(parameter) => match origin {
                None => origin = Some(parameter),
                Some(previous) if previous == parameter => {}
                Some(_) => return Ok(None),
            },
            ExternalAllocationStepV1::Expanded => {}
            ExternalAllocationStepV1::Unsupported => return Ok(None),
        }
    }
    Ok(origin)
}

fn external_allocation_operation_inputs_v1(operation: &Operation) -> Option<[Option<ValueId>; 2]> {
    match &operation.kind {
        OperationKind::SliceData { slice } => Some([Some(*slice), None]),
        OperationKind::GetElementPointer { base, .. } => Some([Some(*base), None]),
        OperationKind::Cast { value, .. } => Some([Some(*value), None]),
        OperationKind::Select {
            true_value,
            false_value,
            ..
        } => Some([Some(*true_value), Some(*false_value)]),
        _ => None,
    }
}

struct LegacyExternalAllocationWorklistV1<'a, 'module> {
    function: &'a Function,
    body: &'a FunctionBody,
    kir: &'a KirCorrelationIndexV1<'module>,
    visited: &'a mut BTreeSet<ValueId>,
    pending: Vec<ValueId>,
    budget: &'a mut UnsupportedIndexCorrelationBudgetV1,
}

impl ExternalAllocationWorklistV1 for LegacyExternalAllocationWorklistV1<'_, '_> {
    type Error = ();

    fn pop(&mut self) -> Result<Option<ValueId>, ()> {
        let value = self.pending.pop();
        if value.is_some() {
            self.budget.charge().ok_or(())?;
        }
        Ok(value)
    }

    fn first_visit(&mut self, value: ValueId) -> Result<bool, ()> {
        Ok(self.visited.insert(value))
    }

    fn expand(&mut self, value: ValueId) -> Result<ExternalAllocationStepV1, ()> {
        if let Some(index) = self
            .body
            .parameters
            .iter()
            .position(|parameter| *parameter == value)
        {
            let ty = self.function.signature.parameters.get(index).ok_or(())?;
            if !matches!(ty, Type::Pointer(_) | Type::Slice(_)) {
                return Ok(ExternalAllocationStepV1::Unsupported);
            }
            return Ok(ExternalAllocationStepV1::Parameter(
                u32::try_from(index).map_err(|_| ())?,
            ));
        }
        if let Some(operation) = self.kir.definitions.get(&value) {
            let Some(inputs) = external_allocation_operation_inputs_v1(operation) else {
                return Ok(ExternalAllocationStepV1::Unsupported);
            };
            self.pending.extend(inputs.into_iter().flatten());
            return Ok(ExternalAllocationStepV1::Expanded);
        }
        let inputs = self.kir.block_parameter_inputs.get(&value).ok_or(())?;
        if inputs.is_empty() {
            return Ok(ExternalAllocationStepV1::Unsupported);
        }
        self.pending.extend(inputs.iter().copied());
        Ok(ExternalAllocationStepV1::Expanded)
    }
}

struct SourceOutputAllocationScratchV1 {
    pending: Vec<ValueId>,
    visited: Vec<usize>,
    incoming: Vec<usize>,
    next: Vec<usize>,
    pending_limit: usize,
    generation: usize,
    storage: usize,
}

// The allocating caller prepays six bookkeeping units before allocating.
// Reconcile its capacity before any other fallible allocation or work charge.
fn source_output_allocation_capacity_storage_v1(
    requested: usize,
    actual: usize,
    element_bytes: usize,
    storage: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let excess = actual
        .checked_sub(requested)
        .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?
        .checked_mul(element_bytes)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let retained = storage
        .checked_add(excess)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.reserve_storage(excess).map_err(Error::Resource)?;
    Ok(retained)
}

fn source_output_allocation_scratch_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputAllocationScratchV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(12).map_err(Error::Resource)?;
    let definitions = inventory.definitions().len();
    let edges = inventory.edge_arguments().len();
    let numeric = definitions
        .checked_mul(2)
        .and_then(|n| n.checked_add(edges))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let pending_limit = numeric
        .checked_add(1)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let mut storage = numeric
        .checked_mul(std::mem::size_of::<usize>())
        .and_then(|n| {
            pending_limit
                .checked_mul(std::mem::size_of::<ValueId>())
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(std::mem::size_of::<SourceOutputAllocationScratchV1>()))
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.reserve_storage(storage).map_err(Error::Resource)?;
    let mut pending = Vec::new();
    let mut visited = Vec::new();
    let mut incoming = Vec::new();
    let mut next = Vec::new();
    budget.charge_work(7).map_err(Error::Resource)?;
    pending
        .try_reserve_exact(pending_limit)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
    storage = source_output_allocation_capacity_storage_v1(
        pending_limit,
        pending.capacity(),
        std::mem::size_of::<ValueId>(),
        storage,
        budget,
    )?;
    for (values, count) in [
        (&mut visited, definitions),
        (&mut incoming, definitions),
        (&mut next, edges),
    ] {
        budget.charge_work(7).map_err(Error::Resource)?;
        values
            .try_reserve_exact(count)
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
        storage = source_output_allocation_capacity_storage_v1(
            count,
            values.capacity(),
            std::mem::size_of::<usize>(),
            storage,
            budget,
        )?;
    }
    budget.charge_work(numeric).map_err(Error::Resource)?;
    visited.resize(definitions, 0);
    incoming.resize(definitions, usize::MAX);
    next.resize(edges, usize::MAX);
    // Reverse construction preserves the old forward predecessor/argument
    // encounter order while allowing each phi's inputs to be visited directly.
    for (ordinal, argument) in inventory.edge_arguments().iter().enumerate().rev() {
        budget.charge_work(7).map_err(Error::Resource)?;
        let target = inventory
            .definitions()
            .get(argument.target_definition)
            .ok_or(Error::Invalid("global incoming definition absent"))?;
        if !matches!(
            target.coordinate,
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { .. }
        ) {
            return Err(Error::Invalid(
                "global incoming target is not a block argument",
            ));
        }
        let head = incoming
            .get_mut(argument.target_definition)
            .ok_or(Error::Invalid("global incoming definition ordinal absent"))?;
        next[ordinal] = *head;
        *head = ordinal;
    }
    Ok(SourceOutputAllocationScratchV1 {
        pending,
        visited,
        incoming,
        next,
        pending_limit,
        generation: 0,
        storage,
    })
}

fn source_output_definition_ordinal_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    budget.charge_work(2).map_err(Error::Resource)?;
    let (start, ordinal) = match coordinate {
        Definition::FunctionArgument {
            function: owner,
            argument,
        } => {
            if owner != function.coordinate {
                return Err(Error::Invalid("global definition function changed"));
            }
            (function.definitions.start, argument)
        }
        Definition::BlockArgument { block, argument } => {
            budget.charge_work(4).map_err(Error::Resource)?;
            let index = function
                .blocks
                .start
                .checked_add(block.block as usize)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            let row = inventory
                .blocks()
                .get(index)
                .ok_or(Error::Invalid("global definition block absent"))?;
            if block.function != function.coordinate || row.coordinate != block {
                return Err(Error::Invalid("global definition block changed"));
            }
            (row.parameters.start, argument)
        }
        Definition::Result { operation, result } => {
            budget.charge_work(7).map_err(Error::Resource)?;
            let index = function
                .blocks
                .start
                .checked_add(operation.block.block as usize)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            let block = inventory
                .blocks()
                .get(index)
                .ok_or(Error::Invalid("global definition block absent"))?;
            let index = block
                .operations
                .start
                .checked_add(operation.operation as usize)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            let row = inventory
                .operations()
                .get(index)
                .ok_or(Error::Invalid("global definition operation absent"))?;
            if operation.block.function != function.coordinate || row.coordinate != operation {
                return Err(Error::Invalid("global definition operation changed"));
            }
            (row.results.start, result)
        }
    };
    budget.charge_work(4).map_err(Error::Resource)?;
    let index = start
        .checked_add(ordinal as usize)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    let row = inventory
        .definitions()
        .get(index)
        .ok_or(Error::Invalid("global dense definition absent"))?;
    if !function.definitions.contains(&index) || row.coordinate != coordinate {
        return Err(Error::Invalid("global dense definition changed"));
    }
    Ok(index)
}

struct SourceOutputAllocationWorklistV1<'a, 'graph, 'ledger, 'limit> {
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV1<'graph>,
    function: &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'graph>,
    scratch: &'ledger mut SourceOutputAllocationScratchV1,
    current: Option<(
        usize,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'graph>,
    )>,
    budget: &'ledger mut AssertOriginBudgetV1<'limit>,
}

impl SourceOutputAllocationWorklistV1<'_, '_, '_, '_> {
    fn push(&mut self, value: ValueId) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.budget.charge_work(2).map_err(Error::Resource)?;
        if self.scratch.pending.len() >= self.scratch.pending_limit {
            return Err(Error::Invalid("global allocation queue bound"));
        }
        self.scratch.pending.push(value);
        Ok(())
    }
}

impl ExternalAllocationWorklistV1 for SourceOutputAllocationWorklistV1<'_, '_, '_, '_> {
    type Error = ProductionSourceOutputErrorV1;

    fn pop(&mut self) -> Result<Option<ValueId>, Self::Error> {
        self.budget.charge_work(1).map_err(Self::Error::Resource)?;
        Ok(self.scratch.pending.pop())
    }

    fn first_visit(&mut self, value: ValueId) -> Result<bool, Self::Error> {
        let definition = self
            .inventory
            .definition_for_value(self.function.coordinate, value, self.budget)
            .map_err(Self::Error::Inventory)?
            .ok_or(Self::Error::Invalid("global allocation definition absent"))?;
        let ordinal = source_output_definition_ordinal_v1(
            self.inventory,
            self.function,
            definition.coordinate,
            self.budget,
        )?;
        self.budget.charge_work(4).map_err(Self::Error::Resource)?;
        let visited = self
            .scratch
            .visited
            .get_mut(ordinal)
            .ok_or(Self::Error::Invalid("global visited definition absent"))?;
        if *visited == self.scratch.generation {
            return Ok(false);
        }
        *visited = self.scratch.generation;
        self.current = Some((ordinal, definition));
        Ok(true)
    }

    fn expand(&mut self, value: ValueId) -> Result<ExternalAllocationStepV1, Self::Error> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        self.budget.charge_work(2).map_err(Self::Error::Resource)?;
        let (ordinal, definition) = self
            .current
            .ok_or(Self::Error::Invalid("global current definition absent"))?;
        if definition.value != Some(value) {
            return Err(Self::Error::Invalid("global current definition changed"));
        }
        let coordinate = definition.coordinate;
        match coordinate {
            Definition::FunctionArgument { function, argument } => {
                if function != self.function.coordinate
                    || !matches!(definition.ty, Type::Pointer(_) | Type::Slice(_))
                {
                    return Ok(ExternalAllocationStepV1::Unsupported);
                }
                Ok(ExternalAllocationStepV1::Parameter(argument))
            }
            Definition::Result { operation, .. } => {
                let operation =
                    source_output_operation_v1(self.inventory.owner(), operation, self.budget)?;
                self.budget.charge_work(1).map_err(Self::Error::Resource)?;
                let Some(inputs) = external_allocation_operation_inputs_v1(operation) else {
                    return Ok(ExternalAllocationStepV1::Unsupported);
                };
                for input in inputs.into_iter().flatten() {
                    self.push(input)?;
                }
                Ok(ExternalAllocationStepV1::Expanded)
            }
            Definition::BlockArgument { .. } => {
                let mut found = false;
                self.budget.charge_work(1).map_err(Self::Error::Resource)?;
                let mut edge = *self
                    .scratch
                    .incoming
                    .get(ordinal)
                    .ok_or(Self::Error::Invalid("global incoming head absent"))?;
                while edge != usize::MAX {
                    self.budget.charge_work(4).map_err(Self::Error::Resource)?;
                    let argument =
                        self.inventory
                            .edge_arguments()
                            .get(edge)
                            .ok_or(Self::Error::Invalid(
                                "global allocation edge argument absent",
                            ))?;
                    if argument.target_definition != ordinal {
                        return Err(Self::Error::Invalid("global incoming target changed"));
                    }
                    let next = *self
                        .scratch
                        .next
                        .get(edge)
                        .ok_or(Self::Error::Invalid("global incoming link absent"))?;
                    if next != usize::MAX && next <= edge {
                        return Err(Self::Error::Invalid("global incoming link order changed"));
                    }
                    self.push(argument.value)?;
                    edge = next;
                    found = true;
                }
                Ok(if found {
                    ExternalAllocationStepV1::Expanded
                } else {
                    ExternalAllocationStepV1::Unsupported
                })
            }
        }
    }
}

fn source_output_allocation_parameter_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    value: ValueId,
    scratch: &mut SourceOutputAllocationScratchV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Option<u32>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(4).map_err(Error::Resource)?;
    let function = inventory
        .functions()
        .get(function.0 as usize)
        .ok_or(Error::Invalid("global allocation function absent"))?;
    scratch.generation = scratch
        .generation
        .checked_add(1)
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    // clear() changes only length for this Copy element; it neither walks
    // the whole definition roster nor resets accepted work or peak history.
    scratch.pending.clear();
    let mut worklist = SourceOutputAllocationWorklistV1 {
        inventory,
        function,
        scratch,
        current: None,
        budget,
    };
    worklist.push(value)?;
    external_allocation_worklist_v1(&mut worklist)
}
