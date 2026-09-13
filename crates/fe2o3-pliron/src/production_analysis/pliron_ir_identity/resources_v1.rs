fn identity_capture_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    record_count: usize,
    record_summary_bytes: usize,
    record_location_name_bytes: usize,
) -> Result<
    ProductionAnalysisResourceUpperBoundV1,
    crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
> {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;

    const CAPTURE_GRAPH_TRAVERSALS_V1: usize = 7;
    const CAPTURE_TEXT_TRAVERSALS_V1: usize = 4;
    const CAPTURE_CANONICAL_BYTE_TRAVERSALS_V1: usize = 2;
    const CAPTURE_RECORD_TRAVERSALS_V1: usize = 2;
    const CAPTURE_SUMMARY_TEXT_TRAVERSALS_V1: usize = 4;
    const CAPTURE_LOCATION_TEXT_TRAVERSALS_V1: usize = 3;

    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let overflow = |resource| ProductionAnalysisResourceLimitV1 { phase, resource };
    let graph_items = [
        census.blocks,
        census.operations,
        census.operands,
        census.results,
        census.successors,
        census.block_arguments,
        census.attributes,
        census.type_nodes,
    ]
    .into_iter()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value)
            .ok_or_else(|| overflow("identity graph item upper bound"))
    })?;
    let attribute_sort_height = usize::BITS as usize - census.attributes.leading_zeros() as usize;
    let work_upper_bound = graph_items
        .checked_mul(CAPTURE_GRAPH_TRAVERSALS_V1)
        .and_then(|work| {
            census
                .identifier_bytes
                .checked_mul(CAPTURE_TEXT_TRAVERSALS_V1)
                .and_then(|text| work.checked_add(text))
        })
        .and_then(|work| {
            census
                .canonical_bytes
                .checked_mul(CAPTURE_CANONICAL_BYTE_TRAVERSALS_V1)
                .and_then(|bytes| work.checked_add(bytes))
        })
        .and_then(|work| {
            record_count
                .checked_mul(CAPTURE_RECORD_TRAVERSALS_V1)
                .and_then(|records| work.checked_add(records))
        })
        .and_then(|work| {
            census
                .identifier_bytes
                .checked_mul(attribute_sort_height)
                .and_then(|comparisons| work.checked_add(comparisons))
        })
        .and_then(|work| {
            record_summary_bytes
                .checked_mul(CAPTURE_SUMMARY_TEXT_TRAVERSALS_V1)
                .and_then(|summaries| work.checked_add(summaries))
        })
        .and_then(|work| {
            record_location_name_bytes
                .checked_mul(CAPTURE_LOCATION_TEXT_TRAVERSALS_V1)
                .and_then(|locations| work.checked_add(locations))
        })
        .and_then(|work| work.checked_add(1))
        .and_then(|work| work.checked_add(census.native_switch_verification_work))
        .ok_or_else(|| overflow("identity capture work upper bound"))?;
    let retained_storage_upper_bound = census
        .canonical_bytes
        .checked_add(record_count)
        .and_then(|storage| storage.checked_add(record_summary_bytes))
        .and_then(|storage| storage.checked_add(record_location_name_bytes))
        // Pass cardinalities, semantic subsets and the two native callback
        // subtotals remain retained with the structural census.
        .and_then(|storage| storage.checked_add(9))
        .ok_or_else(|| overflow("identity retained storage upper bound"))?;
    let temporary_storage_upper_bound = census
        .blocks
        .checked_mul(3)
        .and_then(|storage| {
            census
                .operations
                .checked_mul(3)
                .and_then(|v| storage.checked_add(v))
        })
        .and_then(|storage| {
            census
                .results
                .checked_add(census.block_arguments)
                .and_then(|v| storage.checked_add(v))
        })
        .and_then(|storage| storage.checked_add(census.identifier_bytes))
        .and_then(|storage| storage.checked_add(record_summary_bytes))
        .and_then(|storage| storage.checked_add(census.native_switch_verification_scratch))
        .ok_or_else(|| overflow("identity temporary storage upper bound"))?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        work_upper_bound,
        retained_storage_upper_bound,
        temporary_storage_upper_bound,
    )
}

fn identity_textual_preflight_resource_upper_bound_v1(
    census: IdentityPreflightCensusV1,
) -> Result<
    ProductionAnalysisResourceUpperBoundV1,
    crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
> {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;

    const SIMULTANEOUS_RENDERED_ENTITIES_V1: usize = 4;
    // PLIRON's FunctionTypeInterface returns owned child rosters. A successful
    // bounded rendering proves that the complete descendant roster contains no
    // more items than rendered bytes; one additional entity-sized workspace
    // therefore covers every simultaneously live ancestor roster.
    const TYPE_TRAVERSAL_STORAGE_V1: usize = MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1;
    const TYPE_TRAVERSAL_WORK_PER_RENDERED_BYTE_V1: usize = 4;
    const MAX_DIAGNOSTIC_SUMMARY_BYTES_V1: usize = MAX_DIAGNOSTIC_DETAIL_CHARS_V1 * 4 + 3;

    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let overflow = |resource| ProductionAnalysisResourceLimitV1 { phase, resource };
    let work_upper_bound = census
        .rendered_entities
        .checked_mul(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1)
        .and_then(|work| {
            census
                .records
                .checked_mul(MAX_DIAGNOSTIC_SUMMARY_BYTES_V1)
                .and_then(|summaries| summaries.checked_mul(4))
                .and_then(|summaries| work.checked_add(summaries))
        })
        .and_then(|work| {
            census
                .type_roots
                .checked_mul(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1)
                .and_then(|nodes| nodes.checked_mul(TYPE_TRAVERSAL_WORK_PER_RENDERED_BYTE_V1))
                .and_then(|type_work| work.checked_add(type_work))
        })
        .and_then(|work| work.checked_add(census.structural_work))
        .ok_or_else(|| overflow("identity textual preflight work upper bound"))?;
    let temporary_storage_upper_bound = MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1
        .checked_mul(SIMULTANEOUS_RENDERED_ENTITIES_V1)
        .and_then(|storage| storage.checked_add(MAX_DIAGNOSTIC_SUMMARY_BYTES_V1))
        .and_then(|storage| storage.checked_add(TYPE_TRAVERSAL_STORAGE_V1))
        .and_then(|storage| storage.checked_add(census.structural_storage))
        .and_then(|storage| storage.checked_add(census.max_semantic_attributes_per_dictionary))
        .ok_or_else(|| overflow("identity textual preflight storage upper bound"))?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        work_upper_bound,
        0,
        temporary_storage_upper_bound,
    )
}

fn identity_bound_with_live_prefix_v1(
    bound: ProductionAnalysisResourceUpperBoundV1,
    prefix: usize,
) -> Result<
    ProductionAnalysisResourceUpperBoundV1,
    crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
> {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;
    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let peak = bound.peak_storage_upper_bound().checked_add(prefix).ok_or(
        ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity carried storage upper bound",
        },
    )?;
    // External overlap is temporary here, not retained identity output.
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        bound.work_upper_bound(),
        bound.retained_storage_upper_bound(),
        peak - bound.retained_storage_upper_bound(),
    )
}

fn dominate_identity_preflight_bound_v1(
    capture: ProductionAnalysisResourceUpperBoundV1,
    preflight: ProductionAnalysisResourceUpperBoundV1,
) -> Result<
    ProductionAnalysisResourceUpperBoundV1,
    crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
> {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;

    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let work = capture
        .work_upper_bound()
        .checked_add(preflight.work_upper_bound())
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity preflight and capture work upper bound",
        })?;
    let retained = capture.retained_storage_upper_bound();
    // Textual preflight temporaries are retired before verification, canonical
    // encoding, and retained identity construction begin.
    let peak = capture
        .peak_storage_upper_bound()
        .max(preflight.peak_storage_upper_bound());
    let temporary = peak
        .checked_sub(retained)
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity preflight and capture storage upper bound",
        })?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)
}

fn preflight_identity_structure_v1(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<
    IdentityPreflightCensusV1,
    crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
> {
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;

    let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
    let mut work = 1_usize;
    let mut storage = 1_usize;
    let require = |work, storage| {
        let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, 0, storage)?;
        limits.require(phase, bound).map(|_| ())
    };
    require(work, storage)?;
    let root = function.get_operation().deref(context);
    let root_attributes = root.attributes.0.len();
    work = work
        .checked_add(root_attributes)
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity preflight work upper bound",
        })?;
    storage = storage
        .checked_add(root_attributes)
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity preflight storage upper bound",
        })?;
    require(work, storage)?;
    let root_semantic_attributes = semantic_attribute_count_v1(&root.attributes);
    let mut rendered_entities = root_semantic_attributes
        .checked_mul(9)
        .and_then(|count| count.checked_add(5))
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "identity textual render count",
        })?;
    let mut type_roots =
        root_attributes
            .checked_add(1)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity type-root count",
            })?;
    let mut records = 3_usize;
    let mut max_semantic_attributes_per_dictionary = root_semantic_attributes;
    let mut native_switch_verification_work = 0_usize;
    let mut native_switch_verification_scratch = 0_usize;
    for block in function.get_region(context).deref(context).iter(context) {
        let block = block.deref(context);
        work = work
            .checked_add(1)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight work upper bound",
            })?;
        storage = storage
            .checked_add(2)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight storage upper bound",
            })?;
        require(work, storage)?;
        let block_items = block
            .get_num_arguments()
            .checked_add(block.attributes.0.len())
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight work upper bound",
            })?;
        work = work
            .checked_add(block_items)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight work upper bound",
            })?;
        storage = storage
            .checked_add(block_items)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight storage upper bound",
            })?;
        require(work, storage)?;
        let block_semantic_attributes = semantic_attribute_count_v1(&block.attributes);
        max_semantic_attributes_per_dictionary =
            max_semantic_attributes_per_dictionary.max(block_semantic_attributes);
        rendered_entities = rendered_entities
            .checked_add(
                block_semantic_attributes
                    .checked_mul(9)
                    .and_then(|count| {
                        block
                            .get_num_arguments()
                            .checked_mul(8)
                            .and_then(|arguments| count.checked_add(arguments))
                    })
                    .ok_or(ProductionAnalysisResourceLimitV1 {
                        phase,
                        resource: "identity textual render count",
                    })?,
            )
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity textual render count",
            })?;
        records = records
            .checked_add(block.get_num_arguments())
            .and_then(|count| count.checked_add(2))
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity diagnostic record count",
            })?;
        type_roots = type_roots
            .checked_add(block.attributes.0.len())
            .and_then(|count| count.checked_add(block.get_num_arguments()))
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity type-root count",
            })?;
        for operation in block.iter(context) {
            let pointer = operation;
            let operation = operation.deref(context);
            let operation_items = operation
                .get_num_results()
                .checked_add(operation.get_num_operands())
                .and_then(|value| value.checked_add(operation.get_num_successors()))
                .and_then(|value| value.checked_add(operation.attributes.0.len()))
                .and_then(|value| value.checked_add(1))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity preflight work upper bound",
                })?;
            work = work
                .checked_add(operation_items)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity preflight work upper bound",
                })?;
            storage = storage
                .checked_add(operation_items)
                .and_then(|value| value.checked_add(3))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity preflight storage upper bound",
                })?;
            require(work, storage)?;
            if let Some(switch) =
                Operation::get_op::<dialect_gpu::switch_v3::SwitchOpV3>(pointer, context)
            {
                let prefix =
                    ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, storage, 0)?;
                let available = limits.remaining_after_retained(phase, prefix)?;
                let native = crate::production_analysis::pliron_switch_verification_v1::census_switch_verification_v1(
                    context, switch, available,
                )?;
                work = work.checked_add(native.traversal_work).ok_or(
                    ProductionAnalysisResourceLimitV1 {
                        phase,
                        resource: "identity native-switch census work upper bound",
                    },
                )?;
                native_switch_verification_work = native_switch_verification_work
                    .checked_add(native.callback_work)
                    .ok_or(ProductionAnalysisResourceLimitV1 {
                        phase,
                        resource: "identity native-switch verification work upper bound",
                    })?;
                native_switch_verification_scratch =
                    native_switch_verification_scratch.max(native.callback_scratch);
            }
            let operation_semantic_attributes = semantic_attribute_count_v1(&operation.attributes);
            max_semantic_attributes_per_dictionary =
                max_semantic_attributes_per_dictionary.max(operation_semantic_attributes);
            rendered_entities = rendered_entities
                .checked_add(
                    operation_semantic_attributes
                        .checked_mul(9)
                        .and_then(|count| {
                            operation
                                .get_num_results()
                                .checked_mul(8)
                                .and_then(|results| count.checked_add(results))
                        })
                        .and_then(|count| count.checked_add(3))
                        .ok_or(ProductionAnalysisResourceLimitV1 {
                            phase,
                            resource: "identity textual render count",
                        })?,
                )
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity textual render count",
                })?;
            records = records
                .checked_add(5)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity diagnostic record count",
                })?;
            type_roots = type_roots
                .checked_add(operation.attributes.0.len())
                .and_then(|count| count.checked_add(operation.get_num_results()))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "identity type-root count",
                })?;
        }
    }
    Ok(IdentityPreflightCensusV1 {
        structural_work: work,
        structural_storage: storage,
        rendered_entities,
        type_roots,
        records,
        max_semantic_attributes_per_dictionary,
        native_switch_verification_work,
        native_switch_verification_scratch,
    })
}
