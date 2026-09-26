// C2 transfers at the existing C1 worklist's exact source sites. This adapter
// does not own a CFG, invent source uses, or infer a physical pointer permit.

// Schema-only input from actual writes, including transient representations.
// Each node retains its complete correlated alternatives and original objects.
struct SourceReferenceRepresentationDemandV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    projections: std::ops::Range<usize>,
    selector_source: Option<(SourceReferenceSiteV29, usize)>,
    node: usize,
}

fn source_reference_check_storage_demands_v29(
    instances: &ExecutionInstancesV29<'_>,
    requests: &[source_storage_demands_v29::DemandV29],
    paths: &[source_storage_demands_v29::ComponentStepV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use source_storage_demands_v29::{ComponentStepV29 as Step, DemandKindV29};
    let owner = instances.owner().source_semantic();
    let root = instances
        .instance(instances.root())
        .ok_or(ArgumentResourceV1::Accounting)?
        .function();
    for request in requests {
        budget.charge_work(6)?;
        let instance = instances
            .instance(request.instance)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let local = instance
            .declaration()
            .locals()
            .get(request.local.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if request.root != root
            || request.function != instance.function()
            || (request.kind != DemandKindV29::ReferenceHolder && !request.path.is_empty())
        {
            return Err(source_reference_error_v29(
                "source storage demand changed its original object",
            ));
        }
        let mut ty = local.ty();
        let mut variant = None;
        for step in paths
            .get(request.path.clone())
            .ok_or(ArgumentResourceV1::Accounting)?
        {
            budget.charge_work(3)?;
            let shape = owner
                .types()
                .get(ty.index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?
                .shape();
            match (step, shape, variant) {
                (Step::Variant(index), SemanticTypeShapeV1::Enum { variants, .. }, None)
                    if (*index as usize) < variants.len() =>
                {
                    variant = Some(*index)
                }
                (
                    Step::Field(index),
                    SemanticTypeShapeV1::Enum { variants, .. },
                    Some(selected),
                ) => {
                    ty = *variants
                        .get(selected as usize)
                        .and_then(|row| row.fields().fields().get(*index as usize))
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    variant = None;
                }
                (
                    Step::Field(index),
                    SemanticTypeShapeV1::Tuple(fields)
                    | SemanticTypeShapeV1::Aggregate(fields)
                    | SemanticTypeShapeV1::Union(fields),
                    None,
                ) => {
                    ty = *fields
                        .fields()
                        .get(*index as usize)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                }
                (
                    Step::Element,
                    SemanticTypeShapeV1::Array { element, .. }
                    | SemanticTypeShapeV1::Slice { element },
                    None,
                ) => ty = *element,
                _ => {
                    return Err(source_reference_error_v29(
                        "source storage demand changed its typed component path",
                    ));
                }
            }
        }
        if variant.is_some() || ty != request.ty {
            return Err(source_reference_error_v29(
                "source storage demand changed its final component type",
            ));
        }
    }
    Ok(())
}

impl<'a, 'root, 'source> SourceReferenceBuilderV29<'a, 'root, 'source> {
    fn remove_storage_value(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        kind: SourceReferenceRemovalKindV29,
        target: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if target.projections.is_empty() {
            let mut local = self.local(target.instance, target.local)?;
            local.node = None;
            self.set_local(target.instance, target.local, local)
        } else {
            let absent = self.inactive_node(site, source, kind, target, budget)?;
            self.replace_reference_fields(target, absent, budget)
        }
    }

    fn storage_error(
        &self,
        recorded: source_storage_v29::RecordedStorageFailureV29<'_>,
    ) -> ProductionSemanticKirErrorV1 {
        if !self.plan.failure.matches_recorded(&recorded) {
            return ArgumentResourceV1::Accounting.into();
        }
        self.plan
            .failure
            .first_error()
            .unwrap_or_else(|| ArgumentResourceV1::Accounting.into())
    }

    fn retain_storage_snapshot(
        &mut self,
        snapshot: source_storage_v29::SourceStorageSnapshotV29<'a>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.plan.check_owner(self.plan.instances, budget)?;
            let ordinal = self
                .storage_root
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?
                .snapshot_ordinal(snapshot, budget)
                .map_err(|error| self.storage_error(error))?;
            self.plan.charge(1, budget)?;
            if let Some(Some(index)) = self.storage_snapshot_indices.get(ordinal) {
                if self.plan.storage_snapshots.get(*index) != Some(&snapshot) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                return Ok(*index);
            }
            let count = argument_sum_v1(&[ordinal, 1])?;
            while self.storage_snapshot_indices.len() < count {
                emission_push_v1(&mut self.storage_snapshot_indices, None, budget)?;
            }
            let index = self.plan.storage_snapshots.len();
            emission_push_v1(&mut self.plan.storage_snapshots, snapshot, budget)?;
            self.storage_snapshot_indices[ordinal] = Some(index);
            Ok(index)
        })();
        if let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = &result {
            self.plan.failure.record_resource(*error);
        }
        result
    }

    fn initial_storage_snapshot(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        initialized: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        let Some(root) = &self.storage_root else {
            return Ok(None);
        };
        let snapshot = root
            .snapshot_local(instance, local, initialized, budget)
            .map_err(|error| self.storage_error(error))?;
        snapshot
            .map(|snapshot| self.retain_storage_snapshot(snapshot, budget))
            .transpose()
    }

    fn storage_snapshot(
        &self,
        index: usize,
    ) -> Result<source_storage_v29::SourceStorageSnapshotV29<'a>, ProductionSemanticKirErrorV1>
    {
        self.plan
            .storage_snapshots
            .get(index)
            .copied()
            .ok_or_else(|| ArgumentResourceV1::Accounting.into())
    }

    fn storage_states_equal(
        &self,
        left: Option<usize>,
        right: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        match (left, right) {
            (None, None) => Ok(true),
            (Some(left), Some(right)) => self
                .storage_root
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?
                .snapshots_equivalent(
                    self.storage_snapshot(left)?,
                    self.storage_snapshot(right)?,
                    budget,
                )
                .map_err(|error| self.storage_error(error)),
            _ => Err(source_reference_error_v29(
                "source reference CFG lost its original partial-object state",
            )),
        }
    }

    fn storage_values_equal(
        &self,
        left: Option<SourceReferenceValueStorageV29>,
        right: Option<SourceReferenceValueStorageV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        match (left, right) {
            (None, None) => Ok(true),
            (Some(left), Some(right)) => {
                budget.charge_work(argument_sum_v1(&[left.count.min(right.count), 1])?)?;
                let a = self
                    .plan
                    .projections
                    .get(left.first..argument_sum_v1(&[left.first, left.count])?)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let b = self
                    .plan
                    .projections
                    .get(right.first..argument_sum_v1(&[right.first, right.count])?)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if a != b {
                    return Ok(false);
                }
                if left.selector_source.is_some() || right.selector_source.is_some() {
                    if left.selector_source.is_none() || right.selector_source.is_none() {
                        return Ok(false);
                    }
                    for (ordinal, projection) in a.iter().enumerate() {
                        budget.charge_work(1)?;
                        if matches!(projection.kind(), SemanticProjectionKindV1::Index(_)) {
                            let x = self.plan.selector_for_path(
                                left.selector_source,
                                ordinal,
                                budget,
                            )?;
                            let y = self.plan.selector_for_path(
                                right.selector_source,
                                ordinal,
                                budget,
                            )?;
                            if x.canonical != y.canonical {
                                return Ok(false);
                            }
                        }
                    }
                }
                self.storage_states_equal(Some(left.snapshot), Some(right.snapshot), budget)
            }
            _ => Ok(false),
        }
    }

    fn join_storage_states(
        &mut self,
        left: Option<usize>,
        right: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        match (left, right) {
            (None, None) => Ok(None),
            (Some(left), Some(right)) => {
                let snapshot = self
                    .storage_root
                    .as_ref()
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .join_snapshots(
                        self.storage_snapshot(left)?,
                        self.storage_snapshot(right)?,
                        budget,
                    )
                    .map_err(|error| self.storage_error(error))?;
                self.retain_storage_snapshot(snapshot, budget).map(Some)
            }
            _ => Err(source_reference_error_v29(
                "source reference CFG lost its original partial-object state",
            )),
        }
    }

    fn check_storage_read(
        &self,
        place: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(index) = self.local(place.instance, place.local)?.storage else {
            return Ok(());
        };
        // A dereference without a tracked original loan crosses into external
        // memory. Check the holder prefix here, never its pointee as local bytes.
        budget.charge_work(argument_sum_v1(&[place.projections.len(), 1])?)?;
        let prefix = place
            .projections
            .iter()
            .position(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
            .unwrap_or(place.projections.len());
        let root = self
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let readable = if place.selector_source.is_some() {
            root.snapshot_selected_readable(
                self.storage_snapshot(index)?,
                &place.projections[..prefix],
                &self.plan,
                place.selector_source,
                budget,
            )
        } else {
            root.snapshot_readable(
                self.storage_snapshot(index)?,
                &place.projections[..prefix],
                budget,
            )
        }
        .map_err(|error| self.storage_error(error))?;
        if !readable {
            return Err(source_reference_error_v29(
                "source reference reads an uninitialized partial holder",
            ));
        }
        Ok(())
    }

    fn attach_storage_value(
        &mut self,
        place: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let Some(snapshot) = self.local(place.instance, place.local)?.storage else {
            return Ok(place.node);
        };
        budget.charge_work(place.projections.len())?;
        if place
            .projections
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Ok(place.node);
        }
        let first = self.plan.projections.len();
        for &projection in &place.projections {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        let mut value = self.plan.nodes[place.node];
        budget.charge_work(1)?;
        value.value_origin = Some(value.value_origin.unwrap_or(place.node));
        value.storage = Some(SourceReferenceValueStorageV29 {
            snapshot,
            first,
            count: place.projections.len(),
            selector_source: place.selector_source,
        });
        let node = self.plan.nodes.len();
        emission_push_v1(&mut self.plan.nodes, value, budget)?;
        Ok(node)
    }

    fn mutate_storage_place(
        &mut self,
        target: &SourceReferencePlaceV29,
        mutation: source_storage_v29::SourceStorageRootMutationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let mut local = self.local(target.instance, target.local)?;
        let Some(index) = local.storage else {
            return Ok(false);
        };
        budget.charge_work(target.projections.len())?;
        if target
            .projections
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Ok(false);
        }
        let root = self
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let snapshot = if target.selector_source.is_some() {
            root.mutate_selected_snapshot(
                self.storage_snapshot(index)?,
                &target.projections,
                &self.plan,
                target.selector_source,
                mutation,
                budget,
            )
        } else {
            root.mutate_snapshot(
                self.storage_snapshot(index)?,
                &target.projections,
                mutation,
                budget,
            )
        }
        .map_err(|error| self.storage_error(error))?;
        local.storage = Some(self.retain_storage_snapshot(snapshot, budget)?);
        self.set_local(target.instance, target.local, local)?;
        Ok(true)
    }

    fn retain_storage_representation(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        target: &SourceReferencePlaceV29,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.plan.has_storage_demands {
            return Ok(());
        }
        budget.charge_work(argument_sum_v1(&[2, target.projections.len()])?)?;
        if target.projections.iter().any(|projection| {
            projection.kind() == SemanticProjectionKindV1::Dereference
        }) {
            return Ok(());
        }
        let key = source_reference_access_key_v29(site, source, SourceReferenceAccessV29::Write);
        charge_execution_cfg_lookup_v29(self.plan.representation_demand_sites.len(), budget)?;
        if let Some(indices) = self.plan.representation_demand_sites.get(&key) {
            for &index in indices {
                budget.charge_work(argument_sum_v1(&[7, target.projections.len()])?)?;
                let row = self.plan.representation_demands.get(index)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if row.instance == target.instance
                    && row.local == target.local
                    && row.generation == target.generation
                    && row.selector_source == target.selector_source
                    && self.plan.projections.get(row.projections.clone())
                        == Some(target.projections.as_slice())
                    && self.nodes_equal(row.node, node, 0, budget)?
                {
                    return Ok(());
                }
            }
        } else {
            charge_execution_cfg_lookup_v29(self.plan.representation_demand_sites.len(), budget)?;
            // Prepay the split-path allowance and exact owned key before publication.
            budget.reserve_storage(argument_sum_v1(&[
                execution_cfg_map_entry_storage_v29::<
                    Box<SourceReferenceAccessIndexKeyV29>, Vec<usize>,
                >(self.plan.representation_demand_sites.len())?,
                std::mem::size_of::<SourceReferenceAccessIndexKeyV29>(),
            ])?)?;
            self.plan.representation_demand_sites.insert(Box::new(key), Vec::new());
        }
        let first = self.plan.projections.len();
        for &projection in &target.projections {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        let index = self.plan.representation_demands.len();
        emission_push_v1(&mut self.plan.representation_demands,
            SourceReferenceRepresentationDemandV29 {
                instance: target.instance,
                local: target.local,
                generation: target.generation,
                projections: first..self.plan.projections.len(),
                selector_source: target.selector_source,
                node,
            }, budget,
        )?;
        emission_push_v1(self.plan.representation_demand_sites.get_mut(&key)
            .ok_or(ArgumentResourceV1::Accounting)?, index, budget)?;
        Ok(())
    }

    fn transfer_storage_value(
        &mut self,
        target: &SourceReferencePlaceV29,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.storage_root.is_none() {
            return Ok(());
        }
        self.transfer_storage_value_at_depth(target, node, 0, budget)
    }

    fn transfer_storage_value_at_depth(
        &mut self,
        target: &SourceReferencePlaceV29,
        node: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if depth >= 256 {
            return Err(source_reference_error_v29(
                "source storage value transfer exceeds its typed depth bound",
            ));
        }
        let mut local = self.local(target.instance, target.local)?;
        let Some(destination) = local.storage else {
            return Ok(());
        };
        budget.charge_work(target.projections.len())?;
        if target
            .projections
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Ok(());
        }
        if let Some(source) = self.plan.nodes[node].storage {
            let end = argument_sum_v1(&[source.first, source.count])?;
            let root = self
                .storage_root
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let source_path = self
                .plan
                .projections
                .get(source.first..end)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let snapshot =
                if target.selector_source.is_some() || source.selector_source.is_some() {
                    root.snapshot_selected_copy_from(
                        self.storage_snapshot(destination)?,
                        &target.projections,
                        target.selector_source,
                        self.storage_snapshot(source.snapshot)?,
                        source_path,
                        source.selector_source,
                        &self.plan,
                        budget,
                    )
                } else {
                    root.snapshot_copy_from(
                        self.storage_snapshot(destination)?,
                        &target.projections,
                        self.storage_snapshot(source.snapshot)?,
                        source_path,
                        budget,
                    )
                }
                .map_err(|error| self.storage_error(error))?;
            local.storage = Some(self.retain_storage_snapshot(snapshot, budget)?);
            self.set_local(target.instance, target.local, local)
        } else {
            let value = self.plan.nodes[node];
            if matches!(value.kind, SourceReferenceNodeKindV29::Enum { .. }) {
                return self.transfer_enum_storage_value(target, node, depth, budget);
            }
            if value.kind == SourceReferenceNodeKindV29::Absent {
                self.mutate_storage_place(
                    target,
                    source_storage_v29::SourceStorageRootMutationV29::Deinitialize,
                    budget,
                )?;
                return Ok(());
            }
            let types = self.plan.instances.owner().source_semantic().types();
            let shape = types
                .get(value.ty.index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?
                .shape();
            let fields = match (shape, value.kind) {
                (
                    SemanticTypeShapeV1::Pointer(pointer),
                    SourceReferenceNodeKindV29::Address(set),
                ) if pointer.kind() == SemanticPointerKindV1::Raw
                    && self.plan.raw_sets.get(set).is_some() =>
                {
                    None
                }
                (
                    SemanticTypeShapeV1::Unit
                    | SemanticTypeShapeV1::Scalar(_)
                    | SemanticTypeShapeV1::ValidityScalar(_)
                    | SemanticTypeShapeV1::Pointer(_),
                    SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Loan(_)
                        | SourceReferenceNodeKindV29::Discriminant(_),
                ) => None,
                (
                    SemanticTypeShapeV1::Unit,
                    SourceReferenceNodeKindV29::Aggregate { count: 0, .. },
                ) => None,
                (
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    SourceReferenceNodeKindV29::Aggregate { first, count },
                ) if fields.fields().len() == count => Some((first, count)),
                (
                    SemanticTypeShapeV1::Array { length, .. },
                    SourceReferenceNodeKindV29::Aggregate { first, count },
                ) if u64::try_from(count).ok() == Some(*length) => Some((first, count)),
                // In particular, an arbitrary enum/union value cannot acquire
                // unconditional payload facts without its original typed copy
                // or an exact source constructor/active-variant transfer.
                _ => {
                    return Err(source_reference_error_v29(
                        "source storage value needs its original typed construction state",
                    ));
                }
            };
            self.mutate_storage_place(
                target,
                if fields.is_some_and(|(_, count)| count != 0) {
                    source_storage_v29::SourceStorageRootMutationV29::Deinitialize
                } else {
                    source_storage_v29::SourceStorageRootMutationV29::Initialize
                },
                budget,
            )?;
            if let Some((first, count)) = fields {
                for field in 0..count {
                    budget.charge_work(5)?;
                    let child = *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[first, field])?)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if child >= node {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    let child_ty = self.plan.nodes[child].ty;
                    let kind = match shape {
                        SemanticTypeShapeV1::Tuple(fields)
                        | SemanticTypeShapeV1::Aggregate(fields)
                            if fields.fields().get(field) == Some(&child_ty) =>
                        {
                            SemanticProjectionKindV1::Field(
                                u32::try_from(field).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            )
                        }
                        SemanticTypeShapeV1::Array { element, length } if *element == child_ty => {
                            SemanticProjectionKindV1::ConstantIndex {
                                offset: u64::try_from(field)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                minimum_length: *length,
                                from_end: false,
                            }
                        }
                        _ => return Err(ArgumentResourceV1::Accounting.into()),
                    };
                    budget.reserve_storage(argument_sum_v1(&[
                        std::mem::size_of::<SourceReferencePlaceV29>(),
                        2 * std::mem::size_of::<
                            Result<
                                SemanticProjectionV1,
                                fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1,
                            >,
                        >(),
                        2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
                    ])?)?;
                    let projection = SemanticProjectionV1::new(kind, child_ty)
                        .map_err(|_| ArgumentResourceV1::Accounting)?;
                    let mut projections = source_reference_scratch_v29(
                        argument_sum_v1(&[target.projections.len(), 1])?,
                        budget,
                    )?;
                    budget.charge_work(target.projections.len())?;
                    projections.extend_from_slice(&target.projections);
                    projections.push(projection);
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
            }
            Ok(())
        }
    }

    fn transfer_storage_lifetime(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        live: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut value = self.local(instance, local)?;
        let Some(index) = value.storage else {
            return Ok(());
        };
        let snapshot = self
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?
            .snapshot_lifetime(self.storage_snapshot(index)?, live, budget)
            .map_err(|error| self.storage_error(error))?;
        value.storage = Some(self.retain_storage_snapshot(snapshot, budget)?);
        self.set_local(instance, local, value)
    }
}
