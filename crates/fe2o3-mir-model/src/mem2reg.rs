use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::control_flow::{MirControlFlowAnalysis, analyze_mir_control_flow};
use crate::executable::terminator_edges;
use crate::{
    MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_LOCALS, MirBasicBlock, MirBlockId, MirBlockParameter,
    MirBody, MirBodyForm, MirCall, MirEdge, MirExecutableModule, MirExternalCallRegistry,
    MirLocalId, MirLocalKind, MirMutability, MirOperand, MirPlace, MirProjection, MirRvalue,
    MirStatement, MirStatementKind, MirTerminatorKind, MirTypeKind, MirUnwindAction, MirValueId,
    ValidatedMirExecutableModule,
};

pub const MAX_MEM2REG_OUTPUT_ITEMS: usize = 65_536;
const MAX_MEM2REG_LOCAL_WORDS: usize = MAX_EXECUTABLE_LOCALS.div_ceil(u64::BITS as usize);
pub const MAX_MEM2REG_LIVENESS_STORAGE_ITEMS: usize = MAX_EXECUTABLE_LOCALS
    + MAX_EXECUTABLE_BLOCKS * MAX_MEM2REG_LOCAL_WORDS * 3
    + MAX_MEM2REG_LOCAL_WORDS
    + MAX_EXECUTABLE_BLOCKS * 2;
pub const MAX_MEM2REG_LIVENESS_WORK_UNITS: usize =
    MAX_EXECUTABLE_BLOCKS * MAX_MEM2REG_LOCAL_WORDS * 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegFunctionReport {
    pub identity: String,
    pub promoted_locals: Vec<MirLocalId>,
    pub inserted_parameters: usize,
    pub inserted_definitions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegFunctionResourceReport {
    pub identity: String,
    pub fact_work_units: usize,
    pub liveness_storage_items: usize,
    /// Deterministic for the same canonical executable-MIR encoding.
    ///
    /// Worklist scheduling follows canonical block identities, so an
    /// otherwise equivalent body with different block numbering can report a
    /// different count while remaining under the same hard limit.
    pub liveness_work_units: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegReport {
    pub functions: Vec<MirMem2RegFunctionReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegResourceReport {
    pub functions: Vec<MirMem2RegFunctionResourceReport>,
}

impl MirMem2RegReport {
    pub fn promoted_local_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.promoted_locals.len())
            .sum()
    }

    pub fn inserted_parameter_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.inserted_parameters)
            .sum()
    }

    pub fn inserted_definition_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.inserted_definitions)
            .sum()
    }
}

impl MirMem2RegResourceReport {
    pub fn fact_work_units(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.fact_work_units)
            .sum()
    }

    pub fn liveness_storage_items(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.liveness_storage_items)
            .sum()
    }

    pub fn liveness_work_units(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.liveness_work_units)
            .sum()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegError {
    path: String,
    reason: String,
}

impl MirMem2RegError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirMem2RegError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirMem2RegError {}

/// Promotes a bounded subset of whole-place Copy locals into explicit SSA.
///
/// Parameters are placed at live iterated-dominance-frontier blocks. Values
/// defined in dominating blocks remain directly usable, while joins and loop
/// headers receive explicit edge arguments. Locals with projections,
/// address-taking, storage markers, call destinations, or drops remain slots;
/// eligible locals may be initialized in any block.
pub fn promote_module_to_ssa(
    module: &ValidatedMirExecutableModule,
) -> Result<(ValidatedMirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    let (output, report, _) = promote_module_to_ssa_with_resources(module)?;
    Ok((output, report))
}

/// Promotes places to SSA and returns deterministic resource accounting.
pub fn promote_module_to_ssa_with_resources(
    module: &ValidatedMirExecutableModule,
) -> Result<
    (
        ValidatedMirExecutableModule,
        MirMem2RegReport,
        MirMem2RegResourceReport,
    ),
    MirMem2RegError,
> {
    let source = module.as_module();
    let mut output = source.clone();
    let mut reports = Vec::with_capacity(output.functions.len());
    let mut resource_reports = Vec::with_capacity(output.functions.len());
    let mut output_items = GeneratedItemBudget::default();

    for (function_index, function) in output.functions.iter_mut().enumerate() {
        if !matches!(function.body.form, MirBodyForm::Places) {
            return Err(MirMem2RegError::new(
                format!("module.functions[{function_index}].body.form"),
                "mem2reg accepts only place-form executable MIR",
            ));
        }
        let original = function.body.clone();
        let facts = BodyLocalFacts::collect(&original);
        let promoted = eligible_locals(source, &original, &facts);
        let control_flow = analyze_mir_control_flow(&original).map_err(|error| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                format!("mem2reg requires reducible canonical control flow: {error}"),
            )
        })?;
        let plan = SsaPlan::build(
            function_index,
            &original,
            &promoted,
            &control_flow,
            &facts,
            &mut output_items,
        )?;
        let (body, inserted_parameters, inserted_definitions) =
            transform_body(function_index, &original, &promoted, &control_flow, &plan)?;
        function.body = body;
        reports.push(MirMem2RegFunctionReport {
            identity: function.identity.clone(),
            promoted_locals: promoted,
            inserted_parameters,
            inserted_definitions,
        });
        resource_reports.push(MirMem2RegFunctionResourceReport {
            identity: function.identity.clone(),
            fact_work_units: facts.work_units,
            liveness_storage_items: plan.liveness_storage_items,
            liveness_work_units: plan.liveness_work_units,
        });
    }

    let output = output
        .validate_with_registry(module.registry())
        .map_err(|error| {
            MirMem2RegError::new(
                error.path(),
                format!(
                    "mem2reg produced invalid executable MIR: {}",
                    error.reason()
                ),
            )
        })?;
    Ok((
        output,
        MirMem2RegReport { functions: reports },
        MirMem2RegResourceReport {
            functions: resource_reports,
        },
    ))
}

/// Compatibility constructor that validates untrusted data and immediately
/// delegates to the validated-only transform.
pub fn promote_module_to_ssa_with_registry(
    module: &MirExecutableModule,
    registry: &MirExternalCallRegistry,
) -> Result<(ValidatedMirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    let validated = module
        .validate_with_registry(registry)
        .map_err(|error| MirMem2RegError::new(error.path(), error.reason()))?;
    promote_module_to_ssa(&validated)
}

/// Validates untrusted data, promotes places to SSA, and returns resource accounting.
pub fn promote_module_to_ssa_with_registry_and_resources(
    module: &MirExecutableModule,
    registry: &MirExternalCallRegistry,
) -> Result<
    (
        ValidatedMirExecutableModule,
        MirMem2RegReport,
        MirMem2RegResourceReport,
    ),
    MirMem2RegError,
> {
    let validated = module
        .validate_with_registry(registry)
        .map_err(|error| MirMem2RegError::new(error.path(), error.reason()))?;
    promote_module_to_ssa_with_resources(&validated)
}

#[derive(Default)]
struct GeneratedItemBudget {
    consumed: usize,
}

impl GeneratedItemBudget {
    fn charge(&mut self, function_index: usize, items: usize) -> Result<(), MirMem2RegError> {
        let path = format!("module.functions[{function_index}].body");
        let consumed = self
            .consumed
            .checked_add(items)
            .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg output item budget overflow"))?;
        if consumed > MAX_MEM2REG_OUTPUT_ITEMS {
            return Err(MirMem2RegError::new(
                path,
                format!(
                    "mem2reg output requires {consumed} generated items, exceeding {MAX_MEM2REG_OUTPUT_ITEMS}"
                ),
            ));
        }
        self.consumed = consumed;
        Ok(())
    }
}

struct SsaPlan {
    parameter_locals: Vec<Vec<MirLocalId>>,
    definition_count: usize,
    liveness_storage_items: usize,
    liveness_work_units: usize,
}

#[derive(Default)]
struct LivenessBudget {
    storage_items: usize,
    work_units: usize,
}

impl LivenessBudget {
    fn reserve_storage(
        &mut self,
        function_index: usize,
        items: usize,
    ) -> Result<(), MirMem2RegError> {
        let required = self
            .storage_items
            .checked_add(items)
            .unwrap_or(MAX_MEM2REG_LIVENESS_STORAGE_ITEMS + 1);
        if required > MAX_MEM2REG_LIVENESS_STORAGE_ITEMS {
            return Err(MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                format!(
                    "mem2reg liveness requires {required} storage items, exceeding the deterministic limit {MAX_MEM2REG_LIVENESS_STORAGE_ITEMS}"
                ),
            ));
        }
        self.storage_items = required;
        Ok(())
    }

    fn charge_work(&mut self, function_index: usize, units: usize) -> Result<(), MirMem2RegError> {
        let consumed = self
            .work_units
            .checked_add(units)
            .unwrap_or(MAX_MEM2REG_LIVENESS_WORK_UNITS + 1);
        if consumed > MAX_MEM2REG_LIVENESS_WORK_UNITS {
            return Err(MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                format!(
                    "mem2reg liveness consumed {consumed} work units, exceeding the deterministic limit {MAX_MEM2REG_LIVENESS_WORK_UNITS}"
                ),
            ));
        }
        self.work_units = consumed;
        Ok(())
    }
}

struct IndexedLiveIn {
    words_per_block: usize,
    words: Vec<u64>,
}

impl IndexedLiveIn {
    fn contains(&self, block: MirBlockId, local_index: usize) -> bool {
        let word = block.0 as usize * self.words_per_block + local_index / u64::BITS as usize;
        self.words
            .get(word)
            .is_some_and(|value| value & (1_u64 << (local_index % u64::BITS as usize)) != 0)
    }
}

struct BodyLocalFacts {
    touched: BTreeSet<MirLocalId>,
    disqualified: BTreeSet<MirLocalId>,
    definitions: Vec<BTreeSet<MirLocalId>>,
    upward_exposed_uses: Vec<BTreeSet<MirLocalId>>,
    definition_blocks: Vec<BTreeSet<MirBlockId>>,
    assignment_definition_counts: Vec<usize>,
    work_units: usize,
}

impl BodyLocalFacts {
    fn collect(body: &MirBody) -> Self {
        let mut touched = BTreeSet::new();
        let mut disqualified = BTreeSet::new();
        let mut definitions = vec![BTreeSet::new(); body.blocks.len()];
        let mut upward_exposed_uses = vec![BTreeSet::new(); body.blocks.len()];
        let mut definition_blocks = vec![BTreeSet::new(); body.locals.len()];
        let mut assignment_definition_counts = vec![0_usize; body.locals.len()];
        let mut work_units = body.locals.len();

        for (block_index, block) in body.blocks.iter().enumerate() {
            let block_id = MirBlockId(block_index as u32);
            if block_id == body.entry {
                for (local_index, local) in body.locals.iter().enumerate() {
                    if local.kind == MirLocalKind::Argument {
                        let local_id = MirLocalId(local_index as u32);
                        definitions[block_index].insert(local_id);
                        definition_blocks[local_index].insert(block_id);
                    }
                }
            }

            let mut reads = BTreeSet::new();
            for statement in &block.statements {
                reads.clear();
                inspect_statement(
                    statement,
                    &mut touched,
                    &mut disqualified,
                    &mut reads,
                    &mut work_units,
                );
                for local in &reads {
                    if !definitions[block_index].contains(local) {
                        upward_exposed_uses[block_index].insert(*local);
                    }
                }
                if let MirStatementKind::Assign { place, .. } = &statement.kind
                    && place.projection.is_empty()
                {
                    definitions[block_index].insert(place.local);
                    definition_blocks[place.local.0 as usize].insert(block_id);
                    assignment_definition_counts[place.local.0 as usize] += 1;
                }
            }

            reads.clear();
            inspect_terminator(
                &block.terminator.kind,
                &mut touched,
                &mut disqualified,
                &mut reads,
                &mut work_units,
            );
            for local in reads {
                if !definitions[block_index].contains(&local) {
                    upward_exposed_uses[block_index].insert(local);
                }
            }
        }

        Self {
            touched,
            disqualified,
            definitions,
            upward_exposed_uses,
            definition_blocks,
            assignment_definition_counts,
            work_units,
        }
    }
}

fn compute_liveness(
    function_index: usize,
    body: &MirBody,
    promoted: &[MirLocalId],
    control_flow: &MirControlFlowAnalysis,
    facts: &BodyLocalFacts,
) -> Result<(IndexedLiveIn, LivenessBudget), MirMem2RegError> {
    let block_count = body.blocks.len();
    let words_per_block = promoted.len().div_ceil(u64::BITS as usize);
    let matrix_words = block_count.checked_mul(words_per_block).ok_or_else(|| {
        MirMem2RegError::new(
            format!("module.functions[{function_index}].body"),
            "mem2reg liveness storage item overflow",
        )
    })?;
    let storage_items = body
        .locals
        .len()
        .checked_add(matrix_words.checked_mul(3).ok_or_else(|| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                "mem2reg liveness storage item overflow",
            )
        })?)
        .and_then(|items| items.checked_add(words_per_block))
        .and_then(|items| items.checked_add(block_count.checked_mul(2)?))
        .ok_or_else(|| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                "mem2reg liveness storage item overflow",
            )
        })?;
    let mut budget = LivenessBudget::default();
    budget.reserve_storage(function_index, storage_items)?;

    let mut local_to_bit = vec![usize::MAX; body.locals.len()];
    for (bit, local) in promoted.iter().copied().enumerate() {
        budget.charge_work(function_index, 1)?;
        local_to_bit[local.0 as usize] = bit;
    }
    let mut definitions = vec![0_u64; matrix_words];
    let mut upward_exposed_uses = vec![0_u64; matrix_words];
    for block_index in 0..block_count {
        let offset = block_index * words_per_block;
        for local in &facts.definitions[block_index] {
            budget.charge_work(function_index, 1)?;
            let bit = local_to_bit[local.0 as usize];
            if bit != usize::MAX {
                definitions[offset + bit / u64::BITS as usize] |=
                    1_u64 << (bit % u64::BITS as usize);
            }
        }
        for local in &facts.upward_exposed_uses[block_index] {
            budget.charge_work(function_index, 1)?;
            let bit = local_to_bit[local.0 as usize];
            if bit != usize::MAX {
                upward_exposed_uses[offset + bit / u64::BITS as usize] |=
                    1_u64 << (bit % u64::BITS as usize);
            }
        }
    }

    let mut live_in = vec![0_u64; matrix_words];
    let mut scratch = vec![0_u64; words_per_block];
    let mut queued = vec![true; block_count];
    let mut pending = VecDeque::with_capacity(block_count);
    budget.charge_work(function_index, block_count)?;
    pending.extend(0..block_count);

    while !pending.is_empty() {
        budget.charge_work(function_index, 1)?;
        let block_index = pending
            .pop_front()
            .expect("a nonempty liveness worklist has a front item");
        queued[block_index] = false;
        budget.charge_work(function_index, words_per_block)?;
        scratch.fill(0);
        let block = MirBlockId(block_index as u32);
        for successor in control_flow
            .successors(block)
            .expect("analysis covers every canonical block")
        {
            budget.charge_work(function_index, words_per_block)?;
            let successor_offset = successor.0 as usize * words_per_block;
            for (word, successor_word) in scratch
                .iter_mut()
                .zip(&live_in[successor_offset..successor_offset + words_per_block])
            {
                *word |= successor_word;
            }
        }

        budget.charge_work(function_index, words_per_block)?;
        let offset = block_index * words_per_block;
        let mut changed = false;
        for word_index in 0..words_per_block {
            let next = upward_exposed_uses[offset + word_index]
                | (scratch[word_index] & !definitions[offset + word_index]);
            changed |= live_in[offset + word_index] != next;
            live_in[offset + word_index] = next;
        }
        if changed {
            for predecessor in control_flow
                .predecessors(block)
                .expect("analysis covers every canonical block")
            {
                budget.charge_work(function_index, 1)?;
                let predecessor_index = predecessor.0 as usize;
                if !queued[predecessor_index] {
                    queued[predecessor_index] = true;
                    pending.push_back(predecessor_index);
                }
            }
        }
    }

    Ok((
        IndexedLiveIn {
            words_per_block,
            words: live_in,
        },
        budget,
    ))
}

impl SsaPlan {
    fn build(
        function_index: usize,
        body: &MirBody,
        promoted: &[MirLocalId],
        control_flow: &MirControlFlowAnalysis,
        facts: &BodyLocalFacts,
        output_items: &mut GeneratedItemBudget,
    ) -> Result<Self, MirMem2RegError> {
        let definition_count = promoted.iter().try_fold(0_usize, |total, local| {
            total.checked_add(facts.assignment_definition_counts[local.0 as usize])
        });
        let definition_count = definition_count.ok_or_else(|| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                "mem2reg output item budget overflow",
            )
        })?;
        output_items.charge(function_index, definition_count)?;
        let (live_in, liveness_budget) =
            compute_liveness(function_index, body, promoted, control_flow, facts)?;

        let path = format!("module.functions[{function_index}].body");
        let mut incoming_edges = vec![0_usize; body.blocks.len()];
        for edge in body
            .blocks
            .iter()
            .flat_map(|block| terminator_edges(&block.terminator.kind))
        {
            let incoming = &mut incoming_edges[edge.target.0 as usize];
            *incoming = incoming.checked_add(1).ok_or_else(|| {
                MirMem2RegError::new(&path, "mem2reg edge-argument budget overflow")
            })?;
        }

        let mut parameter_locals = vec![Vec::new(); body.blocks.len()];
        for local in promoted
            .iter()
            .copied()
            .filter(|local| body.locals[local.0 as usize].kind == MirLocalKind::Argument)
        {
            let growth = incoming_edges[body.entry.0 as usize]
                .checked_add(1)
                .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg parameter budget overflow"))?;
            output_items.charge(function_index, growth)?;
            parameter_locals[body.entry.0 as usize].push(local);
        }
        for (local_index, local) in promoted.iter().enumerate() {
            let frontiers = control_flow
                .iterated_dominance_frontier(&facts.definition_blocks[local.0 as usize])
                .expect("definition blocks belong to the analyzed body");
            for block in frontiers {
                if block != body.entry && live_in.contains(block, local_index) {
                    let growth =
                        incoming_edges[block.0 as usize]
                            .checked_add(1)
                            .ok_or_else(|| {
                                MirMem2RegError::new(&path, "mem2reg parameter budget overflow")
                            })?;
                    output_items.charge(function_index, growth)?;
                    parameter_locals[block.0 as usize].push(*local);
                }
            }
        }
        for locals in &mut parameter_locals {
            locals.sort();
            locals.dedup();
        }

        Ok(Self {
            parameter_locals,
            definition_count,
            liveness_storage_items: liveness_budget.storage_items,
            liveness_work_units: liveness_budget.work_units,
        })
    }
}

fn eligible_locals(
    module: &MirExecutableModule,
    body: &MirBody,
    facts: &BodyLocalFacts,
) -> Vec<MirLocalId> {
    body.locals
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, local)| {
            let id = MirLocalId(index as u32);
            let copy_type = match &module.types[local.ty.0 as usize].kind {
                MirTypeKind::Unit | MirTypeKind::Scalar(_) | MirTypeKind::RawPointer { .. } => true,
                MirTypeKind::Reference { mutability, .. } => {
                    *mutability == MirMutability::Immutable
                }
                _ => false,
            };
            if !copy_type || facts.disqualified.contains(&id) || !facts.touched.contains(&id) {
                return None;
            }
            Some(id)
        })
        .collect()
}

fn transform_body(
    function_index: usize,
    body: &MirBody,
    promoted: &[MirLocalId],
    control_flow: &MirControlFlowAnalysis,
    plan: &SsaPlan,
) -> Result<(MirBody, usize, usize), MirMem2RegError> {
    let promoted_set = promoted.iter().copied().collect::<BTreeSet<_>>();
    let mut allocator = ValueAllocator::default();
    let mut parameter_values = vec![Vec::new(); body.blocks.len()];
    let mut definition_values = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        parameter_values[block_index] = plan.parameter_locals[block_index]
            .iter()
            .map(|local| {
                let value = allocator.allocate(function_index, block_index)?;
                Ok((*local, value))
            })
            .collect::<Result<Vec<_>, MirMem2RegError>>()?;
        for (statement_index, statement) in block.statements.iter().enumerate() {
            if let MirStatementKind::Assign { place, .. } = &statement.kind
                && place.projection.is_empty()
                && promoted_set.contains(&place.local)
            {
                let value = allocator.allocate(function_index, block_index)?;
                definition_values.insert((block_index, statement_index), (place.local, value));
            }
        }
    }

    enum RenameEvent {
        Enter(MirBlockId),
        Exit(Vec<MirLocalId>),
    }

    let mut blocks = vec![None; body.blocks.len()];
    let mut current = BTreeMap::<MirLocalId, Vec<MirValueId>>::new();
    let mut events = vec![RenameEvent::Enter(body.entry)];
    while let Some(event) = events.pop() {
        let block_id = match event {
            RenameEvent::Enter(block) => block,
            RenameEvent::Exit(pushed) => {
                for local in pushed.into_iter().rev() {
                    let values = current
                        .get_mut(&local)
                        .expect("a rename exit has a matching value stack");
                    values.pop();
                    if values.is_empty() {
                        current.remove(&local);
                    }
                }
                continue;
            }
        };
        let block_index = block_id.0 as usize;
        let original = &body.blocks[block_index];
        let mut pushed = Vec::new();
        let parameters = parameter_values[block_index]
            .iter()
            .map(|(local, value)| {
                current.entry(*local).or_default().push(*value);
                pushed.push(*local);
                MirBlockParameter {
                    value: *value,
                    ty: body.locals[local.0 as usize].ty,
                    origin: Some(*local),
                }
            })
            .collect::<Vec<_>>();
        let mut statements = Vec::with_capacity(original.statements.len());
        for (statement_index, statement) in original.statements.iter().enumerate() {
            let path = format!(
                "module.functions[{function_index}].body.blocks[{block_index}].statements[{statement_index}]"
            );
            match &statement.kind {
                MirStatementKind::Assign { place, value }
                    if place.projection.is_empty() && promoted_set.contains(&place.local) =>
                {
                    let rvalue = rewrite_rvalue(&path, value, &current, &promoted_set)?;
                    let (_, value) = definition_values[&(block_index, statement_index)];
                    current.entry(place.local).or_default().push(value);
                    pushed.push(place.local);
                    statements.push(MirStatement {
                        kind: MirStatementKind::Define {
                            value,
                            ty: place.ty,
                            rvalue,
                        },
                        span: statement.span.clone(),
                    });
                }
                MirStatementKind::Assign { place, value } => {
                    statements.push(MirStatement {
                        kind: MirStatementKind::Assign {
                            place: place.clone(),
                            value: rewrite_rvalue(&path, value, &current, &promoted_set)?,
                        },
                        span: statement.span.clone(),
                    });
                }
                MirStatementKind::Define { .. } => {
                    return Err(MirMem2RegError::new(
                        path,
                        "place-form input unexpectedly contains an SSA definition",
                    ));
                }
                _ => statements.push(statement.clone()),
            }
        }
        let mut terminator = original.terminator.clone();
        rewrite_terminator(
            function_index,
            block_index,
            &mut terminator.kind,
            &current,
            &plan.parameter_locals,
            &promoted_set,
        )?;
        blocks[block_index] = Some(MirBasicBlock {
            parameters,
            statements,
            terminator,
        });
        events.push(RenameEvent::Exit(pushed));
        for child in control_flow
            .dominator_tree_children(block_id)
            .expect("analysis covers every canonical block")
            .iter()
            .rev()
        {
            events.push(RenameEvent::Enter(*child));
        }
    }

    Ok((
        MirBody {
            form: MirBodyForm::Ssa {
                promoted_locals: promoted.to_vec(),
            },
            locals: body.locals.clone(),
            blocks: blocks
                .into_iter()
                .map(|block| block.expect("reachable dominator traversal visits every block"))
                .collect(),
            entry: body.entry,
        },
        plan.parameter_locals.iter().map(Vec::len).sum(),
        plan.definition_count,
    ))
}

#[derive(Default)]
struct ValueAllocator {
    next: u32,
}

impl ValueAllocator {
    fn allocate(
        &mut self,
        function_index: usize,
        block_index: usize,
    ) -> Result<MirValueId, MirMem2RegError> {
        let value = MirValueId(self.next);
        self.next = self.next.checked_add(1).ok_or_else(|| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body.blocks[{block_index}]"),
                "SSA value identity overflow",
            )
        })?;
        Ok(value)
    }
}

fn rewrite_rvalue(
    path: &str,
    rvalue: &MirRvalue,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    promoted: &BTreeSet<MirLocalId>,
) -> Result<MirRvalue, MirMem2RegError> {
    Ok(match rvalue {
        MirRvalue::Use(operand) => {
            MirRvalue::Use(rewrite_operand(path, operand, current, promoted)?)
        }
        MirRvalue::BinaryOp { op, lhs, rhs } => MirRvalue::BinaryOp {
            op: *op,
            lhs: rewrite_operand(path, lhs, current, promoted)?,
            rhs: rewrite_operand(path, rhs, current, promoted)?,
        },
        MirRvalue::CheckedBinaryOp { op, lhs, rhs } => MirRvalue::CheckedBinaryOp {
            op: *op,
            lhs: rewrite_operand(path, lhs, current, promoted)?,
            rhs: rewrite_operand(path, rhs, current, promoted)?,
        },
        MirRvalue::UnaryOp { op, operand } => MirRvalue::UnaryOp {
            op: *op,
            operand: rewrite_operand(path, operand, current, promoted)?,
        },
        MirRvalue::Cast { kind, operand, ty } => MirRvalue::Cast {
            kind: *kind,
            operand: rewrite_operand(path, operand, current, promoted)?,
            ty: *ty,
        },
        MirRvalue::Aggregate { kind, operands } => MirRvalue::Aggregate {
            kind: kind.clone(),
            operands: operands
                .iter()
                .map(|operand| rewrite_operand(path, operand, current, promoted))
                .collect::<Result<Vec<_>, _>>()?,
        },
        MirRvalue::Repeat { operand, count } => MirRvalue::Repeat {
            operand: rewrite_operand(path, operand, current, promoted)?,
            count: *count,
        },
        MirRvalue::Ref { .. }
        | MirRvalue::AddressOf { .. }
        | MirRvalue::Len(_)
        | MirRvalue::Discriminant(_)
        | MirRvalue::ThreadIndex1d => rvalue.clone(),
    })
}

fn rewrite_operand(
    path: &str,
    operand: &MirOperand,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    promoted: &BTreeSet<MirLocalId>,
) -> Result<MirOperand, MirMem2RegError> {
    match operand {
        MirOperand::Copy(place) | MirOperand::Move(place)
            if place.projection.is_empty() && promoted.contains(&place.local) =>
        {
            current
                .get(&place.local)
                .and_then(|values| values.last().copied())
                .map(MirOperand::Value)
                .ok_or_else(|| {
                    MirMem2RegError::new(
                        path,
                        format!("promoted local {} is read before definition", place.local.0),
                    )
                })
        }
        _ => Ok(operand.clone()),
    }
}

fn rewrite_terminator(
    function_index: usize,
    block_index: usize,
    terminator: &mut MirTerminatorKind,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    let path = format!("module.functions[{function_index}].body.blocks[{block_index}].terminator");
    match terminator {
        MirTerminatorKind::Goto(edge) => {
            rewrite_edge(&path, edge, current, parameter_locals)?;
        }
        MirTerminatorKind::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            *discr = rewrite_operand(&path, discr, current, promoted)?;
            for (_, edge) in targets {
                rewrite_edge(&path, edge, current, parameter_locals)?;
            }
            rewrite_edge(&path, otherwise, current, parameter_locals)?;
        }
        MirTerminatorKind::Call(call) => {
            rewrite_call(&path, call, current, parameter_locals, promoted)?;
        }
        MirTerminatorKind::Drop { target, unwind, .. } => {
            rewrite_edge(&path, target, current, parameter_locals)?;
            rewrite_unwind(&path, unwind, current, parameter_locals)?;
        }
        MirTerminatorKind::Assert {
            condition,
            target,
            unwind,
            ..
        } => {
            *condition = rewrite_operand(&path, condition, current, promoted)?;
            rewrite_edge(&path, target, current, parameter_locals)?;
            rewrite_unwind(&path, unwind, current, parameter_locals)?;
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn rewrite_call(
    path: &str,
    call: &mut MirCall,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    for argument in &mut call.arguments {
        *argument = rewrite_operand(path, argument, current, promoted)?;
    }
    if let Some(target) = &mut call.target {
        rewrite_edge(path, target, current, parameter_locals)?;
    }
    rewrite_unwind(path, &mut call.unwind, current, parameter_locals)
}

fn rewrite_unwind(
    path: &str,
    unwind: &mut MirUnwindAction,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
) -> Result<(), MirMem2RegError> {
    if let MirUnwindAction::Cleanup(edge) = unwind {
        rewrite_edge(path, edge, current, parameter_locals)?;
    }
    Ok(())
}

fn rewrite_edge(
    path: &str,
    edge: &mut MirEdge,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
) -> Result<(), MirMem2RegError> {
    if !edge.arguments.is_empty() {
        return Err(MirMem2RegError::new(
            path,
            "place-form input edge unexpectedly contains arguments",
        ));
    }
    edge.arguments = parameter_locals[edge.target.0 as usize]
        .iter()
        .map(|local| {
            current
                .get(local)
                .and_then(|values| values.last().copied())
                .map(MirOperand::Value)
                .ok_or_else(|| {
                    MirMem2RegError::new(
                        path,
                        format!("promoted local {} is undefined at an edge", local.0),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn inspect_statement(
    statement: &MirStatement,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
    reads: &mut BTreeSet<MirLocalId>,
    work_units: &mut usize,
) {
    charge_fact_work(work_units);
    match &statement.kind {
        MirStatementKind::Assign { place, value } => {
            inspect_place(
                place,
                PlaceUse::Destination,
                touched,
                disqualified,
                reads,
                work_units,
            );
            inspect_rvalue(value, touched, disqualified, reads, work_units);
        }
        MirStatementKind::Define { rvalue, .. } => {
            inspect_rvalue(rvalue, touched, disqualified, reads, work_units);
        }
        MirStatementKind::SetDiscriminant { place, .. } | MirStatementKind::Deinit(place) => {
            inspect_place(
                place,
                PlaceUse::UnsupportedRead,
                touched,
                disqualified,
                reads,
                work_units,
            );
        }
        MirStatementKind::StorageLive(local) | MirStatementKind::StorageDead(local) => {
            touched.insert(*local);
            disqualified.insert(*local);
        }
        MirStatementKind::Nop => {}
    }
}

fn inspect_rvalue(
    rvalue: &MirRvalue,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
    reads: &mut BTreeSet<MirLocalId>,
    work_units: &mut usize,
) {
    charge_fact_work(work_units);
    match rvalue {
        MirRvalue::Use(operand)
        | MirRvalue::UnaryOp { operand, .. }
        | MirRvalue::Cast { operand, .. }
        | MirRvalue::Repeat { operand, .. } => {
            inspect_operand(operand, touched, disqualified, reads, work_units);
        }
        MirRvalue::BinaryOp { lhs, rhs, .. } | MirRvalue::CheckedBinaryOp { lhs, rhs, .. } => {
            inspect_operand(lhs, touched, disqualified, reads, work_units);
            inspect_operand(rhs, touched, disqualified, reads, work_units);
        }
        MirRvalue::Ref { place, .. } | MirRvalue::AddressOf { place, .. } => {
            inspect_place(
                place,
                PlaceUse::Address,
                touched,
                disqualified,
                reads,
                work_units,
            );
        }
        MirRvalue::Len(place) | MirRvalue::Discriminant(place) => {
            inspect_place(
                place,
                PlaceUse::Read,
                touched,
                disqualified,
                reads,
                work_units,
            );
        }
        MirRvalue::Aggregate { operands, .. } => {
            for operand in operands {
                inspect_operand(operand, touched, disqualified, reads, work_units);
            }
        }
        MirRvalue::ThreadIndex1d => {}
    }
}

fn inspect_operand(
    operand: &MirOperand,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
    reads: &mut BTreeSet<MirLocalId>,
    work_units: &mut usize,
) {
    charge_fact_work(work_units);
    if let MirOperand::Copy(place) | MirOperand::Move(place) = operand {
        inspect_place(
            place,
            PlaceUse::Read,
            touched,
            disqualified,
            reads,
            work_units,
        );
    }
}

#[derive(Clone, Copy)]
enum PlaceUse {
    Read,
    Destination,
    Address,
    UnsupportedRead,
    UnsupportedDestination,
}

fn inspect_place(
    place: &MirPlace,
    usage: PlaceUse,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
    reads: &mut BTreeSet<MirLocalId>,
    work_units: &mut usize,
) {
    charge_fact_work(work_units);
    touched.insert(place.local);
    if !place.projection.is_empty()
        || matches!(
            usage,
            PlaceUse::Address | PlaceUse::UnsupportedRead | PlaceUse::UnsupportedDestination
        )
    {
        disqualified.insert(place.local);
    }
    if matches!(
        usage,
        PlaceUse::Read | PlaceUse::Address | PlaceUse::UnsupportedRead
    ) {
        reads.insert(place.local);
    }
    for projection in &place.projection {
        charge_fact_work(work_units);
        if let MirProjection::Index { local } = projection {
            touched.insert(*local);
            disqualified.insert(*local);
            if !matches!(
                usage,
                PlaceUse::Destination | PlaceUse::UnsupportedDestination
            ) {
                reads.insert(*local);
            }
        }
    }
}

fn inspect_terminator(
    terminator: &MirTerminatorKind,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
    reads: &mut BTreeSet<MirLocalId>,
    work_units: &mut usize,
) {
    charge_fact_work(work_units);
    match terminator {
        MirTerminatorKind::SwitchInt { discr, .. }
        | MirTerminatorKind::Assert {
            condition: discr, ..
        } => inspect_operand(discr, touched, disqualified, reads, work_units),
        MirTerminatorKind::Call(call) => {
            for argument in &call.arguments {
                inspect_operand(argument, touched, disqualified, reads, work_units);
            }
            if let Some(destination) = &call.destination {
                inspect_place(
                    destination,
                    PlaceUse::UnsupportedDestination,
                    touched,
                    disqualified,
                    reads,
                    work_units,
                );
            }
        }
        MirTerminatorKind::Drop { place, .. } => {
            inspect_place(
                place,
                PlaceUse::UnsupportedRead,
                touched,
                disqualified,
                reads,
                work_units,
            );
        }
        MirTerminatorKind::Goto(_) | MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {
        }
    }
}

fn charge_fact_work(work_units: &mut usize) {
    *work_units = work_units.saturating_add(1);
}

#[cfg(test)]
mod liveness_budget_tests {
    use super::*;

    #[test]
    fn storage_budget_accepts_the_boundary_and_rejects_the_next_item() {
        let mut budget = LivenessBudget::default();
        budget
            .reserve_storage(7, MAX_MEM2REG_LIVENESS_STORAGE_ITEMS)
            .unwrap();

        let error = budget.reserve_storage(7, 1).unwrap_err();
        assert_eq!(error.path(), "module.functions[7].body");
        assert_eq!(
            error.reason(),
            format!(
                "mem2reg liveness requires {} storage items, exceeding the deterministic limit {MAX_MEM2REG_LIVENESS_STORAGE_ITEMS}",
                MAX_MEM2REG_LIVENESS_STORAGE_ITEMS + 1
            )
        );
    }

    #[test]
    fn work_budget_accepts_the_boundary_and_rejects_the_next_item() {
        let mut budget = LivenessBudget::default();
        budget
            .charge_work(11, MAX_MEM2REG_LIVENESS_WORK_UNITS)
            .unwrap();

        let error = budget.charge_work(11, 1).unwrap_err();
        assert_eq!(error.path(), "module.functions[11].body");
        assert_eq!(
            error.reason(),
            format!(
                "mem2reg liveness consumed {} work units, exceeding the deterministic limit {MAX_MEM2REG_LIVENESS_WORK_UNITS}",
                MAX_MEM2REG_LIVENESS_WORK_UNITS + 1
            )
        );
    }
}
