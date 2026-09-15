//! Relocates only reserved pure roots, retaining every memory event and source site.

use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionRankedAccessSourceV1, ProductionRankedExecutableEffectSourceV1,
};

mod remap;
use remap::Remap;
mod dominance;
use dominance::ReadDominance;

type Site = (usize, usize);
type O = ProductionRankedOperationV1;
type X = ProductionSemanticExpressionV2;
type V = ProductionRankedValueV1;
type Id = ProductionRankedValueIdV1;
type E = ProductionReferenceEffectJoinErrorV2;

struct Work {
    used: usize,
}

impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), E> {
        self.used = self.used.checked_add(amount).ok_or_else(resource)?;
        if self.used > MAX_REFERENCE_STATEMENTS_V1 {
            return Err(resource());
        }
        Ok(())
    }
}

fn resource() -> E {
    E::Recipe(ProductionRankedKernelErrorV1::ResourceLimit {
        resource: "source read root placement work",
        limit: MAX_REFERENCE_STATEMENTS_V1,
        actual: MAX_REFERENCE_STATEMENTS_V1 + 1,
    })
}

fn reject(reason: &'static str) -> E {
    E::UnsupportedReference(reason)
}

struct MappedEffect {
    target: Site,
    expected: O,
}

/// Private custody over the exact old/new memory-event correspondence.
pub(super) struct CheckedReadRootPlacementV1 {
    effects: BTreeMap<Site, MappedEffect>,
}

impl CheckedReadRootPlacementV1 {
    pub(super) fn remap_sources(
        &self,
        kernel: &ProductionRankedKernelV1,
        accesses: &mut [ProductionRankedAccessSourceV1],
        generated: &mut [ProductionRankedExecutableEffectSourceV1],
    ) -> Result<(), E> {
        let count = accesses
            .len()
            .checked_add(generated.len())
            .ok_or_else(resource)?;
        if count != self.effects.len() || count > MAX_REFERENCE_STATEMENTS_V1 {
            return Err(reject(
                "read-root placement requires the complete exact source effect roster",
            ));
        }
        let mut used = BTreeSet::new();
        let mut locate = |block: u32, operation: u32| -> Result<(u32, u32), E> {
            let site = (block as usize, operation as usize);
            if !used.insert(site) {
                return Err(reject("read-root placement source effect is duplicated"));
            }
            let mapped = self
                .effects
                .get(&site)
                .ok_or_else(|| reject("read-root placement source effect is absent"))?;
            if kernel
                .blocks()
                .get(mapped.target.0)
                .and_then(|block| block.operations().get(mapped.target.1))
                != Some(&mapped.expected)
            {
                return Err(reject(
                    "read-root placement changed its retained target effect",
                ));
            }
            Ok((
                u32::try_from(mapped.target.0).map_err(|_| resource())?,
                u32::try_from(mapped.target.1).map_err(|_| resource())?,
            ))
        };
        // Validate the entire join before changing either caller-owned roster.
        let remapped_accesses = accesses
            .iter()
            .map(|source| {
                let (block, operation) = locate(source.ranked_block(), source.ranked_operation())?;
                Ok(ProductionRankedAccessSourceV1::new(
                    source.semantic_block(),
                    source.semantic_statement(),
                    source.semantic_access_ordinal(),
                    block,
                    operation,
                ))
            })
            .collect::<Result<Vec<_>, E>>()?;
        let remapped_generated = generated
            .iter()
            .map(|source| {
                let (block, operation) = locate(source.ranked_block(), source.ranked_operation())?;
                Ok(ProductionRankedExecutableEffectSourceV1::new(
                    source.semantic_block(),
                    source.semantic_effect_ordinal(),
                    block,
                    operation,
                    source.origin(),
                    source.recipe_identity(),
                ))
            })
            .collect::<Result<Vec<_>, E>>()?;
        accesses.copy_from_slice(&remapped_accesses);
        generated.copy_from_slice(&remapped_generated);
        Ok(())
    }
}

pub(super) fn required(outputs: &[PreparedReferenceOutputV2]) -> Result<bool, E> {
    let mut work = Work { used: 0 };
    let mut found = false;
    for output in outputs {
        for expression in [&output.gpu_expression, &output.reference_expression] {
            visit_loads(expression, &mut work, &mut |_, _| {
                found = true;
                Ok(())
            })?;
        }
    }
    Ok(found)
}

pub(super) fn place(
    kernel: &ProductionRankedKernelV1,
    outputs: &mut [PreparedReferenceOutputV2],
    effect_ir: &ReferenceEffectIrV1,
    bounds_work: &mut ReferenceSymbolicWorkBudgetV2,
) -> Result<(Vec<ProductionRankedBlockV1>, CheckedReadRootPlacementV1), E> {
    let mut work = Work { used: 0 };
    let original_entry = kernel
        .blocks()
        .first()
        .ok_or(E::WriteLocation)?
        .operations();
    work.charge(original_entry.len())?;
    let mut entry = original_entry.to_vec();
    let mut entry_views = BTreeSet::new();
    for operation in original_entry {
        work.charge(1)?;
        if let O::View { result, .. } | O::ViewInSpace { result, .. } = operation {
            work.charge(2)?;
            entry_views.insert(V::Local(*result));
        }
    }
    let mut moved = BTreeMap::<Id, Site>::new();
    let mut reservations = BTreeSet::new();
    let mut read_descriptors = BTreeMap::new();
    let mut dominance = ReadDominance::new(kernel, &mut work)?;
    for output in outputs.iter() {
        work.charge(1)?;
        if !entry_views.contains(&output.write.view) {
            return Err(reject(
                "output ownership metadata requires an entry-defined view",
            ));
        }
        let [truth, gpu, cpu, coordinates @ ..] = output.reserved_values.as_slice() else {
            return Err(reject("read-root placement has an incomplete reservation"));
        };
        for id in &output.reserved_values {
            if !reservations.insert(*id) {
                return Err(E::InvalidReservedValue(id.get()));
            }
        }
        let destination = (output.write.block, output.write.operation);
        let mut reads = false;
        for expression in [&output.gpu_expression, &output.reference_expression] {
            visit_loads(expression, &mut work, &mut |load, work| {
                reads = true;
                let site = (load.block as usize, load.operation as usize);
                if let Some(previous) = read_descriptors.get(&site) {
                    if previous != load {
                        return Err(reject(
                            "read-root placement has conflicting metadata for one source read",
                        ));
                    }
                } else {
                    read_descriptors.insert(site, load.clone());
                }
                dominance.require_preceding(site, destination, work)?;
                if !matches!(kernel.blocks().get(load.block as usize)
                    .and_then(|block| block.operations().get(load.operation as usize)),
                    Some(O::Access { kind: dialect_kernel::AccessKindAttr::Read, view, indices })
                    if *view == load.view && indices.as_slice() == load.indices.as_ref())
                {
                    return Err(reject(
                        "read-root placement lost the exact original read access",
                    ));
                }
                Ok(())
            })?;
        }
        work.charge(
            entry
                .len()
                .checked_mul(output.reserved_values.len())
                .ok_or_else(resource)?,
        )?;
        replace_reserved_semantic_expression_v2(
            &mut entry,
            *truth,
            X::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            },
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        )?;
        replace_reserved_semantic_expression_v2(
            &mut entry,
            *gpu,
            output.gpu_expression.clone(),
            output.numerical_contract,
        )?;
        replace_reserved_semantic_expression_v2(
            &mut entry,
            *cpu,
            output.reference_expression.clone(),
            output.numerical_contract,
        )?;
        for (axis, id) in coordinates.iter().enumerate() {
            compact_row_v1::place_coordinate(
                &mut entry,
                *id,
                u32::try_from(axis).map_err(|_| resource())?,
                &output.reference_write.coordinate,
                effect_ir,
                bounds_work,
            )?;
        }
        if reads {
            moved.insert(*gpu, destination);
            moved.insert(*cpu, destination);
        }
    }
    let mut pending = BTreeMap::<Site, Vec<(Site, O)>>::new();
    for (operation, op) in entry.iter().enumerate() {
        work.charge(1)?;
        if let Some(destination) = operation_result_v2(op).and_then(|id| moved.get(&id)) {
            pending
                .entry(*destination)
                .or_default()
                .push(((0, operation), op.clone()));
        }
    }
    if pending.values().map(Vec::len).sum::<usize>() != moved.len() {
        return Err(reject(
            "read-root placement reservation has no unique entry definition",
        ));
    }
    let mut placed = Vec::with_capacity(kernel.blocks().len());
    for (block_index, block) in kernel.blocks().iter().enumerate() {
        let operations = if block_index == 0 {
            entry.as_slice()
        } else {
            block.operations()
        };
        let mut destination = Vec::with_capacity(operations.len());
        for (operation, op) in operations.iter().enumerate() {
            work.charge(1)?;
            let site = (block_index, operation);
            if let Some(roots) = pending.remove(&site) {
                destination.extend(roots);
            }
            if block_index == 0 && operation_result_v2(op).is_some_and(|id| moved.contains_key(&id))
            {
                continue;
            }
            destination.push((site, op.clone()));
        }
        placed.push(destination);
    }
    if !pending.is_empty() {
        return Err(E::WriteLocation);
    }
    let mut values = BTreeMap::new();
    let mut sites = BTreeMap::new();
    for (block, operations) in placed.iter().enumerate() {
        for (operation, (old_site, op)) in operations.iter().enumerate() {
            work.charge(1)?;
            if sites.insert(*old_site, (block, operation)).is_some() {
                return Err(E::WriteLocation);
            }
            if let Some(old) = operation_result_v2(op) {
                let new = Id::new(u32::try_from(values.len()).map_err(|_| resource())?);
                if values.insert(old, new).is_some() {
                    return Err(E::InvalidReservedValue(old.get()));
                }
            }
        }
    }
    let mut remap = Remap::new(values, sites, &mut work);
    // A reserved placeholder must not already influence any source operation.
    for block in kernel.blocks() {
        for op in block.operations() {
            remap.reject_reserved_uses(op, &reservations)?;
        }
        remap.reject_reserved_terminator_uses(block.terminator(), &reservations)?;
    }
    let mut effects = BTreeMap::new();
    let mut blocks = Vec::with_capacity(placed.len());
    for (block, operations) in placed.into_iter().enumerate() {
        let mut rewritten = Vec::with_capacity(operations.len());
        for (old_site, mut op) in operations {
            remap.operation(&mut op)?;
            if is_effect(&op) {
                effects.insert(
                    old_site,
                    MappedEffect {
                        target: remap.site(old_site)?,
                        expected: op.clone(),
                    },
                );
            }
            rewritten.push(op);
        }
        let mut terminator = kernel.blocks()[block].terminator().clone();
        remap.terminator(&mut terminator)?;
        blocks.push(ProductionRankedBlockV1::with_index_arguments(
            kernel.blocks()[block].index_argument_count(),
            rewritten,
            terminator,
        ));
    }
    remap.charge(outputs.len())?;
    let mut ownership = Vec::with_capacity(outputs.len());
    for output in outputs {
        let old_write = (output.write.block, output.write.operation);
        let target = remap.site(old_write)?;
        output.write.block = target.0;
        output.write.operation = target.1;
        remap.value(&mut output.write.view)?;
        for index in &mut output.write.indices {
            remap.value(index)?;
        }
        if let Ok(expression) = &mut output.write.value {
            remap.expression(expression)?;
        }
        remap.expression(&mut output.gpu_expression)?;
        remap.expression(&mut output.reference_expression)?;
        for id in &mut output.reserved_values {
            remap.id(id)?;
        }
        let effect = effects.get_mut(&old_write).ok_or(E::WriteLocation)?;
        if !matches!(&effect.expected, O::Access { kind, view, indices }
            if *kind == dialect_kernel::AccessKindAttr::Write
                && *view == output.write.view && *indices == output.write.indices)
        {
            return Err(reject("read-root placement changed its exact output write"));
        }
        effect.expected = O::ValueAccess {
            kind: dialect_kernel::AccessKindAttr::Write,
            view: output.write.view,
            indices: output.write.indices.clone(),
            value: V::Local(output.reserved_values[1]),
        };
        ownership.push(O::OwnershipContract {
            view: output.write.view,
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        });
    }
    // Metadata is unconditional; executable operations keep their original sites.
    let entry = &blocks[0];
    remap.charge(entry.operations().len())?;
    let mut operations = entry.operations().to_vec();
    operations.extend(ownership);
    blocks[0] = ProductionRankedBlockV1::with_index_arguments(
        entry.index_argument_count(),
        operations,
        entry.terminator().clone(),
    );
    Ok((blocks, CheckedReadRootPlacementV1 { effects }))
}

fn is_effect(op: &O) -> bool {
    matches!(
        op,
        O::Access { .. }
            | O::ValueAccess { .. }
            | O::AtomicAccess { .. }
            | O::AtomicValueAccess { .. }
            | O::AllocationEffect { .. }
    )
}

fn visit_loads(
    expression: &X,
    work: &mut Work,
    visitor: &mut impl FnMut(&fe2o3_pliron::ProductionSemanticLoadV2, &mut Work) -> Result<(), E>,
) -> Result<(), E> {
    expression.validate().map_err(E::SemanticExpression)?;
    fn visit(
        expression: &X,
        work: &mut Work,
        visitor: &mut impl FnMut(&fe2o3_pliron::ProductionSemanticLoadV2, &mut Work) -> Result<(), E>,
    ) -> Result<(), E> {
        work.charge(1)?;
        match expression {
            X::Load(load) => visitor(load, work),
            X::Symbol { .. } | X::Constant { .. } => Ok(()),
            X::Unary { operand, .. } | X::Cast { operand, .. } => visit(operand, work, visitor),
            X::Binary { lhs, rhs, .. } | X::Compare { lhs, rhs, .. } => {
                visit(lhs, work, visitor)?;
                visit(rhs, work, visitor)
            }
            X::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                visit(condition, work, visitor)?;
                visit(when_true, work, visitor)?;
                visit(when_false, work, visitor)
            }
        }
    }
    visit(expression, work, visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_placement_work_limit_is_not_raised() {
        let mut work = Work { used: 0 };
        work.charge(MAX_REFERENCE_STATEMENTS_V1).unwrap();
        assert!(
            matches!(work.charge(1), Err(E::Recipe(ProductionRankedKernelErrorV1::ResourceLimit {
            resource: "source read root placement work", limit, actual,
        })) if limit == MAX_REFERENCE_STATEMENTS_V1 && actual == limit + 1)
        );
    }
}
