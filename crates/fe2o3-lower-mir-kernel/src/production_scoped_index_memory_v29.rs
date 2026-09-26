// MemorySSA locates possible reaching effects; exact physical slot membership
// and full scalar writes are checked here, not inferred from a memory version.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18, CanonicalKirMemorySsaErrorV1,
    CanonicalKirMemorySsaNodeIdV1 as IndexMemoryNodeId,
    CanonicalKirMemorySsaNodeV1 as IndexMemoryNode, CanonicalKirMemorySsaV18,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as IndexDefinition,
    CanonicalKirFunctionCoordinateV1 as IndexFunction,
    CanonicalKirOperationCoordinateV1 as IndexOperation,
};

#[derive(Clone, Copy)]
enum IndexMemorySeedV29 {
    Pending,
    Unknown,
    Write { value: ValueId, scalar: ScalarType },
}

struct IndexMemoryEquationV29 {
    slot: usize,
    node: IndexMemoryNodeId,
    seed: IndexMemorySeedV29,
    inputs: std::ops::Range<usize>,
}

#[derive(Clone, Copy)]
enum IndexGuardSeedV29 {
    Loaded { block: BlockId, gap: usize },
    Success { block: BlockId },
}

pub(super) struct SourceIndexMemoryV29<'view, 'inventory, 'graph> {
    inventory: &'inventory CanonicalKirInventoryV18<'graph>,
    memory: Option<&'view CanonicalKirMemorySsaV18<'inventory, 'graph>>,
    physical: &'view SourceAddressMemoryV29<'graph>,
    function: IndexFunction,
    slots: &'view [ScopedSourceSlotV29],
    accesses: &'view [SourceAddressAccessV29],
    equations: Vec<IndexMemoryEquationV29>,
    inputs: Vec<usize>,
    index: BTreeMap<(usize, usize), usize>,
    bounds: BTreeMap<(usize, usize, u64), bool>,
    expanded: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
}

fn index_memory_error_v29(error: CanonicalKirMemorySsaErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        CanonicalKirMemorySsaErrorV1::Resource(error) => error.into(),
        _ => source_raw_physical_error_v29(),
    }
}

fn index_equation_header_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<origin_worklist_v1::OriginWorkV1<bool>>(),
        std::mem::size_of::<
            Result<origin_worklist_v1::OriginWorkV1<bool>, origin_worklist_v1::OriginWorkErrorV1>,
        >(),
        std::mem::size_of::<
            Result<
                Vec<origin_worklist_v1::OriginStateV1<bool>>,
                origin_worklist_v1::OriginWorkErrorV1,
            >,
        >(),
    ])
}

fn index_normalization_header_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Option<u64>>(),
        std::mem::size_of::<Result<Option<u64>, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<(&Type, Option<&Operation>)>(),
        std::mem::size_of::<Result<(&Type, Option<&Operation>), ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<&SourceIndexMemoryV29<'_, '_, '_>>(),
    ])
}

impl<'view, 'inventory, 'graph> SourceIndexMemoryV29<'view, 'inventory, 'graph> {
    pub(super) fn new(
        inventory: &'inventory CanonicalKirInventoryV18<'graph>,
        memory: &'view CanonicalKirMemorySsaV18<'inventory, 'graph>,
        physical: &'view SourceAddressMemoryV29<'graph>,
        function: IndexFunction,
        slots: &'view [ScopedSourceSlotV29],
        accesses: &'view [SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::with_optional_versions(inventory, Some(memory), physical, function, slots, accesses, budget)
    }

    pub(super) fn with_optional_versions(
        inventory: &'inventory CanonicalKirInventoryV18<'graph>,
        memory: Option<&'view CanonicalKirMemorySsaV18<'inventory, 'graph>>,
        physical: &'view SourceAddressMemoryV29<'graph>,
        function: IndexFunction,
        slots: &'view [ScopedSourceSlotV29],
        accesses: &'view [SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        if memory.is_some_and(|memory| !memory.belongs_to(inventory)) {
            return Err(source_raw_physical_error_v29());
        }
        let original = inventory
            .functions()
            .get(function.0 as usize)
            .filter(|row| row.coordinate == function)
            .and_then(|row| row.function.body.as_ref())
            .ok_or_else(source_raw_physical_error_v29)?;
        if original.blocks.len() != physical.blocks.len() {
            return Err(source_raw_physical_error_v29());
        }
        for block in &original.blocks {
            budget.charge_work(1)?;
            let at = physical.block(block.id, budget)?;
            if !std::ptr::eq(block, physical.blocks[at].1) {
                return Err(source_raw_physical_error_v29());
            }
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            // Reused fixed envelopes for the bounded selector normalization
            // query. No definition walk allocates or retains per-value state.
            index_normalization_header_v29()?,
        ])?)?;
        Ok(Self {
            inventory,
            memory,
            physical,
            function,
            slots,
            accesses,
            equations: Vec::new(),
            inputs: Vec::new(),
            index: BTreeMap::new(),
            bounds: BTreeMap::new(),
            expanded: 0,
            ledger: budget.work_ledger_identity_v1(),
            required: budget.storage(),
        })
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1() || budget.storage() < self.required {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    // Literal array addresses need only exact immutable definitions. Every
    // read-from or selected-index equation still requires the shared owner.
    fn memory(&self) -> Result<&'view CanonicalKirMemorySsaV18<'inventory, 'graph>, ProductionSemanticKirErrorV1> {
        self.memory.ok_or_else(source_raw_physical_error_v29)
    }

    fn retain_growth(
        &mut self,
        before: usize,
        budget: &ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let growth = budget
            .storage()
            .checked_sub(before)
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.required = argument_sum_v1(&[self.required, growth])?;
        self.check(budget)
    }

    fn operation(
        &self,
        coordinate: IndexOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'graph Operation, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if coordinate.block.function != self.function {
            return Err(source_raw_physical_error_v29());
        }
        self.inventory
            .functions()
            .get(self.function.0 as usize)
            .and_then(|row| row.function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
            .and_then(|block| block.operations.get(coordinate.operation as usize))
            .ok_or_else(source_raw_physical_error_v29)
    }

    fn block_id(
        &self,
        coordinate: IndexOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if coordinate.block.function != self.function {
            return Err(source_raw_physical_error_v29());
        }
        self.inventory
            .functions()
            .get(self.function.0 as usize)
            .and_then(|row| row.function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
            .map(|block| block.id)
            .ok_or_else(source_raw_physical_error_v29)
    }

    fn intern(
        &mut self,
        slot: usize,
        node: IndexMemoryNodeId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        charge_execution_cfg_lookup_v29(self.index.len(), budget)?;
        let key = (slot, node.index());
        if let Some(&existing) = self.index.get(&key) {
            return Ok(existing);
        }
        let slot_row = self
            .slots
            .get(slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        let scalar = slot_row.scalar_array()?;
        if scalar.length != 1
            || scalar.bytes != scalar.element.size
            || !matches!(
                scalar.element.element,
                PrivateRetainedElementFactsV1::Scalar(_)
            )
        {
            return Err(source_raw_physical_error_v29());
        }
        let upper = argument_product_v1(self.slots.len(), self.memory()?.node_count())?;
        if self.equations.len() >= upper {
            return Err(source_raw_physical_error_v29());
        }
        // Publish the map only after its paid node exists. A failed allocation
        // leaves this scope poisoned by the original resource ledger.
        let before = budget.storage();
        reserve_execution_cfg_map_entry_v29::<(usize, usize), usize>(self.index.len(), budget)?;
        let ordinal = self.equations.len();
        emission_push_v1(
            &mut self.equations,
            IndexMemoryEquationV29 {
                slot,
                node,
                seed: IndexMemorySeedV29::Pending,
                inputs: 0..0,
            },
            budget,
        )?;
        self.index.insert(key, ordinal);
        self.retain_growth(before, budget)?;
        Ok(ordinal)
    }

    fn push_input(
        &mut self,
        slot: usize,
        node: IndexMemoryNodeId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let input = self.intern(slot, node, budget)?;
        let before = budget.storage();
        emission_push_v1(&mut self.inputs, input, budget)?;
        self.retain_growth(before, budget)?;
        Ok(())
    }

    fn expand(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        while self.expanded < self.equations.len() {
            self.check(budget)?;
            budget.charge_work(3)?;
            let at = self.expanded;
            let slot = self.equations[at].slot;
            let node = self.equations[at].node;
            let first = self.inputs.len();
            let seed = match self
                .memory()?
                .node(node, budget)
                .map_err(index_memory_error_v29)?
            {
                IndexMemoryNode::LiveOnEntry { function } => {
                    if *function != self.function {
                        return Err(source_raw_physical_error_v29());
                    }
                    IndexMemorySeedV29::Unknown
                }
                IndexMemoryNode::Use {
                    operation,
                    incoming,
                } => {
                    self.operation(*operation, budget)?;
                    self.push_input(slot, *incoming, budget)?;
                    IndexMemorySeedV29::Pending
                }
                IndexMemoryNode::Phi { block, .. } => {
                    if block.function != self.function {
                        return Err(source_raw_physical_error_v29());
                    }
                    let memory = self.memory()?;
                    let inputs = memory
                        .phi_inputs(node, budget)
                        .map_err(index_memory_error_v29)?;
                    if inputs.is_empty() {
                        IndexMemorySeedV29::Unknown
                    } else {
                        for input in inputs {
                            budget.charge_work(1)?;
                            self.push_input(slot, input.state(), budget)?;
                        }
                        IndexMemorySeedV29::Pending
                    }
                }
                IndexMemoryNode::Def {
                    operation,
                    incoming,
                } => {
                    let coordinate = *operation;
                    let incoming = *incoming;
                    let operation = self.operation(coordinate, budget)?;
                    match &operation.kind {
                        OperationKind::Store {
                            pointer,
                            value,
                            access,
                        } if !access.volatile => {
                            let block = self.block_id(coordinate, budget)?;
                            let tracked = SourceAddressMemoryV29::access(
                                self.accesses,
                                block,
                                coordinate.operation as usize,
                                budget,
                            )?;
                            match (self.physical.exact(*pointer, budget)?, tracked) {
                                (Some(actual), Some(row))
                                    if row.slot == actual && actual != slot =>
                                {
                                    self.push_input(slot, incoming, budget)?;
                                    IndexMemorySeedV29::Pending
                                }
                                (Some(actual), Some(row))
                                    if row.slot == actual && actual == slot =>
                                {
                                    let Type::Scalar(scalar) = self.physical.ty(*value, budget)?
                                    else {
                                        return Err(source_raw_physical_error_v29());
                                    };
                                    if self.slots[slot].scalar_array()?.length != 1 || !operation.results.is_empty()
                                    {
                                        return Err(source_raw_physical_error_v29());
                                    }
                                    let scalar = *scalar;
                                    // A genuine scalar Load copied by this Store
                                    // follows its own exact MemorySSA incoming
                                    // version, including cycles. It is never an
                                    // initialization seed by itself.
                                    let definition = self.inventory.definition_for_value(self.function, *value, budget)
                                        .map_err(|error| match error {
                                            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
                                            _ => source_raw_physical_error_v29(),
                                        })?.ok_or_else(source_raw_physical_error_v29)?;
                                    let copy = if let IndexDefinition::Result {
                                        operation: source,
                                        result,
                                    } = definition.coordinate
                                    {
                                        let original = self.operation(source, budget)?;
                                        match original.kind {
                                            OperationKind::Load { pointer, access }
                                                if !access.volatile =>
                                            {
                                                if result != 0
                                                    || !matches!(original.results.as_slice(), [row]
                                                    if row.id == *value && row.ty == Type::Scalar(scalar))
                                                {
                                                    return Err(source_raw_physical_error_v29());
                                                }
                                                let source_slot =
                                                    self.physical.exact(pointer, budget)?;
                                                let source_access = SourceAddressMemoryV29::access(
                                                    self.accesses,
                                                    self.block_id(source, budget)?,
                                                    source.operation as usize,
                                                    budget,
                                                )?;
                                                match (source_slot, source_access) {
                                                    (Some(source_slot), Some(access))
                                                        if source_slot == access.slot =>
                                                    {
                                                        let node = self
                                                            .memory()?
                                                            .operation(source, budget)
                                                            .map_err(index_memory_error_v29)?
                                                            .ok_or_else(
                                                                source_raw_physical_error_v29,
                                                            )?;
                                                        let IndexMemoryNode::Use {
                                                            operation,
                                                            incoming,
                                                        } = self
                                                            .memory()?
                                                            .node(node, budget)
                                                            .map_err(index_memory_error_v29)?
                                                        else {
                                                            return Err(
                                                                source_raw_physical_error_v29(),
                                                            );
                                                        };
                                                        if *operation != source {
                                                            return Err(
                                                                source_raw_physical_error_v29(),
                                                            );
                                                        }
                                                        Some((source_slot, *incoming))
                                                    }
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    };
                                    if let Some((source_slot, incoming)) = copy {
                                        self.push_input(source_slot, incoming, budget)?;
                                        IndexMemorySeedV29::Pending
                                    } else {
                                        IndexMemorySeedV29::Write {
                                            value: *value,
                                            scalar,
                                        }
                                    }
                                }
                                // Only the independently solved physical graph
                                // can exclude an untracked Global/Constant write.
                                (None, None) => {
                                    self.push_input(slot, incoming, budget)?;
                                    IndexMemorySeedV29::Pending
                                }
                                _ => IndexMemorySeedV29::Unknown,
                            }
                        }
                        OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }) => {
                            let access = source_address_value_access_v29(operation)?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            let tracked = SourceAddressMemoryV29::access(self.accesses,
                                self.block_id(coordinate, budget)?, coordinate.operation as usize, budget)?;
                            match (self.physical.exact(access.pointer, budget)?, tracked) {
                                (Some(actual), Some(row)) if actual == row.slot && actual != slot
                                    && !access.access.volatile => {
                                    // The complete checked access census proves
                                    // this typed write belongs to another object.
                                    self.push_input(slot, incoming, budget)?;
                                    IndexMemorySeedV29::Pending
                                }
                                _ => IndexMemorySeedV29::Unknown,
                            }
                        }
                        OperationKind::Alloca { .. } => {
                            let [result] = operation.results.as_slice() else {
                                return Err(source_raw_physical_error_v29());
                            };
                            if self.physical.exact(result.id, budget)? == Some(slot) {
                                IndexMemorySeedV29::Unknown
                            } else {
                                self.push_input(slot, incoming, budget)?;
                                IndexMemorySeedV29::Pending
                            }
                        }
                        // Atomics, guarded writes, barriers, calls and unknown
                        // effects are not ordinary disjoint scalar stores.
                        _ => IndexMemorySeedV29::Unknown,
                    }
                }
            };
            self.equations[at].seed = seed;
            self.equations[at].inputs = first..self.inputs.len();
            self.expanded += 1;
        }
        Ok(())
    }

    pub(super) fn definition(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&'graph Operation>, ProductionSemanticKirErrorV1> {
        let definition = self
            .inventory
            .definition_for_value(self.function, value, budget)
            .map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                    error.into()
                }
                _ => source_raw_physical_error_v29(),
            })?
            .ok_or_else(source_raw_physical_error_v29)?;
        let IndexDefinition::Result { operation, result } = definition.coordinate else {
            return Ok(None);
        };
        let operation = self.operation(operation, budget)?;
        if operation
            .results
            .get(result as usize)
            .is_none_or(|row| row.id != value)
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(operation))
    }

    pub(super) fn constant(
        &self,
        value: ValueId,
        scalar: ScalarType,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u64>, ProductionSemanticKirErrorV1> {
        if self.physical.ty(value, budget)? != &Type::Scalar(scalar) {
            return Ok(None);
        }
        Ok(self
            .definition(value, budget)?
            .and_then(|row| source_selector_constant_v29(row, value, scalar)))
    }

    pub(super) fn reevaluation(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(BlockId, usize), ProductionSemanticKirErrorV1> {
        let definition = self
            .inventory
            .definition_for_value(self.function, value, budget)
            .map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                    error.into()
                }
                _ => source_raw_physical_error_v29(),
            })?
            .ok_or_else(source_raw_physical_error_v29)?;
        let (block, gap) = match definition.coordinate {
            IndexDefinition::Result { operation, .. } => {
                (operation.block, operation.operation as usize)
            }
            IndexDefinition::BlockArgument { block, .. } => (block, 0),
            IndexDefinition::FunctionArgument { function, .. } if function == self.function => (
                fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 { function, block: 0 },
                0,
            ),
            _ => return Err(source_raw_physical_error_v29()),
        };
        let original = self.block_id(
            IndexOperation {
                block,
                operation: 0,
            },
            budget,
        )?;
        Ok((original, gap))
    }

    pub(super) fn value_bounded(
        &self,
        value: ValueId,
        scalar: ScalarType,
        length: u64,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let maximum = match scalar {
            ScalarType::U8 => u64::from(u8::MAX),
            ScalarType::U16 => u64::from(u16::MAX),
            ScalarType::U32 => u64::from(u32::MAX),
            ScalarType::U64 | ScalarType::Index => u64::MAX,
            _ => return Ok(false),
        };
        if maximum < length {
            return Ok(true);
        }
        if let Some(value) = self.constant(value, scalar, budget)? {
            return Ok(value < length);
        }
        let Some(operation) = self.definition(value, budget)? else {
            return Ok(false);
        };
        if operation.results.len() != 1
            || operation.results[0].id != value
            || operation.results[0].ty != Type::Scalar(scalar)
        {
            return Err(source_raw_physical_error_v29());
        }
        let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
            return Ok(false);
        };
        if self.physical.ty(lhs, budget)? != &Type::Scalar(scalar)
            || self.physical.ty(rhs, budget)? != &Type::Scalar(scalar)
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(match op {
            BinaryOp::BitAnd => self
                .constant(lhs, scalar, budget)?
                .into_iter()
                .chain(self.constant(rhs, scalar, budget)?)
                .any(|mask| mask < length),
            BinaryOp::Remainder => self
                .constant(rhs, scalar, budget)?
                .is_some_and(|divisor| divisor != 0 && divisor <= length),
            _ => false,
        })
    }

    // Query-local equations reuse the memoized (slot,node) dependencies. They
    // quantify all actual writes; no same-site or numeric identity is a version.
    pub(super) fn check_bound(
        &mut self,
        slot: usize,
        load: IndexOperation,
        result: ValueId,
        length: u64,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let operation = self.operation(load, budget)?;
        let OperationKind::Load { pointer, access } = operation.kind else {
            return Err(source_raw_physical_error_v29());
        };
        let [actual] = operation.results.as_slice() else {
            return Err(source_raw_physical_error_v29());
        };
        if actual.id != result
            || access.volatile
            || self.physical.exact(pointer, budget)? != Some(slot)
        {
            return Err(source_raw_physical_error_v29());
        }
        let node = self
            .memory()?
            .operation(load, budget)
            .map_err(index_memory_error_v29)?
            .ok_or_else(source_raw_physical_error_v29)?;
        let IndexMemoryNode::Use {
            operation: original,
            incoming,
        } = self
            .memory()?
            .node(node, budget)
            .map_err(index_memory_error_v29)?
        else {
            return Err(source_raw_physical_error_v29());
        };
        if *original != load {
            return Err(source_raw_physical_error_v29());
        }
        let incoming = *incoming;
        let key = (slot, incoming.index(), length);
        charge_execution_cfg_lookup_v29(self.bounds.len(), budget)?;
        if let Some(&checked) = self.bounds.get(&key) {
            return Ok(checked);
        }
        let start = self.intern(slot, incoming, budget)?;
        self.expand(budget)?;
        let floor = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<BTreeMap<usize, usize>>(),
            std::mem::size_of::<Vec<usize>>(),
            std::mem::size_of::<Vec<(usize, usize)>>(),
            std::mem::size_of::<Vec<origin_worklist_v1::OriginStateV1<bool>>>(),
            std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
            index_equation_header_v29()?,
        ])?)?;
        let mut reached = Vec::new();
        let mut links = Vec::new();
        let mut selected = BTreeMap::new();
        reserve_execution_cfg_map_entry_v29::<usize, usize>(0, budget)?;
        emission_push_v1(&mut reached, start, budget)?;
        selected.insert(start, 0);
        let mut next = 0;
        while next < reached.len() {
            budget.charge_work(2)?;
            let row = self
                .equations
                .get(reached[next])
                .ok_or_else(source_raw_physical_error_v29)?;
            for &source in &self.inputs[row.inputs.clone()] {
                charge_execution_cfg_lookup_v29(selected.len(), budget)?;
                let source = match selected.get(&source) {
                    Some(&ordinal) => ordinal,
                    None => {
                        reserve_execution_cfg_map_entry_v29::<usize, usize>(
                            selected.len(),
                            budget,
                        )?;
                        let ordinal = reached.len();
                        emission_push_v1(&mut reached, source, budget)?;
                        selected.insert(source, ordinal);
                        ordinal
                    }
                };
                emission_push_v1(&mut links, (source, next), budget)?;
            }
            next += 1;
        }
        let mut work = origin_worklist_v1::OriginWorkV1::new(reached.len(), links.len(), budget)
            .map_err(source_address_equation_error_v29)?;
        for &ordinal in &reached {
            budget.charge_work(1)?;
            let row = &self.equations[ordinal];
            let seed = match row.seed {
                IndexMemorySeedV29::Pending => origin_worklist_v1::OriginStateV1::Pending,
                IndexMemorySeedV29::Unknown => origin_worklist_v1::OriginStateV1::Unknown,
                IndexMemorySeedV29::Write { value, scalar } => {
                    if self.value_bounded(value, scalar, length, budget)? {
                        origin_worklist_v1::OriginStateV1::Exact(true)
                    } else {
                        origin_worklist_v1::OriginStateV1::Unknown
                    }
                }
            };
            work.seed_next(seed, budget)
                .map_err(source_address_equation_error_v29)?;
        }
        for &(source, target) in &links {
            work.add_link(source, target, budget)
                .map_err(source_address_equation_error_v29)?;
        }
        let states = work
            .solve(budget)
            .map_err(source_address_equation_error_v29)?;
        let accepted = states.first() == Some(&origin_worklist_v1::OriginStateV1::Exact(true));
        drop((states, selected, reached, links));
        let bytes = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(bytes)?;
        let before = budget.storage();
        reserve_execution_cfg_map_entry_v29::<(usize, usize, u64), bool>(
            self.bounds.len(),
            budget,
        )?;
        self.bounds.insert(key, accepted);
        self.retain_growth(before, budget)?;
        Ok(accepted)
    }

    fn may_write(
        &self,
        slot: usize,
        coordinate: IndexOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(node) = self
            .memory()?
            .operation(coordinate, budget)
            .map_err(index_memory_error_v29)?
        else {
            return Ok(false);
        };
        match self
            .memory()?
            .node(node, budget)
            .map_err(index_memory_error_v29)?
        {
            IndexMemoryNode::Use { operation, .. } if *operation == coordinate => return Ok(false),
            IndexMemoryNode::Def { operation, .. } if *operation == coordinate => {}
            _ => return Err(source_raw_physical_error_v29()),
        }
        let operation = self.operation(coordinate, budget)?;
        Ok(match operation.kind {
            OperationKind::Store { pointer, .. } => {
                let actual = self.physical.exact(pointer, budget)?;
                let access = SourceAddressMemoryV29::access(
                    self.accesses,
                    self.block_id(coordinate, budget)?,
                    coordinate.operation as usize,
                    budget,
                )?;
                match (actual, access) {
                    (Some(actual), Some(access)) if actual == access.slot => actual == slot,
                    (None, None) => false,
                    _ => return Err(source_raw_physical_error_v29()),
                }
            }
            OperationKind::Alloca { .. } => {
                let [result] = operation.results.as_slice() else {
                    return Err(source_raw_physical_error_v29());
                };
                self.physical.exact(result.id, budget)? == Some(slot)
            }
            // Unknown aliasing, ordered effects and synchronization cannot
            // preserve a retained scalar's dynamic version.
            _ => true,
        })
    }

    // Must facts on the existing physical graph. The constants mean this
    // particular read/guard is current, not merely that some write reaches.
    // A repeated static Def/Phi never revives an invalidated dynamic fact.
    fn current_guard_fact(
        &self,
        slot: usize,
        seed: IndexGuardSeedV29,
        target: (BlockId, usize),
        kills: &[SourceAddressKillV29],
        lifetimes: &[SourceAddressLifetimeV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let floor = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(usize, usize)>>(),
            std::mem::size_of::<Vec<origin_worklist_v1::OriginStateV1<bool>>>(),
            std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
            index_equation_header_v29()?,
        ])?)?;
        let original = self.inventory.functions()[self.function.0 as usize]
            .function
            .body
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        let count = original.blocks.len();
        let false_node = argument_product_v1(count, 2)?;
        let true_node = argument_sum_v1(&[false_node, 1])?;
        let nodes = argument_sum_v1(&[true_node, 1])?;
        let mut links = Vec::new();
        let mut query = None;
        let mut seed_seen = false;
        for (block_index, block) in original.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            let before = argument_product_v1(block_index, 2)?;
            let after = argument_sum_v1(&[before, 1])?;
            let mut current = before;
            // The existing sorted source boundary rows use block IDs, whereas
            // the canonical inventory uses body ordinals. Join once per block.
            budget.charge_work(argument_product_v1(
                2,
                call_splice_search_work_v1(kills.len()),
            )?)?;
            let mut kill = kills.partition_point(|row| row.block < block.id);
            let kill_end = kills.partition_point(|row| row.block <= block.id);
            budget.charge_work(argument_product_v1(
                2,
                call_splice_search_work_v1(lifetimes.len()),
            )?)?;
            let mut lifetime = lifetimes.partition_point(|row| row.block < block.id);
            let lifetime_end = lifetimes.partition_point(|row| row.block <= block.id);
            for gap in 0..=block.operations.len() {
                budget.charge_work(1)?;
                // A load seed occurs just after its operation. A boundary at
                // the following gap must still invalidate it before any use.
                if matches!(seed, IndexGuardSeedV29::Loaded { block: at, gap: point } if at == block.id && point == gap)
                {
                    if seed_seen {
                        return Err(source_raw_physical_error_v29());
                    }
                    seed_seen = true;
                    current = true_node;
                }
                while kill < kill_end && kills[kill].gap == gap {
                    budget.charge_work(1)?;
                    if kills[kill].slot == slot {
                        current = false_node;
                    }
                    kill += 1;
                }
                while lifetime < lifetime_end && lifetimes[lifetime].gap == gap {
                    budget.charge_work(1)?;
                    if lifetimes[lifetime].slot == slot {
                        current = false_node;
                    }
                    lifetime += 1;
                }
                if target == (block.id, gap) && query.replace(current).is_some() {
                    return Err(source_raw_physical_error_v29());
                }
                if gap < block.operations.len() {
                    let coordinate = IndexOperation {
                        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                            function: self.function,
                            block: u32::try_from(block_index)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        },
                        operation: u32::try_from(gap)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    };
                    if self.may_write(slot, coordinate, budget)? {
                        current = false_node;
                    }
                }
            }
            if kill != kill_end || lifetime != lifetime_end {
                return Err(source_raw_physical_error_v29());
            }
            emission_push_v1(&mut links, (current, after), budget)?;
            let mut ordinal = 0;
            block.terminator.as_ref().ok_or_else(source_raw_physical_error_v29)?
                .try_visit_edges_v1(|target, arguments| {
                    budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                    let source = if matches!(seed, IndexGuardSeedV29::Success { block: at } if at == block.id) && ordinal == 0 {
                        if seed_seen { return Err(source_raw_physical_error_v29()); }
                        seed_seen = true;
                        true_node
                    } else { after };
                    // Use the inventory's original body coordinate, not the
                    // source block number or a second reachability table.
                    let coordinate = self.inventory.block_for_id(self.function, target, budget)
                        .map_err(|error| match error {
                            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
                            _ => source_raw_physical_error_v29(),
                        })?.ok_or_else(source_raw_physical_error_v29)?;
                    emission_push_v1(&mut links, (source, argument_product_v1(coordinate.coordinate.block as usize, 2)?), budget)?;
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                    Ok::<_, ProductionSemanticKirErrorV1>(())
                })?;
        }
        let query = query.ok_or_else(source_raw_physical_error_v29)?;
        if !seed_seen {
            return Err(source_raw_physical_error_v29());
        }
        let mut work = origin_worklist_v1::OriginWorkV1::<bool>::new(nodes, links.len(), budget)
            .map_err(source_address_equation_error_v29)?;
        for node in 0..nodes {
            // Physical function invocation is an explicit false predecessor,
            // even when its original entry has a backedge.
            let seed = if node == false_node || node == 0 {
                origin_worklist_v1::OriginStateV1::Exact(false)
            } else if node == true_node {
                origin_worklist_v1::OriginStateV1::Exact(true)
            } else {
                origin_worklist_v1::OriginStateV1::Pending
            };
            work.seed_next(seed, budget)
                .map_err(source_address_equation_error_v29)?;
        }
        for &(source, target) in &links {
            work.add_link(source, target, budget)
                .map_err(source_address_equation_error_v29)?;
        }
        let states = work
            .solve(budget)
            .map_err(source_address_equation_error_v29)?;
        let valid = states.get(query) == Some(&origin_worklist_v1::OriginStateV1::Exact(true));
        drop((states, links));
        let owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(owned)?;
        Ok(valid)
    }

    fn guard_bound(
        &self,
        located: &SourceIndexGuardLocationV29,
        load: IndexOperation,
        kills: &[SourceAddressKillV29],
        lifetimes: &[SourceAddressLifetimeV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let guard = located.source;
        budget.charge_work(6)?;
        let actual_load = self.operation(located.load, budget)?;
        if !matches!(actual_load.kind, OperationKind::Load { pointer, access }
            if !access.volatile && self.physical.exact(pointer, budget)? == Some(guard.slot))
            || !matches!(actual_load.results.as_slice(), [result] if result.id == guard.value && result.ty == Type::Scalar(guard.scalar))
        {
            return Err(source_raw_physical_error_v29());
        }
        let node = self
            .memory()?
            .operation(located.load, budget)
            .map_err(index_memory_error_v29)?
            .ok_or_else(source_raw_physical_error_v29)?;
        if !matches!(self.memory()?.node(node, budget).map_err(index_memory_error_v29)?,
            IndexMemoryNode::Use { operation, .. } if *operation == located.load)
        {
            return Err(source_raw_physical_error_v29());
        }
        let comparison = self
            .definition(guard.condition, budget)?
            .ok_or_else(source_raw_physical_error_v29)?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = comparison.kind
        else {
            return Err(source_raw_physical_error_v29());
        };
        if lhs != guard.value
            || self.constant(rhs, guard.scalar, budget)? != Some(guard.length)
            || !matches!(comparison.results.as_slice(), [result] if result.id == guard.condition && result.ty == Type::BOOL)
        {
            return Err(source_raw_physical_error_v29());
        }
        let block = self.physical.blocks[self.physical.block(guard.block, budget)?].1;
        if !matches!(block.terminator.as_ref(), Some(Terminator::ConditionalBranch { condition, then_target, else_target, .. })
            if *condition == guard.condition && *then_target == guard.success && *else_target == guard.failure && *then_target != *else_target)
        {
            return Err(source_raw_physical_error_v29());
        }
        let loaded = IndexGuardSeedV29::Loaded {
            block: self.block_id(located.load, budget)?,
            gap: argument_sum_v1(&[located.load.operation as usize, 1])?,
        };
        if !self.current_guard_fact(
            guard.slot,
            loaded,
            (guard.block, block.operations.len()),
            kills,
            lifetimes,
            budget,
        )? {
            return Ok(false);
        }
        self.current_guard_fact(
            guard.slot,
            IndexGuardSeedV29::Success { block: guard.block },
            (self.block_id(load, budget)?, load.operation as usize),
            kills,
            lifetimes,
            budget,
        )
    }

    #[cfg(test)]
    pub(super) fn test_initialized_v29(
        &self,
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            self.inventory.functions()[self.function.0 as usize].function,
            self.physical,
            self.slots,
            self.accesses,
            kills,
            budget,
        )
    }

    #[cfg(test)]
    pub(super) fn test_guard_bound_v29(
        &self,
        guard: &SourceIndexGuardLocationV29,
        load: IndexOperation,
        kills: &[SourceAddressKillV29],
        lifetimes: &[SourceAddressLifetimeV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.guard_bound(guard, load, kills, lifetimes, budget)
    }

    pub(super) fn check_index_normalization(
        &self,
        original: ValueId,
        scalar: ScalarType,
        offset: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u64>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        if self.physical.ty(original, budget)? != &Type::Scalar(scalar) {
            return Err(source_raw_physical_error_v29());
        }
        if let Some(offset) = self.constant(offset, ScalarType::Index, budget)? {
            // This is the same bounded constant proof used before emission:
            // the original SSA definition, not the output literal, supplies
            // the value and the exact unsigned narrowing/widening semantics.
            let constant = source_selector_unsigned_constant_v29(
                original, scalar, self.physical.index.values.len(), budget,
                |value, budget| Ok((self.physical.ty(value, budget)?, self.definition(value, budget)?)),
            )?;
            if constant != Some(offset) {
                return Err(source_raw_physical_error_v29());
            }
            return Ok(constant);
        }
        let mut value = offset;
        for (kind, scalar) in plan_integer_cast_v1(scalar, ScalarType::Index)
            .ok_or_else(source_raw_physical_error_v29)?
            .into_iter()
            .flatten()
            .rev()
        {
            budget.charge_work(4)?;
            let operation = self
                .definition(value, budget)?
                .ok_or_else(source_raw_physical_error_v29)?;
            let OperationKind::Cast { kind: actual, value: input, ref to } = operation.kind else {
                return Err(source_raw_physical_error_v29());
            };
            if actual != kind
                || to != &Type::Scalar(scalar)
                || !matches!(operation.results.as_slice(), [result] if result.id == value && result.ty == *to)
            {
                return Err(source_raw_physical_error_v29());
            }
            value = input;
        }
        if value != original {
            return Err(source_raw_physical_error_v29());
        }
        Ok(None)
    }

    pub(super) fn check_recipe(
        &mut self,
        located: &SourceIndexLocationV29,
        guards: &[SourceIndexGuardLocationV29],
        kills: &[SourceAddressKillV29],
        lifetimes: &[SourceAddressLifetimeV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let row = located.source;
        budget.charge_work(8)?;
        let slot = self
            .slots
            .get(row.array_slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        let scalar = slot.scalar_array()?;
        if slot.origin.pointer != row.base
            || scalar.length != row.length
            || row.length == 0
            || self.physical.exact(row.pointer, budget)? != Some(row.array_slot)
            || self.physical.exact(row.base, budget)? != Some(row.array_slot)
        {
            return Err(source_raw_physical_error_v29());
        }
        let actual = self.physical.blocks[self.physical.block(row.block, budget)?]
            .1
            .operations
            .get(row.operation)
            .ok_or_else(source_raw_physical_error_v29)?;
        if !matches!(actual.kind, OperationKind::GetElementPointer { base, offset }
            if base == row.base && offset == row.offset)
            || !matches!(actual.results.as_slice(), [result] if result.id == row.pointer)
        {
            return Err(source_raw_physical_error_v29());
        }
        self.physical.check_gep_type(actual, budget)?;
        let constant = self.check_index_normalization(row.original, row.scalar, row.offset, budget)?;
        match (row.load_anchor, row.index_slot, located.load) {
            (Some(_), Some(slot), Some(load)) => {
                if self.check_bound(slot, load, row.original, row.length, budget)? {
                    return Ok(());
                }
                for guard in guards {
                    budget.charge_work(5)?;
                    if guard.source.slot == slot
                        && guard.source.instance == row.instance
                        && guard.source.length == row.length
                        && guard.source.scalar == row.scalar
                        && self.guard_bound(guard, load, kills, lifetimes, budget)?
                    {
                        return Ok(());
                    }
                }
            }
            (None, None, None)
                if constant.is_some_and(|value| value < row.length)
                    || self.value_bounded(row.original, row.scalar, row.length, budget)? =>
            {
                return Ok(());
            }
            _ => {}
        }
        Err(source_reference_error_v29(
            "source index needs exact current memory or successful guard bounds",
        ))
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;

    #[test]
    fn index_equations_account_for_consuming_solver_and_result_envelopes() {
        use std::mem::size_of;
        let expected = size_of::<origin_worklist_v1::OriginWorkV1<bool>>()
            + size_of::<
                Result<
                    origin_worklist_v1::OriginWorkV1<bool>,
                    origin_worklist_v1::OriginWorkErrorV1,
                >,
            >()
            + size_of::<
                Result<
                    Vec<origin_worklist_v1::OriginStateV1<bool>>,
                    origin_worklist_v1::OriginWorkErrorV1,
                >,
            >();
        assert_eq!(index_equation_header_v29().unwrap(), expected);
    }

    #[test]
    fn index_normalization_reuses_one_fixed_query_header_bundle() {
        use std::mem::size_of;
        let expected = size_of::<Option<u64>>()
            + size_of::<Result<Option<u64>, ProductionSemanticKirErrorV1>>()
            + size_of::<(&Type, Option<&Operation>)>()
            + size_of::<Result<(&Type, Option<&Operation>), ProductionSemanticKirErrorV1>>()
            + size_of::<&SourceIndexMemoryV29<'_, '_, '_>>();
        assert_eq!(index_normalization_header_v29().unwrap(), expected);
    }
}
