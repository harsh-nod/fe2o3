fn semantic_attribute_count_v1(attributes: &AttributeDict) -> usize {
    attributes
        .0
        .values()
        .filter(|attribute| !is_debug_info_attribute_v1(attribute.as_ref()))
        .count()
}

fn prescan(context: &Context, function: &FuncOp) -> Result<PrescanV1, PlironIrIdentityErrorV1> {
    let root = function.get_operation();
    let root_ref = root.deref(context);
    let root_name = render_operation_name_v1(context, root, PlironPreserveLocationV1::Function)?;
    if root_name != "builtin.func" {
        return Err(PlironIrIdentityErrorV1::UnsupportedRoot {
            operation: root_name,
            detail: "identity construction requires builtin.func",
        });
    }
    if root_ref.num_regions() != 1
        || root_ref.get_num_results() != 0
        || root_ref.get_num_operands() != 0
        || root_ref.get_num_successors() != 0
    {
        return Err(PlironIrIdentityErrorV1::UnsupportedRoot {
            operation: root_name,
            detail: "builtin.func must have exactly one region and no SSA results, operands, or successors",
        });
    }
    check_limit(
        PlironPreserveLocationV1::Function,
        "attributes",
        root_ref.attributes.0.len(),
        MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
    )?;
    let mut type_nodes = validate_attribute_dict(
        context,
        &root_ref.attributes,
        PlironPreserveLocationV1::Function,
    )?;
    let function_type = function.get_type(context);
    type_nodes = type_nodes
        .checked_add(validate_and_count_type_handle_v1(
            context,
            function_type,
            PlironPreserveLocationV1::Function,
        )?)
        .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
    let function_type_ref = function_type.deref(context);
    if function_type_ref.downcast_ref::<FunctionType>().is_none() {
        return Err(PlironIrIdentityErrorV1::UnsupportedType {
            location: PlironPreserveLocationV1::Function,
            ty: render_type_id_v1(&*function_type_ref, PlironPreserveLocationV1::Function)?,
        });
    }
    drop(function_type_ref);

    let mut blocks = Vec::new();
    let mut operations = Vec::new();
    let mut operation_count = 0_usize;
    let mut values = 0_usize;
    let mut operands = 0_usize;
    let mut successors = 0_usize;
    let mut attributes = root_ref.attributes.0.len();
    let mut block_arguments = 0_usize;
    let mut max_operation_arity = 0_usize;
    let mut max_successor_arity = 0_usize;
    for block in function.get_region(context).deref(context).iter(context) {
        check_limit(
            PlironPreserveLocationV1::Function,
            "basic blocks",
            blocks
                .len()
                .checked_add(1)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?,
            MAX_PLIRON_IDENTITY_BLOCKS_V1,
        )?;
        let block_index = blocks.len();
        let block_ref = block.deref(context);
        let block_location = PlironPreserveLocationV1::Block { block: block_index };
        type_nodes = type_nodes
            .checked_add(validate_attribute_dict(
                context,
                &block_ref.attributes,
                block_location.clone(),
            )?)
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        for argument in block_ref.arguments() {
            type_nodes = type_nodes
                .checked_add(validate_and_count_type_handle_v1(
                    context,
                    argument.get_type(context),
                    block_location.clone(),
                )?)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        }
        block_arguments = block_arguments
            .checked_add(block_ref.get_num_arguments())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        values = values
            .checked_add(block_ref.get_num_arguments())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        attributes = attributes
            .checked_add(block_ref.attributes.0.len())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        check_limit(
            PlironPreserveLocationV1::Block { block: block_index },
            "SSA values",
            values,
            MAX_PLIRON_IDENTITY_VALUES_V1,
        )?;
        check_limit(
            PlironPreserveLocationV1::Block { block: block_index },
            "attributes",
            attributes,
            MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
        )?;
        let mut block_operations = Vec::new();
        for (operation_index, operation) in block_ref.iter(context).enumerate() {
            operation_count = operation_count
                .checked_add(1)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            let dynamic = Operation::get_op_dyn(operation, context);
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
            check_limit(
                location.clone(),
                "operations",
                operation_count,
                MAX_PLIRON_IDENTITY_OPERATIONS_V1,
            )?;
            if !is_production_ranked_operation_v1(dynamic.as_ref()) {
                return Err(PlironIrIdentityErrorV1::UnsupportedOperation {
                    location,
                    detail: "operation is outside the closed ranked operation allowlist",
                });
            }
            let raw = operation.deref(context);
            type_nodes = type_nodes
                .checked_add(validate_attribute_dict(
                    context,
                    &raw.attributes,
                    location.clone(),
                )?)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            for result in raw.results() {
                type_nodes = type_nodes
                    .checked_add(validate_and_count_type_handle_v1(
                        context,
                        result.get_type(context),
                        location.clone(),
                    )?)
                    .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            }
            if raw.num_regions() != 0 {
                return Err(PlironIrIdentityErrorV1::UnsupportedOperation {
                    location,
                    detail: "ranked body operations must not contain nested regions",
                });
            }
            values = values
                .checked_add(raw.get_num_results())
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            operands = operands
                .checked_add(raw.get_num_operands())
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            successors = successors
                .checked_add(raw.get_num_successors())
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            attributes = attributes
                .checked_add(raw.attributes.0.len())
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
            max_operation_arity = max_operation_arity.max(
                raw.get_num_results()
                    .checked_add(raw.get_num_operands())
                    .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?,
            );
            max_successor_arity = max_successor_arity.max(raw.get_num_successors());
            check_limit(
                location.clone(),
                "SSA values",
                values,
                MAX_PLIRON_IDENTITY_VALUES_V1,
            )?;
            check_limit(
                location.clone(),
                "operands",
                operands,
                MAX_PLIRON_IDENTITY_OPERANDS_V1,
            )?;
            check_limit(
                location.clone(),
                "CFG successors",
                successors,
                MAX_PLIRON_IDENTITY_SUCCESSORS_V1,
            )?;
            check_limit(
                location,
                "attributes",
                attributes,
                MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
            )?;
            block_operations.push(operation);
        }
        blocks.push(block);
        operations.push(block_operations);
    }
    if blocks.is_empty() {
        return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
            detail: "builtin.func has no basic blocks".to_owned(),
        });
    }
    Ok(PrescanV1 {
        blocks,
        operations,
        values,
        operands,
        successors,
        block_arguments,
        attributes,
        type_nodes,
        max_operation_arity,
        max_successor_arity,
    })
}

fn validate_attribute_dict(
    context: &Context,
    attributes: &AttributeDict,
    location: PlironPreserveLocationV1,
) -> Result<usize, PlironIrIdentityErrorV1> {
    let mut type_nodes = 0_usize;
    for (key, attribute) in &attributes.0 {
        let attribute_id = attribute.get_attr_id();
        if !is_production_attribute_id_parts_v1(
            attribute_id.dialect.as_ref(),
            attribute_id.name.as_ref(),
        ) {
            return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
                location: location.clone(),
                attribute: render_attribute_id_v1(attribute, location.clone())?,
            });
        }
        if ((attribute_id.dialect.as_ref() == "kernel"
            && AsRef::<str>::as_ref(&attribute_id.name) == "publication_atomic")
            || key.as_ref() == "kernel_publication_atomic")
            && (key.as_ref() != "kernel_publication_atomic"
                || attribute
                    .downcast_ref::<dialect_kernel::PublicationAtomicAccessAttr>()
                    .is_none()
                || !matches!(&location, PlironPreserveLocationV1::Operation { name, .. } if name == "kernel.access"))
        {
            return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
                location: location.clone(),
                attribute: render_attribute_id_v1(attribute, location.clone())?,
            });
        }
        if let Some(type_attribute) = attribute.downcast_ref::<TypeAttr>() {
            type_nodes = type_nodes
                .checked_add(validate_and_count_type_handle_v1(
                    context,
                    type_attribute.get_type(context),
                    location.clone(),
                )?)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        }
    }
    Ok(type_nodes)
}

fn encode_attributes(
    context: &Context,
    attributes: &AttributeDict,
    location: PlironPreserveLocationV1,
    encoder: &mut IdentityEncoderV1,
) -> Result<(), PlironIrIdentityErrorV1> {
    let mut sorted = attributes
        .0
        .iter()
        .filter(|(_, attribute)| !is_debug_info_attribute_v1(attribute.as_ref()))
        .collect::<Vec<_>>();
    sorted.sort_by(|lhs, rhs| lhs.0.cmp(rhs.0));
    for (key, _) in &sorted {
        check_limit(
            location.clone(),
            "attribute key bytes",
            key.as_ref().len(),
            MAX_IDENTIFIER_BYTES_V1,
        )?;
    }
    let mut summary = DiagnosticSummaryV1::default();
    if sorted.is_empty() {
        summary.append(format_args!("no attributes"));
    }
    for (index, (key, attribute)) in sorted.iter().enumerate() {
        let (attribute_id, value) = render_attribute(context, attribute, location.clone())?;
        if index != 0 {
            summary.append(format_args!(", "));
        }
        summary.append(format_args!("{key}={attribute_id} {value}"));
    }
    encoder.record(
        location.clone(),
        "attributes",
        summary.finish(),
        |encoder| {
            encoder.usize(sorted.len())?;
            for (key, attribute) in sorted {
                let (attribute_id, value) = render_attribute(context, attribute, location.clone())?;
                encoder.string(AsRef::<str>::as_ref(key).as_bytes())?;
                encoder.string(attribute_id.as_bytes())?;
                encoder.string(value.as_bytes())?;
            }
            Ok(())
        },
    )
}

fn is_debug_info_attribute_v1(attribute: &dyn pliron::attribute::Attribute) -> bool {
    let id = attribute.get_attr_id();
    id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
}

fn render_attribute(
    context: &Context,
    attribute: &AttrObj,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    let attribute_id = render_attribute_id_v1(attribute, location.clone())?;
    if !is_production_attribute_id(&attribute_id) {
        return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
            location,
            attribute: attribute_id,
        });
    }
    // New protocol markers have a closed encoding independent of their printer.
    // Native verification still checks the separately retained access attributes
    // and SSA result shape; this marker alone grants no publication authority.
    if attribute_id == "kernel.publication_atomic" {
        use dialect_kernel::PublicationAtomicAccessAttr;
        let value = match attribute.downcast_ref::<PublicationAtomicAccessAttr>() {
            Some(PublicationAtomicAccessAttr::ReleaseRequestU32) => {
                "u32:release:system:store=1:results=0"
            }
            Some(PublicationAtomicAccessAttr::ReleaseReadyU32) => {
                "u32:release:system:store=2:results=0"
            }
            Some(PublicationAtomicAccessAttr::AcquireU32) => {
                "u32:acquire:system:result=index-zext-u32"
            }
            None => {
                return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
                    location,
                    attribute: attribute_id,
                });
            }
        };
        return Ok((attribute_id, value.to_owned()));
    }
    let value = render_bounded(location, "attribute", |writer| {
        use pliron::builtin::attributes::{FPDoubleAttr, FPHalfAttr, FPSingleAttr};
        use pliron::utils::apfloat::Float;

        // APFloat's printable decimal/NaN text is not a bitwise identity.
        // These attributes were previously rejected; all old encodings stay
        // byte-for-byte unchanged.
        if let Some(value) = attribute.downcast_ref::<FPHalfAttr>() {
            return write!(writer, "ieee16:0x{:04x}", value.0.to_bits());
        }
        if let Some(value) = attribute.downcast_ref::<FPSingleAttr>() {
            return write!(writer, "ieee32:0x{:08x}", value.0.to_bits());
        }
        if let Some(value) = attribute.downcast_ref::<FPDoubleAttr>() {
            return write!(writer, "ieee64:0x{:016x}", value.0.to_bits());
        }
        write!(writer, "{}", attribute.disp(context))
    })?;
    Ok((attribute_id, value))
}

fn render_attribute_id_v1(
    attribute: &AttrObj,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_bounded(location, "attribute id", |writer| {
        write!(writer, "{}", attribute.get_attr_id())
    })
}

fn render_type(
    context: &Context,
    value: Value,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    render_type_handle(context, value.get_type(context), location)
}

fn render_type_handle(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    // Prescan validates the closed type tree before either encoding pass. Do
    // not repeat the owned FunctionType child-roster traversal here.
    let type_id = {
        let borrowed = ty.deref(context);
        render_type_id_v1(&*borrowed, location.clone())?
    };
    let value = render_bounded(location, "type", |writer| {
        write!(writer, "{}", ty.disp(context))
    })?;
    Ok((type_id, value))
}

fn render_operation_name_v1(
    context: &Context,
    operation: Ptr<Operation>,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_bounded(location, "operation name", |writer| {
        write!(
            writer,
            "{}",
            Operation::get_op_dyn(operation, context).get_opid()
        )
    })
}

fn render_type_id_v1(
    ty: &dyn Type,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_bounded(location, "type id", |writer| {
        write!(writer, "{}", ty.get_type_id())
    })
}

fn validate_and_count_type_handle_v1(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
) -> Result<usize, PlironIrIdentityErrorV1> {
    // FunctionTypeInterface exposes owned child rosters only. Render through
    // the byte/depth-guarded writer before requesting either roster. The builtin
    // FunctionType printer emits at least one byte for every descendant type,
    // so the successful byte count is an executable bound on both total nodes
    // and the sum of child handles simultaneously retained by recursive frames.
    let rendered_bytes = render_type_preflight_bounded_v2(location.clone(), |writer| {
        write!(writer, "{}", ty.disp(context))
    })?
    .len()
    .max(1);
    let mut remaining_nodes = rendered_bytes;
    validate_and_count_type_handle_at_depth_v1(context, ty, location, 0, &mut remaining_nodes)
}

fn validate_and_count_type_handle_at_depth_v1(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
    depth: usize,
    remaining_nodes: &mut usize,
) -> Result<usize, PlironIrIdentityErrorV1> {
    check_limit(
        location.clone(),
        "type nesting depth",
        depth,
        MAX_PLIRON_IDENTITY_TYPE_NESTING_V1,
    )?;
    *remaining_nodes = remaining_nodes.checked_sub(1).ok_or_else(|| {
        PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location: location.clone(),
            resource: "type nodes bounded by rendered bytes",
            actual: usize::MAX,
            limit: 0,
        }
    })?;
    let borrowed = ty.deref(context);
    if !is_production_type(&*borrowed) {
        return Err(PlironIrIdentityErrorV1::UnsupportedType {
            location: location.clone(),
            ty: render_type_id_v1(&*borrowed, location)?,
        });
    }
    let data_child = if let Some(pointer) =
        borrowed.downcast_ref::<dialect_gpu::optimization_v1::PointerType>()
    {
        Some(pointer.pointee())
    } else if let Some(slice) = borrowed.downcast_ref::<dialect_gpu::optimization_v1::SliceType>() {
        Some(slice.element())
    } else if let Some(vector) =
        borrowed.downcast_ref::<dialect_gpu::vector_v12::FixedVectorTypeV12>()
    {
        if pliron::common_traits::Verify::verify(vector, context).is_err() {
            return Err(PlironIrIdentityErrorV1::UnsupportedType {
                location: location.clone(),
                ty: render_type_id_v1(&*borrowed, location)?,
            });
        }
        Some(vector.element())
    } else {
        None
    };
    if let Some(child) = data_child {
        drop(borrowed);
        let child_type = child.deref(context);
        if !crate::kir_bridge_v1::ranked_data_type_node_is_supported_v2(&*child_type) {
            return Err(PlironIrIdentityErrorV1::UnsupportedType {
                location: location.clone(),
                ty: render_type_id_v1(&*child_type, location)?,
            });
        }
        drop(child_type);
        let children = validate_and_count_type_handle_at_depth_v1(
            context,
            child,
            location,
            depth + 1,
            remaining_nodes,
        )?;
        return children
            .checked_add(1)
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX));
    }
    let nested = if let Some(function) = borrowed.downcast_ref::<FunctionType>() {
        let arguments = function.arg_types();
        let results = function.res_types();
        let child_count = arguments
            .len()
            .checked_add(results.len())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        check_limit(
            location.clone(),
            "function signature types",
            child_count,
            MAX_PLIRON_IDENTITY_VALUES_V1,
        )?;
        if child_count > *remaining_nodes {
            return Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                location: location.clone(),
                resource: "type nodes bounded by rendered bytes",
                actual: child_count,
                limit: *remaining_nodes,
            });
        }
        (arguments, results)
    } else {
        (Vec::new(), Vec::new())
    };
    drop(borrowed);
    nested
        .0
        .into_iter()
        .chain(nested.1)
        .try_fold(1_usize, |total, nested_type| {
            total
                .checked_add(validate_and_count_type_handle_at_depth_v1(
                    context,
                    nested_type,
                    location.clone(),
                    depth + 1,
                    remaining_nodes,
                )?)
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))
        })
}

fn is_production_type(ty: &dyn Type) -> bool {
    ty.downcast_ref::<FunctionType>().is_some()
        || ty.downcast_ref::<IntegerType>().is_some()
        || ty.downcast_ref::<UnitType>().is_some()
        || ty.downcast_ref::<FP16Type>().is_some()
        || ty.downcast_ref::<FP32Type>().is_some()
        || ty.downcast_ref::<FP64Type>().is_some()
        || ty.downcast_ref::<IndexType>().is_some()
        || ty
            .downcast_ref::<dialect_gpu::optimization_v1::IndexType>()
            .is_some()
        || ty
            .downcast_ref::<dialect_gpu::optimization_v1::BFloat16Type>()
            .is_some()
        || ty
            .downcast_ref::<dialect_gpu::optimization_v1::PointerType>()
            .is_some()
        || ty
            .downcast_ref::<dialect_gpu::optimization_v1::SliceType>()
            .is_some()
        || ty
            .downcast_ref::<dialect_gpu::vector_v12::FixedVectorTypeV12>()
            .is_some()
        || ty.downcast_ref::<PipelineType>().is_some()
        || ty.downcast_ref::<RankedViewType>().is_some()
        || ty.downcast_ref::<SemanticScalarType>().is_some()
        || is_checked_access_capability_type(ty)
        || ty.downcast_ref::<ObligationRefType>().is_some()
        || ty.downcast_ref::<EvidenceRefType>().is_some()
}

fn is_production_attribute_id(attribute: &str) -> bool {
    attribute
        .split_once('.')
        .is_some_and(|(dialect, name)| is_production_attribute_id_parts_v1(dialect, name))
}

fn is_production_attribute_id_parts_v1(dialect: &str, name: &str) -> bool {
    match dialect {
        "builtin" => matches!(
            name,
            "identifier"
                | "debug_info"
                | "string"
                | "bool"
                | "integer"
                | "half"
                | "single"
                | "double"
                | "unit"
                | "type"
                | "operand_segment_sizes"
        ),
        "gpu" => matches!(
            name,
            "address_space"
                | "execution_domain"
                | "execution_extent"
                | "grid_identity"
                | "hierarchy"
                | "memory_order"
                | "memory_scope"
                | "subgroup_size"
                | "access_mode"
                | "unary_kind"
                | "binary_kind"
                | "compare_predicate"
                | "cast_kind"
                | "index_value"
                | "bf16_value"
        ),
        "kernel" => matches!(
            name,
            "access_kind"
                | "allocation_origin"
                | "analysis_split_control_count"
                | "atomic_ordering"
                | "atomic_scope"
                | "publication_atomic"
                | "dimension"
                | "index_binary_kind"
                | "index_value"
                | "invocation_dimension"
                | "launch_extent"
                | "memory_space"
                | "noalias_class"
                | "ownership_coverage"
                | "ownership_partition"
                | "pipeline_event_kind"
                | "semantic_binary_kind"
                | "semantic_cast_kind"
                | "semantic_compare_kind"
                | "semantic_constant"
                | "semantic_coverage_binding"
                | "semantic_domain_bound"
                | "semantic_evaluation_order"
                | "semantic_exceptional_value"
                | "semantic_expression_commitment"
                | "semantic_ieee_rounding"
                | "semantic_numerical_policy"
                | "semantic_overflow"
                | "semantic_scalar_kind"
                | "semantic_step_bound"
                | "semantic_symbol"
                | "semantic_typed_binary_kind"
                | "semantic_unary_kind"
                | "tensor_convergence"
                | "tensor_fragment"
                | "tensor_instruction"
                | "tensor_value_root"
        ),
        "proof" => matches!(
            name,
            "absolute_error_f64_bits"
                | "covered_boundary"
                | "evidence_status"
                | "id"
                | "property"
                | "relative_error_f64_bits"
        ),
        _ => false,
    }
}
