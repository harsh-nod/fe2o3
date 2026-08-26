//! Compiler-owned full-domain discharge for safe-reference slice bounds.
//!
//! This stage joins exact reference-MIR bounds assertions to ranked view
//! extents. It is workload neutral and accepts only relations derived from the
//! retained MIR and ranked recipe.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use dialect_kernel::{DYNAMIC_EXTENT, IndexBinaryKindAttr};
use fe2o3_pliron::{
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedValueV1,
};

use crate::reference_effect_v1::{
    ReferenceArgumentRelationV1, ReferenceBinaryOpV1, ReferenceCastKindV1, ReferenceConstantV1,
    ReferenceEffectExpressionV1, ReferenceEffectIrV1, ReferenceOutputCoordinateV1,
    ReferenceOutputWriteV1, ReferenceScalarTypeV1, ResolvedReferenceBoundsCheckV1,
};

const MAX_BOUND_NODES_V2: usize = 8_192;
const MAX_BOUND_DEPTH_V2: usize = 64;

pub(crate) struct CompilerOwnedOutputDomainV2<'a> {
    pub(crate) reference: &'a ReferenceOutputWriteV1,
    pub(crate) ranked_view: ProductionRankedValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceBoundsDischargeErrorV2 {
    block: u32,
    detail: String,
}

impl ReferenceBoundsDischargeErrorV2 {
    fn new(block: u32, detail: impl Into<String>) -> Self {
        Self {
            block,
            detail: detail.into(),
        }
    }

    pub(crate) const fn block(&self) -> u32 {
        self.block
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExtentExprV2 {
    Constant(u64),
    Argument(u32),
    Binary(ExtentBinaryKindV2, Box<Self>, Box<Self>),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExtentBinaryKindV2 {
    Add,
    Multiply,
    Divide,
    Remainder,
}

impl ExtentExprV2 {
    fn constant_value(&self) -> Option<u64> {
        match self {
            Self::Constant(value) => Some(*value),
            Self::Argument(_) => None,
            Self::Binary(kind, lhs, rhs) => {
                let lhs = lhs.constant_value()?;
                let rhs = rhs.constant_value()?;
                match kind {
                    ExtentBinaryKindV2::Add => lhs.checked_add(rhs),
                    ExtentBinaryKindV2::Multiply => lhs.checked_mul(rhs),
                    ExtentBinaryKindV2::Divide => (rhs != 0).then_some(lhs / rhs),
                    ExtentBinaryKindV2::Remainder => (rhs != 0).then_some(lhs % rhs),
                }
            }
        }
    }
}

impl fmt::Display for ExtentExprV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(value) => write!(formatter, "{value}"),
            Self::Argument(argument) => write!(formatter, "%arg{argument}"),
            Self::Binary(kind, lhs, rhs) => write!(
                formatter,
                "({lhs} {} {rhs})",
                match kind {
                    ExtentBinaryKindV2::Add => "+",
                    ExtentBinaryKindV2::Multiply => "*",
                    ExtentBinaryKindV2::Divide => "/",
                    ExtentBinaryKindV2::Remainder => "%",
                }
            ),
        }
    }
}

#[derive(Clone)]
struct SliceAccessV2 {
    block: u32,
    reference_argument: u32,
    index: ReferenceEffectExpressionV1,
}

#[derive(Clone, Copy)]
struct IntervalV2 {
    minimum: u64,
    maximum: u64,
}

/// Discharges every retained slice check over every point in the exact ranked
/// output domain. No check is treated as an assumption: the relation must be
/// independently implied by the ranked extents.
pub(crate) fn discharge_reference_bounds_over_ranked_domains_v2(
    kernel: &ProductionRankedKernelV1,
    effect_ir: &ReferenceEffectIrV1,
    outputs: &[CompilerOwnedOutputDomainV2<'_>],
) -> Result<(), ReferenceBoundsDischargeErrorV2> {
    let checks = effect_ir.resolved_bounds_checks_v1().map_err(|_| {
        ReferenceBoundsDischargeErrorV2::new(
            0,
            "a retained safe-slice bounds assertion cannot be normalized",
        )
    })?;
    let mut accesses = Vec::new();
    let mut nodes = 0;
    for output in outputs {
        collect_accesses(
            output.reference.block,
            &output.reference.rhs,
            &mut accesses,
            &mut nodes,
            0,
        )?;
        if let ReferenceOutputCoordinateV1::Dynamic(index) = &output.reference.coordinate {
            collect_accesses(output.reference.block, index, &mut accesses, &mut nodes, 0)?;
            accesses.push(SliceAccessV2 {
                block: output.reference.block,
                reference_argument: effect_ir
                    .reference_argument_for_kernel_argument_v1(output.reference.argument)
                    .map_err(|_| {
                        ReferenceBoundsDischargeErrorV2::new(
                            output.reference.block,
                            "dynamic output extent cannot be mapped to its exact reference argument",
                        )
                    })?,
                index: index.clone(),
            });
        }
    }
    if accesses.is_empty() && checks.is_empty() {
        return Ok(());
    }

    let definitions = definitions(kernel)?;
    let domains = point_domains(kernel, outputs, &definitions)?;
    let mut normalized = Vec::with_capacity(checks.len());
    for check in checks {
        validate_check(&check)?;
        let ReferenceEffectExpressionV1::InputLength { reference_argument } = check.length else {
            return Err(ReferenceBoundsDischargeErrorV2::new(
                check.block,
                "bounds length is not the exact length of one logical slice argument",
            ));
        };
        normalized.push((check, reference_argument));
    }

    let mut used = vec![false; normalized.len()];
    for access in &accesses {
        let matches = normalized
            .iter()
            .enumerate()
            .filter(|(_, (check, argument))| {
                *argument == access.reference_argument && check.index == access.index
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(ReferenceBoundsDischargeErrorV2::new(
                access.block,
                format!(
                    "safe-slice access `{}` has no exact retained bounds assertion",
                    describe_expr(&access.index)
                ),
            ));
        }
        let extent = slice_extent(
            kernel,
            effect_ir,
            access.reference_argument,
            &definitions,
            access.block,
        )?;
        prove_bound(access, &extent, &domains)?;
        for index in matches {
            used[index] = true;
        }
    }
    if let Some((_, (check, _))) = used.iter().zip(normalized.iter()).find(|(used, _)| !**used) {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            check.block,
            "bounds assertion has no exact retained reference load or output access",
        ));
    }
    Ok(())
}

fn validate_check(
    check: &ResolvedReferenceBoundsCheckV1,
) -> Result<(), ReferenceBoundsDischargeErrorV2> {
    if !check.expected {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            check.block,
            "safe-slice bounds assertion does not require the in-bounds condition",
        ));
    }
    if !matches!(
        &check.condition,
        ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::LessThan,
            lhs,
            rhs,
            checked: false,
        } if **lhs == check.index && **rhs == check.length
    ) {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            check.block,
            "retained assertion condition is not the exact index-less-than-length comparison",
        ));
    }
    Ok(())
}

fn collect_accesses(
    block: u32,
    expression: &ReferenceEffectExpressionV1,
    accesses: &mut Vec<SliceAccessV2>,
    nodes: &mut usize,
    depth: usize,
) -> Result<(), ReferenceBoundsDischargeErrorV2> {
    *nodes = nodes.checked_add(1).ok_or_else(|| {
        ReferenceBoundsDischargeErrorV2::new(block, "reference bounds node count overflowed")
    })?;
    if *nodes > MAX_BOUND_NODES_V2 || depth >= MAX_BOUND_DEPTH_V2 {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            "reference bounds proof exceeds its fixed expression budget",
        ));
    }
    match expression {
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => {
            accesses.push(SliceAccessV2 {
                block,
                reference_argument: *reference_argument,
                index: (**index).clone(),
            });
            collect_accesses(block, index, accesses, nodes, depth + 1)?;
        }
        ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } => {
            collect_accesses(block, lhs, accesses, nodes, depth + 1)?;
            collect_accesses(block, rhs, accesses, nodes, depth + 1)?;
        }
        ReferenceEffectExpressionV1::Unary { operand, .. }
        | ReferenceEffectExpressionV1::Cast { operand, .. } => {
            collect_accesses(block, operand, accesses, nodes, depth + 1)?;
        }
        _ => {}
    }
    Ok(())
}

fn definitions(
    kernel: &ProductionRankedKernelV1,
) -> Result<BTreeMap<u32, &ProductionRankedOperationV1>, ReferenceBoundsDischargeErrorV2> {
    let mut result = BTreeMap::new();
    for operation in kernel.blocks().iter().flat_map(|block| block.operations()) {
        let identity = match operation {
            ProductionRankedOperationV1::View { result, .. }
            | ProductionRankedOperationV1::ViewInSpace { result, .. }
            | ProductionRankedOperationV1::IndexConstant { result, .. }
            | ProductionRankedOperationV1::IndexUnknown { result }
            | ProductionRankedOperationV1::InvocationIndex { result, .. }
            | ProductionRankedOperationV1::IndexBinary { result, .. }
            | ProductionRankedOperationV1::DeterministicJoin { result, .. }
            | ProductionRankedOperationV1::CheckedTiledIndex2D { result, .. }
            | ProductionRankedOperationV1::CheckedRowStripedIndex2D { result, .. }
            | ProductionRankedOperationV1::Dimension { result, .. }
            | ProductionRankedOperationV1::SemanticConstant { result, .. }
            | ProductionRankedOperationV1::SemanticSymbol { result, .. }
            | ProductionRankedOperationV1::SemanticExpression { result, .. }
            | ProductionRankedOperationV1::TensorResultComponent { result, .. } => Some(*result),
            _ => None,
        };
        if let Some(identity) = identity
            && result.insert(identity.get(), operation).is_some()
        {
            return Err(ReferenceBoundsDischargeErrorV2::new(
                0,
                format!("ranked value %{} has multiple definitions", identity.get()),
            ));
        }
    }
    Ok(result)
}

fn point_domains(
    kernel: &ProductionRankedKernelV1,
    outputs: &[CompilerOwnedOutputDomainV2<'_>],
    definitions: &BTreeMap<u32, &ProductionRankedOperationV1>,
) -> Result<BTreeMap<u32, ExtentExprV2>, ReferenceBoundsDischargeErrorV2> {
    let mut domains = BTreeMap::new();
    for output in outputs {
        let shape = view_shape(
            kernel,
            output.ranked_view,
            definitions,
            output.reference.block,
        )?;
        if let ReferenceOutputCoordinateV1::Dynamic(
            ReferenceEffectExpressionV1::PointCoordinate { axis },
        ) = &output.reference.coordinate
        {
            let [extent] = shape.as_slice() else {
                return Err(ReferenceBoundsDischargeErrorV2::new(
                    output.reference.block,
                    format!(
                        "dynamic output coordinate requires a rank-1 view, found rank {}",
                        shape.len()
                    ),
                ));
            };
            insert_domain(&mut domains, *axis, extent.clone(), output.reference.block)?;
            continue;
        }
        let ReferenceOutputCoordinateV1::LogicalPoint(coordinates) = &output.reference.coordinate
        else {
            continue;
        };
        if shape.len() != coordinates.len() {
            return Err(ReferenceBoundsDischargeErrorV2::new(
                output.reference.block,
                format!(
                    "logical output rank {} disagrees with ranked view rank {}",
                    coordinates.len(),
                    shape.len()
                ),
            ));
        }
        for (dimension, (coordinate, extent)) in coordinates.iter().zip(shape).enumerate() {
            let ReferenceEffectExpressionV1::PointCoordinate { axis } = coordinate else {
                return Err(ReferenceBoundsDischargeErrorV2::new(
                    output.reference.block,
                    format!(
                        "output dimension {dimension} is not one exact point-coordinate domain"
                    ),
                ));
            };
            insert_domain(&mut domains, *axis, extent, output.reference.block)?;
        }
    }
    Ok(domains)
}

fn insert_domain(
    domains: &mut BTreeMap<u32, ExtentExprV2>,
    axis: u32,
    extent: ExtentExprV2,
    block: u32,
) -> Result<(), ReferenceBoundsDischargeErrorV2> {
    if let Some(previous) = domains.insert(axis, extent.clone())
        && previous != extent
    {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            format!("point axis {axis} has conflicting ranked extents {previous} and {extent}"),
        ));
    }
    Ok(())
}

fn slice_extent(
    kernel: &ProductionRankedKernelV1,
    effect_ir: &ReferenceEffectIrV1,
    reference_argument: u32,
    definitions: &BTreeMap<u32, &ProductionRankedOperationV1>,
    block: u32,
) -> Result<ExtentExprV2, ReferenceBoundsDischargeErrorV2> {
    let arguments = effect_ir
        .relations
        .iter()
        .filter_map(|relation| match relation {
            ReferenceArgumentRelationV1::SharedSliceInput { argument, .. }
            | ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. }
                if effect_ir
                    .reference_argument_for_kernel_argument_v1(*argument)
                    .is_ok_and(|actual| actual == reference_argument) =>
            {
                Some(*argument)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [argument] = arguments.as_slice() else {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            format!(
                "slice length for reference argument {reference_argument} does not have exactly one logical ABI relation"
            ),
        ));
    };
    let allocation_origin = u64::from(*argument).checked_add(1).ok_or_else(|| {
        ReferenceBoundsDischargeErrorV2::new(block, "slice allocation origin overflowed")
    })?;
    let mut extents = kernel
        .blocks()
        .iter()
        .flat_map(|ranked_block| ranked_block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::View {
                result,
                allocation_origin: actual,
                ..
            }
            | ProductionRankedOperationV1::ViewInSpace {
                result,
                allocation_origin: actual,
                ..
            } if *actual == allocation_origin => Some(ProductionRankedValueV1::Local(*result)),
            _ => None,
        })
        .map(|view| view_shape(kernel, view, definitions, block))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|shape| match shape.as_slice() {
            [extent] => Ok(extent.clone()),
            _ => Err(ReferenceBoundsDischargeErrorV2::new(
                block,
                format!(
                    "slice allocation origin {allocation_origin} has ranked view rank {} instead of 1",
                    shape.len()
                ),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    extents.sort();
    extents.dedup();
    match extents.as_slice() {
        [extent] => Ok(extent.clone()),
        [] => Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            format!(
                "slice allocation origin {allocation_origin} has no ranked extent retained by the compiler"
            ),
        )),
        _ => Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            format!(
                "slice allocation origin {allocation_origin} has conflicting ranked extents: {}",
                extents
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

fn view_shape(
    _kernel: &ProductionRankedKernelV1,
    view: ProductionRankedValueV1,
    definitions: &BTreeMap<u32, &ProductionRankedOperationV1>,
    block: u32,
) -> Result<Vec<ExtentExprV2>, ReferenceBoundsDischargeErrorV2> {
    let ProductionRankedValueV1::Local(view) = view else {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            "ranked view has no compiler-owned local definition",
        ));
    };
    let (shape, dynamic_extents) = match definitions.get(&view.get()) {
        Some(ProductionRankedOperationV1::View {
            shape,
            dynamic_extents,
            ..
        })
        | Some(ProductionRankedOperationV1::ViewInSpace {
            shape,
            dynamic_extents,
            ..
        }) => (shape, dynamic_extents),
        _ => {
            return Err(ReferenceBoundsDischargeErrorV2::new(
                block,
                format!(
                    "ranked value %{} is not an exact view definition",
                    view.get()
                ),
            ));
        }
    };
    let mut dynamic = dynamic_extents.iter().copied();
    let result = shape
        .iter()
        .map(|extent| {
            if *extent == DYNAMIC_EXTENT {
                extent_expr(
                    dynamic.next().ok_or_else(|| {
                        ReferenceBoundsDischargeErrorV2::new(
                            block,
                            format!("ranked view %{} is missing a dynamic extent", view.get()),
                        )
                    })?,
                    definitions,
                    &mut BTreeSet::new(),
                    0,
                    block,
                )
            } else {
                Ok(ExtentExprV2::Constant(*extent))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if dynamic.next().is_some() {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            format!("ranked view %{} retains excess dynamic extents", view.get()),
        ));
    }
    Ok(result)
}

fn extent_expr(
    value: ProductionRankedValueV1,
    definitions: &BTreeMap<u32, &ProductionRankedOperationV1>,
    visiting: &mut BTreeSet<u32>,
    depth: usize,
    block: u32,
) -> Result<ExtentExprV2, ReferenceBoundsDischargeErrorV2> {
    if depth >= MAX_BOUND_DEPTH_V2 {
        return Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            "ranked extent expression exceeds its fixed normalization depth",
        ));
    }
    match value {
        ProductionRankedValueV1::Argument(argument) => Ok(ExtentExprV2::Argument(argument)),
        ProductionRankedValueV1::BlockArgument { .. } => Err(ReferenceBoundsDischargeErrorV2::new(
            block,
            "ranked extent depends on a control-flow block argument",
        )),
        ProductionRankedValueV1::Local(identity) => {
            if !visiting.insert(identity.get()) {
                return Err(ReferenceBoundsDischargeErrorV2::new(
                    block,
                    format!(
                        "ranked extent expression contains a cycle at %{}",
                        identity.get()
                    ),
                ));
            }
            let normalized = match definitions.get(&identity.get()) {
                Some(ProductionRankedOperationV1::IndexConstant { value, .. }) => {
                    ExtentExprV2::Constant(*value)
                }
                Some(ProductionRankedOperationV1::IndexBinary { kind, lhs, rhs, .. }) => {
                    ExtentExprV2::Binary(
                        match kind {
                            IndexBinaryKindAttr::Add => ExtentBinaryKindV2::Add,
                            IndexBinaryKindAttr::Multiply => ExtentBinaryKindV2::Multiply,
                            IndexBinaryKindAttr::Divide => ExtentBinaryKindV2::Divide,
                            IndexBinaryKindAttr::Remainder => ExtentBinaryKindV2::Remainder,
                        },
                        Box::new(extent_expr(*lhs, definitions, visiting, depth + 1, block)?),
                        Box::new(extent_expr(*rhs, definitions, visiting, depth + 1, block)?),
                    )
                }
                _ => {
                    return Err(ReferenceBoundsDischargeErrorV2::new(
                        block,
                        format!(
                            "ranked extent %{} is not an exact constant, argument, or index expression",
                            identity.get()
                        ),
                    ));
                }
            };
            visiting.remove(&identity.get());
            Ok(normalized)
        }
    }
}

fn prove_bound(
    access: &SliceAccessV2,
    input_extent: &ExtentExprV2,
    domains: &BTreeMap<u32, ExtentExprV2>,
) -> Result<(), ReferenceBoundsDischargeErrorV2> {
    // A point coordinate is definitionally below its domain extent. Equality
    // with the independently retained input-view extent is therefore enough;
    // the bounds assertion itself does not participate in this proof.
    if let ReferenceEffectExpressionV1::PointCoordinate { axis } = access.index
        && domains.get(&axis) == Some(input_extent)
    {
        return Ok(());
    }

    let static_domains = domains
        .iter()
        .map(|(axis, extent)| Some((*axis, extent.constant_value()?)))
        .collect::<Option<BTreeMap<_, _>>>();
    if let Some(static_domains) = static_domains {
        // There is no reference invocation in an empty output domain.
        if static_domains.values().any(|extent| *extent == 0) {
            return Ok(());
        }
        match interval(&access.index, &static_domains, 0) {
            Ok(index) => {
                if input_extent
                    .constant_value()
                    .is_some_and(|extent| index.maximum < extent)
                {
                    return Ok(());
                }
                return Err(ReferenceBoundsDischargeErrorV2::new(
                    access.block,
                    format!(
                        "cannot prove full-domain bound `{}` < `{input_extent}`: derived index range {}..={} over {}; the retained extent is unsafe or mismatched",
                        describe_expr(&access.index),
                        index.minimum,
                        index.maximum,
                        describe_domains(domains),
                    ),
                ));
            }
            Err(reason) => {
                return Err(ReferenceBoundsDischargeErrorV2::new(
                    access.block,
                    format!(
                        "cannot prove full-domain bound `{}` < `{input_extent}` over {}: {reason}",
                        describe_expr(&access.index),
                        describe_domains(domains),
                    ),
                ));
            }
        }
    }
    Err(ReferenceBoundsDischargeErrorV2::new(
        access.block,
        format!(
            "cannot prove full-domain bound `{}` < `{input_extent}` over {}: no exact ranked extent relation connects this bound to every point-coordinate extent",
            describe_expr(&access.index),
            describe_domains(domains),
        ),
    ))
}

fn interval(
    expression: &ReferenceEffectExpressionV1,
    domains: &BTreeMap<u32, u64>,
    depth: usize,
) -> Result<IntervalV2, &'static str> {
    if depth >= MAX_BOUND_DEPTH_V2 {
        return Err("index expression exceeds the bounded interval depth");
    }
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => {
            let extent = domains
                .get(axis)
                .copied()
                .ok_or("point coordinate has no exact ranked output extent")?;
            Ok(IntervalV2 {
                minimum: 0,
                maximum: extent
                    .checked_sub(1)
                    .ok_or("point coordinate domain is empty")?,
            })
        }
        ReferenceEffectExpressionV1::Constant(constant) => {
            let value = unsigned_constant(constant)
                .ok_or("index constant is not an exact unsigned machine value")?;
            Ok(IntervalV2 {
                minimum: value,
                maximum: value,
            })
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            ..
        } => {
            let lhs = interval(lhs, domains, depth + 1)?;
            let rhs = interval(rhs, domains, depth + 1)?;
            match operation {
                ReferenceBinaryOpV1::Add => Ok(IntervalV2 {
                    minimum: lhs
                        .minimum
                        .checked_add(rhs.minimum)
                        .ok_or("index addition can overflow")?,
                    maximum: lhs
                        .maximum
                        .checked_add(rhs.maximum)
                        .ok_or("index addition can overflow")?,
                }),
                ReferenceBinaryOpV1::Subtract if lhs.minimum >= rhs.maximum => Ok(IntervalV2 {
                    minimum: lhs.minimum - rhs.maximum,
                    maximum: lhs.maximum - rhs.minimum,
                }),
                ReferenceBinaryOpV1::Subtract => Err("index subtraction can underflow"),
                ReferenceBinaryOpV1::Multiply => Ok(IntervalV2 {
                    minimum: lhs
                        .minimum
                        .checked_mul(rhs.minimum)
                        .ok_or("index multiplication can overflow")?,
                    maximum: lhs
                        .maximum
                        .checked_mul(rhs.maximum)
                        .ok_or("index multiplication can overflow")?,
                }),
                ReferenceBinaryOpV1::Divide if rhs.minimum > 0 => Ok(IntervalV2 {
                    minimum: lhs.minimum / rhs.maximum,
                    maximum: lhs.maximum / rhs.minimum,
                }),
                ReferenceBinaryOpV1::Remainder if rhs.minimum == rhs.maximum && rhs.minimum > 0 => {
                    Ok(IntervalV2 {
                        minimum: 0,
                        maximum: lhs.maximum.min(rhs.minimum - 1),
                    })
                }
                ReferenceBinaryOpV1::Divide | ReferenceBinaryOpV1::Remainder => {
                    Err("index divisor may be zero or varies over the proof domain")
                }
                ReferenceBinaryOpV1::BitXor
                | ReferenceBinaryOpV1::BitAnd
                | ReferenceBinaryOpV1::BitOr
                | ReferenceBinaryOpV1::ShiftLeft
                | ReferenceBinaryOpV1::ShiftRight
                | ReferenceBinaryOpV1::Equal
                | ReferenceBinaryOpV1::LessThan
                | ReferenceBinaryOpV1::LessEqual
                | ReferenceBinaryOpV1::NotEqual
                | ReferenceBinaryOpV1::GreaterEqual
                | ReferenceBinaryOpV1::GreaterThan => {
                    Err("index operator has no admitted unsigned interval rule")
                }
            }
        }
        ReferenceEffectExpressionV1::Cast {
            kind: ReferenceCastKindV1::Integer,
            source,
            target,
            operand,
        } => {
            let interval = interval(operand, domains, depth + 1)?;
            let source_bits =
                unsigned_bits(*source).ok_or("index cast source is not an unsigned integer")?;
            let target_bits =
                unsigned_bits(*target).ok_or("index cast target is not an unsigned integer")?;
            let target_maximum = if target_bits == 64 {
                u64::MAX
            } else {
                (1_u64 << target_bits) - 1
            };
            if target_bits < source_bits && interval.maximum > target_maximum {
                return Err("narrowing index cast can truncate");
            }
            Ok(interval)
        }
        ReferenceEffectExpressionV1::KernelScalarArgument { .. } => {
            Err("unrestricted scalar argument has no compiler-owned range relation")
        }
        ReferenceEffectExpressionV1::InputLoad { .. }
        | ReferenceEffectExpressionV1::InputLength { .. } => {
            Err("index depends on a memory value rather than the output domain")
        }
        ReferenceEffectExpressionV1::Unary { .. } | ReferenceEffectExpressionV1::Cast { .. } => {
            Err("index unary or cast operation has no admitted unsigned interval rule")
        }
    }
}

fn unsigned_bits(scalar: ReferenceScalarTypeV1) -> Option<u32> {
    Some(match scalar {
        ReferenceScalarTypeV1::U8 => 8,
        ReferenceScalarTypeV1::U16 => 16,
        ReferenceScalarTypeV1::U32 => 32,
        ReferenceScalarTypeV1::U64 | ReferenceScalarTypeV1::Usize => 64,
        _ => return None,
    })
}

fn unsigned_constant(constant: &ReferenceConstantV1) -> Option<u64> {
    let ReferenceConstantV1::Scalar { scalar, bits } = constant else {
        return None;
    };
    unsigned_bits(*scalar)?;
    u64::try_from(*bits).ok()
}

fn describe_expr(expression: &ReferenceEffectExpressionV1) -> String {
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => format!("point[{axis}]"),
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            format!("scalar[{argument}]")
        }
        ReferenceEffectExpressionV1::Constant(constant) => unsigned_constant(constant).map_or_else(
            || "<non-index constant>".to_owned(),
            |value| value.to_string(),
        ),
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => format!("input[{reference_argument}][{}]", describe_expr(index)),
        ReferenceEffectExpressionV1::InputLength { reference_argument } => {
            format!("len(input[{reference_argument}])")
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            ..
        } => format!(
            "({} {} {})",
            describe_expr(lhs),
            match operation {
                ReferenceBinaryOpV1::Add => "+",
                ReferenceBinaryOpV1::Subtract => "-",
                ReferenceBinaryOpV1::Multiply => "*",
                ReferenceBinaryOpV1::Divide => "/",
                ReferenceBinaryOpV1::Remainder => "%",
                ReferenceBinaryOpV1::BitXor => "^",
                ReferenceBinaryOpV1::BitAnd => "&",
                ReferenceBinaryOpV1::BitOr => "|",
                ReferenceBinaryOpV1::ShiftLeft => "<<",
                ReferenceBinaryOpV1::ShiftRight => ">>",
                ReferenceBinaryOpV1::Equal => "==",
                ReferenceBinaryOpV1::LessThan => "<",
                ReferenceBinaryOpV1::LessEqual => "<=",
                ReferenceBinaryOpV1::NotEqual => "!=",
                ReferenceBinaryOpV1::GreaterEqual => ">=",
                ReferenceBinaryOpV1::GreaterThan => ">",
            },
            describe_expr(rhs),
        ),
        ReferenceEffectExpressionV1::Unary { operand, .. } => {
            format!("unary({})", describe_expr(operand))
        }
        ReferenceEffectExpressionV1::Cast { operand, .. } => {
            format!("cast({})", describe_expr(operand))
        }
    }
}

fn describe_domains(domains: &BTreeMap<u32, ExtentExprV2>) -> String {
    if domains.is_empty() {
        return "scalar output domain".to_owned();
    }
    domains
        .iter()
        .map(|(axis, extent)| format!("0 <= point[{axis}] < {extent}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_effect_v1::{
        ReferenceAssignmentV1, ReferenceBlockV1, ReferenceBoundsCheckV1, ReferenceOperandV1,
        ReferencePathPredicateV1, ReferencePlaceV1, ReferenceTerminatorV1, ReferenceValueV1,
    };
    use fe2o3_pliron::{
        ProductionRankedBlockV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    };

    #[derive(Clone, Copy)]
    enum TestExtent {
        Static(u64),
        Argument(u32),
    }

    fn constant(value: u64) -> ReferenceEffectExpressionV1 {
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize,
            bits: u128::from(value),
        })
    }

    fn binary(
        operation: ReferenceBinaryOpV1,
        lhs: ReferenceEffectExpressionV1,
        rhs: ReferenceEffectExpressionV1,
    ) -> ReferenceEffectExpressionV1 {
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            checked: false,
        }
    }

    fn operand_constant(value: u64) -> ReferenceOperandV1 {
        ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize,
            bits: u128::from(value),
        })
    }

    fn expression_operand(
        expression: &ReferenceEffectExpressionV1,
        assignments: &mut Vec<ReferenceAssignmentV1>,
        next_local: &mut u32,
    ) -> ReferenceOperandV1 {
        match expression {
            ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::Usize,
                bits,
            }) => operand_constant(u64::try_from(*bits).unwrap()),
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 } => {
                ReferenceOperandV1::Copy(ReferencePlaceV1 {
                    local: 1,
                    projection: Box::default(),
                })
            }
            ReferenceEffectExpressionV1::Binary {
                operation,
                lhs,
                rhs,
                checked,
            } => {
                let lhs = expression_operand(lhs, assignments, next_local);
                let rhs = expression_operand(rhs, assignments, next_local);
                let local = *next_local;
                *next_local += 1;
                assignments.push(ReferenceAssignmentV1 {
                    statement: assignments.len() as u32,
                    destination: ReferencePlaceV1 {
                        local,
                        projection: Box::default(),
                    },
                    value: ReferenceValueV1::Binary {
                        operation: *operation,
                        lhs,
                        rhs,
                        checked: *checked,
                    },
                });
                ReferenceOperandV1::Copy(ReferencePlaceV1 {
                    local,
                    projection: Box::default(),
                })
            }
            _ => panic!("test index must be a point/constant arithmetic expression"),
        }
    }

    fn fixture(
        index: ReferenceEffectExpressionV1,
        input_extent: TestExtent,
        output_extent: TestExtent,
        with_check: bool,
    ) -> (
        ProductionRankedKernelV1,
        ReferenceEffectIrV1,
        ReferenceOutputWriteV1,
    ) {
        let input_view = ProductionRankedValueIdV1::new(0);
        let output_view = ProductionRankedValueIdV1::new(1);
        let shape_and_extent = |extent| match extent {
            TestExtent::Static(extent) => (vec![extent], vec![]),
            TestExtent::Argument(argument) => (
                vec![DYNAMIC_EXTENT],
                vec![ProductionRankedValueV1::Argument(argument)],
            ),
        };
        let (input_shape, input_dynamic) = shape_and_extent(input_extent);
        let (output_shape, output_dynamic) = shape_and_extent(output_extent);
        let argument_count = [input_extent, output_extent]
            .into_iter()
            .filter_map(|extent| match extent {
                TestExtent::Argument(argument) => Some(argument as usize + 1),
                TestExtent::Static(_) => None,
            })
            .max()
            .unwrap_or(0);
        let kernel = ProductionRankedKernelV1::new(
            "workload_neutral_reference_bounds",
            argument_count,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::View {
                        result: input_view,
                        element_width: 32,
                        writable: false,
                        shape: input_shape,
                        dynamic_extents: input_dynamic,
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::View {
                        result: output_view,
                        element_width: 32,
                        writable: true,
                        shape: output_shape,
                        dynamic_extents: output_dynamic,
                        allocation_origin: 2,
                        noalias_class: 2,
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();

        let mut assignments = Vec::new();
        let mut next_local = 4;
        let index_operand = expression_operand(&index, &mut assignments, &mut next_local);
        let length_local = next_local;
        next_local += 1;
        assignments.push(ReferenceAssignmentV1 {
            statement: assignments.len() as u32,
            destination: ReferencePlaceV1 {
                local: length_local,
                projection: Box::default(),
            },
            value: ReferenceValueV1::InputLength {
                reference_argument: 1,
            },
        });
        let condition_local = next_local;
        next_local += 1;
        assignments.push(ReferenceAssignmentV1 {
            statement: assignments.len() as u32,
            destination: ReferencePlaceV1 {
                local: condition_local,
                projection: Box::default(),
            },
            value: ReferenceValueV1::Binary {
                operation: ReferenceBinaryOpV1::LessThan,
                lhs: index_operand.clone(),
                rhs: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                    local: length_local,
                    projection: Box::default(),
                }),
                checked: false,
            },
        });
        let blocks = vec![
            ReferenceBlockV1 {
                block: 0,
                assignments: assignments.into_boxed_slice(),
                terminator: if with_check {
                    ReferenceTerminatorV1::Assert {
                        condition: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                            local: condition_local,
                            projection: Box::default(),
                        }),
                        expected: true,
                        success: 1,
                        bounds_check: Some(ReferenceBoundsCheckV1 {
                            index: index_operand,
                            length: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                                local: length_local,
                                projection: Box::default(),
                            }),
                        }),
                    }
                } else {
                    ReferenceTerminatorV1::Goto { target: 1 }
                },
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
        ];
        let output = ReferenceOutputWriteV1 {
            argument: 1,
            block: 1,
            statement: 0,
            coordinate: ReferenceOutputCoordinateV1::LogicalPoint(
                vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice(),
            ),
            guard: ReferencePathPredicateV1::unconditional_v1(),
            rhs: ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 1,
                index: Box::new(index),
            },
            value: ReferenceValueV1::Use(operand_constant(0)),
        };
        let effect_ir = ReferenceEffectIrV1 {
            argument_count: 3,
            local_count: next_local,
            relations: vec![
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument: 0,
                    axis: 0,
                },
                ReferenceArgumentRelationV1::SharedSliceInput {
                    argument: 0,
                    element: ReferenceScalarTypeV1::U32,
                },
                ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument: 1,
                    element: ReferenceScalarTypeV1::U32,
                },
            ]
            .into_boxed_slice(),
            blocks: blocks.into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: vec![output.clone()].into_boxed_slice(),
        };
        (kernel, effect_ir, output)
    }

    fn discharge(
        kernel: &ProductionRankedKernelV1,
        effect_ir: &ReferenceEffectIrV1,
        output: &ReferenceOutputWriteV1,
    ) -> Result<(), ReferenceBoundsDischargeErrorV2> {
        discharge_reference_bounds_over_ranked_domains_v2(
            kernel,
            effect_ir,
            &[CompilerOwnedOutputDomainV2 {
                reference: output,
                ranked_view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
            }],
        )
    }

    #[test]
    fn exact_symbolic_extent_relation_proves_the_complete_dynamic_domain() {
        let (kernel, effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(3),
            true,
        );
        discharge(&kernel, &effect_ir, &output).unwrap();
    }

    #[test]
    fn empty_output_domain_is_vacuously_safe_without_fabricating_an_index() {
        let access = SliceAccessV2 {
            block: 0,
            reference_argument: 1,
            index: ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
        };
        prove_bound(
            &access,
            &ExtentExprV2::Constant(0),
            &BTreeMap::from([(0, ExtentExprV2::Constant(0))]),
        )
        .unwrap();
    }

    #[test]
    fn static_affine_index_is_proved_from_its_full_domain_maximum() {
        let index = binary(
            ReferenceBinaryOpV1::Add,
            binary(
                ReferenceBinaryOpV1::Multiply,
                ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
                constant(2),
            ),
            constant(1),
        );
        let (kernel, effect_ir, output) =
            fixture(index, TestExtent::Static(16), TestExtent::Static(8), true);
        discharge(&kernel, &effect_ir, &output).unwrap();
    }

    #[test]
    fn exact_boundary_is_accepted_and_first_unsafe_index_is_reported() {
        let (kernel, effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Static(8),
            TestExtent::Static(8),
            true,
        );
        discharge(&kernel, &effect_ir, &output).unwrap();

        let (kernel, effect_ir, output) = fixture(
            binary(
                ReferenceBinaryOpV1::Add,
                ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
                constant(1),
            ),
            TestExtent::Static(8),
            TestExtent::Static(8),
            true,
        );
        let error = discharge(&kernel, &effect_ir, &output).unwrap_err();
        assert!(
            error.detail().contains("derived index range 1..=8"),
            "{error:?}"
        );
        assert!(error.detail().contains("point[0] < 8"), "{error:?}");
    }

    #[test]
    fn arithmetic_overflow_fails_closed_instead_of_wrapping_the_proof() {
        let (kernel, effect_ir, output) = fixture(
            binary(
                ReferenceBinaryOpV1::Add,
                ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
                constant(u64::MAX),
            ),
            TestExtent::Static(u64::MAX),
            TestExtent::Static(2),
            true,
        );
        let error = discharge(&kernel, &effect_ir, &output).unwrap_err();
        assert!(
            error.detail().contains("addition can overflow"),
            "{error:?}"
        );
    }

    #[test]
    fn unrelated_dynamic_extents_are_not_treated_as_equal() {
        let (kernel, effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(4),
            true,
        );
        let error = discharge(&kernel, &effect_ir, &output).unwrap_err();
        assert!(error.detail().contains("`point[0]` < `%arg3`"), "{error:?}");
        assert!(error.detail().contains("point[0] < %arg4"), "{error:?}");
        assert!(
            error.detail().contains("no exact ranked extent relation"),
            "{error:?}"
        );
    }

    #[test]
    fn missing_rust_bounds_assertion_remains_a_terminal_failure() {
        let (kernel, effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(3),
            false,
        );
        let error = discharge(&kernel, &effect_ir, &output).unwrap_err();
        assert!(
            error
                .detail()
                .contains("no exact retained bounds assertion")
        );
    }

    #[test]
    fn duplicate_checks_across_a_branch_and_canonical_loop_are_all_discharged() {
        let (kernel, mut effect_ir, mut output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(3),
            true,
        );
        let ReferenceTerminatorV1::Assert {
            condition,
            expected,
            bounds_check,
            ..
        } = effect_ir.blocks[0].terminator.clone()
        else {
            unreachable!("fixture retains one bounds assertion")
        };
        let duplicate = ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success: 3,
            bounds_check,
        };
        effect_ir.blocks[1].terminator = ReferenceTerminatorV1::Goto { target: 2 };
        effect_ir.blocks = vec![
            effect_ir.blocks[0].clone(),
            effect_ir.blocks[1].clone(),
            ReferenceBlockV1 {
                block: 2,
                assignments: Box::default(),
                terminator: duplicate,
            },
            ReferenceBlockV1 {
                block: 3,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Goto { target: 2 },
            },
        ]
        .into_boxed_slice();
        output.block = 2;
        discharge(&kernel, &effect_ir, &output).unwrap();
    }

    #[test]
    fn an_unused_bounds_check_is_not_silently_erased() {
        let (kernel, mut effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Static(8),
            TestExtent::Static(8),
            true,
        );
        let extra_condition = effect_ir.local_count;
        effect_ir.local_count += 1;
        effect_ir.blocks = vec![
            effect_ir.blocks[0].clone(),
            effect_ir.blocks[1].clone(),
            ReferenceBlockV1 {
                block: 2,
                assignments: vec![ReferenceAssignmentV1 {
                    statement: 0,
                    destination: ReferencePlaceV1 {
                        local: extra_condition,
                        projection: Box::default(),
                    },
                    value: ReferenceValueV1::Binary {
                        operation: ReferenceBinaryOpV1::LessThan,
                        lhs: operand_constant(1),
                        rhs: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                            local: 4,
                            projection: Box::default(),
                        }),
                        checked: false,
                    },
                }]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Assert {
                    condition: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                        local: extra_condition,
                        projection: Box::default(),
                    }),
                    expected: true,
                    success: 1,
                    bounds_check: Some(ReferenceBoundsCheckV1 {
                        index: operand_constant(1),
                        length: ReferenceOperandV1::Copy(ReferencePlaceV1 {
                            local: 4,
                            projection: Box::default(),
                        }),
                    }),
                },
            },
        ]
        .into_boxed_slice();
        let error = discharge(&kernel, &effect_ir, &output).unwrap_err();
        assert!(
            error
                .detail()
                .contains("bounds assertion has no exact retained reference load")
        );
    }

    #[test]
    fn safe_dynamic_output_slice_check_is_joined_to_its_ranked_view() {
        let (kernel, mut effect_ir, mut output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(3),
            true,
        );
        effect_ir.relations[2] = ReferenceArgumentRelationV1::DisjointOutputSlice {
            argument: 1,
            element: ReferenceScalarTypeV1::U32,
        };
        for assignment in &mut effect_ir.blocks[0].assignments {
            if matches!(assignment.value, ReferenceValueV1::InputLength { .. }) {
                assignment.value = ReferenceValueV1::InputLength {
                    reference_argument: 2,
                };
            }
        }
        output.coordinate =
            ReferenceOutputCoordinateV1::Dynamic(ReferenceEffectExpressionV1::PointCoordinate {
                axis: 0,
            });
        output.rhs = constant(17);
        discharge(&kernel, &effect_ir, &output).unwrap();
    }

    #[test]
    fn multi_output_domains_must_agree_before_any_check_is_erased() {
        let (kernel, effect_ir, output) = fixture(
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
            TestExtent::Argument(3),
            TestExtent::Argument(3),
            true,
        );
        let conflicting_view = ProductionRankedValueIdV1::new(2);
        let mut blocks = kernel.blocks().to_vec();
        let mut operations = blocks[0].operations().to_vec();
        operations.push(ProductionRankedOperationV1::View {
            result: conflicting_view,
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(4)],
            allocation_origin: 3,
            noalias_class: 3,
        });
        blocks[0] = ProductionRankedBlockV1::new(operations, ProductionRankedTerminatorV1::Return);
        let kernel = ProductionRankedKernelV1::new("conflicting_domains", 5, blocks).unwrap();
        let error = discharge_reference_bounds_over_ranked_domains_v2(
            &kernel,
            &effect_ir,
            &[
                CompilerOwnedOutputDomainV2 {
                    reference: &output,
                    ranked_view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                },
                CompilerOwnedOutputDomainV2 {
                    reference: &output,
                    ranked_view: ProductionRankedValueV1::Local(conflicting_view),
                },
            ],
        )
        .unwrap_err();
        assert!(error.detail().contains("conflicting ranked extents"));
    }
}
