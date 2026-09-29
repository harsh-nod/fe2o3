// This collection is private to the pending original-source transaction. A row
// is omitted from private memory only after every actual guard query succeeds.
struct SourceIssuedAccessesV29<'scope, 'owner, 'source> {
    instances: &'scope ExecutionInstancesV29<'owner>,
    source_index: &'scope SourceAddressSourceIndexV29<'source>,
    actual: SourceIssuedActualV29<'source>,
    originals: BTreeMap<usize, SourceIssuedOriginalV29<'scope, 'owner, 'source>>,
    issuers: BTreeSet<(usize, SsaValueV1)>,
    queries: Vec<SourceIssuedAccessV29>,
    transports: Vec<SourceIssuedRootTransportV29>,
    retained: PendingSourceIssuedRolesV29,
    owned: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl<'scope, 'owner, 'source> SourceIssuedAccessesV29<'scope, 'owner, 'source> {
    fn new(
        references: &SourceReferencePlanV29<'_, '_>,
        instances: &'scope ExecutionInstancesV29<'owner>,
        source_index: &'scope SourceAddressSourceIndexV29<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        references.check_owner(instances, budget)?;
        let floor = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut actual =
            SourceIssuedActualV29::from_function(&source_index.pending.function, budget)?;
        actual.bind_root_arguments(
            references,
            source_index,
            &source_index.pending.function,
            budget,
        )?;
        let owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(Self {
            instances,
            source_index,
            actual,
            originals: BTreeMap::new(),
            issuers: BTreeSet::new(),
            queries: Vec::new(),
            transports: Vec::new(),
            retained: PendingSourceIssuedRolesV29::empty(),
            owned,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor,
        })
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < argument_sum_v1(&[self.floor, self.owned])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn access(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
        row: &ScopedMemoryAnchorV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        references.plan.check_owner(self.instances, budget)?;
        let before = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<SourceIssuedRecipeV29>(),
            std::mem::size_of::<Result<Option<&ValueDef>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Constant>(),
            std::mem::size_of::<SourceIssuedRootTransportV29>(),
            std::mem::size_of::<PendingSourceIssuedIssuerV29>(),
            std::mem::size_of::<(SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29)>(),
            std::mem::size_of::<Result<(SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        budget.charge_work(5)?;
        let frame = row.source.ok_or_else(source_issued_error_v29)?;
        let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
            return Err(source_issued_error_v29());
        };
        if place.projections().len() != 1
            || place.projections()[0].kind() != SemanticProjectionKindV1::Dereference
        {
            return Err(source_issued_error_v29());
        }
        let resolved = self.resolve_access_v26(instance, frame.site, role, place, budget)?;
        let accepted = if let Some(SourceIssuedResolvedAccessV26 {
            issuer_instance,
            recipe,
            pointer,
        }) = resolved
        {
            if recipe.form != SourceIssuedFormV29::Pointer {
                return Err(source_issued_error_v29());
            }
            let function = self
                .instances
                .instance(instance)
                .ok_or_else(source_issued_error_v29)?
                .declaration();
            if function.locals()[place.local().index() as usize].ty() != recipe.pointer_type
                || lower_scalar_type(self.instances.owner().source_semantic().types(), place.ty())?
                    != Type::Scalar(recipe.element)
            {
                return Err(source_issued_error_v29());
            }
            let key = (issuer_instance.index(), recipe.issuer);
            charge_execution_cfg_lookup_v29(self.issuers.len(), budget)?;
            if !self.issuers.contains(&key) {
                charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
                let original = self
                    .originals
                    .get(&issuer_instance.index())
                    .ok_or_else(source_issued_error_v29)?;
                let (transport, retained) =
                    original.actual_issuer(recipe, references, &self.actual, budget)?;
                emission_push_v1(&mut self.transports, transport, budget)?;
                emission_push_v1(&mut self.retained.issuers, retained, budget)?;
                reserve_execution_cfg_map_entry_v29::<(usize, SsaValueV1), ()>(
                    self.issuers.len(),
                    budget,
                )?;
                self.issuers.insert(key);
            }
            self.source_index
                .frame_gap(instance, frame, row.block, row.position, budget)?;
            let operation =
                self.source_index
                    .emitted
                    .operation(instance, row.block, row.position, budget)?;
            let occurrences = self
                .instances
                .occurrences(instance)
                .ok_or_else(source_issued_error_v29)?;
            check_scoped_payload_v29(function, &occurrences, row, operation, budget)?;
            let access =
                source_address_value_access_v29(operation)?.ok_or_else(source_issued_error_v29)?;
            if access.object
                || access.pointer != pointer
                || !matches!(
                    access.access.address_space,
                    AddressSpace::Global | AddressSpace::Generic
                )
                || !source_issued_memory_pointer_v26(
                    &self.actual,
                    pointer,
                    access.access,
                    access.writing,
                    recipe.element,
                    budget,
                )?
                || access.access.alignment == 0
                || u64::from(access.access.alignment)
                    > self.instances.owner().source_semantic().types()[place.ty().index() as usize]
                        .layout()
                        .alignment_bytes()
                || (access.writing && recipe.access != AccessMode::ReadWrite)
                || *self.actual.value(access.value, budget)?.ty != Type::Scalar(recipe.element)
                || !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { pointer, .. } if pointer == access.pointer)
            {
                return Err(source_issued_error_v29());
            }
            charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
            let original = self
                .originals
                .get(&instance.index())
                .ok_or_else(source_issued_error_v29)?;
            check_source_issued_payload_v29(original, row, operation, &self.actual, budget)?;
            let (block, _) = self
                .source_index
                .emitted
                .point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    budget,
                )?
                .ok_or_else(source_issued_error_v29)?;
            emission_push_v1(
                &mut self.queries,
                SourceIssuedAccessV29 {
                    instance: instance.index(),
                    anchor,
                    issuer_instance,
                    issuer: recipe.issuer,
                    pointer,
                    issuer_pointer: recipe.pointer,
                    access: access.access,
                    writing: access.writing,
                    present: recipe.present,
                    block,
                    guard: None,
                },
                budget,
            )?;
            true
        } else {
            false
        };
        self.owned = argument_sum_v1(&[
            self.owned,
            budget
                .storage()
                .checked_sub(before)
                .ok_or(ArgumentResourceV1::Accounting)?,
        ])?;
        Ok(accepted)
    }

    fn finish(
        mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingSourceIssuedRolesV29, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let before = budget.storage();
        call_splice_sort_work_v1(argument_product_v1(self.queries.len(), 2)?, budget)
            .map_err(source_address_call_error_v29)?;
        self.queries
            .sort_unstable_by_key(|query| (query.instance, query.anchor));
        for pair in self.queries.windows(2) {
            budget.charge_work(2)?;
            if (pair[0].instance, pair[0].anchor) == (pair[1].instance, pair[1].anchor) {
                return Err(source_issued_error_v29());
            }
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<SourceIssuedGuardV29>>(),
            std::mem::size_of::<Result<Vec<SourceIssuedGuardV29>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Vec<std::ops::Range<usize>>>(),
            std::mem::size_of::<Result<Vec<std::ops::Range<usize>>, ProductionSemanticKirErrorV1>>(
            ),
            std::mem::size_of::<Result<PendingSourceIssuedRolesV29, ProductionSemanticKirErrorV1>>(
            ),
            std::mem::size_of::<PendingSourceIssuedRolesV29>(),
            std::mem::size_of::<PendingSourceIssuedAccessV29>(),
            std::mem::size_of::<Option<ProductionCallInstanceIdV1>>(),
            std::mem::size_of::<Result<ProductionCallInstanceIdV1, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Option<(BlockId, usize)>>(),
            std::mem::size_of::<Result<(BlockId, usize), ProductionSemanticKirErrorV1>>(),
            3 * std::mem::size_of::<usize>(),
        ])?)?;
        let guards =
            source_issued_guards_v29(&self.source_index.pending.function, &self.actual, budget)?;
        budget.reserve_storage(std::mem::size_of::<Vec<SourceIssuedPointerTransportV26>>())?;
        let mut pointer_transports = emission_vec_v1(self.queries.len(), budget)?;
        for query in &self.queries {
            budget.charge_work(3)?;
            pointer_transports.push(SourceIssuedPointerTransportV26 {
                pointer: query.pointer,
                issuer: query.issuer_pointer,
            });
        }
        check_source_issued_pointer_transports_v26(
            &self.source_index.pending.function,
            &self.actual,
            &pointer_transports,
            budget,
        )?;
        // Precompute ranges outside the CFG scope: it exclusively owns the
        // ledger and permits only closed, live-metered graph queries inside.
        let mut ranges = emission_vec_v1(self.queries.len(), budget)?;
        for query in &self.queries {
            budget.charge_work(argument_product_v1(
                call_splice_search_work_v1(guards.len()),
                2,
            )?)?;
            let first = guards.partition_point(|guard| guard.present < query.present);
            let end = guards.partition_point(|guard| guard.present <= query.present);
            ranges.push(first..end);
        }
        budget.charge_work(self.transports.len())?;
        fe2o3_kernel_ir::with_function_control_flow_v1(
            &self.source_index.pending.function,
            Default::default(),
            budget,
            |view| {
                for transport in &mut self.transports {
                    transport.checked = transport.receiver == transport.input
                        || view.unique_value_origin(transport.receiver)? == Some(transport.input);
                }
                for (query, range) in self.queries.iter_mut().zip(&ranges) {
                    for guard in &guards[range.clone()] {
                        if view.success_edge_dominates(guard.block, guard.edge, query.block)? {
                            query.guard = Some((guard.block, guard.edge));
                            break;
                        }
                    }
                }
                Ok(())
            },
        )
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
            _ => source_issued_error_v29(),
        })?;
        for transport in &self.transports {
            budget.charge_work(1)?;
            if !transport.checked {
                return Err(source_issued_error_v29());
            }
        }
        for query in &self.queries {
            budget.charge_work(3)?;
            let (guard_block, guard_edge) = query.guard.ok_or_else(source_issued_error_v29)?;
            let instance = self
                .instances
                .id_at(query.instance)
                .ok_or_else(source_issued_error_v29)?;
            emission_push_v1(
                &mut self.retained.accesses,
                PendingSourceIssuedAccessV29 {
                    instance,
                    anchor: query.anchor,
                    issuer_instance: query.issuer_instance,
                    issuer: query.issuer,
                    pointer: query.pointer,
                    access: query.access,
                    writing: query.writing,
                    guard_block,
                    guard_edge,
                },
                budget,
            )?;
        }
        call_splice_sort_work_v1(self.retained.issuers.len(), budget)
            .map_err(source_address_call_error_v29)?;
        self.retained
            .issuers
            .sort_unstable_by_key(|row| (row.instance.index(), row.block.index()));
        for pair in self.retained.issuers.windows(2) {
            budget.charge_work(2)?;
            if (pair[0].instance, pair[0].block) == (pair[1].instance, pair[1].block) {
                return Err(source_issued_error_v29());
            }
        }
        for issuer in &self.retained.issuers {
            charge_execution_cfg_lookup_v29(self.retained.sources.len(), budget)?;
            if self
                .retained
                .sources
                .binary_search_by_key(&(issuer.instance.index(), issuer.block.index()), |row| {
                    (row.instance.index(), row.block.index())
                })
                .is_err()
            {
                return Err(source_issued_error_v29());
            }
        }
        let extra = budget
            .storage()
            .checked_sub(before)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let owned = argument_sum_v1(&[self.owned, extra])?;
        let retained = std::mem::replace(&mut self.retained, PendingSourceIssuedRolesV29::empty());
        let refund = owned
            .checked_sub(retained.retained_storage()?)
            .ok_or(ArgumentResourceV1::Accounting)?;
        drop((self, guards, ranges));
        budget.release_storage(refund)?;
        Ok(retained)
    }
}

fn check_source_issued_payload_v29(
    original: &SourceIssuedOriginalV29<'_, '_, '_>,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let ScopedMemoryAnchorKindV29::Access {
        payload: Some(payload),
        ..
    } = row.kind
    else {
        return Err(source_issued_error_v29());
    };
    if matches!(payload, ScopedMemoryPayloadV29::Load { .. }) {
        return Ok(());
    }
    let ScopedMemoryPayloadV29::Store {
        value,
        source: ScopedMemoryStoreSourceV29::Operand {
            site, role, source, ..
        },
    } = payload
    else {
        return Err(source_issued_error_v29());
    };
    let function = original
        .instances
        .instance(original.instance)
        .ok_or_else(source_issued_error_v29)?
        .declaration();
    match (source, scoped_source_operand_v29(function, site, role)) {
        (
            ScopedMemoryOperandSourceV29::Place(
                occurrence @ ScopedMemoryOccurrenceV29::Promoted { .. },
            ),
            Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)),
        ) => {
            let archive = original
                .source_index
                .sidecar(original.instance, budget)?
                .execution_observation
                .as_ref()
                .ok_or_else(source_issued_error_v29)?;
            archive.check_original_v29(original.instances, original.instance, budget)?;
            check_scoped_payload_archive_v29(&archive.bindings, place, occurrence, value, budget)
        }
        (ScopedMemoryOperandSourceV29::Constant, Some(SemanticOperandV1::Constant(source))) => {
            budget.charge_work(5)?;
            let SemanticConstantValueV1::Scalar(bits) = source.value() else {
                return Err(source_issued_error_v29());
            };
            let ty = lower_scalar_type(
                original.instances.owner().source_semantic().types(),
                source.ty(),
            )?;
            let expected = lower_constant(ty, *bits)?;
            let Some(Operation {
                kind: OperationKind::Constant(actual),
                results,
            }) = actual.value(value, budget)?.operation
            else {
                return Err(source_issued_error_v29());
            };
            if results.len() != 1
                || results[0].id != value
                || *actual != expected
                || !matches!(operation.kind, OperationKind::Store { value: stored, .. } if stored == value)
            {
                return Err(source_issued_error_v29());
            }
            Ok(())
        }
        _ => Err(source_issued_error_v29()),
    }
}
