// Genuine original Source slice replay. MAIN legacy V1 remains in its original host.

include!("production_source_descriptor_origin_v30.rs");

#[cfg(test)]
include!("production_source_descriptor_replay_facts_v1762_test_hooks.rs");

struct SourceSliceReplayFactsV18<'a> {
    common: SliceFacts<'a>,
    address_operation: SliceOperation,
    data_operation: SliceOperation,
    length_operation: SliceOperation,
    guard_condition: SliceDefinition,
    guard_edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
}

fn source_slice_replay_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<SourceSliceReplayFactsV18<'_>>(),
        std::mem::align_of::<SourceSliceReplayFactsV18<'_>>(),
        source_descriptor_origin_headers_v30()?,
    ])
}

fn descriptor_write_lookup_headers_v18() -> Result<usize, ArgumentResourceV1> {
    type Definition<'a> = &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>;
    argument_sum_v1(&[
        size_of::<Definition<'_>>(),
        size_of::<Option<Definition<'_>>>(),
        size_of::<Result<Option<Definition<'_>>, CanonicalKirInventoryErrorV1>>(),
        size_of::<SliceResult<Option<Definition<'_>>>>(),
        size_of::<SliceResult<Definition<'_>>>(),
    ])
}

fn with_slice_argument_v18<R>(
    arguments: ArgumentViewDataV18<'_>,
    budget: &mut SliceBudget<'_>,
    facts: &SliceFacts<'_>,
    argument: u32,
    use_view: impl for<'s> FnOnce(&ProductionSliceAccessViewV1<'s>) -> SliceResult<R>,
) -> SliceResult<R> {
    let mut matches = 0_usize;
    arguments.visit_nodes_scoped(budget, |node, _| {
        if matches!(node.coverage(), ProductionArgumentCoverageV1::Parameter(value)
            if value.slot() == argument as usize)
        {
            matches += 1;
        }
        Ok(())
    })?;
    if matches != 1 {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let mut use_view = Some(use_view);
    let mut result = None;
    arguments.visit_nodes_scoped(budget, |source, _| {
        if matches!(source.coverage(), ProductionArgumentCoverageV1::Parameter(value)
            if value.slot() == argument as usize)
        {
            let visit = use_view
                .take()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            result = Some(visit(&ProductionSliceAccessViewV1 { facts, source })?);
        }
        Ok(())
    })?;
    result.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Joins a selected-body read to the original root ABI and actual carrier.
    /// The local hint belongs to the selected body, never a reused root local.
    /// Bounds, value transport and assertion-success correspondence are checked;
    /// this grants no allocation, borrow, initialization or execution authority.
    pub fn with_checked_slice_access_v18<R>(
        &self,
        root: usize,
        instance: usize,
        site: ProductionSliceAccessSiteV1,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'s> FnOnce(
            &ProductionSliceAccessViewV1<'s>,
            Option<SemanticLocalIdV1>,
        ) -> SliceResult<R>,
    ) -> SourceOwnedResultV18<R> {
        self.with_descriptor_access_v18(root, instance, site, false, budget, use_view)
    }

    fn with_descriptor_access_v18<R>(
        &self,
        root: usize,
        instance: usize,
        site: ProductionSliceAccessSiteV1,
        write: bool,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'s> FnOnce(
            &ProductionSliceAccessViewV1<'s>,
            Option<SemanticLocalIdV1>,
        ) -> SliceResult<R>,
    ) -> SourceOwnedResultV18<R> {
        self.with_descriptor_replay_access_v18(
            root,
            instance,
            site,
            write,
            budget,
            |view, local, _| use_view(view, local),
        )
    }

    fn with_descriptor_replay_access_v18<R>(
        &self,
        root: usize,
        instance: usize,
        site: ProductionSliceAccessSiteV1,
        write: bool,
        budget: &mut SliceBudget<'_>,
        use_view: impl for<'s> FnOnce(
            &ProductionSliceAccessViewV1<'s>,
            Option<SemanticLocalIdV1>,
            &SourceSliceReplayFactsV18<'s>,
        ) -> SliceResult<R>,
    ) -> SourceOwnedResultV18<R> {
        let mut use_view = SourceCallbackCustodyV29::new(use_view);
        self.retain_query((|| {
            self.query(budget)?;
            let floor = budget.storage();
            let headers = source_slice_replay_headers_v18()?;
            let result =
                scoped_source_attempt_v29(self.source.cleanup, budget, floor, move |budget| {
                    // These facts outlive the whole-value origin table and remain live
                    // throughout the ABI join. Its transient result credit is separate.
                    budget.reserve_storage(headers)?;
                    let semantic = self.source.source_semantic(budget)?;
                    let (original_root, function_ordinal) = self.source.root(root, budget)?;
                    let (source_function, incoming) =
                        self.source.instance(root, instance, budget)?;
                    let selection = semantic
                        .select_kernel_body_for_root_v1(original_root)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected slice source body",
                        ))?;
                    budget.charge_work(4)?;
                    let source_root = semantic
                        .functions()
                        .get(original_root.index() as usize)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "original slice root declaration",
                        ))?;
                    let selected = semantic
                        .functions()
                        .get(source_function.index() as usize)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected slice declaration",
                        ))?;
                    if site.root != original_root
                        || site.function != source_function
                        || selection.body() != source_function
                        || (selection.body() == original_root
                            && (instance != 0 || incoming.is_some()))
                        || (selection.body() != original_root
                            && incoming != Some((0, source_root.entry())))
                    {
                        return self
                            .source
                            .missing("slice source/root/forwarding association");
                    }
                    let function = self.inventory.functions().get(function_ordinal).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("slice physical root function"),
                    )?;
                    let facts = super::value_origin_v1::with_whole_value_origins_v18(
                        self,
                        function.coordinate,
                        budget,
                        |origins, budget| {
                            SourceSliceQueryV18 {
                                owner: SourceSliceOwnerV18::Source {
                                    relation: self,
                                    root,
                                    instance,
                                },
                                inventory: self.inventory,
                                function,
                                site,
                                origins,
                            }
                            .descriptor_facts(write, budget)
                            .map_err(source_argument_error_v18)
                        },
                    )?;
                    let SliceDefinition::FunctionArgument {
                        function: input_function,
                        argument,
                    } = facts.common.input
                    else {
                        return self
                            .source
                            .missing("slice has no whole original input carrier");
                    };
                    if input_function != function.coordinate {
                        return self
                            .source
                            .missing("slice input belongs to another physical root");
                    }
                    budget.charge_work(selected.locals().len())?;
                    self.with_root_argument_data_v18(root, budget, |arguments, budget| {
                        with_slice_argument_v18(
                            arguments,
                            budget,
                            &facts.common,
                            argument,
                            |view| {
                                let mut direct_local = None;
                                if view.source().source_path().is_empty() {
                                    for (ordinal, local) in selected.locals().iter().enumerate() {
                                        if local.role()
                                            == SemanticLocalRoleV1::Argument(
                                                view.source().source_argument(),
                                            )
                                        {
                                            if local.ty() != view.source().source().ty()
                                                || direct_local.is_some()
                                            {
                                                return Err(
                                        ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                                    );
                                            }
                                            direct_local = Some(SemanticLocalIdV1::from_index(
                                                u32::try_from(ordinal)
                                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                            ));
                                        }
                                    }
                                }
                                use_view
                                    .take()
                                    .expect("slice access consumer is invoked once")(
                                    view,
                                    direct_local,
                                    &facts,
                                )
                            },
                        )
                    })
                })?;
            // The borrowed facts and argument join have ended. Never include
            // the shared attempt's own transient header in this release.
            budget
                .release_storage(headers)
                .inspect_err(|_| self.source.cleanup.deny_refund())?;
            Ok(result)
        })())
    }
}

#[derive(Clone, Copy)]
enum SourceSliceOwnerV18<'a> {
    Source {
        relation: &'a ProductionSourceCorrespondenceV18<'a>,
        root: usize,
        instance: usize,
    },
}

struct SourceSliceQueryV18<'a, 's, O = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> {
    owner: SourceSliceOwnerV18<'a>,
    inventory: &'a CanonicalKirInventoryV1<'a, O>,
    function: &'a CanonicalKirFunctionRefV1<'a>,
    site: ProductionSliceAccessSiteV1,
    origins: &'s super::value_origin_v1::WholeValueOriginsV1<'a, O>,
}

fn source_slice_query_error_v18(
    error: ProductionSourceOwnedViewErrorV18,
) -> ProductionSemanticKirErrorV1 {
    match error {
        ProductionSourceOwnedViewErrorV18::Resource(error) => error.into(),
        // The original query failure remains in the actual source guard. This
        // internal adapter carries no construction or replacement-source path.
        ProductionSourceOwnedViewErrorV18::Binding(_)
        | ProductionSourceOwnedViewErrorV18::Analysis(_)
        | ProductionSourceOwnedViewErrorV18::PrivateMemory(_)
        | ProductionSourceOwnedViewErrorV18::Source(_) => {
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        }
    }
}

impl<'a, O> SourceSliceQueryV18<'a, '_, O> {
    fn operation(
        &self,
        coordinate: SliceOperation,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        budget.charge_work(4)?;
        if coordinate.block.function != self.function.coordinate {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let blocks = self
            .inventory
            .blocks()
            .get(self.function.blocks.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let block = blocks
            .get(coordinate.block.block as usize)
            .filter(|block| block.coordinate == coordinate.block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.inventory
            .operations()
            .get(block.operations.clone())
            .and_then(|operations| operations.get(coordinate.operation as usize))
            .filter(|operation| operation.coordinate == coordinate)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn origin(&self, value: ValueId, budget: &mut SliceBudget<'_>) -> SliceResult<SliceDefinition> {
        self.origins
            .resolve(value, budget)
            .map_err(slice_inventory_error)?
            .ok_or_else(|| {
                self.site
                    .unsupported("slice access has conflicting or ungrounded SSA origins")
            })
    }

    fn descriptor_origin(
        &self,
        value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        match source_descriptor_origin_v30(
            self.inventory,
            self.function,
            self.origins,
            value,
            budget,
        )? {
            DescriptorOriginV30::Exact(origin) => Ok(origin),
            DescriptorOriginV30::Unknown => Err(self
                .site
                .unsupported("slice access has conflicting or ungrounded SSA origins")),
            DescriptorOriginV30::Cyclic => Err(self
                .site
                .unsupported("slice descriptor transport is cyclic")),
        }
    }

    fn defining_operation(
        &self,
        value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<&'a CanonicalKirOperationRefV1<'a>> {
        match self.origin(value, budget)? {
            SliceDefinition::Result {
                operation,
                result: 0,
            } => self.operation(operation, budget),
            _ => Err(self
                .site
                .unsupported("slice access operand has no supported defining operation")),
        }
    }

    fn is_retained_private(
        &self,
        mut pointer: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<bool> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let origin = self
                .origins
                .resolve(pointer, budget)
                .map_err(slice_inventory_error)?;
            let origin = origin.ok_or_else(|| {
                self.site.unsupported(
                    "retained private storage has conflicting or ungrounded SSA origins",
                )
            })?;
            let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            else {
                return Ok(false);
            };
            let actual = self.operation(operation, budget)?;
            let block = &self.inventory.blocks()
                [self.function.blocks.start + operation.block.block as usize];
            let _location =
                FunctionOperationLocation::new(block.block.id, operation.operation as usize);
            let SourceSliceOwnerV18::Source { relation, root, .. } = self.owner;
            let retained = relation
                .retained_scalar_allocation(root, operation, budget)
                .map_err(source_slice_query_error_v18)?;
            if retained {
                return Ok(true);
            }
            match &actual.operation.kind {
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => pointer = *value,
                _ => return Ok(false),
            }
        }
        Err(self
            .site
            .unsupported("retained private storage transport is cyclic"))
    }

    fn visit_site_operations(
        &self,
        budget: &mut SliceBudget<'_>,
        mut visit: impl FnMut(
            &'a CanonicalKirOperationRefV1<'a>,
            &mut SliceBudget<'_>,
        ) -> SliceResult<()>,
    ) -> SliceResult<()> {
        let SourceSliceOwnerV18::Source {
            relation,
            root,
            instance,
        } = self.owner;
        {
            let rows = relation
                .source_operation_rows(root, instance, self.site.block, self.site.statement, budget)
                .map_err(source_slice_query_error_v18)?;
            for row in rows {
                let mapped = relation
                    .mapped_source_operation(row.location, budget)
                    .map_err(source_slice_query_error_v18)?;
                match mapped {
                    ProductionSourceOperationV18::Operation(coordinate) => {
                        visit(self.operation(coordinate, budget)?, budget)?;
                    }
                    ProductionSourceOperationV18::Gap { .. }
                    | ProductionSourceOperationV18::RemovedCall
                    | ProductionSourceOperationV18::NoOperations => {}
                }
            }
            return Ok(());
        }
    }

    fn access(&self, budget: &mut SliceBudget<'_>) -> SliceResult<SliceAccess> {
        let mut ordinal = 0_u32;
        let mut selected = None;
        self.visit_site_operations(budget, |operation, budget| {
            let effects = self
                .inventory
                .effects()
                .get(operation.effects.clone())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.charge_work(effects.len())?;
            let mut effects = effects.iter().filter(|effect| {
                matches!(
                    effect.effect,
                    KirLocalMemoryEffectRefV1::Read(_)
                        | KirLocalMemoryEffectRefV1::Write(_)
                        | KirLocalMemoryEffectRefV1::VolatileRead(_)
                        | KirLocalMemoryEffectRefV1::VolatileWrite(_)
                        | KirLocalMemoryEffectRefV1::Atomic { .. }
                )
            });
            try_visit_kir_memory_accesses_v1(
                operation.operation,
                |(pointer, access, space, atomic)| -> SliceResult<()> {
                    budget.charge_work(1)?;
                    let effect = effects
                        .next()
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    let plain_private = space == dialect_kernel::MemorySpaceAttr::Private
                        && atomic.is_none()
                        && matches!(
                            (&operation.operation.kind, access),
                            (
                                OperationKind::Load { .. },
                                dialect_kernel::AccessKindAttr::Read
                            ) | (
                                OperationKind::Store { .. },
                                dialect_kernel::AccessKindAttr::Write
                            )
                        );
                    if plain_private && self.is_retained_private(pointer, budget)? {
                        return Ok(());
                    }
                    if ordinal == self.site.access {
                        selected = Some(effect.coordinate);
                    }
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    Ok(())
                },
            )?;
            if effects.next().is_some() {
                return Err(self
                    .site
                    .unsupported("source slice span has an unmodeled memory effect"));
            }
            Ok(())
        })?;
        selected.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn index_origin(
        &self,
        mut value: ValueId,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        for _ in 0..self.function.definitions.len() {
            budget.charge_work(1)?;
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(
                definition.ty,
                Type::Scalar(ScalarType::Index | ScalarType::U64)
            ) {
                return Err(self
                    .site
                    .unsupported("slice bounds require the exact unsigned index domain"));
            }
            let origin = self.origin(value, budget)?;
            if let SliceDefinition::Result {
                operation,
                result: 0,
            } = origin
            {
                let operation = self.operation(operation, budget)?;
                if let OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: input,
                    to,
                } = &operation.operation.kind
                    && matches!(to, Type::Scalar(ScalarType::Index | ScalarType::U64))
                {
                    value = *input;
                    continue;
                }
            }
            return Ok(origin);
        }
        Err(self.site.unsupported("slice index transport is cyclic"))
    }

    fn descriptor_facts(
        &self,
        write: bool,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SourceSliceReplayFactsV18<'a>> {
        let access = self.access(budget)?;
        let read = self.operation(access.operation, budget)?;
        let (pointer, memory, loaded_type) = match (&read.operation.kind, write) {
            (OperationKind::Load { pointer, access }, false) => {
                let [result] = read.operation.results.as_slice() else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                (pointer, access, &result.ty)
            }
            (
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                },
                true,
            ) if read.operation.results.is_empty() => {
                budget.reserve_storage(descriptor_write_lookup_headers_v18()?)?;
                let definition = self
                    .inventory
                    .definition_for_value(self.function.coordinate, *value, budget)
                    .map_err(slice_inventory_error)?
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if definition.ty.as_scalar().is_none() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                (pointer, access, definition.ty)
            }
            _ => {
                return Err(self.site.unsupported(if write {
                    "checked slice correspondence requires a plain scalar write"
                } else {
                    "checked slice correspondence requires a plain read"
                }));
            }
        };
        if !matches!(
            memory.address_space,
            AddressSpace::Global | AddressSpace::Generic
        ) || memory.volatile
            || access.effect != 0
        {
            return Err(self
                .site
                .unsupported("checked slice correspondence requires a nonvolatile global read"));
        }
        let address = self.defining_operation(*pointer, budget)?;
        let OperationKind::GetElementPointer { base, offset } = address.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address is not an exact element projection"));
        };
        let data = self.defining_operation(base, budget)?;
        let OperationKind::SliceData { slice: data_slice } = data.operation.kind else {
            return Err(self
                .site
                .unsupported("slice read address has no exact slice data carrier"));
        };
        let (definition, success_edge) = self.assertion_endpoint(budget)?;
        let edges = self
            .inventory
            .edges()
            .get(self.function.edges.clone())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(edges.len())?;
        let mut predecessors = edges
            .iter()
            .filter(|edge| edge.target == access.operation.block);
        if predecessors.next().map(|edge| edge.coordinate) != Some(success_edge)
            || predecessors.next().is_some()
        {
            return Err(self
                .site
                .unsupported("slice read requires the unique assertion-success predecessor"));
        }
        let SliceDefinition::Result {
            operation: compare,
            result: 0,
        } = definition
        else {
            return Err(self
                .site
                .unsupported("slice assertion has no exact comparison definition"));
        };
        let compare = self.operation(compare, budget)?;
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = compare.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion is not an exact less-than comparison"));
        };
        let index = self.index_origin(offset, budget)?;
        if index != self.index_origin(lhs, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SliceDefinition::Result {
            operation: length,
            result: 0,
        } = self.index_origin(rhs, budget)?
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let length = self.operation(length, budget)?;
        let OperationKind::SliceLength {
            slice: length_slice,
        } = length.operation.kind
        else {
            return Err(self
                .site
                .unsupported("slice assertion does not use a slice length"));
        };
        let input = self.descriptor_origin(data_slice, budget)?;
        if input != self.descriptor_origin(length_slice, budget)? {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !matches!(input, SliceDefinition::FunctionArgument { .. }) {
            return Err(self
                .site
                .unsupported("slice carrier is not exact whole-entry transport"));
        }
        let SliceDefinition::FunctionArgument { function, argument } = input else {
            unreachable!()
        };
        let Type::Slice(root_slice) = self
            .function
            .function
            .signature
            .parameters
            .get(argument as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let Type::Slice(data_type) = self
            .inventory
            .definition_for_value(self.function.coordinate, data_slice, budget)
            .map_err(slice_inventory_error)?
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if function != self.function.coordinate
            || root_slice.address_space != AddressSpace::Global
            || data_type.address_space != memory.address_space
            || root_slice.element != data_type.element
            || root_slice.access != data_type.access
            || (write && root_slice.access != AccessMode::ReadWrite)
            || (write && root_slice.element.as_ref() != loaded_type)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let carrier = |value, budget: &mut SliceBudget<'_>| -> SliceResult<SliceDefinition> {
            let definition = self
                .inventory
                .definition_for_value(self.function.coordinate, value, budget)
                .map_err(slice_inventory_error)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(definition.ty, Type::Slice(_)) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(definition.coordinate)
        };
        Ok(SourceSliceReplayFactsV18 {
            common: SliceFacts {
                access,
                data_carrier: carrier(data_slice, budget)?,
                length_carrier: carrier(length_slice, budget)?,
                input,
                index,
                memory: *memory,
                loaded_type,
            },
            address_operation: address.coordinate,
            data_operation: data.coordinate,
            length_operation: length.coordinate,
            guard_condition: definition,
            guard_edge: success_edge,
        })
    }

    fn assertion_endpoint(
        &self,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<(
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    )> {
        let SourceSliceOwnerV18::Source {
            relation,
            root,
            instance,
        } = self.owner;
        let assertion = relation
            .assertion(root, instance, self.site.assertion, budget)
            .map_err(source_slice_query_error_v18)?;
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            definition,
            success_edge,
            ..
        } = assertion.outcome()
        else {
            return Err(self
                .site
                .unsupported("slice assertion was elided by an existing rule"));
        };
        if !assertion.expected() || assertion.semantic_success() != self.site.block {
            return Err(self
                .site
                .unsupported("slice read is not the selected positive assertion successor"));
        }
        Ok((definition, success_edge))
    }
}
