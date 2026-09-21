fn build_identity(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<(BuiltIdentityV1, ProductionAnalysisResourceUpperBoundV1), BuildIdentityFailureV1> {
    build_identity_observed_v1(context, function, limits, None)
}

fn build_identity_observed_v1(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: RenderObserverV1<'_, '_, '_>,
) -> Result<(BuiltIdentityV1, ProductionAnalysisResourceUpperBoundV1), BuildIdentityFailureV1> {
    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let preflight = match observer {
        None => preflight_identity_structure_v1(context, function, limits),
        Some(_) => {
            preflight_identity_structure_with_observation_v1(context, function, limits, observer)
        }
    }
    .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let callbacks = preflight.native_switch_verification_work;
    let frozen_text = identity_textual_preflight_resource_upper_bound_v1(preflight)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    require_identity_prefix_v1(limits, Ok(frozen_text), callbacks, observer)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let prescan = match observer {
        None => prescan(context, function),
        Some(_) => prescan_observed_v1(context, function, observer),
    }?;
    let closure_limits = limits
        .remaining_after_retained(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            frozen_text,
        )
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let project = |local| {
        carry_identity_callbacks_v1(frozen_text.checked_then_retain(local, phase)?, callbacks)
    };
    let order = match observer {
        None => def_use_closure_v1::check(context, function, &prescan, closure_limits),
        Some(parent) => parent.with_projection(&project, |nested| {
            def_use_closure_v1::check_with_observation_v1(
                context,
                function,
                &prescan,
                closure_limits,
                Some(nested),
            )
        }),
    }
    .map_err(|failure| match observer {
        None => def_use_closure_v1::identity_failure(context, &prescan, failure),
        Some(_) => def_use_closure_v1::identity_failure_with_observation_v1(
            context, &prescan, failure, observer,
        ),
    })?;
    let closure_bound = order.resource_upper_bound();
    let order_storage = closure_bound.retained_storage_upper_bound();
    let textual_preflight_bound = identity_bound_with_live_prefix_v1(frozen_text, order_storage)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    let textual_preflight_bound =
        dominate_identity_preflight_bound_v1(closure_bound, textual_preflight_bound)
            .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    require_identity_prefix_v1(limits, Ok(textual_preflight_bound), callbacks, observer)
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
        let root_name = render_operation_name_observed_v1(
            context,
            root,
            PlironPreserveLocationV1::Function,
            observer,
        )?;
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
            observer,
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
                observer,
            )?;
            for (argument_index, argument) in block_ref.arguments().enumerate() {
                let (type_id, ty) =
                    render_type(context, argument, block_location.clone(), observer)?;
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
                let name = render_operation_name_observed_v1(
                    context,
                    operation,
                    PlironPreserveLocationV1::Block { block: block_index },
                    observer,
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
                    let (type_id, ty) = render_type(context, result, location.clone(), observer)?;
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
                            let (type_id, ty) =
                                render_type(context, result, location.clone(), observer)?;
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
                encode_attributes(
                    context,
                    &raw.attributes,
                    location.clone(),
                    encoder,
                    observer,
                )?;
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
        semantic_definitions += usize::from(is_semantic_refinement_definition_v1(&*operation));
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
    // The order index stays live through counting and verification. Conservatively
    // cover its overlap with the bundled capture envelope, including later encoding.
    let resource_upper_bound =
        identity_bound_with_live_prefix_v1(resource_upper_bound, order_storage)
            .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    // Admit the cumulative preflight/capture work before native verification
    // or retained identity allocation, not after both phases have executed.
    let resource_upper_bound =
        dominate_identity_preflight_bound_v1(resource_upper_bound, textual_preflight_bound)
            .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    // Native callbacks are already part of this final capture envelope.
    require_identity_prefix_v1(limits, Ok(resource_upper_bound), 0, observer)
        .map_err(BuildIdentityFailureV1::ResourceLimit)?;
    #[cfg(test)]
    def_use_closure_v1::full_verification();
    let verification = catch_unwind(AssertUnwindSafe(move || {
        scoped_verification_v1::verify(context, function, order)
    }));
    #[cfg(test)]
    def_use_closure_v1::require_order_index_retired_for_tests();
    match verification {
        Err(_) => {
            if let Some(observer) = observer {
                observer.caught();
            }
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: "the PLIRON verifier panicked".to_owned(),
            }
            .into());
        }
        Ok(Err(error)) => {
            let detail = render_bounded_observed_v1(
                PlironPreserveLocationV1::Function,
                "structural verifier diagnostic",
                observer,
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

#[cfg(test)]
mod observed_capture_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use dialect_kernel::{IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, ReturnOp};

    const PHASE: ProductionAnalysisResourcePhaseV1 =
        ProductionAnalysisResourcePhaseV1::StructuralIdentity;

    fn fixture(external_user: bool) -> (Context, FuncOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "capture_observer".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let constant = IndexConstantOp::new(&mut context, 7);
        constant.get_operation().insert_at_back(entry, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        if external_user {
            let foreign = FuncOp::new(&mut context, "external_user".try_into().unwrap(), signature);
            let block = foreign.get_entry_block(&context);
            let value = constant.result(&context);
            IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Add, value, value)
                .get_operation()
                .insert_at_back(block, &context);
            ReturnOp::new(&mut context)
                .get_operation()
                .insert_at_back(block, &context);
        }
        (context, function)
    }

    #[test]
    fn observed_capture_matches_ordinary_with_live_floor_and_exact_limits() {
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        for short in [0, 1, 2] {
            let (context, function) = fixture(false);
            let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
            let Ok(floor_owner) = provider.capture_with_resource_limits_v1(hard) else {
                panic!("ordinary identity capture must succeed");
            };
            let bound = floor_owner.resource_upper_bound;
            let total = bound.checked_then_retain(bound, PHASE).unwrap();
            let limits = ProductionAnalysisResourceLimitsV1::new(
                total.work_upper_bound() - usize::from(short == 1),
                total.peak_storage_upper_bound() - usize::from(short == 2),
            );
            {
                let mut receipt = Receipt::new(bound, limits).unwrap();
                let phase = receipt.phase(PHASE, 0).unwrap();
                let result =
                    provider.capture_with_resource_observation_v1(hard, Some(&phase.observer(&Ok)));
                if short == 0 {
                    let Ok(observed) = result else {
                        panic!("exact observed capture must succeed");
                    };
                    assert_eq!(observed.resource_upper_bound, bound);
                    assert_eq!(observed.input_census, floor_owner.input_census);
                    assert!(
                        observed
                            .snapshot
                            .identity
                            .exactly_matches(&floor_owner.snapshot.identity)
                    );
                    phase.commit(bound).unwrap();
                    assert_eq!(receipt.complete(), Ok(bound));
                    let released = receipt
                        .drop_owner(PHASE, observed, bound.retained_storage_upper_bound())
                        .unwrap();
                    assert_eq!(released.retained_storage_upper_bound(), 0);
                    assert_eq!(released.work_upper_bound(), bound.work_upper_bound());
                    assert_eq!(
                        released.peak_storage_upper_bound(),
                        bound.peak_storage_upper_bound()
                    );
                } else {
                    let Err(IdentityCaptureFailureV1::ResourceLimit(error)) = result else {
                        panic!("one-short cumulative limit must refuse capture");
                    };
                    drop(phase);
                    assert_eq!(error.phase, PHASE);
                    assert_eq!(
                        error.resource,
                        if short == 1 {
                            "work upper bound"
                        } else {
                            "peak storage upper bound"
                        }
                    );
                    let state = receipt.snapshot();
                    assert!(state.committed.work_upper_bound() > 0);
                    assert_eq!(state.first_denial, Some(error));
                    assert!(!state.caught_panic);
                    limits
                        .require(
                            PHASE,
                            bound.checked_then_retain(state.committed, PHASE).unwrap(),
                        )
                        .unwrap();
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
                }
                assert_eq!(def_use_closure_v1::order_index_counts_for_tests().1, 0);
            }
            drop(floor_owner);
        }
    }

    #[test]
    fn observed_capture_semantic_failure_keeps_prefix_without_resource_denial() {
        let (context, function) = fixture(true);
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let preflight = preflight_identity_structure_v1(&context, &function, hard).unwrap();
        let text = identity_textual_preflight_resource_upper_bound_v1(preflight).unwrap();
        let mut receipt = Receipt::new(Default::default(), hard).unwrap();
        let phase = receipt.phase(PHASE, 0).unwrap();
        let result = LivePlironStructuralIdentityProviderV1::new(&context, &function)
            .capture_with_resource_observation_v1(hard, Some(&phase.observer(&Ok)));
        assert!(matches!(
            result,
            Err(IdentityCaptureFailureV1::Unavailable {
                source_code: "FE2O3-PRESERVE-000",
                ..
            })
        ));
        drop(phase);
        let state = receipt.snapshot();
        assert!(state.committed.work_upper_bound() > text.work_upper_bound());
        assert_eq!(state.first_denial, None);
        assert!(!state.caught_panic);
        assert_eq!(receipt.complete(), Ok(state.committed));
        assert_eq!(def_use_closure_v1::order_index_counts_for_tests().1, 0);
    }

    #[test]
    fn observed_capture_caught_traversal_panic_retains_the_header() {
        let (context, function) = fixture(false);
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let mut receipt = Receipt::new(Default::default(), hard).unwrap();
        let phase = receipt.phase(PHASE, 0).unwrap();
        let held = function.get_operation().deref_mut(&context);
        let result = LivePlironStructuralIdentityProviderV1::new(&context, &function)
            .capture_with_resource_observation_v1(hard, Some(&phase.observer(&Ok)));
        drop(held);
        assert!(matches!(
            result,
            Err(IdentityCaptureFailureV1::Unavailable {
                source_code: "FE2O3-PRESERVE-005",
                ..
            })
        ));
        drop(phase);
        let state = receipt.snapshot();
        assert_eq!(
            (
                state.committed.work_upper_bound(),
                state.committed.peak_storage_upper_bound()
            ),
            (1, 1)
        );
        assert_eq!(state.first_denial, None);
        assert!(state.caught_panic);
        assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
    }

    struct LegacyProvider<'a> {
        live: LivePlironStructuralIdentityProviderV1<'a>,
        captures: usize,
    }

    impl PlironStructuralIdentityProviderV1 for LegacyProvider<'_> {
        type Snapshot = BuiltIdentityV1;
        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            self.live.mutation_epoch()
        }
        fn capture_with_resource_limits_v1(
            &mut self,
            limits: ProductionAnalysisResourceLimitsV1,
        ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1>
        {
            self.captures += 1;
            self.live.capture_with_resource_limits_v1(limits)
        }
        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            self.live.label(snapshot)
        }
        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            self.live.require_exact_identity(expected, observed)
        }
        fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
            self.live.retain_exact_identity(snapshot)
        }
    }

    #[test]
    fn provider_without_observation_refuses_before_ordinary_capture() {
        let (context, function) = fixture(false);
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let mut provider = LegacyProvider {
            live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
            captures: 0,
        };
        let mut receipt = Receipt::new(Default::default(), hard).unwrap();
        let phase = receipt.phase(PHASE, 0).unwrap();
        assert!(matches!(
            provider.capture_with_resource_observation_v1(hard, Some(&phase.observer(&Ok))),
            Err(IdentityCaptureFailureV1::Unavailable {
                source_code: "FE2O3-PRESERVE-028",
                ..
            })
        ));
        assert_eq!(provider.captures, 0);
        drop(phase);
        assert_eq!(receipt.snapshot(), Default::default());
        assert!(
            provider
                .capture_with_resource_observation_v1(hard, None)
                .is_ok()
        );
        assert_eq!(provider.captures, 1);
    }
}
