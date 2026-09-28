// A source census for the one module layout table, not an allocation, loan or
// initializedness certificate. Object coordinates remain instance-qualified.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Module storage-owner cutover follows this contract"
    )
)]
mod source_storage_demands_v29 {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    use std::mem::size_of;

    type Error = ProductionSemanticKirErrorV1;
    type Budget<'a> = ArgumentBudgetV1<'a>;
    const NOMINAL: u8 = 1;
    const REFERENCE: u8 = 2;
    const NONE: usize = usize::MAX;

    fn error() -> Error {
        unsupported(
            0,
            None,
            None,
            "source storage demand differs from its original type or instance",
        )
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum ComponentStepV29 {
        Field(u32),
        Variant(u32),
        // A homogeneous component schema, never a concrete runtime index.
        Element,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum DemandKindV29 {
        WholeBackingCandidate,
        // The C1 transport/backing join must resolve this before allocation.
        UnresolvedNominalBacking,
        ReferenceHolder,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub(super) struct DemandV29 {
        pub(super) root: SemanticFunctionIdV1,
        pub(super) instance: ProductionCallInstanceIdV1,
        pub(super) function: SemanticFunctionIdV1,
        pub(super) local: SemanticLocalIdV1,
        pub(super) ty: SemanticTypeIdV1,
        pub(super) kind: DemandKindV29,
        pub(super) path: std::ops::Range<usize>,
    }

    pub(super) struct RootDemandRangeV29 {
        root: SemanticFunctionIdV1,
        requests: std::ops::Range<usize>,
    }

    pub(super) struct SourceStorageDemandsV29<'source> {
        owner: &'source ProductionSemanticSsaOwnerV1,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        slot: usize,
        floor: usize,
        retained: usize,
        types: Vec<SemanticTypeIdV1>,
        requests: Vec<DemandV29>,
        paths: Vec<ComponentStepV29>,
        roots: Vec<RootDemandRangeV29>,
    }

    impl<'source> SourceStorageDemandsV29<'source> {
        pub(super) fn collect(
            owner: &'source ProductionSemanticSsaOwnerV1,
            budget: &mut Budget<'_>,
        ) -> Result<Self, Error> {
            let floor = budget.storage();
            scoped_slot_attempt_v29(budget, |budget| {
                let mut output = Self {
                    owner,
                    ledger: budget.work_ledger_identity_v1(),
                    slot: budget as *const Budget<'_> as usize,
                    floor,
                    retained: 0,
                    types: Vec::new(),
                    requests: Vec::new(),
                    paths: Vec::new(),
                    roots: Vec::new(),
                };
                output.build(budget)?;
                output.retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let exact = argument_sum_v1(&[
                    bytes(&output.types)?,
                    bytes(&output.requests)?,
                    bytes(&output.paths)?,
                    bytes(&output.roots)?,
                ])?;
                if output.retained != exact {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok(output)
            })
        }

        fn check(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &Budget<'_>,
        ) -> Result<(), Error> {
            if !std::ptr::eq(self.owner, owner)
                || self.ledger != budget.work_ledger_identity_v1()
                || self.slot != budget as *const Budget<'_> as usize
                || self
                    .floor
                    .checked_add(self.retained)
                    .is_none_or(|minimum| budget.storage() < minimum)
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(())
        }

        pub(super) fn types<'a>(
            &'a self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &mut Budget<'_>,
        ) -> Result<&'a [SemanticTypeIdV1], Error> {
            self.check(owner, budget)?;
            budget.charge_work(1)?;
            Ok(&self.types)
        }

        pub(super) fn requests<'a>(
            &'a self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &mut Budget<'_>,
        ) -> Result<(&'a [DemandV29], &'a [ComponentStepV29]), Error> {
            self.check(owner, budget)?;
            budget.charge_work(1)?;
            Ok((&self.requests, &self.paths))
        }

        pub(super) fn discard(self, budget: &mut Budget<'_>) -> Result<(), Error> {
            self.check(self.owner, budget)?;
            let retained = self.retained;
            drop(self);
            budget.release_storage(retained)?;
            Ok(())
        }

        // The returned path ranges index the borrowed module-wide path slice.
        // Even an empty request range is authenticated against an original root.
        pub(super) fn root_requests<'a>(
            &'a self,
            owner: &ProductionSemanticSsaOwnerV1,
            ordinal: usize,
            budget: &mut Budget<'_>,
        ) -> Result<(&'a [DemandV29], &'a [ComponentStepV29]), Error> {
            self.check(owner, budget)?;
            budget.charge_work(4)?;
            let root = self.roots.get(ordinal).ok_or_else(error)?;
            if owner.source_semantic().roots().get(ordinal) != Some(&root.root) {
                return Err(error());
            }
            let requests = self.requests.get(root.requests.clone()).ok_or_else(error)?;
            Ok((requests, &self.paths))
        }

        fn build(&mut self, budget: &mut Budget<'_>) -> Result<(), Error> {
            let owner = self.owner;
            let semantic = owner.source_semantic();
            let types = semantic.types();
            let properties = classify(types, budget)?;
            self.roots = emission_vec_v1(semantic.roots().len(), budget)?;
            let mut demanded = filled(types.len(), false, budget)?;
            let mut cached =
                emission_vec_v1::<Option<Vec<u8>>>(semantic.functions().len(), budget)?;
            budget.charge_work(semantic.functions().len())?;
            cached.resize_with(semantic.functions().len(), || None);
            for &root in semantic.roots() {
                budget.charge_work(1)?;
                let first = self.requests.len();
                production_call_instances_v1::with_production_call_instances_v1(
                    owner,
                    root,
                    budget,
                    |instances, budget| {
                        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                            self.scan_instances(
                                root,
                                instances,
                                &mut cached,
                                &properties,
                                &mut demanded,
                                budget,
                            ),
                        )
                    },
                )
                .map_err(scoped_root_instance_error_v29)??;
                emission_push_v1(
                    &mut self.roots,
                    RootDemandRangeV29 {
                        root,
                        requests: first..self.requests.len(),
                    },
                    budget,
                )?;
            }
            // Type order depends only on original source IDs, not call expansion.
            for (index, wanted) in demanded.iter().enumerate() {
                budget.charge_work(1)?;
                if *wanted {
                    emission_push_v1(
                        &mut self.types,
                        SemanticTypeIdV1::from_index(
                            u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        budget,
                    )?;
                }
            }
            for row in &mut cached {
                budget.charge_work(1)?;
                if let Some(flags) = row.take() {
                    discard(flags, budget)?;
                }
            }
            discard(cached, budget)?;
            discard(demanded, budget)?;
            discard(properties, budget)?;
            Ok(())
        }

        fn scan_instances(
            &mut self,
            root: SemanticFunctionIdV1,
            instances: &production_call_instances_v1::ProductionCallInstancePlanV1<'_>,
            cached: &mut [Option<Vec<u8>>],
            properties: &[u8],
            demanded: &mut [bool],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            let owner = self.owner;
            let types = owner.source_semantic().types();
            for (index, instance) in instances.instances().iter().enumerate() {
                budget.charge_work(3)?;
                let id = instances.id_at(index).ok_or_else(error)?;
                let flags = cached
                    .get_mut(instance.function().index() as usize)
                    .ok_or_else(error)?;
                if flags.is_none() {
                    *flags = Some(scoped_slot_candidates_v29(
                        instance.declaration(),
                        instance.ssa(),
                        budget,
                    )?);
                }
                let flags = flags.as_ref().ok_or_else(error)?;
                for (local, declaration) in instance.declaration().locals().iter().enumerate() {
                    budget.charge_work(5)?;
                    let ty = declaration.ty();
                    let property = *properties.get(ty.index() as usize).ok_or_else(error)?;
                    let coordinate = Coordinate {
                        root,
                        instance: id,
                        function: instance.function(),
                        local: SemanticLocalIdV1::from_index(
                            u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                    };
                    if flags.get(local) == Some(&2) && !direct_nominal(types, ty)? {
                        let kind = if property & NOMINAL == 0 {
                            DemandKindV29::WholeBackingCandidate
                        } else {
                            DemandKindV29::UnresolvedNominalBacking
                        };
                        self.append(coordinate, ty, kind, &[], demanded, budget)?;
                    }
                    if property & REFERENCE != 0 && !direct_nominal(types, ty)? {
                        self.holder(coordinate, ty, properties, demanded, budget)?;
                    }
                }
            }
            Ok(())
        }

        fn append(
            &mut self,
            source: Coordinate,
            ty: SemanticTypeIdV1,
            kind: DemandKindV29,
            path: &[ComponentStepV29],
            demanded: &mut [bool],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            budget.charge_work(3)?;
            if kind != DemandKindV29::UnresolvedNominalBacking {
                *demanded.get_mut(ty.index() as usize).ok_or_else(error)? = true;
            }
            let first = self.paths.len();
            for &step in path {
                budget.charge_work(1)?;
                emission_push_v1(&mut self.paths, step, budget)?;
            }
            emission_push_v1(
                &mut self.requests,
                DemandV29 {
                    root: source.root,
                    instance: source.instance,
                    function: source.function,
                    local: source.local,
                    ty,
                    kind,
                    path: first..self.paths.len(),
                },
                budget,
            )
        }

        fn holder(
            &mut self,
            source: Coordinate,
            ty: SemanticTypeIdV1,
            properties: &[u8],
            demanded: &mut [bool],
            budget: &mut Budget<'_>,
        ) -> Result<(), Error> {
            let owner = self.owner;
            let types = owner.source_semantic().types();
            let mut frames = emission_vec_v1(1, budget)?;
            frames.push(Frame {
                ty,
                parent: NONE,
                step: None,
                visit: true,
            });
            let mut cursor = 0;
            let mut path = Vec::new();
            while cursor < frames.len() {
                budget.charge_work(4)?;
                let frame = frames[cursor];
                if !frame.visit {
                    cursor += 1;
                    continue;
                }
                let property = *properties
                    .get(frame.ty.index() as usize)
                    .ok_or_else(error)?;
                if property & REFERENCE != 0 && !direct_nominal(types, frame.ty)? {
                    if property & NOMINAL == 0 {
                        path.clear();
                        let mut ancestor = cursor;
                        loop {
                            budget.charge_work(1)?;
                            let row = frames[ancestor];
                            if let Some(step) = row.step {
                                emission_push_v1(&mut path, step, budget)?;
                            }
                            if row.parent == NONE {
                                break;
                            }
                            if row.parent >= ancestor {
                                return Err(error());
                            }
                            ancestor = row.parent;
                        }
                        budget.charge_work(path.len())?;
                        path.reverse();
                        self.append(
                            source,
                            frame.ty,
                            DemandKindV29::ReferenceHolder,
                            &path,
                            demanded,
                            budget,
                        )?;
                    } else {
                        visit_components(
                            &types[frame.ty.index() as usize],
                            budget,
                            |ty, first, second, budget| {
                                let mut parent = cursor;
                                if let Some(step) = first {
                                    parent = frames.len();
                                    emission_push_v1(
                                        &mut frames,
                                        Frame {
                                            ty: frame.ty,
                                            parent: cursor,
                                            step: Some(step),
                                            visit: false,
                                        },
                                        budget,
                                    )?;
                                    // Intermediate variant coordinates carry no type visit.
                                }
                                emission_push_v1(
                                    &mut frames,
                                    Frame {
                                        ty,
                                        parent,
                                        step: Some(second),
                                        visit: true,
                                    },
                                    budget,
                                )
                            },
                        )?;
                    }
                }
                cursor += 1;
            }
            discard(path, budget)?;
            discard(frames, budget)
        }
    }

    #[derive(Clone, Copy)]
    struct Coordinate {
        root: SemanticFunctionIdV1,
        instance: ProductionCallInstanceIdV1,
        function: SemanticFunctionIdV1,
        local: SemanticLocalIdV1,
    }

    #[derive(Clone, Copy)]
    struct Frame {
        ty: SemanticTypeIdV1,
        parent: usize,
        step: Option<ComponentStepV29>,
        visit: bool,
    }

    fn bytes<T>(rows: &Vec<T>) -> Result<usize, Error> {
        Ok(argument_product_v1(rows.capacity(), size_of::<T>())?)
    }
    fn discard<T>(rows: Vec<T>, budget: &mut Budget<'_>) -> Result<(), Error> {
        let bytes = bytes(&rows)?;
        drop(rows);
        budget.release_storage(bytes)?;
        Ok(())
    }
    fn filled<T: Copy>(count: usize, value: T, budget: &mut Budget<'_>) -> Result<Vec<T>, Error> {
        let mut rows = emission_vec_v1(count, budget)?;
        budget.charge_work(count)?;
        rows.resize(count, value);
        Ok(rows)
    }

    fn direct_nominal(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Result<bool, Error> {
        let ty = types.get(ty.index() as usize).ok_or_else(error)?;
        if matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)) {
            return Ok(true);
        }
        match ty.shape() {
            SemanticTypeShapeV1::Pointer(pointer) => Ok(matches!(
                types
                    .get(pointer.pointee().index() as usize)
                    .ok_or_else(error)?
                    .rust_type_kind(),
                SemanticRustTypeKindV1::Execution(_)
            )),
            _ => Ok(false),
        }
    }

    fn visit_components(
        declaration: &SemanticTypeDeclV1,
        budget: &mut Budget<'_>,
        mut visit: impl FnMut(
            SemanticTypeIdV1,
            Option<ComponentStepV29>,
            ComponentStepV29,
            &mut Budget<'_>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match declaration.shape() {
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                for (index, &ty) in fields.fields().iter().enumerate() {
                    budget.charge_work(1)?;
                    visit(
                        ty,
                        None,
                        ComponentStepV29::Field(
                            u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        budget,
                    )?;
                }
            }
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                budget.charge_work(1)?;
                visit(*element, None, ComponentStepV29::Element, budget)?;
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for (variant, row) in variants.iter().enumerate() {
                    budget.charge_work(1)?;
                    let variant = ComponentStepV29::Variant(
                        u32::try_from(variant).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    );
                    for (field, &ty) in row.fields().fields().iter().enumerate() {
                        budget.charge_work(1)?;
                        visit(
                            ty,
                            Some(variant),
                            ComponentStepV29::Field(
                                u32::try_from(field).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            budget,
                        )?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    struct ReverseEdge {
        parent: usize,
        next: usize,
        mask: u8,
    }

    // Two monotone bits over original type IDs. Pointer cycles do not expand;
    // a reference bit crosses containment only, a nominal bit also follows
    // pointee-layout dependencies. Function signatures are not object fields.
    fn classify(types: &[SemanticTypeDeclV1], budget: &mut Budget<'_>) -> Result<Vec<u8>, Error> {
        let mut properties = filled(types.len(), 0_u8, budget)?;
        let mut heads = filled(types.len(), NONE, budget)?;
        let mut edges = Vec::new();
        let mut queue = Vec::new();
        for (parent, declaration) in types.iter().enumerate() {
            budget.charge_work(2)?;
            if matches!(
                declaration.rust_type_kind(),
                SemanticRustTypeKindV1::Execution(_)
            ) {
                properties[parent] |= NOMINAL;
            }
            let mut edge =
                |child: SemanticTypeIdV1, mask: u8, budget: &mut Budget<'_>| -> Result<(), Error> {
                    budget.charge_work(2)?;
                    let head = heads.get_mut(child.index() as usize).ok_or_else(error)?;
                    let next = *head;
                    *head = edges.len();
                    emission_push_v1(&mut edges, ReverseEdge { parent, next, mask }, budget)
                };
            match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) => {
                    if pointer.kind() == SemanticPointerKindV1::Reference {
                        properties[parent] |= REFERENCE;
                    }
                    edge(pointer.pointee(), NOMINAL, budget)?;
                }
                SemanticTypeShapeV1::Enum { discriminant, .. } => {
                    edge(*discriminant, NOMINAL, budget)?;
                    visit_components(declaration, budget, |child, _, _, budget| {
                        edge(child, NOMINAL | REFERENCE, budget)
                    })?;
                }
                _ => visit_components(declaration, budget, |child, _, _, budget| {
                    edge(child, NOMINAL | REFERENCE, budget)
                })?,
            }
            for bit in [NOMINAL, REFERENCE] {
                if properties[parent] & bit != 0 {
                    emission_push_v1(&mut queue, (parent, bit), budget)?;
                }
            }
        }
        let mut cursor = 0;
        while cursor < queue.len() {
            budget.charge_work(1)?;
            let (child, bit) = queue[cursor];
            let mut next = heads[child];
            while next != NONE {
                budget.charge_work(3)?;
                let edge = edges[next];
                if edge.mask & bit != 0 && properties[edge.parent] & bit == 0 {
                    properties[edge.parent] |= bit;
                    emission_push_v1(&mut queue, (edge.parent, bit), budget)?;
                }
                next = edge.next;
            }
            cursor += 1;
        }
        discard(queue, budget)?;
        discard(edges, budget)?;
        discard(heads, budget)?;
        Ok(properties)
    }
}
