impl<'a> SemanticFunctionLoweringV1<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new_interprocedural(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        defined_function_ids: BTreeMap<SemanticFunctionIdV1, FunctionId>,
        defined_function_signatures: BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1>,
        result_types: Vec<Type>,
        parameters: SemanticParameterBindingsV1<'_>,
        assert_failure_block: Option<BlockId>,
        required_workgroup: Option<[u32; 3]>,
        infallible_asserts: BTreeSet<u32>,
        launch_rank: u8,
        authenticated_ranked_control: bool,
        max_operations: usize,
        mut private_array_work: PrivateArrayRecorderWorkV1<'a>,
        private_array_sources: Option<PrivateArraySourcesV1<'_>>,
        call_returns: CallReturnBufferV1,
        mut emission_work: Option<&'a mut dyn SemanticEmissionBudgetV1>,
        emission_placement: SemanticEmissionPlacementV1,
        mut execution: Option<ExecutionAvailabilityV29<'a>>,
        lifecycle: Option<&'a mut (dyn ExecutionLifecycleConsumerV29 + 'a)>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if let Some(execution) = &execution {
            execution.check_source(function, semantic_ssa)?;
            if !std::ptr::eq(execution.cfg.types, types) {
                return Err(execution_cfg_error_v29());
            }
        }
        if let Some(consumer) = lifecycle.as_deref() {
            let cursor = execution
                .as_ref()
                .ok_or_else(execution_availability_error_v29)?;
            let budget = emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?;
            cursor.check_ledger(budget)?;
            let result = consumer.check_instance(cursor, budget);
            cursor.check_ledger(budget)?;
            result?;
        }
        let mut locals = vec![None; function.locals().len()];
        let option_producers = semantic_option_producers_v1(function, callables)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        let option_dominance = SemanticOptionDominanceV1::analyze(function, &option_producers)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        let enum_payload_dominance = SemanticEnumPayloadDominanceV1::analyze(function, types)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        if let Some(parameter_local_bindings) = parameters.local_bindings {
            for binding in parameter_local_bindings {
                let (local, value) = match binding {
                    PlannedParameterLocalBindingV1::Direct { local, value, ty } => (
                        *local,
                        SemanticValueBindingV1::Value {
                            id: *value,
                            ty: ty.clone(),
                        },
                    ),
                    PlannedParameterLocalBindingV1::Flattened {
                        local,
                        semantic_type,
                        values,
                    } => (
                        *local,
                        binding_from_value_defs(types, *semantic_type, values)?,
                    ),
                };
                locals[local] = Some(value);
            }
        } else {
            for ((_, local, _), (value, ty)) in parameters
                .declarations
                .iter()
                .zip(parameters.values.iter().zip(parameters.types))
            {
                locals[*local] = Some(SemanticValueBindingV1::Value {
                    id: *value,
                    ty: ty.clone(),
                });
            }
        }
        let nominal_floor = if let Some(cursor) = execution.as_mut() {
            cursor.install_call_parameters_v29(
                &parameters,
                &mut locals,
                emission_work.as_deref_mut(),
            )?
        } else {
            0
        };
        #[cfg(test)]
        if let Some(cursor) = &execution {
            // Inert fixture inputs only; no producer is constructed in production.
            for (local, binding) in &cursor.entry_seeds {
                let local = *local as usize;
                if !function
                    .locals()
                    .get(local)
                    .is_some_and(|local| local.role().is_entry_argument())
                    || cursor.cfg.nominal_locals[local] == 0
                    || locals[local].is_some()
                {
                    return Err(execution_cfg_error_v29());
                }
                locals[local] = Some(binding.clone());
            }
        }
        let parameter_floor = emission_placement.value_floor(parameters.values)?;
        let mut direct_parameters = BTreeMap::new();
        if let Some(parameter_local_bindings) = parameters.local_bindings {
            for binding in parameter_local_bindings {
                if let PlannedParameterLocalBindingV1::Direct { local, ty, .. } = binding {
                    let local = u32::try_from(*local).map_err(|_| {
                        unsupported(0, None, None, "parameter local does not fit Kernel IR")
                    })?;
                    if direct_parameters.insert(local, ty.clone()).is_some() {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
        }
        let mut next_value = u32::try_from(function.locals().len())
            .map_err(|_| unsupported(0, None, None, "local count does not fit Kernel IR"))?;
        next_value = next_value.max(parameter_floor).max(nominal_floor);
        let control_flow_ssa = SemanticControlFlowSsaPlanV1::analyze_with_execution_v29(
            SemanticSsaTransportInputV1 {
                types,
                callables,
                function,
                semantic_function,
            },
            semantic_ssa,
            &option_dominance,
            &direct_parameters,
            max_operations,
            max_operations,
            emission_work.as_deref_mut(),
            execution.as_ref(),
            lifecycle.as_deref(),
        )?;
        let workgroup_pipeline_contracts = workgroup_pipeline_type_contracts_v1(
            types,
            callables,
            &control_flow_ssa.compiler_issued_bindings,
        )?;
        let authenticated_loop_induction_bounds = if authenticated_ranked_control {
            authenticated_loop_induction_bounds_v1(types, function)?
        } else {
            BTreeMap::new()
        };
        let promoted_enum_variant_by_value = analyze_promoted_enum_variants_v1(
            types,
            function,
            &control_flow_ssa,
            max_operations,
            max_operations,
        )?;
        let mut retained_local_slots = BTreeMap::new();
        let private_array_enabled = control_flow_ssa.has_retained_arrays;
        for (local, plan) in &control_flow_ssa.retained_local_slots {
            let pointer = ValueId(next_value);
            next_value = next_value.checked_add(1).ok_or_else(|| {
                unsupported(0, None, None, "retained-local slot identity overflow")
            })?;
            retained_local_slots.insert(
                *local,
                SemanticRetainedLocalSlotV1 {
                    pointer,
                    semantic_type: plan.semantic_type,
                    kernel_type: plan.kernel_type.clone(),
                    alignment: plan.alignment,
                    array: plan.array,
                },
            );
        }
        let mut block_parameters = BTreeMap::new();
        for block in 0..function.blocks().len() as u32 {
            if block == function.entry().index() {
                continue;
            }
            let mut parameters = BTreeMap::new();
            for local in control_flow_ssa.live_in(block) {
                let promoted = control_flow_ssa
                    .promoted
                    .get(local)
                    .expect("live-in local must be promoted");
                let nominal = promoted.transport == SemanticPromotedTransportV1::Execution;
                let mut components = if nominal {
                    let budget = emission_work
                        .as_deref_mut()
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    reserve_execution_cfg_map_entry_v29::<u32, Vec<ValueDef>>(
                        parameters.len(),
                        budget,
                    )?;
                    reserve_execution_cfg_map_entry_v29::<u32, BTreeMap<u32, Vec<ValueDef>>>(
                        block_parameters.len(),
                        budget,
                    )?;
                    emission_vec_v1(promoted.kernel_types.len(), budget)?
                } else {
                    Vec::with_capacity(promoted.kernel_types.len())
                };
                for ty in &promoted.kernel_types {
                    let ty = if nominal {
                        execution_cfg_clone_type_v29(
                            ty,
                            emission_work
                                .as_deref_mut()
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        )?
                    } else {
                        ty.clone()
                    };
                    components.push(ValueDef::new(ValueId(next_value), ty));
                    next_value = next_value.checked_add(1).ok_or_else(|| {
                        unsupported(0, Some(block), None, "block-parameter identity overflow")
                    })?;
                }
                parameters.insert(*local, components);
            }
            block_parameters.insert(block, parameters);
        }
        let enum_payload_sources =
            plan_unique_enum_payload_sources_v1(types, function, &control_flow_ssa);
        let (enum_payload_storage, enum_payload_requires_compile_time_custody) =
            plan_enum_payload_storage_v1(
                types,
                function,
                &control_flow_ssa,
                &enum_payload_sources,
                &mut next_value,
            )?;
        let pending_semantic_ssa_definitions = control_flow_ssa
            .definition_values
            .iter()
            .map(|(site, values)| (*site, values.iter().copied().collect()))
            .collect();
        let mut private_array_outer = PrivateArrayPayloadV1::default();
        if private_array_enabled {
            private_array_work.activate()?;
            if let Some(sources) = private_array_sources {
                private_array_outer = sources.payload(&mut private_array_work)?;
            }
        }
        Ok(Self {
            fixed_array_analysis: None,
            private_arrays: PrivateArrayFunctionRecorderV1::new(
                private_array_work,
                private_array_enabled,
                max_operations,
                private_array_outer,
                emission_placement,
            ),
            types,
            callables,
            function,
            correspondence_owner,
            semantic_function,
            defined_function_ids,
            defined_function_signatures,
            result_types,
            locals,
            retained_local_slots,
            retained_local_allocas_emitted: false,
            retained_local_initialized: BTreeSet::new(),
            option_dominance,
            enum_payload_dominance,
            enum_payload_storage,
            enum_payload_sources,
            enum_payload_requires_compile_time_custody,
            enum_payload_compile_time_custody: BTreeMap::new(),
            enum_payload_allocas_emitted: false,
            control_flow_ssa,
            workgroup_pipeline_contracts,
            promoted_enum_variant_by_value,
            block_parameters,
            semantic_ssa_bindings: BTreeMap::new(),
            pending_semantic_ssa_definitions,
            next_value,
            emission_placement,
            emission_work,
            execution,
            execution_calls: None,
            lifecycle,
            assert_failure_block,
            required_workgroup,
            infallible_asserts,
            launch_rank,
            max_operations,
            emitted_operations: 0,
            emitted_workgroup_memory_extents: BTreeMap::new(),
            emitted_unsigned_constants: BTreeMap::new(),
            emitted_u32_constants: BTreeMap::new(),
            emitted_u32_bitand_masks: BTreeMap::new(),
            authenticated_loop_induction_bounds,
            emitted_unsigned_exclusive_bounds: BTreeMap::new(),
            generated_terminator_values: Vec::new(),
            call_returns,
        })
    }
}
