impl<'a> SemanticFunctionLoweringV1<'a> {
    #[cfg(test)]
    #[expect(
        clippy::too_many_arguments,
        reason = "Test-only adapter mirrors the production lowering constructor"
    )]
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        parameters: SemanticParameterBindingsV1<'_>,
        assert_failure_block: Option<BlockId>,
        required_workgroup: Option<[u32; 3]>,
        infallible_asserts: BTreeSet<u32>,
        launch_rank: u8,
        authenticated_ranked_control: bool,
        max_operations: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let semantic_function = SemanticFunctionIdV1::from_index(0);
        let semantic_ssa = plan_semantic_function_ssa_with_module_v1(
            semantic_function,
            function,
            types,
            callables,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        Self::new_interprocedural(
            types,
            callables,
            function,
            &semantic_ssa,
            semantic_function,
            semantic_function,
            BTreeMap::new(),
            BTreeMap::new(),
            Vec::new(),
            parameters,
            assert_failure_block,
            required_workgroup,
            infallible_asserts,
            launch_rank,
            authenticated_ranked_control,
            max_operations,
            PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(1, max_operations)),
            None,
            CallReturnBufferV1::for_function(
                function,
                callables,
                &BTreeMap::new(),
                0,
                &mut budget,
            )?,
            SemanticEmissionPlacementV1::default(),
        )
    }

    #[cfg(test)]
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
        private_array_work: PrivateArrayRecorderWorkV1<'a>,
        private_array_sources: Option<(&PrivateArrayMergeV1, Option<&PrivateArrayMergeV1>)>,
        call_returns: CallReturnBufferV1,
        emission_placement: SemanticEmissionPlacementV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_interprocedural_with_borrowed(
            types,
            callables,
            function,
            semantic_ssa,
            correspondence_owner,
            semantic_function,
            defined_function_ids,
            defined_function_signatures,
            result_types,
            parameters,
            assert_failure_block,
            required_workgroup,
            infallible_asserts,
            launch_rank,
            authenticated_ranked_control,
            max_operations,
            private_array_work,
            private_array_sources,
            call_returns,
            BorrowedAggregatePreparationV1::default(),
            None,
            emission_placement,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_interprocedural_with_borrowed(
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
        private_array_sources: Option<(&PrivateArrayMergeV1, Option<&PrivateArrayMergeV1>)>,
        call_returns: CallReturnBufferV1,
        borrowed_aggregate_preparation: BorrowedAggregatePreparationV1,
        mut borrowed_aggregate_work: Option<&'a mut dyn BorrowedAggregateBudgetV1>,
        emission_placement: SemanticEmissionPlacementV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut locals = vec![None; function.locals().len()];
        let mut borrowed_aggregate_views = Vec::new();
        let option_producers = semantic_option_producers_v1(function, callables)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        let option_dominance = SemanticOptionDominanceV1::analyze(function, &option_producers)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        let enum_payload_dominance = SemanticEnumPayloadDominanceV1::analyze(function, types)
            .map_err(|error| unsupported(0, None, None, error.detail()))?;
        if let Some(parameter_local_bindings) = parameters.local_bindings {
            for binding in parameter_local_bindings {
                let (local, value) = match binding {
                    PlannedParameterLocalBindingV1::BorrowedAggregate {
                        local,
                        shape,
                        values,
                    } => {
                        let budget = borrowed_aggregate_work.as_deref_mut().ok_or_else(|| {
                            borrowed_aggregate_error_v1(
                                "borrowed entry has no shared resource ledger",
                            )
                        })?;
                        let shape = shape.clone_in(budget)?;
                        let mut retained = borrowed_aggregate_vec_v1(values.len(), budget)?;
                        if values.len() != shape.leaves.len() {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        for (value, leaf) in values.iter().zip(&shape.leaves) {
                            if value.ty != leaf.parameter_type(shape.access, budget)? {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                            budget.reserve_storage(std::mem::size_of::<Type>())?;
                            retained.push(value.clone());
                        }
                        let local_id = SemanticLocalIdV1::from_index(*local as u32);
                        let owner = BorrowedAggregateSourceOwnerV1 {
                            root: correspondence_owner,
                            function: semantic_function,
                            local: local_id,
                            lifetime_start: BorrowedAggregateSourceAnchorV1::Entry {
                                local: local_id,
                            },
                        };
                        let view = borrowed_aggregate_views.len();
                        borrowed_aggregate_push_v1(
                            &mut borrowed_aggregate_views,
                            BorrowedAggregateViewV1 {
                                owner,
                                shape,
                                values: retained,
                            },
                            budget,
                        )?;
                        (*local, SemanticValueBindingV1::BorrowedAggregate { view })
                    }
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
        next_value = next_value.max(parameter_floor);
        let control_flow_ssa = SemanticControlFlowSsaPlanV1::analyze_with_borrowed(
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
            &borrowed_aggregate_preparation.locals,
            borrowed_aggregate_work.as_deref_mut(),
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
                let mut components = Vec::with_capacity(promoted.kernel_types.len());
                for ty in promoted.kernel_types.iter().cloned() {
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
            if let Some((root, outer)) = private_array_sources {
                private_array_outer = root.payload(0, 0, 0, &mut private_array_work)?;
                if let Some(outer) = outer {
                    private_array_outer = private_array_outer.add(
                        outer.payload(0, 0, 0, &mut private_array_work)?,
                        &mut private_array_work,
                    )?;
                }
            }
        }
        Ok(Self {
            borrowed_aggregate_work,
            borrowed_aggregate_function: None,
            borrowed_aggregate_preparation,
            borrowed_aggregate_views,
            borrowed_aggregate_storage: BTreeMap::new(),
            borrowed_aggregate_fields: Vec::new(),
            borrowed_aggregate_calls: Vec::new(),
            fixed_array_analysis: None,
            private_arrays: PrivateArrayFunctionRecorderV1::new(
                private_array_work,
                private_array_enabled,
                max_operations,
                private_array_outer,
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
