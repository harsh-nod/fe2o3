//! Logical retained storage of inert reference IR and output records.
//! No constructor/admission/authentication behavior is changed.
use super::*;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};
use std::fmt;

/// An observation-only depth bound, not an extraction or producer limit.
/// Expressions of greater depth are explicitly refused, never partially accepted.
pub const MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceRetainedStorageErrorV1 {
    Counter(LogicalStorageErrorV1),
    ExpressionDepthLimit,
}
impl From<LogicalStorageErrorV1> for ReferenceRetainedStorageErrorV1 {
    fn from(value: LogicalStorageErrorV1) -> Self {
        Self::Counter(value)
    }
}
impl fmt::Display for ReferenceRetainedStorageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reference retained-storage observation refused: {self:?}"
        )
    }
}
impl std::error::Error for ReferenceRetainedStorageErrorV1 {}

type Result<T> = std::result::Result<T, ReferenceRetainedStorageErrorV1>;
fn fixed<T: Copy>(_: &T) {}
fn copy_row<T: Copy>() {}
fn tick(c: &mut LogicalStorageCounterV1) -> Result<()> {
    c.charge(0, 1)?;
    Ok(())
}
fn rows<T>(values: &Box<[T]>, c: &mut LogicalStorageCounterV1) -> Result<()> {
    c.array::<T>(values.len())?;
    Ok(())
}
fn constant(value: &ReferenceConstantV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    match value {
        ReferenceConstantV1::ZeroSized => {}
        ReferenceConstantV1::Scalar { scalar, bits } => {
            fixed(scalar);
            fixed(bits);
        }
    }
    Ok(())
}
fn place(value: &ReferencePlaceV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    let ReferencePlaceV1 { local, projection } = value;
    fixed(local);
    rows(projection, c)?;
    // Projection is not Copy: exhaustively inspect every row after a debit.
    for p in projection {
        tick(c)?;
        match p {
            ReferencePlaceProjectionV1::Dereference => {}
            ReferencePlaceProjectionV1::Field(value) | ReferencePlaceProjectionV1::Index(value) => {
                fixed(value)
            }
            ReferencePlaceProjectionV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                fixed(offset);
                fixed(minimum_length);
                fixed(from_end);
            }
        }
    }
    Ok(())
}
fn operand(value: &ReferenceOperandV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    match value {
        ReferenceOperandV1::Copy(value) | ReferenceOperandV1::Move(value) => place(value, c),
        ReferenceOperandV1::Constant(value) => constant(value, c),
    }
}
fn push_expression<'a>(
    child: &'a Box<ReferenceEffectExpressionV1>,
    depth: usize,
    pending: &mut [Option<(&'a ReferenceEffectExpressionV1, usize)>;
             MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1],
    count: &mut usize,
    c: &mut LogicalStorageCounterV1,
) -> Result<()> {
    if depth >= MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1 || *count >= pending.len() {
        return Err(ReferenceRetainedStorageErrorV1::ExpressionDepthLimit);
    }
    // The Box handle is in its parent expression; charge its pointee once.
    c.array::<ReferenceEffectExpressionV1>(1)?;
    pending[*count] = Some((child.as_ref(), depth + 1));
    *count += 1;
    Ok(())
}
fn expression(root: &ReferenceEffectExpressionV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    // Refuse before initializing even the fixed scratch array.
    tick(c)?;
    let mut pending = [None; MAX_REFERENCE_RETAINED_EXPRESSION_DEPTH_V1];
    let mut count = 0;
    let mut node = root;
    let mut depth = 1;
    loop {
        match node {
            ReferenceEffectExpressionV1::PointCoordinate { axis } => fixed(axis),
            ReferenceEffectExpressionV1::KernelScalarArgument { argument } => fixed(argument),
            ReferenceEffectExpressionV1::Constant(value) => constant(value, c)?,
            ReferenceEffectExpressionV1::InputLength { reference_argument } => {
                fixed(reference_argument)
            }
            ReferenceEffectExpressionV1::InputLoad {
                reference_argument,
                index,
            } => {
                fixed(reference_argument);
                push_expression(index, depth, &mut pending, &mut count, c)?;
            }
            ReferenceEffectExpressionV1::Binary {
                operation,
                lhs,
                rhs,
                checked,
            } => {
                fixed(operation);
                fixed(checked);
                // DFS retains at most one pending sibling per depth.
                push_expression(rhs, depth, &mut pending, &mut count, c)?;
                push_expression(lhs, depth, &mut pending, &mut count, c)?;
            }
            ReferenceEffectExpressionV1::Unary { operation, operand } => {
                fixed(operation);
                push_expression(operand, depth, &mut pending, &mut count, c)?;
            }
            ReferenceEffectExpressionV1::Cast {
                kind,
                source,
                target,
                operand,
            } => {
                fixed(kind);
                fixed(source);
                fixed(target);
                push_expression(operand, depth, &mut pending, &mut count, c)?;
            }
        }
        if count == 0 {
            return Ok(());
        }
        count -= 1;
        let next = pending[count]
            .take()
            .ok_or(ReferenceRetainedStorageErrorV1::ExpressionDepthLimit)?;
        tick(c)?;
        (node, depth) = next;
    }
}
fn value(value: &ReferenceValueV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    match value {
        ReferenceValueV1::Use(value) => operand(value, c)?,
        ReferenceValueV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            fixed(operation);
            fixed(checked);
            operand(lhs, c)?;
            operand(rhs, c)?;
        }
        ReferenceValueV1::Unary {
            operation,
            operand: input,
        } => {
            fixed(operation);
            operand(input, c)?;
        }
        ReferenceValueV1::Cast {
            kind,
            source,
            target,
            operand: input,
        } => {
            fixed(kind);
            fixed(source);
            fixed(target);
            operand(input, c)?;
        }
        ReferenceValueV1::InputLength { reference_argument } => fixed(reference_argument),
        ReferenceValueV1::SafeHelperCall {
            helper,
            parameters,
            result,
            arguments,
            summary,
        } => {
            fixed(helper);
            fixed(result);
            copy_row::<ReferenceScalarTypeV1>();
            rows(parameters, c)?;
            rows(arguments, c)?;
            for input in arguments {
                operand(input, c)?;
            }
            let _: &Box<ReferenceEffectExpressionV1> = summary;
            c.array::<ReferenceEffectExpressionV1>(1)?;
            expression(summary, c)?;
        }
    }
    Ok(())
}
fn terminator(value: &ReferenceTerminatorV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    match value {
        ReferenceTerminatorV1::Return => {}
        ReferenceTerminatorV1::Goto { target } => fixed(target),
        ReferenceTerminatorV1::Switch {
            discriminant,
            values,
            otherwise,
        } => {
            fixed(otherwise);
            copy_row::<(u128, u32)>();
            operand(discriminant, c)?;
            rows(values, c)?;
        }
        ReferenceTerminatorV1::Assert {
            condition,
            expected,
            success,
            bounds_check,
        } => {
            fixed(expected);
            fixed(success);
            operand(condition, c)?;
            if let Some(bounds) = bounds_check {
                tick(c)?;
                let ReferenceBoundsCheckV1 { index, length } = bounds;
                operand(index, c)?;
                operand(length, c)?;
            }
        }
    }
    Ok(())
}
fn coordinate(value: &ReferenceOutputCoordinateV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    match value {
        ReferenceOutputCoordinateV1::LogicalPoint(values) => {
            rows(values, c)?;
            for point in values {
                expression(point, c)?;
            }
        }
        ReferenceOutputCoordinateV1::SingleCoordinate => {}
        ReferenceOutputCoordinateV1::Dynamic(value) => expression(value, c)?,
        ReferenceOutputCoordinateV1::Constant {
            offset,
            minimum_length,
            from_end,
        } => {
            fixed(offset);
            fixed(minimum_length);
            fixed(from_end);
        }
    }
    Ok(())
}
fn predicate(value: &ReferencePathPredicateV1, c: &mut LogicalStorageCounterV1) -> Result<()> {
    tick(c)?;
    let ReferencePathPredicateV1 { clauses } = value;
    rows(clauses, c)?;
    for clause in clauses {
        tick(c)?;
        let ReferenceGuardClauseV1 { atoms } = clause;
        rows(atoms, c)?;
        for atom in atoms {
            tick(c)?;
            match atom {
                ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant,
                    values,
                    inside_set,
                } => {
                    fixed(inside_set);
                    copy_row::<u128>();
                    expression(discriminant, c)?;
                    rows(values, c)?;
                }
                ReferenceGuardAtomV1::Assert {
                    condition,
                    expected,
                } => {
                    fixed(expected);
                    expression(condition, c)?;
                }
            }
        }
    }
    Ok(())
}

impl ReferenceEffectExpressionV1 {
    /// Charge this inline expression's complete owned heap, not its header.
    /// One shared-counter node visit precedes every inspected node. Box pointee
    /// headers are charged once; a fixed stack replaces recursive traversal.
    /// Depth >128 explicitly refuses; no partial subtree success is returned.
    pub fn charge_retained_heap_storage_v1(&self, c: &mut LogicalStorageCounterV1) -> Result<()> {
        expression(self, c)
    }
}
impl ReferenceOutputWriteV1 {
    /// Complete owned heap of one inline output, including its original value,
    /// coordinate, guard, RHS and all separately owned expression copies.
    pub fn charge_retained_heap_storage_v1(&self, c: &mut LogicalStorageCounterV1) -> Result<()> {
        tick(c)?;
        let Self {
            argument,
            block,
            statement,
            coordinate: location,
            guard,
            rhs,
            value: original,
        } = self;
        fixed(argument);
        fixed(block);
        fixed(statement);
        coordinate(location, c)?;
        predicate(guard, c)?;
        expression(rhs, c)?;
        value(original, c)?;
        Ok(())
    }
}
impl ReferenceEffectIrV1 {
    /// Complete current IR heap, excluding its inline header and separate
    /// binding-level output clone. All original arrays/trees are counted.
    ///
    /// Every branch/node/allocation debits the caller's SAME counter. Fixed
    /// Copy payload arrays need no row scan. Dynamic rows are visited only after
    /// item payment. No heap scratch, clone, hash, validation or authentication
    /// occurs. The only traversal scratch is a fixed128-slot expression stack.
    /// First Counter/depth error stops; prefix charges remain and the enclosing
    /// observation MUST be discarded. Not allocator/peak/RSS telemetry.
    pub fn charge_retained_heap_storage_v1(&self, c: &mut LogicalStorageCounterV1) -> Result<()> {
        tick(c)?;
        let Self {
            argument_count,
            local_count,
            relations,
            blocks,
            loop_summaries,
            observable_output_effects,
        } = self;
        fixed(argument_count);
        fixed(local_count);
        copy_row::<ReferenceArgumentRelationV1>();
        rows(relations, c)?;
        rows(blocks, c)?;
        for body in blocks {
            tick(c)?;
            let ReferenceBlockV1 {
                block,
                assignments,
                terminator: end,
            } = body;
            fixed(block);
            rows(assignments, c)?;
            for assignment in assignments {
                tick(c)?;
                let ReferenceAssignmentV1 {
                    statement,
                    destination,
                    value: input,
                } = assignment;
                fixed(statement);
                place(destination, c)?;
                value(input, c)?;
            }
            terminator(end, c)?;
        }
        rows(loop_summaries, c)?;
        for summary in loop_summaries {
            tick(c)?;
            let ReferenceLoopSummaryV2 {
                header,
                latch,
                exit,
                exact_iterations,
                maximum_iterations,
                carried_locals,
                initial_state_sha256,
                transition_sha256,
                variant_sha256,
            } = summary;
            fixed(header);
            fixed(latch);
            fixed(exit);
            fixed(exact_iterations);
            fixed(maximum_iterations);
            fixed(initial_state_sha256);
            fixed(transition_sha256);
            fixed(variant_sha256);
            copy_row::<u32>();
            rows(carried_locals, c)?;
        }
        rows(observable_output_effects, c)?;
        for output in observable_output_effects {
            output.charge_retained_heap_storage_v1(c)?;
        }
        Ok(())
    }
}
