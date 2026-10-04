// The new Unit-local rule is separate from the existing raw-empty call rules.
fn check_unit_local_unit_signature_v1(
    function: SemanticFunctionIdV1,
    types: &[SemanticTypeDeclV1],
    source: &SemanticFunctionDeclV1,
    physical: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(SemanticLocalIdV1, SemanticTypeIdV1), ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let bad = || {
        unsupported(
            function.index(),
            None,
            None,
            "local helper requires zero arguments and an ignored Unit result",
        )
    };
    let abi = source.abi();
    let unit = abi.source_output_type();
    if !abi.source_input_types().is_empty()
        || !abi.arguments().is_empty()
        || abi.fixed_count() != 0
        || abi.c_variadic()
        || !physical.signature.parameters.is_empty()
        || !physical.signature.results.is_empty()
        || !matches!(
            types
                .get(unit.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Unit)
        )
        || abi.return_value().ty() != unit
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
        || !matches!(
            abi.return_value().mode(),
            fe2o3_mir_model::semantic_mir_v1::SemanticAbiPassModeV1::Ignore
        )
    {
        return Err(bad());
    }
    budget.charge_work(
        source
            .locals()
            .len()
            .checked_mul(3)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
    )?;
    let mut returned = None;
    for (ordinal, local) in source.locals().iter().enumerate() {
        match local.role() {
            SemanticLocalRoleV1::Return => {
                if local.ty() != unit || returned.replace(ordinal).is_some() {
                    return Err(bad());
                }
            }
            SemanticLocalRoleV1::Argument(_) | SemanticLocalRoleV1::RustCallTupleField { .. } => {
                return Err(bad());
            }
            SemanticLocalRoleV1::Temporary => {}
        }
    }
    let ordinal = u32::try_from(returned.ok_or_else(bad)?)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    Ok((SemanticLocalIdV1::from_index(ordinal), unit))
}

#[derive(Clone, Copy)]
struct UnitLocalCallerLookupV1 {
    key: [usize; 2],
    group: usize,
}

struct UnitLocalCallLookupV1 {
    callers: Vec<UnitLocalCallerLookupV1>,
}

impl UnitLocalCallLookupV1 {
    fn build(
        groups: &[CanonicalCallGroupV1<'_>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let mut callers = unit_local_vec_v1(groups.len(), budget)?;
        for (group, row) in groups.iter().enumerate() {
            budget.charge_work(3)?;
            unit_local_push_v1(
                &mut callers,
                UnitLocalCallerLookupV1 {
                    key: [
                        row.function.source.correspondence_owner.index() as usize,
                        row.function.source.semantic_function.index() as usize,
                    ],
                    group,
                },
                budget,
            )?;
        }
        assert_origin_sort_v1(&mut callers, budget, |left, right, budget| {
            budget.charge_work(2)?;
            Ok(left.key.cmp(&right.key))
        })
        .map_err(call_index_error_v1)?;
        budget.charge_work(argument_product_v1(callers.len(), 2)?)?;
        if callers.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(unit_local_mismatch_v1());
        }
        Ok(Self { callers })
    }

    fn caller(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let key = [root.index() as usize, function.index() as usize];
        let index = assert_origin_find_v1(&self.callers, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.key.cmp(&key))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(1)?;
        Ok(self.callers[index].group)
    }
}

fn check_unit_local_calls_v1(
    subject: CanonicalCallSubjectV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    groups: &[CanonicalCallGroupV1<'_>],
    calls: &[CanonicalCallBindingV1<'_>],
    rows: &mut SealedUnitLocalSourceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    if rows.associations.is_empty() {
        return if rows.bodies.is_empty() {
            Ok(())
        } else {
            Err(mismatch())
        };
    }
    budget.charge_work(1)?;
    if !inventory.belongs_to(subject.executable) {
        return Err(mismatch());
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        let lookups = UnitLocalCallLookupV1::build(groups, budget)?;
        check_unit_local_indexed_calls_v1(subject, inventory, groups, calls, rows, &lookups, budget)
    })
}

fn check_unit_local_indexed_calls_v1(
    subject: CanonicalCallSubjectV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    groups: &[CanonicalCallGroupV1<'_>],
    calls: &[CanonicalCallBindingV1<'_>],
    rows: &mut SealedUnitLocalSourceV1,
    lookups: &UnitLocalCallLookupV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::{CanonicalKirBlockCoordinateV1, CanonicalKirOperationCoordinateV1};
    use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let semantic = subject.semantic_ssa.source_semantic();
    let captures = subject.semantic_ssa.occurrences_v1().ok_or_else(mismatch)?;
    for binding in calls {
        budget.charge_work(8)?;
        let callee_group = groups.get(binding.callee).ok_or_else(mismatch)?;
        let site = binding.site;
        let key = UnitLocalAssociationKeyV1 {
            root: site.callee.correspondence_owner,
            function: site.callee.semantic_function,
            physical: callee_group.function.canonical.coordinate.0 as usize,
        };
        let Some(association) = rows.find_association(key, budget)? else {
            // A different root's body proof cannot turn a missing association
            // into an old RawEmpty call. Body rows are physically sorted.
            let local_body = assert_origin_find_v1(&rows.bodies, budget, |body, budget| {
                budget.charge_work(1)?;
                Ok(body.physical.cmp(&key.physical))
            })?;
            if local_body.is_some() {
                return Err(mismatch());
            }
            continue;
        };
        let proof = *rows.associations.get(association).ok_or_else(mismatch)?;
        budget.charge_work(24)?;
        let bad = |detail| {
            unsupported(
                site.caller.semantic_function.index(),
                Some(site.anchor.semantic_block.index()),
                None,
                detail,
            )
        };
        if key != proof.key
            || site.caller.correspondence_owner != key.root
            || !std::ptr::eq(site.callee, callee_group.function.source)
            || !std::ptr::eq(site.callee_target, callee_group.function.canonical.function)
            || site.callee.role != SemanticKirFunctionRoleV1::InternalHelper
            || !site.source.arguments().is_empty()
            || !site.source.variadic_argument_abis().is_empty()
            || !matches!(site.source.unwind(), SemanticUnwindActionV1::Unreachable)
        {
            return Err(bad(
                "local helper call requires no source arguments and no unwind edge",
            ));
        }
        let Some(SemanticCallableDeclV1::Defined { function }) = semantic
            .callables()
            .get(site.source.callee().index() as usize)
        else {
            return Err(mismatch());
        };
        if *function != key.function {
            return Err(mismatch());
        }
        let destination = site.source.destination().ok_or_else(mismatch)?;
        let place = destination.place();
        if !place.projections().is_empty()
            || place.ty() != proof.unit_type
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err(bad(
                "local helper call requires an exact ignored Unit destination",
            ));
        }
        let caller_source = semantic
            .functions()
            .get(site.caller.semantic_function.index() as usize)
            .ok_or_else(mismatch)?;
        if caller_source
            .locals()
            .get(place.local().index() as usize)
            .is_none_or(|local| local.ty() != proof.unit_type)
        {
            return Err(mismatch());
        }
        let SemanticKirCallReturnKindV1::Call {
            arguments_first,
            call_operation,
            destination_end,
            destination: SemanticKirCallDestinationV1::Local,
            transport,
        } = site.anchor.kind
        else {
            return Err(mismatch());
        };
        let end = call_operation
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if site.anchor.correspondence_owner != key.root
            || site.anchor.semantic_function != site.caller.semantic_function
            || site.span.correspondence_owner != key.root
            || site.span.semantic_function != site.caller.semantic_function
            || site.span.semantic_block != site.anchor.semantic_block
            || site.span.kernel_ir_block != site.block.id
            || arguments_first != call_operation
            || destination_end != end
            || site.span.first_operation_ordinal != call_operation
            || site.span.operation_count != 1
            || !transport.range()?.is_empty()
        {
            return Err(bad(
                "local helper call has native argument/result/continuation work",
            ));
        }
        let operation = site
            .block
            .operations
            .get(call_operation as usize)
            .ok_or_else(mismatch)?;
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            return Err(mismatch());
        };
        budget.charge_work(argument_sum_v1(&[
            callee.as_str().len(),
            site.callee.kernel_ir_function.as_str().len(),
            4,
        ])?)?;
        if callee != &site.callee.kernel_ir_function
            || !arguments.is_empty()
            || !operation.results.is_empty()
        {
            return Err(mismatch());
        }
        let Some(Terminator::Branch {
            target: continuation,
            arguments,
        }) = &site.block.terminator
        else {
            return Err(mismatch());
        };
        if !arguments.is_empty() {
            return Err(bad("local helper call has native continuation values"));
        }

        // Preserve original group indices and reuse the live canonical block
        // inventory. Neither lowering order nor BlockId is a stored ordinal.
        let caller_index = lookups.caller(key.root, site.caller.semantic_function, budget)?;
        budget.charge_work(3)?;
        let caller_group = groups.get(caller_index).ok_or_else(mismatch)?;
        if !std::ptr::eq(caller_group.function.source, site.caller) {
            return Err(mismatch());
        }
        let caller_coordinate = caller_group.function.canonical.coordinate;
        let actual_caller = inventory
            .block_for_id(caller_coordinate, site.block.id, budget)
            .map_err(canonical_call_inventory_error_v1)?
            .ok_or_else(mismatch)?;
        budget.charge_work(2)?;
        if !std::ptr::eq(actual_caller.block, site.block) {
            return Err(mismatch());
        }
        let caller_block = actual_caller.coordinate.block as usize;
        let continuation_entry = inventory
            .block_for_id(caller_coordinate, *continuation, budget)
            .map_err(canonical_call_inventory_error_v1)?
            .ok_or_else(mismatch)?;
        budget.charge_work(2)?;
        let continuation_ordinal = continuation_entry.coordinate.block as usize;
        let continuation_block = continuation_entry.block;
        budget.charge_work(4)?;
        let continuation_span = caller_group
            .spans
            .get(continuation_ordinal)
            .ok_or_else(mismatch)?;
        if continuation_span.correspondence_owner != key.root
            || continuation_span.semantic_function != site.caller.semantic_function
            || continuation_span.semantic_block != destination.edge().target()
            || continuation_span.kernel_ir_block != *continuation
        {
            return Err(mismatch());
        }
        if !continuation_block.parameters.is_empty() {
            return Err(bad("local helper call has native continuation parameters"));
        }
        let caller = UnitLocalAssociationKeyV1 {
            root: key.root,
            function: site.caller.semantic_function,
            physical: caller_group.function.canonical.coordinate.0 as usize,
        };
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(site.anchor.semantic_block.index()), 0);
        let capture = captures.function(caller.function).ok_or_else(mismatch)?;
        budget.charge_work(8)?;
        let plan = subject
            .semantic_ssa
            .plan_for_function(caller.function)
            .ok_or_else(mismatch)?;
        if !std::ptr::eq(capture.owner(), subject.semantic_ssa)
            || plan.function_identity() != caller_source.identity()
        {
            return Err(mismatch());
        }
        let successor = assert_origin_find_v1(capture.successors(), budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.id().cmp(&edge))
        })?
        .ok_or_else(mismatch)?;
        if capture.successors()[successor].edge() != destination.edge() {
            return Err(mismatch());
        }
        let definition =
            assert_origin_find_v1(capture.edge_definitions(), budget, |row, budget| {
                budget.charge_work(3)?;
                Ok((row.edge(), row.ordinal()).cmp(&(edge, 0)))
            })?
            .ok_or_else(mismatch)?;
        let captured = &capture.edge_definitions()[definition];
        budget.charge_work(10)?;
        let definitions = plan.plan().edge_definitions(edge).ok_or_else(mismatch)?;
        if captured.variable().get() != place.local().index()
            || !captured.is_reachable()
            || definitions.len() != usize::from(captured.is_promoted())
            || captured.value() != definitions.first().map(|row| row.value())
            || definitions
                .first()
                .is_some_and(|row| row.variable() != captured.variable())
            || capture
                .edge_definitions()
                .get(definition + 1)
                .is_some_and(|row| row.edge() == edge)
        {
            return Err(mismatch());
        }
        let start = rows.values.len();
        rows.append_value(
            UnitLocalValueRowV1 {
                key: caller,
                origin: UnitLocalValueOriginV1::Edge {
                    edge,
                    variable: captured.variable(),
                },
                role: None,
                source_type: proof.unit_type,
                source_ssa: captured.value(),
                native: UnitLocalNativeValueV1::IgnoredUnit,
                recipe: UnitLocalValueRecipeV1::Unit,
                known_bits: None,
            },
            budget,
        )?;
        let arguments = plan.plan().edge_arguments(edge).ok_or_else(mismatch)?;
        let target = SsaBlockIdV1::new(destination.edge().target().index());
        let live = plan
            .plan()
            .transport_variables(target)
            .ok_or_else(mismatch)?;
        budget.charge_work(2)?;
        if arguments.len() != live.len() {
            return Err(mismatch());
        }
        budget.charge_work(argument_product_v1(arguments.len(), 8)?)?;
        for (argument, variable) in arguments.iter().zip(live) {
            let local = caller_source
                .locals()
                .get(variable.get() as usize)
                .ok_or_else(mismatch)?;
            if argument.variable() != *variable
                || !matches!(
                    semantic
                        .types()
                        .get(local.ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Unit)
                )
            {
                return Err(bad(
                    "local helper call continuation is not entirely ignored Unit",
                ));
            }
            rows.append_value(
                UnitLocalValueRowV1 {
                    key: caller,
                    origin: UnitLocalValueOriginV1::Edge {
                        edge,
                        variable: *variable,
                    },
                    role: None,
                    source_type: local.ty(),
                    source_ssa: Some(argument.value()),
                    native: UnitLocalNativeValueV1::IgnoredUnit,
                    recipe: UnitLocalValueRecipeV1::Unit,
                    known_bits: None,
                },
                budget,
            )?;
        }
        budget.charge_work(17)?;
        let returned = rows
            .control
            .get(proof.return_control)
            .ok_or_else(mismatch)?;
        let UnitLocalControlKindV1::Return {
            local,
            unit_type,
            unit_value,
        } = returned.kind
        else {
            return Err(mismatch());
        };
        let unit_row = rows.values.get(unit_value).ok_or_else(mismatch)?;
        let [return_site] = site.returns else {
            return Err(mismatch());
        };
        let SemanticKirCallReturnKindV1::Return { components } = return_site.kind else {
            return Err(mismatch());
        };
        if returned.association != association
            || returned.source_block != Some(return_site.semantic_block)
            || return_site.correspondence_owner != key.root
            || return_site.semantic_function != key.function
            || !components.range()?.is_empty()
            || local != proof.unit_return_local
            || unit_type != proof.unit_type
            || unit_row.key != key
            || unit_row.source_type != proof.unit_type
            || !matches!(unit_row.native, UnitLocalNativeValueV1::IgnoredUnit)
            || !matches!(unit_row.recipe, UnitLocalValueRecipeV1::Unit)
            || unit_row.role.is_some()
            || !matches!(unit_row.origin, UnitLocalValueOriginV1::UnitReturn { block, local: found }
                if block == return_site.semantic_block && found == local)
        {
            return Err(mismatch());
        }
        let end = rows.values.len();
        rows.record_call(
            association,
            UnitLocalCallRowV1 {
                caller,
                source_block: site.anchor.semantic_block,
                callee_association: association,
                call: CanonicalKirOperationCoordinateV1 {
                    block: CanonicalKirBlockCoordinateV1 {
                        function: caller_group.function.canonical.coordinate,
                        block: u32::try_from(caller_block).map_err(|_| mismatch())?,
                    },
                    operation: call_operation,
                },
                destination_local: place.local(),
                unit_type: proof.unit_type,
                source_edge: edge,
                continuation_source: destination.edge().target(),
                continuation_physical: *continuation,
                ignored_values: (start, end),
                return_control: proof.return_control,
            },
            budget,
        )?;
    }
    budget.charge_work(rows.associations.len())?;
    if rows.associations.iter().any(|row| row.call_count == 0) {
        return Err(mismatch());
    }
    sort_unit_local_calls_v1(&mut rows.calls, budget)?;
    Ok(())
}
