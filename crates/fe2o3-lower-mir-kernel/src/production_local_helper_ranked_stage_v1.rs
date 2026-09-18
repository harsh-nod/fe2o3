// This stage consumes original-N local-call evidence only. Later consumers of
// private effects remain closed; no generic receipt or semantic owner is made.

struct UnitLocalRankedEntryV1 {
    binding: Option<(usize, SemanticFunctionIdV1)>,
    calls: usize,
    kernel_seen: bool,
}

struct UnitLocalRankedRootRowV1 {
    calls: std::ops::Range<usize>,
    translation: ProductionMirPlironTranslationValidationV1,
}

/// Scoped original-N ranked-call coverage, not a generally admitted compiler
/// owner. Every local call retains its real private effects and the same-owner
/// source relation. This cannot unlock later source/output or formal admission.
pub struct ProductionUnitLocalRankedStageV1<'s> {
    owner: &'s ProductionPreRankedKirOwnerV1,
    source: &'s ProductionUnitLocalSourceV1<'s>,
    candidates: &'s [ProductionRankedSemanticProjectionRootV1],
    rows: &'s [UnitLocalRankedRootRowV1],
    calls: &'s [ProductionUnitLocalBoundsNeutralCallV1<'s>],
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}

/// One selected source root with entry-only translation and the exact local
/// calls omitted from the external ranked footprint. It is not a formal-memory
/// receipt and cannot be converted into a legacy ranked module receipt.
pub struct ProductionUnitLocalRankedRootV1<'s> {
    candidate: &'s ProductionRankedSemanticProjectionRootV1,
    row: &'s UnitLocalRankedRootRowV1,
    calls: &'s [ProductionUnitLocalBoundsNeutralCallV1<'s>],
}

impl ProductionUnitLocalRankedRootV1<'_> {
    /// Exact selected source root, not an independently admitted identifier.
    pub fn selected_root(&self) -> SemanticFunctionIdV1 {
        self.candidate.selected_root
    }

    /// Root entry effects only; helper-local accesses are not counted as empty.
    pub fn entry_translation(&self) -> &ProductionMirPlironTranslationValidationV1 {
        &self.row.translation
    }

    /// Each token remains root/caller/source-site/native-occurrence qualified.
    pub fn local_calls(&self) -> &[ProductionUnitLocalBoundsNeutralCallV1<'_>] {
        self.calls
    }
}

impl ProductionUnitLocalRankedStageV1<'_> {
    /// Number of checked roots in the complete ordered candidate roster.
    pub fn root_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of distinct checked local-call occurrences across all roots.
    pub fn local_call_count(&self) -> usize {
        self.calls.len()
    }

    /// Borrow one root under the original work ledger and live storage floor.
    /// This charges six work units, including out-of-range and refused queries.
    pub fn root(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionUnitLocalRankedRootV1<'_>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !self.source.inventory.belongs_to(self.owner.executable()) {
            return Err(unit_local_mismatch_v1());
        }
        let Some(row) = self.rows.get(ordinal) else {
            return Ok(None);
        };
        Ok(Some(ProductionUnitLocalRankedRootV1 {
            candidate: self
                .candidates
                .get(ordinal)
                .ok_or_else(unit_local_mismatch_v1)?,
            row,
            calls: self
                .calls
                .get(row.calls.clone())
                .ok_or_else(unit_local_mismatch_v1)?,
        }))
    }

    /// This intermediate stage grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn unit_ranked_root_index_v1(
    roots: &[ProductionRankedSemanticProjectionRootV1],
    root: SemanticFunctionIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    assert_origin_find_v1(roots, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.selected_root.cmp(&root))
    })
    .map_err(call_index_error_v1)?
    .ok_or_else(unit_local_mismatch_v1)
}

impl ProductionPreRankedKirOwnerV1 {
    /// Checks one complete ranked roster against this original materialization,
    /// retaining the sealed local-source relation and exact call tokens in scope.
    /// Existing raw-empty receipt/attachment APIs and all later private-effect
    /// consumer fences remain unchanged.
    ///
    /// Caller keeps this owner's full unit-local source floor reserved. Ranked
    /// graphs, source replay, and legacy entry-translation engines retain their
    /// existing independent limits; their allocations are not charged to this
    /// canonical ledger. New inventory, call indexes, actual-capacity coverage
    /// buffers and scope headers are charged here. No executable graph is copied.
    /// Callback outputs must be pre-reserved; callback scratch must be dropped
    /// and released before return. Result/error/unwind restore the incoming
    /// floor under the original ledger; accounting violations fail closed.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionPreRankedKirOwnerV1, ProductionRankedSemanticProjectionRootV1};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1,
    ///     roots: &[ProductionRankedSemanticProjectionRootV1], budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     owner.with_checked_unit_local_ranked_stage_v1(roots, budget, |stage, budget| {
    ///         saved = stage.root(0, budget)?;
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_unit_local_ranked_stage_v1<'w, R>(
        &self,
        roots: &[ProductionRankedSemanticProjectionRootV1],
        budget: &mut ArgumentBudgetV1<'w>,
        use_stage: impl for<'s> FnOnce(
            &ProductionUnitLocalRankedStageV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(argument_sum_v1(&[5, roots.len()])?)?;
            if self.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::UnitLocal {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "ranked local-call stage requires retained Unit-local source evidence",
                ));
            }
            if budget.storage() < self.unit_local_source_storage_floor_v1()? {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            validate_source_ranked_roster_v1(&self.semantic_ssa, &self.source_launch, roots)?;
            let (inventory, storage) = CanonicalKirInventoryV1::derive(self.executable(), budget)
                .map_err(canonical_call_inventory_error_v1)?;
            budget.reserve_storage(storage.retained_storage())?;
            with_unit_local_ranked_stage_inventory_v1(self, roots, &inventory, budget, use_stage)
        })
    }
}

fn with_unit_local_ranked_stage_inventory_v1<'w, R>(
    owner: &ProductionPreRankedKirOwnerV1,
    roots: &[ProductionRankedSemanticProjectionRootV1],
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    use_stage: impl for<'s> FnOnce(
        &ProductionUnitLocalRankedStageV1<'s>,
        &mut ArgumentBudgetV1<'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    owner.with_checked_canonical_calls_v1(inventory, budget, |canonical, budget| {
        owner.with_checked_unit_local_source_v1(inventory, budget, |source, budget| {
            with_canonical_call_scratch_v1(budget, |budget| {
                budget.charge_work(8)?;
                if !source.belongs_to(inventory)
                    || !canonical.belongs_to(inventory)
                    || source.rows.associations.is_empty()
                    || roots.len() != inventory.kernels().len()
                {
                    return Err(unit_local_mismatch_v1());
                }
                budget.reserve_storage(argument_sum_v1(&[
                    std::mem::size_of::<ProductionUnitLocalRankedStageV1<'_>>(),
                    std::mem::size_of::<Vec<UnitLocalRankedEntryV1>>(),
                    std::mem::size_of::<Vec<usize>>(),
                    std::mem::size_of::<Vec<Option<usize>>>(),
                    std::mem::size_of::<Vec<bool>>(),
                    std::mem::size_of::<Vec<ProductionUnitLocalBoundsNeutralCallV1<'_>>>(),
                    std::mem::size_of::<Vec<UnitLocalRankedRootRowV1>>(),
                ])?)?;
                let mut entries = unit_local_vec_v1(roots.len(), budget)?;
                for _ in roots {
                    unit_local_push_v1(
                        &mut entries,
                        UnitLocalRankedEntryV1 {
                            binding: None,
                            calls: 0,
                            kernel_seen: false,
                        },
                        budget,
                    )?;
                }
                let mut entry_roots = unit_local_vec_v1(inventory.functions().len(), budget)?;
                budget.charge_work(inventory.functions().len())?;
                entry_roots.resize(inventory.functions().len(), usize::MAX);
                let mut associations = unit_local_vec_v1(source.rows.associations.len(), budget)?;
                budget.charge_work(source.rows.associations.len())?;
                associations.resize(source.rows.associations.len(), None::<usize>);
                let mut seen = unit_local_vec_v1(source.rows.calls.len(), budget)?;
                budget.charge_work(source.rows.calls.len())?;
                seen.resize(source.rows.calls.len(), false);
                let mut tokens = unit_local_vec_v1(source.rows.calls.len(), budget)?;
                let mut rows = unit_local_vec_v1(roots.len(), budget)?;

                // The canonical scope independently joined all actual ordinary
                // call occurrences and root-qualified function associations.
                for group in &canonical.groups {
                    budget.charge_work(7)?;
                    let record = group.function.source;
                    let physical = group.function.canonical.coordinate.0 as usize;
                    let root =
                        unit_ranked_root_index_v1(roots, record.correspondence_owner, budget)?;
                    match record.role {
                        SemanticKirFunctionRoleV1::KernelEntry => {
                            let slot = entry_roots
                                .get_mut(physical)
                                .ok_or_else(unit_local_mismatch_v1)?;
                            if *slot != usize::MAX || entries[root].binding.is_some() {
                                return Err(unit_local_mismatch_v1());
                            }
                            *slot = root;
                            entries[root].binding = Some((physical, record.semantic_function));
                        }
                        SemanticKirFunctionRoleV1::InternalHelper => {
                            match owner.helper_memory.functions.get(physical) {
                                Some(RetainedHelperKindV1::RawEmpty) => {}
                                Some(RetainedHelperKindV1::Local { .. }) => {
                                    let index = source
                                        .rows
                                        .find_association(
                                            UnitLocalAssociationKeyV1 {
                                                root: record.correspondence_owner,
                                                function: record.semantic_function,
                                                physical,
                                            },
                                            budget,
                                        )?
                                        .ok_or_else(unit_local_mismatch_v1)?;
                                    if associations[index].replace(0).is_some() {
                                        return Err(unit_local_mismatch_v1());
                                    }
                                }
                                _ => return Err(unit_local_mismatch_v1()),
                            }
                        }
                    }
                }
                for kernel in inventory.kernels() {
                    budget.charge_work(5)?;
                    let root = *entry_roots
                        .get(kernel.entry.0 as usize)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let entry = entries.get_mut(root).ok_or_else(unit_local_mismatch_v1)?;
                    let candidate = roots.get(root).ok_or_else(unit_local_mismatch_v1)?;
                    budget.charge_work(argument_sum_v1(&[
                        kernel.kernel.id.as_str().len(),
                        candidate.function_name().len(),
                    ])?)?;
                    if entry.kernel_seen || kernel.kernel.id.as_str() != candidate.function_name() {
                        return Err(unit_local_mismatch_v1());
                    }
                    entry.kernel_seen = true;
                }

                for binding in &canonical.calls {
                    budget.charge_work(12)?;
                    let callee = canonical
                        .groups
                        .get(binding.callee)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let physical = callee.function.canonical.coordinate.0 as usize;
                    if matches!(
                        owner.helper_memory.functions.get(physical),
                        Some(RetainedHelperKindV1::RawEmpty)
                    ) {
                        continue;
                    }
                    if !matches!(
                        owner.helper_memory.functions.get(physical),
                        Some(RetainedHelperKindV1::Local { .. })
                    ) {
                        return Err(unit_local_mismatch_v1());
                    }
                    let site = binding.site;
                    let root =
                        unit_ranked_root_index_v1(roots, site.caller.correspondence_owner, budget)?;
                    let (entry, caller) =
                        entries[root].binding.ok_or_else(unit_local_mismatch_v1)?;
                    let native = inventory
                        .calls()
                        .get(binding.call)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    if site.caller.role != SemanticKirFunctionRoleV1::KernelEntry
                        || site.caller.semantic_function != caller
                        || native.coordinate.block.function.0 as usize != entry
                        || native.target.map(|target| target.0 as usize) != Some(physical)
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    let token = source
                        .bounds_neutral_call_v1(
                            site.caller.correspondence_owner,
                            caller,
                            site.anchor.semantic_block,
                            site.source,
                            budget,
                        )?
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let key = [
                        site.caller.correspondence_owner.index(),
                        caller.index(),
                        site.anchor.semantic_block.index(),
                    ];
                    let index = assert_origin_find_v1(&source.rows.calls, budget, |row, budget| {
                        budget.charge_work(3)?;
                        Ok(row.source_site_key().cmp(&key))
                    })
                    .map_err(call_index_error_v1)?
                    .ok_or_else(unit_local_mismatch_v1)?;
                    budget.charge_work(10)?;
                    let association = token.row.callee_association;
                    if seen[index]
                        || !std::ptr::eq(token.row, &source.rows.calls[index])
                        || !std::ptr::eq(token.source_call(), site.source)
                        || !std::ptr::eq(token.operation(), native.operation)
                        || token.native_call() != native.coordinate
                        || token.association.key.root != site.caller.correspondence_owner
                        || token.association.key.function
                            != callee.function.source.semantic_function
                        || token.association.key.physical != physical
                        || !std::ptr::eq(
                            token.association,
                            source
                                .rows
                                .associations
                                .get(association)
                                .ok_or_else(unit_local_mismatch_v1)?,
                        )
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    let count = associations
                        .get_mut(association)
                        .ok_or_else(unit_local_mismatch_v1)?
                        .as_mut()
                        .ok_or_else(unit_local_mismatch_v1)?;
                    *count = argument_sum_v1(&[*count, 1])?;
                    entries[root].calls = argument_sum_v1(&[entries[root].calls, 1])?;
                    seen[index] = true;
                    unit_local_push_v1(&mut tokens, token, budget)?;
                }
                budget.charge_work(argument_sum_v1(&[
                    seen.len(),
                    associations.len(),
                    entries.len(),
                ])?)?;
                if seen.iter().any(|seen| !seen)
                    || entries
                        .iter()
                        .any(|entry| entry.binding.is_none() || !entry.kernel_seen)
                    || associations
                        .iter()
                        .zip(&source.rows.associations)
                        .any(|(count, row)| *count != Some(row.call_count))
                {
                    return Err(unit_local_mismatch_v1());
                }

                let mut first = 0;
                for (index, candidate) in roots.iter().enumerate() {
                    budget.charge_work(5)?;
                    let end = argument_sum_v1(&[first, entries[index].calls])?;
                    let call_slice = tokens.get(first..end).ok_or_else(unit_local_mismatch_v1)?;
                    budget.charge_work(call_slice.len())?;
                    if call_slice
                        .iter()
                        .any(|call| call.root() != candidate.selected_root)
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    let translation = validate_mir_pliron_translation_with_semantic_and_budget_v1(
                        Some(owner.semantic_ssa.source_semantic()),
                        owner.executable.module(),
                        &owner.correspondence,
                        candidate.function_name(),
                        &candidate.lowering,
                        &candidate.access_sources,
                        &candidate.executable_effect_sources,
                        owner.limits.max_operations,
                        budget,
                    )
                    .map_err(ProductionSemanticKirErrorV1::MirPlironTranslation)?;
                    unit_local_push_v1(
                        &mut rows,
                        UnitLocalRankedRootRowV1 {
                            calls: first..end,
                            translation,
                        },
                        budget,
                    )?;
                    first = end;
                }
                budget.charge_work(1)?;
                if first != tokens.len() {
                    return Err(unit_local_mismatch_v1());
                }
                let stage = ProductionUnitLocalRankedStageV1 {
                    owner,
                    source,
                    candidates: roots,
                    rows: &rows,
                    calls: &tokens,
                    ledger: budget as *const ArgumentBudgetV1<'_> as usize,
                    work_ledger: budget.work_ledger_identity_v1(),
                    floor: budget.storage(),
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    use_stage(&stage, budget)
                }));
                if stage.work_ledger != budget.work_ledger_identity_v1()
                    || stage.floor != budget.storage()
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                match result {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        })
    })
}
