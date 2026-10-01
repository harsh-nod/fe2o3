//! Compiler-created storage is an actual allocation, not an original MIR object.
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceEnumSpillOriginV48 as Origin;

#[derive(Clone, Copy, Debug)]
pub(super) struct Spill {
    pub(super) root: usize,
    pub(super) origin: Origin,
    pub(super) operation: Operation,
    pub(super) definition: usize,
    pub(super) physical_owner: usize,
}

type Key = [usize; 6];

impl Spill {
    fn key(&self) -> Key {
        [
            self.root,
            self.origin.instance,
            self.origin.local as usize,
            self.origin.variant as usize,
            self.origin.field as usize,
            self.origin.component,
        ]
    }
}

pub(super) struct CompilerSpills {
    rows: Vec<Spill>,
    by_operation: Vec<(Operation, usize)>,
}

impl CompilerSpills {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        relation: &Correspondence<'_>,
        operations: &mut Vec<Operation>,
        frames: &mut Vec<Option<Frame>>,
        source_order: &mut [(SourceKey, usize)],
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let source = relation.source(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let mut count = 0usize;
        for root in 0..roots {
            count = count
                .checked_add(relation.enum_spill_count_v48(root, out.budget)?)
                .ok_or(Resource::Arithmetic)?;
        }
        let mut rows: Vec<Spill> = vector(count, out)?;
        let mut by_operation = vector(count, out)?;
        let mut claimed = vector(operations.len(), out)?;
        out.budget.charge_work(operations.len())?;
        claimed.resize(operations.len(), None);
        for root in 0..roots {
            let (owner, physical) = source.root(root, out.budget)?;
            let function = inventory
                .functions()
                .get(physical)
                .ok_or_else(mismatch)?
                .coordinate;
            for ordinal in 0..relation.enum_spill_count_v48(root, out.budget)? {
                let receipt = relation.enum_spill_v48(root, ordinal, out.budget)?;
                let origin = receipt.origin(out.budget)?;
                let (definition, operation) = receipt.allocation(out.budget)?;
                let actual = inventory.operations().get(operation).ok_or_else(mismatch)?;
                let instance = plan.instance(root, origin.instance, out)?;
                out.budget.charge_work(8)?;
                if !instance.active
                    || origin.local as usize >= instance.locals.len()
                    || actual.coordinate.block.function != function
                    || actual.results.start != definition
                    || actual.results.len() != 1
                {
                    return Err(mismatch());
                }
                let at = locate(operations, &actual.coordinate, out.budget)?;
                if frames.get(at).is_none_or(Option::is_some)
                    || claimed.get(at).is_none_or(Option::is_some)
                    || rows.len() == count
                {
                    return Err(mismatch());
                }
                let row = Spill {
                    root,
                    origin,
                    operation: actual.coordinate,
                    definition,
                    physical_owner: owner.index() as usize,
                };
                if rows
                    .last()
                    .is_some_and(|previous| previous.key() >= row.key())
                {
                    return Err(mismatch());
                }
                claimed[at] = Some(rows.len());
                rows.push(row);
            }
        }
        if rows.len() != count || frames.len() != operations.len() {
            return Err(mismatch());
        }

        // Partition the complete actual allocation census. Original descriptors
        // remain dense; a compiler spill never acquires a source slot number.
        let mut remap = vector(operations.len(), out)?;
        out.budget.charge_work(operations.len())?;
        remap.resize(operations.len(), usize::MAX);
        let mut kept = 0usize;
        for at in 0..operations.len() {
            out.budget.charge_work(4)?;
            match (frames[at], claimed[at]) {
                (Some(frame), None) => {
                    remap[at] = kept;
                    operations[kept] = operations[at];
                    frames[kept] = Some(frame);
                    kept = kept.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
                (None, Some(row)) if row < rows.len() => {
                    by_operation.push((operations[at], row));
                }
                _ => return Err(mismatch()),
            }
        }
        if by_operation.len() != count || source_order.len() != kept {
            return Err(mismatch());
        }
        for (_, position) in source_order {
            out.budget.charge_work(2)?;
            *position = *remap
                .get(*position)
                .filter(|at| **at != usize::MAX)
                .ok_or_else(mismatch)?;
        }
        operations.truncate(kept);
        frames.truncate(kept);
        let credit = claimed
            .capacity()
            .checked_mul(size_of::<Option<usize>>())
            .and_then(|a| {
                remap
                    .capacity()
                    .checked_mul(size_of::<usize>())
                    .and_then(|b| a.checked_add(b))
            })
            .ok_or(Resource::Arithmetic)?;
        drop(claimed);
        drop(remap);
        out.budget.release_storage(credit)?;
        Ok(Self { rows, by_operation })
    }

    pub(super) fn allocation(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<&Spill>> {
        let (mut lo, mut hi) = (0, self.by_operation.len());
        while lo < hi {
            out.budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.by_operation[middle].0 < operation {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        out.budget.charge_work(1)?;
        Ok(self
            .by_operation
            .get(lo)
            .filter(|row| row.0 == operation)
            .map(|(_, row)| &self.rows[*row]))
    }

    pub(super) fn field(&self, key: Key, out: &mut Writer<'_, '_>) -> Result<Option<&Spill>> {
        let (mut lo, mut hi) = (0, self.rows.len());
        while lo < hi {
            out.budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.rows[middle].key() < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        out.budget.charge_work(1)?;
        Ok(self.rows.get(lo).filter(|row| row.key() == key))
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<CompilerSpills>()
        + h::<Spill>()
        + h::<Origin>()
        + h::<Vec<Spill>>()
        + h::<Vec<(Operation, usize)>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<usize>>()
        + h::<Key>()
        + h::<Option<&Spill>>()
        + 20 * size_of::<usize>()
        + 14 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_spill_partition_headers_have_independent_fixed_fields() {
        type Row = (usize, Origin, Operation, usize, usize);
        type Index = (Vec<Row>, Vec<(Operation, usize)>);
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(size_of::<Spill>(), size_of::<Row>());
        assert_eq!(size_of::<CompilerSpills>(), size_of::<Index>());
        assert_eq!(
            headers(),
            h::<Index>()
                + h::<Row>()
                + h::<Origin>()
                + h::<Vec<Row>>()
                + h::<Vec<(Operation, usize)>>()
                + h::<Vec<Option<usize>>>()
                + h::<Vec<usize>>()
                + h::<[usize; 6]>()
                + h::<Option<&Row>>()
                + 20 * size_of::<usize>()
                + 14 * size_of::<&()>()
        );
    }
}
