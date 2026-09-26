// Alternatives are complete payload states. A variant key is not an active-tag proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceEnumAlternativeV29 {
    ty: SemanticTypeIdV1,
    variant: u32,
    opaque: Option<usize>,
    first: usize,
    count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceEnumViewV29 {
    source: usize,
    first: usize,
    count: usize,
    children: usize,
    child_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceEnumObservationV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    first: usize,
    count: usize,
}

struct SourceReferenceEnumEdgeV29 {
    site: SourceReferenceSiteV29,
    edge: SourceReferenceCfgEdgeV29,
    discriminant: usize,
    observation: usize,
}

fn source_reference_enum_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source enum payload differs from its original guarded value")
}

impl SourceReferencePlanV29<'_, '_> {
    fn enum_edge_variants(
        &self,
        receipt: &SourceReferenceEnumEdgeV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<u32>, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        budget.charge_work(12)?;
        let observation = self.enum_observations.get(receipt.observation)
            .ok_or_else(source_reference_enum_error_v29)?;
        let observed = self.instances.instance(observation.site.instance)
            .and_then(|instance| instance.declaration().blocks().get(observation.site.block.index() as usize))
            .and_then(|block| observation.site.statement.and_then(|index| block.statements().get(index)))
            .ok_or_else(source_reference_enum_error_v29)?;
        let SemanticStatementKindV1::Assign(assignment) = observed.kind() else {
            return Err(source_reference_enum_error_v29());
        };
        let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
            return Err(source_reference_enum_error_v29());
        };
        if place as *const SemanticPlaceV1 as usize != observation.source || place.ty() != observation.ty {
            return Err(source_reference_enum_error_v29());
        }
        let original = self.instances.instance(receipt.site.instance)
            .and_then(|instance| instance.declaration().blocks().get(receipt.site.block.index() as usize))
            .ok_or_else(source_reference_enum_error_v29)?;
        let SemanticTerminatorKindV1::SwitchInt { discriminant, targets } = original.terminator().kind() else {
            return Err(source_reference_enum_error_v29());
        };
        let node = self.nodes.get(receipt.discriminant).ok_or_else(source_reference_enum_error_v29)?;
        if receipt.site.statement.is_some()
            || node.kind != SourceReferenceNodeKindV29::Discriminant(receipt.observation)
            || node.ty != discriminant.ty() || node.ty != assignment.value().result_type()
        {
            return Err(source_reference_enum_error_v29());
        }
        let selected = targets.values().get(receipt.edge.ordinal);
        let edge = if let Some(selected) = selected {
            selected.edge()
        } else if receipt.edge.ordinal == targets.values().len() {
            targets.otherwise()
        } else {
            return Err(source_reference_enum_error_v29());
        };
        if edge.target().index() as usize != receipt.edge.target || edge.role() != receipt.edge.role {
            return Err(source_reference_enum_error_v29());
        }
        let SemanticTypeShapeV1::Enum { variants, .. } = self.instances.owner()
            .source_semantic().types()[observation.ty.index() as usize].shape() else {
            return Err(source_reference_enum_error_v29());
        };
        let mut allowed = source_reference_scratch_v29(variants.len(), budget)?;
        for (ordinal, variant) in variants.iter().enumerate() {
            budget.charge_work(2)?;
            if variant.is_uninhabited() { continue; }
            let included = if let Some(selected) = selected {
                selected.value() == variant.discriminant()
            } else {
                charge_execution_cfg_lookup_v29(targets.values().len(), budget)?;
                targets.values().binary_search_by_key(&variant.discriminant(), |target| target.value()).is_err()
            };
            if included {
                allowed.push(u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?);
            }
        }
        Ok(allowed)
    }

    fn enum_variant_fields(
        &self,
        ty: SemanticTypeIdV1,
        variant: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&[SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let declaration = self.instances.owner().source_semantic().types()
            .get(ty.index() as usize).ok_or_else(source_reference_enum_error_v29)?;
        let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
            return Err(source_reference_enum_error_v29());
        };
        let variant = variants.get(variant as usize).ok_or_else(source_reference_enum_error_v29)?;
        if variant.is_uninhabited() {
            return Err(source_reference_enum_error_v29());
        }
        Ok(variant.fields().fields())
    }

    fn enum_alternative(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceEnumAlternativeV29, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let alternative = self.enum_alternatives.get(ordinal).copied()
            .ok_or_else(source_reference_enum_error_v29)?;
        if let Some(source) = alternative.opaque {
            budget.charge_work(2)?;
            let row = self.nodes.get(source).ok_or_else(source_reference_enum_error_v29)?;
            if alternative.first != 0 || alternative.count != 0 || row.ty != alternative.ty
                || !matches!(row.kind, SourceReferenceNodeKindV29::Plain(_)) || row.inactive.is_some()
            {
                return Err(source_reference_enum_error_v29());
            }
        }
        Ok(alternative)
    }

    fn enum_member(
        &self,
        first: usize,
        count: usize,
        offset: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if offset >= count {
            return Err(source_reference_enum_error_v29());
        }
        self.enum_members.get(argument_sum_v1(&[first, offset])?).copied()
            .ok_or_else(source_reference_enum_error_v29)
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn push_enum_member(
        members: &mut Vec<usize>,
        member: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if members.len() == members.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        members.push(member);
        Ok(())
    }

    fn canonicalize_enum_members(
        members: &mut Vec<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        fn sift(
            members: &mut [usize],
            mut root: usize,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            loop {
                budget.charge_work(8)?;
                let left = argument_sum_v1(&[argument_product_v1(root, 2)?, 1])?;
                if left >= members.len() { return Ok(()); }
                let right = argument_sum_v1(&[left, 1])?;
                let child = if right < members.len() && members[right] > members[left] { right } else { left };
                if members[root] >= members[child] { return Ok(()); }
                members.swap(root, child);
                root = child;
            }
        }
        for root in (0..members.len() / 2).rev() {
            sift(members, root, budget)?;
        }
        for end in (1..members.len()).rev() {
            budget.charge_work(1)?;
            members.swap(0, end);
            sift(&mut members[..end], 0, budget)?;
        }
        let mut kept = 0;
        for read in 0..members.len() {
            budget.charge_work(2)?;
            if kept == 0 || members[kept - 1] != members[read] {
                members[kept] = members[read];
                kept += 1;
            }
        }
        members.truncate(kept);
        Ok(())
    }

    fn expand_plain_enum(
        &mut self,
        original: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let row = *self.plan.nodes.get(original).ok_or_else(source_reference_enum_error_v29)?;
        if !matches!(row.kind, SourceReferenceNodeKindV29::Plain(_)) || row.inactive.is_some() {
            return Err(source_reference_enum_error_v29());
        }
        let SemanticTypeShapeV1::Enum { variants, .. } = self.plan.instances.owner()
            .source_semantic().types()[row.ty.index() as usize].shape() else {
            return Err(source_reference_enum_error_v29());
        };
        let count = variants.len();
        let mut alternatives = source_reference_scratch_v29(count, budget)?;
        for variant in 0..count {
            budget.charge_work(2)?;
            let variant = u32::try_from(variant).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let SemanticTypeShapeV1::Enum { variants, .. } = self.plan.instances.owner()
                .source_semantic().types()[row.ty.index() as usize].shape() else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            if variants[variant as usize].is_uninhabited() {
                continue;
            }
            let member = self.intern_opaque_enum_alternative(original, variant, budget)?;
            Self::push_enum_member(&mut alternatives, member, budget)?;
        }
        Self::canonicalize_enum_members(&mut alternatives, budget)?;
        self.intern_enum_value(row.ty, &alternatives, budget)
    }

    fn project_enum_value(
        &mut self,
        original: usize,
        projection: SemanticProjectionV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let row = *self.plan.nodes.get(original).ok_or_else(source_reference_enum_error_v29)?;
        let (source, old) = match row.kind {
            SourceReferenceNodeKindV29::Enum { .. } => (original, None),
            SourceReferenceNodeKindV29::EnumView(view) => {
                let view = *self.plan.enum_views.get(view).ok_or_else(source_reference_enum_error_v29)?;
                (view.source, Some(view))
            }
            _ => return Err(source_reference_enum_error_v29()),
        };
        let count = argument_sum_v1(&[old.map_or(0, |view| view.count), 1])?;
        if count > 256 {
            return Err(source_reference_enum_error_v29());
        }
        let mut path = source_reference_scratch_v29(count, budget)?;
        if let Some(view) = old {
            let end = argument_sum_v1(&[view.first, view.count])?;
            let previous = self.plan.projections.get(view.first..end)
                .ok_or_else(source_reference_enum_error_v29)?;
            budget.charge_work(previous.len())?;
            path.extend_from_slice(previous);
        }
        path.push(projection);
        // The downcast remains part of the view: it is not a tuple conversion or
        // permission to read a payload. C2 checks the complete original path.
        let ty = projection.result_type();
        let candidates = self.enum_projected_nodes(source, &path, budget)?;
        if candidates.len() == 1 && !matches!(projection.kind(), SemanticProjectionKindV1::Downcast(_)) {
            return Ok(candidates[0]);
        }
        charge_execution_cfg_lookup_v29(self.plan.enum_view_nodes.len(), budget)?;
        if let Some(nodes) = self.plan.enum_view_nodes.get(&source) {
            'candidate: for &node in nodes {
                budget.charge_work(2)?;
                let SourceReferenceNodeKindV29::EnumView(view) = self.plan.nodes[node].kind else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                let view = self.plan.enum_views[view];
                if view.count != path.len() || self.plan.nodes[node].ty != ty {
                    continue;
                }
                for (ordinal, projection) in path.iter().enumerate() {
                    budget.charge_work(1)?;
                    if self.plan.projections.get(argument_sum_v1(&[view.first, ordinal])?) != Some(projection) {
                        continue 'candidate;
                    }
                }
                return Ok(node);
            }
        }
        let mut descriptor = candidates.first().and_then(|&child| self.plan.nodes[child].descriptor);
        for &child in candidates.iter().skip(1) {
            budget.charge_work(1)?;
            descriptor = self.merge_descriptor_values(ty, descriptor, self.plan.nodes[child].descriptor, budget)?;
        }
        let first = self.plan.projections.len();
        for projection in path {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        let children = self.plan.children.len();
        for &child in &candidates {
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        let view = self.plan.enum_views.len();
        emission_push_v1(&mut self.plan.enum_views, SourceReferenceEnumViewV29 {
            source, first, count, children, child_count: candidates.len(),
        }, budget)?;
        let node = self.node(ty, SourceReferenceNodeKindV29::EnumView(view), budget)?;
        self.plan.nodes[node].descriptor = descriptor;
        charge_execution_cfg_lookup_v29(self.plan.enum_view_nodes.len(), budget)?;
        if !self.plan.enum_view_nodes.contains_key(&source) {
            reserve_execution_cfg_map_entry_v29::<usize, Vec<usize>>(
                self.plan.enum_view_nodes.len(), budget)?;
            self.plan.enum_view_nodes.insert(source, Vec::new());
        }
        emission_push_v1(self.plan.enum_view_nodes.get_mut(&source)
            .ok_or(ArgumentResourceV1::Accounting)?, node, budget)?;
        Ok(node)
    }

    fn enum_projected_nodes(
        &mut self,
        source: usize,
        path: &[SemanticProjectionV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<usize>, ProductionSemanticKirErrorV1> {
        let row = *self.plan.nodes.get(source).ok_or_else(source_reference_enum_error_v29)?;
        let SourceReferenceNodeKindV29::Enum { first, count } = row.kind else {
            return Err(source_reference_enum_error_v29());
        };
        let Some(projection) = path.first() else {
            return Err(source_reference_enum_error_v29());
        };
        let SemanticProjectionKindV1::Downcast(variant) = projection.kind() else {
            return Err(source_reference_enum_error_v29());
        };
        if projection.result_type() != row.ty {
            return Err(source_reference_enum_error_v29());
        }
        self.plan.enum_variant_fields(row.ty, variant, budget)?;
        let mut candidates = source_reference_scratch_v29(count, budget)?;
        for offset in 0..count {
            let member = self.plan.enum_member(first, count, offset, budget)?;
            let alternative = self.plan.enum_alternative(member, budget)?;
            if alternative.variant != variant {
                continue;
            }
            if path.len() == 1 {
                // An incomplete Downcast is a transient typed view only.
                continue;
            }
            let SemanticProjectionKindV1::Field(field) = path[1].kind() else {
                return Err(source_reference_enum_error_v29());
            };
            let fields = self.plan.enum_variant_fields(row.ty, variant, budget)?;
            if fields.get(field as usize) != Some(&path[1].result_type()) {
                return Err(source_reference_enum_error_v29());
            }
            let mut child = if let Some(opaque) = alternative.opaque {
                if opaque >= source { return Err(source_reference_enum_error_v29()); }
                self.plain(path[1].result_type(), budget)?
            } else {
                if field as usize >= alternative.count { return Err(source_reference_enum_error_v29()); }
                *self.plan.children.get(argument_sum_v1(&[alternative.first, field as usize])?)
                    .ok_or_else(source_reference_enum_error_v29)?
            };
            if self.plan.nodes[child].ty != path[1].result_type() {
                return Err(source_reference_enum_error_v29());
            }
            for &projection in &path[2..] {
                budget.charge_work(2)?;
                child = match (projection.kind(), self.plan.nodes[child].kind) {
                    (_, SourceReferenceNodeKindV29::EnumView(_))
                    | (SemanticProjectionKindV1::Downcast(_), SourceReferenceNodeKindV29::Enum { .. }) => {
                        self.project_enum_value(child, projection, budget)?
                    }
                    (SemanticProjectionKindV1::Field(field), SourceReferenceNodeKindV29::Aggregate { .. }) => {
                        self.field(child, field as usize, budget)?
                    }
                    (SemanticProjectionKindV1::ConstantIndex { .. }, SourceReferenceNodeKindV29::Aggregate { .. }) => {
                        self.array_projection_node(child, projection, budget)?
                    }
                    (_, SourceReferenceNodeKindV29::Plain(_)) => self.plain(projection.result_type(), budget)?,
                    _ => return Err(source_reference_enum_error_v29()),
                };
                if self.plan.nodes[child].ty != projection.result_type() {
                    return Err(source_reference_enum_error_v29());
                }
            }
            let mut equal = false;
            for &previous in &candidates {
                if self.nodes_equal(previous, child, 0, budget)? {
                    equal = true;
                    break;
                }
            }
            if !equal {
                candidates.push(child);
            }
        }
        if path.len() > 1 && candidates.is_empty() {
            return Err(source_reference_enum_error_v29());
        }
        Ok(candidates)
    }

    fn intern_enum_alternative(
        &mut self,
        ty: SemanticTypeIdV1,
        variant: u32,
        operands: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let fields = self.plan.enum_variant_fields(ty, variant, budget)?;
        if fields.len() != operands.len() {
            return Err(source_reference_enum_error_v29());
        }
        for (&node, &expected) in operands.iter().zip(fields) {
            budget.charge_work(2)?;
            if self.plan.nodes.get(node).map(|row| row.ty) != Some(expected) {
                return Err(source_reference_enum_error_v29());
            }
        }
        let key = (ty.index(), variant);
        charge_execution_cfg_lookup_v29(self.plan.enum_alternative_types.len(), budget)?;
        if let Some(candidates) = self.plan.enum_alternative_types.get(&key) {
            'candidate: for &id in candidates {
                let alternative = self.plan.enum_alternative(id, budget)?;
                budget.charge_work(2)?;
                if alternative.ty != ty || alternative.variant != variant {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                if alternative.opaque.is_some() { continue; }
                if alternative.count != operands.len() { return Err(ArgumentResourceV1::Accounting.into()); }
                for (field, &actual) in operands.iter().enumerate() {
                    budget.charge_work(1)?;
                    let existing = *self.plan.children
                        .get(argument_sum_v1(&[alternative.first, field])?)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    if !self.nodes_equal(existing, actual, 0, budget)? {
                        continue 'candidate;
                    }
                }
                return Ok(id);
            }
        }
        // No child copy precedes the canonical equality check.
        let first = self.plan.children.len();
        for &node in operands {
            emission_push_v1(&mut self.plan.children, node, budget)?;
        }
        let id = self.plan.enum_alternatives.len();
        emission_push_v1(&mut self.plan.enum_alternatives, SourceReferenceEnumAlternativeV29 {
            ty,
            variant,
            opaque: None,
            first,
            count: operands.len(),
        }, budget)?;
        charge_execution_cfg_lookup_v29(self.plan.enum_alternative_types.len(), budget)?;
        if !self.plan.enum_alternative_types.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<(u32, u32), Vec<usize>>(
                self.plan.enum_alternative_types.len(), budget)?;
            self.plan.enum_alternative_types.insert(key, Vec::new());
        }
        emission_push_v1(self.plan.enum_alternative_types.get_mut(&key)
            .ok_or(ArgumentResourceV1::Accounting)?, id, budget)?;
        Ok(id)
    }

    fn intern_opaque_enum_alternative(
        &mut self,
        original: usize,
        variant: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let row = *self.plan.nodes.get(original).ok_or_else(source_reference_enum_error_v29)?;
        if !matches!(row.kind, SourceReferenceNodeKindV29::Plain(_)) || row.inactive.is_some() {
            return Err(source_reference_enum_error_v29());
        }
        self.plan.enum_variant_fields(row.ty, variant, budget)?;
        let key = (row.ty.index(), variant);
        charge_execution_cfg_lookup_v29(self.plan.enum_alternative_types.len(), budget)?;
        if let Some(candidates) = self.plan.enum_alternative_types.get(&key) {
            for &id in candidates {
                let alternative = self.plan.enum_alternative(id, budget)?;
                if let Some(source) = alternative.opaque {
                    if self.nodes_equal(source, original, 0, budget)? { return Ok(id); }
                }
            }
        }
        let id = self.plan.enum_alternatives.len();
        emission_push_v1(&mut self.plan.enum_alternatives, SourceReferenceEnumAlternativeV29 {
            ty: row.ty, variant, opaque: Some(original), first: 0, count: 0,
        }, budget)?;
        charge_execution_cfg_lookup_v29(self.plan.enum_alternative_types.len(), budget)?;
        if !self.plan.enum_alternative_types.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<(u32, u32), Vec<usize>>(
                self.plan.enum_alternative_types.len(), budget)?;
            self.plan.enum_alternative_types.insert(key, Vec::new());
        }
        emission_push_v1(self.plan.enum_alternative_types.get_mut(&key)
            .ok_or(ArgumentResourceV1::Accounting)?, id, budget)?;
        Ok(id)
    }

    fn intern_enum_value(
        &mut self,
        ty: SemanticTypeIdV1,
        alternatives: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if alternatives.is_empty() {
            return Err(source_reference_enum_error_v29());
        }
        for (ordinal, &id) in alternatives.iter().enumerate() {
            let alternative = self.plan.enum_alternative(id, budget)?;
            if alternative.ty != ty || alternative.opaque.is_some_and(|source| source >= self.plan.nodes.len())
                || (ordinal != 0 && alternatives[ordinal - 1] >= id)
            {
                return Err(source_reference_enum_error_v29());
            }
        }
        charge_execution_cfg_lookup_v29(self.plan.enum_nodes.len(), budget)?;
        if let Some(candidates) = self.plan.enum_nodes.get(&ty.index()) {
            'candidate: for &node in candidates {
                budget.charge_work(2)?;
                let SourceReferenceNodeKindV29::Enum { first, count } = self.plan.nodes[node].kind else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                if count != alternatives.len() {
                    continue;
                }
                for (offset, &right) in alternatives.iter().enumerate() {
                    if self.plan.enum_member(first, count, offset, budget)? != right {
                        continue 'candidate;
                    }
                }
                return Ok(node);
            }
        }
        // Sorted canonical alternative IDs retain whole-state correlation.
        let first = self.plan.enum_members.len();
        for &alternative in alternatives {
            emission_push_v1(&mut self.plan.enum_members, alternative, budget)?;
        }
        let node = self.node(ty, SourceReferenceNodeKindV29::Enum {
            first,
            count: alternatives.len(),
        }, budget)?;
        charge_execution_cfg_lookup_v29(self.plan.enum_nodes.len(), budget)?;
        if !self.plan.enum_nodes.contains_key(&ty.index()) {
            reserve_execution_cfg_map_entry_v29::<u32, Vec<usize>>(self.plan.enum_nodes.len(), budget)?;
            self.plan.enum_nodes.insert(ty.index(), Vec::new());
        }
        emission_push_v1(self.plan.enum_nodes.get_mut(&ty.index())
            .ok_or(ArgumentResourceV1::Accounting)?, node, budget)?;
        Ok(node)
    }

    fn construct_enum_value(
        &mut self,
        ty: SemanticTypeIdV1,
        variant: u32,
        operands: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(std::mem::size_of::<[usize; 1]>())?;
        let alternative = self.intern_enum_alternative(ty, variant, operands, budget)?;
        self.intern_enum_value(ty, &[alternative], budget)
    }

    fn merge_enum_values(
        &mut self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let a = *self.plan.nodes.get(left).ok_or_else(source_reference_enum_error_v29)?;
        let b = *self.plan.nodes.get(right).ok_or_else(source_reference_enum_error_v29)?;
        let (SourceReferenceNodeKindV29::Enum { first: a_first, count: a_count },
            SourceReferenceNodeKindV29::Enum { first: b_first, count: b_count }) = (a.kind, b.kind)
        else {
            return Err(source_reference_enum_error_v29());
        };
        if a.ty != b.ty {
            return Err(source_reference_enum_error_v29());
        }
        let capacity = argument_sum_v1(&[a_count, b_count])?;
        let mut members = source_reference_scratch_v29(capacity, budget)?;
        let (mut ai, mut bi) = (0, 0);
        while ai < a_count || bi < b_count {
            budget.charge_work(3)?;
            let take_a = bi == b_count || (ai < a_count
                && self.plan.enum_member(a_first, a_count, ai, budget)?
                    <= self.plan.enum_member(b_first, b_count, bi, budget)?);
            let next = if take_a {
                let value = self.plan.enum_member(a_first, a_count, ai, budget)?;
                ai += 1;
                value
            } else {
                let value = self.plan.enum_member(b_first, b_count, bi, budget)?;
                bi += 1;
                value
            };
            if members.last() != Some(&next) {
                members.push(next);
            }
        }
        self.intern_enum_value(a.ty, &members, budget)
    }

    fn merge_enum_views(
        &mut self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let a = *self.plan.enum_views.get(left).ok_or_else(source_reference_enum_error_v29)?;
        let b = *self.plan.enum_views.get(right).ok_or_else(source_reference_enum_error_v29)?;
        if a.count != b.count || a.count == 0 { return Err(source_reference_enum_error_v29()); }
        let mut path = source_reference_scratch_v29(a.count, budget)?;
        for offset in 0..a.count {
            budget.charge_work(3)?;
            let projection = *self.plan.projections.get(argument_sum_v1(&[a.first, offset])?)
                .ok_or_else(source_reference_enum_error_v29)?;
            if self.plan.projections.get(argument_sum_v1(&[b.first, offset])?) != Some(&projection) {
                return Err(source_reference_cfg_obligation_v29());
            }
            path.push(projection);
        }
        // Union complete parents before projecting; independently unioning the
        // children would discard correlations with their sibling payloads.
        let value = self.merge_enum_values(a.source, b.source, budget)?;
        self.enum_path_node(value, &path, budget)
    }
}
