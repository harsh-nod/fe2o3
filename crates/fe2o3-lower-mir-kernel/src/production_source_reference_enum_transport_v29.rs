// Rebuild immutable payload paths without combining fields from distinct alternatives.
fn source_reference_binding_origin_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    origin: SourceReferenceBindingOriginV29,
    source_type: SemanticTypeIdV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    execution_cfg_charge_node_v29(nodes, budget)?;
    match origin {
        SourceReferenceBindingOriginV29::SingleLoan(loan) => {
            budget.source_reference_charge_v29(plan, 1)?;
            if plan.loans.get(loan).map(|loan| loan.source_type) != Some(source_type) {
                return Err(source_reference_enum_error_v29());
            }
            source_reference_payload_types_v29(plan, loan, budget)
        }
        SourceReferenceBindingOriginV29::EnumView(node) => {
            budget.source_reference_charge_v29(plan, 8)?;
            let row = plan
                .nodes
                .get(node)
                .ok_or_else(source_reference_enum_error_v29)?;
            let SourceReferenceNodeKindV29::EnumView(index) = row.kind else {
                return Err(source_reference_enum_error_v29());
            };
            let view = plan
                .enum_views
                .get(index)
                .ok_or_else(source_reference_enum_error_v29)?;
            let parent = plan
                .nodes
                .get(view.source)
                .ok_or_else(source_reference_enum_error_v29)?;
            if row.ty != source_type
                || row.inactive.is_some()
                || view.source >= node
                || view.count < 2
                || view.child_count == 0
                || !matches!(parent.kind, SourceReferenceNodeKindV29::Enum { .. })
            {
                return Err(source_reference_enum_error_v29());
            }
            let path = plan
                .projections
                .get(view.first..argument_sum_v1(&[view.first, view.count])?)
                .ok_or_else(source_reference_enum_error_v29)?;
            if !matches!(path[0].kind(), SemanticProjectionKindV1::Downcast(_))
                || path[0].result_type() != parent.ty
                || path.last().map(|step| step.result_type()) != Some(source_type)
            {
                return Err(source_reference_enum_error_v29());
            }
            let mut common = None;
            for offset in 0..view.child_count {
                budget.source_reference_charge_v29(plan, 3)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[view.children, offset])?)
                    .ok_or_else(source_reference_enum_error_v29)?;
                let child_row = plan
                    .nodes
                    .get(child)
                    .ok_or_else(source_reference_enum_error_v29)?;
                if child >= node || child_row.ty != source_type || child_row.inactive.is_some() {
                    return Err(source_reference_enum_error_v29());
                }
                let child_origin = match child_row.kind {
                    SourceReferenceNodeKindV29::Loan(loan) => {
                        SourceReferenceBindingOriginV29::SingleLoan(loan)
                    }
                    SourceReferenceNodeKindV29::EnumView(_) => {
                        SourceReferenceBindingOriginV29::EnumView(child)
                    }
                    _ => return Err(source_reference_enum_error_v29()),
                };
                let candidate = source_reference_binding_origin_types_v29(
                    plan,
                    child_origin,
                    source_type,
                    nodes,
                    budget,
                )?;
                if let Some(previous) = &common {
                    budget.source_reference_charge_v29(plan, candidate.len())?;
                    if previous != &candidate {
                        return Err(source_reference_error_v29(
                            "correlated reference payloads require one selected physical schema",
                        ));
                    }
                } else {
                    common = Some(candidate);
                }
            }
            common.ok_or_else(source_reference_enum_error_v29)
        }
    }
}

fn source_reference_enum_single_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(Type, SourceReferenceEnumAlternativeV29), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 6)?;
    let row = plan
        .nodes
        .get(node)
        .ok_or_else(source_reference_enum_error_v29)?;
    let SourceReferenceNodeKindV29::Enum { first, count: 1 } = row.kind else {
        return Err(source_reference_error_v29(
            "correlated enum alternatives require selected object transport",
        ));
    };
    let member = *plan
        .enum_members
        .get(first)
        .ok_or_else(source_reference_enum_error_v29)?;
    let alternative = *plan
        .enum_alternatives
        .get(member)
        .ok_or_else(source_reference_enum_error_v29)?;
    let types = plan.instances.owner().source_semantic().types();
    let SemanticTypeShapeV1::Enum {
        discriminant,
        variants,
    } = types[row.ty.index() as usize].shape()
    else {
        return Err(source_reference_enum_error_v29());
    };
    let fields = variants
        .get(alternative.variant as usize)
        .filter(|variant| !variant.is_uninhabited())
        .ok_or_else(source_reference_enum_error_v29)?
        .fields()
        .fields();
    if alternative.ty != row.ty || alternative.opaque.is_some() || alternative.count != fields.len()
    {
        return Err(source_reference_enum_error_v29());
    }
    for (offset, ty) in fields.iter().enumerate() {
        budget.source_reference_charge_v29(plan, 2)?;
        let child = *plan
            .children
            .get(argument_sum_v1(&[alternative.first, offset])?)
            .ok_or_else(source_reference_enum_error_v29)?;
        if child >= node || plan.nodes[child].ty != *ty {
            return Err(source_reference_enum_error_v29());
        }
    }
    Ok((lower_scalar_type(types, *discriminant)?, alternative))
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn refine_discriminant_edge(
        &mut self,
        site: SourceReferenceSiteV29,
        edge: SourceReferenceCfgEdgeV29,
        discriminant: usize,
        output: &SourceReferenceCfgStateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceReferenceCfgStateV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let SourceReferenceNodeKindV29::Discriminant(observation) =
            self.plan.nodes[discriminant].kind
        else {
            return Ok(None);
        };
        budget.reserve_storage(std::mem::size_of::<SourceReferenceEnumEdgeV29>())?;
        let receipt = SourceReferenceEnumEdgeV29 {
            site,
            edge,
            discriminant,
            observation,
        };
        let observation = *self
            .plan
            .enum_observations
            .get(observation)
            .ok_or_else(source_reference_enum_error_v29)?;
        let copy = self.cfg_clone(output, budget)?;
        self.cfg_install(&copy, budget)?;
        let mut local = self.local(observation.instance, observation.local)?;
        if local.generation != observation.generation {
            return Err(source_reference_enum_error_v29());
        }
        let snapshot =
            self.storage_snapshot(local.storage.ok_or_else(source_reference_enum_error_v29)?)?;
        let root = self
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let refined = root
            .snapshot_discriminant_edge(snapshot, &self.plan, &receipt, budget)
            .map_err(|error| self.storage_error(error))?;
        let Some(refined) = refined else {
            return Ok(None);
        };
        local.storage = Some(self.retain_storage_snapshot(refined, budget)?);
        self.set_local(observation.instance, observation.local, local)?;
        self.cfg_capture(budget).map(Some)
    }

    fn invalidate_discriminant_values(
        &mut self,
        instance: Option<ProductionCallInstanceIdV1>,
        local: Option<SemanticLocalIdV1>,
        result: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if self.plan.enum_observations.is_empty() {
            return Ok(result);
        }
        self.with_memo_v29(budget, |this, memo, budget| {
            for frame in 0..this.frames.len() {
                budget.charge_work(1)?;
                let Some(state) = this.frames[frame] else {
                    continue;
                };
                for ordinal in 0..this.plan.states[state].len() {
                    budget.charge_work(1)?;
                    if let Some(node) = this.plan.states[state][ordinal].node {
                        let changed = this
                            .invalidate_discriminant_node(node, instance, local, 0, memo, budget)?;
                        this.plan.states[state][ordinal].node = Some(changed);
                    }
                }
            }
            result
                .map(|node| {
                    this.invalidate_discriminant_node(node, instance, local, 0, memo, budget)
                })
                .transpose()
        })
    }

    fn invalidate_discriminant_node(
        &mut self,
        node: usize,
        instance: Option<ProductionCallInstanceIdV1>,
        local: Option<SemanticLocalIdV1>,
        depth: usize,
        memo: &mut SourceReferenceMemoV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        if depth >= 256 {
            return Err(source_reference_enum_error_v29());
        }
        charge_execution_cfg_lookup_v29(memo.len(), budget)?;
        if let Some(&known) = memo.get(&node) {
            return Ok(known);
        }
        budget.charge_work(2)?;
        let row = self.plan.nodes[node];
        let mut descriptor = row.descriptor;
        let kind = match row.kind {
            SourceReferenceNodeKindV29::Discriminant(index) => {
                let observation = self
                    .plan
                    .enum_observations
                    .get(index)
                    .ok_or_else(source_reference_enum_error_v29)?;
                if instance.is_none_or(|value| value == observation.instance)
                    && local.is_none_or(|value| value == observation.local)
                {
                    SourceReferenceNodeKindV29::Plain(None)
                } else {
                    row.kind
                }
            }
            SourceReferenceNodeKindV29::Aggregate { first, count } => {
                let mut children = source_reference_scratch_v29(count, budget)?;
                let mut changed = false;
                for offset in 0..count {
                    let child = *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[first, offset])?)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    if child >= node {
                        return Err(source_reference_enum_error_v29());
                    }
                    let next = self.invalidate_discriminant_node(
                        child,
                        instance,
                        local,
                        depth + 1,
                        memo,
                        budget,
                    )?;
                    changed |= child != next;
                    children.push(next);
                }
                if changed {
                    let first = self.plan.children.len();
                    for child in children {
                        emission_push_v1(&mut self.plan.children, child, budget)?;
                    }
                    SourceReferenceNodeKindV29::Aggregate { first, count }
                } else {
                    row.kind
                }
            }
            SourceReferenceNodeKindV29::Enum { first, count } => {
                let mut alternatives = source_reference_scratch_v29(count, budget)?;
                for offset in 0..count {
                    let member = self.plan.enum_member(first, count, offset, budget)?;
                    let alternative = self.plan.enum_alternative(member, budget)?;
                    if let Some(source) = alternative.opaque {
                        if source >= node {
                            return Err(source_reference_enum_error_v29());
                        }
                        Self::push_enum_member(&mut alternatives, member, budget)?;
                        continue;
                    }
                    let mut children = source_reference_scratch_v29(alternative.count, budget)?;
                    for field in 0..alternative.count {
                        let child = *self
                            .plan
                            .children
                            .get(argument_sum_v1(&[alternative.first, field])?)
                            .ok_or_else(source_reference_enum_error_v29)?;
                        if child >= node {
                            return Err(source_reference_enum_error_v29());
                        }
                        children.push(self.invalidate_discriminant_node(
                            child,
                            instance,
                            local,
                            depth + 1,
                            memo,
                            budget,
                        )?);
                    }
                    let member = self.intern_enum_alternative(
                        row.ty,
                        alternative.variant,
                        &children,
                        budget,
                    )?;
                    Self::push_enum_member(&mut alternatives, member, budget)?;
                }
                Self::canonicalize_enum_members(&mut alternatives, budget)?;
                let next = self.intern_enum_value(row.ty, &alternatives, budget)?;
                self.plan.nodes[next].kind
            }
            SourceReferenceNodeKindV29::EnumView(view) => {
                let view = *self
                    .plan
                    .enum_views
                    .get(view)
                    .ok_or_else(source_reference_enum_error_v29)?;
                if view.source >= node {
                    return Err(source_reference_enum_error_v29());
                }
                let source = self.invalidate_discriminant_node(
                    view.source,
                    instance,
                    local,
                    depth + 1,
                    memo,
                    budget,
                )?;
                let next = self.rebuild_enum_view(source, view, budget)?;
                descriptor = self.plan.nodes[next].descriptor;
                self.plan.nodes[next].kind
            }
            SourceReferenceNodeKindV29::Absent
            | SourceReferenceNodeKindV29::Plain(_)
            | SourceReferenceNodeKindV29::Loan(_)
            | SourceReferenceNodeKindV29::Address(_) => row.kind,
        };
        let next = if kind == row.kind && descriptor == row.descriptor {
            node
        } else {
            let next = self.plan.nodes.len();
            emission_push_v1(
                &mut self.plan.nodes,
                SourceReferenceNodeV29 {
                    kind,
                    descriptor,
                    value_origin: None,
                    ..row
                },
                budget,
            )?;
            next
        };
        memo.insert(node, next, budget)?;
        Ok(next)
    }

    fn check_storage_discriminant(
        &self,
        place: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(index) = self.local(place.instance, place.local)?.storage else {
            return Err(source_reference_enum_error_v29());
        };
        budget.charge_work(place.projections.len())?;
        if place.selector_source.is_some()
            || place
                .projections
                .iter()
                .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Err(source_reference_enum_error_v29());
        }
        let initialized = self
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?
            .snapshot_discriminant_initialized(
                self.storage_snapshot(index)?,
                &place.projections,
                budget,
            )
            .map_err(|error| self.storage_error(error))?;
        if !initialized {
            return Err(source_reference_error_v29(
                "source discriminant reads an uninitialized or invalid tag",
            ));
        }
        Ok(())
    }

    fn observe_enum_discriminant(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let resolved = self.resolve_reference_place(
            site,
            place,
            SourceReferenceAccessV29::ReadDiscriminant,
            budget,
        )?;
        if let Some(loan) = resolved.loan {
            self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
        }
        for (index, observation) in self.plan.enum_observations.iter().enumerate() {
            budget.charge_work(8)?;
            if observation.site != site
                || observation.source != place as *const SemanticPlaceV1 as usize
                || observation.instance != resolved.instance
                || observation.local != resolved.local
                || observation.generation != resolved.generation
                || observation.ty != place.ty()
                || observation.count != resolved.projections.len()
            {
                continue;
            }
            budget.charge_work(observation.count)?;
            if self
                .plan
                .projections
                .get(observation.first..argument_sum_v1(&[observation.first, observation.count])?)
                != Some(resolved.projections.as_slice())
            {
                continue;
            }
            return self.node(ty, SourceReferenceNodeKindV29::Discriminant(index), budget);
        }
        let first = self.plan.projections.len();
        for &projection in &resolved.projections {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        let index = self.plan.enum_observations.len();
        emission_push_v1(
            &mut self.plan.enum_observations,
            SourceReferenceEnumObservationV29 {
                site,
                source: place as *const SemanticPlaceV1 as usize,
                instance: resolved.instance,
                local: resolved.local,
                generation: resolved.generation,
                ty: place.ty(),
                first,
                count: resolved.projections.len(),
            },
            budget,
        )?;
        self.node(ty, SourceReferenceNodeKindV29::Discriminant(index), budget)
    }

    fn expire_enum_address_node(
        &mut self,
        node: usize,
        instance: ProductionCallInstanceIdV1,
        local: Option<SemanticLocalIdV1>,
        depth: usize,
        memo: &mut SourceReferenceMemoV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let row = self.plan.nodes[node];
        match row.kind {
            SourceReferenceNodeKindV29::Enum { first, count } => {
                let mut alternatives = source_reference_scratch_v29(count, budget)?;
                for offset in 0..count {
                    let member = self.plan.enum_member(first, count, offset, budget)?;
                    let alternative = self.plan.enum_alternative(member, budget)?;
                    if let Some(source) = alternative.opaque {
                        if source >= node {
                            return Err(source_reference_enum_error_v29());
                        }
                        Self::push_enum_member(&mut alternatives, member, budget)?;
                        continue;
                    }
                    let mut children = source_reference_scratch_v29(alternative.count, budget)?;
                    for field in 0..alternative.count {
                        budget.charge_work(1)?;
                        let child = *self
                            .plan
                            .children
                            .get(argument_sum_v1(&[alternative.first, field])?)
                            .ok_or_else(source_reference_enum_error_v29)?;
                        if child >= node {
                            return Err(source_reference_enum_error_v29());
                        }
                        children.push(self.expire_address_node(
                            child,
                            instance,
                            local,
                            depth + 1,
                            memo,
                            budget,
                        )?);
                    }
                    let member = self.intern_enum_alternative(
                        row.ty,
                        alternative.variant,
                        &children,
                        budget,
                    )?;
                    Self::push_enum_member(&mut alternatives, member, budget)?;
                }
                Self::canonicalize_enum_members(&mut alternatives, budget)?;
                self.intern_enum_value(row.ty, &alternatives, budget)
            }
            SourceReferenceNodeKindV29::EnumView(view) => {
                let view = *self
                    .plan
                    .enum_views
                    .get(view)
                    .ok_or_else(source_reference_enum_error_v29)?;
                if view.source >= node {
                    return Err(source_reference_enum_error_v29());
                }
                let source = self.expire_address_node(
                    view.source,
                    instance,
                    local,
                    depth + 1,
                    memo,
                    budget,
                )?;
                self.rebuild_enum_view(source, view, budget)
            }
            _ => Err(source_reference_enum_error_v29()),
        }
    }

    fn replace_enum_path(
        &mut self,
        current: usize,
        path: &[SemanticProjectionV1],
        expected: usize,
        replacement: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if depth >= 256
            || path.is_empty()
            || self.plan.nodes.get(expected).map(|node| node.ty)
                != self.plan.nodes.get(replacement).map(|node| node.ty)
        {
            return Err(source_reference_enum_error_v29());
        }
        let resolved = self.enum_path_node(current, path, budget)?;
        if !self.nodes_equal(resolved, expected, 0, budget)? {
            return Err(source_reference_enum_error_v29());
        }
        self.replace_enum_path_inner(current, path, replacement, depth, budget)
    }

    fn rebuild_enum_view(
        &mut self,
        source: usize,
        view: SourceReferenceEnumViewV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let mut path = source_reference_scratch_v29(view.count, budget)?;
        for offset in 0..view.count {
            budget.charge_work(1)?;
            path.push(
                *self
                    .plan
                    .projections
                    .get(argument_sum_v1(&[view.first, offset])?)
                    .ok_or_else(source_reference_enum_error_v29)?,
            );
        }
        self.enum_path_node(source, &path, budget)
    }

    fn enum_path_node(
        &mut self,
        mut node: usize,
        path: &[SemanticProjectionV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        for &projection in path {
            budget.charge_work(2)?;
            node = match (projection.kind(), self.plan.nodes[node].kind) {
                (SemanticProjectionKindV1::Downcast(_), SourceReferenceNodeKindV29::Plain(_)) => {
                    let expanded = self.expand_plain_enum(node, budget)?;
                    self.project_enum_value(expanded, projection, budget)?
                }
                (_, SourceReferenceNodeKindV29::EnumView(_))
                | (
                    SemanticProjectionKindV1::Downcast(_),
                    SourceReferenceNodeKindV29::Enum { .. },
                ) => self.project_enum_value(node, projection, budget)?,
                (
                    SemanticProjectionKindV1::Field(field),
                    SourceReferenceNodeKindV29::Aggregate { .. },
                ) => self.field(node, field as usize, budget)?,
                (
                    SemanticProjectionKindV1::ConstantIndex { .. },
                    SourceReferenceNodeKindV29::Aggregate { .. },
                ) => self.array_projection_node(node, projection, budget)?,
                (SemanticProjectionKindV1::Field(field), SourceReferenceNodeKindV29::Plain(_)) => {
                    let ty = self.ordinary_reference_field_type(node, field as usize, budget)?;
                    if ty != projection.result_type() {
                        return Err(source_reference_enum_error_v29());
                    }
                    self.plain(ty, budget)?
                }
                _ => return Err(source_reference_enum_error_v29()),
            };
            if self.plan.nodes[node].ty != projection.result_type() {
                return Err(source_reference_enum_error_v29());
            }
        }
        Ok(node)
    }

    fn replace_enum_path_inner(
        &mut self,
        original: usize,
        path: &[SemanticProjectionV1],
        replacement: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if depth >= 256 {
            return Err(source_reference_enum_error_v29());
        }
        let row = *self
            .plan
            .nodes
            .get(original)
            .ok_or_else(source_reference_enum_error_v29)?;
        if path.is_empty() {
            if row.ty != self.plan.nodes[replacement].ty {
                return Err(source_reference_enum_error_v29());
            }
            return Ok(replacement);
        }
        let projection = path[0];
        if let SemanticProjectionKindV1::Downcast(variant) = projection.kind() {
            if projection.result_type() != row.ty || path.len() < 2 {
                return Err(source_reference_enum_error_v29());
            }
            let current = if matches!(row.kind, SourceReferenceNodeKindV29::Plain(_)) {
                self.expand_plain_enum(original, budget)?
            } else {
                original
            };
            let SourceReferenceNodeKindV29::Enum { first, count } = self.plan.nodes[current].kind
            else {
                return Err(source_reference_enum_error_v29());
            };
            let SemanticProjectionKindV1::Field(field) = path[1].kind() else {
                return Err(source_reference_enum_error_v29());
            };
            let fields = self.plan.enum_variant_fields(row.ty, variant, budget)?;
            if fields.get(field as usize) != Some(&path[1].result_type()) {
                return Err(source_reference_enum_error_v29());
            }
            let mut alternatives = source_reference_scratch_v29(count, budget)?;
            let mut matched = false;
            for offset in 0..count {
                let member = self.plan.enum_member(first, count, offset, budget)?;
                let alternative = self.plan.enum_alternative(member, budget)?;
                let next = if alternative.variant == variant {
                    matched = true;
                    let field_count = self
                        .plan
                        .enum_variant_fields(row.ty, variant, budget)?
                        .len();
                    if let Some(source) = alternative.opaque {
                        if source >= current {
                            return Err(source_reference_enum_error_v29());
                        }
                    } else if alternative.count != field_count {
                        return Err(source_reference_enum_error_v29());
                    }
                    let mut children = source_reference_scratch_v29(field_count, budget)?;
                    for ordinal in 0..field_count {
                        budget.charge_work(2)?;
                        let child = if alternative.opaque.is_some() {
                            let ty =
                                self.plan.enum_variant_fields(row.ty, variant, budget)?[ordinal];
                            self.plain(ty, budget)?
                        } else {
                            *self
                                .plan
                                .children
                                .get(argument_sum_v1(&[alternative.first, ordinal])?)
                                .ok_or_else(source_reference_enum_error_v29)?
                        };
                        let child = if ordinal == field as usize {
                            self.replace_enum_path_inner(
                                child,
                                &path[2..],
                                replacement,
                                depth + 2,
                                budget,
                            )?
                        } else {
                            child
                        };
                        children.push(child);
                    }
                    self.intern_enum_alternative(row.ty, variant, &children, budget)?
                } else {
                    member
                };
                Self::push_enum_member(&mut alternatives, next, budget)?;
            }
            if !matched {
                return Err(source_reference_enum_error_v29());
            }
            Self::canonicalize_enum_members(&mut alternatives, budget)?;
            return self.intern_enum_value(row.ty, &alternatives, budget);
        }
        let current = if matches!(row.kind, SourceReferenceNodeKindV29::Plain(_)) {
            self.expand_plain_reference_fields(original, budget)?
        } else {
            original
        };
        let SourceReferenceNodeKindV29::Aggregate { first, count } = self.plan.nodes[current].kind
        else {
            return Err(source_reference_enum_error_v29());
        };
        let field = match projection.kind() {
            SemanticProjectionKindV1::Field(field) => field as usize,
            SemanticProjectionKindV1::ConstantIndex {
                offset, from_end, ..
            } => {
                self.array_projection_node(current, projection, budget)?;
                let SemanticTypeShapeV1::Array { length, .. } =
                    self.plan.instances.owner().source_semantic().types()[row.ty.index() as usize]
                        .shape()
                else {
                    return Err(source_reference_enum_error_v29());
                };
                usize::try_from(if from_end {
                    length
                        .checked_sub(offset)
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                } else {
                    offset
                })
                .map_err(|_| ArgumentResourceV1::Arithmetic)?
            }
            _ => return Err(source_reference_enum_error_v29()),
        };
        if field >= count {
            return Err(source_reference_enum_error_v29());
        }
        let selected = self.plan.children[argument_sum_v1(&[first, field])?];
        if self.plan.nodes[selected].ty != projection.result_type() {
            return Err(source_reference_enum_error_v29());
        }
        let selected =
            self.replace_enum_path_inner(selected, &path[1..], replacement, depth + 1, budget)?;
        let start = self.plan.children.len();
        for ordinal in 0..count {
            budget.charge_work(1)?;
            let child = if ordinal == field {
                selected
            } else {
                self.plan.children[argument_sum_v1(&[first, ordinal])?]
            };
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        self.node(
            row.ty,
            SourceReferenceNodeKindV29::Aggregate {
                first: start,
                count,
            },
            budget,
        )
    }

    fn transfer_enum_storage_value(
        &mut self,
        target: &SourceReferencePlaceV29,
        node: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let value = self.plan.nodes[node];
        let SourceReferenceNodeKindV29::Enum { first, count } = value.kind else {
            return Err(source_reference_enum_error_v29());
        };
        let before = self.local(target.instance, target.local)?;
        let mut result = None;
        for offset in 0..count {
            self.set_local(target.instance, target.local, before)?;
            let member = self.plan.enum_member(first, count, offset, budget)?;
            let alternative = self.plan.enum_alternative(member, budget)?;
            if let Some(source) = alternative.opaque {
                if source >= node || self.plan.nodes[source].storage.is_none() {
                    return Err(source_reference_error_v29(
                        "opaque enum transfer requires the original typed value snapshot",
                    ));
                }
                self.transfer_storage_value_at_depth(target, source, depth + 1, budget)?;
                let after = self.local(target.instance, target.local)?.storage;
                result = if offset == 0 {
                    after
                } else {
                    self.join_storage_states(result, after, budget)?
                };
                continue;
            }
            self.mutate_storage_place(
                target,
                source_storage_v29::SourceStorageRootMutationV29::BeginVariant(alternative.variant),
                budget,
            )?;
            for field in 0..alternative.count {
                budget.charge_work(5)?;
                let child = *self
                    .plan
                    .children
                    .get(argument_sum_v1(&[alternative.first, field])?)
                    .ok_or_else(source_reference_enum_error_v29)?;
                if child >= node {
                    return Err(source_reference_enum_error_v29());
                }
                let child_ty = self.plan.nodes[child].ty;
                if self
                    .plan
                    .enum_variant_fields(value.ty, alternative.variant, budget)?
                    .get(field)
                    != Some(&child_ty)
                {
                    return Err(source_reference_enum_error_v29());
                }
                budget.reserve_storage(std::mem::size_of::<SourceReferencePlaceV29>())?;
                let mut projections = source_reference_scratch_v29(
                    argument_sum_v1(&[target.projections.len(), 2])?,
                    budget,
                )?;
                budget.charge_work(target.projections.len())?;
                projections.extend_from_slice(&target.projections);
                projections.push(
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Downcast(alternative.variant),
                        value.ty,
                    )
                    .map_err(|_| ArgumentResourceV1::Accounting)?,
                );
                projections.push(
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(
                            u32::try_from(field).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        child_ty,
                    )
                    .map_err(|_| ArgumentResourceV1::Accounting)?,
                );
                let child_target = SourceReferencePlaceV29 {
                    selector_source: target.selector_source,
                    instance: target.instance,
                    local: target.local,
                    generation: target.generation,
                    value: target.value,
                    representation_root: target.representation_root,
                    node: child,
                    projections,
                    anchor: None,
                    loan: target.loan,
                    shared_path: target.shared_path,
                    traversed: Vec::new(),
                };
                self.transfer_storage_value_at_depth(&child_target, child, depth + 1, budget)?;
            }
            self.mutate_storage_place(
                target,
                source_storage_v29::SourceStorageRootMutationV29::SetDiscriminant(
                    alternative.variant,
                ),
                budget,
            )?;
            let after = self.local(target.instance, target.local)?.storage;
            result = if offset == 0 {
                after
            } else {
                self.join_storage_states(result, after, budget)?
            };
        }
        let mut after = before;
        after.storage = result;
        self.set_local(target.instance, target.local, after)
    }
}
