//! Sparse SSA propagation for target-neutral kernel index expressions.
//!
//! This is deliberately a value analysis, not a race detector. It derives
//! bounded unsigned formulas from SSA definitions and records the launch
//! domain named by `kernel.invocation_index`. Memory and synchronization
//! passes consume these facts without duplicating expression recognition.

use std::collections::{HashMap, VecDeque};

use dialect_kernel::{
    CheckedTiledIndex2DOp, DimensionOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    InvocationIndexOp, MAX_RANKED_MEMORY_RANK, RankedViewOp, ranked_view_type,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation,
    r#type::TypedHandle,
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
            Self::Unknown | Self::Remainder { .. } | Self::CheckedTiled2D(_) => None,
        }
    }

    pub fn constant_value(&self) -> Option<u64> {
        match self {
            Self::Affine(affine) => affine.is_constant(),
            Self::Unknown | Self::Remainder { .. } | Self::CheckedTiled2D(_) => None,
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
        }
    }

    pub fn maximum(&self, launch_extents: &[u64]) -> Option<u64> {
        match self {
            Self::Unknown => None,
            Self::Affine(affine) => affine.maximum(launch_extents),
            Self::Remainder { modulus, .. } => modulus.checked_sub(1),
            Self::CheckedTiled2D(_) => None,
        }
    }

    pub const fn checked_tiled_2d(&self) -> Option<&SparseCheckedTiledIndex2DV1> {
        match self {
            Self::CheckedTiled2D(fact) => Some(fact),
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

#[derive(Clone, Copy)]
struct SparseDefinitionV1 {
    operation: Ptr<Operation>,
    result: Value,
}

pub fn analyze_pliron_sparse_indices_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<SparseIndexAnalysisV1, SparseIndexFailureV1> {
    let mut definitions = Vec::new();
    let mut use_count = 0_usize;
    for block in function.get_region(context).deref(context).iter(context) {
        for operation in block.deref(context).iter(context) {
            let raw = operation.deref(context);
            use_count = use_count.saturating_add(raw.get_num_operands());
            if use_count > MAX_SPARSE_INDEX_USES_V1 {
                return Err(limit("SSA use", MAX_SPARSE_INDEX_USES_V1, use_count));
            }
            if raw.get_num_results() == 1 {
                if definitions.len() == MAX_SPARSE_INDEX_VALUES_V1 {
                    return Err(limit(
                        "SSA value",
                        MAX_SPARSE_INDEX_VALUES_V1,
                        definitions.len() + 1,
                    ));
                }
                definitions.push(SparseDefinitionV1 {
                    operation,
                    result: raw.get_result(0),
                });
            }
        }
    }

    let mut consumers: HashMap<Value, Vec<usize>> = HashMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        for operand in definition.operation.deref(context).operands() {
            consumers.entry(operand).or_default().push(index);
        }
    }

    let mut facts = HashMap::new();
    let mut launch_extents = Vec::new();
    let mut pending = (0..definitions.len()).collect::<VecDeque<_>>();
    let mut queued = vec![true; definitions.len()];
    let mut work = 0_usize;
    while let Some(index) = pending.pop_front() {
        queued[index] = false;
        work = work.saturating_add(1);
        if work > MAX_SPARSE_INDEX_WORK_UNITS_V1 {
            return Err(limit(
                "sparse propagation work",
                MAX_SPARSE_INDEX_WORK_UNITS_V1,
                work,
            ));
        }
        let definition = definitions[index];
        let fact = derive_fact(context, definition.operation, &facts, &mut launch_extents)?;
        if facts.get(&definition.result) == Some(&fact) {
            continue;
        }
        facts.insert(definition.result, fact);
        if let Some(users) = consumers.get(&definition.result) {
            for user in users {
                if !queued[*user] {
                    queued[*user] = true;
                    pending.push_back(*user);
                }
            }
        }
    }
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

fn derive_fact(
    context: &Context,
    operation: Ptr<Operation>,
    facts: &HashMap<Value, SparseIndexFactV1>,
    launch_extents: &mut Vec<Option<u64>>,
) -> Result<SparseIndexFactV1, SparseIndexFailureV1> {
    let operation = Operation::get_op_dyn(operation, context);
    if let Some(constant) = operation.downcast_ref::<IndexConstantOp>() {
        return Ok(constant
            .value(context)
            .map(SparseAffineIndexV1::constant)
            .map(SparseIndexFactV1::Affine)
            .unwrap_or(SparseIndexFactV1::Unknown));
    }
    if let Some(invocation) = operation.downcast_ref::<InvocationIndexOp>() {
        let Some(dimension) = invocation
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
        else {
            return Ok(SparseIndexFactV1::Unknown);
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
        return Ok(SparseIndexFactV1::Affine(SparseAffineIndexV1::invocation(
            dimension,
        )));
    }
    if let Some(dimension) = operation.downcast_ref::<DimensionOp>() {
        let Some(dimension_index) = dimension
            .dimension(context)
            .and_then(|dimension| usize::try_from(dimension).ok())
        else {
            return Ok(SparseIndexFactV1::Unknown);
        };
        let view = dimension.view(context);
        if let Some(view_type) = ranked_view_type(view, context) {
            let view_type: TypedHandle<dialect_kernel::RankedViewType> = view_type;
            let extent = view_type.deref(context).shape()[dimension_index];
            if extent != dialect_kernel::DYNAMIC_EXTENT {
                return Ok(SparseIndexFactV1::Affine(SparseAffineIndexV1::constant(
                    extent,
                )));
            }
            if let Some(definition) = view.defining_op() {
                let definition = Operation::get_op_dyn(definition, context);
                if let Some(view) = definition.downcast_ref::<RankedViewOp>()
                    && let Some(extent) = view.dynamic_extent(context, dimension_index)
                {
                    return Ok(facts
                        .get(&extent)
                        .cloned()
                        .unwrap_or(SparseIndexFactV1::Unknown));
                }
            }
        }
        return Ok(SparseIndexFactV1::Unknown);
    }
    if let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() {
        let lhs = facts
            .get(&binary.lhs(context))
            .cloned()
            .unwrap_or(SparseIndexFactV1::Unknown);
        let rhs = facts
            .get(&binary.rhs(context))
            .cloned()
            .unwrap_or(SparseIndexFactV1::Unknown);
        return Ok(derive_binary(binary.kind(context), lhs, rhs));
    }
    if let Some(tiled) = operation.downcast_ref::<CheckedTiledIndex2DOp>() {
        let [invocation, component, rows, columns, row_stride] = tiled.operands(context);
        let Some(invocation) = facts.get(&invocation).and_then(SparseIndexFactV1::affine) else {
            return Ok(SparseIndexFactV1::Unknown);
        };
        let Some(geometry) = tiled.geometry(context) else {
            return Ok(SparseIndexFactV1::Unknown);
        };
        return Ok(SparseIndexFactV1::CheckedTiled2D(
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
    Ok(SparseIndexFactV1::Unknown)
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
