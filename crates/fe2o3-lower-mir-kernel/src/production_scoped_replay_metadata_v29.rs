// Exact retained-metadata agreement with a fresh reconstruction. Allocation
// envelopes/capacities remain owned, but are not part of source semantics.
use super::*;

fn fixed<T: Copy + Eq>(
    a: T,
    b: T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[1, size_of::<T>()])?)?;
    Ok(a == b)
}

fn rows<T>(
    a: &[T],
    b: &[T],
    budget: &mut ArgumentBudgetV1<'_>,
    mut compare: impl FnMut(
        &T,
        &T,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        budget.charge_work(1)?;
        if !compare(a, b, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fixed_rows<T: Copy + Eq>(
    a: &[T],
    b: &[T],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    rows(a, b, budget, |a, b, budget| fixed(*a, *b, budget))
}

fn optional<T>(
    a: Option<&T>,
    b: Option<&T>,
    budget: &mut ArgumentBudgetV1<'_>,
    compare: impl FnOnce(
        &T,
        &T,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    match (a, b) {
        (None, None) => Ok(true),
        (Some(a), Some(b)) => compare(a, b, budget),
        _ => Ok(false),
    }
}

fn ledger(
    a: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    b: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    budget: &ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if a != budget.work_ledger_identity_v1() || b != a {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

fn coordinates(
    a: &OwnedInstanceCoordinatesV1,
    b: &OwnedInstanceCoordinatesV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let OwnedInstanceCoordinatesV1 {
        semantic_sha256,
        ssa,
        root,
        sources,
        seeds,
        spans,
        controls,
        anchors,
        returns,
        components,
        values,
        storage: _,
    } = a;
    Ok(fixed(
        (*semantic_sha256, *ssa, *root),
        (b.semantic_sha256, b.ssa, b.root),
        budget,
    )? && fixed_rows(&sources.rows, &b.sources.rows, budget)?
        && rows(&seeds.rows, &b.seeds.rows, budget, |a, b, budget| {
            let InstanceSeedV1 {
                instance,
                container,
                function_name,
                parameters,
            } = a;
            budget.charge_work(size_of::<InstanceSeedV1>())?;
            Ok(
                (instance, container, parameters) == (&b.instance, &b.container, &b.parameters)
                    && private_array_equal_bytes_v1(
                        function_name.as_bytes(),
                        b.function_name.as_bytes(),
                        budget,
                    )?,
            )
        })?
        && fixed_rows(&spans.rows, &b.spans.rows, budget)?
        && rows(&controls.rows, &b.controls.rows, budget, |a, b, budget| {
            let InstanceControlV1 {
                instance,
                original_block,
                semantic_block,
                physical_block,
                origin,
                return_values,
                expected_branch,
            } = a;
            budget.charge_work(size_of::<InstanceControlV1>())?;
            Ok((
                instance,
                original_block,
                semantic_block,
                physical_block,
                origin,
                return_values,
                expected_branch,
            ) == (
                &b.instance,
                &b.original_block,
                &b.semantic_block,
                &b.physical_block,
                &b.origin,
                &b.return_values,
                &b.expected_branch,
            ))
        })?
        && rows(&anchors.rows, &b.anchors.rows, budget, |a, b, budget| {
            let InstanceCallAnchorV1 {
                instance,
                source,
                physical,
                arguments,
                results,
                removed,
            } = a;
            budget.charge_work(size_of::<InstanceCallAnchorV1>())?;
            Ok((instance, source, physical, arguments, results, removed)
                == (
                    &b.instance,
                    &b.source,
                    &b.physical,
                    &b.arguments,
                    &b.results,
                    &b.removed,
                ))
        })?
        && rows(&returns.rows, &b.returns.rows, budget, |a, b, budget| {
            let InstanceReturnAnchorV1 {
                instance,
                source,
                original_block,
            } = a;
            budget.charge_work(size_of::<InstanceReturnAnchorV1>())?;
            Ok((instance, source, original_block) == (&b.instance, &b.source, &b.original_block))
        })?
        && fixed_rows(&components.rows, &b.components.rows, budget)?
        && fixed_rows(&values.rows, &b.values.rows, budget)?)
}

fn source_slots(
    a: &OwnedScopedSourceSlotsV29,
    b: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let OwnedScopedSourceSlotsV29 {
        source,
        ledger: owner,
        instances,
        slots,
        retained_storage: _,
    } = a;
    ledger(*owner, b.ledger, budget)?;
    Ok(fixed(*source, b.source, budget)?
        && rows(instances, &b.instances, budget, |a, b, budget| {
            let ScopedSourceSlotInstanceV29 {
                instance,
                function,
                incoming,
                placement,
                slots,
            } = a;
            budget.charge_work(size_of::<ScopedSourceSlotInstanceV29>())?;
            Ok((instance, function, incoming, placement, slots)
                == (
                    &b.instance,
                    &b.function,
                    &b.incoming,
                    &b.placement,
                    &b.slots,
                ))
        })?
        && fixed_rows(slots, &b.slots, budget)?)
}

fn initialization(
    a: &ScopedRetainedInitializationV29,
    b: &ScopedRetainedInitializationV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let ScopedRetainedInitializationV29 {
        subject,
        blocks,
        initialized_locals,
        retained_storage: _,
    } = a;
    ledger(subject.ledger, b.subject.ledger, budget)?;
    Ok(fixed(*subject, b.subject, budget)?
        && rows(blocks, &b.blocks, budget, |a, b, budget| {
            let ScopedInitializedEntryV29 { block, initialized } = a;
            budget.charge_work(size_of::<ScopedInitializedEntryV29>())?;
            Ok((block, initialized) == (&b.block, &b.initialized))
        })?
        && fixed_rows(initialized_locals, &b.initialized_locals, budget)?)
}

fn memory(
    a: &ScopedMemoryAnchorsV29,
    b: &ScopedMemoryAnchorsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let ScopedMemoryAnchorsV29 {
        subject,
        placement,
        rows,
    } = a;
    ledger(subject.ledger, b.subject.ledger, budget)?;
    Ok(
        fixed((*subject, *placement), (b.subject, b.placement), budget)?
            && fixed_rows(rows, &b.rows, budget)?,
    )
}

fn assertion(
    a: &PendingAssertOriginV1,
    b: &PendingAssertOriginV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let PendingAssertOriginV1 {
        site,
        emitted_function,
        block,
        first_operation,
        operation_count,
        expected,
        semantic_success,
        physical_success,
        argument_start,
        argument_count,
        outcome,
    } = a;
    budget.charge_work(size_of::<PendingAssertOriginV1>())?;
    let same_outcome = match (*outcome, b.outcome) {
        (
            PendingAssertOutcomeV1::Emitted {
                condition: ac,
                failure: af,
            },
            PendingAssertOutcomeV1::Emitted {
                condition: bc,
                failure: bf,
            },
        ) => (ac, af) == (bc, bf),
        (
            PendingAssertOutcomeV1::ElidedByExistingRule,
            PendingAssertOutcomeV1::ElidedByExistingRule,
        ) => true,
        _ => false,
    };
    Ok(same_outcome
        && (
            site,
            block,
            first_operation,
            operation_count,
            expected,
            semantic_success,
            physical_success,
            argument_start,
            argument_count,
        ) == (
            &b.site,
            &b.block,
            &b.first_operation,
            &b.operation_count,
            &b.expected,
            &b.semantic_success,
            &b.physical_success,
            &b.argument_start,
            &b.argument_count,
        )
        && private_array_equal_bytes_v1(
            emitted_function.as_bytes(),
            b.emitted_function.as_bytes(),
            budget,
        )?)
}

fn assertions(
    a: &InstanceAssertCaptureV1,
    b: &InstanceAssertCaptureV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let InstanceAssertCaptureV1 {
        ledger: owner,
        source,
        instance,
        function,
        placement,
        records,
        arguments,
        storage: _,
        failed,
    } = a;
    ledger(*owner, b.ledger, budget)?;
    budget.charge_work(2)?;
    Ok(!failed
        && !b.failed
        && fixed(
            (*source, *instance, *function, *placement),
            (b.source, b.instance, b.function, b.placement),
            budget,
        )?
        && rows(records, &b.records, budget, assertion)?
        && fixed_rows(arguments, &b.arguments, budget)?)
}

fn lifecycle(
    a: &PendingLifecycleEventsV29,
    b: &PendingLifecycleEventsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let PendingLifecycleEventsV29 {
        ledger: owner,
        source,
        instance,
        function,
        placement,
        provider,
        expected_rows,
        rows,
        retained_storage: _,
    } = a;
    ledger(*owner, b.ledger, budget)?;
    Ok(fixed(
        (
            *source,
            *instance,
            *function,
            *placement,
            *provider,
            *expected_rows,
        ),
        (
            b.source,
            b.instance,
            b.function,
            b.placement,
            b.provider,
            b.expected_rows,
        ),
        budget,
    )? && fixed_rows(rows, &b.rows, budget)?)
}

fn private_arrays(
    a: &PrivateArrayFunctionRowsV1,
    b: &PrivateArrayFunctionRowsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let PrivateArrayFunctionRowsV1 {
        active,
        placement,
        slots,
        effects,
        payload,
    } = a;
    let PrivateArrayPayloadV1 {
        occupied,
        capacity: _,
    } = payload;
    Ok(fixed(
        (*active, *placement, *occupied),
        (b.active, b.placement, b.payload.occupied),
        budget,
    )? && fixed_rows(slots, &b.slots, budget)?
        && fixed_rows(effects, &b.effects, budget)?)
}

fn component(
    a: &SemanticKirParameterComponentBindingV1,
    b: &SemanticKirParameterComponentBindingV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let SemanticKirParameterComponentBindingV1 {
        correspondence_owner,
        semantic_function,
        semantic_local,
        semantic_component_type,
        projection,
        kernel_ir_value,
    } = a;
    Ok(fixed(
        (
            *correspondence_owner,
            *semantic_function,
            *semantic_local,
            *semantic_component_type,
            *kernel_ir_value,
        ),
        (
            b.correspondence_owner,
            b.semantic_function,
            b.semantic_local,
            b.semantic_component_type,
            b.kernel_ir_value,
        ),
        budget,
    )? && fixed_rows(projection, &b.projection, budget)?)
}

fn sidecar(
    a: &PendingInstanceSidecarsV29,
    b: &PendingInstanceSidecarsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let PendingInstanceSidecarsV29 {
        next_value,
        #[cfg(test)]
            execution_observation: _,
        source_call_instance,
        scoped_slot_origins,
        scoped_initialization,
        scoped_memory_anchors,
        instance_assert_origins,
        lifecycle_events,
        private_arrays: arrays,
        operation_capabilities,
        diagnostic_declarations,
        float_declarations,
        blocks,
        statement_operation_spans,
        terminator_operation_spans,
        generated_terminator_values,
        call_returns,
        synthetic_operation_spans,
        parameter_bindings,
        parameter_component_bindings,
        ignored_parameter_bindings,
        emitted_operations,
    } = a;
    let CallReturnBufferV1 { sites, components } = call_returns;
    let CallPayloadBufferV1 {
        rows: call_sites,
        requested: _,
    } = sites;
    let CallPayloadBufferV1 {
        rows: call_components,
        requested: _,
    } = components;
    budget.charge_work(5)?;
    if !diagnostic_declarations.is_empty()
        || !float_declarations.is_empty()
        || !b.diagnostic_declarations.is_empty()
        || !b.float_declarations.is_empty()
        || operation_capabilities.len() != b.operation_capabilities.len()
    {
        return Ok(false);
    }
    for (a, b) in operation_capabilities.iter().zip(&b.operation_capabilities) {
        budget.charge_work(argument_sum_v1(&[
            scoped_capability_width_v29(a)?,
            scoped_capability_width_v29(b)?,
        ])?)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(fixed(
        (*next_value, *source_call_instance, *emitted_operations),
        (b.next_value, b.source_call_instance, b.emitted_operations),
        budget,
    )? && optional(
        scoped_slot_origins.as_ref(),
        b.scoped_slot_origins.as_ref(),
        budget,
        |a, b, budget| fixed_rows(a, b, budget),
    )? && optional(
        scoped_initialization.as_ref(),
        b.scoped_initialization.as_ref(),
        budget,
        initialization,
    )? && optional(
        scoped_memory_anchors.as_ref(),
        b.scoped_memory_anchors.as_ref(),
        budget,
        memory,
    )? && optional(
        instance_assert_origins.as_ref(),
        b.instance_assert_origins.as_ref(),
        budget,
        assertions,
    )? && optional(
        lifecycle_events.as_ref(),
        b.lifecycle_events.as_ref(),
        budget,
        lifecycle,
    )? && private_arrays(arrays, &b.private_arrays, budget)?
        && fixed_rows(blocks, &b.blocks, budget)?
        && fixed_rows(
            statement_operation_spans,
            &b.statement_operation_spans,
            budget,
        )?
        && fixed_rows(
            terminator_operation_spans,
            &b.terminator_operation_spans,
            budget,
        )?
        && fixed_rows(
            generated_terminator_values,
            &b.generated_terminator_values,
            budget,
        )?
        && fixed_rows(call_sites, &b.call_returns.sites.rows, budget)?
        && fixed_rows(call_components, &b.call_returns.components.rows, budget)?
        && fixed_rows(
            synthetic_operation_spans,
            &b.synthetic_operation_spans,
            budget,
        )?
        && fixed_rows(parameter_bindings, &b.parameter_bindings, budget)?
        && rows(
            parameter_component_bindings,
            &b.parameter_component_bindings,
            budget,
            component,
        )?
        && fixed_rows(
            ignored_parameter_bindings,
            &b.ignored_parameter_bindings,
            budget,
        )?)
}

pub(super) fn matches_roots(
    a: &[ScopedModuleRootV29],
    b: &[ScopedModuleRootV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    rows(a, b, budget, |a, b, budget| {
        let ScopedModuleRootV29 {
            function_ordinal,
            sidecars,
            coordinates: coords,
            slot_relocation,
            source_slots: slots,
            insertions,
            declarations,
            private_payload,
            requires_context_issue,
            inherited_emission_storage: _,
            inherited_assembly_storage: _,
        } = a;
        let PrivateArrayPayloadV1 {
            occupied,
            capacity: _,
        } = private_payload;
        Ok(fixed(
            (*function_ordinal, *requires_context_issue, *occupied),
            (
                b.function_ordinal,
                b.requires_context_issue,
                b.private_payload.occupied,
            ),
            budget,
        )? && coordinates(coords, &b.coordinates, budget)?
            && source_slots(slots, &b.source_slots, budget)?
            && optional(
                slot_relocation.as_ref(),
                b.slot_relocation.as_ref(),
                budget,
                |a, b, budget| a.matches_replay(b, budget),
            )?
            && fixed_rows(insertions, &b.insertions, budget)?
            && rows(declarations, &b.declarations, budget, |a, b, budget| {
                let ScopedDeclarationUseV29 {
                    instance,
                    kind,
                    id,
                    function_ordinal,
                } = a;
                Ok(fixed(
                    (*instance, *kind, *function_ordinal),
                    (b.instance, b.kind, b.function_ordinal),
                    budget,
                )? && private_array_equal_bytes_v1(
                    id.as_str().as_bytes(),
                    b.id.as_str().as_bytes(),
                    budget,
                )?)
            })?
            && rows(&sidecars.rows, &b.sidecars.rows, budget, sidecar)?)
    })
}
