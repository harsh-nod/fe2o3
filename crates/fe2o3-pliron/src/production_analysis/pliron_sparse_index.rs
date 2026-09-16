//! Sparse SSA propagation for target-neutral kernel index expressions.
//!
//! This is deliberately a value analysis, not a race detector. It derives
//! bounded unsigned formulas from SSA definitions and records the launch
//! domain named by `kernel.invocation_index`. Memory and synchronization
//! passes consume these facts without duplicating expression recognition.

use std::collections::{HashMap, VecDeque};

use dialect_kernel::{
    CheckedRowStripedIndex2DOp, CheckedTiledIndex2DOp, DimensionOp, IndexBinaryKindAttr,
    IndexBinaryOp, IndexConstantOp, InvocationIndexOp, MAX_RANKED_MEMORY_RANK, RankedViewOp,
    ranked_view_type,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    r#type::{Typed, TypedHandle},
    value::Value,
};

use crate::production_analysis::pliron_control_edges_v1::{ControlErrorV1, ControlViewV1};
#[cfg(test)]
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryFailureV1;
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
#[cfg(test)]
use dialect_kernel::{BranchArgsOp, IndexEqualBranchArgsOp, IndexLessThanBranchArgsOp};

pub const MAX_SPARSE_INDEX_VALUES_V1: usize = 65_536;
pub const MAX_SPARSE_INDEX_USES_V1: usize = 262_144;
pub const MAX_SPARSE_INDEX_WORK_UNITS_V1: usize = 1_048_576;

include!("pliron_sparse_index/resources_v1.rs");
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SparseIndexFailureV1 {
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    InconsistentLaunchExtent {
        dimension: usize,
        first: u64,
        second: u64,
    },
    MalformedControlFlow {
        detail: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseAffineIndexV1 {
    constant: u64,
    coefficients: [u64; MAX_RANKED_MEMORY_RANK],
}

impl SparseAffineIndexV1 {
    fn constant(value: u64) -> Self {
        Self {
            constant: value,
            coefficients: [0; MAX_RANKED_MEMORY_RANK],
        }
    }

    fn invocation(dimension: usize) -> Self {
        let mut coefficients = [0; MAX_RANKED_MEMORY_RANK];
        coefficients[dimension] = 1;
        Self {
            constant: 0,
            coefficients,
        }
    }

    fn checked_add(&self, other: &Self) -> Option<Self> {
        let mut coefficients = [0; MAX_RANKED_MEMORY_RANK];
        for (result, (lhs, rhs)) in coefficients
            .iter_mut()
            .zip(self.coefficients.iter().zip(other.coefficients))
        {
            *result = lhs.checked_add(rhs)?;
        }
        Some(Self {
            constant: self.constant.checked_add(other.constant)?,
            coefficients,
        })
    }

    fn checked_scale(&self, factor: u64) -> Option<Self> {
        let mut coefficients = [0; MAX_RANKED_MEMORY_RANK];
        for (result, coefficient) in coefficients.iter_mut().zip(self.coefficients) {
            *result = coefficient.checked_mul(factor)?;
        }
        Some(Self {
            constant: self.constant.checked_mul(factor)?,
            coefficients,
        })
    }

    pub const fn constant_term(&self) -> u64 {
        self.constant
    }

    pub const fn coefficients(&self) -> &[u64; MAX_RANKED_MEMORY_RANK] {
        &self.coefficients
    }

    pub fn evaluate(&self, invocation: &[u64]) -> Option<u64> {
        let mut value = self.constant;
        for (dimension, coefficient) in self.coefficients.iter().copied().enumerate() {
            let coordinate = invocation.get(dimension).copied().unwrap_or(0);
            value = value.checked_add(coefficient.checked_mul(coordinate)?)?;
        }
        Some(value)
    }

    pub fn maximum(&self, launch_extents: &[u64]) -> Option<u64> {
        let mut value = self.constant;
        for (dimension, coefficient) in self.coefficients.iter().copied().enumerate() {
            // An unused axis need not have a finite extent. A used one must.
            if coefficient == 0 {
                continue;
            }
            let coordinate = launch_extents.get(dimension)?.checked_sub(1)?;
            value = value.checked_add(coefficient.checked_mul(coordinate)?)?;
        }
        Some(value)
    }

    fn is_constant(&self) -> Option<u64> {
        self.coefficients
            .iter()
            .all(|coefficient| *coefficient == 0)
            .then_some(self.constant)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SparseIndexFactV1 {
    Unknown,
    Affine(SparseAffineIndexV1),
    MachineOverflow(SparseMachineOverflowV1),
    Remainder {
        dividend: SparseAffineIndexV1,
        modulus: u64,
    },
    CheckedTiled2D(SparseCheckedTiledIndex2DV1),
    CheckedRowStriped2D(SparseCheckedRowStripedIndex2DV1),
}

/// Concrete checked-integer failure retained through sparse SSA propagation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseMachineOverflowV1 {
    operation: IndexBinaryKindAttr,
    invocation: Vec<u64>,
    lhs: u64,
    rhs: u64,
}

impl SparseMachineOverflowV1 {
    pub const fn operation(&self) -> IndexBinaryKindAttr {
        self.operation
    }

    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn operands(&self) -> (u64, u64) {
        (self.lhs, self.rhs)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseCheckedTiledIndex2DV1 {
    invocation: SparseAffineIndexV1,
    component: Value,
    rows: Value,
    columns: Value,
    row_stride: Value,
    geometry: [u64; 4],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseCheckedRowStripedIndex2DV1 {
    invocation: SparseAffineIndexV1,
    component: Value,
    rows: Value,
    columns: Value,
    row_stride: Value,
    geometry: [u64; 2],
}

impl SparseCheckedRowStripedIndex2DV1 {
    pub const fn invocation(&self) -> &SparseAffineIndexV1 {
        &self.invocation
    }

    pub const fn component(&self) -> Value {
        self.component
    }

    pub const fn runtime_layout(&self) -> [Value; 3] {
        [self.rows, self.columns, self.row_stride]
    }

    pub const fn geometry(&self) -> [u64; 2] {
        self.geometry
    }
}

impl SparseCheckedTiledIndex2DV1 {
    pub const fn invocation(&self) -> &SparseAffineIndexV1 {
        &self.invocation
    }

    pub const fn component(&self) -> Value {
        self.component
    }

    pub const fn runtime_layout(&self) -> [Value; 3] {
        [self.rows, self.columns, self.row_stride]
    }

    pub const fn geometry(&self) -> [u64; 4] {
        self.geometry
    }
}

impl SparseIndexFactV1 {
    pub const fn affine(&self) -> Option<&SparseAffineIndexV1> {
        match self {
            Self::Affine(affine) => Some(affine),
            Self::Unknown
            | Self::MachineOverflow(_)
            | Self::Remainder { .. }
            | Self::CheckedTiled2D(_)
            | Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub fn constant_value(&self) -> Option<u64> {
        match self {
            Self::Affine(affine) => affine.is_constant(),
            Self::Remainder { dividend, modulus } if *modulus != 0 => {
                dividend.is_constant().map(|value| value % modulus)
            }
            Self::Unknown
            | Self::MachineOverflow(_)
            | Self::Remainder { .. }
            | Self::CheckedTiled2D(_)
            | Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub fn evaluate(&self, invocation: &[u64]) -> Option<u64> {
        match self {
            Self::Unknown => None,
            Self::MachineOverflow(_) => None,
            Self::Affine(affine) => affine.evaluate(invocation),
            Self::Remainder { dividend, modulus } if *modulus != 0 => {
                dividend.evaluate(invocation).map(|value| value % modulus)
            }
            Self::Remainder { .. } => None,
            Self::CheckedTiled2D(_) => None,
            Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub fn maximum(&self, launch_extents: &[u64]) -> Option<u64> {
        match self {
            Self::Unknown => None,
            Self::MachineOverflow(_) => None,
            Self::Affine(affine) => affine.maximum(launch_extents),
            Self::Remainder { modulus, .. } => modulus.checked_sub(1),
            Self::CheckedTiled2D(_) => None,
            Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub const fn checked_tiled_2d(&self) -> Option<&SparseCheckedTiledIndex2DV1> {
        match self {
            Self::CheckedTiled2D(fact) => Some(fact),
            _ => None,
        }
    }

    pub const fn machine_overflow(&self) -> Option<&SparseMachineOverflowV1> {
        match self {
            Self::MachineOverflow(overflow) => Some(overflow),
            _ => None,
        }
    }

    pub const fn checked_row_striped_2d(&self) -> Option<&SparseCheckedRowStripedIndex2DV1> {
        match self {
            Self::CheckedRowStriped2D(fact) => Some(fact),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SparseIndexAnalysisV1 {
    owner: Ptr<Operation>,
    facts: HashMap<Value, SparseValueFactsV1>,
    launch_extents: Vec<u64>,
    declared_launch_extents: Vec<Option<u64>>,
}

impl SparseIndexAnalysisV1 {
    pub(crate) fn fact_ref(&self, value: Value) -> &SparseIndexFactV1 {
        self.facts
            .get(&value)
            .map(|facts| &facts.numeric)
            .unwrap_or(&SparseIndexFactV1::Unknown)
    }

    pub fn fact(&self, value: Value) -> SparseIndexFactV1 {
        self.fact_ref(value).clone()
    }

    pub(crate) fn stable_root(
        &self,
        context: &Context,
        function: &FuncOp,
        value: Value,
    ) -> Option<SparseStableRootV1> {
        if function.get_operation() != self.owner || self.owner.try_deref(context).is_err() {
            return None;
        }
        self.facts.get(&value).and_then(|facts| facts.stable)
    }

    pub fn launch_extents(&self) -> &[u64] {
        &self.launch_extents
    }

    pub fn invocation_count(&self) -> Option<u64> {
        self.launch_extents
            .iter()
            .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
    }

    /// Returns the extent explicitly carried by an invocation-coordinate
    /// producer. The execution layout remains the authoritative full domain;
    /// this records only consistency constraints from SSA coordinate uses.
    pub fn declared_launch_extent(&self, dimension: usize) -> Option<u64> {
        self.declared_launch_extents
            .get(dimension)
            .copied()
            .flatten()
    }

    pub fn has_declared_launch_extent(&self) -> bool {
        self.declared_launch_extents.iter().any(Option::is_some)
    }
}

#[derive(Clone, Debug)]
struct SparseDefinitionV1 {
    kind: SparseDefinitionKindV1,
    result: Value,
}

#[derive(Clone, Debug)]
enum SparseDefinitionKindV1 {
    Operation(Ptr<Operation>),
    EntryArgument { ordinal: usize },
    Merge(Vec<Value>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the dense hot fixed-point lattice avoids one allocation and pointer chase per SSA value"
)]
enum SparseIndexLatticeV1 {
    Pending,
    Known(SparseIndexFactV1),
}

include!("pliron_sparse_index/stable_roots_v1.rs");
include!("pliron_sparse_index/propagation_v1.rs");

#[derive(Clone, Debug)]
struct SparseEdgeV1 {
    source: usize,
    target: usize,
    successor: usize,
}

#[cfg(test)]
pub(crate) fn analyze_pliron_sparse_indices_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<SparseIndexAnalysisV1, SparseIndexFailureV1> {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function)
        .map_err(sparse_inventory_failure)?;
    analyze_pliron_sparse_indices_with_inventory_v1(context, function, &inventory)
}

pub(crate) fn analyze_pliron_sparse_indices_with_inventory_v1(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<SparseIndexAnalysisV1, SparseIndexFailureV1> {
    analyze_pliron_sparse_indices_with_work_v1(context, function, inventory)
        .map(|(analysis, _work)| analysis)
}

fn analyze_pliron_sparse_indices_with_work_v1(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<(SparseIndexAnalysisV1, usize), SparseIndexFailureV1> {
    let entry = function.get_entry_block(context);
    let blocks = inventory.blocks().to_vec();
    let mut block_indices = HashMap::new();
    let mut block_arguments = HashMap::new();
    for (index, block) in blocks.iter().copied().enumerate() {
        if index == MAX_SPARSE_INDEX_VALUES_V1 {
            return Err(limit("CFG block", MAX_SPARSE_INDEX_VALUES_V1, index + 1));
        }
        block_indices.insert(block, index);
        block_arguments.insert(block, block.deref(context).arguments().collect::<Vec<_>>());
    }

    let mut value_count = 0;
    for arguments in block_arguments.values() {
        charge_sparse_values_v1(&mut value_count, arguments.len())?;
    }
    for site in inventory.operations() {
        charge_sparse_values_v1(
            &mut value_count,
            site.pointer().deref(context).get_num_results(),
        )?;
    }
    // A dense sidecar shares the existing definition indices. Block argument
    // types need one roster search each in pinned Pliron; payloads do not.
    let mut definition_types = Vec::with_capacity(value_count);
    let mut preparation_work = 0_usize;
    let mut definitions = Vec::new();
    let mut definition_indices = HashMap::new();
    let mut input_count = 0_usize;
    let mut launch_extents = Vec::new();
    for (block_index, block) in blocks.iter().copied().enumerate() {
        for (ordinal, argument) in block_arguments
            .get(&block)
            .expect("collected block has arguments")
            .iter()
            .enumerate()
        {
            push_definition(
                &mut definitions,
                &mut definition_indices,
                SparseDefinitionV1 {
                    kind: if block == entry {
                        SparseDefinitionKindV1::EntryArgument { ordinal }
                    } else {
                        SparseDefinitionKindV1::Merge(Vec::new())
                    },
                    result: *argument,
                },
            )?;
            charge_work(&mut preparation_work, ordinal + 1)?;
            definition_types.push(argument.get_type(context));
        }
        for site in inventory.block_operations(block_index) {
            let operation = site.pointer();
            let raw = operation.deref(context);
            charge_uses(&mut input_count, raw.get_num_operands())?;
            let dynamic = Operation::get_op_dyn(operation, context);
            if let Some(invocation) = dynamic.downcast_ref::<InvocationIndexOp>() {
                record_launch_extent(invocation, context, &mut launch_extents)?;
            }
            for result_index in 0..raw.get_num_results() {
                push_definition(
                    &mut definitions,
                    &mut definition_indices,
                    SparseDefinitionV1 {
                        kind: SparseDefinitionKindV1::Operation(operation),
                        result: raw.get_result(result_index),
                    },
                )?;
                definition_types.push(raw.get_type(result_index));
            }
        }
    }

    let mut edges = Vec::new();
    let mut successors = vec![Vec::new(); blocks.len()];
    for (source, block) in blocks.iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        let control =
            ControlViewV1::observe(context, terminator).map_err(sparse_control_error_v1)?;
        charge_uses(&mut input_count, control.successor_count())?;
        for successor_index in 0..control.successor_count() {
            let edge = control
                .edge(successor_index)
                .map_err(|_| malformed("typed edge operand and block argument counts differ"))?;
            let successor = edge.target();
            let Some(&target) = block_indices.get(&successor) else {
                return Err(malformed("a branch targets a block outside the kernel"));
            };
            for index in 0..edge.argument_count() {
                let (incoming, argument) = edge
                    .argument_at(index)
                    .map_err(|_| malformed("typed edge has an invalid argument ordinal"))?;
                let incoming_index = definition_indices
                    .get(&incoming)
                    .ok_or_else(|| malformed("edge operand is not defined in this function"))?;
                let argument_index = definition_indices
                    .get(&argument)
                    .ok_or_else(|| malformed("block argument has no sparse definition"))?;
                if definition_types[*incoming_index] != definition_types[*argument_index] {
                    return Err(malformed(
                        "typed edge operand and block argument types differ",
                    ));
                }
            }
            if successor == entry && edge.argument_count() != 0 {
                return Err(malformed(
                    "an entry argument cannot receive a CFG edge operand",
                ));
            }
            let edge_index = edges.len();
            edges.push(SparseEdgeV1 {
                source,
                target,
                successor: successor_index,
            });
            successors[source].push(edge_index);
        }
    }

    let mut reachable = vec![false; blocks.len()];
    let Some(&entry_index) = block_indices.get(&entry) else {
        return Err(malformed("the function entry block is outside its body"));
    };
    reachable[entry_index] = true;
    let mut reachable_worklist = VecDeque::from([entry_index]);
    while let Some(source) = reachable_worklist.pop_front() {
        charge_work(&mut preparation_work, 1)?;
        for edge_index in &successors[source] {
            charge_work(&mut preparation_work, 1)?;
            let target = edges[*edge_index].target;
            if !reachable[target] {
                reachable[target] = true;
                reachable_worklist.push_back(target);
            }
        }
    }

    for edge in edges.iter().filter(|edge| reachable[edge.source]) {
        let terminator = blocks[edge.source]
            .deref(context)
            .get_terminator(context)
            .ok_or_else(|| malformed("reachable edge has no terminator"))?;
        let control =
            ControlViewV1::observe(context, terminator).map_err(sparse_control_error_v1)?;
        let edge = control
            .edge(edge.successor)
            .map_err(|_| malformed("typed edge has an invalid successor ordinal"))?;
        for index in 0..edge.argument_count() {
            let (incoming, argument) = edge
                .argument_at(index)
                .map_err(|_| malformed("typed edge has an invalid argument ordinal"))?;
            let index = definition_indices
                .get(&argument)
                .copied()
                .ok_or_else(|| malformed("block argument has no sparse definition"))?;
            let SparseDefinitionKindV1::Merge(inputs) = &mut definitions[index].kind else {
                return Err(malformed("an entry argument receives a reachable CFG edge"));
            };
            inputs.push(incoming);
        }
    }

    let mut consumers: HashMap<Value, Vec<usize>> = HashMap::new();
    let mut consumer_count = 0_usize;
    for (index, definition) in definitions.iter().enumerate() {
        let dependencies = match &definition.kind {
            SparseDefinitionKindV1::Operation(operation) => {
                operation_dependencies(context, *operation)
            }
            SparseDefinitionKindV1::Merge(inputs) => inputs.clone(),
            SparseDefinitionKindV1::EntryArgument { .. } => Vec::new(),
        };
        charge_uses(&mut consumer_count, dependencies.len())?;
        for dependency in dependencies {
            consumers.entry(dependency).or_default().push(index);
        }
    }

    let resolved_launch_extents = if launch_extents.is_empty() {
        vec![1]
    } else {
        launch_extents
            .iter()
            .map(|extent| extent.unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let mut propagation_work = preparation_work;
    let mut roots =
        initialize_sparse_stable_roots_v1(context, entry, &definitions, &mut propagation_work)?;
    charge_work(&mut propagation_work, definitions.len())?;
    let mut publications = vec![0_u8; definitions.len()];
    let mut lattice = vec![SparseIndexLatticeV1::Pending; definitions.len()];
    let mut pending = (0..definitions.len()).collect::<VecDeque<_>>();
    let mut queued = vec![true; definitions.len()];
    let mut pending_roots_closed = false;
    loop {
        let Some(index) = pending.pop_front() else {
            if pending_roots_closed {
                break;
            }
            pending_roots_closed = true;
            // A provisional anchor must not hide an unresolved incoming cycle.
            // Poison every residual Pending root, then drain its consumers.
            for (index, root) in roots.iter_mut().enumerate() {
                let next = match root {
                    SparseStableRootLatticeV1::Pending => SparseStableRootLatticeV1::Unknown,
                    _ => *root,
                };
                if publish_sparse_stable_root_v1(root, next, &mut propagation_work)? {
                    enqueue_sparse_consumers_v1(
                        definitions[index].result,
                        &consumers,
                        &mut queued,
                        &mut pending,
                        &mut propagation_work,
                    )?;
                }
            }
            continue;
        };
        queued[index] = false;
        charge_work(&mut propagation_work, 1)?;
        let next = derive_definition(
            context,
            &definitions[index],
            &lattice,
            &definition_indices,
            &resolved_launch_extents,
            &mut propagation_work,
        )?;
        let numeric_changed = publish_sparse_fact_v1(
            &mut lattice[index],
            next,
            &mut publications[index],
            &mut propagation_work,
        )?;
        let root = derive_sparse_stable_root_v1(
            &definitions[index],
            roots[index],
            &roots,
            &definition_indices,
            &mut propagation_work,
        )?;
        let root_changed =
            publish_sparse_stable_root_v1(&mut roots[index], root, &mut propagation_work)?;
        if numeric_changed || root_changed {
            enqueue_sparse_consumers_v1(
                definitions[index].result,
                &consumers,
                &mut queued,
                &mut pending,
                &mut propagation_work,
            )?;
        }
    }

    let facts = definitions
        .iter()
        .zip(lattice)
        .zip(roots)
        .map(|((definition, lattice), root)| {
            let fact = match lattice {
                SparseIndexLatticeV1::Pending => SparseIndexFactV1::Unknown,
                SparseIndexLatticeV1::Known(fact) => fact,
            };
            (
                definition.result,
                SparseValueFactsV1 {
                    numeric: fact,
                    stable: match root {
                        SparseStableRootLatticeV1::Known(root) => Some(root),
                        SparseStableRootLatticeV1::Pending | SparseStableRootLatticeV1::Unknown => {
                            None
                        }
                    },
                },
            )
        })
        .collect();
    let declared_launch_extents = launch_extents.clone();
    Ok((
        SparseIndexAnalysisV1 {
            owner: function.get_operation(),
            facts,
            launch_extents: resolved_launch_extents,
            declared_launch_extents,
        },
        propagation_work,
    ))
}

#[cfg(test)]
fn sparse_inventory_failure(
    failure: BoundedPlironFunctionInventoryFailureV1,
) -> SparseIndexFailureV1 {
    SparseIndexFailureV1::ResourceLimit {
        resource: failure.resource(),
        limit: failure.limit(),
        actual: failure.actual(),
    }
}

fn push_definition(
    definitions: &mut Vec<SparseDefinitionV1>,
    definition_indices: &mut HashMap<Value, usize>,
    definition: SparseDefinitionV1,
) -> Result<(), SparseIndexFailureV1> {
    if definitions.len() == MAX_SPARSE_INDEX_VALUES_V1 {
        return Err(limit(
            "SSA value",
            MAX_SPARSE_INDEX_VALUES_V1,
            definitions.len() + 1,
        ));
    }
    let index = definitions.len();
    definition_indices.insert(definition.result, index);
    definitions.push(definition);
    Ok(())
}

fn charge_sparse_values_v1(
    count: &mut usize,
    additional: usize,
) -> Result<(), SparseIndexFailureV1> {
    let actual = count.saturating_add(additional);
    if actual > MAX_SPARSE_INDEX_VALUES_V1 {
        return Err(limit("SSA value", MAX_SPARSE_INDEX_VALUES_V1, actual));
    }
    *count = actual;
    Ok(())
}

fn sparse_control_error_v1(error: ControlErrorV1) -> SparseIndexFailureV1 {
    malformed(match error {
        ControlErrorV1::OperandCount => "typed conditional edge has a malformed operand count",
        ControlErrorV1::MissingEdgeArguments => {
            "a block argument has a predecessor without typed edge operands"
        }
        _ => "unsupported or malformed control edge shape",
    })
}

fn record_launch_extent(
    invocation: &InvocationIndexOp,
    context: &Context,
    launch_extents: &mut Vec<Option<u64>>,
) -> Result<(), SparseIndexFailureV1> {
    let Some(dimension) = invocation
        .dimension(context)
        .and_then(|dimension| usize::try_from(dimension).ok())
        .filter(|dimension| *dimension < MAX_RANKED_MEMORY_RANK)
    else {
        return Ok(());
    };
    let extent = invocation.launch_extent(context).unwrap_or(0);
    if launch_extents.len() <= dimension {
        launch_extents.resize(dimension + 1, None);
    }
    match launch_extents[dimension] {
        None => launch_extents[dimension] = Some(extent),
        Some(first) if first != extent => {
            return Err(SparseIndexFailureV1::InconsistentLaunchExtent {
                dimension,
                first,
                second: extent,
            });
        }
        _ => {}
    }
    Ok(())
}

fn operation_dependencies(context: &Context, operation: Ptr<Operation>) -> Vec<Value> {
    let mut dependencies = operation.deref(context).operands().collect::<Vec<_>>();
    let dynamic = Operation::get_op_dyn(operation, context);
    if let Some(dimension) = dynamic.downcast_ref::<DimensionOp>()
        && let Some(dimension_index) = dimension
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
    {
        let view = dimension.view(context);
        if let Some(definition) = view.defining_op() {
            let definition = Operation::get_op_dyn(definition, context);
            if let Some(view) = definition.downcast_ref::<RankedViewOp>()
                && let Some(extent) = view.dynamic_extent(context, dimension_index)
                && !dependencies.contains(&extent)
            {
                dependencies.push(extent);
            }
        }
    }
    dependencies
}

fn derive_operation(
    context: &Context,
    operation: Ptr<Operation>,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
    launch_extents: &[u64],
) -> SparseIndexLatticeV1 {
    let operation = Operation::get_op_dyn(operation, context);
    if let Some(constant) = operation.downcast_ref::<IndexConstantOp>() {
        return known(
            constant
                .value(context)
                .map(SparseAffineIndexV1::constant)
                .map(SparseIndexFactV1::Affine)
                .unwrap_or(SparseIndexFactV1::Unknown),
        );
    }
    if let Some(invocation) = operation.downcast_ref::<InvocationIndexOp>() {
        let Some(dimension) = invocation
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
            .filter(|dimension| *dimension < MAX_RANKED_MEMORY_RANK)
        else {
            return known(SparseIndexFactV1::Unknown);
        };
        return known(SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(
            dimension,
        )));
    }
    if let Some(dimension) = operation.downcast_ref::<DimensionOp>() {
        let Some(dimension_index) = dimension
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
        else {
            return known(SparseIndexFactV1::Unknown);
        };
        let view = dimension.view(context);
        if let Some(view_type) = ranked_view_type(view, context) {
            let view_type: TypedHandle<dialect_kernel::RankedViewType> = view_type;
            let Some(extent) = view_type
                .deref(context)
                .shape()
                .get(dimension_index)
                .copied()
            else {
                return known(SparseIndexFactV1::Unknown);
            };
            if extent != dialect_kernel::DYNAMIC_EXTENT {
                return known(SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(
                    extent,
                )));
            }
            if let Some(definition) = view.defining_op() {
                let definition = Operation::get_op_dyn(definition, context);
                if let Some(view) = definition.downcast_ref::<RankedViewOp>()
                    && let Some(extent) = view.dynamic_extent(context, dimension_index)
                {
                    return lookup(extent, lattice, definition_indices);
                }
            }
        }
        return known(SparseIndexFactV1::Unknown);
    }
    if let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() {
        let lhs = lookup(binary.lhs(context), lattice, definition_indices);
        let rhs = lookup(binary.rhs(context), lattice, definition_indices);
        let (SparseIndexLatticeV1::Known(lhs), SparseIndexLatticeV1::Known(rhs)) = (lhs, rhs)
        else {
            return SparseIndexLatticeV1::Pending;
        };
        return known(derive_binary(
            binary.kind(context),
            lhs,
            rhs,
            launch_extents,
        ));
    }
    if let Some(tiled) = operation.downcast_ref::<CheckedTiledIndex2DOp>() {
        let [invocation, component, rows, columns, row_stride] = tiled.operands(context);
        let SparseIndexLatticeV1::Known(invocation) =
            lookup(invocation, lattice, definition_indices)
        else {
            return SparseIndexLatticeV1::Pending;
        };
        let Some(invocation) = invocation.affine() else {
            return known(SparseIndexFactV1::Unknown);
        };
        let Some(geometry) = tiled.geometry(context) else {
            return known(SparseIndexFactV1::Unknown);
        };
        return known(SparseIndexFactV1::CheckedTiled2D(
            SparseCheckedTiledIndex2DV1 {
                invocation: invocation.clone(),
                component,
                rows,
                columns,
                row_stride,
                geometry,
            },
        ));
    }
    if let Some(striped) = operation.downcast_ref::<CheckedRowStripedIndex2DOp>() {
        let [invocation, component, rows, columns, row_stride] = striped.operands(context);
        let SparseIndexLatticeV1::Known(invocation) =
            lookup(invocation, lattice, definition_indices)
        else {
            return SparseIndexLatticeV1::Pending;
        };
        let Some(invocation) = invocation.affine() else {
            return known(SparseIndexFactV1::Unknown);
        };
        let Some(geometry) = striped.geometry(context) else {
            return known(SparseIndexFactV1::Unknown);
        };
        return known(SparseIndexFactV1::CheckedRowStriped2D(
            SparseCheckedRowStripedIndex2DV1 {
                invocation: invocation.clone(),
                component,
                rows,
                columns,
                row_stride,
                geometry,
            },
        ));
    }
    known(SparseIndexFactV1::Unknown)
}

const fn known(fact: SparseIndexFactV1) -> SparseIndexLatticeV1 {
    SparseIndexLatticeV1::Known(fact)
}

fn charge_uses(use_count: &mut usize, additional: usize) -> Result<(), SparseIndexFailureV1> {
    *use_count = use_count.saturating_add(additional);
    if *use_count > MAX_SPARSE_INDEX_USES_V1 {
        return Err(limit(
            "SSA use or CFG edge",
            MAX_SPARSE_INDEX_USES_V1,
            *use_count,
        ));
    }
    Ok(())
}

const fn malformed(detail: &'static str) -> SparseIndexFailureV1 {
    SparseIndexFailureV1::MalformedControlFlow { detail }
}

fn derive_binary(
    kind: Option<IndexBinaryKindAttr>,
    lhs: SparseIndexFactV1,
    rhs: SparseIndexFactV1,
    launch_extents: &[u64],
) -> SparseIndexFactV1 {
    if let SparseIndexFactV1::MachineOverflow(overflow) = lhs {
        return SparseIndexFactV1::MachineOverflow(overflow);
    }
    if let SparseIndexFactV1::MachineOverflow(overflow) = rhs {
        return SparseIndexFactV1::MachineOverflow(overflow);
    }
    let (SparseIndexFactV1::Affine(lhs), SparseIndexFactV1::Affine(rhs)) = (lhs, rhs) else {
        return SparseIndexFactV1::Unknown;
    };
    match kind {
        Some(IndexBinaryKindAttr::Add) => match lhs.checked_add(&rhs) {
            Some(result) if affine_range_is_not_definitely_overflowing(&result, launch_extents) => {
                SparseIndexFactV1::Affine(result)
            }
            _ => overflow_or_unknown(
                IndexBinaryKindAttr::Add,
                &lhs,
                &rhs,
                launch_extents,
                u64::checked_add,
            ),
        },
        Some(IndexBinaryKindAttr::Multiply) => match (lhs.is_constant(), rhs.is_constant()) {
            (Some(factor), _) => match rhs.checked_scale(factor) {
                Some(result)
                    if affine_range_is_not_definitely_overflowing(&result, launch_extents) =>
                {
                    SparseIndexFactV1::Affine(result)
                }
                _ => overflow_or_unknown(
                    IndexBinaryKindAttr::Multiply,
                    &lhs,
                    &rhs,
                    launch_extents,
                    u64::checked_mul,
                ),
            },
            (_, Some(factor)) => match lhs.checked_scale(factor) {
                Some(result)
                    if affine_range_is_not_definitely_overflowing(&result, launch_extents) =>
                {
                    SparseIndexFactV1::Affine(result)
                }
                _ => overflow_or_unknown(
                    IndexBinaryKindAttr::Multiply,
                    &lhs,
                    &rhs,
                    launch_extents,
                    u64::checked_mul,
                ),
            },
            _ => SparseIndexFactV1::Unknown,
        },
        Some(IndexBinaryKindAttr::Remainder) => rhs
            .is_constant()
            .filter(|modulus| *modulus != 0)
            .map(|modulus| SparseIndexFactV1::Remainder {
                dividend: lhs,
                modulus,
            })
            .unwrap_or(SparseIndexFactV1::Unknown),
        Some(IndexBinaryKindAttr::Divide) => SparseIndexFactV1::Unknown,
        None => SparseIndexFactV1::Unknown,
    }
}

fn overflow_or_unknown(
    operation: IndexBinaryKindAttr,
    lhs: &SparseAffineIndexV1,
    rhs: &SparseAffineIndexV1,
    launch_extents: &[u64],
    checked: fn(u64, u64) -> Option<u64>,
) -> SparseIndexFactV1 {
    let Some(invocation) = maximum_known_invocation(&[lhs, rhs], launch_extents) else {
        return SparseIndexFactV1::Unknown;
    };
    let (Some(lhs), Some(rhs)) = (lhs.evaluate(&invocation), rhs.evaluate(&invocation)) else {
        return SparseIndexFactV1::Unknown;
    };
    if checked(lhs, rhs).is_some() {
        return SparseIndexFactV1::Unknown;
    }
    SparseIndexFactV1::MachineOverflow(SparseMachineOverflowV1 {
        operation,
        invocation,
        lhs,
        rhs,
    })
}

/// Uses zero for an unbounded axis only when every expression is independent
/// of that axis. The resulting point is therefore a witness for every
/// non-empty runtime extent, not an assumed upper bound.
fn maximum_known_invocation(
    expressions: &[&SparseAffineIndexV1],
    launch_extents: &[u64],
) -> Option<Vec<u64>> {
    launch_extents
        .iter()
        .copied()
        .enumerate()
        .map(|(dimension, extent)| {
            if extent != 0 {
                return extent.checked_sub(1);
            }
            expressions
                .iter()
                .all(|expression| expression.coefficients[dimension] == 0)
                .then_some(0)
        })
        .collect()
}

fn affine_range_is_not_definitely_overflowing(
    expression: &SparseAffineIndexV1,
    launch_extents: &[u64],
) -> bool {
    maximum_known_invocation(&[expression], launch_extents)
        .is_none_or(|invocation| expression.evaluate(&invocation).is_some())
}

const fn limit(resource: &'static str, limit: usize, actual: usize) -> SparseIndexFailureV1 {
    SparseIndexFailureV1::ResourceLimit {
        resource,
        limit,
        actual,
    }
}

include!("pliron_sparse_index/resource_tests.rs");
include!("pliron_sparse_index/local_bound_tests.rs");

#[cfg(test)]
#[path = "pliron_sparse_index/constant_remainder_tests.rs"]
mod constant_remainder_tests;

#[cfg(test)]
#[path = "pliron_sparse_index/native_control_v1_tests.rs"]
mod native_control_v1_tests;
