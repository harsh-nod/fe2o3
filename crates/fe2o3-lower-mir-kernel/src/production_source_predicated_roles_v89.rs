// The ordinary Store path and its pending value recipes remain unchanged.
// Only the scoped V88 original-call/native constructor supplies additional rows.
fn predicated_source_role_headers_v89<F>() -> SourceOwnedResultV18<usize> {
    type Frame<'a> = (
        CheckedDescriptorSourceRolesV18<'a>,
        PendingGlobalSourceAccessesV18<'a>,
        Vec<DescriptorSourceRoleRowV18>,
        DescriptorSourceRoleRowV18,
        GlobalSourceAccessPairV18,
        [GlobalSourceAccessEndpointV18; 2],
        [SliceOperation; 14],
        [SliceDefinition; 8],
        [ValueId; 16],
        [&'a (); 24],
        [usize; 12],
        [(usize, DescriptorSourceRoleV18); 4],
        std::array::IntoIter<(usize, DescriptorSourceRoleV18), 4>,
        Option<PendingSourceNativeWriteV88<'a>>,
        SourceOwnedResultV18<GlobalSourceAccessEndpointV18>,
        SourceOwnedResultV18<()>,
        std::slice::Iter<'a, DescriptorSourceRoleRowV18>,
    );
    Ok(argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        2 * size_of::<SourceOwnedResultV18<Frame<'_>>>(),
        4 * size_of::<&mut F>(),
    ])?)
}

fn predicated_source_endpoint_v89(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    locations: [SliceOperation; 7],
    root: ValueId,
    index: ValueId,
    scalar: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<GlobalSourceAccessEndpointV18> {
    budget.charge_work(64)?;
    let definition = |value, budget: &mut ArgumentBudgetV1<'_>| {
        inventory
            .definition_for_value(locations[6].block.function, value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .map(|row| row.coordinate)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "predicated source endpoint definition absent",
            ))
    };
    let store = source_operation_row_v18(inventory, locations[6], budget)?.operation;
    let OperationKind::GuardedStore {
        pointer,
        predicate,
        value,
        access,
    } = store.kind
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "predicated source endpoint is not a checked write",
        ));
    };
    let address = source_operation_row_v18(inventory, locations[5], budget)?.operation;
    let OperationKind::GetElementPointer { offset, .. } = address.kind else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "predicated source address is absent",
        ));
    };
    let extent = source_operation_row_v18(inventory, locations[1], budget)?.operation;
    let [extent] = extent.results.as_slice() else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "predicated source extent result differs",
        ));
    };
    if access.volatile
        || access.address_space != AddressSpace::Global
        || !store.results.is_empty()
        || !matches!(address.results.as_slice(), [result] if result.id == pointer)
        || extent.ty != Type::BOOL
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "predicated source endpoint shape differs",
        ));
    }
    Ok(GlobalSourceAccessEndpointV18 {
        logical: GlobalSourceLogicalEndpointV18 {
            access: SliceAccess {
                operation: locations[6],
                effect: 0,
            },
            root: definition(root, budget)?,
            index: definition(index, budget)?,
            length: locations[0],
            data: locations[4],
            address: locations[5],
            guard: GlobalSourceGuardV85::ExplicitPredicate {
                condition: definition(predicate, budget)?,
                bound_comparison: definition(extent.id, budget)?,
            },
        },
        address_index: definition(offset, budget)?,
        formation_pointer: pointer,
        pointer,
        value,
        memory: access,
        scalar,
        writing: true,
    })
}

fn predicated_source_pair_v89(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fact: &OptimizedSourceWriteV87,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<GlobalSourceAccessPairV18> {
    let input = predicated_source_endpoint_v89(
        original.inventory,
        fact.input,
        fact.source.root_input,
        fact.source.index,
        fact.source.element,
        budget,
    )?;
    let output = predicated_source_endpoint_v89(
        optimized.output_inventory(budget)?,
        fact.output,
        fact.root_input,
        fact.index,
        fact.source.element,
        budget,
    )?;
    Ok(GlobalSourceAccessPairV18 {
        instance: fact.source.instance.index(),
        origin: GlobalSourceAccessOriginV18::SourceWriteV89 {
            instance: fact.source.instance.index(),
            anchor: fact.source.anchor,
        },
        input,
        output,
    })
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn with_predicated_global_source_expressions_v89<'work, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'scope> FnMut(
            usize,
            &PendingGlobalSourceAccessesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        self.global_expression_entry_v23(optimized, budget)?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| {
            budget.reserve_storage(predicated_source_role_headers_v89::<F>()?)?;
            let mut roots = |root,
                             legacy: &PendingGlobalSourceAccessesV18<'_>,
                             budget: &mut ArgumentBudgetV1<'work>| {
                self.with_pending_source_native_writes_v88(optimized, root, native, budget, |writes, budget| {
                    let build = |budget: &mut ArgumentBudgetV1<'work>| {
                    let mut rows = emission_vec_v1(legacy.roles.rows.len(), budget).map_err(source_emission_error_v18)?;
                    budget.charge_work(legacy.roles.rows.len())?;
                    rows.extend_from_slice(legacy.roles.rows);
                    for ordinal in 0..writes.original_call_count(budget)? {
                        let Some(write) = writes.write(ordinal, budget)? else { continue; };
                        let pair = predicated_source_pair_v89(self, optimized, write.fact, budget)?;
                        if pair.output.logical.guard != write.guard {
                            return self.source.missing("predicated source/native predicate differs");
                        }
                        for (offset, role) in [(0, DescriptorSourceRoleV18::Length), (4, DescriptorSourceRoleV18::Data),
                            (5, DescriptorSourceRoleV18::Address), (6, DescriptorSourceRoleV18::Write)] {
                            budget.charge_work(12)?;
                            let index = descriptor_role_index_v18(&rows, write.fact.output[offset], budget)?;
                            let row = rows.get_mut(index).filter(|row| row.output == write.fact.output[offset])
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("predicated descriptor occurrence absent"))?;
                            if let Some(existing) = row.role {
                                // Pure CSE descendants may be shared by several authenticated calls.
                                // Effects may not be counted twice or replace an ordinary source role.
                                if existing != role || role == DescriptorSourceRoleV18::Write {
                                    return self.source.missing("predicated descriptor role overlaps an existing effect");
                                }
                                continue;
                            }
                            row.input = Some(write.fact.input[offset]);
                            row.role = Some(role);
                            if role == DescriptorSourceRoleV18::Write {
                                row.instance = Some(pair.instance);
                                row.global = Some(pair);
                                // V88 checked the exact source call's scalar payload and its optimized descendant.
                                // This does not modify ordinary Store's separate pending-recipe traversal.
                                row.write_recipe_pending = false;
                            }
                        }
                    }
                    let retained = budget.storage();
                    let roles = CheckedDescriptorSourceRolesV18 { original: self, optimized, root, rows: &rows, scope: DescriptorRoleScopeV18::new(budget) };
                    let result = consume(root, &PendingGlobalSourceAccessesV18 { roles: &roles }, budget);
                    let post = roles.check(budget).and_then(|()| writes.check(budget));
                    if budget.storage() != retained {
                        self.source.cleanup.deny_refund();
                        native.refuse_retained_custody();
                        return self.retain_query(Err(ArgumentResourceV1::Accounting.into()));
                    }
                    drop(roles);
                    drop(rows);
                    result.and(post)
                    };
                    let frame = argument_sum_v1(&[std::mem::size_of_val(&build), std::mem::align_of_val(&build)])?;
                    self.retain_query(source_scalar_normalization_scratch_v18(self.source.cleanup, budget, frame, build))
                })
            };
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&roots),
                std::mem::align_of_val(&roots),
            ])?)?;
            self.with_global_source_expressions_v23(optimized, budget, &mut roots)
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        self.retain_query(source_scalar_normalization_scratch_v18(
            self.source.cleanup,
            budget,
            frame,
            run,
        ))
    }
}

impl PendingGlobalSourceAccessesV18<'_> {
    fn check_predicated_native_pair_v89(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), PendingGlobalNativeErrorV18> {
        let original = self.original();
        original.retain_query((|| {
            budget.charge_work(160)?;
            if self.access(pair.output.logical.access.operation, budget)? != Some(pair)
                || !matches!(pair.origin, GlobalSourceAccessOriginV18::SourceWriteV89 { instance, .. } if instance == pair.instance)
                || !pair.input.writing || !pair.output.writing
            {
                return original.source.missing("predicated native substituted checked source pair");
            }
            let inventory = self.optimized().pending_global_output_v18(original, budget)?;
            native.check_owner(inventory.owner(), budget).map_err(slice_entry_native_error_v25)?;
            for coordinate in [pair.output.logical.length, pair.output.logical.data,
                pair.output.logical.address, pair.output.logical.access.operation] {
                let actual = native.operation(inventory.owner(), coordinate, budget).map_err(slice_entry_native_error_v25)?;
                let expected = source_operation_row_v18(inventory, coordinate, budget)?.operation;
                if !actual.is_some_and(|actual| std::ptr::eq(actual, expected)) {
                    return original.source.missing("predicated native carrier differs");
                }
            }
            let GlobalSourceGuardV85::ExplicitPredicate { condition, bound_comparison } = pair.output.logical.guard else {
                return original.source.missing("predicated native occurrence has a CFG guard");
            };
            let definitions = native.explicit_guard_definitions_v86(inventory.owner(), condition, bound_comparison, budget)
                .map_err(slice_entry_native_error_v25)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("predicated native predicate is absent"))?;
            for ((actual, value), expected) in definitions.into_iter().zip([condition, bound_comparison]) {
                let SliceDefinition::Result { operation, result } = expected else {
                    return original.source.missing("predicated native predicate is not an original result");
                };
                let expected = source_operation_row_v18(inventory, operation, budget)?.operation;
                if !std::ptr::eq(actual, expected)
                    || !expected.results.get(result as usize).is_some_and(|expected| std::ptr::eq(value, expected)) {
                    return original.source.missing("predicated native predicate definition differs");
                }
            }
            native.check_owner(inventory.owner(), budget).map_err(slice_entry_native_error_v25)
        })()).map_err(Into::into)
    }

    fn check_predicated_store_domain_v89(
        &self,
        facts: &GlobalStoreFactsV25<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        fact: &GlobalStoreFactV25<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.original();
        original.retain_query((|| {
            budget.charge_work(192)?;
            if self.access(pair.output.logical.access.operation, budget)? != Some(pair) {
                return original.source.missing("predicated domain substituted source pair");
            }
            let inventory = self.optimized().pending_global_output_v18(original, budget)?;
            let owner = facts.owner(budget).map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            let endpoint = &pair.output;
            let GlobalSourceGuardV85::ExplicitPredicate { condition, bound_comparison } = endpoint.logical.guard else {
                return original.source.missing("predicated domain requires an explicit source guard");
            };
            let root = optimized_source_definition_row_v18(inventory, endpoint.logical.root, budget)?;
            let index = optimized_source_definition_row_v18(inventory, endpoint.logical.index, budget)?;
            let selected = optimized_source_definition_row_v18(inventory, endpoint.address_index, budget)?;
            let predicate = optimized_source_definition_row_v18(inventory, condition, budget)?;
            let comparison = optimized_source_definition_row_v18(inventory, bound_comparison, budget)?;
            let domain = fact.domain();
            let length = source_operation_row_v18(inventory, endpoint.logical.length, budget)?.operation;
            let normalized = match fact.normalized_index_origin() {
                fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value) => index.value == Some(value),
                fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value) =>
                    index.value == Some(value) && matches!(index.coordinate, SliceDefinition::BlockArgument { .. }),
            };
            let SliceDefinition::Result { operation: selected_at, result: 0 } = selected.coordinate else {
                return original.source.missing("predicated address index is not its checked Select");
            };
            let selected_op = source_operation_row_v18(inventory, selected_at, budget)?.operation;
            let OperationKind::Select { condition: actual_predicate, true_value, false_value } = selected_op.kind else {
                return original.source.missing("predicated address formation lost zero selection");
            };
            let zero = inventory.definition_for_value(fact.operation().block.function, false_value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("predicated zero definition absent"))?;
            let SliceDefinition::Result { operation: zero_at, result: 0 } = zero.coordinate else {
                return original.source.missing("predicated false address is not a zero constant");
            };
            let zero = source_operation_row_v18(inventory, zero_at, budget)?.operation;
            let SliceDefinition::Result { operation: comparison_at, result: 0 } = comparison.coordinate else {
                return original.source.missing("predicated extent comparison is not an original result");
            };
            let comparison_op = source_operation_row_v18(inventory, comparison_at, budget)?.operation;
            let Some(bits) = endpoint.scalar.bit_width() else {
                return original.source.missing("predicated element width is unknown");
            };
            if !std::ptr::eq(owner, inventory.owner()) || !std::ptr::eq(fact.owner(), owner)
                || !matches!(pair.origin, GlobalSourceAccessOriginV18::SourceWriteV89 { .. })
                || !endpoint.writing || !pair.input.writing || endpoint.memory.volatile
                || endpoint.memory.address_space != AddressSpace::Global
                || fact.operation() != endpoint.logical.access.operation
                || domain.path() != fe2o3_kernel_ir::FormalGuardedPathV1::ExplicitPredicate
                || root.coordinate != (SliceDefinition::FunctionArgument { function: fact.operation().block.function, argument: domain.allocation().parameter_index() })
                || root.value != Some(domain.slice()) || !normalized
                || selected.ty != &Type::INDEX
                || index.value != Some(domain.index()) || index.value != Some(domain.guard_index()) || index.ty != &Type::INDEX
                || predicate.ty != &Type::BOOL || predicate.value != Some(actual_predicate) || predicate.value != Some(domain.predicate())
                || comparison.ty != &Type::BOOL
                || Some(true_value) != index.value
                || !matches!(zero.kind, OperationKind::Constant(Constant::Index(0)))
                || !matches!(zero.results.as_slice(), [result] if result.id == false_value && result.ty == Type::INDEX)
                || !matches!(comparison_op.kind, OperationKind::Compare { predicate: fe2o3_kernel_ir::ComparePredicate::LessThan, lhs, rhs } if (lhs, rhs) == fact.comparison_operands())
                || !matches!(length.results.as_slice(), [result] if result.id == fact.normalized_length_origin() && result.ty == Type::INDEX)
                || !matches!(root.ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global && *slice.element == Type::Scalar(endpoint.scalar))
                || domain.pointer() != endpoint.pointer || endpoint.formation_pointer != endpoint.pointer
                || bits % 8 != 0 || !matches!(bits / 8, 1 | 2 | 4 | 8)
                || domain.element_bytes() != u64::from(bits / 8)
                || !endpoint.memory.alignment.is_power_of_two() || u64::from(endpoint.memory.alignment) > domain.element_bytes()
            {
                return original.source.missing("predicated source and local Store domain differ");
            }
            // Explicit predicate truth is conditional on the store executing.
            // No CFG true_at fact is fabricated for unconditional address formation.
            Ok(())
        })())
    }
}
