// Entry source coordinates are inert. The initializer census independently
// reconstructs the physical ABI parameter before any object memory admission.
fn source_object_entry_local_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticLocalIdV1, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<SemanticLocalIdV1>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 24)?;
        let ScopedObjectSourceV29::EntryArgument { local } = endpoint.source else {
            return Err(scoped_object_error_v29());
        };
        let original = plan
            .instances
            .instance(instance)
            .ok_or_else(scoped_object_error_v29)?;
        let declaration = original
            .declaration()
            .locals()
            .get(local.index() as usize)
            .ok_or_else(scoped_object_error_v29)?;
        let types = plan.instances.owner().source_semantic().types();
        let shape = types
            .get(declaration.ty().index() as usize)
            .ok_or_else(scoped_object_error_v29)?
            .shape();
        let empty = ScopedObjectPathV29 { first: 0, count: 0 };
        if endpoint.object
            != (ScopedObjectIdentityV29::Local {
                instance,
                local,
                generation: 0,
            })
            || !declaration.role().is_entry_argument()
            || !matches!(
                shape,
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            )
            || endpoint.root_type != declaration.ty()
            || endpoint.projected_type != declaration.ty()
            || endpoint.root_schema != endpoint.projected_schema
            || endpoint.source_path != empty
            || endpoint.path != empty
        {
            return Err(scoped_object_error_v29());
        }
        let entry = plan
            .entries
            .get(instance.index())
            .and_then(|row| *row)
            .and_then(|index| plan.states.get(index))
            .and_then(|state| state.get(local.index() as usize))
            .ok_or_else(scoped_object_error_v29)?;
        let node = entry
            .node
            .and_then(|index| plan.nodes.get(index))
            .ok_or_else(scoped_object_error_v29)?;
        if entry.generation != 0
            || node.ty != declaration.ty()
            || !matches!(node.kind, SourceReferenceNodeKindV29::Plain(_))
        {
            return Err(scoped_object_error_v29());
        }
        Ok(local)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn emit_source_object_entry_v29(
        &mut self,
        entry: BlockId,
        identity: ScopedAllocationIdentityV29,
        slot: &SemanticRetainedLocalSlotV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } = identity else {
            return Err(scoped_object_error_v29());
        };
        let declaration = self
            .function
            .locals()
            .get(local as usize)
            .ok_or_else(scoped_object_error_v29)?;
        if !declaration.role().is_entry_argument() || generation != 0 {
            return Ok(());
        }
        if !matches!(
            self.types[declaration.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        ) {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(entry.0),
                None,
                "typed entry allocation requires source-bound object materialization",
            ));
        }
        let (endpoint, value, access) = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Type>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 12)?;
            let SemanticRetainedStorageV29::Object {
                cell,
                schema,
                bytes,
                alignment,
            } = slot.storage
            else {
                return Err(scoped_object_error_v29());
            };
            let local = SemanticLocalIdV1::from_index(local);
            let empty = ScopedObjectPathV29 { first: 0, count: 0 };
            let endpoint = ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Local {
                    instance: cursor.instance,
                    local,
                    generation: 0,
                },
                source: ScopedObjectSourceV29::EntryArgument { local },
                root_type: slot.semantic_type,
                projected_type: slot.semantic_type,
                root_schema: schema,
                projected_schema: schema,
                source_path: empty,
                path: empty,
            };
            source_object_entry_local_v29(plan, cursor.instance, endpoint, budget)?;
            if this
                .scoped_memory
                .as_ref()
                .is_none_or(|recorder| recorder.frame.is_some())
                || !budget.source_object_storage_matches_v29(
                    plan,
                    cell,
                    cursor.instance,
                    local,
                    0,
                    schema,
                    Some((bytes, alignment)),
                )?
            {
                return Err(scoped_object_error_v29());
            }
            let expected = lower_scalar_type(this.types, slot.semantic_type)?;
            let Some(SemanticValueBindingV1::Value { id, ty }) = this
                .locals
                .get(local.index() as usize)
                .and_then(Option::as_ref)
            else {
                return Err(scoped_object_error_v29());
            };
            if *ty != expected {
                return Err(scoped_object_error_v29());
            }
            let access =
                memory_access_for_type(this.types, slot.semantic_type, AddressSpace::Private)?;
            if access.alignment != alignment {
                return Err(scoped_object_error_v29());
            }
            Ok((endpoint, *id, access))
        })?;
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::WriteValue {
                destination: endpoint,
                value: ScopedObjectValueOriginV29::Original(
                    ScopedMemoryStoreSourceV29::EntryArgument {
                        local: SemanticLocalIdV1::from_index(local),
                        ty: slot.semantic_type,
                    },
                ),
            },
            |this| {
                this.push_operation(operations, || {
                    Operation::new(
                        Vec::new(),
                        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            address: slot.pointer,
                            value,
                            access,
                        }),
                    )
                })
            },
        )
    }
}

fn source_reference_object_entry_store_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    slot: &ScopedSourceSlotV29,
    lowered: &LoweredFunctionResultV1,
    location: PrivateArrayPhysicalLocationV1,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    // Only the slot vectors survive the inventory query. ABI reconstruction
    // and source-join headers must not become retained slot storage.
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        source_reference_owned_prepay_v29::<()>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&ScopedMemoryAnchorV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&ScopedObjectPayloadV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<MemoryAccess>(plan, budget)?;
        budget.charge_work(8)?;
        let (
            ScopedAllocationIdentityV29::OriginalObject {
                local,
                generation: 0,
            },
            ScopedAllocationSourceV29::OriginalObject {
                cell,
                schema: selected,
            },
            ScopedSlotRepresentationV29::Object {
                schema,
                bytes,
                alignment,
            },
        ) = (
            slot.origin.identity,
            slot.origin.source,
            slot.representation,
        )
        else {
            return Err(scoped_object_error_v29());
        };
        if selected != schema
            || slot.instance != instance
            || lowered.source_call_instance != Some(instance)
            || !budget.source_object_storage_matches_v29(
                plan,
                cell,
                instance,
                SemanticLocalIdV1::from_index(local),
                0,
                schema,
                Some((bytes, alignment)),
            )?
        {
            return Err(scoped_object_error_v29());
        }
        let anchors = lowered
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_object_error_v29)?;
        anchors.check_object_ledger(budget)?;
        budget.charge_work(argument_product_v1(anchors.rows.len(), 3)?)?;
        let mut matching = anchors
            .rows
            .iter()
            .filter(|row| row.block == location.block && row.position == location.operation);
        let anchor = matching.next().ok_or_else(scoped_object_error_v29)?;
        if matching.next().is_some() || anchor.source.is_some() {
            return Err(scoped_object_error_v29());
        }
        let payload = anchors.object_payload(anchor, budget)?;
        payload.check_operation(operation, budget)?;
        let ScopedObjectRoleV29::WriteValue {
            destination,
            value:
                ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::EntryArgument {
                    local: original,
                    ty,
                }),
        } = payload.role
        else {
            return Err(scoped_object_error_v29());
        };
        let local = SemanticLocalIdV1::from_index(local);
        if source_object_entry_local_v29(plan, instance, destination, budget)? != local
            || original != local
            || ty != slot.origin.semantic_type
            || destination.root_type != ty
            || destination.root_schema != schema
        {
            return Err(scoped_object_error_v29());
        }
        let parameter =
            source_reference_cell_initial_parameter_v29(plan, instance, local, lowered, budget)?;
        let access = memory_access_for_type(
            plan.instances.owner().source_semantic().types(),
            ty,
            AddressSpace::Private,
        )?;
        budget.charge_work(5)?;
        if access.alignment != alignment
            || !operation.results.is_empty()
            || !matches!(operation.kind, OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address, value, access: actual
        }) if address == slot.origin.pointer && value == parameter && actual == access)
        {
            return Err(scoped_object_error_v29());
        }
        Ok(())
    });
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
