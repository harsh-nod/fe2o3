//! Sparse SSA propagation for target-neutral kernel index expressions.
//!
//! This is deliberately a value analysis, not a race detector. It derives
//! bounded unsigned formulas from SSA definitions and records the launch
//! domain named by `kernel.invocation_index`. Memory and synchronization
//! passes consume these facts without duplicating expression recognition.

use std::collections::{HashMap, VecDeque};

use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, CheckedRowStripedIndex2DOp, CheckedTiledIndex2DOp, DimensionOp,
    IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexLessThanBranchArgsOp, InvocationIndexOp, MAX_RANKED_MEMORY_RANK, RankedViewOp,
    ranked_view_type,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::OpBox,
    operation::Operation,
    r#type::{Typed, TypedHandle},
    value::Value,
};

pub const MAX_SPARSE_INDEX_VALUES_V1: usize = 65_536;
pub const MAX_SPARSE_INDEX_USES_V1: usize = 262_144;
pub const MAX_SPARSE_INDEX_WORK_UNITS_V1: usize = 1_048_576;

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
        let invocation = launch_extents
            .iter()
            .map(|extent| extent.checked_sub(1))
            .collect::<Option<Vec<_>>>()?;
        self.evaluate(&invocation)
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
    Remainder {
        dividend: SparseAffineIndexV1,
        modulus: u64,
    },
    CheckedTiled2D(SparseCheckedTiledIndex2DV1),
    CheckedRowStriped2D(SparseCheckedRowStripedIndex2DV1),
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
            | Self::Remainder { .. }
            | Self::CheckedTiled2D(_)
            | Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub fn constant_value(&self) -> Option<u64> {
        match self {
            Self::Affine(affine) => affine.is_constant(),
            Self::Unknown
            | Self::Remainder { .. }
            | Self::CheckedTiled2D(_)
            | Self::CheckedRowStriped2D(_) => None,
        }
    }

    pub fn evaluate(&self, invocation: &[u64]) -> Option<u64> {
        match self {
            Self::Unknown => None,
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

    pub const fn checked_row_striped_2d(&self) -> Option<&SparseCheckedRowStripedIndex2DV1> {
        match self {
            Self::CheckedRowStriped2D(fact) => Some(fact),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SparseIndexAnalysisV1 {
    facts: HashMap<Value, SparseIndexFactV1>,
    launch_extents: Vec<u64>,
    declared_launch_extents: Vec<Option<u64>>,
}

impl SparseIndexAnalysisV1 {
    pub fn fact(&self, value: Value) -> SparseIndexFactV1 {
        self.facts
            .get(&value)
            .cloned()
            .unwrap_or(SparseIndexFactV1::Unknown)
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
    EntryArgument,
    Merge(Vec<Value>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SparseIndexLatticeV1 {
    Pending,
    Known(SparseIndexFactV1),
}

#[derive(Clone, Debug)]
struct SparseEdgeV1 {
    source: usize,
    target: usize,
    arguments: Vec<Value>,
}

pub fn analyze_pliron_sparse_indices_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<SparseIndexAnalysisV1, SparseIndexFailureV1> {
    let entry = function.get_entry_block(context);
    let mut blocks = Vec::new();
    let mut block_indices = HashMap::new();
    let mut block_arguments = HashMap::new();
    for block in function.get_region(context).deref(context).iter(context) {
        if blocks.len() == MAX_SPARSE_INDEX_VALUES_V1 {
            return Err(limit(
                "CFG block",
                MAX_SPARSE_INDEX_VALUES_V1,
                blocks.len() + 1,
            ));
        }
        let index = blocks.len();
        blocks.push(block);
        block_indices.insert(block, index);
        block_arguments.insert(block, block.deref(context).arguments().collect::<Vec<_>>());
    }

    let mut definitions = Vec::new();
    let mut definition_indices = HashMap::new();
    let mut input_count = 0_usize;
    let mut launch_extents = Vec::new();
    for block in blocks.iter().copied() {
        for argument in block_arguments
            .get(&block)
            .expect("collected block has arguments")
        {
            push_definition(
                &mut definitions,
                &mut definition_indices,
                SparseDefinitionV1 {
                    kind: if block == entry {
                        SparseDefinitionKindV1::EntryArgument
                    } else {
                        SparseDefinitionKindV1::Merge(Vec::new())
                    },
                    result: *argument,
                },
            )?;
        }
        for operation in block.deref(context).iter(context) {
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
            }
        }
    }

    let mut edges = Vec::new();
    let mut successors = vec![Vec::new(); blocks.len()];
    for (source, block) in blocks.iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        let raw = terminator.deref(context);
        charge_uses(&mut input_count, raw.get_num_successors())?;
        let dynamic = Operation::get_op_dyn(terminator, context);
        let edge_arguments = typed_edge_arguments(context, &dynamic, &block_arguments)?;
        for (successor_index, successor) in raw.successors().enumerate() {
            let Some(&target) = block_indices.get(&successor) else {
                return Err(malformed("a branch targets a block outside the kernel"));
            };
            let arguments = block_arguments
                .get(&successor)
                .expect("kernel successor has collected arguments");
            let incoming = match &edge_arguments {
                Some(edges) => edges
                    .get(successor_index)
                    .cloned()
                    .ok_or_else(|| malformed("typed edge does not describe every successor"))?,
                None if arguments.is_empty() => Vec::new(),
                None => {
                    return Err(malformed(
                        "a block argument has a predecessor without typed edge operands",
                    ));
                }
            };
            if incoming.len() != arguments.len() {
                return Err(malformed(
                    "typed edge operand and block argument counts differ",
                ));
            }
            for (incoming, argument) in incoming.iter().zip(arguments) {
                if incoming.get_type(context) != argument.get_type(context) {
                    return Err(malformed(
                        "typed edge operand and block argument types differ",
                    ));
                }
            }
            if successor == entry && !incoming.is_empty() {
                return Err(malformed(
                    "an entry argument cannot receive a CFG edge operand",
                ));
            }
            let edge_index = edges.len();
            edges.push(SparseEdgeV1 {
                source,
                target,
                arguments: incoming,
            });
            successors[source].push(edge_index);
        }
    }

    let mut preparation_work = 0_usize;
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
        let target = blocks[edge.target];
        let arguments = block_arguments
            .get(&target)
            .expect("reachable target has collected arguments");
        for (argument, incoming) in arguments.iter().zip(&edge.arguments) {
            let index = definition_indices
                .get(argument)
                .copied()
                .ok_or_else(|| malformed("block argument has no sparse definition"))?;
            let SparseDefinitionKindV1::Merge(inputs) = &mut definitions[index].kind else {
                return Err(malformed("an entry argument receives a reachable CFG edge"));
            };
            inputs.push(*incoming);
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
            SparseDefinitionKindV1::EntryArgument => Vec::new(),
        };
        charge_uses(&mut consumer_count, dependencies.len())?;
        for dependency in dependencies {
            consumers.entry(dependency).or_default().push(index);
        }
    }

    let mut lattice = vec![SparseIndexLatticeV1::Pending; definitions.len()];
    let mut pending = (0..definitions.len()).collect::<VecDeque<_>>();
    let mut queued = vec![true; definitions.len()];
    let mut propagation_work = preparation_work;
    while let Some(index) = pending.pop_front() {
        queued[index] = false;
        charge_work(&mut propagation_work, 1)?;
        let next = derive_definition(context, &definitions[index], &lattice, &definition_indices);
        if lattice[index] == next {
            continue;
        }
        lattice[index] = next;
        if let Some(users) = consumers.get(&definitions[index].result) {
            for user in users {
                if !queued[*user] {
                    queued[*user] = true;
                    pending.push_back(*user);
                }
            }
        }
    }

    let facts = definitions
        .iter()
        .zip(lattice)
        .map(|(definition, lattice)| {
            let fact = match lattice {
                SparseIndexLatticeV1::Pending => SparseIndexFactV1::Unknown,
                SparseIndexLatticeV1::Known(fact) => fact,
            };
            (definition.result, fact)
        })
        .collect();
    let declared_launch_extents = launch_extents.clone();
    let launch_extents = if launch_extents.is_empty() {
        vec![1]
    } else {
        launch_extents
            .into_iter()
            .map(|extent| extent.unwrap_or(0))
            .collect()
    };
    Ok(SparseIndexAnalysisV1 {
        facts,
        launch_extents,
        declared_launch_extents,
    })
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

fn typed_edge_arguments(
    context: &Context,
    operation: &OpBox,
    block_arguments: &HashMap<Ptr<BasicBlock>, Vec<Value>>,
) -> Result<Option<Vec<Vec<Value>>>, SparseIndexFailureV1> {
    let raw = operation.get_operation().deref(context);
    if operation.downcast_ref::<BranchArgsOp>().is_some() {
        if raw.get_num_successors() != 1 {
            return Err(malformed("kernel.br_args has a malformed successor count"));
        }
        let target = raw.get_successor(0);
        let Some(arguments) = block_arguments.get(&target) else {
            return Err(malformed("a branch targets a block outside the kernel"));
        };
        if raw.get_num_operands() != arguments.len() {
            return Err(malformed("kernel.br_args has a malformed operand count"));
        }
        return Ok(Some(vec![raw.operands().collect()]));
    }
    if operation
        .downcast_ref::<IndexLessThanBranchArgsOp>()
        .is_some()
        || operation.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
    {
        if raw.get_num_successors() != 2 {
            return Err(malformed(
                "typed conditional edge has a malformed successor count",
            ));
        }
        let Some(true_arguments) = block_arguments.get(&raw.get_successor(0)) else {
            return Err(malformed("a branch targets a block outside the kernel"));
        };
        let Some(false_arguments) = block_arguments.get(&raw.get_successor(1)) else {
            return Err(malformed("a branch targets a block outside the kernel"));
        };
        let expected = 2_usize
            .checked_add(true_arguments.len())
            .and_then(|count| count.checked_add(false_arguments.len()))
            .ok_or_else(|| malformed("typed conditional edge operand count overflows"))?;
        if raw.get_num_operands() != expected {
            return Err(malformed(
                "typed conditional edge has a malformed operand count",
            ));
        }
        let true_values = (0..true_arguments.len())
            .map(|index| raw.get_operand(2 + index))
            .collect();
        let false_values = (0..false_arguments.len())
            .map(|index| raw.get_operand(2 + true_arguments.len() + index))
            .collect();
        return Ok(Some(vec![true_values, false_values]));
    }
    if let Some(split) = operation.downcast_ref::<AnalysisSplitOp>() {
        if raw.get_num_successors() != 2 {
            return Err(malformed(
                "kernel.analysis_split has a malformed successor count",
            ));
        }
        let Some(first_arguments) = block_arguments.get(&raw.get_successor(0)) else {
            return Err(malformed("a branch targets a block outside the kernel"));
        };
        let Some(second_arguments) = block_arguments.get(&raw.get_successor(1)) else {
            return Err(malformed("a branch targets a block outside the kernel"));
        };
        let control_count = split.control_dependencies(context).len();
        let expected = control_count
            .checked_add(first_arguments.len())
            .and_then(|count| count.checked_add(second_arguments.len()))
            .ok_or_else(|| malformed("analysis split operand count overflows"))?;
        if raw.get_num_operands() != expected {
            return Err(malformed(
                "kernel.analysis_split has a malformed operand count",
            ));
        }
        let first_values = (0..first_arguments.len())
            .map(|index| raw.get_operand(control_count + index))
            .collect();
        let second_values = (0..second_arguments.len())
            .map(|index| raw.get_operand(control_count + first_arguments.len() + index))
            .collect();
        return Ok(Some(vec![first_values, second_values]));
    }
    Ok(None)
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

fn derive_definition(
    context: &Context,
    definition: &SparseDefinitionV1,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
) -> SparseIndexLatticeV1 {
    match &definition.kind {
        SparseDefinitionKindV1::EntryArgument => {
            SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown)
        }
        SparseDefinitionKindV1::Merge(inputs) => merge_facts(inputs, lattice, definition_indices),
        SparseDefinitionKindV1::Operation(operation) => {
            derive_operation(context, *operation, lattice, definition_indices)
        }
    }
}

fn merge_facts(
    inputs: &[Value],
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
) -> SparseIndexLatticeV1 {
    let mut merged = None;
    for input in inputs {
        let SparseIndexLatticeV1::Known(fact) = lookup(*input, lattice, definition_indices) else {
            continue;
        };
        if fact == SparseIndexFactV1::Unknown {
            return SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown);
        }
        match &merged {
            None => merged = Some(fact),
            Some(previous) if *previous == fact => {}
            Some(_) => return SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown),
        }
    }
    merged
        .map(SparseIndexLatticeV1::Known)
        .unwrap_or(SparseIndexLatticeV1::Pending)
}

fn lookup(
    value: Value,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
) -> SparseIndexLatticeV1 {
    definition_indices
        .get(&value)
        .map(|index| lattice[*index].clone())
        .unwrap_or(SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown))
}

fn derive_operation(
    context: &Context,
    operation: Ptr<Operation>,
    lattice: &[SparseIndexLatticeV1],
    definition_indices: &HashMap<Value, usize>,
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
        return known(derive_binary(binary.kind(context), lhs, rhs));
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

fn charge_work(work: &mut usize, additional: usize) -> Result<(), SparseIndexFailureV1> {
    *work = work.saturating_add(additional);
    if *work > MAX_SPARSE_INDEX_WORK_UNITS_V1 {
        return Err(limit(
            "sparse propagation work",
            MAX_SPARSE_INDEX_WORK_UNITS_V1,
            *work,
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
) -> SparseIndexFactV1 {
    let (SparseIndexFactV1::Affine(lhs), SparseIndexFactV1::Affine(rhs)) = (lhs, rhs) else {
        return SparseIndexFactV1::Unknown;
    };
    match kind {
        Some(IndexBinaryKindAttr::Add) => lhs
            .checked_add(&rhs)
            .map(SparseIndexFactV1::Affine)
            .unwrap_or(SparseIndexFactV1::Unknown),
        Some(IndexBinaryKindAttr::Multiply) => match (lhs.is_constant(), rhs.is_constant()) {
            (Some(factor), _) => rhs
                .checked_scale(factor)
                .map(SparseIndexFactV1::Affine)
                .unwrap_or(SparseIndexFactV1::Unknown),
            (_, Some(factor)) => lhs
                .checked_scale(factor)
                .map(SparseIndexFactV1::Affine)
                .unwrap_or(SparseIndexFactV1::Unknown),
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

const fn limit(resource: &'static str, limit: usize, actual: usize) -> SparseIndexFailureV1 {
    SparseIndexFailureV1::ResourceLimit {
        resource,
        limit,
        actual,
    }
}
