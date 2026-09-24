// Narrow helper ABI and per-occurrence ISA joins. No root marker rule changes.
// Exact retained-source construction and custody remain mandatory. This borrowed
// check does not grant custody to a replaced graph or equate distinct equal-valued
// SSA IDs; explicit owner replay independently detects such substitutions.
use fe2o3_kernel_ir::{AssemblyOption, AssemblySourceIdentity};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendReprV1, SemanticBackendScalarV1, SemanticFieldsShapeV1, SemanticFunctionAbiV1,
    SemanticGfx942InlineU32V30, SemanticRustTypeKindV1, SemanticRustcVariantsV1,
    SemanticTypeLayoutDetailsV1,
};

fn singleton_u32_return_type_v30(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, Error> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or("native helper tuple result type outside owner")?;
    let SemanticTypeShapeV1::Tuple(fields) = declaration.shape() else {
        return Err("native helper non-scalar return is not an exact singleton tuple");
    };
    let [component] = fields.fields() else {
        return Err("native helper tuple return requires exactly one field");
    };
    let field = types
        .get(component.index() as usize)
        .ok_or("native helper tuple field outside owner")?;
    if declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || field.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || !matches!(
            field.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
    {
        return Err("native helper singleton return field is not ordinary u32");
    }
    let layout = declaration.layout();
    let scalar = field.layout();
    let SemanticTypeLayoutDetailsV1::Aggregate(details) = layout.details() else {
        return Err("native helper singleton return lacks exact aggregate layout");
    };
    if layout.rustc_size_bytes() != 4
        || scalar.rustc_size_bytes() != 4
        || layout.unadjusted_abi_alignment_bytes() != 4
        || scalar.unadjusted_abi_alignment_bytes() != 4
        || !matches!(
            scalar.variants(),
            SemanticRustcVariantsV1::Single { index: 0 }
        )
        || !matches!(scalar.fields(), SemanticFieldsShapeV1::Primitive)
        || !matches!(scalar.details(), SemanticTypeLayoutDetailsV1::None)
        || layout.size_bytes() != Some(4)
        || layout.alignment_bytes() != 4
        || scalar.size_bytes() != Some(4)
        || scalar.alignment_bytes() != 4
        || layout.is_uninhabited()
        || scalar.is_uninhabited()
        || layout.largest_niche().is_some()
        || scalar.largest_niche().is_some()
        || layout.fields().source_order_offsets_bytes() != Some(&[0][..])
        || layout.fields().memory_order_source_indices() != Some(&[0][..])
        || !matches!(
            layout.variants(),
            SemanticRustcVariantsV1::Single { index: 0 }
        )
        || details.field_offsets() != [0]
        || !details.padding().is_empty()
        || !matches!(
            scalar.backend_repr(),
            SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized { .. })
        )
        || layout.backend_repr() != scalar.backend_repr()
    {
        return Err("native helper singleton return layout differs from one u32");
    }
    Ok(Type::Scalar(ScalarType::U32))
}

fn native_helper_return_type_v30(
    semantic: &AdmittedInertSemanticMirV1,
    ty: SemanticTypeIdV1,
    meter: &mut dyn Meter,
) -> Result<Type, Error> {
    meter.work(32)?;
    match scalar_type(semantic, ty) {
        Ok(scalar) => Ok(scalar),
        Err(_) => singleton_u32_return_type_v30(semantic.types(), ty),
    }
}

fn ordinary_inline_u32_type_v30(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|declaration| {
        declaration.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
            && matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
    })
}

fn source_inline_call_v30<'a>(
    semantic: &'a AdmittedInertSemanticMirV1,
    block: &'a fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
) -> Option<(
    &'a SemanticDirectCallV1,
    SemanticGfx942InlineU32V30,
    &'a SemanticFunctionAbiV1,
)> {
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return None;
    };
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(instruction),
        binding,
        ..
    } = semantic.callables().get(call.callee().index() as usize)?
    else {
        return None;
    };
    Some((call, *instruction, binding.abi()))
}

fn native_helper_inline_source_v30(
    semantic: &AdmittedInertSemanticMirV1,
    correspondence: &SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    source_index: usize,
    function: &Function,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    meter.work(8)?;
    let source = semantic
        .functions()
        .get(source_index)
        .ok_or("native helper inline source is outside owner")?;
    let source_id = SemanticFunctionIdV1::from_index(
        u32::try_from(source_index).map_err(|_| "native helper source index overflow")?,
    );
    let body = function
        .body
        .as_ref()
        .ok_or("native helper inline body is absent")?;
    let mut actual_count = 0usize;
    for block in &body.blocks {
        meter.work(1)?;
        for operation in &block.operations {
            meter.work(1)?;
            if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                actual_count = actual_count
                    .checked_add(1)
                    .ok_or("native helper inline count overflow")?;
            }
        }
    }
    let mut source_count = 0usize;
    for (block_index, block) in source.blocks().iter().enumerate() {
        meter.work(3)?;
        let Some((call, instruction, abi)) = source_inline_call_v30(semantic, block) else {
            continue;
        };
        source_count = source_count
            .checked_add(1)
            .ok_or("native helper inline count overflow")?;
        meter.work(224)?;
        let occurrence = call
            .inline_assembly_source_v30()
            .ok_or("native helper inline occurrence missing")?;
        let identity = AssemblySourceIdentity::new(
            occurrence.frontend_unit(),
            *occurrence.function().as_bytes(),
            occurrence.contract(),
            occurrence.statement(),
        );
        let destination = call
            .destination()
            .ok_or("native helper inline destination missing")?;
        let ty = destination.place().ty();
        if source.role() != SemanticFunctionRoleV1::InternalHelper
            || identity.function != *source.identity().as_bytes()
            || !identity.is_complete()
            || instruction.option_bits() != SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
            || abi.can_unwind()
            || abi.c_variadic()
            || abi.canon_abi() != SemanticCanonAbiV1::Rust
            || abi.extern_abi() != SemanticExternAbiV1::Rust
            || !abi.hidden_arguments().is_empty()
            || abi.fixed_count() as usize != instruction.input_count()
            || abi.source_input_types().len() != instruction.input_count()
            || abi.arguments().len() != instruction.input_count()
            || abi.source_argument_ownership().len() != instruction.input_count()
            || abi.source_output_type() != ty
            || !ordinary_inline_u32_type_v30(semantic.types(), ty)
            || call.arguments().len() != instruction.input_count()
            || !call.variadic_argument_abis().is_empty()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
            || !destination.place().projections().is_empty()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err("native helper inline source contract mismatch");
        }
        let returned = abi.return_value();
        if returned.source_ty() != ty
            || returned.adjusted().is_some()
            || returned.pointee_override().is_some()
            || !matches!(returned.mode(), SemanticAbiPassModeV1::Direct(_))
        {
            return Err("native helper inline result ABI mismatch");
        }
        for (((argument, declared), ownership), value) in abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .zip(abi.source_argument_ownership())
            .zip(call.arguments())
        {
            meter.work(12)?;
            if *declared != ty
                || value.ty() != ty
                || !argument.is_source()
                || argument.ty() != ty
                || argument.value().adjusted().is_some()
                || argument.value().pointee_override().is_some()
                || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
                || *ownership != SemanticSourceArgumentOwnershipV1::ByValue
            {
                return Err("native helper inline input ABI mismatch");
            }
        }
        // A duplicated source occurrence must not justify two overlapping spans.
        for earlier in &source.blocks()[..block_index] {
            meter.work(36)?;
            if source_inline_call_v30(semantic, earlier).is_some_and(|(earlier, _, _)| {
                earlier
                    .inline_assembly_source_v30()
                    .is_some_and(|earlier| earlier.statement() == identity.statement)
            }) {
                return Err("native helper inline source occurrence reused");
            }
        }
        let mut matched_span = None;
        for span in correspondence.terminator_operation_spans() {
            meter.work(4)?;
            if span.correspondence_owner() == root
                && span.semantic_function() == source_id
                && span.semantic_block().index() as usize == block_index
                && matched_span.replace(span).is_some()
            {
                return Err("native helper inline source span duplicated");
            }
        }
        let span = matched_span.ok_or("native helper inline source span missing")?;
        let mut native_block = None;
        for candidate in &body.blocks {
            meter.work(1)?;
            if candidate.id == span.kernel_ir_block() && native_block.replace(candidate).is_some() {
                return Err("native helper inline block identity duplicated");
            }
        }
        let native_block = native_block.ok_or("native helper inline span block missing")?;
        let start = usize::try_from(span.first_operation_ordinal())
            .map_err(|_| "native helper inline span overflow")?;
        let end = start
            .checked_add(span.operation_count() as usize)
            .ok_or("native helper inline span overflow")?;
        let operations = native_block
            .operations
            .get(start..end)
            .ok_or("native helper inline source span outside body")?;
        let mut matched = false;
        for operation in operations {
            meter.work(1)?;
            let OperationKind::InlineAssembly(assembly) = &operation.kind else {
                continue;
            };
            meter.work(
                assembly
                    .mnemonic
                    .len()
                    .checked_add(160)
                    .ok_or("native helper inline identity work overflow")?,
            )?;
            if std::mem::replace(&mut matched, true)
                || assembly.source != identity
                || assembly.mnemonic != instruction.instruction().mnemonic()
                || assembly.options.len() != 1
                || !assembly.options.contains(&AssemblyOption::NoMemory)
                || !assembly.declared_effects.is_empty()
                || assembly.operands.len() != instruction.input_count() + 1
            {
                return Err("native helper inline occurrence or closed instruction substituted");
            }
        }
        if !matched {
            return Err("native helper inline source call has no actual instruction");
        }
    }
    if actual_count != source_count {
        return Err("native helper inline source/native occurrence census differs");
    }
    Ok(())
}
