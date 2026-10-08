include!("production_source_reference_current_projection_v29.rs");
include!("production_source_atomic_storage_path_v41.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceRawFormationV29 {
    AddressOf,
    ReferenceCast,
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceRawOriginV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    formation: SourceReferenceRawFormationV29,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    pointer_type: SemanticTypeIdV1,
    first: usize,
    count: usize,
    parent: Option<usize>,
    mutable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceRawChoiceV29 {
    origin: usize,
    expired: bool,
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceRawSetV29 {
    first: usize,
    count: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    parent: Option<usize>,
    mutable: bool,
    projection_count: usize,
}

#[derive(Clone, Copy)]
enum SourceReferenceRawChoicesV29 {
    Singleton(SourceReferenceRawChoiceV29),
    Retained(SourceReferenceRawSetV29),
}

struct SourceReferenceRawUnionV29 {
    left: SourceReferenceRawChoicesV29,
    right: SourceReferenceRawChoicesV29,
    left_index: usize,
    right_index: usize,
    expire: bool,
}

#[derive(Clone, Copy)]
struct SourceReferenceRawHolderV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    node: usize,
    first: usize,
    count: usize,
    parent: Option<usize>,
    shared_path: bool,
    selector_source: Option<(SourceReferenceSiteV29, usize)>,
}

#[derive(Clone, Copy)]
struct SourceReferenceRawAccessV29 {
    ordinal: usize,
    site: SourceReferenceSiteV29,
    source: usize,
    access: SourceReferenceAccessV29,
    crossing: SourceReferenceAccessV29,
    projection: usize,
    set: usize,
    pointee: SemanticTypeIdV1,
    ty: SemanticTypeIdV1,
    holder: SourceReferenceRawHolderV29,
}

type SourceReferenceRawAccessKeyV29 = (SourceReferenceAccessIndexKeyV29, usize);

#[derive(Clone, Copy)]
struct SourceReferenceRawPathV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    access: SourceReferenceAccessV29,
    last_dereference: usize,
    crossed: bool,
}

fn source_reference_raw_original_access_v29(
    declaration: &SemanticFunctionDeclV1,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceReferenceAccessV29>, ProductionSemanticKirErrorV1> {
    // The query borrows original source and returns only a Copy access kind.
    // Its visitor/operand/place return envelopes die before the next query;
    // retain their full prepaid peak, but not their completed-query storage.
    with_canonical_call_scratch_v1(budget, |budget| {
        source_reference_raw_original_access_inner_v29(declaration, site, source, budget)
    })
}

fn source_reference_raw_original_access_inner_v29(
    declaration: &SemanticFunctionDeclV1,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceReferenceAccessV29>, ProductionSemanticKirErrorV1> {
    fn place(
        found: &mut Option<SourceReferenceAccessV29>,
        source: &SemanticPlaceV1,
        candidate: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<()>(budget)?;
        budget.charge_work(3)?;
        if std::ptr::eq(source, candidate) {
            if found.is_some() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            *found = Some(access);
        }
        Ok(())
    }
    fn operand(
        found: &mut Option<SourceReferenceAccessV29>,
        source: &SemanticPlaceV1,
        candidate: &SemanticOperandV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<()>(budget)?;
        budget.charge_work(1)?;
        match candidate {
            SemanticOperandV1::Copy(value) | SemanticOperandV1::Move(value) => {
                place(found, source, value, SourceReferenceAccessV29::Read, budget)
            }
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    budget.charge_work(2)?;
    let block = declaration
        .blocks()
        .get(site.block.index() as usize)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let mut found = None;
    // Inspect only this original site. Source-only callers do not acquire an
    // occurrence table or a physical permit by authenticating a borrowed place.
    if let Some(index) = site.statement {
        let statement = block
            .statements()
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                place(
                    &mut found,
                    source,
                    assignment.destination(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                source_reference_emission_prepay_v29::<()>(budget)?;
                assignment
                    .value()
                    .kind()
                    .try_visit_operands(|value| operand(&mut found, source, value, budget))?;
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow { kind, place: value } => place(
                        &mut found,
                        source,
                        value,
                        SourceReferenceAccessV29::Borrow(*kind),
                        budget,
                    )?,
                    SemanticRvalueKindV1::AddressOf { place: value, .. } => place(
                        &mut found,
                        source,
                        value,
                        SourceReferenceAccessV29::Address,
                        budget,
                    )?,
                    SemanticRvalueKindV1::Length(value) => place(
                        &mut found,
                        source,
                        value,
                        SourceReferenceAccessV29::Read,
                        budget,
                    )?,
                    SemanticRvalueKindV1::Discriminant(value) => place(
                        &mut found,
                        source,
                        value,
                        SourceReferenceAccessV29::ReadDiscriminant,
                        budget,
                    )?,
                    SemanticRvalueKindV1::Load(load) => place(
                        &mut found,
                        source,
                        load.source(),
                        SourceReferenceAccessV29::Read,
                        budget,
                    )?,
                    SemanticRvalueKindV1::Use(_)
                    | SemanticRvalueKindV1::Unary { .. }
                    | SemanticRvalueKindV1::Binary { .. }
                    | SemanticRvalueKindV1::CheckedBinary(_)
                    | SemanticRvalueKindV1::UncheckedBinary(_)
                    | SemanticRvalueKindV1::Cast { .. }
                    | SemanticRvalueKindV1::Aggregate(_) => {}
                }
            }
            SemanticStatementKindV1::Store(store) => {
                place(
                    &mut found,
                    source,
                    store.destination(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                operand(&mut found, source, store.value(), budget)?;
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                place(
                    &mut found,
                    source,
                    atomic.destination(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                place(
                    &mut found,
                    source,
                    atomic.address(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                operand(&mut found, source, atomic.value(), budget)?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                place(
                    &mut found,
                    source,
                    atomic.destination(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                place(
                    &mut found,
                    source,
                    atomic.address(),
                    SourceReferenceAccessV29::Write,
                    budget,
                )?;
                operand(&mut found, source, atomic.expected(), budget)?;
                operand(&mut found, source, atomic.replacement(), budget)?;
            }
            SemanticStatementKindV1::SetDiscriminant { place: value, .. }
            | SemanticStatementKindV1::Deinitialize(value) => place(
                &mut found,
                source,
                value,
                SourceReferenceAccessV29::Write,
                budget,
            )?,
            SemanticStatementKindV1::Assume(value) => operand(&mut found, source, value, budget)?,
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => {}
        }
    } else {
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                for value in call.arguments() {
                    operand(&mut found, source, value, budget)?;
                }
                if let Some(destination) = call.destination() {
                    place(
                        &mut found,
                        source,
                        destination.place(),
                        SourceReferenceAccessV29::Write,
                        budget,
                    )?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for value in call.arguments() {
                    operand(&mut found, source, value, budget)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                operand(&mut found, source, discriminant, budget)?
            }
            SemanticTerminatorKindV1::Drop { place: value, .. } => place(
                &mut found,
                source,
                value,
                SourceReferenceAccessV29::Write,
                budget,
            )?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                operand(&mut found, source, condition, budget)?;
                for index in 0..2 {
                    budget.charge_work(1)?;
                    if let Some(value) = execution_assert_operand_v29(message, index) {
                        operand(&mut found, source, value, budget)?;
                    }
                }
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok(found)
}

impl SourceReferencePlanV29<'_, '_> {
    fn raw_source_path(
        &self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceReferenceRawPathV29>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.check_owner(self.instances, budget)?;
            source_reference_emission_prepay_v29::<Option<SourceReferenceRawPathV29>>(budget)?;
            if source.projections().is_empty() {
                return Ok(None);
            }
            self.charge(5, budget)?;
            let instance = self
                .instances
                .instance(site.instance)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let declaration = instance.declaration();
            let mut ty = declaration
                .locals()
                .get(source.local().index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?
                .ty();
            let types = self.instances.owner().source_semantic().types();
            budget.reserve_storage(std::mem::size_of::<(bool, Option<usize>)>())?;
            let (mut raw, mut last_dereference) = (false, None);
            for (ordinal, projection) in source.projections().iter().enumerate() {
                self.charge(3, budget)?;
                if projection.kind() == SemanticProjectionKindV1::Dereference {
                    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                        .get(ty.index() as usize)
                        .map(SemanticTypeDeclV1::shape)
                    else {
                        return Err(ArgumentResourceV1::Accounting.into());
                    };
                    if pointer.pointee() != projection.result_type() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    raw |= pointer.kind() == SemanticPointerKindV1::Raw
                        && pointer.metadata() == SemanticPointerMetadataV1::None;
                    last_dereference = Some(ordinal);
                }
                ty = projection.result_type();
            }
            if ty != source.ty() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if !raw {
                return Ok(None);
            }
            if self.instances.instance_reachable(site.instance) != Some(true)
                || self.instances.block_reachable(site.instance, site.block) != Some(true)
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            source_reference_emission_prepay_v29::<Option<SourceReferenceAccessV29>>(budget)?;
            if source_reference_raw_original_access_v29(declaration, site, source, budget)?
                != Some(access)
            {
                // Original-place custody failures remain sticky if a caller catches them.
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(Some(SourceReferenceRawPathV29 {
                site,
                source: source as *const SemanticPlaceV1 as usize,
                access,
                last_dereference: last_dereference.ok_or(ArgumentResourceV1::Accounting)?,
                crossed: false,
            }))
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }

    fn raw_choice(
        &self,
        choices: SourceReferenceRawChoicesV29,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceReferenceRawChoiceV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        match choices {
            SourceReferenceRawChoicesV29::Singleton(value) => Ok((index == 0).then_some(value)),
            SourceReferenceRawChoicesV29::Retained(range) => {
                if index >= range.count {
                    return Ok(None);
                }
                self.raw_choices
                    .get(argument_sum_v1(&[range.first, index])?)
                    .copied()
                    .map(Some)
                    .ok_or_else(|| ArgumentResourceV1::Accounting.into())
            }
        }
    }

    fn next_raw_union(
        &self,
        union: &mut SourceReferenceRawUnionV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceReferenceRawChoiceV29>, ProductionSemanticKirErrorV1> {
        let left = self.raw_choice(union.left, union.left_index, budget)?;
        let right = self.raw_choice(union.right, union.right_index, budget)?;
        budget.charge_work(1)?;
        let choice = match (left, right) {
            (None, None) => None,
            (Some(a), Some(b)) if a.origin == b.origin => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                union.right_index = argument_sum_v1(&[union.right_index, 1])?;
                Some(SourceReferenceRawChoiceV29 {
                    origin: a.origin,
                    expired: a.expired || b.expired,
                })
            }
            (Some(a), Some(b)) if a.origin < b.origin => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                Some(a)
            }
            (Some(a), None) => {
                union.left_index = argument_sum_v1(&[union.left_index, 1])?;
                Some(a)
            }
            (_, Some(b)) => {
                union.right_index = argument_sum_v1(&[union.right_index, 1])?;
                Some(b)
            }
        };
        Ok(choice.map(|mut choice| {
            choice.expired |= union.expire;
            choice
        }))
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn cast_address_value(
        &mut self,
        site: SourceReferenceSiteV29,
        kind: SemanticCastKindV1,
        operand: &SemanticOperandV1,
        output: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let source = self
            .plan
            .instances
            .instance(site.instance)
            .and_then(|row| row.declaration().blocks().get(site.block.index() as usize))
            .and_then(|block| {
                site.statement
                    .and_then(|index| block.statements().get(index))
            })
            .ok_or_else(|| {
                source_reference_error_v29("source raw cast has no original statement")
            })?;
        let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
            return Err(source_reference_error_v29(
                "source raw cast is not an original assignment",
            ));
        };
        let SemanticRvalueKindV1::Cast {
            kind: expected_kind,
            operand: expected,
        } = assignment.value().kind()
        else {
            return Err(source_reference_error_v29(
                "source raw cast operation differs",
            ));
        };
        if kind != *expected_kind
            || !std::ptr::eq(operand, expected)
            || assignment.value().result_type() != output
        {
            return Err(source_reference_error_v29(
                "source raw cast operand differs",
            ));
        }
        if kind == SemanticCastKindV1::PointerWithExposedProvenance {
            return Err(source_reference_error_v29(
                "source numeric address cannot reconstruct pointer provenance",
            ));
        }
        let node = self.operand(site, operand, budget)?;
        let row = *self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if row.ty != operand.ty() {
            return Err(source_reference_error_v29(
                "source raw cast input type differs",
            ));
        }
        if self.plan.instances.owner().source_semantic().wire_version()
            == fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
            && let Some(result) =
                self.cast_atomic_view_v41(site, kind, operand, node, output, budget)?
        {
            return Ok(result);
        }
        if kind == SemanticCastKindV1::PointerExposeProvenance {
            source_reference_check_address_exposure_v29(
                self.plan.instances.owner().source_semantic().types(),
                row.ty,
                output,
                budget,
            )?;
            self.observe_node_address(node, budget)?;
            // Raw addresses need not carry a reference loan to be observable.
            budget.charge_work(1)?;
            self.plan.address_observed = true;
            return self.plain(output, budget);
        }
        if !matches!(
            row.kind,
            SourceReferenceNodeKindV29::Address(_) | SourceReferenceNodeKindV29::Loan(_)
        ) {
            self.observe_node_address(node, budget)?;
            return self.plain(output, budget);
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let (SemanticTypeShapeV1::Pointer(input), SemanticTypeShapeV1::Pointer(result)) = (
            types[row.ty.index() as usize].shape(),
            types[output.index() as usize].shape(),
        ) else {
            return Err(source_reference_error_v29(
                "source raw address cast cannot expose or reconstruct provenance",
            ));
        };
        if kind != SemanticCastKindV1::Pointer
            || result.kind() != SemanticPointerKindV1::Raw
            || input.pointee() != result.pointee()
            || input.address_space() != result.address_space()
            || input.pointer_width_bits() != result.pointer_width_bits()
            || input.metadata() != SemanticPointerMetadataV1::None
            || result.metadata() != SemanticPointerMetadataV1::None
        {
            return Err(source_reference_error_v29(
                "source raw address cast changes its admitted representation",
            ));
        }
        let set = match row.kind {
            SourceReferenceNodeKindV29::Address(set)
                if input.kind() == SemanticPointerKindV1::Raw =>
            {
                set
            }
            SourceReferenceNodeKindV29::Loan(loan)
                if input.kind() == SemanticPointerKindV1::Reference =>
            {
                self.check_loan_use(loan, budget)?;
                self.observe_node_address(node, budget)?;
                budget.charge_work(6)?;
                let record = self
                    .plan
                    .loans
                    .get(loan)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let borrowed = self
                    .plan
                    .origins
                    .get(record.origin)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let origin = SourceReferenceRawOriginV29 {
                    site,
                    source: operand as *const SemanticOperandV1 as usize,
                    formation: SourceReferenceRawFormationV29::ReferenceCast,
                    instance: borrowed.instance,
                    local: borrowed.local,
                    generation: borrowed.generation,
                    ty: borrowed.ty,
                    pointer_type: output,
                    first: borrowed.projections.start,
                    count: borrowed.projections.len(),
                    parent: Some(loan),
                    mutable: record.kind == SemanticBorrowKindV1::Mutable
                        && result.mutability() == SemanticMutabilityV1::Mutable,
                };
                self.storage_live_at(origin.instance, origin.local, budget)?;
                let origin = self.retain_raw_formation(origin, budget)?;
                let choice = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
                    origin,
                    expired: false,
                });
                self.raw_set(choice, choice, false, budget)?
            }
            _ => {
                return Err(source_reference_error_v29(
                    "source raw cast lacks its exact pointer derivation",
                ));
            }
        };
        // The origin's access permission is unchanged even if MIR changes raw
        // pointer mutability. A later dereference still checks every origin.
        self.raw_node(output, set, budget)
    }

    fn retain_raw_formation(
        &mut self,
        origin: SourceReferenceRawOriginV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let statement = origin
            .site
            .statement
            .ok_or(ArgumentResourceV1::Accounting)?;
        let key = (
            origin.site.instance.index(),
            origin.site.block.index(),
            statement,
            origin.generation,
        );
        charge_execution_cfg_lookup_v29(self.plan.raw_origin_sites.len(), budget)?;
        let end = argument_sum_v1(&[origin.first, origin.count])?;
        if end > self.plan.projections.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if let Some(&index) = self.plan.raw_origin_sites.get(&key) {
            let previous = self
                .plan
                .raw_origins
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.charge_work(argument_sum_v1(&[12, origin.count])?)?;
            let previous_end = argument_sum_v1(&[previous.first, previous.count])?;
            if previous.site != origin.site
                || previous.source != origin.source
                || previous.formation != origin.formation
                || previous.instance != origin.instance
                || previous.local != origin.local
                || previous.generation != origin.generation
                || previous.ty != origin.ty
                || previous.pointer_type != origin.pointer_type
                || previous.parent != origin.parent
                || previous.mutable != origin.mutable
                || self.plan.projections.get(previous.first..previous_end)
                    != self.plan.projections.get(origin.first..end)
            {
                return Err(source_reference_error_v29(
                    "source raw formation changed its original derivation",
                ));
            }
            return Ok(index);
        }
        let index = self.plan.raw_origins.len();
        emission_push_v1(&mut self.plan.raw_origins, origin, budget)?;
        reserve_execution_cfg_map_entry_v29::<(usize, u32, usize, u32), usize>(
            self.plan.raw_origin_sites.len(),
            budget,
        )?;
        self.plan.raw_origin_sites.insert(key, index);
        Ok(index)
    }

    fn retain_raw_holder(
        &mut self,
        resolved: &SourceReferencePlaceV29,
        set: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceRawHolderV29, ProductionSemanticKirErrorV1> {
        self.plan.check_owner(self.plan.instances, budget)?;
        source_reference_emission_prepay_v29::<SourceReferenceRawHolderV29>(budget)?;
        self.plan
            .charge(argument_sum_v1(&[8, resolved.projections.len()])?, budget)?;
        if !matches!(self.plan.nodes.get(resolved.node).map(|row| row.kind),
            Some(SourceReferenceNodeKindV29::Address(actual)) if actual == set)
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let first = self.plan.projections.len();
        for &projection in &resolved.projections {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        Ok(SourceReferenceRawHolderV29 {
            instance: resolved.instance,
            local: resolved.local,
            generation: resolved.generation,
            node: resolved.node,
            first,
            count: resolved.projections.len(),
            parent: resolved.loan,
            shared_path: resolved.shared_path,
            selector_source: resolved.selector_source,
        })
    }

    fn retain_raw_access(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        path: &SourceReferenceRawPathV29,
        projection: usize,
        set: usize,
        mut holder: SourceReferenceRawHolderV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.plan.check_owner(self.plan.instances, budget)?;
            source_reference_emission_prepay_v29::<SourceReferenceRawAccessV29>(budget)?;
            source_reference_emission_prepay_v29::<SourceReferenceRawAccessKeyV29>(budget)?;
            self.plan.charge(8, budget)?;
            let boundary = source
                .projections()
                .get(projection)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let input = if projection == 0 {
                self.plan
                    .instances
                    .instance(site.instance)
                    .and_then(|instance| {
                        instance
                            .declaration()
                            .locals()
                            .get(source.local().index() as usize)
                    })
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .ty()
            } else {
                source.projections()[projection - 1].result_type()
            };
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
                .plan
                .instances
                .owner()
                .source_semantic()
                .types()
                .get(input.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            let crossing = if projection < path.last_dereference {
                SourceReferenceAccessV29::Read
            } else {
                access
            };
            self.plan.charge(5, budget)?;
            let holder_node = self
                .plan
                .nodes
                .get(holder.node)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let holder_end = argument_sum_v1(&[holder.first, holder.count])?;
            if holder_node.ty != input
                || !matches!(holder_node.kind, SourceReferenceNodeKindV29::Address(actual) if actual == set)
                || self
                    .plan
                    .projections
                    .get(holder.first..holder_end)
                    .is_none()
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if path.site != site
                || path.source != source as *const SemanticPlaceV1 as usize
                || path.access != access
                || projection > path.last_dereference
                || boundary.kind() != SemanticProjectionKindV1::Dereference
                || pointer.kind() != SemanticPointerKindV1::Raw
                || pointer.metadata() != SemanticPointerMetadataV1::None
                || pointer.pointee() != boundary.result_type()
                || self
                    .plan
                    .raw_sets
                    .get(set)
                    .is_none_or(|row| row.ty != boundary.result_type())
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let key = (
                source_reference_access_key_v29(site, source, access),
                projection,
            );
            charge_execution_cfg_lookup_v29(self.plan.raw_accesses.len(), budget)?;
            if let Some(previous) = self.plan.raw_accesses.get(&key).map(|row| **row) {
                budget.charge_work(6)?;
                if previous.site != site
                    || previous.source != source as *const SemanticPlaceV1 as usize
                    || previous.access != access
                    || previous.crossing != crossing
                    || previous.projection != projection
                    || previous.pointee != boundary.result_type()
                    || previous.ty != source.ty()
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                self.plan
                    .charge(argument_sum_v1(&[8, holder.count])?, budget)?;
                if previous.holder.instance != holder.instance
                    || previous.holder.local != holder.local
                    || previous.holder.parent != holder.parent
                    || previous.holder.shared_path != holder.shared_path
                    || previous.holder.selector_source != holder.selector_source
                    || self.plan.projections.get(
                        previous.holder.first
                            ..argument_sum_v1(&[previous.holder.first, previous.holder.count])?,
                    ) != self.plan.projections.get(holder.first..holder_end)
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                holder.generation = self.join_storage_epochs(
                    holder.instance,
                    holder.local,
                    previous.holder.generation,
                    holder.generation,
                    budget,
                )?;
                holder.node = self.merge_node(previous.holder.node, holder.node, budget)?;
                holder.first = previous.holder.first;
                let mut selected_set = set;
                if previous.set != set {
                    let a = *self
                        .plan
                        .raw_sets
                        .get(previous.set)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let b = *self
                        .plan
                        .raw_sets
                        .get(set)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let set = self.raw_set(
                        SourceReferenceRawChoicesV29::Retained(a),
                        SourceReferenceRawChoicesV29::Retained(b),
                        false,
                        budget,
                    )?;
                    selected_set = set;
                }
                if !matches!(self.plan.nodes.get(holder.node).map(|row| row.kind),
                Some(SourceReferenceNodeKindV29::Address(actual)) if actual == selected_set)
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let published = self
                    .plan
                    .raw_accesses
                    .get_mut(&key)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                published.set = selected_set;
                published.holder = holder;
            } else {
                charge_execution_cfg_lookup_v29(self.plan.raw_accesses.len(), budget)?;
                // Keep full source keys and records out of unused BTree split slots.
                // Pay the unchanged node allowance and both owned allocations first.
                budget.reserve_storage(argument_sum_v1(&[
                    execution_cfg_map_entry_storage_v29::<
                        Box<SourceReferenceRawAccessKeyV29>,
                        Box<SourceReferenceRawAccessV29>,
                    >(self.plan.raw_accesses.len())?,
                    std::mem::size_of::<SourceReferenceRawAccessKeyV29>(),
                    std::mem::size_of::<SourceReferenceRawAccessV29>(),
                ])?)?;
                self.plan.raw_accesses.insert(
                    Box::new(key),
                    Box::new(SourceReferenceRawAccessV29 {
                        ordinal: self.plan.raw_accesses.len(),
                        site,
                        source: source as *const SemanticPlaceV1 as usize,
                        access,
                        crossing,
                        projection,
                        set,
                        pointee: boundary.result_type(),
                        ty: source.ty(),
                        holder,
                    }),
                );
            }
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(&self.plan, error))
    }

    fn expire_address_node(
        &mut self,
        node: usize,
        instance: ProductionCallInstanceIdV1,
        local: Option<SemanticLocalIdV1>,
        depth: usize,
        memo: &mut SourceReferenceMemoV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        if depth >= 256 {
            return Err(source_reference_error_v29(
                "source raw address expiry exceeds its typed depth bound",
            ));
        }
        charge_execution_cfg_lookup_v29(memo.len(), budget)?;
        if let Some(&value) = memo.get(&node) {
            return Ok(value);
        }
        budget.charge_work(2)?;
        let original = *self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let mut descriptor = original.descriptor;
        let kind = match original.kind {
            SourceReferenceNodeKindV29::Enum { .. } | SourceReferenceNodeKindV29::EnumView(_) => {
                let rebuilt =
                    self.expire_enum_address_node(node, instance, local, depth, memo, budget)?;
                descriptor = self.plan.nodes[rebuilt].descriptor;
                self.plan.nodes[rebuilt].kind
            }
            SourceReferenceNodeKindV29::Address(set) => {
                let row = *self
                    .plan
                    .raw_sets
                    .get(set)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if row.instance == instance && local.is_none_or(|local| local == row.local) {
                    let source = SourceReferenceRawChoicesV29::Retained(row);
                    SourceReferenceNodeKindV29::Address(self.raw_set(source, source, true, budget)?)
                } else {
                    original.kind
                }
            }
            SourceReferenceNodeKindV29::Aggregate { first, count } => {
                let end = argument_sum_v1(&[first, count])?;
                if end > self.plan.children.len() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let mut changed = false;
                for index in first..end {
                    budget.charge_work(1)?;
                    let child = self.plan.children[index];
                    if child >= node {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    changed |=
                        self.expire_address_node(child, instance, local, depth + 1, memo, budget)?
                            != child;
                }
                if changed {
                    let next = self.plan.children.len();
                    for index in first..end {
                        budget.charge_work(1)?;
                        let child = self.plan.children[index];
                        charge_execution_cfg_lookup_v29(memo.len(), budget)?;
                        let child = *memo.get(&child).ok_or(ArgumentResourceV1::Accounting)?;
                        emission_push_v1(&mut self.plan.children, child, budget)?;
                    }
                    SourceReferenceNodeKindV29::Aggregate { first: next, count }
                } else {
                    original.kind
                }
            }
            SourceReferenceNodeKindV29::Absent
            | SourceReferenceNodeKindV29::Plain(_)
            | SourceReferenceNodeKindV29::Discriminant(_)
            | SourceReferenceNodeKindV29::Loan(_) => original.kind,
        };
        let value = if kind == original.kind && descriptor == original.descriptor {
            node
        } else {
            let value = self.plan.nodes.len();
            emission_push_v1(
                &mut self.plan.nodes,
                SourceReferenceNodeV29 {
                    kind,
                    descriptor,
                    value_origin: None,
                    ..original
                },
                budget,
            )?;
            value
        };
        memo.insert(node, value, budget)?;
        Ok(value)
    }

    fn expire_addresses(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: Option<SemanticLocalIdV1>,
        result: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if self.plan.raw_origins.is_empty() {
            return Ok(result);
        }
        self.with_memo_v29(budget, |this, memo, budget| {
            for index in 0..this.frames.len() {
                budget.charge_work(1)?;
                let Some(state) = this.frames[index] else {
                    continue;
                };
                for slot in 0..this.plan.states[state].len() {
                    budget.charge_work(1)?;
                    let Some(node) = this.plan.states[state][slot].node else {
                        continue;
                    };
                    let node = this.expire_address_node(node, instance, local, 0, memo, budget)?;
                    this.plan.states[state][slot].node = Some(node);
                }
            }
            result
                .map(|node| this.expire_address_node(node, instance, local, 0, memo, budget))
                .transpose()
        })
    }

    fn raw_set(
        &mut self,
        left: SourceReferenceRawChoicesV29,
        right: SourceReferenceRawChoicesV29,
        expire: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<SourceReferenceRawUnionV29>(),
            2 * std::mem::size_of::<SourceReferenceRawChoicesV29>(),
            std::mem::size_of::<SourceReferenceRawOriginV29>(),
            4 * std::mem::size_of::<
                Result<Option<SourceReferenceRawChoiceV29>, ProductionSemanticKirErrorV1>,
            >(),
            2 * std::mem::size_of::<Option<SourceReferenceRawChoiceV29>>(),
            2 * std::mem::size_of::<Result<usize, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let first = self
            .plan
            .raw_choice(left, 0, budget)?
            .ok_or(ArgumentResourceV1::Accounting)?;
        let object = *self
            .plan
            .raw_origins
            .get(first.origin)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let key = (object.instance.index(), object.local.index());
        let mut union = SourceReferenceRawUnionV29 {
            left,
            right,
            left_index: 0,
            right_index: 0,
            expire,
        };
        let mut count = 0usize;
        while let Some(choice) = self.plan.next_raw_union(&mut union, budget)? {
            budget.charge_work(4)?;
            let origin = self
                .plan
                .raw_origins
                .get(choice.origin)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if origin.instance != object.instance
                || origin.local != object.local
                || origin.ty != object.ty
                || origin.pointer_type != object.pointer_type
                || origin.parent != object.parent
                || origin.mutable != object.mutable
                || origin.count != object.count
            {
                return Err(source_reference_error_v29(
                    "source raw-address choice needs correlated object transport",
                ));
            }
            budget.charge_work(origin.count)?;
            let a = argument_sum_v1(&[origin.first, origin.count])?;
            let b = argument_sum_v1(&[object.first, object.count])?;
            if self.plan.projections.get(origin.first..a)
                != self.plan.projections.get(object.first..b)
            {
                return Err(source_reference_error_v29(
                    "source raw-address choice changes its typed path",
                ));
            }
            count = argument_sum_v1(&[count, 1])?;
        }
        charge_execution_cfg_lookup_v29(self.plan.raw_set_objects.len(), budget)?;
        if let Some(indices) = self.plan.raw_set_objects.get(&key) {
            for &index in indices {
                budget.charge_work(1)?;
                let row = self
                    .plan
                    .raw_sets
                    .get(index)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if row.count != count {
                    continue;
                }
                let mut union = SourceReferenceRawUnionV29 {
                    left,
                    right,
                    left_index: 0,
                    right_index: 0,
                    expire,
                };
                let mut position = 0usize;
                let mut same = true;
                while let Some(choice) = self.plan.next_raw_union(&mut union, budget)? {
                    budget.charge_work(1)?;
                    if self
                        .plan
                        .raw_choices
                        .get(argument_sum_v1(&[row.first, position])?)
                        != Some(&choice)
                    {
                        same = false;
                        break;
                    }
                    position = argument_sum_v1(&[position, 1])?;
                }
                if same && position == row.count {
                    return Ok(index);
                }
            }
        }
        let first = self.plan.raw_choices.len();
        let mut union = SourceReferenceRawUnionV29 {
            left,
            right,
            left_index: 0,
            right_index: 0,
            expire,
        };
        while let Some(choice) = self.plan.next_raw_union(&mut union, budget)? {
            emission_push_v1(&mut self.plan.raw_choices, choice, budget)?;
        }
        let index = self.plan.raw_sets.len();
        emission_push_v1(
            &mut self.plan.raw_sets,
            SourceReferenceRawSetV29 {
                first,
                count,
                instance: object.instance,
                local: object.local,
                ty: object.ty,
                parent: object.parent,
                mutable: object.mutable,
                projection_count: object.count,
            },
            budget,
        )?;
        charge_execution_cfg_lookup_v29(self.plan.raw_set_objects.len(), budget)?;
        if !self.plan.raw_set_objects.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<(usize, u32), Vec<usize>>(
                self.plan.raw_set_objects.len(),
                budget,
            )?;
            self.plan.raw_set_objects.insert(key, Vec::new());
        }
        emission_push_v1(
            self.plan
                .raw_set_objects
                .get_mut(&key)
                .ok_or(ArgumentResourceV1::Accounting)?,
            index,
            budget,
        )?;
        Ok(index)
    }

    fn raw_node(
        &mut self,
        ty: SemanticTypeIdV1,
        set: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        let object = self
            .plan
            .raw_sets
            .get(set)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .map(|ty| ty.shape())
        else {
            return Err(source_reference_error_v29(
                "source raw address node has no pointer type",
            ));
        };
        if object.count == 0
            || pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != object.ty
        {
            return Err(source_reference_error_v29(
                "source raw address node changes its typed target",
            ));
        }
        let key = (ty.index(), set);
        charge_execution_cfg_lookup_v29(self.plan.raw_nodes.len(), budget)?;
        if let Some(&node) = self.plan.raw_nodes.get(&key) {
            return Ok(node);
        }
        let node = self.node(ty, SourceReferenceNodeKindV29::Address(set), budget)?;
        reserve_execution_cfg_map_entry_v29::<(u32, usize), usize>(
            self.plan.raw_nodes.len(),
            budget,
        )?;
        self.plan.raw_nodes.insert(key, node);
        Ok(node)
    }

    fn storage_live_at(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let root = self.storage_root.as_ref().ok_or_else(|| {
            source_reference_error_v29(
                "source raw address requires checked original storage lifetime",
            )
        })?;
        let snapshot = self.local(instance, local)?.storage.ok_or_else(|| {
            source_reference_error_v29("source raw address has no original typed storage state")
        })?;
        if !root
            .snapshot_live(self.storage_snapshot(snapshot)?, budget)
            .map_err(|error| self.storage_error(error))?
        {
            return Err(source_reference_error_v29(
                "source raw address names dead storage",
            ));
        }
        Ok(())
    }

    fn address_value(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        pointer_type: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let statement = site
            .statement
            .ok_or_else(|| source_reference_error_v29("source raw address has no statement"))?;
        let function = self
            .plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let actual = function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .ok_or(ArgumentResourceV1::Accounting)?;
        let SemanticStatementKindV1::Assign(assignment) = actual.kind() else {
            return Err(ArgumentResourceV1::Accounting.into());
        };
        let SemanticRvalueKindV1::AddressOf {
            place,
            mutability: exact,
        } = assignment.value().kind()
        else {
            return Err(source_reference_error_v29(
                "source raw address changed its original operation",
            ));
        };
        if !std::ptr::eq(place, source)
            || *exact != mutability
            || assignment.value().result_type() != pointer_type
        {
            return Err(source_reference_error_v29(
                "source raw address changed its original operand",
            ));
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(pointer_type.index() as usize)
            .map(|ty| ty.shape())
        else {
            return Err(source_reference_error_v29(
                "source raw address result is not a pointer",
            ));
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != source.ty()
            || pointer.mutability() != mutability
        {
            return Err(source_reference_error_v29(
                "source raw address pointer contract differs",
            ));
        }
        if let Some(node) =
            self.address_atomic_view_v41(site, source, pointer_type, mutability, budget)?
        {
            return Ok(node);
        }
        let target =
            self.resolve_reference_place(site, source, SourceReferenceAccessV29::Address, budget)?;
        self.storage_live_at(target.instance, target.local, budget)?;
        if target.selector_source.is_some() {
            return Err(source_reference_error_v29(
                "source raw selected address requires correlated selector transport",
            ));
        }
        if target.shared_path && mutability == SemanticMutabilityV1::Mutable {
            return Err(source_reference_error_v29(
                "source raw address widens a shared derivation",
            ));
        }
        self.observe_address(&target, budget)?;
        let key = (
            site.instance.index(),
            site.block.index(),
            statement,
            target.generation,
        );
        charge_execution_cfg_lookup_v29(self.plan.raw_origin_sites.len(), budget)?;
        let origin = if let Some(&index) = self.plan.raw_origin_sites.get(&key) {
            let found = self
                .plan
                .raw_origins
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.charge_work(argument_sum_v1(&[8, target.projections.len()])?)?;
            let end = argument_sum_v1(&[found.first, found.count])?;
            if found.source != source as *const SemanticPlaceV1 as usize
                || found.site != site
                || found.formation != SourceReferenceRawFormationV29::AddressOf
                || found.instance != target.instance
                || found.local != target.local
                || found.ty != source.ty()
                || found.pointer_type != pointer_type
                || found.parent != target.loan
                || found.mutable != (mutability == SemanticMutabilityV1::Mutable)
                || self.plan.projections.get(found.first..end)
                    != Some(target.projections.as_slice())
            {
                return Err(source_reference_error_v29(
                    "source raw address site changed its original object",
                ));
            }
            index
        } else {
            let first = self.plan.projections.len();
            for projection in &target.projections {
                emission_push_v1(&mut self.plan.projections, *projection, budget)?;
            }
            let index = self.plan.raw_origins.len();
            emission_push_v1(
                &mut self.plan.raw_origins,
                SourceReferenceRawOriginV29 {
                    site,
                    source: source as *const SemanticPlaceV1 as usize,
                    formation: SourceReferenceRawFormationV29::AddressOf,
                    instance: target.instance,
                    local: target.local,
                    generation: target.generation,
                    ty: source.ty(),
                    pointer_type,
                    first,
                    count: target.projections.len(),
                    parent: target.loan,
                    mutable: mutability == SemanticMutabilityV1::Mutable,
                },
                budget,
            )?;
            reserve_execution_cfg_map_entry_v29::<(usize, u32, usize, u32), usize>(
                self.plan.raw_origin_sites.len(),
                budget,
            )?;
            self.plan.raw_origin_sites.insert(key, index);
            index
        };
        let choice = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
            origin,
            expired: false,
        });
        let set = self.raw_set(choice, choice, false, budget)?;
        self.raw_node(pointer_type, set, budget)
    }

    fn resolve_raw_target(
        &mut self,
        resolved: &mut SourceReferencePlaceV29,
        set: usize,
        ty: SemanticTypeIdV1,
        access: SourceReferenceAccessV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_storage_read(resolved, budget)?;
        self.check_place_access(resolved, SourceReferenceAccessV29::Read, budget)?;
        if let Some(loan) = resolved.loan {
            self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
        }
        let range = self.plan.raw_projection_range(set, ty, budget)?;
        let choices = *self
            .plan
            .raw_sets
            .get(set)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let end = argument_sum_v1(&[choices.first, choices.count])?;
        if choices.count == 0 || end > self.plan.raw_choices.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        for index in choices.first..end {
            budget.charge_work(4)?;
            let choice = self.plan.raw_choices[index];
            let origin = *self
                .plan
                .raw_origins
                .get(choice.origin)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if choice.expired {
                return Err(source_reference_error_v29(
                    "source raw pointer outlived its storage activation",
                ));
            }
            if origin.ty != ty {
                return Err(source_reference_error_v29(
                    "source raw pointer changed its pointee",
                ));
            }
            if !origin.mutable
                && matches!(
                    access,
                    SourceReferenceAccessV29::Write
                        | SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable)
                )
            {
                return Err(source_reference_error_v29(
                    "source raw pointer access widens its original permission",
                ));
            }
            self.storage_live_at(origin.instance, origin.local, budget)?;
            let local = self.local(origin.instance, origin.local)?;
            if !self.epoch_includes(
                origin.instance,
                origin.local,
                local.generation,
                origin.generation,
                budget,
            )? {
                return Err(source_reference_error_v29(
                    "source raw pointer changed its current activation",
                ));
            }
        }
        if let Some(parent) = choices.parent {
            self.check_loan_use(parent, budget)?;
        }
        // This common target exists only after every alternative's full target
        // agrees. No selected or first origin supplies otherwise-missing facts.
        self.plan.charge(3, budget)?;
        let root_type = self
            .plan
            .instances
            .instance(choices.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(choices.local.index() as usize)
            })
            .map(|local| local.ty())
            .ok_or(ArgumentResourceV1::Accounting)?;
        let local = self.local(choices.instance, choices.local)?;
        let node = match local.node {
            Some(node) => node,
            None if matches!(
                access,
                SourceReferenceAccessV29::Write | SourceReferenceAccessV29::Address
            ) =>
            {
                self.node(root_type, SourceReferenceNodeKindV29::Absent, budget)?
            }
            None => {
                return Err(source_reference_error_v29(
                    "source raw pointer reads an undefined referent",
                ));
            }
        };
        if self.plan.nodes.get(node).map(|row| row.ty) != Some(root_type) {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let target = self.current_projection_node(node, range.clone(), ty, budget)?;
        let projections = self.plan.current_projection_path(range, budget)?;
        resolved.instance = choices.instance;
        resolved.local = choices.local;
        resolved.generation = local.generation;
        resolved.value = node;
        resolved.representation_root = node;
        resolved.node = target;
        resolved.projections = projections;
        resolved.selector_source = None;
        resolved.anchor = match self.plan.nodes[target].kind {
            SourceReferenceNodeKindV29::Plain(anchor) => anchor,
            _ => None,
        };
        resolved.loan = choices.parent;
        // Copying a raw pointer reads its holder. Pointee rights still come
        // exclusively from every checked inner origin, not holder mutability.
        resolved.shared_path = !choices.mutable;
        Ok(())
    }
}

fn source_reference_check_address_exposure_v29(
    types: &[SemanticTypeDeclV1],
    input: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let input = types.get(input.index() as usize).map(|row| row.shape());
    let output = types.get(output.index() as usize).map(|row| row.shape());
    let scalar = match output {
        Some(SemanticTypeShapeV1::Scalar(scalar)) => Some(*scalar),
        Some(SemanticTypeShapeV1::ValidityScalar(validity)) => Some(validity.scalar()),
        _ => None,
    };
    // This is the existing admitted MIR exposure contract, not a pointer
    // reconstruction or permission to use the resulting integer as an address.
    if matches!(
        (input, scalar),
        (
            Some(SemanticTypeShapeV1::Pointer(pointer)),
            Some(SemanticScalarTypeV1::Integer { signed: false, bits }),
        ) if pointer.kind() == SemanticPointerKindV1::Raw
            && pointer.address_space() <= 6
            && pointer.metadata() == SemanticPointerMetadataV1::None
            && pointer.pointer_width_bits() == bits
    ) {
        Ok(())
    } else {
        Err(source_reference_error_v29(
            "source address exposure differs from its admitted scalar representation",
        ))
    }
}
