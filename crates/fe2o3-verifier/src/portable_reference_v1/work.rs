//! Shared extraction debit rules; this trait owns no resource ledger.

use super::*;

pub trait ReferenceWorkV1 {
    fn charge(&self, amount: usize) -> Result<(), ReferenceBindingErrorV1>;

    fn product(&self, a: usize, b: usize) -> Result<(), ReferenceBindingErrorV1> {
        self.charge(a.checked_mul(b).ok_or_else(overflow)?)
    }

    fn rows<T>(&self, count: usize) -> Result<(), ReferenceBindingErrorV1> {
        self.product(
            count,
            std::mem::size_of::<T>()
                .checked_add(1)
                .ok_or_else(overflow)?,
        )
    }

    /// Conservative tree search/update bound, including node movement/allocation.
    fn tree<T>(&self, count: usize) -> Result<(), ReferenceBindingErrorV1> {
        self.rows::<T>(count.checked_add(1).ok_or_else(overflow)?)
    }

    fn grow<T>(&self, len: usize) -> Result<(), ReferenceBindingErrorV1> {
        self.rows::<T>(
            len.checked_add(1)
                .and_then(|n| n.checked_mul(8))
                .ok_or_else(overflow)?,
        )
    }

    /// Prepay comparison, movement, and deduplication using a quadratic envelope.
    /// `units` includes all recursively compared payloads, not just row count.
    fn sort(&self, count: usize, units: usize) -> Result<(), ReferenceBindingErrorV1> {
        let passes = count
            .checked_add(1)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(overflow)?;
        self.product(passes, units.checked_add(count).ok_or_else(overflow)?)
    }

    fn expression(
        &self,
        expression: &ReferenceEffectExpressionV1,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        let mut nodes = 0;
        self.expression_inner(expression, 0, &mut nodes)
    }

    fn expression_inner(
        &self,
        expression: &ReferenceEffectExpressionV1,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(ReferenceBindingErrorV1::new(
                "reference extraction expression exceeds depth limit",
            ));
        }
        *nodes = nodes.checked_add(1).ok_or_else(overflow)?;
        if *nodes > MAX_REFERENCE_EXPRESSION_NODES_V1 {
            return Err(ReferenceBindingErrorV1::new(
                "reference extraction expression exceeds node limit",
            ));
        }
        let mut units = std::mem::size_of::<ReferenceEffectExpressionV1>() + 1;
        match expression {
            ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } => {
                units = add(units, self.expression_inner(lhs, depth + 1, nodes)?)?;
                units = add(units, self.expression_inner(rhs, depth + 1, nodes)?)?;
            }
            ReferenceEffectExpressionV1::Unary { operand, .. }
            | ReferenceEffectExpressionV1::Cast { operand, .. }
            | ReferenceEffectExpressionV1::InputLoad { index: operand, .. } => {
                units = add(units, self.expression_inner(operand, depth + 1, nodes)?)?;
            }
            _ => {}
        }
        Ok(units)
    }

    fn clone_expression(
        &self,
        expression: &ReferenceEffectExpressionV1,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge(self.expression(expression)?)
    }

    fn place(&self, place: &ReferencePlaceV1) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let bytes = place
            .projection
            .len()
            .checked_mul(std::mem::size_of::<ReferencePlaceProjectionV1>() + 1)
            .ok_or_else(overflow)?;
        add(std::mem::size_of::<ReferencePlaceV1>() + 1, bytes)
    }

    fn operand(&self, operand: &ReferenceOperandV1) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let row = std::mem::size_of::<ReferenceOperandV1>() + 1;
        match operand {
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place) => {
                add(row, self.place(place)?)
            }
            ReferenceOperandV1::Constant(_) => Ok(row),
        }
    }

    fn value(&self, value: &ReferenceValueV1) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = std::mem::size_of::<ReferenceValueV1>() + 1;
        match value {
            ReferenceValueV1::Use(operand)
            | ReferenceValueV1::Unary { operand, .. }
            | ReferenceValueV1::Cast { operand, .. } => units = add(units, self.operand(operand)?)?,
            ReferenceValueV1::Binary { lhs, rhs, .. } => {
                units = add(units, self.operand(lhs)?)?;
                units = add(units, self.operand(rhs)?)?;
            }
            ReferenceValueV1::InputLength { .. } => {}
            ReferenceValueV1::SafeHelperCall {
                parameters,
                arguments,
                summary,
                ..
            } => {
                units = add(
                    units,
                    parameters
                        .len()
                        .checked_mul(std::mem::size_of::<ReferenceScalarTypeV1>() + 1)
                        .ok_or_else(overflow)?,
                )?;
                for argument in arguments {
                    units = add(units, self.operand(argument)?)?;
                }
                units = add(units, self.expression(summary)?)?;
            }
        }
        Ok(units)
    }

    fn atom(&self, atom: &ReferenceGuardAtomV1) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = std::mem::size_of::<ReferenceGuardAtomV1>() + 1;
        match atom {
            ReferenceGuardAtomV1::SwitchValueSet {
                discriminant,
                values,
                ..
            } => {
                units = add(units, self.expression(discriminant)?)?;
                units = add(
                    units,
                    values
                        .len()
                        .checked_mul(std::mem::size_of::<u128>() + 1)
                        .ok_or_else(overflow)?,
                )?;
            }
            ReferenceGuardAtomV1::Assert { condition, .. } => {
                units = add(units, self.expression(condition)?)?
            }
        }
        Ok(units)
    }

    fn clauses(
        &self,
        clauses: &[ReferenceGuardClauseV1],
    ) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = 0;
        for clause in clauses {
            self.charge(1)?;
            units = add(units, std::mem::size_of::<ReferenceGuardClauseV1>() + 1)?;
            for atom in &clause.atoms {
                units = add(units, self.atom(atom)?)?;
            }
        }
        Ok(units)
    }

    fn clone_predicate(
        &self,
        predicate: &ReferencePathPredicateV1,
    ) -> Result<(), ReferenceBindingErrorV1> {
        self.charge(self.clauses(&predicate.clauses)?)
    }

    fn effect(&self, effect: &ReferenceOutputWriteV1) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = std::mem::size_of::<ReferenceOutputWriteV1>() + 1;
        match &effect.coordinate {
            ReferenceOutputCoordinateV1::Dynamic(expression) => {
                units = add(units, self.expression(expression)?)?
            }
            ReferenceOutputCoordinateV1::LogicalPoint(expressions) => {
                for expression in expressions {
                    units = add(units, self.expression(expression)?)?;
                }
            }
            _ => {}
        }
        units = add(units, self.clauses(&effect.guard.clauses)?)?;
        units = add(units, self.expression(&effect.rhs)?)?;
        add(units, self.value(&effect.value)?)
    }

    fn effects(
        &self,
        effects: &[ReferenceOutputWriteV1],
    ) -> Result<usize, ReferenceBindingErrorV1> {
        let mut units = 0;
        for effect in effects {
            units = add(units, self.effect(effect)?)?;
        }
        Ok(units)
    }

    fn ir_hash(&self, ir: &ReferenceEffectIrV1) -> Result<(), ReferenceBindingErrorV1> {
        self.rows::<ReferenceArgumentRelationV1>(ir.relations.len())?;
        for block in &ir.blocks {
            self.charge(1)?;
            for assignment in &block.assignments {
                self.charge(1)?;
                self.charge(self.place(&assignment.destination)?)?;
                self.charge(self.value(&assignment.value)?)?;
            }
            self.charge(1)?;
            match &block.terminator {
                ReferenceTerminatorV1::Switch {
                    discriminant,
                    values,
                    ..
                } => {
                    self.charge(self.operand(discriminant)?)?;
                    self.rows::<(u128, u32)>(values.len())?;
                }
                ReferenceTerminatorV1::Assert {
                    condition,
                    bounds_check,
                    ..
                } => {
                    self.charge(self.operand(condition)?)?;
                    if let Some(bounds) = bounds_check {
                        self.charge(self.operand(&bounds.index)?)?;
                        self.charge(self.operand(&bounds.length)?)?;
                    }
                }
                _ => {}
            }
        }
        for summary in &ir.loop_summaries {
            self.rows::<ReferenceLoopSummaryV2>(1)?;
            self.rows::<u32>(summary.carried_locals.len())?;
        }
        self.charge(self.effects(&ir.observable_output_effects)?)
    }
}

pub fn add(a: usize, b: usize) -> Result<usize, ReferenceBindingErrorV1> {
    a.checked_add(b).ok_or_else(overflow)
}

fn overflow() -> ReferenceBindingErrorV1 {
    ReferenceBindingErrorV1::new("reference extraction work accounting overflowed")
}

/// Descriptive queries only; never a source-authentication meter.
pub(super) struct InspectionWorkV1;

impl ReferenceWorkV1 for InspectionWorkV1 {
    fn charge(&self, _: usize) -> Result<(), ReferenceBindingErrorV1> {
        Ok(())
    }
}
