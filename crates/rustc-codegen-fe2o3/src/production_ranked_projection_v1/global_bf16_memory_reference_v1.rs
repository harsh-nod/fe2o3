//! Independent CPU read-event requests, not memory/refinement evidence.
//!
//! The caller must bind every input to the actual retained source occurrence.
//! These formulas have no Load leaves and cannot substitute for Pauli's common
//! live read producer, memory version, or volatile-event correspondence proof.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMfmaOperandContractV1, SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1,
    SemanticMfmaRegisterDistributionV1,
};
use fe2o3_pliron::{
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2, ProductionOverflowContractV2,
    ProductionSemanticBinaryOpV2 as Binary, ProductionSemanticComparisonV2 as Compare,
    ProductionSemanticExpressionV2 as Expr, ProductionSemanticScalarTypeV2 as Scalar,
};

#[cfg(test)]
#[path = "global_bf16_memory_reference_v1/tests.rs"]
mod tests;

const INDEX: Scalar = Scalar::Integer {
    signed: false,
    bits: 64,
};
const BITS: Scalar = Scalar::Integer {
    signed: false,
    bits: 16,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CpuReadRequestErrorV1 {
    UnsupportedFragment,
    InvalidSourceExpression,
    UnboundMemoryInput,
    UnsupportedDynamicStride,
    ResourceLimit,
}

/// Existing typed expressions resolved independently from source SSA. Neither
/// this record nor its constructor authenticates the inputs' source identity.
pub(super) struct CpuReadInputsV1 {
    pub lane: Expr,
    pub offset: Expr,
    pub rows: Expr,
    pub columns: Expr,
    pub stride: Expr,
    pub first_base: Expr,
    pub second_base: Expr,
    pub allocation_length: Expr,
}

pub(super) struct CpuReadEventV1 {
    /// Original loop iteration, not a ranked operation ID or free load symbol.
    pub component: u8,
    pub index: Expr,
    pub guard: Expr,
    pub fallback: Expr,
}

pub(super) struct CpuReadScheduleV1 {
    /// Must be discharged from the same current Wave64 witness as the source.
    pub lane_precondition: Expr,
    /// Four distinct volatile events in source order, even for equal indices.
    pub events: [CpuReadEventV1; 4],
}

pub(super) fn source_read_schedule_v1(
    contract: SemanticMfmaOperandContractV1,
    inputs: CpuReadInputsV1,
) -> Result<CpuReadScheduleV1, CpuReadRequestErrorV1> {
    use CpuReadRequestErrorV1 as E;
    if contract.profile != SemanticMfmaProfileV1::Bf16F32M16N16K16
        || contract.register_distribution != SemanticMfmaRegisterDistributionV1::Tile16x16
        || contract.wave_width != 64
    {
        return Err(E::UnsupportedFragment);
    }
    let mut input_nodes = 0_usize;
    for expression in [
        &inputs.lane,
        &inputs.offset,
        &inputs.rows,
        &inputs.columns,
        &inputs.stride,
        &inputs.first_base,
        &inputs.second_base,
        &inputs.allocation_length,
    ] {
        if expression.scalar() != INDEX || expression.contains_float_semantics() {
            return Err(E::InvalidSourceExpression);
        }
        let stats = expression
            .validate()
            .map_err(|_| E::InvalidSourceExpression)?;
        expression
            .validate_static_domains()
            .map_err(|_| E::InvalidSourceExpression)?;
        require_source_scalar(expression)?;
        input_nodes = input_nodes
            .checked_add(stats.nodes)
            .ok_or(E::ResourceLimit)?;
    }
    // The fixed four-event expansion duplicates source subtrees. Bound them
    // before constructing any expanded trees, then check the actual total.
    if input_nodes > MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 / 128 {
        return Err(E::ResourceLimit);
    }
    let constant_stride = match &inputs.stride {
        Expr::Constant { bits, .. } => Some(*bits),
        _ => None,
    };
    // On an active read row < rows. Exact unsigned source casts can therefore
    // discharge multiplication overflow without an ambient range assertion.
    let product_bounded = source_unsigned_max(&inputs.rows)
        .saturating_sub(1)
        .checked_mul(source_unsigned_max(&inputs.stride))
        .is_some();
    if constant_stride.is_none() && !product_bounded {
        return Err(E::UnsupportedDynamicStride);
    }
    let minor = binary(Binary::BitAnd, inputs.lane.clone(), index(15));
    let group = binary(
        Binary::Multiply,
        binary(Binary::ShiftRight, inputs.lane.clone(), index(4)),
        index(4),
    );
    let (minor_base, reduction_base) = match contract.role {
        SemanticMfmaOperandRoleV1::A => (&inputs.first_base, &inputs.second_base),
        SemanticMfmaOperandRoleV1::B => (&inputs.second_base, &inputs.first_base),
    };
    let (minor, minor_valid) = checked_add(minor_base.clone(), minor);
    let (first_reduction, reduction_valid) = checked_add(reduction_base.clone(), group);
    let events = std::array::from_fn(|component| {
        let (reduction, component_valid) =
            checked_add(first_reduction.clone(), index(component as u64));
        let (row, column) = match contract.role {
            SemanticMfmaOperandRoleV1::A => (minor.clone(), reduction),
            SemanticMfmaOperandRoleV1::B => (reduction, minor.clone()),
        };
        let row_offset = binary(Binary::Multiply, row.clone(), inputs.stride.clone());
        let row_offset_valid = match constant_stride {
            Some(0) | None => boolean(true),
            Some(stride) => compare(Compare::LessOrEqual, row.clone(), index(u64::MAX / stride)),
        };
        let (row_start, offset_valid) = checked_add(inputs.offset.clone(), row_offset);
        let (physical, column_valid) = checked_add(row_start, column.clone());
        let guard = [
            minor_valid.clone(),
            reduction_valid.clone(),
            component_valid,
            compare(Compare::LessThan, row, inputs.rows.clone()),
            compare(Compare::LessThan, column, inputs.columns.clone()),
            row_offset_valid,
            offset_valid,
            column_valid,
            compare(
                Compare::LessThan,
                physical.clone(),
                inputs.allocation_length.clone(),
            ),
        ]
        .into_iter()
        .reduce(and)
        .expect("fixed nonempty guard");
        CpuReadEventV1 {
            component: component as u8,
            index: physical,
            guard,
            fallback: Expr::Constant {
                scalar: BITS,
                bits: 0,
            },
        }
    });
    let schedule = CpuReadScheduleV1 {
        lane_precondition: compare(Compare::LessThan, inputs.lane, index(64)),
        events,
    };
    let mut nodes = 0_usize;
    for expression in std::iter::once(&schedule.lane_precondition).chain(
        schedule
            .events
            .iter()
            .flat_map(|event| [&event.index, &event.guard, &event.fallback]),
    ) {
        let stats = expression.validate().map_err(|_| E::ResourceLimit)?;
        expression
            .validate_static_domains()
            .map_err(|_| E::InvalidSourceExpression)?;
        nodes = nodes.checked_add(stats.nodes).ok_or(E::ResourceLimit)?;
        if nodes > MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 {
            return Err(E::ResourceLimit);
        }
    }
    Ok(schedule)
}

fn require_source_scalar(expression: &Expr) -> Result<(), CpuReadRequestErrorV1> {
    match expression {
        Expr::Load(_) => Err(CpuReadRequestErrorV1::UnboundMemoryInput),
        Expr::Symbol { symbol, .. }
            if *symbol >= fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2 =>
        {
            Err(CpuReadRequestErrorV1::UnboundMemoryInput)
        }
        Expr::Symbol { .. } | Expr::Constant { .. } => Ok(()),
        Expr::Unary { operand, .. } | Expr::Cast { operand, .. } => require_source_scalar(operand),
        Expr::Binary { lhs, rhs, .. } | Expr::Compare { lhs, rhs, .. } => {
            require_source_scalar(lhs)?;
            require_source_scalar(rhs)
        }
        Expr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            require_source_scalar(condition)?;
            require_source_scalar(when_true)?;
            require_source_scalar(when_false)
        }
    }
}

fn source_unsigned_max(expression: &Expr) -> u64 {
    match expression {
        Expr::Constant { bits, .. } => *bits,
        Expr::Cast {
            kind: fe2o3_pliron::ProductionSemanticCastV2::Integer,
            source:
                Scalar::Integer {
                    signed: false,
                    bits,
                },
            target: INDEX,
            ..
        } if *bits < 64 => (1_u64 << bits) - 1,
        _ => u64::MAX,
    }
}

fn index(bits: u64) -> Expr {
    Expr::Constant {
        scalar: INDEX,
        bits,
    }
}
fn boolean(value: bool) -> Expr {
    Expr::Constant {
        scalar: Scalar::Bool,
        bits: u64::from(value),
    }
}
fn binary(operation: Binary, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        operation,
        scalar: INDEX,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}
fn compare(operation: Compare, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Compare {
        operation,
        operand_scalar: INDEX,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}
fn and(lhs: Expr, rhs: Expr) -> Expr {
    Expr::Select {
        scalar: Scalar::Bool,
        condition: Box::new(lhs),
        when_true: Box::new(rhs),
        when_false: Box::new(boolean(false)),
    }
}
fn checked_add(lhs: Expr, rhs: Expr) -> (Expr, Expr) {
    let valid = compare(
        Compare::LessOrEqual,
        rhs.clone(),
        binary(Binary::Subtract, index(u64::MAX), lhs.clone()),
    );
    (binary(Binary::Add, lhs, rhs), valid)
}
