//! Independent admission: no candidate-builder call or hash-only binding.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Replays complete graph coverage once, then permits paid O(1) queries in a
/// nonescaping scope. Inputs are prepaid: graph/inventory under their existing
/// receipts, candidate actual capacity and metadata logical extent. The guard
/// retains the entire incoming floor, not just its own header reservation.
///
/// Metadata is INERT: this verifies its anchor roster and reference ranges, not
/// source ownership or the truth/completeness of source facts. Missing source
/// facts cannot discharge any pending obligation. Later lowerer/Pliron admission
/// must establish them and run actual ranked checks.
///
/// Callback scratch must be released on query/exit. Foreign ledger/slot queries,
/// undercut backing and ignored query-resource failures poison the result. Rejected
/// results drop while backing is still paid; caught panic payloads drop only after
/// same-ledger cleanup. No replacement ledger is refunded or minted credits restored.
///
/// The checked facade cannot escape its paid scope:
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalRankedMetadataV1,
///     CanonicalRankedCandidateV1, CanonicalRankedViewErrorV1,
///     with_checked_canonical_ranked_view_v1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'i, 'g, 'm>(inventory: &'i CanonicalKirInventoryV1<'g>,
///     metadata: &'i CanonicalRankedMetadataV1<'g, 'm>,
///     candidate: &'i CanonicalRankedCandidateV1<'i, 'g, 'm>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_checked_canonical_ranked_view_v1(inventory, metadata, candidate,
///         budget, |view, _| Ok::<_, CanonicalRankedViewErrorV1>(view));
/// }
/// ```
pub fn with_checked_canonical_ranked_view_v1<'i, 'g, 'm, 'work, T, E>(
    inventory: &'i Inventory<'g>,
    metadata: &'i Metadata<'g, 'm>,
    candidate: &'i Candidate<'i, 'g, 'm>,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut CheckedCanonicalRankedViewV1<'scope, 'i, 'g, 'm>,
        &mut Budget<'work>,
    ) -> std::result::Result<T, E>,
) -> std::result::Result<T, E>
where
    E: From<Error>,
{
    budget
        .charge_work(1)
        .map_err(Error::from)
        .map_err(E::from)?;
    if !std::ptr::eq(candidate.inventory, inventory) {
        return Err(E::from(Error::ForeignInventory));
    }
    if !std::ptr::eq(candidate.metadata, metadata) || !inventory.belongs_to(metadata.owner) {
        return Err(E::from(Error::ForeignMetadata));
    }
    let minimum = add(
        candidate.retained_storage().map_err(E::from)?,
        metadata.storage_extent(budget).map_err(E::from)?,
    )
    .map_err(E::from)?;
    if budget.storage() < minimum {
        return Err(E::from(Error::Resource(Resource::Accounting)));
    }
    let retained = add(
        size_of::<control::Accounting>(),
        size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>(),
    )
    .map_err(E::from)?;
    let floor = budget.storage();
    budget
        .reserve_storage(retained)
        .map_err(Error::from)
        .map_err(E::from)?;
    let mut accounting = control::Accounting::new(budget, floor, retained);
    let mut panics = [None, None];
    let protected = catch_unwind(AssertUnwindSafe(|| -> Result<std::result::Result<T, E>> {
        check(inventory, metadata, candidate, budget)?;
        let mut view = CheckedCanonicalRankedViewV1 {
            candidate,
            accounting: &mut accounting,
        };
        Ok(run(&mut view, budget))
    }));
    let mut returned = None;
    let mut failure = None;
    match protected {
        Ok(Ok(value)) => returned = Some(value),
        Ok(Err(error)) => failure = Some(error),
        Err(payload) => {
            panics[0] = Some(payload);
            failure = Some(Error::Panicked);
        }
    }
    if let Err(error) = accounting.postcheck(budget) {
        if let Some(value) = returned.take() {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
                panics[1] = Some(payload);
            }
        }
        failure = Some(error);
    }
    // No graph/candidate owner is duplicated or moved. The scoped view has ended.
    if let Err(error) = accounting.release(budget) {
        if let Some(value) = returned.take() {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
                panics[1] = Some(payload);
            }
        }
        failure = Some(error);
    }
    drop(panics);
    match failure {
        Some(error) => Err(E::from(error)),
        None => returned.expect("successful checked-view callback retained its result"),
    }
}

struct Reader<'a> {
    rows: &'a [Row],
    next: usize,
}
impl Reader<'_> {
    fn expect(&mut self, expected: Row, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let actual = self.rows.get(self.next).copied().ok_or(Error::MissingRow {
            ordinal: self.next,
            expected,
        })?;
        if actual != expected {
            return Err(Error::MismatchedRow {
                ordinal: self.next,
                expected,
                actual,
            });
        }
        self.next = add(self.next, 1)?;
        Ok(())
    }
    fn requirement(
        &mut self,
        owner: RequirementOwner,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.expect(
            row(
                Subject::Requirement { owner, ordinal },
                Role::Requirement,
                Obligations::NONE.with(Obligation::Target),
            ),
            budget,
        )
    }
}

fn check(
    inventory: &Inventory<'_>,
    metadata: &Metadata<'_, '_>,
    candidate: &Candidate<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let mut reader = Reader {
        rows: &candidate.rows,
        next: 0,
    };
    reader.expect(
        row(Subject::Module, Role::Module, Obligations::NONE),
        budget,
    )?;
    for (i, _) in inventory.kernels().iter().enumerate() {
        reader.expect(
            row(
                Subject::Kernel(i),
                Role::Kernel,
                Obligations::NONE.with(Obligation::Launch),
            ),
            budget,
        )?;
    }
    for (i, f) in inventory.functions().iter().enumerate() {
        budget.charge_work(1)?;
        let (role, obligations) = effects::function(f.function.body.is_some());
        reader.expect(row(Subject::Function(i), role, obligations), budget)?;
    }
    for (i, b) in inventory.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        let (kind, obligations) = control::terminator(b.terminator);
        reader.expect(
            row(Subject::Block(i), Role::Block(kind), obligations),
            budget,
        )?;
    }
    for (i, _) in inventory.definitions().iter().enumerate() {
        reader.expect(
            row(Subject::Definition(i), Role::Definition, Obligations::NONE),
            budget,
        )?;
    }
    for (i, operation) in inventory.operations().iter().enumerate() {
        budget.charge_work(1)?;
        let (kind, mut obligations) = effects::operation(i, &operation.operation.kind)?;
        if !operation.compiler_ordering().is_empty() {
            obligations = obligations
                .with(Obligation::Ordering)
                .with(Obligation::Contract);
        }
        reader.expect(
            row(Subject::Operation(i), Role::Operation(kind), obligations),
            budget,
        )?;
    }
    for (i, _) in inventory.uses().iter().enumerate() {
        reader.expect(row(Subject::Use(i), Role::Use, Obligations::NONE), budget)?;
    }
    // Traverse actual block terminators independently, rather than the builder's
    // per-edge classification routine. Compare original payloads before rows.
    let mut edge_index = 0usize;
    for block in inventory.blocks() {
        budget.charge_work(1)?;
        let mut occurrence = 0usize;
        block.terminator.try_visit_edges_v1(|target, args| {
            budget.charge_work(add(2, args.len())?)?;
            let edge = inventory
                .edges()
                .get(edge_index)
                .ok_or(Error::InconsistentInventory)?;
            if edge.coordinate.source != block.coordinate
                || edge.coordinate.successor as usize != occurrence
                || edge.target_id != target
                || edge.arguments != args
            {
                return Err(Error::InconsistentInventory);
            }
            let kind = match block.terminator {
                fe2o3_kernel_ir::Terminator::Branch { .. } => EdgeClass::Branch,
                fe2o3_kernel_ir::Terminator::ConditionalBranch { .. } => {
                    if occurrence == 0 {
                        EdgeClass::True
                    } else {
                        EdgeClass::False
                    }
                }
                fe2o3_kernel_ir::Terminator::Switch { cases, .. } => {
                    if occurrence < cases.len() {
                        EdgeClass::SwitchCase(occurrence)
                    } else {
                        EdgeClass::SwitchDefault
                    }
                }
                fe2o3_kernel_ir::Terminator::IntegerSwitch { cases, .. } => {
                    if occurrence < cases.len() {
                        EdgeClass::IntegerCase(occurrence)
                    } else {
                        EdgeClass::IntegerDefault
                    }
                }
                fe2o3_kernel_ir::Terminator::Return { .. }
                | fe2o3_kernel_ir::Terminator::Unreachable => {
                    return Err(Error::InconsistentInventory);
                }
            };
            reader.expect(
                row(
                    Subject::Edge(edge_index),
                    Role::Edge(kind),
                    Obligations::NONE.with(Obligation::Control),
                ),
                budget,
            )?;
            edge_index = add(edge_index, 1)?;
            occurrence = add(occurrence, 1)?;
            Ok::<_, Error>(())
        })?;
        if occurrence != block.edges.len() {
            return Err(Error::InconsistentInventory);
        }
    }
    if edge_index != inventory.edges().len() {
        return Err(Error::InconsistentInventory);
    }
    for (i, argument) in inventory.edge_arguments().iter().enumerate() {
        budget.charge_work(1)?;
        let definition = inventory
            .definitions()
            .get(argument.incoming_definition)
            .ok_or(Error::InconsistentInventory)?;
        if definition.value != Some(argument.value) {
            return Err(Error::InconsistentInventory);
        }
        reader.expect(
            row(
                Subject::EdgeArgument(i),
                Role::EdgeArgument,
                Obligations::NONE,
            ),
            budget,
        )?;
    }
    for (i, effect) in inventory.effects().iter().enumerate() {
        budget.charge_work(1)?;
        reader.expect(
            row(
                Subject::Effect(i),
                Role::Effect,
                effects::effect(effect.effect),
            ),
            budget,
        )?;
    }
    for (i, _) in inventory.calls().iter().enumerate() {
        reader.expect(row(Subject::Call(i), Role::Call, effects::call()), budget)?;
    }
    // Independent complete owner roster. Do not call the builder's requirement walk.
    for (ordinal, _) in inventory
        .owner()
        .module()
        .required_capabilities
        .iter()
        .enumerate()
    {
        reader.requirement(RequirementOwner::Module, ordinal, budget)?;
    }
    for (i, kernel) in inventory.kernels().iter().enumerate() {
        budget.charge_work(1)?;
        for (ordinal, _) in kernel.kernel.required_capabilities.iter().enumerate() {
            reader.requirement(RequirementOwner::Kernel(i), ordinal, budget)?;
        }
    }
    for (i, function) in inventory.functions().iter().enumerate() {
        budget.charge_work(1)?;
        for (ordinal, _) in function.function.required_capabilities.iter().enumerate() {
            reader.requirement(RequirementOwner::Function(i), ordinal, budget)?;
        }
    }
    for (i, op) in inventory.operations().iter().enumerate() {
        budget.charge_work(
            op.operation
                .required_capability_visitation_work_v1()
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut ordinal = 0usize;
        op.operation.try_visit_required_capabilities_v1(|_| {
            reader.requirement(RequirementOwner::Operation(i), ordinal, budget)?;
            ordinal = add(ordinal, 1)?;
            Ok::<_, Error>(())
        })?;
        budget.charge_work(1)?;
        if let fe2o3_kernel_ir::OperationKind::Atomic(atomic) = &op.operation.kind {
            let pointer = inventory
                .definition_for_value(op.coordinate.block.function, atomic.pointer, budget)
                .map_err(|error| match error {
                    crate::CanonicalKirInventoryErrorV1::Resource(error) => Error::Resource(error),
                    crate::CanonicalKirInventoryErrorV1::InconsistentOwner => {
                        Error::InconsistentInventory
                    }
                })?
                .ok_or(Error::InconsistentInventory)?;
            budget.charge_work(1)?;
            if fe2o3_kernel_ir::atomic_pointer_capability_v1(atomic, pointer.ty).is_some() {
                reader.requirement(RequirementOwner::AtomicPointer(i), 0, budget)?;
            }
        }
    }
    let mut previous = None;
    for (i, annotation) in metadata.rows.iter().enumerate() {
        budget.charge_work(2)?;
        let key = (annotation.subject, annotation.kind);
        if previous.is_some_and(|value| value >= key) {
            return Err(Error::MetadataOrder { ordinal: i });
        }
        previous = Some(key);
        if !subject_exists(inventory, annotation.subject) {
            return Err(Error::MetadataSubject { ordinal: i });
        }
        for (f, fact) in annotation.facts.iter().enumerate() {
            budget.charge_work(1)?;
            let valid = match fact {
                Fact::Definition(index) => *index < inventory.definitions().len(),
                Fact::Effect(index) => *index < inventory.effects().len(),
                Fact::Unsigned(_) | Fact::Signed(_) | Fact::Identity(_) => true,
            };
            if !valid {
                return Err(Error::MetadataFact { row: i, fact: f });
            }
        }
        reader.expect(
            row(
                Subject::Metadata(i),
                Role::Metadata,
                effects::metadata(annotation.kind),
            ),
            budget,
        )?;
    }
    budget.charge_work(1)?;
    if reader.next != candidate.rows.len() {
        return Err(Error::ExtraRows { first: reader.next });
    }
    Ok(())
}

fn subject_exists(inventory: &Inventory<'_>, subject: Subject) -> bool {
    match subject {
        Subject::Module => true,
        Subject::Kernel(i) => i < inventory.kernels().len(),
        Subject::Function(i) => i < inventory.functions().len(),
        Subject::Block(i) => i < inventory.blocks().len(),
        Subject::Definition(i) => i < inventory.definitions().len(),
        Subject::Operation(i) => i < inventory.operations().len(),
        Subject::Use(i) => i < inventory.uses().len(),
        Subject::Edge(i) => i < inventory.edges().len(),
        Subject::EdgeArgument(i) => i < inventory.edge_arguments().len(),
        Subject::Effect(i) => i < inventory.effects().len(),
        Subject::Call(i) => i < inventory.calls().len(),
        // Avoid self-referential annotation graphs and untyped capability claims.
        Subject::Requirement { .. } | Subject::Metadata(_) => false,
    }
}
