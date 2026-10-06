//! Exact original activation sites joined to independently retained backings.
//! The index is inert; only the original byte interpreter activates lifetimes.

use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceObjectActivationV40 as Activation;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLocalIdV1 as Local, SemanticStatementKindV1 as Statement, SemanticTypeIdV1 as TypeId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct ObjectActivation {
    // Root, invocation instance, original local, atomic original generation.
    key: [usize; 4],
    flat_local: usize,
    pub(in super::super) descriptor: usize,
    pub(in super::super) ty: TypeId,
    pub(in super::super) origin: Activation,
}

#[cfg(test)]
#[path = "original_semantic_mir_source_object_activations_v40_tests.rs"]
mod tests;

pub(super) struct SourceObjects {
    rows: Vec<ObjectActivation>,
}

fn error() -> Error {
    Error::Statement("original object activation and retained backing differ")
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceObjects>()
        + h::<Vec<ObjectActivation>>()
        + h::<ObjectActivation>()
        + h::<Activation>()
        + h::<[usize; 4]>()
        + h::<Option<ObjectActivation>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectLifetimeV40<'_, '_>>()
        + h::<(usize, usize, Local, u32, TypeId)>()
        + h::<(u32, Activation)>()
        + h::<Operation>()
        + h::<&Frame>()
        + h::<std::ops::Range<usize>>()
        + 30 * size_of::<usize>()
        + 12 * size_of::<&()>()
}

fn sort(rows: &mut [ObjectActivation], out: &mut Writer<'_, '_>) -> Result<()> {
    fn sift(
        rows: &mut [ObjectActivation],
        mut root: usize,
        end: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        loop {
            out.budget.charge_work(1)?;
            let mut child = root
                .checked_mul(2)
                .and_then(|row| row.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                break;
            }
            out.budget.charge_work(2)?;
            if child + 1 < end && rows[child].key < rows[child + 1].key {
                child += 1;
            }
            if rows[root].key >= rows[child].key {
                break;
            }
            out.budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..rows.len() / 2).rev() {
        out.budget.charge_work(1)?;
        sift(rows, root, rows.len(), out)?;
    }
    for end in (1..rows.len()).rev() {
        out.budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, out)?;
    }
    Ok(())
}

impl SourceObjects {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        relation: &Correspondence<'_>,
        operations: &[Operation],
        frames: &[Option<Frame>],
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = relation.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) || operations.len() != frames.len() {
            return Err(error());
        }
        let roots = source.root_count(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let mut capacity = 0usize;
        // Pay the entire escaping vector before any recipe is appended.
        for root in 0..roots {
            let count = relation.memory_object_lifetime_count_v40(root, out.budget)?;
            for ordinal in 0..count {
                let recipe = relation.memory_object_lifetime_at_v40(root, ordinal, out.budget)?;
                capacity = capacity
                    .checked_add(recipe.activation_count(out.budget)?)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        let mut rows = vector(capacity, out)?;
        for root in 0..roots {
            let count = relation.memory_object_lifetime_count_v40(root, out.budget)?;
            for ordinal in 0..count {
                let recipe = relation.memory_object_lifetime_at_v40(root, ordinal, out.budget)?;
                let (found_root, instance, local, _, ty) = recipe.identity(out.budget)?;
                let at = locate(
                    operations,
                    &recipe.original_backing(out.budget)?,
                    out.budget,
                )?;
                let frame = frames.get(at).and_then(Option::as_ref).ok_or_else(error)?;
                let invocation = plan.instance(root, instance, out)?;
                let function = semantic
                    .functions()
                    .get(invocation.function.index() as usize)
                    .ok_or_else(error)?;
                let original = semantic
                    .types()
                    .get(ty.index() as usize)
                    .ok_or_else(error)?;
                let flat_local = invocation
                    .locals
                    .start
                    .checked_add(local.index() as usize)
                    .ok_or(Resource::Arithmetic)?;
                out.budget.charge_work(11)?;
                if found_root != root
                    || frame.root() != root
                    || frame.instance() != instance
                    || frame.local() != local.index()
                    || frame.semantic_type() != ty
                    || frame.function() != invocation.function
                    || !invocation.active
                    || flat_local >= invocation.locals.end
                    || frame.source_generation().is_none()
                    || function
                        .locals()
                        .get(local.index() as usize)
                        .is_none_or(|decl| decl.ty() != ty)
                    || original.layout().size_bytes() != Some(frame.bytes())
                    || u64::from(frame.alignment()) % original.layout().alignment_bytes() != 0
                {
                    return Err(error());
                }
                for member in 0..recipe.activation_count(out.budget)? {
                    let (generation, origin) = recipe.activation(member, out.budget)?;
                    out.budget.charge_work(3)?;
                    match origin {
                        Activation::Entry if generation == 0 => {}
                        Activation::StorageLive { block, statement } => {
                            if !matches!(function.blocks().get(block.index() as usize)
                                .and_then(|row| row.statements().get(statement)).map(|row| row.kind()),
                                Some(Statement::StorageLive(found)) if *found == local)
                            {
                                return Err(error());
                            }
                        }
                        _ => return Err(error()),
                    }
                    if rows.len() >= capacity || rows.len() >= rows.capacity() {
                        return Err(Resource::Accounting.into());
                    }
                    rows.push(ObjectActivation {
                        key: [root, instance, local.index() as usize, generation as usize],
                        flat_local,
                        descriptor: at,
                        ty,
                        origin,
                    });
                }
            }
        }
        if rows.len() != capacity {
            return Err(error());
        }
        sort(&mut rows, out)?;
        let mut count = 0usize;
        for index in 0..rows.len() {
            out.budget.charge_work(3)?;
            let row = rows[index];
            if count != 0 && rows[count - 1].key == row.key {
                // Distinct logical join labels may name one original activation;
                // they must agree on the exact whole-object physical backing.
                if rows[count - 1] != row {
                    return Err(error());
                }
            } else {
                rows[count] = row;
                count += 1;
            }
        }
        rows.truncate(count);
        Ok(Self { rows })
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.reserve_storage(headers())?;
        write!(out, "spec fn invocation_source_object_binding_v40(local: int, object: InvocationSourceObjectV40) -> bool {{ false")
            .map_err(|_| out.error())?;
        for row in &self.rows {
            out.budget.charge_work(4)?;
            write!(out, "\n    || (local == {}int && object.descriptor == {}int && object.activation == {}int && object.slot == invocation_source_slot_{}_v36())",
                row.flat_local, row.descriptor, row.key[3], row.descriptor)
                .map_err(|_| out.error())?;
        }
        write!(out, "\n}}\n").map_err(|_| out.error())
    }

    pub(super) fn local_range(
        &self,
        root: usize,
        instance: usize,
        local: u32,
        out: &mut Writer<'_, '_>,
    ) -> Result<std::ops::Range<usize>> {
        let key = [root, instance, local as usize];
        let bound = |upper: bool, out: &mut Writer<'_, '_>| -> Result<usize> {
            let (mut lo, mut hi) = (0, self.rows.len());
            while lo < hi {
                out.budget.charge_work(1)?;
                let middle = lo + (hi - lo) / 2;
                let found = &self.rows[middle].key[..3];
                if found < key.as_slice() || upper && found == key.as_slice() {
                    lo = middle + 1;
                } else {
                    hi = middle;
                }
            }
            Ok(lo)
        };
        Ok(bound(false, out)?..bound(true, out)?)
    }

    pub(super) fn activation(
        &self,
        root: usize,
        instance: usize,
        local: u32,
        generation: u32,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<ObjectActivation>> {
        let key = [root, instance, local as usize, generation as usize];
        let (mut lo, mut hi) = (0, self.rows.len());
        while lo < hi {
            out.budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if self.rows[middle].key < key {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        out.budget.charge_work(1)?;
        Ok(self.rows.get(lo).filter(|row| row.key == key).copied())
    }
}
