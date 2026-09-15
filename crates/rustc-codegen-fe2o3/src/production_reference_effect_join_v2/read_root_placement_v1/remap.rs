use super::*;
use fe2o3_pliron::ProductionSemanticLoadV2;

pub(super) struct Remap<'a> {
    values: BTreeMap<Id, Id>,
    sites: BTreeMap<Site, Site>,
    work: &'a mut Work,
}

impl<'a> Remap<'a> {
    pub(super) fn new(
        values: BTreeMap<Id, Id>,
        sites: BTreeMap<Site, Site>,
        work: &'a mut Work,
    ) -> Self {
        Self {
            values,
            sites,
            work,
        }
    }
    pub(super) fn site(&self, site: Site) -> Result<Site, E> {
        self.sites.get(&site).copied().ok_or(E::WriteLocation)
    }
    pub(super) fn charge(&mut self, amount: usize) -> Result<(), E> {
        self.work.charge(amount)
    }
    pub(super) fn id(&mut self, id: &mut Id) -> Result<(), E> {
        Rewrite::id(self, id)
    }
    pub(super) fn value(&mut self, value: &mut V) -> Result<(), E> {
        Rewrite::value(self, value)
    }
    pub(super) fn expression(&mut self, expression: &mut X) -> Result<(), E> {
        rewrite_expression(expression, self)
    }
    pub(super) fn operation(&mut self, op: &mut O) -> Result<(), E> {
        operation(op, self)
    }
    pub(super) fn terminator(&mut self, term: &mut ProductionRankedTerminatorV1) -> Result<(), E> {
        terminator(term, self)
    }
    pub(super) fn reject_reserved_uses(
        &mut self,
        op: &O,
        reserved: &BTreeSet<Id>,
    ) -> Result<(), E> {
        if let O::SemanticExpression { expression, .. } = op {
            self.charge(expression.validate().map_err(E::SemanticExpression)?.nodes)?;
        }
        operation(
            &mut op.clone(),
            &mut NoReservedUses {
                reserved,
                work: self.work,
            },
        )
    }
    pub(super) fn reject_reserved_terminator_uses(
        &mut self,
        term: &ProductionRankedTerminatorV1,
        reserved: &BTreeSet<Id>,
    ) -> Result<(), E> {
        terminator(
            &mut term.clone(),
            &mut NoReservedUses {
                reserved,
                work: self.work,
            },
        )
    }
}

trait Rewrite {
    fn charge(&mut self, amount: usize) -> Result<(), E>;
    fn id(&mut self, id: &mut Id) -> Result<(), E>;
    fn value(&mut self, value: &mut V) -> Result<(), E>;
    fn load(&mut self, load: &mut ProductionSemanticLoadV2) -> Result<(), E> {
        self.value(&mut load.view)?;
        for index in &mut load.indices {
            self.value(index)?;
        }
        Ok(())
    }
}

impl Rewrite for Remap<'_> {
    fn charge(&mut self, amount: usize) -> Result<(), E> {
        self.work.charge(amount)
    }
    fn id(&mut self, id: &mut Id) -> Result<(), E> {
        self.charge(1)?;
        *id = *self
            .values
            .get(id)
            .ok_or(E::InvalidReservedValue(id.get()))?;
        Ok(())
    }
    fn value(&mut self, value: &mut V) -> Result<(), E> {
        self.charge(1)?;
        if let V::Local(id) = value {
            self.id(id)?;
        }
        Ok(())
    }
    fn load(&mut self, load: &mut ProductionSemanticLoadV2) -> Result<(), E> {
        let target = self.site((load.block as usize, load.operation as usize))?;
        load.block = u32::try_from(target.0).map_err(|_| resource())?;
        load.operation = u32::try_from(target.1).map_err(|_| resource())?;
        self.value(&mut load.view)?;
        for index in &mut load.indices {
            self.value(index)?;
        }
        Ok(())
    }
}

struct NoReservedUses<'a> {
    reserved: &'a BTreeSet<Id>,
    work: &'a mut Work,
}

impl Rewrite for NoReservedUses<'_> {
    fn charge(&mut self, amount: usize) -> Result<(), E> {
        self.work.charge(amount)
    }
    fn id(&mut self, _: &mut Id) -> Result<(), E> {
        self.charge(1)
    }
    fn value(&mut self, value: &mut V) -> Result<(), E> {
        self.charge(1)?;
        if let V::Local(id) = value
            && self.reserved.contains(id)
        {
            return Err(reject(
                "read-root placement reservation already has a source use",
            ));
        }
        Ok(())
    }
}

fn rewrite_expression(expression: &mut X, map: &mut impl Rewrite) -> Result<(), E> {
    expression.validate().map_err(E::SemanticExpression)?;
    fn visit(expression: &mut X, map: &mut impl Rewrite) -> Result<(), E> {
        map.charge(1)?;
        match expression {
            X::Load(load) => map.load(load),
            X::Symbol { .. } | X::Constant { .. } => Ok(()),
            X::Unary { operand, .. } | X::Cast { operand, .. } => visit(operand, map),
            X::Binary { lhs, rhs, .. } | X::Compare { lhs, rhs, .. } => {
                visit(lhs, map)?;
                visit(rhs, map)
            }
            X::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                visit(condition, map)?;
                visit(when_true, map)?;
                visit(when_false, map)
            }
        }
    }
    visit(expression, map)
}

fn operation(op: &mut O, map: &mut impl Rewrite) -> Result<(), E> {
    map.charge(1)?;
    match op {
        O::ExecutionLayout { .. }
        | O::AllocationEffect { .. }
        | O::Barrier { .. }
        | O::Fence { .. } => {}
        O::View {
            result,
            dynamic_extents,
            ..
        }
        | O::ViewInSpace {
            result,
            dynamic_extents,
            ..
        } => {
            map.id(result)?;
            for value in dynamic_extents {
                map.value(value)?;
            }
        }
        O::IndexConstant { result, .. }
        | O::IndexUnknown { result }
        | O::InvocationIndex { result, .. }
        | O::SemanticConstant { result, .. }
        | O::SemanticSymbol { result, .. } => {
            map.id(result)?;
        }
        O::IndexUnsignedCast { result, source, .. } => {
            map.id(result)?;
            map.value(source)?;
        }
        O::IndexBinary {
            result, lhs, rhs, ..
        } => {
            map.id(result)?;
            map.value(lhs)?;
            map.value(rhs)?;
        }
        O::DeterministicJoin {
            result,
            dependencies,
        } => {
            map.id(result)?;
            for value in dependencies {
                map.value(value)?;
            }
        }
        O::CheckedTiledIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            ..
        }
        | O::CheckedRowStripedIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            ..
        } => {
            map.id(result)?;
            for value in [invocation, component, rows, columns, row_stride] {
                map.value(value)?;
            }
        }
        O::Dimension { result, view, .. } => {
            map.id(result)?;
            map.value(view)?;
        }
        O::Access { view, indices, .. } | O::AtomicAccess { view, indices, .. } => {
            map.value(view)?;
            for value in indices {
                map.value(value)?;
            }
        }
        O::ValueAccess {
            view,
            indices,
            value,
            ..
        }
        | O::AtomicValueAccess {
            view,
            indices,
            value,
            ..
        } => {
            map.value(view)?;
            map.value(value)?;
            for value in indices {
                map.value(value)?;
            }
        }
        O::OwnershipContract { view, .. } => {
            map.value(view)?;
        }
        O::SemanticExpression {
            result, expression, ..
        } => {
            map.id(result)?;
            rewrite_expression(expression, map)?;
        }
        // Rewriting already normalized proof obligations would invalidate their
        // subjects. Multi-result and other unsupported schemas stay closed.
        _ => {
            return Err(reject(
                "read-root placement cannot remap this ranked operation contract",
            ));
        }
    }
    Ok(())
}

fn terminator(term: &mut ProductionRankedTerminatorV1, map: &mut impl Rewrite) -> Result<(), E> {
    use ProductionRankedTerminatorV1 as T;
    map.charge(1)?;
    match term {
        T::IndexLessThan { lhs, rhs, .. } | T::IndexEqual { lhs, rhs, .. } => {
            map.value(lhs)?;
            map.value(rhs)?;
        }
        T::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            ..
        }
        | T::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            ..
        } => {
            map.value(lhs)?;
            map.value(rhs)?;
            for value in true_arguments.iter_mut().chain(false_arguments) {
                map.value(value)?;
            }
        }
        T::AnalysisSplit {
            control_dependencies,
            ..
        } => {
            for value in control_dependencies {
                map.value(value)?;
            }
        }
        T::AnalysisSplitArgs {
            control_dependencies,
            first_arguments,
            second_arguments,
            ..
        } => {
            for value in control_dependencies
                .iter_mut()
                .chain(first_arguments)
                .chain(second_arguments)
            {
                map.value(value)?;
            }
        }
        T::Branch { .. } | T::Return | T::Trap => {}
        T::BranchArgs { arguments, .. } => {
            for value in arguments {
                map.value(value)?;
            }
        }
        T::BranchArgsAdd { value, step, .. } => {
            map.value(value)?;
            map.value(step)?;
        }
        T::BranchArgsAddAt {
            arguments, step, ..
        } => {
            map.value(step)?;
            for value in arguments {
                map.value(value)?;
            }
        }
    }
    Ok(())
}
