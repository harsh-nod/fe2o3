fn build_identity(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<(BuiltIdentityV1, ProductionAnalysisResourceUpperBoundV1), BuildIdentityFailureV1> {
    let preflight = preflight_identity_structure_v1(context, function, limits)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let textual_preflight_bound = identity_textual_preflight_resource_upper_bound_v1(preflight)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    limits
        .require(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            textual_preflight_bound,
        )
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let prescan = prescan(context, function)?;
    let closure_limits = limits
        .remaining_after_retained(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            textual_preflight_bound,
        )
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let closure_bound = def_use_closure_v1::check(context, function, &prescan, closure_limits)
        .map_err(|failure| def_use_closure_v1::identity_failure(context, &prescan, failure))?;
    let textual_preflight_bound =
        dominate_identity_preflight_bound_v1(closure_bound, textual_preflight_bound)
            .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let encode = |encoder: &mut IdentityEncoderV1,
                  block_ids: Option<&HashMap<Ptr<BasicBlock>, u64>>,
                  value_ids: Option<&HashMap<Value, u64>>| {
        encoder.record(
            PlironPreserveLocationV1::Function,
            "format",
            "ranked structural identity v1".to_owned(),
            |encoder| encoder.string(TRANSCRIPT_MAGIC_V1),
        )?;
        let root = function.get_operation();
        let root_raw = root.deref(context);
        let root_name =
            render_operation_name_v1(context, root, PlironPreserveLocationV1::Function)?;
        encoder.record(
            PlironPreserveLocationV1::Function,
            "operation",
            root_name.clone(),
            |encoder| {
                encoder.string(root_name.as_bytes())?;
                encoder.usize(1)
            },
        )?;
        encode_attributes(
            context,
            &root_raw.attributes,
            PlironPreserveLocationV1::Function,
            encoder,
        )?;

        for (block_index, block) in prescan.blocks.iter().copied().enumerate() {
            let block_ref = block.deref(context);
            let block_location = PlironPreserveLocationV1::Block { block: block_index };
            encoder.record(
                block_location.clone(),
                "block",
                format!(
                    "{} arguments, {} operations",
                    block_ref.get_num_arguments(),
                    prescan.operations[block_index].len()
                ),
                |encoder| {
                    encoder.usize(block_index)?;
                    encoder.usize(block_ref.get_num_arguments())?;
                    encoder.usize(prescan.operations[block_index].len())
                },
            )?;
            encode_attributes(
                context,
                &block_ref.attributes,
                block_location.clone(),
                encoder,
            )?;
            for (argument_index, argument) in block_ref.arguments().enumerate() {
                let (type_id, ty) = render_type(context, argument, block_location.clone())?;
                let value_id = value_ids.map_or(0, |values| values[&argument]);
                let mut summary = DiagnosticSummaryV1::default();
                summary.append(format_args!("argument {argument_index}: {ty}"));
                encoder.record(
                    block_location.clone(),
                    "block argument type",
                    summary.finish(),
                    |encoder| {
                        encoder.usize(argument_index)?;
                        encoder.u64(value_id)?;
                        encoder.string(type_id.as_bytes())?;
                        encoder.string(ty.as_bytes())
                    },
                )?;
            }

            for (operation_index, operation) in
                prescan.operations[block_index].iter().copied().enumerate()
            {
                let raw = operation.deref(context);
                let name = render_operation_name_v1(
                    context,
                    operation,
                    PlironPreserveLocationV1::Block { block: block_index },
                )?;
                let location = PlironPreserveLocationV1::Operation {
                    block: block_index,
                    operation: operation_index,
                    name: name.clone(),
                };
                encoder.record(location.clone(), "operation", name.clone(), |encoder| {
                    encoder.string(name.as_bytes())?;
                    encoder.usize(raw.get_num_results())?;
                    encoder.usize(raw.get_num_operands())?;
                    encoder.usize(raw.get_num_successors())
                })?;
                let mut result_summary = DiagnosticSummaryV1::default();
                if raw.get_num_results() == 0 {
                    result_summary.append(format_args!("no results"));
                }
                for result in raw.results() {
                    let (type_id, ty) = render_type(context, result, location.clone())?;
                    if !result_summary.is_empty() {
                        result_summary.append(format_args!(", "));
                    }
                    let value = value_ids.map_or(0, |values| values[&result]);
                    result_summary.append(format_args!("v{value}: {type_id} {ty}"));
                }
                encoder.record(
                    location.clone(),
                    "result types",
                    result_summary.finish(),
                    |encoder| {
                        encoder.usize(raw.get_num_results())?;
                        for result in raw.results() {
                            let (type_id, ty) = render_type(context, result, location.clone())?;
                            let value = value_ids.map_or(0, |values| values[&result]);
                            encoder.u64(value)?;
                            encoder.string(type_id.as_bytes())?;
                            encoder.string(ty.as_bytes())?;
                        }
                        Ok(())
                    },
                )?;
                let mut operand_summary = DiagnosticSummaryV1::default();
                if raw.get_num_operands() == 0 {
                    operand_summary.append(format_args!("no operands"));
                }
                for (operand_index, operand) in raw.operands().enumerate() {
                    let Some(value_id) = value_ids
                        .and_then(|values| values.get(&operand).copied())
                        .or((value_ids.is_none()).then_some(0))
                    else {
                        return Err(BuildIdentityFailureV1::from(
                            PlironIrIdentityErrorV1::ExternalOperand {
                                location: location.clone(),
                                operand: operand_index,
                                value: "<external SSA value>".to_owned(),
                            },
                        ));
                    };
                    if operand_index != 0 {
                        operand_summary.append(format_args!(", "));
                    }
                    operand_summary.append(format_args!("v{value_id}"));
                }
                encoder.record(
                    location.clone(),
                    "operands",
                    operand_summary.finish(),
                    |encoder| {
                        encoder.usize(raw.get_num_operands())?;
                        for (operand_index, operand) in raw.operands().enumerate() {
                            let Some(value_id) = value_ids
                                .and_then(|values| values.get(&operand).copied())
                                .or((value_ids.is_none()).then_some(0))
                            else {
                                return Err(PlironIrIdentityErrorV1::ExternalOperand {
                                    location: location.clone(),
                                    operand: operand_index,
                                    value: "<external SSA value>".to_owned(),
                                });
                            };
                            encoder.u64(value_id)?;
                        }
                        Ok(())
                    },
                )?;
                encode_attributes(context, &raw.attributes, location.clone(), encoder)?;
                let mut successor_summary = DiagnosticSummaryV1::default();
                if raw.get_num_successors() == 0 {
                    successor_summary.append(format_args!("no successors"));
                }
                for (successor_index, successor) in raw.successors().enumerate() {
                    let Some(block_id) = block_ids
                        .and_then(|blocks| blocks.get(&successor).copied())
                        .or((block_ids.is_none()).then_some(0))
                    else {
                        return Err(BuildIdentityFailureV1::from(
                            PlironIrIdentityErrorV1::ExternalSuccessor {
                                location: location.clone(),
                                successor: successor_index,
                            },
                        ));
                    };
                    if successor_index != 0 {
                        successor_summary.append(format_args!(", "));
                    }
                    successor_summary.append(format_args!("block {block_id}"));
                }
                encoder.record(
                    location.clone(),
                    "successors",
                    successor_summary.finish(),
                    |encoder| {
                        encoder.usize(raw.get_num_successors())?;
                        for (successor_index, successor) in raw.successors().enumerate() {
                            let Some(block_id) = block_ids
                                .and_then(|blocks| blocks.get(&successor).copied())
                                .or((block_ids.is_none()).then_some(0))
                            else {
                                return Err(PlironIrIdentityErrorV1::ExternalSuccessor {
                                    location: location.clone(),
                                    successor: successor_index,
                                });
                            };
                            encoder.u64(block_id)?;
                        }
                        Ok(())
                    },
                )?;
            }
        }
        Ok(())
    };
    let mut counter = IdentityEncoderV1::counting();
    encode(&mut counter, None, None)?;
    let result_count = prescan
        .values
        .checked_sub(prescan.block_arguments)
        .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
    let mut pipeline_creates = 0_usize;
    let mut pipeline_events = 0_usize;
    let mut ranked_accesses = 0_usize;
    let mut workgroup_ranked_accesses = 0_usize;
    let mut allocation_effects = 0_usize;
    let mut collective_transpose_candidates = 0_usize;
    let mut ownership_contracts = 0_usize;
    let mut effect_refinement_contracts = 0_usize;
    let mut index_lt_branch_candidates = 0_usize;
    let mut semantic_definitions = 0_usize;
    let mut semantic_refinement_contracts = 0_usize;
    for operation in prescan.operations.iter().flatten().copied() {
        let operation = Operation::get_op_dyn(operation, context);
        pipeline_creates += usize::from(operation.downcast_ref::<PipelineCreateOp>().is_some());
        pipeline_events += usize::from(operation.downcast_ref::<PipelineEventOp>().is_some());
        if let Some(access) = operation.downcast_ref::<RankedAccessOp>() {
            ranked_accesses += 1;
            workgroup_ranked_accesses += usize::from(
                access
                    .view(context)
                    .defining_op()
                    .and_then(|definition| {
                        Operation::get_op_dyn(definition, context)
                            .downcast_ref::<RankedViewOp>()
                            .copied()
                    })
                    .is_some_and(|view| {
                        view.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                    }),
            );
        }
        if let Some(effect) = operation.downcast_ref::<AllocationEffectOp>() {
            allocation_effects += 1;
            collective_transpose_candidates += usize::from(
                effect.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                    && matches!(
                        effect
                            .allocation_origin(context)
                            .zip(effect.noalias_class(context)),
                        Some(
                            (
                                GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
                                GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
                            ) | (
                                GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
                                GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1,
                            )
                        )
                    ),
            );
        }
        ownership_contracts +=
            usize::from(operation.downcast_ref::<OwnershipContractOp>().is_some());
        effect_refinement_contracts += usize::from(is_effect_refinement_contract_v1(&*operation));
        index_lt_branch_candidates += usize::from(
            operation
                .downcast_ref::<IndexLessThanBranchArgsOp>()
                .is_some(),
        );
        semantic_definitions += usize::from(
            is_semantic_refinement_definition_v1(&*operation)
                || operation
                    .downcast_ref::<dialect_kernel::SemanticTypedReadOp>()
                    .is_some(),
        );
        semantic_refinement_contracts +=
            usize::from(is_semantic_refinement_contract_v1(&*operation));
    }
    let input_census = ProductionAnalysisInputCensusV1 {
        blocks: prescan.blocks.len(),
        operations: prescan.operations.iter().map(Vec::len).sum(),
        operands: prescan.operands,
        results: result_count,
        successors: prescan.successors,
        block_arguments: prescan.block_arguments,
        attributes: prescan.attributes,
        type_nodes: prescan.type_nodes,
        identifier_bytes: counter.string_payload_bytes,
        canonical_bytes: counter.encoded_len,
        max_operation_arity: prescan.max_operation_arity,
        max_successor_arity: prescan.max_successor_arity,
        pipeline_creates,
        pipeline_events,
        ranked_accesses,
        workgroup_ranked_accesses,
        allocation_effects,
        collective_transpose_candidates,
        ownership_contracts,
        effect_refinement_contracts,
        index_lt_branch_candidates,
        semantic_definitions,
        semantic_refinement_contracts,
        native_switch_verification_work: preflight.native_switch_verification_work,
        native_switch_verification_scratch: preflight.native_switch_verification_scratch,
    };
    let record_summary_storage_upper_bound = counter
        .record_count
        .checked_mul(MAX_DIAGNOSTIC_DETAIL_CHARS_V1 * 4 + 3)
        .ok_or(BuildIdentityFailureV1::ResourceLimit(
            crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                resource: "identity diagnostic summary storage upper bound",
            },
        ))?;
    let resource_upper_bound = identity_capture_resource_upper_bound_v1(
        input_census,
        counter.record_count,
        record_summary_storage_upper_bound,
        counter.record_location_name_bytes,
    )
    .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    // Admit the cumulative preflight/capture work before native verification
    // or retained identity allocation, not after both phases have executed.
    let resource_upper_bound =
        dominate_identity_preflight_bound_v1(resource_upper_bound, textual_preflight_bound)
            .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    limits
        .require(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            resource_upper_bound,
        )
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    #[cfg(test)]
    def_use_closure_v1::full_verification();
    let verification = catch_unwind(AssertUnwindSafe(|| {
        scoped_verification_v1::verify(context, function)
    }));
    match verification {
        Err(_) => {
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: "the PLIRON verifier panicked".to_owned(),
            }
            .into());
        }
        Ok(Err(error)) => {
            let detail = render_bounded(
                PlironPreserveLocationV1::Function,
                "structural verifier diagnostic",
                |writer| write!(writer, "{error}"),
            )?;
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: truncate_detail(&detail),
            }
            .into());
        }
        Ok(Ok(())) => {}
    }
    let mut block_ids = HashMap::new();
    block_ids
        .try_reserve(prescan.blocks.len())
        .map_err(|_| canonical_bytes_resource_error(counter.encoded_len))?;
    for (index, block) in prescan.blocks.iter().copied().enumerate() {
        block_ids.insert(block, index as u64);
    }
    let mut value_ids = HashMap::<Value, u64>::new();
    value_ids
        .try_reserve(prescan.values)
        .map_err(|_| canonical_bytes_resource_error(counter.encoded_len))?;
    let mut next_value = 0_u64;
    for (block_index, block) in prescan.blocks.iter().copied().enumerate() {
        for argument in block.deref(context).arguments() {
            value_ids.insert(argument, next_value);
            next_value += 1;
        }
        for operation in &prescan.operations[block_index] {
            for result in operation.deref(context).results() {
                value_ids.insert(result, next_value);
                next_value += 1;
            }
        }
    }
    let mut encoder = IdentityEncoderV1::emitting(counter.encoded_len, counter.record_count)?;
    encode(&mut encoder, Some(&block_ids), Some(&value_ids))?;
    if encoder.encoded_len != counter.encoded_len
        || encoder.record_count != counter.record_count
        || encoder.string_payload_bytes != counter.string_payload_bytes
    {
        return Err(PlironIrIdentityErrorV1::TraversalPanicked.into());
    }
    let sha256 = Sha256::digest(&encoder.bytes).into();
    Ok((
        BuiltIdentityV1 {
            identity: PlironIrStructuralIdentityV1 {
                sha256,
                canonical: encoder.bytes,
                blocks: prescan.blocks.len(),
                operations: prescan.operations.iter().map(Vec::len).sum(),
                values: prescan.values,
            },
            records: encoder.records,
            input_census,
        },
        resource_upper_bound,
    ))
}
