// The physical payload is usable only inside the exact checked source owner.
// It is not an address, allocation grant, or generic pointer representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceBindingOriginV29 {
    SingleLoan(usize),
    EnumView(usize),
}

impl SourceReferenceBindingOriginV29 {
    fn single_loan(self) -> Result<usize, ProductionSemanticKirErrorV1> {
        match self {
            Self::SingleLoan(loan) => Ok(loan),
            Self::EnumView(_) => Err(source_reference_error_v29(
                "correlated reference choice requires per-alternative checked storage effects",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticSourceReferenceBindingV29 {
    owner: usize,
    source: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: ProductionCallInstanceIdV1,
    origin: SourceReferenceBindingOriginV29,
    source_type: SemanticTypeIdV1,
    values: Vec<ValueDef>,
}

struct SourceReferenceEmissionV29<'a, 'source> {
    plan: &'a SourceReferencePlanV29<'a, 'source>,
    claimed: Vec<std::cell::Cell<bool>>,
    sites: Vec<((usize, u32, usize), usize)>,
    block_sites: Vec<((usize, u32, usize), usize)>,
    cell_accesses: Vec<std::cell::Cell<Option<SourceReferenceCellUseV29>>>,
    cell_failure_reads: Vec<std::cell::Cell<Option<ScopedMemoryAnchorV29>>>,
    selectors: Vec<std::cell::Cell<Option<SourceReferenceSelectorUseV29>>>,
    descriptors: Vec<std::cell::Cell<Option<SourceReferenceDescriptorUseV29>>>,
    descriptor_guards: Vec<std::cell::Cell<Option<SourceReferenceDescriptorGuardUseV29>>>,
    raw_formations: Vec<std::cell::Cell<Option<SourceRawFormationReceiptV29>>>,
    floor: usize,
    owned: usize,
}

fn source_reference_record_failure_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    error: &ProductionSemanticKirErrorV1,
) {
    if let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) = error {
        plan.failure.record_resource(*error);
    }
}

fn source_reference_owned_vec_v29<T>(
    plan: &SourceReferencePlanV29<'_, '_>,
    capacity: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Vec<T>>(plan, budget)?;
    source_reference_emission_vec_v29(capacity, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_owned_push_v29<T>(
    plan: &SourceReferencePlanV29<'_, '_>,
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_emission_push_v29(rows, row, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_owned_prepay_v29<T>(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_emission_prepay_v29::<T>(budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_emission_headers_v29<T>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<T>(),
        argument_product_v1(
            2,
            std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
        )?,
    ])
}

// Keep owned locals and both constructor/caller return envelopes paid until the
// surrounding emission scope has destroyed them. This does not assume slot reuse.
fn source_reference_emission_prepay_v29<T>(
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.reserve_storage(source_reference_emission_headers_v29::<T>()?)
}

fn source_reference_emission_vec_v29<T>(
    capacity: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Vec<T>>(budget)?;
    emission_vec_v1(capacity, budget)
}

fn source_reference_emission_push_v29<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let headers = if rows.len() == rows.capacity() {
        source_reference_emission_headers_v29::<Vec<T>>()?
    } else {
        0
    };
    // emission_push -> emission_vec is non-reentrant. The original Vec stays
    // live while its one replacement Vec and return envelopes are constructed.
    budget.reserve_storage(headers)?;
    emission_push_v1(rows, row, budget)?;
    budget.release_storage(headers)
}

fn source_reference_optional_emission_v29<'a, 'source>(
    plan: Option<&'a SourceReferencePlanV29<'a, 'source>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceReferenceEmissionV29<'a, 'source>>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<SourceReferenceEmissionV29<'_, '_>>>(budget)?;
    match plan {
        Some(plan) => Ok(Some(SourceReferenceEmissionV29::new(plan, budget)?)),
        None => Ok(None),
    }
}

fn with_optional_source_reference_plan_v29<'work, R>(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        Option<&SourceReferencePlanV29<'_, '_>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<R>(budget)?;
    if source_reference_borrows_present_v29(instances, budget)? {
        with_source_reference_plan_v29(instances, budget, |plan, budget| {
            consume(Some(plan), budget)
        })
    } else {
        consume(None, budget)
    }
}

impl<'a, 'source> SourceReferenceEmissionV29<'a, 'source> {
    fn new(
        plan: &'a SourceReferencePlanV29<'a, 'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        plan.check_owner(plan.instances, budget)?;
        // Pay once before consumers run; abort_scope consumes this owner and
        // cannot request another source traversal after their work is exhausted.
        plan.charge(5, budget)?;
        let before = budget.storage();
        source_reference_owned_prepay_v29::<Self>(plan, budget)?;
        // abort_scope returns after destroying Self. Keep its two result
        // envelopes in the caller scope, excluded from the backing refund.
        let abort_headers = source_reference_emission_headers_v29::<()>()?;
        source_reference_emission_prepay_v29::<()>(budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
        let mut claimed = source_reference_owned_vec_v29(plan, plan.loans.len(), budget)?;
        budget.source_reference_charge_v29(plan, plan.loans.len())?;
        claimed.resize_with(plan.loans.len(), || std::cell::Cell::new(false));
        let mut sites = source_reference_owned_vec_v29(plan, plan.loans.len(), budget)?;
        for (index, loan) in plan.loans.iter().enumerate() {
            budget.source_reference_charge_v29(plan, 4)?;
            sites.push((source_reference_site_key_v29(loan.site)?, index));
        }
        source_reference_sort_sites_v29(plan, &mut sites, budget)?;
        let mut block_sites = source_reference_owned_vec_v29(plan, plan.blocks.len(), budget)?;
        for (index, block) in plan.blocks.iter().enumerate() {
            budget.source_reference_charge_v29(plan, 4)?;
            block_sites.push(((block.instance.index(), block.block.index(), 0), index));
        }
        source_reference_sort_sites_v29(plan, &mut block_sites, budget)?;
        for rows in [&sites, &block_sites] {
            for pair in rows.windows(2) {
                budget.source_reference_charge_v29(plan, 4)?;
                if pair[0].0 == pair[1].0 {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
            }
        }
        let mut cell_accesses = source_reference_owned_vec_v29(plan, plan.accesses.len(), budget)?;
        budget.source_reference_charge_v29(plan, plan.accesses.len())?;
        cell_accesses.resize_with(plan.accesses.len(), || std::cell::Cell::new(None));
        let mut cell_failure_reads =
            source_reference_owned_vec_v29(plan, plan.accesses.len(), budget)?;
        budget.source_reference_charge_v29(plan, plan.accesses.len())?;
        cell_failure_reads.resize_with(plan.accesses.len(), || std::cell::Cell::new(None));
        let mut selectors = Vec::new();
        if !plan.selectors.is_empty() {
            selectors = source_reference_owned_vec_v29(plan, plan.selectors.len(), budget)?;
            budget.source_reference_charge_v29(plan, plan.selectors.len())?;
            selectors.resize_with(plan.selectors.len(), || std::cell::Cell::new(None));
        }
        let mut descriptors = Vec::new();
        let mut descriptor_guards = Vec::new();
        if !plan.descriptors.is_empty() {
            descriptors = source_reference_owned_vec_v29(plan, plan.descriptors.len(), budget)?;
            budget.source_reference_charge_v29(plan, plan.descriptors.len())?;
            descriptors.resize_with(plan.descriptors.len(), || std::cell::Cell::new(None));
            descriptor_guards =
                source_reference_owned_vec_v29(plan, plan.descriptor_guards.len(), budget)?;
            budget.source_reference_charge_v29(plan, plan.descriptor_guards.len())?;
            descriptor_guards
                .resize_with(plan.descriptor_guards.len(), || std::cell::Cell::new(None));
        }
        let mut raw_formations = Vec::new();
        if !plan.raw_origins.is_empty() {
            raw_formations = source_reference_owned_vec_v29(plan, plan.raw_origins.len(), budget)?;
            budget.source_reference_charge_v29(plan, plan.raw_origins.len())?;
            raw_formations.resize_with(plan.raw_origins.len(), || std::cell::Cell::new(None));
        }
        let floor = budget.storage();
        Ok(Self {
            plan,
            claimed,
            sites,
            block_sites,
            cell_accesses,
            cell_failure_reads,
            selectors,
            descriptors,
            descriptor_guards,
            raw_formations,
            floor,
            owned: floor
                .checked_sub(before)
                .and_then(|owned| owned.checked_sub(abort_headers))
                .ok_or(ArgumentResourceV1::Accounting)?,
        })
    }

    fn check(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self.plan)?;
        if budget.storage() < self.floor {
            if let Some(root) = &self.plan.storage_root {
                root.deny_active_root_refund();
            }
            self.plan
                .failure
                .record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn abort_scope(
        self,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let plan = self.plan;
        let owned = self.owned;
        // Even a sticky earlier failure cannot authorize refunds from a foreign
        // slot/ledger or an undercut floor. No work or scratch is requested here.
        let retained = plan.retains_custody(instances, budget)
            && budget.storage() >= self.floor
            && plan
                .storage_root
                .as_ref()
                .is_none_or(|arena| arena.retains_after_refund(owned, budget))
            && self
                .floor
                .checked_sub(owned)
                .is_some_and(|before| before >= plan.retained_floor);
        if !retained {
            if let Some(root) = &plan.storage_root {
                root.deny_active_root_refund();
            }
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
        }
        drop(self);
        if retained && let Err(error) = budget.release_storage(owned) {
            plan.failure.record_resource(error);
        }
        match plan.failure.first_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn claim(
        &self,
        site: SourceReferenceSiteV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let index = self.loan_at(site, budget)?.ok_or_else(|| {
            source_reference_error_v29("source reference borrow is absent from its owner")
        })?;
        if self.claimed[index].replace(true) {
            return Err(source_reference_error_v29(
                "source reference borrow was emitted twice",
            ));
        }
        Ok(index)
    }

    fn loan_at(
        &self,
        site: SourceReferenceSiteV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let key = source_reference_site_key_v29(site)?;
        let index = source_reference_lookup_site_v29(self.plan, &self.sites, key, budget)?;
        if let Some(index) = index {
            if self.plan.loans.get(index).map(|loan| loan.site) != Some(site) {
                return Err(ArgumentResourceV1::Accounting.into());
            }
        }
        Ok(index)
    }

    fn block_node(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let index = source_reference_lookup_site_v29(
            self.plan,
            &self.block_sites,
            (instance.index(), block.index(), 0),
            budget,
        )?
        .ok_or_else(execution_cfg_error_v29)?;
        budget.source_reference_charge_v29(self.plan, 4)?;
        let row = self
            .plan
            .blocks
            .get(index)
            .ok_or_else(execution_cfg_error_v29)?;
        if row.instance != instance || row.block != block {
            return Err(execution_cfg_error_v29());
        }
        Ok(self
            .plan
            .states
            .get(row.entry)
            .and_then(|state| state.get(local.index() as usize))
            .ok_or_else(execution_cfg_error_v29)?
            .node)
    }

    fn finish(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        if !self.plan.raw_origins.is_empty() || self.plan.address_observed {
            return Err(source_reference_error_v29(
                "source raw address requires checked physical formation and stored-pointer correspondence",
            ));
        }
        scoped_raw_admission_v29::require_original_zero_raw_v29(
            self.plan.instances,
            Some(self.plan),
            budget,
        )?;
        self.finish_source_claims_v29(budget)
    }

    // The ordinary finish above retains the original-zero-raw gate. The private
    // expanded-root transaction calls this only after its physical census.
    fn finish_source_claims_v29(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.source_reference_charge_v29(self.plan, self.claimed.len())?;
        if self.claimed.iter().any(|claimed| !claimed.get()) {
            return Err(source_reference_error_v29(
                "source reference borrow was not emitted",
            ));
        }
        budget.source_reference_charge_v29(self.plan, self.selectors.len())?;
        if self.selectors.iter().any(|claim| {
            claim.get().is_none_or(|claim| {
                matches!(claim.producer, SourceReferenceSelectorProducerV29::Pending)
            })
        }) {
            return Err(source_reference_error_v29(
                "source selector has no exact emitted producer",
            ));
        }
        budget.source_reference_charge_v29(self.plan, self.descriptors.len())?;
        if self.descriptors.iter().any(|claim| {
            claim.get().is_none_or(|claim| {
                !matches!(
                    claim.producer,
                    SourceReferenceSelectorProducerV29::Address { .. }
                )
            })
        }) {
            return Err(source_descriptor_error_v29());
        }
        Ok(())
    }
}

include!("production_source_reference_selector_emission_v29.rs");

fn source_reference_lookup_site_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    rows: &[((usize, u32, usize), usize)],
    key: (usize, u32, usize),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    let (mut first, mut end) = (0, rows.len());
    while first < end {
        budget.source_reference_charge_v29(plan, 8)?;
        let middle = first + (end - first) / 2;
        let (candidate, index) = rows[middle];
        match candidate.cmp(&key) {
            std::cmp::Ordering::Less => first = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Some(index)),
        }
    }
    Ok(None)
}

fn source_reference_site_key_v29(
    site: SourceReferenceSiteV29,
) -> Result<(usize, u32, usize), ProductionSemanticKirErrorV1> {
    Ok((
        site.instance.index(),
        site.block.index(),
        site.statement.ok_or_else(|| {
            source_reference_error_v29("source reference borrow lacks a statement")
        })?,
    ))
}

fn source_reference_sort_sites_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    sites: &mut [((usize, u32, usize), usize)],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    fn sift(
        plan: &SourceReferencePlanV29<'_, '_>,
        rows: &mut [((usize, u32, usize), usize)],
        mut root: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        loop {
            budget.source_reference_charge_v29(plan, 8)?;
            let left = argument_sum_v1(&[argument_product_v1(root, 2)?, 1])?;
            if left >= rows.len() {
                return Ok(());
            }
            let right = argument_sum_v1(&[left, 1])?;
            let child = if right < rows.len() && rows[right].0 > rows[left].0 {
                right
            } else {
                left
            };
            if rows[root].0 >= rows[child].0 {
                return Ok(());
            }
            rows.swap(root, child);
            root = child;
        }
    }
    for root in (0..sites.len() / 2).rev() {
        sift(plan, sites, root, budget)?;
    }
    for end in (1..sites.len()).rev() {
        budget.source_reference_charge_v29(plan, 1)?;
        sites.swap(0, end);
        sift(plan, &mut sites[..end], 0, budget)?;
    }
    Ok(())
}

fn source_reference_entry_node_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    block: Option<SemanticBlockIdV1>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let state = if let Some(block) = block {
        budget.source_reference_charge_v29(plan, plan.blocks.len())?;
        plan.blocks
            .iter()
            .find(|row| row.instance == instance && row.block == block)
            .map(|row| row.entry)
    } else {
        budget.source_reference_charge_v29(plan, 1)?;
        plan.entries.get(instance.index()).copied().flatten()
    }
    .ok_or_else(|| source_reference_error_v29("source reference entry state is missing"))?;
    budget.source_reference_charge_v29(plan, 2)?;
    Ok(plan
        .states
        .get(state)
        .and_then(|state| state.get(local.index() as usize))
        .ok_or_else(|| source_reference_error_v29("source reference entry local is missing"))?
        .node)
}

fn source_reference_node_has_loan_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let mut pending = source_reference_owned_vec_v29(plan, 1, budget)?;
    pending.push(node);
    while let Some(index) = pending.pop() {
        budget.source_reference_charge_v29(plan, 1)?;
        match plan
            .nodes
            .get(index)
            .ok_or_else(|| {
                source_reference_error_v29("source reference node is outside its owner")
            })?
            .kind
        {
            SourceReferenceNodeKindV29::Loan(_) => return Ok(true),
            SourceReferenceNodeKindV29::Plain(_)
            | SourceReferenceNodeKindV29::Absent
            | SourceReferenceNodeKindV29::Discriminant(_)
            | SourceReferenceNodeKindV29::Address(_) => {}
            SourceReferenceNodeKindV29::Enum { first, count } => {
                for offset in 0..count {
                    budget.source_reference_charge_v29(plan, 3)?;
                    let member = *plan.enum_members.get(argument_sum_v1(&[first, offset])?)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    let alternative = plan.enum_alternatives.get(member)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    for field in 0..alternative.count {
                        budget.source_reference_charge_v29(plan, 1)?;
                        let child = *plan.children.get(argument_sum_v1(&[alternative.first, field])?)
                            .ok_or_else(source_reference_enum_error_v29)?;
                        if child >= index { return Err(source_reference_enum_error_v29()); }
                        source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                    }
                }
            }
            SourceReferenceNodeKindV29::EnumView(view) => {
                budget.source_reference_charge_v29(plan, 2)?;
                let view = plan.enum_views.get(view).ok_or_else(source_reference_enum_error_v29)?;
                if view.source >= index { return Err(source_reference_enum_error_v29()); }
                for field in 0..view.child_count {
                    budget.source_reference_charge_v29(plan, 1)?;
                    let child = *plan.children.get(argument_sum_v1(&[view.children, field])?)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    if child >= index { return Err(source_reference_enum_error_v29()); }
                    source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                }
            }
            SourceReferenceNodeKindV29::Aggregate { first, count } => {
                for offset in 0..count {
                    budget.source_reference_charge_v29(plan, 1)?;
                    let child = *plan
                        .children
                        .get(argument_sum_v1(&[first, offset])?)
                        .ok_or_else(|| {
                            source_reference_error_v29("source reference child is missing")
                        })?;
                    if child >= index {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                }
            }
        }
    }
    Ok(false)
}

fn source_reference_payload_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 1)?;
    if matches!(plan.cells.strategies.get(loan), Some(SourceReferenceCellStrategyV29::Object(_))) {
        let pointer = budget.source_reference_object_pointer_type_v29(plan, loan)?;
        let mut types = source_reference_owned_vec_v29(plan, 1, budget)?;
        types.push(pointer);
        return Ok(types);
    }
    if let Some((_, cell)) = budget.source_reference_scalar_cell_v29(plan, loan)? {
        let pointer = source_reference_cell_pointer_type_v29(plan, loan, cell, budget)?;
        let mut types = source_reference_owned_vec_v29(plan, 1, budget)?;
        types.push(pointer);
        return Ok(types);
    }
    let representation = budget.source_reference_representation_v29(plan, loan)?;
    source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 3)?;
    let record = plan
        .loans
        .get(loan)
        .ok_or_else(|| source_reference_error_v29("source reference loan is outside its owner"))?;
    let origin = &plan.origins[record.origin];
    let source = plan.instances.owner().source_semantic();
    match representation {
        SourceReferenceRepresentationV29::StableReferent => {
            execution_cfg_types_v29(source.types(), origin.ty, budget)
                .inspect_err(|error| source_reference_record_failure_v29(plan, error))
        }
        SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor) => {
            let root = plan
                .instances
                .instance(plan.root)
                .ok_or_else(execution_call_error_v29)?;
            let fields = match source.types()[anchor.ty.index() as usize].shape() {
                SemanticTypeShapeV1::Aggregate(fields) => fields.fields().len(),
                _ => {
                    return Err(source_reference_error_v29(
                        "source reference allocation carrier differs",
                    ));
                }
            };
            budget.source_reference_charge_v29(
                plan,
                argument_sum_v1(&[
                    argument_product_v1(source.callables().len(), 4)?,
                    argument_product_v1(fields, 20)?,
                    32,
                ])?,
            )?;
            // The authenticated descriptor returns exactly one scalar Slice box.
            budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
            let ty = authenticated_disjoint_slice_parameter(
                source.types(),
                source.callables(),
                root.declaration(),
                anchor.argument,
                anchor.ty,
            )
            .ok_or_else(|| {
                source_reference_error_v29("source reference allocation owner differs")
            })?;
            let mut types = source_reference_owned_vec_v29(plan, 1, budget)?;
            types.push(ty);
            Ok(types)
        }
        SourceReferenceRepresentationV29::NeedsAddressable(_) => Err(source_reference_error_v29(
            "source reference requires checked addressable storage and writeback",
        )),
    }
}

fn source_reference_validate_binding_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 8)?;
    if binding.owner != plan as *const SourceReferencePlanV29<'_, '_> as usize
        || binding.source != plan.source
        || binding.ssa != plan.ssa
        || binding.root != plan.root
        || matches!(binding.origin, SourceReferenceBindingOriginV29::SingleLoan(loan)
            if plan.loans.get(loan).map(|loan| loan.source_type) != Some(binding.source_type))
    {
        return Err(source_reference_error_v29(
            "source reference binding belongs to another owner",
        ));
    }
    let expected = match binding.origin {
        SourceReferenceBindingOriginV29::SingleLoan(loan) => source_reference_payload_types_v29(plan, loan, budget)?,
        SourceReferenceBindingOriginV29::EnumView(_) => source_reference_binding_origin_types_v29(
            plan, binding.origin, binding.source_type, &mut 0, budget)?,
    };
    budget.source_reference_charge_v29(plan, expected.len())?;
    if binding.values.len() != expected.len()
        || binding
            .values
            .iter()
            .zip(&expected)
            .any(|(value, ty)| &value.ty != ty)
    {
        return Err(source_reference_error_v29(
            "source reference physical payload differs",
        ));
    }
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn lower_allocation_carrier_receiver_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        receiver: usize,
        operations: &mut Vec<Operation>,
    ) -> Result<(ValueId, Type), ProductionSemanticKirErrorV1> {
        if self.execution.as_ref().and_then(|cursor| cursor.references).is_none() {
            return self
                .lower_operand(block, None, &call.arguments()[receiver], operations)?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail));
        }
        self.with_scoped_payload_header_v29(
            argument_sum_v1(&[
                std::mem::size_of::<(SemanticTypeIdV1, bool)>(),
                std::mem::size_of::<Result<(SemanticTypeIdV1, bool), ProductionSemanticKirErrorV1>>(),
                std::mem::size_of::<SemanticValueBindingV1>(),
                std::mem::size_of::<Option<(ValueId, Type)>>(),
            ])?,
            |this| {
                let (carrier, writes) = this.with_emission_budget_v1(|this, budget| {
                    let cursor = this.execution.as_ref().ok_or_else(allocation_receiver_error_v29)?;
                    let references = cursor.references.ok_or_else(allocation_receiver_error_v29)?;
                    references.check(budget)?;
                    budget.source_reference_charge_v29(references.plan, 4)?;
                    let original = references.plan.instances.instance(cursor.instance)
                        .ok_or_else(allocation_receiver_error_v29)?;
                    let source = references.plan.instances.owner().source_semantic();
                    if !std::ptr::eq(original.declaration(), this.function)
                        || !std::ptr::eq(source.types(), this.types)
                        || !std::ptr::eq(source.callables(), this.callables)
                    {
                        return Err(allocation_receiver_error_v29());
                    }
                    allocation_receiver_contract_v29(
                        this.function,
                        this.callables,
                        block,
                        call,
                        receiver,
                        budget,
                    )
                })?;
                let operand = &call.arguments()[receiver];
                let role = ExecutionOperandV29::CallArgument(
                    u32::try_from(receiver).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let value = this.lower_source_operand_v29(
                    block, None, Some(role), operand, operations,
                )?;
                match value {
                    SemanticValueBindingV1::SourceReference(mut binding) => {
                        this.with_emission_budget_v1(|this, budget| {
                            let references = this.execution.as_ref()
                                .and_then(|cursor| cursor.references)
                                .ok_or_else(allocation_receiver_error_v29)?;
                            references.check(budget)?;
                            source_reference_allocation_value_v29(
                                references.plan, this.types, &mut binding,
                                operand.ty(), carrier, writes, budget,
                            )?.ok_or_else(allocation_receiver_error_v29)
                        })
                    }
                    SemanticValueBindingV1::Value { id, ty: Type::Slice(slice) } => {
                        Ok((id, Type::Slice(slice)))
                    }
                    _ => Err(allocation_receiver_error_v29()),
                }
            },
        )
    }

    fn try_lower_source_reference_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let SemanticRvalueKindV1::Borrow { kind, place } = value else {
            return Ok(None);
        };
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(None);
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block,
            statement: statement.map(|x| x as usize),
        };
        let loan = self.with_emission_budget_v1(|this, budget| {
            references.check(budget)?;
            let Some(index) = references.loan_at(site, budget)? else {
                return Ok(None);
            };
            let loan = &references.plan.loans[index];
            let declaration = references
                .plan
                .instances
                .instance(instance)
                .ok_or_else(execution_call_error_v29)?
                .declaration();
            if !std::ptr::eq(declaration, this.function)
                || loan.kind != *kind
                || loan.source_type != result_type
            {
                return Err(source_reference_error_v29(
                    "source reference borrow source differs",
                ));
            }
            let original = site
                .statement
                .and_then(|statement| {
                    declaration
                        .blocks()
                        .get(block.index() as usize)
                        .and_then(|block| block.statements().get(statement))
                })
                .ok_or_else(|| {
                    source_reference_error_v29("source reference borrow statement is missing")
                })?;
            if !matches!(original.kind(), SemanticStatementKindV1::Assign(assignment)
                if std::ptr::eq(assignment.value().kind(), value))
            {
                return Err(source_reference_error_v29(
                    "source reference borrow occurrence differs",
                ));
            }
            if budget
                .source_reference_scalar_cell_v29(references.plan, index)?
                .is_none()
            {
                budget.source_reference_representation_v29(references.plan, index)?;
            }
            Ok(Some(index))
        })?;
        let Some(loan) = loan else { return Ok(None) };
        self.with_emission_budget_v1(|this, budget| {
            source_reference_owned_prepay_v29::<Option<SemanticValueBindingV1>>(
                references.plan,
                budget,
            )?;
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(execution_availability_error_v29)?;
            let site = execution_site_v29(block, statement);
            if let Some(event) = cursor.find_occurrence(
                site,
                ExecutionOperandV29::RvaluePlace,
                ExecutionEventV29::BaseUse,
                budget,
            )? {
                if cursor.occurrences.events()[event].is_promoted() {
                    let definition = cursor.use_place(
                        site,
                        ExecutionOperandV29::RvaluePlace,
                        place,
                        false,
                        budget,
                    )?;
                    check_source_use_archive_v29(
                        cursor,
                        &this.control_flow_ssa.cfg_carriers,
                        &this.locals,
                        &this.semantic_ssa_bindings,
                        site,
                        ExecutionOperandV29::RvaluePlace,
                        place,
                        definition,
                        budget,
                    )?;
                } else {
                    // The A owner checks the exact retained local and generation;
                    // non-promoted storage has no SSA definition to manufacture.
                    let occurrence = &cursor.occurrences.events()[event];
                    if occurrence.event().variable().get() != place.local().index()
                        || occurrence.resolved().is_some()
                    {
                        return Err(execution_availability_error_v29());
                    }
                    cursor.claim_events(&[event], budget)?;
                }
            } else {
                budget.source_reference_charge_v29(
                    references.plan,
                    cursor.occurrences.elisions().len(),
                )?;
                if !cursor.occurrences.elisions().contains(&site) {
                    return Err(execution_availability_error_v29());
                }
            }
            Ok(())
        })?;
        let referent = match self
            .source_reference_scalar_address_v29(site, statement, place, loan, operations)?
        {
            Some(address) => address,
            None => self.resolve_place(block, statement, place, operations)?,
        };
        self.with_emission_budget_v1(|_, budget| {
            references.check(budget)?;
            budget.source_reference_reserve_v29(
                references.plan,
                std::mem::size_of::<SemanticSourceReferenceBindingV29>(),
            )?;
            let mut values = source_reference_owned_vec_v29(references.plan, 0, budget)?;
            execution_cfg_values_v29(&referent, &mut values, &mut 0, budget)?;
            let binding = SemanticSourceReferenceBindingV29 {
                owner: references.plan as *const SourceReferencePlanV29<'_, '_> as usize,
                source: references.plan.source,
                ssa: references.plan.ssa,
                root: references.plan.root,
                origin: SourceReferenceBindingOriginV29::SingleLoan(loan),
                source_type: result_type,
                values,
            };
            source_reference_validate_binding_v29(references.plan, &binding, budget)?;
            if references.claim(site, budget)? != loan {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(Some(SemanticValueBindingV1::SourceReference(binding)))
        })
    }

    fn dereference_source_reference_v29(
        &mut self,
        mut binding: SemanticSourceReferenceBindingV29,
        source_type: SemanticTypeIdV1,
        projection: &SemanticProjectionV1,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        projection_index: usize,
        access: SourceReferenceAccessV29,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if let Some(value) = self.try_dereference_scalar_cell_v29(
            &binding,
            block,
            statement,
            source,
            projection_index,
            access,
            operations,
        )? {
            return Ok(value);
        }
        self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(|| {
                    source_reference_error_v29("source reference has no active owner")
                })?;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<SemanticValueBindingV1>(references.plan, budget)?;
            if projection.kind() != SemanticProjectionKindV1::Dereference {
                return Err(source_reference_error_v29(
                    "source reference projection differs",
                ));
            }
            if let Some((id, ty)) = source_reference_allocation_value_v29(
                references.plan, this.types, &mut binding, source_type,
                projection.result_type(), false, budget,
            )? {
                return Ok(SemanticValueBindingV1::Value { id, ty });
            }
            let loan = &references.plan.loans[binding.origin.single_loan()?];
            let origin = &references.plan.origins[loan.origin];
            let mut values = binding.values.iter();
            let result = rebuild_execution_cfg_binding_v29(
                this.types,
                origin.ty,
                true,
                &mut [].iter(),
                &mut values,
                &mut 0,
                budget,
            )?;
            if values.next().is_some() {
                return Err(execution_call_error_v29());
            }
            Ok(result)
        })
    }
}

fn allocation_receiver_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("allocation receiver differs from its original checked carrier")
}

fn allocation_receiver_contract_v29(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    receiver: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(SemanticTypeIdV1, bool), ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let Some(SemanticTerminatorKindV1::Call(original)) = function.blocks()
        .get(block.index() as usize).map(|block| block.terminator().kind()) else {
        return Err(allocation_receiver_error_v29());
    };
    if !std::ptr::eq(original, call) || receiver != 0 || call.arguments().get(receiver).is_none() {
        return Err(allocation_receiver_error_v29());
    }
    let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
        callables.get(call.callee().index() as usize) else {
        return Err(allocation_receiver_error_v29());
    };
    use SemanticCompilerIntrinsicOperationV1 as Intrinsic;
    Ok(match operation {
        Intrinsic::DisjointSliceLen { disjoint_slice, .. }
        | Intrinsic::WriteOnlyDisjointSliceLen { disjoint_slice, .. } => (*disjoint_slice, false),
        Intrinsic::DisjointSliceGetMut { disjoint_slice, .. }
        | Intrinsic::DisjointSliceGetDisjointMut { disjoint_slice, .. }
        | Intrinsic::DisjointSliceGetMutExclusive { disjoint_slice, .. }
        | Intrinsic::DisjointSliceGetBlockMut { disjoint_slice, .. }
        | Intrinsic::DisjointSliceGetTiled2dMut { disjoint_slice, .. }
        | Intrinsic::DisjointSliceGetRowStriped2dMut { disjoint_slice, .. }
        | Intrinsic::WriteOnlyDisjointSliceWrite { disjoint_slice, .. } => (*disjoint_slice, true),
        _ => return Err(allocation_receiver_error_v29()),
    })
}

fn source_reference_allocation_value_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    types: &[SemanticTypeDeclV1],
    binding: &mut SemanticSourceReferenceBindingV29,
    source_type: SemanticTypeIdV1,
    referent_type: SemanticTypeIdV1,
    requires_mutable: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(ValueId, Type)>, ProductionSemanticKirErrorV1> {
    source_reference_owned_prepay_v29::<Option<(ValueId, Type)>>(plan, budget)?;
    source_reference_owned_prepay_v29::<
        Result<Option<(ValueId, Type)>, ProductionSemanticKirErrorV1>,
    >(plan, budget)?;
    if source_reference_allocation_borrowed_v29(plan, types, binding, source_type,
        referent_type, requires_mutable, budget)?.is_none() {
        return Ok(None);
    }
    let value = binding.values.pop().ok_or_else(allocation_receiver_error_v29)?;
    Ok(Some((value.id, value.ty)))
}

// Share the source/loan checks with final immutable issuer replay. The borrowed
// value remains a locator; the caller must authenticate its actual graph use.
fn source_reference_allocation_borrowed_v29<'binding>(
    plan: &SourceReferencePlanV29<'_, '_>,
    types: &[SemanticTypeDeclV1],
    binding: &'binding SemanticSourceReferenceBindingV29,
    source_type: SemanticTypeIdV1,
    referent_type: SemanticTypeIdV1,
    requires_mutable: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<&'binding ValueDef>, ProductionSemanticKirErrorV1> {
    source_reference_owned_prepay_v29::<Option<&ValueDef>>(plan, budget)?;
    source_reference_owned_prepay_v29::<Result<Option<&ValueDef>, ProductionSemanticKirErrorV1>>(plan, budget)?;
    source_reference_validate_binding_v29(plan, binding, budget)?;
    budget.source_reference_charge_v29(plan, 8)?;
    let loan = &plan.loans[binding.origin.single_loan()?];
    let origin = &plan.origins[loan.origin];
    if requires_mutable && loan.kind != SemanticBorrowKindV1::Mutable {
        return Err(allocation_receiver_error_v29());
    }
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types.get(source_type.index() as usize)
        .map(SemanticTypeDeclV1::shape) else {
        return Err(source_reference_error_v29("source reference projection type differs"));
    };
    if binding.source_type != source_type
        || pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.pointee() != origin.ty
        || referent_type != origin.ty
    {
        return Err(source_reference_error_v29("source reference projection differs"));
    }
    if !matches!(loan.representation, SourceReferenceRepresentationV29::ExistingAllocationBinding(_)) {
        return Ok(None);
    }
    if binding.values.len() != 1 || !matches!(binding.values[0].ty, Type::Slice(_)) {
        return Err(allocation_receiver_error_v29());
    }
    Ok(binding.values.first())
}
