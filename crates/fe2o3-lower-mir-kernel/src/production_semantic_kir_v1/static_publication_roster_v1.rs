#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetainedStaticPublicationEventV1 {
    pub(crate) semantic_block: u32,
    pub(crate) semantic_statement: Option<u32>,
    pub(crate) semantic_ordinal: u32,
    pub(crate) kir_location: FunctionOperationLocation,
    pub(crate) ranked_block: u32,
    pub(crate) ranked_operation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetainedStaticPublicationRosterV1 {
    pub(crate) semantic_function: SemanticFunctionIdV1,
    pub(crate) selected_root: SemanticFunctionIdV1,
    // Store payload, release READY, release REQUEST, acquire, guarded read.
    pub(crate) events: [RetainedStaticPublicationEventV1; 5],
    pub(crate) payload_parameter: ValueId,
    pub(crate) flags_parameter: ValueId,
    pub(crate) producer_index: ValueId,
    pub(crate) payload_source_argument: u32,
    pub(crate) flags_source_argument: u32,
    pub(crate) consumer_index: ValueId,
    pub(crate) acquire: ValueId,
    pub(crate) guard_predicate: ValueId,
    pub(crate) global_extents: [u64; 3],
    pub(crate) workgroup_extents: [u64; 3],
    pub(crate) full_physical_workgroups: bool,
}

impl ProductionSemanticKirOwnerV1 {
    pub(crate) fn retained_static_publication_roster_v1(
        &self,
        kernel_id: &str,
    ) -> Result<Option<RetainedStaticPublicationRosterV1>, ProductionMemoryDischargeFailureV1> {
        let invalid = || {
            ProductionMemoryDischargeFailureV1::stage(
                "static publication requires an exact replayed source/ranked/Kernel IR roster",
            )
        };
        self.verify_equivalence().map_err(|_| invalid())?;
        let module = self.module();
        let kernel = module
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == kernel_id)
            .ok_or_else(invalid)?;
        let function = module
            .functions
            .iter()
            .find(|function| function.id == kernel.entry)
            .ok_or_else(invalid)?;
        let body = function.body.as_ref().ok_or_else(invalid)?;
        let mut records = self
            .correspondence
            .lowered_functions()
            .iter()
            .filter(|record| {
                record.role() == SemanticKirFunctionRoleV1::KernelEntry
                    && record.kernel_ir_function() == &kernel.entry
            });
        let record = records.next().ok_or_else(invalid)?;
        if records.next().is_some() {
            return Err(invalid());
        }
        let semantic_function = record.semantic_function();
        let semantic = self.semantic_ssa.source_semantic();
        let source = semantic
            .functions()
            .get(semantic_function.index() as usize)
            .ok_or_else(invalid)?;
        let remaining = self
            .limits
            .max_operations
            .checked_mul(UNSUPPORTED_INDEX_CORRELATION_STEPS_PER_OPERATION_V1)
            .ok_or_else(invalid)?;
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining };
        let mut terminal_sites = [None; 2];
        for (index, block) in source.blocks().iter().enumerate() {
            budget.charge().ok_or_else(invalid)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let slot = match semantic.callables().get(call.callee().index() as usize) {
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 { .. },
                    ..
                }) => 0,
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 { .. },
                    ..
                }) => 1,
                _ => continue,
            };
            let site = SemanticAccessSiteV1 {
                block: u32::try_from(index).map_err(|_| invalid())?,
                statement: None,
                ordinal: 0,
            };
            if terminal_sites[slot].replace(site).is_some() {
                return Err(invalid());
            }
        }
        if terminal_sites == [None, None] {
            return Ok(None);
        }
        let [Some(producer), Some(consumer)] = terminal_sites else {
            return Err(invalid());
        };
        let mut matching = self
            .generic_checks
            .iter()
            .filter(|checks| checks.function_name == kernel_id);
        let checks = matching.next().ok_or_else(invalid)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        if semantic
            .select_kernel_body_for_root_v1(checks.selected_root)
            .is_none_or(|selection| selection.body() != semantic_function)
        {
            return Err(invalid());
        }

        let mut launches = self
            .launch_roots
            .as_deref()
            .ok_or_else(invalid)?
            .iter()
            .filter(|launch| launch.selected_root == checks.selected_root);
        let launch = launches.next().ok_or_else(invalid)?;
        if launches.next().is_some()
            || launch.launch_rank != 1
            || launch.global_extents != [256, 1, 1]
            || launch.workgroup_extents != [128, 1, 1]
            || !launch.full_physical_workgroups
        {
            return Err(invalid());
        }
        let kir = build_kir_correlation_index(body, self.limits.max_operations, &mut budget)
            .ok_or_else(invalid)?;
        if !kir.unmodeled_memory_effects.is_empty() {
            return Err(invalid());
        }
        let sites = index_semantic_access_sites(
            &self.correspondence,
            record.correspondence_owner(),
            semantic_function,
            &kir,
            &mut budget,
        )
        .ok_or_else(invalid)?;
        let ranked = index_ranked_correlation(
            &checks.lowering,
            &checks.access_sources,
            self.limits.max_operations,
            &mut budget,
        )
        .ok_or_else(invalid)?;
        let correlation = StaticPublicationCorrelationV1 {
            semantic: Some(semantic),
            semantic_function,
            kir: &kir,
            sites: &sites,
            ranked: &ranked,
            lowering: &checks.lowering,
        };
        let mut consumers = [None; 5];
        for effect in &kir.memory_consumers {
            budget.charge().ok_or_else(invalid)?;
            let Some(site) = sites.get(&(effect.location, effect.operation_access_ordinal)) else {
                continue;
            };
            let position = if site.block == producer.block && site.statement.is_none() {
                if site.ordinal >= 2 {
                    return Err(invalid());
                }
                site.ordinal as usize
            } else if site.block == consumer.block && site.statement.is_none() {
                if site.ordinal >= 3 {
                    return Err(invalid());
                }
                2 + site.ordinal as usize
            } else {
                continue;
            };
            if effect.operation_access_ordinal != 0
                || consumers[position].replace(*effect).is_some()
            {
                return Err(invalid());
            }
        }
        let [
            Some(store),
            Some(ready),
            Some(request),
            Some(acquire),
            Some(read),
        ] = consumers
        else {
            return Err(invalid());
        };
        let consumer_site = SemanticAccessSiteV1 {
            ordinal: 2,
            ..consumer
        };
        correlation
            .authenticate_read(consumer_site, consumer_site, read, 3, &mut budget)
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
        let values = correlation
            .exact_consumer_roster(consumer_site, read, &mut budget)
            .ok_or_else(invalid)?;
        let (producer_payload, producer_flags, producer_index) = correlation
            .exact_producer_roster(producer, store, ready, &mut budget)
            .ok_or_else(invalid)?;
        let root = |value, budget: &mut UnsupportedIndexCorrelationBudgetV1| -> Option<ValueId> {
            let index = external_allocation_parameter_v1(
                function,
                &kir,
                value,
                &mut BTreeSet::new(),
                budget,
            )?;
            body.parameters.get(index as usize).copied()
        };
        let payload_parameter = root(producer_payload, &mut budget).ok_or_else(invalid)?;
        let flags_parameter = root(producer_flags, &mut budget).ok_or_else(invalid)?;
        let payload_source_argument = semantic_source_argument_for_kir_parameter_v1(
            &self.correspondence,
            record.correspondence_owner(),
            source,
            payload_parameter,
        )
        .ok_or_else(invalid)?;
        let flags_source_argument = semantic_source_argument_for_kir_parameter_v1(
            &self.correspondence,
            record.correspondence_owner(),
            source,
            flags_parameter,
        )
        .ok_or_else(invalid)?;
        if payload_parameter == flags_parameter
            || root(values.payload, &mut budget) != Some(payload_parameter)
            || root(values.flags, &mut budget) != Some(flags_parameter)
        {
            return Err(invalid());
        }
        // Bind the original physical roots to the corresponding source arguments,
        // not merely to equal-looking site or allocation labels.
        for (site, expected) in [
            (producer, [payload_parameter, flags_parameter]),
            (consumer, [payload_parameter, flags_parameter]),
        ] {
            budget.charge().ok_or_else(invalid)?;
            let SemanticTerminatorKindV1::Call(call) =
                source.blocks()[site.block as usize].terminator().kind()
            else {
                return Err(invalid());
            };
            for (argument, root) in call.arguments().iter().take(2).zip(expected) {
                budget.charge().ok_or_else(invalid)?;
                let (SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place)) = argument
                else {
                    return Err(invalid());
                };
                let source_argument = semantic_source_argument_for_kir_parameter_v1(
                    &self.correspondence,
                    record.correspondence_owner(),
                    source,
                    root,
                )
                .ok_or_else(invalid)?;
                let local = source
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(invalid)?;
                if !place.projections().is_empty()
                    || local.role() != SemanticLocalRoleV1::Argument(source_argument)
                {
                    return Err(invalid());
                }
            }
        }
        let mut events = [None; 5];
        for (index, effect) in [store, ready, request, acquire, read]
            .into_iter()
            .enumerate()
        {
            budget.charge().ok_or_else(invalid)?;
            let site = sites.get(&(effect.location, 0)).ok_or_else(invalid)?;
            let ranked = ranked.sources_by_site.get(site).ok_or_else(invalid)?;
            events[index] = Some(RetainedStaticPublicationEventV1 {
                semantic_block: site.block,
                semantic_statement: site.statement,
                semantic_ordinal: site.ordinal,
                kir_location: effect.location,
                ranked_block: ranked.ranked_block,
                ranked_operation: ranked.ranked_operation,
            });
        }
        let [Some(a), Some(b), Some(c), Some(d), Some(e)] = events else {
            return Err(invalid());
        };
        Ok(Some(RetainedStaticPublicationRosterV1 {
            semantic_function,
            selected_root: checks.selected_root,
            events: [a, b, c, d, e],
            payload_parameter,
            payload_source_argument,
            flags_source_argument,
            flags_parameter,
            producer_index,
            consumer_index: values.cell,
            acquire: values.acquired,
            guard_predicate: values.predicate,
            global_extents: launch.global_extents,
            workgroup_extents: launch.workgroup_extents,
            full_physical_workgroups: launch.full_physical_workgroups,
        }))
    }
}

impl StaticPublicationCorrelationV1<'_, '_> {
    fn exact_producer_roster(
        &self,
        site: SemanticAccessSiteV1,
        store: KirMemoryConsumerV1,
        ready: KirMemoryConsumerV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<(ValueId, ValueId, ValueId)> {
        if store.location.block != ready.location.block
            || store.location.operation_index >= ready.location.operation_index
        {
            return None;
        }
        budget.charge()?;
        let store_op = self.kir.operations.get(&store.location)?;
        let OperationKind::Store {
            pointer, access, ..
        } = store_op.kind
        else {
            return None;
        };
        if pointer != store.pointer
            || access != MemoryAccess::new(AddressSpace::Global, 4)
            || !store_op.results.is_empty()
        {
            return None;
        }
        budget.charge()?;
        let ready_op = self.kir.operations.get(&ready.location)?;
        let OperationKind::Atomic(atomic) = &ready_op.kind else {
            return None;
        };
        let value = atomic.value?;
        if !ready_op.results.is_empty()
            || ready_op.kind != static_publication_atomic_v1(ready.pointer, Some(value))
            || self.definition(value, budget)?.kind != OperationKind::Constant(Constant::U32(2))
        {
            return None;
        }
        let OperationKind::GetElementPointer { base, offset: cell } =
            self.definition(pointer, budget)?.kind
        else {
            return None;
        };
        let OperationKind::SliceData { slice: payload } = self.definition(base, budget)?.kind
        else {
            return None;
        };
        let OperationKind::GetElementPointer { base, offset } =
            self.definition(ready.pointer, budget)?.kind
        else {
            return None;
        };
        let OperationKind::SliceData { slice: flags } = self.definition(base, budget)?.kind else {
            return None;
        };
        if cell != offset || payload == flags {
            return None;
        }
        budget.charge()?;
        let ranked_store = self.ranked.sources_by_site.get(&site)?;
        budget.charge()?;
        let ranked_ready = self
            .ranked
            .sources_by_site
            .get(&SemanticAccessSiteV1 { ordinal: 1, ..site })?;
        if ranked_store.ranked_block != ranked_ready.ranked_block
            || ranked_store.ranked_operation >= ranked_ready.ranked_operation
        {
            return None;
        }
        let operations = self
            .lowering
            .kernel()
            .blocks()
            .get(ranked_store.ranked_block as usize)?
            .operations();
        let (payload_view, index) = match operations.get(ranked_store.ranked_operation as usize)? {
            ProductionRankedOperationV1::Access {
                kind: dialect_kernel::AccessKindAttr::Write,
                view,
                indices,
            }
            | ProductionRankedOperationV1::ValueAccess {
                kind: dialect_kernel::AccessKindAttr::Write,
                view,
                indices,
                ..
            } => (view, indices.as_slice()),
            _ => return None,
        };
        let ProductionRankedOperationV1::PublicationAtomicStoreU32 {
            view,
            index: flag_index,
            value: 2,
        } = operations.get(ranked_ready.ranked_operation as usize)?
        else {
            return None;
        };
        if payload_view == view || index != [*flag_index] {
            return None;
        }
        Some((payload, flags, cell))
    }
}
