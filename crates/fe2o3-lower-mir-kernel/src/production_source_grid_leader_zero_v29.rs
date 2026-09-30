// A source effect with no physical components. This records a checked source
// disposition, not an address/content origin or a newly issued capability.
#[cfg(test)]
type SourceGridLeaderBindingObserverV29 =
    fn(&mut SemanticFunctionLoweringV1<'_, '_>, SemanticBlockIdV1, Option<u32>, usize);

#[cfg(test)]
thread_local! {
    static SOURCE_GRID_LEADER_BINDING_OBSERVER_V29: std::cell::Cell<Option<SourceGridLeaderBindingObserverV29>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedZeroObjectV29 {
    endpoint: ScopedObjectEndpointV29,
    access: SourceReferenceAccessV29,
    frame: ScopedMemoryFrameV29,
    block: BlockId,
    gap: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceGridLeaderBorrowV29 {
    site: SourceReferenceSiteV29,
    ty: SemanticTypeIdV1,
    origin: usize,
}

fn source_grid_leader_zero_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "logical GridLeader effect differs from its original source contract",
    )
}

fn source_grid_leader_zero_type_v29(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(source_grid_leader_zero_error_v29)?;
    if !matches!(declaration.shape(), SemanticTypeShapeV1::Aggregate(_))
        || declaration.layout().size_bytes() != Some(0)
        || declaration.layout().alignment_bytes() != 1
        || declaration.layout().is_uninhabited()
        || !matches!(
            declaration.layout().backend_repr(),
            SemanticBackendReprV1::Memory { .. }
        )
    {
        return Ok(false);
    }
    for callable in callables {
        budget.charge_work(2)?;
        if matches!(callable, SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader },
            ..
        } if *grid_leader == ty)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn check_grid_leader_origin_v29(
        &self,
        loan: usize,
        expected: SemanticTypeIdV1,
        source_type: SemanticTypeIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let plan = self.plan;
        source_reference_owned_prepay_v29::<SourceGridLeaderBorrowV29>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 14)?;
        let source = plan.instances.owner().source_semantic();
        let record = plan
            .loans
            .get(loan)
            .ok_or_else(source_grid_leader_zero_error_v29)?;
        let proof = self
            .grid_leaders
            .get(loan)
            .and_then(std::cell::Cell::get)
            .ok_or_else(source_grid_leader_zero_error_v29)?;
        if record.kind != SemanticBorrowKindV1::Shared
            || proof.site != record.site
            || proof.ty != expected
            || proof.origin != record.origin
            || plan.origins.get(record.origin).map(|origin| origin.ty) != Some(expected)
            || !self.claimed.get(loan).is_some_and(std::cell::Cell::get)
            || record.source_type != source_type
            || !matches!(source.types().get(source_type.index() as usize).map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Pointer(pointer))
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.pointee() == expected)
            || !source_grid_leader_zero_type_v29(
                source.types(),
                source.callables(),
                expected,
                budget,
            )
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
        {
            return Err(source_grid_leader_zero_error_v29());
        }
        Ok(())
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn prepay_grid_leader_availability_v29(
        &self,
        availability: SemanticCapabilityAvailabilityV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let SemanticCapabilityAvailabilityV1::EnumPayload { local, .. } = availability {
            let references = self
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let plan = references.plan;
            // Pay both original alternatives, including live-in's linear scan.
            charge_execution_cfg_lookup_v29(self.control_flow_ssa.live_in.len(), budget)?;
            budget.source_reference_charge_v29(plan, self.function.locals().len())?;
            charge_execution_cfg_lookup_v29(self.control_flow_ssa.entry_definitions.len(), budget)?;
            charge_execution_cfg_lookup_v29(
                self.control_flow_ssa.block_entry_values.len(),
                budget,
            )?;
            charge_execution_cfg_lookup_v29(self.promoted_enum_variant_by_value.len(), budget)?;
            let enum_type = self
                .function
                .locals()
                .get(local.index() as usize)
                .and_then(|local| self.types.get(local.ty().index() as usize))
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let SemanticTypeShapeV1::Enum { variants, .. } = enum_type.shape() else {
                return Err(source_grid_leader_zero_error_v29());
            };
            charge_execution_cfg_lookup_v29(variants.len(), budget)?;
        }
        Ok(())
    }

    fn source_grid_leader_object_binding_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        source: ScopedObjectEndpointV29,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            source_reference_emission_prepay_v29::<SemanticValueBindingV1>(budget)?;
            budget.charge_work(12)?;
            let local = place.local().index() as usize;
            if !place.projections().is_empty()
                || this.function.locals().get(local).map(|row| row.ty()) != Some(place.ty())
                || source.root_type != place.ty()
                || source.projected_type != place.ty()
                || source.root_schema != source.projected_schema
                || source.path.count != 0
                || source.source_path.count != 0
                || !matches!(source.object, ScopedObjectIdentityV29::Local { instance, local: actual, .. }
                    if actual == place.local() && this.execution.as_ref().is_some_and(|cursor| cursor.instance == instance))
                || !source_grid_leader_zero_type_v29(this.types, this.callables, place.ty(), budget)?
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            #[cfg(test)]
            if let Some(observe) = SOURCE_GRID_LEADER_BINDING_OBSERVER_V29.get() {
                observe(this, block, statement, local);
            }
            let Some(SemanticValueBindingV1::GridLeader { availability }) =
                this.locals.get(local).and_then(Option::as_ref)
            else {
                return Err(source_grid_leader_zero_error_v29());
            };
            // Only copy the original live logical binding. Layout, an empty
            // payload, and the endpoint alone cannot produce availability.
            let binding = SemanticValueBindingV1::GridLeader { availability: *availability };
            this.prepay_grid_leader_availability_v29(*availability, budget)?;
            this.check_binding_capability_availability_v29(block, statement, &binding)?;
            Ok(binding)
        })
    }

    fn capture_grid_leader_borrow_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        loan: usize,
        operations: &mut Vec<Operation>,
    ) -> Result<
        Option<(SourceGridLeaderBorrowV29, SemanticValueBindingV1)>,
        ProductionSemanticKirErrorV1,
    > {
        let origin = self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<
                Option<(SourceGridLeaderBorrowV29, SemanticValueBindingV1)>,
            >(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<usize>>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 5)?;
            let record = plan
                .loans
                .get(loan)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let origin = plan
                .origins
                .get(record.origin)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            if this.types[origin.ty.index() as usize].layout().size_bytes() != Some(0)
                || !source_grid_leader_zero_type_v29(this.types, this.callables, origin.ty, budget)?
            {
                return Ok(None);
            }
            source_reference_owned_prepay_v29::<SemanticValueBindingV1>(plan, budget)?;
            if record.site != site
                || record.kind != SemanticBorrowKindV1::Shared
                || origin.ty != place.ty()
                || !matches!(this.types[record.source_type.index() as usize].shape(),
                    SemanticTypeShapeV1::Pointer(pointer)
                        if pointer.kind() == SemanticPointerKindV1::Reference
                            && pointer.mutability() == SemanticMutabilityV1::Immutable
                            && pointer.pointee() == origin.ty)
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            Ok(Some(record.origin))
        })?;
        let Some(origin) = origin else {
            return Ok(None);
        };
        let access = SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared);
        let binding = if let Some((source, _, _)) =
            self.source_object_local_endpoint_v29(site.block, statement, place, access)?
        {
            self.source_grid_leader_object_binding_v29(site.block, statement, place, source)?
        } else {
            self.resolve_place_for_source_reference_access_v29(
                site.block, statement, place, access, operations,
            )?
            .0
        };
        let SemanticValueBindingV1::GridLeader { .. } = binding else {
            return Err(source_grid_leader_zero_error_v29());
        };
        // The place resolver checked availability in the original source function.
        // The receipt records that predicate, not an Option index to reinterpret
        // in a different helper. Only this original live loan may consume it.
        Ok(Some((
            SourceGridLeaderBorrowV29 {
                site,
                ty: place.ty(),
                origin,
            },
            binding,
        )))
    }

    fn lower_grid_leader_argument_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        absent_detail: &'static str,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let binding = self.lower_operand(block, None, &call.arguments()[1], operations)?;
        if matches!(binding, SemanticValueBindingV1::GridLeader { .. }) {
            return Ok(());
        }
        let SemanticValueBindingV1::SourceReference(binding) = binding else {
            return Err(unsupported(0, Some(block.index()), None, absent_detail));
        };
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let references = cursor
                .references
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_validate_binding_v29(plan, &binding, budget)?;
            budget.source_reference_charge_v29(plan, 4)?;
            let original = plan
                .instances
                .instance(cursor.instance)
                .ok_or_else(source_grid_leader_zero_error_v29)?
                .declaration();
            if !std::ptr::eq(original, this.function)
                || !matches!(original.blocks().get(block.index() as usize)
                    .map(|block| block.terminator().kind()),
                    Some(SemanticTerminatorKindV1::Call(actual)) if std::ptr::eq(actual, call))
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            let expected = match this.callables.get(call.callee().index() as usize) {
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                            grid_leader,
                            ..
                        },
                    ..
                }) => *grid_leader,
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                            witness,
                            kind: SemanticWriteOnlyDisjointWriteKindV1::GridExclusive,
                            ..
                        },
                    ..
                }) => *witness,
                _ => return Err(source_grid_leader_zero_error_v29()),
            };
            let loan = binding.origin.single_loan()?;
            if binding.source_type != call.arguments()[1].ty() {
                return Err(source_grid_leader_zero_error_v29());
            }
            references.check_grid_leader_origin_v29(loan, expected, binding.source_type, budget)?;
            // lower_operand has joined the current original occurrence and its
            // loan node; the immutable reference plan checked this call's live
            // ReadReferent effect. The retained proof authenticates the token,
            // independently of whatever physical address carries the reference.
            Ok(())
        })
    }

    fn record_grid_leader_zero_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        endpoint: ScopedObjectEndpointV29,
        access: SourceReferenceAccessV29,
        value: Option<&SemanticValueBindingV1>,
        volatility: SemanticVolatilityV1,
        gap: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let references = cursor
                .references
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<ScopedZeroObjectV29>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 15)?;
            let original = plan
                .instances
                .instance(cursor.instance)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let source = plan.instances.owner().source_semantic();
            if !std::ptr::eq(original.declaration(), this.function)
                || !std::ptr::eq(source.types(), this.types)
                || !std::ptr::eq(source.callables(), this.callables)
                || volatility != SemanticVolatilityV1::NonVolatile
                || !place.projections().is_empty()
                || !source_grid_leader_zero_type_v29(
                    this.types,
                    this.callables,
                    place.ty(),
                    budget,
                )?
                || endpoint.root_type != place.ty()
                || endpoint.projected_type != place.ty()
                || endpoint.root_schema != endpoint.projected_schema
                || endpoint.path.count != 0
                || endpoint.source_path.count != 0
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            let allowed = match (access, value) {
                (SourceReferenceAccessV29::Read, None) => true,
                (
                    SourceReferenceAccessV29::Write,
                    Some(SemanticValueBindingV1::GridLeader { availability }),
                ) => match availability {
                    SemanticCapabilityAvailabilityV1::Option(availability) => {
                        this.option_dominance.allows(*availability, block)
                    }
                    SemanticCapabilityAvailabilityV1::EnumPayload { local, variant } => {
                        this.prepay_grid_leader_availability_v29(*availability, budget)?;
                        this.enum_variant_is_available_v1(*local, *variant, block)
                    }
                },
                _ => false,
            };
            if !allowed {
                return Err(source_grid_leader_zero_error_v29());
            }
            let recorder = this
                .scoped_memory
                .as_mut()
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let frame = recorder
                .frame
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let physical_block = recorder
                .block
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(source_grid_leader_zero_error_v29());
            };
            if frame.site != execution_site_v29(block, statement)
                || endpoint.source
                    != (ScopedObjectSourceV29::Place {
                        site: frame.site,
                        role,
                        local: place.local(),
                        prefix: 0,
                    })
                || !scoped_object_original_place_v29(this.function, frame.site, role)
                    .is_some_and(|actual| std::ptr::eq(actual, place))
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            recorder.anchors.check_object_ledger(budget)?;
            source_reference_owned_push_v29(
                plan,
                &mut recorder.anchors.zero_objects,
                ScopedZeroObjectV29 {
                    endpoint,
                    access,
                    frame,
                    block: physical_block,
                    gap,
                },
                budget,
            )
        })
    }
}

fn check_source_grid_leader_zero_census_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    terminal: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let source = instances.owner().source_semantic();
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(1)?;
        let Some(anchors) = &sidecar.scoped_memory_anchors else {
            continue;
        };
        anchors.check_object_ledger(budget)?;
        for zero in &anchors.zero_objects {
            budget.charge_work(20)?;
            let instance = sidecar
                .source_call_instance
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            if !std::ptr::eq(source_index.sidecar(instance, budget)?, sidecar) {
                return Err(source_grid_leader_zero_error_v29());
            }
            let original = instances
                .instance(instance)
                .ok_or_else(source_grid_leader_zero_error_v29)?
                .declaration();
            let (
                ScopedObjectIdentityV29::Local {
                    instance: actual,
                    local,
                    generation,
                },
                ScopedObjectSourceV29::Place {
                    site,
                    role,
                    local: source_local,
                    prefix: 0,
                },
            ) = (zero.endpoint.object, zero.endpoint.source)
            else {
                return Err(source_grid_leader_zero_error_v29());
            };
            let place = scoped_object_original_place_v29(original, site, role)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            if actual != instance
                || local != source_local
                || local != place.local()
                || !place.projections().is_empty()
                || zero.frame != ScopedMemoryFrameV29::operand(site, Some(role))
                || !matches!(
                    zero.access,
                    SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
                )
                || zero.endpoint.root_type != place.ty()
                || zero.endpoint.projected_type != place.ty()
                || zero.endpoint.root_schema != zero.endpoint.projected_schema
                || zero.endpoint.source_path.count != 0
                || zero.endpoint.path.count != 0
                || !source_grid_leader_zero_type_v29(
                    source.types(),
                    source.callables(),
                    place.ty(),
                    budget,
                )?
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            let (block, statement) = scoped_memory_site_key_v29(site);
            let key = source_reference_access_key_v29(
                SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(block),
                    statement: statement.map(|value| value as usize),
                },
                place,
                zero.access,
            );
            charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
            let ordinal = *plan
                .access_sites
                .get(&key)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            let access = plan
                .accesses
                .get(ordinal)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            if access.instance != instance
                || access.local != local
                || access.source_local != place.local()
                || access.generation != generation
                || access.ty != place.ty()
                || access.loan.is_some()
                || access.shared_path
                || !access.projections.is_empty()
                || !access.traversed.is_empty()
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            let slot = source_address_object_slot_v29(
                instances,
                plan,
                slots,
                instance,
                local,
                generation,
                place.ty(),
                budget,
            )?;
            if slots.slots.get(slot).map(|slot| slot.representation)
                != Some(ScopedSlotRepresentationV29::Object {
                    schema: zero.endpoint.root_schema,
                    bytes: 0,
                    alignment: 1,
                })
            {
                return Err(source_grid_leader_zero_error_v29());
            }
            source_index.frame_gap(instance, zero.frame, zero.block, zero.gap, budget)?;
            let seen = terminal
                .get_mut(ordinal)
                .ok_or_else(source_grid_leader_zero_error_v29)?;
            if std::mem::replace(seen, true) {
                return Err(source_grid_leader_zero_error_v29());
            }
        }
    }
    Ok(())
}
