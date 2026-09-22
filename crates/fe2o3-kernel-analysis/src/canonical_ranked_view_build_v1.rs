//! Candidate construction. The independent checker does not call this traversal.
use super::*;

/// Builds ordered inert rows in two linear traversals of an existing inventory.
/// The graph/inventory/metadata are caller-owned and must remain prepaid. The
/// returned actual-capacity receipt is transferred: reserve it while the candidate
/// lives, and release only after dropping or transferring that candidate. Work and
/// peak/failure history persist; current storage returns to its incoming floor.
pub fn build_canonical_ranked_candidate_v1<'i, 'g, 'm>(
    inventory: &'i Inventory<'g>,
    metadata: &'i Metadata<'g, 'm>,
    budget: &mut Budget<'_>,
) -> Result<(Candidate<'i, 'g, 'm>, CanonicalRankedCandidateStorageV1)> {
    control::transaction(budget, |budget| {
        budget.charge_work(1)?;
        if !inventory.belongs_to(metadata.owner) {
            return Err(Error::ForeignMetadata);
        }
        let mut count = 0usize;
        visit(inventory, metadata, budget, |_, _| {
            count = add(count, 1)?;
            Ok(())
        })?;
        budget.reserve_storage(size_of::<Candidate<'_, '_, '_>>())?;
        let requested = payload::<Row>(count)?;
        budget.reserve_storage(requested)?;
        budget.charge_work(count)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        // Reconcile allocator excess before using any element. A denied excess
        // drops the vector before the surrounding transaction restores its floor.
        let actual = payload::<Row>(rows.capacity())?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        visit(inventory, metadata, budget, |item, _| {
            if rows.len() >= count || rows.len() >= rows.capacity() {
                return Err(Error::InconsistentInventory);
            }
            rows.push(item);
            Ok(())
        })?;
        if rows.len() != count {
            return Err(Error::InconsistentInventory);
        }
        let candidate = Candidate::from_rows(inventory, metadata, rows);
        let retained = candidate.retained_storage()?;
        Ok((candidate, CanonicalRankedCandidateStorageV1(retained)))
    })
}

fn visit(
    inventory: &Inventory<'_>,
    metadata: &Metadata<'_, '_>,
    budget: &mut Budget<'_>,
    mut emit: impl FnMut(Row, &mut Budget<'_>) -> Result<()>,
) -> Result<()> {
    let mut push = |item, budget: &mut Budget<'_>| {
        budget.charge_work(1)?;
        emit(item, budget)
    };
    push(
        row(Subject::Module, Role::Module, Obligations::NONE),
        budget,
    )?;
    for index in 0..inventory.kernels().len() {
        push(
            row(
                Subject::Kernel(index),
                Role::Kernel,
                Obligations::NONE.with(Obligation::Launch),
            ),
            budget,
        )?;
    }
    for (index, function) in inventory.functions().iter().enumerate() {
        budget.charge_work(1)?;
        let (role, obligations) = effects::function(function.function.body.is_some());
        push(row(Subject::Function(index), role, obligations), budget)?;
    }
    for (index, block) in inventory.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        let (class, obligations) = control::terminator(block.terminator);
        push(
            row(Subject::Block(index), Role::Block(class), obligations),
            budget,
        )?;
    }
    for index in 0..inventory.definitions().len() {
        push(
            row(
                Subject::Definition(index),
                Role::Definition,
                Obligations::NONE,
            ),
            budget,
        )?;
    }
    for (index, op) in inventory.operations().iter().enumerate() {
        budget.charge_work(1)?;
        let (class, mut obligations) = effects::operation(index, &op.operation.kind)?;
        if !op.compiler_ordering().is_empty() {
            obligations = obligations
                .with(Obligation::Ordering)
                .with(Obligation::Contract);
        }
        push(
            row(
                Subject::Operation(index),
                Role::Operation(class),
                obligations,
            ),
            budget,
        )?;
    }
    for index in 0..inventory.uses().len() {
        push(
            row(Subject::Use(index), Role::Use, Obligations::NONE),
            budget,
        )?;
    }
    for index in 0..inventory.edges().len() {
        let class = control::edge(inventory, index, budget)?;
        push(
            row(
                Subject::Edge(index),
                Role::Edge(class),
                Obligations::NONE.with(Obligation::Control),
            ),
            budget,
        )?;
    }
    for index in 0..inventory.edge_arguments().len() {
        push(
            row(
                Subject::EdgeArgument(index),
                Role::EdgeArgument,
                Obligations::NONE,
            ),
            budget,
        )?;
    }
    for (index, effect) in inventory.effects().iter().enumerate() {
        budget.charge_work(1)?;
        push(
            row(
                Subject::Effect(index),
                Role::Effect,
                effects::effect(effect.effect),
            ),
            budget,
        )?;
    }
    for index in 0..inventory.calls().len() {
        push(
            row(Subject::Call(index), Role::Call, effects::call()),
            budget,
        )?;
    }
    effects::requirements(inventory, budget, |owner, ordinal, budget| {
        push(
            row(
                Subject::Requirement { owner, ordinal },
                Role::Requirement,
                Obligations::NONE.with(Obligation::Target),
            ),
            budget,
        )
    })?;
    for (index, annotation) in metadata.rows.iter().enumerate() {
        budget.charge_work(1)?;
        push(
            row(
                Subject::Metadata(index),
                Role::Metadata,
                effects::metadata(annotation.kind),
            ),
            budget,
        )?;
    }
    Ok(())
}
