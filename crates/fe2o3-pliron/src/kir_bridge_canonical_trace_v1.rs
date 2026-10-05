//! Immutable occurrence correspondence for a newly imported exact current owner.
use super::*;
use crate::production_analysis::{
    CanonicalRankedPolicyFailureV1 as Failure, ControlViewV1,
    reserve_native_trace_map_v1 as reserve_map, reserve_native_trace_rows_v1 as reserve_rows,
};
use pliron::builtin::{
    attributes::IdentifierAttr,
    op_interfaces::{ATTR_KEY_SYM_NAME, OneRegionInterface},
};

type Budget<'w> = CanonicalKernelIrVerificationResourceBudgetV1<'w>;
type Resource = CanonicalKernelIrVerificationResourceErrorV1;

#[derive(Clone, Copy)]
pub(crate) enum NativeSubjectV1<'g> {
    Operation(&'g KirOperation),
    Terminator(&'g Terminator),
}

pub(crate) struct NativeOccurrenceV1<'g> {
    pub(crate) pointer: Ptr<Operation>,
    pub(crate) block: usize,
    pub(crate) operation: usize,
    pub(crate) subject: NativeSubjectV1<'g>,
}

pub(crate) struct NativeFunctionV1<'g> {
    pub(crate) owner: &'g VerifiedCanonicalKernelIrModuleV12,
    pub(crate) ordinal: usize,
    pub(crate) pointer: Option<Ptr<Operation>>,
    pub(crate) blocks: Vec<Ptr<BasicBlock>>,
    pub(crate) block_index: HashMap<Ptr<BasicBlock>, usize>,
    pub(crate) occurrences: Vec<NativeOccurrenceV1<'g>>,
    pub(crate) occurrence_index: HashMap<Ptr<Operation>, usize>,
    pub(crate) values: HashMap<Value, (ValueId, &'g Type)>,
}

pub(crate) struct NativeCanonicalTraceProjectionV1<'g> {
    graph: KirPlironGraphV12<'g>,
    witness: NativeBridgeWitnessV1,
    epoch: u64,
    functions: Vec<NativeFunctionV1<'g>>,
}

fn epoch(context: &Context) -> Result<u64, Failure> {
    context
        .ir_mutation_attempt_epoch()
        .map(|v| v.value())
        .map_err(|_| Failure::Mutation)
}

fn schema(
    context: &Context,
    pointer: Ptr<Operation>,
    keys: &[&str],
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    let raw = pointer.deref(context);
    budget.charge_work(1)?;
    if raw.attributes.0.len() != keys.len() {
        return Err(Failure::NativeSchema);
    }
    for key in raw.attributes.0.keys() {
        let name: &str = key.as_ref();
        budget.charge_work(
            name.len()
                .checked_add(keys.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if !keys.contains(&name) {
            return Err(Failure::NativeSchema);
        }
    }
    Ok(())
}

fn operation_keys(kind: &OperationKind) -> &'static [&'static str] {
    match kind {
        OperationKind::Constant(_) => &["gpu_constant_value"],
        OperationKind::Unary { .. } => &["gpu_unary_kind"],
        OperationKind::Binary { .. } => &["gpu_binary_kind"],
        OperationKind::Compare { .. } => &["gpu_compare_predicate"],
        OperationKind::Cast { .. } => &["gpu_cast_kind"],
        OperationKind::Call { .. } => &["gpu_call_callee", "gpu_call_signature"],
        OperationKind::Load { .. } => &[
            "gpu_load_address_space",
            "gpu_load_alignment",
            "gpu_load_volatile",
        ],
        OperationKind::Store { .. } => &[
            "gpu_store_address_space",
            "gpu_store_alignment",
            "gpu_store_volatile",
        ],
        OperationKind::Select { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. } => &[],
        _ => &["gpu_preserved_operation_kind"],
    }
}

fn terminator_keys(end: &Terminator) -> &'static [&'static str] {
    match end {
        Terminator::Return { .. } | Terminator::Branch { .. } => &[],
        Terminator::ConditionalBranch { .. } => &["operand_segment_sizes"],
        Terminator::Switch { .. } | Terminator::IntegerSwitch { .. } => &[
            "gpu_switch_kind",
            "gpu_switch_cases",
            "gpu_switch_offsets",
            "operand_segment_sizes",
        ],
        _ => &["gpu_preserved_terminator_kind", "operand_segment_sizes"],
    }
}

impl<'g> NativeCanonicalTraceProjectionV1<'g> {
    pub(crate) fn import(
        owner: &'g VerifiedCanonicalKernelIrModuleV12,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let (graph, witness) = import_native_neutral_v1(owner, budget)?;
        let epoch = epoch(&graph.session.context)?;
        let functions = reserve_rows(owner.module().functions.len(), budget)?;
        let mut result = Self {
            graph,
            witness,
            epoch,
            functions,
        };
        result.capture(budget)?;
        result.check(budget)?;
        Ok(result)
    }

    pub(crate) fn owner(&self) -> &'g VerifiedCanonicalKernelIrModuleV12 {
        self.graph.source
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn functions(&self) -> &[NativeFunctionV1<'g>] {
        &self.functions
    }
    pub(crate) fn check_epoch(&self) -> Result<(), Failure> {
        if epoch(&self.graph.session.context)? != self.epoch {
            return Err(Failure::Mutation);
        }
        Ok(())
    }

    fn capture(&mut self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.graph.validate_custody_v12()?;
        let context = &self.graph.session.context;
        let root = self.graph.session.operations[&self.graph.root.identity];
        schema(context, root, &["sym_name"], budget)?;
        let raw = root.deref(context);
        if !Operation::is_op::<ModuleOp>(root, context)
            || raw.num_regions() != 1
            || raw.get_num_operands() != 0
            || raw.get_num_results() != 0
            || raw.get_num_successors() != 0
            || raw
                .attributes
                .get::<IdentifierAttr>(&ATTR_KEY_SYM_NAME)
                .is_none_or(|v| v.as_ref().as_ref() != "kir_bridge_v12")
        {
            return Err(Failure::NativeSchema);
        }
        let region = raw.get_region(0);
        let region = region.deref(context);
        let mut root_blocks = region.iter(context);
        let root_block = root_blocks.next().ok_or(Failure::NativeSchema)?;
        if root_blocks.next().is_some()
            || root_block.deref(context).get_num_arguments() != 0
            || !root_block.deref(context).attributes.0.is_empty()
        {
            return Err(Failure::NativeSchema);
        }
        let root_body = root_block.deref(context);
        let mut live_functions = root_body.iter(context);
        for (ordinal, canonical) in self.graph.source.module().functions.iter().enumerate() {
            budget.charge_work(1)?;
            let Some(body) = &canonical.body else {
                self.functions.push(NativeFunctionV1 {
                    owner: self.graph.source,
                    ordinal,
                    pointer: None,
                    blocks: Vec::new(),
                    block_index: HashMap::new(),
                    occurrences: Vec::new(),
                    occurrence_index: HashMap::new(),
                    values: HashMap::new(),
                });
                continue;
            };
            let pointer = live_functions.next().ok_or(Failure::NativeSchema)?;
            schema(context, pointer, &["sym_name", "func_type"], budget)?;
            let function =
                Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            let raw = pointer.deref(context);
            let name = raw
                .attributes
                .get::<IdentifierAttr>(&ATTR_KEY_SYM_NAME)
                .ok_or(Failure::NativeSchema)?;
            let name: &str = name.as_ref().as_ref();
            budget.charge_work(name.len())?;
            let digits = name.strip_prefix("kir_fn_").ok_or(Failure::NativeSchema)?;
            if digits.parse::<usize>() != Ok(ordinal)
                || digits.is_empty()
                || (digits.len() > 1 && digits.starts_with('0'))
                || !digits.bytes().all(|v| v.is_ascii_digit())
                || raw.num_regions() != 1
                || raw.get_num_operands() != 0
                || raw.get_num_results() != 0
                || raw.get_num_successors() != 0
            {
                return Err(Failure::NativeSchema);
            }
            let mut value_count = body.parameters.len();
            let mut operation_count = body.blocks.len();
            for block in &body.blocks {
                budget.charge_work(1)?;
                value_count = value_count
                    .checked_add(block.parameters.len())
                    .ok_or(Resource::Arithmetic)?;
                operation_count = operation_count
                    .checked_add(block.operations.len())
                    .ok_or(Resource::Arithmetic)?;
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    value_count = value_count
                        .checked_add(operation.results.len())
                        .ok_or(Resource::Arithmetic)?;
                }
            }
            let mut row = NativeFunctionV1 {
                owner: self.graph.source,
                ordinal,
                pointer: Some(pointer),
                blocks: reserve_rows(body.blocks.len(), budget)?,
                block_index: reserve_map(body.blocks.len(), budget)?,
                occurrences: reserve_rows(operation_count, budget)?,
                occurrence_index: reserve_map(operation_count, budget)?,
                values: reserve_map(value_count, budget)?,
            };
            let live_region = function.get_region(context);
            let live_region = live_region.deref(context);
            let mut blocks = live_region.iter(context);
            for (bi, block) in body.blocks.iter().enumerate() {
                budget.charge_work(1)?;
                let pointer = blocks.next().ok_or(Failure::NativeSchema)?;
                let raw = pointer.deref(context);
                let offset = if bi == 0 { body.parameters.len() } else { 0 };
                if raw.get_num_arguments()
                    != offset
                        .checked_add(block.parameters.len())
                        .ok_or(Resource::Arithmetic)?
                    || !raw.attributes.0.is_empty()
                {
                    return Err(Failure::NativeSchema);
                }
                row.blocks.push(pointer);
                if row.block_index.insert(pointer, bi).is_some() {
                    return Err(Failure::ExactGraph);
                }
                if bi == 0 {
                    for (vi, id) in body.parameters.iter().enumerate() {
                        bind(
                            context,
                            &mut row.values,
                            raw.get_argument(vi),
                            *id,
                            &canonical.signature.parameters[vi],
                            budget,
                        )?;
                    }
                }
                for (vi, value) in block.parameters.iter().enumerate() {
                    bind(
                        context,
                        &mut row.values,
                        raw.get_argument(offset + vi),
                        value.id,
                        &value.ty,
                        budget,
                    )?;
                }
                let mut operations = raw.iter(context);
                for (oi, canonical) in block.operations.iter().enumerate() {
                    budget.charge_work(1)?;
                    let pointer = operations.next().ok_or(Failure::NativeSchema)?;
                    schema(context, pointer, operation_keys(&canonical.kind), budget)?;
                    let raw = pointer.deref(context);
                    if raw.num_regions() != 0
                        || raw.get_num_successors() != 0
                        || raw.get_num_results() != canonical.results.len()
                    {
                        return Err(Failure::NativeSchema);
                    }
                    for (vi, value) in canonical.results.iter().enumerate() {
                        bind(
                            context,
                            &mut row.values,
                            raw.get_result(vi),
                            value.id,
                            &value.ty,
                            budget,
                        )?;
                    }
                    if row
                        .occurrence_index
                        .insert(pointer, row.occurrences.len())
                        .is_some()
                    {
                        return Err(Failure::ExactGraph);
                    }
                    row.occurrences.push(NativeOccurrenceV1 {
                        pointer,
                        block: bi,
                        operation: oi,
                        subject: NativeSubjectV1::Operation(canonical),
                    });
                }
                let end = block.terminator.as_ref().ok_or(Failure::NativeSchema)?;
                let pointer = operations.next().ok_or(Failure::NativeSchema)?;
                schema(context, pointer, terminator_keys(end), budget)?;
                if operations.next().is_some() || raw.get_terminator(context) != Some(pointer) {
                    return Err(Failure::NativeSchema);
                }
                if row
                    .occurrence_index
                    .insert(pointer, row.occurrences.len())
                    .is_some()
                {
                    return Err(Failure::ExactGraph);
                }
                row.occurrences.push(NativeOccurrenceV1 {
                    pointer,
                    block: bi,
                    operation: block.operations.len(),
                    subject: NativeSubjectV1::Terminator(end),
                });
            }
            if blocks.next().is_some()
                || row.values.len() != value_count
                || row.occurrences.len() != operation_count
            {
                return Err(Failure::ExactGraph);
            }
            self.functions.push(row);
        }
        if live_functions.next().is_some() {
            return Err(Failure::ExactGraph);
        }
        Ok(())
    }

    pub(crate) fn check(&mut self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.check_epoch()?;
        self.graph.validate_custody_v12()?;
        let context = &self.graph.session.context;
        if self.functions.len() != self.graph.source.module().functions.len() {
            return Err(Failure::ExactGraph);
        }
        for (function_ordinal, row) in self.functions.iter().enumerate() {
            budget.charge_work(1)?;
            if row.ordinal != function_ordinal || !std::ptr::eq(row.owner, self.graph.source) {
                return Err(Failure::ExactGraph);
            }
            let canonical = &self.graph.source.module().functions[function_ordinal];
            if canonical.body.is_none() != row.pointer.is_none() {
                return Err(Failure::ExactGraph);
            }
            if let Some(body) = &canonical.body {
                if row.blocks.len() != body.blocks.len()
                    || row.block_index.len() != row.blocks.len()
                    || row.occurrence_index.len() != row.occurrences.len()
                {
                    return Err(Failure::ExactGraph);
                }
                let function =
                    Operation::get_op::<FuncOp>(row.pointer.ok_or(Failure::ExactGraph)?, context)
                        .ok_or(Failure::ExactGraph)?;
                let region = function.get_region(context);
                let region = region.deref(context);
                let mut actual_blocks = region.iter(context);
                let mut cursor = 0;
                let mut values = 0_usize;
                for (bi, block) in row.blocks.iter().enumerate() {
                    budget.charge_work(1)?;
                    if row.block_index.get(block) != Some(&bi)
                        || actual_blocks.next() != Some(*block)
                    {
                        return Err(Failure::ExactGraph);
                    }
                    let canonical_block = &body.blocks[bi];
                    let raw_block = block.deref(context);
                    let offset = if bi == 0 { body.parameters.len() } else { 0 };
                    if raw_block.get_num_arguments() != offset + canonical_block.parameters.len() {
                        return Err(Failure::ExactGraph);
                    }
                    if bi == 0 {
                        for (slot, id) in body.parameters.iter().enumerate() {
                            budget.charge_work(1)?;
                            if row.values.get(&raw_block.get_argument(slot))
                                != Some(&(*id, &canonical.signature.parameters[slot]))
                            {
                                return Err(Failure::ExactGraph);
                            }
                        }
                    }
                    for (slot, value) in canonical_block.parameters.iter().enumerate() {
                        budget.charge_work(1)?;
                        if row.values.get(&raw_block.get_argument(offset + slot))
                            != Some(&(value.id, &value.ty))
                        {
                            return Err(Failure::ExactGraph);
                        }
                    }
                    values = values
                        .checked_add(raw_block.get_num_arguments())
                        .ok_or(Resource::Arithmetic)?;
                    let mut operations = 0_usize;
                    for (oi, pointer) in block.deref(context).iter(context).enumerate() {
                        budget.charge_work(1)?;
                        let occurrence = row.occurrences.get(cursor).ok_or(Failure::ExactGraph)?;
                        if occurrence.pointer != pointer
                            || occurrence.block != bi
                            || occurrence.operation != oi
                        {
                            return Err(Failure::ExactGraph);
                        }
                        let subject_matches = match occurrence.subject {
                            NativeSubjectV1::Operation(op) => canonical_block
                                .operations
                                .get(oi)
                                .is_some_and(|expected| std::ptr::eq(op, expected)),
                            NativeSubjectV1::Terminator(end) => {
                                oi == canonical_block.operations.len()
                                    && canonical_block
                                        .terminator
                                        .as_ref()
                                        .is_some_and(|expected| std::ptr::eq(end, expected))
                            }
                        };
                        if !subject_matches {
                            return Err(Failure::ExactGraph);
                        }
                        values = values
                            .checked_add(pointer.deref(context).get_num_results())
                            .ok_or(Resource::Arithmetic)?;
                        operations += 1;
                        cursor += 1;
                    }
                    if operations != canonical_block.operations.len() + 1 {
                        return Err(Failure::ExactGraph);
                    }
                }
                if cursor != row.occurrences.len()
                    || values != row.values.len()
                    || actual_blocks.next().is_some()
                {
                    return Err(Failure::ExactGraph);
                }
            } else if !row.blocks.is_empty()
                || !row.block_index.is_empty()
                || !row.occurrences.is_empty()
                || !row.occurrence_index.is_empty()
                || !row.values.is_empty()
            {
                return Err(Failure::ExactGraph);
            }
            for (ordinal, occurrence) in row.occurrences.iter().enumerate() {
                budget.charge_work(1)?;
                if row.occurrence_index.get(&occurrence.pointer) != Some(&ordinal) {
                    return Err(Failure::ExactGraph);
                }
                let raw = occurrence.pointer.deref(context);
                if let NativeSubjectV1::Operation(operation) = occurrence.subject {
                    for (slot, result) in operation.results.iter().enumerate() {
                        budget.charge_work(1)?;
                        if row
                            .values
                            .get(&raw.get_result(slot))
                            .is_none_or(|(id, ty)| *id != result.id || **ty != result.ty)
                        {
                            return Err(Failure::ExactGraph);
                        }
                    }
                }
                let mut index = 0;
                let mut check_use = |id: ValueId| -> Result<(), Failure> {
                    budget.charge_work(1)?;
                    if index >= raw.get_num_operands()
                        || row.values.get(&raw.get_operand(index)).map(|v| v.0) != Some(id)
                    {
                        return Err(Failure::ExactGraph);
                    }
                    index += 1;
                    Ok(())
                };
                match occurrence.subject {
                    NativeSubjectV1::Operation(op) => op.kind.try_visit_operands(&mut check_use)?,
                    NativeSubjectV1::Terminator(end) => end.try_visit_operands(&mut check_use)?,
                }
                if index != raw.get_num_operands() {
                    return Err(Failure::ExactGraph);
                }
                if let NativeSubjectV1::Terminator(end) = occurrence.subject {
                    let control = ControlViewV1::observe(context, occurrence.pointer)
                        .map_err(|_| Failure::NativeSchema)?;
                    let body = canonical.body.as_ref().ok_or(Failure::ExactGraph)?;
                    let mut edge_ordinal = 0;
                    end.try_visit_edges_v1(|target, arguments| -> Result<(), Failure> {
                        budget.charge_work(1)?;
                        let edge = control
                            .edge(edge_ordinal)
                            .map_err(|_| Failure::ExactGraph)?;
                        let native_target = *row
                            .block_index
                            .get(&edge.target())
                            .ok_or(Failure::ExactGraph)?;
                        if body.blocks[native_target].id != target
                            || edge.argument_count() != arguments.len()
                        {
                            return Err(Failure::ExactGraph);
                        }
                        for (slot, id) in arguments.iter().enumerate() {
                            budget.charge_work(1)?;
                            let (source, destination) =
                                edge.argument_at(slot).map_err(|_| Failure::ExactGraph)?;
                            if row.values.get(&source).map(|v| v.0) != Some(*id)
                                || source.get_type(context) != destination.get_type(context)
                            {
                                return Err(Failure::ExactGraph);
                            }
                        }
                        edge_ordinal += 1;
                        Ok(())
                    })?;
                    if edge_ordinal != control.successor_count() {
                        return Err(Failure::ExactGraph);
                    }
                }
            }
        }
        // Reuses the existing importer schema/type decoders and preserved-payload remapper.
        let floor = budget.storage();
        let extracted = self
            .graph
            .extract_admitted_inner_v1(budget, true, Some(&self.witness))?;
        let expected = self.graph.source.canonical().canonical_bytes();
        let actual = extracted.0.canonical().canonical_bytes();
        budget.charge_work(
            expected
                .len()
                .checked_add(actual.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let equal = expected == actual;
        drop(extracted);
        restore_bridge_floor_v12(budget, floor)?;
        if !equal {
            return Err(Failure::ExactGraph);
        }
        self.check_epoch()
    }

    pub(crate) fn with_function<T>(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
        run: impl FnOnce(
            &Context,
            &FuncOp,
            &NativeFunctionV1<'g>,
            &mut Budget<'_>,
        ) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.check_epoch()?;
        budget.charge_work(1)?;
        let row = self
            .functions
            .get(ordinal)
            .ok_or(Failure::InvalidQuery { function: ordinal })?;
        let pointer = row.pointer.ok_or(Failure::UnsupportedGraph {
            function: ordinal,
            block: None,
            operation: None,
        })?;
        let context = &self.graph.session.context;
        let function =
            Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
        run(context, &function, row, budget)
    }

    #[cfg(test)]
    pub(crate) fn test_live<T>(&self, run: impl FnOnce(&Context, Ptr<Operation>) -> T) -> T {
        run(
            &self.graph.session.context,
            self.graph.session.operations[&self.graph.root.identity],
        )
    }

    #[cfg(test)]
    pub(crate) fn test_corrupt_occurrences(&mut self, mode: usize) {
        let row = self
            .functions
            .iter_mut()
            .find(|row| !row.occurrences.is_empty())
            .unwrap();
        match mode {
            0 => {
                row.occurrences.pop();
            }
            1 => {
                row.occurrences.swap(0, 1);
            }
            2 => {
                row.occurrences[0].operation += 1;
            }
            3 => {
                row.occurrence_index.clear();
            }
            4 => {
                row.occurrences[0].subject = row.occurrences[1].subject;
            }
            5 => {
                row.ordinal += 1;
            }
            6 => {
                row.values.clear();
            }
            7 => row.occurrences[1].pointer = row.occurrences[0].pointer,
            8 => {
                for (id, _) in row.values.values_mut() {
                    id.0 += 1;
                }
            }
            _ => {
                row.block_index.insert(row.blocks[0], 1);
            }
        }
    }
}

fn bind<'g>(
    context: &Context,
    values: &mut HashMap<Value, (ValueId, &'g Type)>,
    native: Value,
    id: ValueId,
    ty: &'g Type,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    budget.charge_work(1)?;
    if native.get_type(context)
        != KirBridgeTypeProfileV12::V12
            .to_pliron(context, ty)
            .map_err(KirBridgeErrorV12::from)?
        || values.insert(native, (id, ty)).is_some()
    {
        return Err(Failure::ExactGraph);
    }
    Ok(())
}
