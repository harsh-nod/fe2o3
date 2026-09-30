// Whole-descriptor alternatives belong to the existing C1 fixed point. They
// select a physical representation; they do not prove an access or a lifetime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceDescriptorOriginV29 {
    original_argument: u32,
    data_component: u8,
    length_component: u8,
}

#[derive(Clone, Copy, Debug)]
struct SourceDescriptorSetV29 {
    first: usize,
    count: usize,
    has_unknown: bool,
    representation: AddressSpace,
}

type SourceDescriptorSetKeyV29 = (AddressSpace, bool, usize, u64);

#[derive(Clone, Copy)]
struct SourceDescriptorValueV29 {
    ty: SemanticTypeIdV1,
    descriptor: Option<usize>,
}

#[derive(Clone, Copy)]
enum SourceDescriptorAtomsV29 {
    Empty,
    One(SourceDescriptorOriginV29),
    Retained { first: usize, count: usize },
}

#[derive(Clone, Copy)]
struct SourceDescriptorUnionV29 {
    left: SourceDescriptorAtomsV29,
    right: SourceDescriptorAtomsV29,
    left_index: usize,
    right_index: usize,
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn descriptor_atom(
        &self,
        atoms: SourceDescriptorAtomsV29,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceDescriptorOriginV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        match atoms {
            SourceDescriptorAtomsV29::Empty => Ok(None),
            SourceDescriptorAtomsV29::One(origin) => Ok((index == 0).then_some(origin)),
            SourceDescriptorAtomsV29::Retained { first, count } if index < count => self
                .plan
                .descriptor_origins
                .get(argument_sum_v1(&[first, index])?)
                .copied()
                .map(Some)
                .ok_or_else(|| ArgumentResourceV1::Accounting.into()),
            SourceDescriptorAtomsV29::Retained { .. } => Ok(None),
        }
    }

    fn next_descriptor_union(
        &self,
        union: &mut SourceDescriptorUnionV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceDescriptorOriginV29>, ProductionSemanticKirErrorV1> {
        let left = self.descriptor_atom(union.left, union.left_index, budget)?;
        let right = self.descriptor_atom(union.right, union.right_index, budget)?;
        let next = match (left, right) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        };
        if next.is_some() && left == next {
            union.left_index = argument_sum_v1(&[union.left_index, 1])?;
        }
        if next.is_some() && right == next {
            union.right_index = argument_sum_v1(&[union.right_index, 1])?;
        }
        Ok(next)
    }

    fn intern_descriptor_set(
        &mut self,
        left: SourceDescriptorAtomsV29,
        right: SourceDescriptorAtomsV29,
        has_unknown: bool,
        representation: AddressSpace,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            2 * std::mem::size_of::<SourceDescriptorUnionV29>(),
            std::mem::size_of::<SourceDescriptorSetKeyV29>(),
            3 * std::mem::size_of::<Option<SourceDescriptorOriginV29>>(),
            2 * std::mem::size_of::<
                Result<Option<SourceDescriptorOriginV29>, ProductionSemanticKirErrorV1>,
            >(),
        ])?)?;
        let start = SourceDescriptorUnionV29 {
            left,
            right,
            left_index: 0,
            right_index: 0,
        };
        let mut union = start;
        let mut count = 0usize;
        let mut hash = 0xcbf29ce484222325u64;
        while let Some(origin) = self.next_descriptor_union(&mut union, budget)? {
            budget.charge_work(3)?;
            count = argument_sum_v1(&[count, 1])?;
            // This hash is a lookup accelerator only. Every collision is
            // compared against the complete ordered correlated origin rows.
            hash = (hash ^ u64::from(origin.original_argument)).wrapping_mul(0x100000001b3);
            hash = (hash ^ u64::from(origin.data_component)).wrapping_mul(0x100000001b3);
            hash = (hash ^ u64::from(origin.length_component)).wrapping_mul(0x100000001b3);
        }
        let key = (representation, has_unknown, count, hash);
        charge_execution_cfg_lookup_v29(self.plan.descriptor_set_index.len(), budget)?;
        if let Some(indices) = self.plan.descriptor_set_index.get(&key) {
            for &index in indices {
                budget.charge_work(1)?;
                let row = self
                    .plan
                    .descriptor_sets
                    .get(index)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if row.count != count
                    || row.representation != representation
                    || row.has_unknown != has_unknown
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let mut union = start;
                let mut position = 0;
                let mut equal = true;
                while let Some(origin) = self.next_descriptor_union(&mut union, budget)? {
                    budget.charge_work(1)?;
                    if self
                        .plan
                        .descriptor_origins
                        .get(argument_sum_v1(&[row.first, position])?)
                        != Some(&origin)
                    {
                        equal = false;
                        break;
                    }
                    position = argument_sum_v1(&[position, 1])?;
                }
                if equal && position == count {
                    return Ok(index);
                }
            }
        }
        let first = self.plan.descriptor_origins.len();
        let mut union = start;
        while let Some(origin) = self.next_descriptor_union(&mut union, budget)? {
            emission_push_v1(&mut self.plan.descriptor_origins, origin, budget)?;
        }
        let index = self.plan.descriptor_sets.len();
        emission_push_v1(
            &mut self.plan.descriptor_sets,
            SourceDescriptorSetV29 {
                first,
                count,
                has_unknown,
                representation,
            },
            budget,
        )?;
        charge_execution_cfg_lookup_v29(self.plan.descriptor_set_index.len(), budget)?;
        if !self.plan.descriptor_set_index.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<SourceDescriptorSetKeyV29, Vec<usize>>(
                self.plan.descriptor_set_index.len(),
                budget,
            )?;
            self.plan.descriptor_set_index.insert(key, Vec::new());
        }
        emission_push_v1(
            self.plan
                .descriptor_set_index
                .get_mut(&key)
                .ok_or(ArgumentResourceV1::Accounting)?,
            index,
            budget,
        )?;
        Ok(index)
    }

    fn seed_descriptor_argument(
        &mut self,
        node: usize,
        argument: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(profile) = self.plan.descriptor_root else {
            return Ok(());
        };
        let ty = self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?
            .ty;
        if profile.shared_slice(argument, ty, budget)? {
            let origin = SourceDescriptorOriginV29 {
                original_argument: argument,
                data_component: 0,
                length_component: 1,
            };
            let descriptor = self.intern_descriptor_set(
                SourceDescriptorAtomsV29::One(origin),
                SourceDescriptorAtomsV29::Empty,
                false,
                AddressSpace::Global,
                budget,
            )?;
            self.plan.nodes[node].descriptor = Some(descriptor);
        }
        Ok(())
    }

    fn merge_descriptor_facts(
        &mut self,
        left: SourceReferenceNodeV29,
        right: SourceReferenceNodeV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if left.ty != right.ty {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.merge_descriptor_values(left.ty, left.descriptor, right.descriptor, budget)
    }

    fn merge_descriptor_values(
        &mut self,
        ty: SemanticTypeIdV1,
        left: Option<usize>,
        right: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if left == right {
            return Ok(left);
        }
        budget.charge_work(2)?;
        let original = source_descriptor_original_space_v29(
            self.plan.instances.owner().source_semantic().types(),
            ty,
        )?
        .ok_or(ArgumentResourceV1::Accounting)?;
        let row = |descriptor: Option<usize>| -> Result<SourceDescriptorSetV29, ProductionSemanticKirErrorV1> {
            match descriptor {
                Some(index) => self.plan.descriptor_sets.get(index).copied().ok_or_else(|| ArgumentResourceV1::Accounting.into()),
                None => Ok(SourceDescriptorSetV29 { first: 0, count: 0, has_unknown: true, representation: original }),
            }
        };
        let a = row(left)?;
        let b = row(right)?;
        let representation = if a.representation == b.representation {
            a.representation
        } else {
            AddressSpace::Generic
        };
        self.intern_descriptor_set(
            SourceDescriptorAtomsV29::Retained {
                first: a.first,
                count: a.count,
            },
            SourceDescriptorAtomsV29::Retained {
                first: b.first,
                count: b.count,
            },
            a.has_unknown || b.has_unknown,
            representation,
            budget,
        )
        .map(Some)
    }

    fn retain_descriptor_value(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        prefix: usize,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.descriptor_root.is_none() {
            return Ok(());
        }
        budget.charge_work(3)?;
        let value = *self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if source_descriptor_original_space_v29(
            self.plan.instances.owner().source_semantic().types(),
            value.ty,
        )?
        .is_none()
            || value.kind == SourceReferenceNodeKindV29::Absent
            || value.inactive.is_some()
        {
            return Ok(());
        }
        let key = (
            site.instance.index(),
            site.block.index(),
            site.statement,
            source as *const SemanticPlaceV1 as usize,
            prefix,
        );
        charge_execution_cfg_lookup_v29(self.plan.descriptor_values.len(), budget)?;
        if let Some(previous) = self.plan.descriptor_values.get(&key).copied() {
            if previous.ty != value.ty {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let descriptor = self.merge_descriptor_values(
                value.ty,
                previous.descriptor,
                value.descriptor,
                budget,
            )?;
            self.plan
                .descriptor_values
                .get_mut(&key)
                .ok_or(ArgumentResourceV1::Accounting)?
                .descriptor = descriptor;
        } else {
            reserve_execution_cfg_map_entry_v29::<
                SourceReferenceSelectorSiteV29,
                SourceDescriptorValueV29,
            >(self.plan.descriptor_values.len(), budget)?;
            self.plan.descriptor_values.insert(
                key,
                SourceDescriptorValueV29 {
                    ty: value.ty,
                    descriptor: value.descriptor,
                },
            );
        }
        Ok(())
    }
}

impl SourceReferencePlanV29<'_, '_> {
    fn descriptor_value_space(
        &self,
        instance: ProductionCallInstanceIdV1,
        occurrence: usize,
        source: &SemanticPlaceV1,
        prefix: usize,
        ty: SemanticTypeIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<AddressSpace, ProductionSemanticKirErrorV1> {
        if self.descriptor_root.is_none() {
            return source_descriptor_original_space_v29(
                self.instances.owner().source_semantic().types(),
                ty,
            )?
            .ok_or_else(source_descriptor_error_v29);
        }
        budget.source_reference_charge_v29(self, 12)?;
        let original = self
            .instances
            .instance(instance)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let occurrences = self
            .instances
            .occurrences(instance)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let event = occurrences
            .events()
            .get(occurrence)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if event.role() != ExecutionEventV29::BaseUse
            || event.event().variable().get() != source.local().index()
            || !source_reference_selector_place_v29(
                original.declaration(),
                event.site(),
                event.operand(),
            )
            .is_some_and(|place| std::ptr::eq(place, source))
        {
            return Err(source_descriptor_error_v29());
        }
        let source_ty = if prefix == 0 {
            original
                .declaration()
                .locals()
                .get(source.local().index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?
                .ty()
        } else {
            source
                .projections()
                .get(prefix - 1)
                .ok_or(ArgumentResourceV1::Accounting)?
                .result_type()
        };
        if source_ty != ty {
            return Err(source_descriptor_error_v29());
        }
        let key = source_reference_selector_site_v29(instance, event.site(), source, prefix);
        budget.source_reference_charge_v29(
            self,
            self.descriptor_values.len().checked_ilog2().unwrap_or(0) as usize + 2,
        )?;
        let value = self
            .descriptor_values
            .get(&key)
            .ok_or_else(source_descriptor_error_v29)?;
        if value.ty != ty {
            return Err(source_descriptor_error_v29());
        }
        match value.descriptor {
            Some(index) => self
                .descriptor_sets
                .get(index)
                .map(|row| row.representation)
                .ok_or_else(|| ArgumentResourceV1::Accounting.into()),
            None => source_descriptor_original_space_v29(
                self.instances.owner().source_semantic().types(),
                ty,
            )?
            .ok_or_else(source_descriptor_error_v29),
        }
    }
}

fn source_descriptor_original_space_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Option<AddressSpace>, ProductionSemanticKirErrorV1> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or(ArgumentResourceV1::Accounting)?;
    match declaration.shape() {
        SemanticTypeShapeV1::Pointer(pointer)
            if pointer.metadata() == SemanticPointerMetadataV1::SliceLength =>
        {
            source_address_space_v18(pointer.address_space()).map(Some)
        }
        _ => Ok(None),
    }
}

fn source_descriptor_node_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: Option<usize>,
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let types = plan.instances.owner().source_semantic().types();
    if plan.descriptor_root.is_none() {
        return source_parameter_type_v18(types, &[], ty);
    }
    budget.source_reference_charge_v29(plan, 2)?;
    let descriptor = match node {
        Some(node) => {
            let value = plan.nodes.get(node).ok_or(ArgumentResourceV1::Accounting)?;
            if value.ty != ty
                || value.kind == SourceReferenceNodeKindV29::Absent
                || value.inactive.is_some()
            {
                return Err(source_reference_error_v29(
                    "descriptor representation has no live original source node",
                ));
            }
            value.descriptor
        }
        None => None,
    };
    let mut physical = source_parameter_type_v18(types, &[], ty)?;
    if let Some(index) = descriptor {
        let row = plan
            .descriptor_sets
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let Type::Slice(slice) = &mut physical else {
            return Err(ArgumentResourceV1::Accounting.into());
        };
        slice.address_space = row.representation;
    }
    Ok(physical)
}
