// Included by production_complete_body_source_vnext.rs in the sole importer.
// Exact MIR36 source grammar; no older-profile source is relabeled.
const COMPLETE_BODY_SOURCE_BLOCK_LIMIT: usize = 4096;
const COMPLETE_BODY_SOURCE_LOCAL_LIMIT: usize = 4096;
const COMPLETE_BODY_SOURCE_ITEM_LIMIT: usize = 65_536;

struct CompleteBodyContextVNext<'a> {
    function: &'a SemanticFunctionDeclV1,
    symbol: &'a str,
    input: crate::CompleteBodyCanonicalInputVNext<'a>,
    marker: SemanticBlockIdV1,
    root: SemanticFunctionIdV1,
    argument_locals: [usize; 5],
}

fn complete_body_refusal(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}
fn complete_body_symbol_vnext(symbol: &str) -> bool {
    (1..=128).contains(&symbol.len())
        && symbol
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && symbol
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn complete_body_context_vnext<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CompleteBodyContextVNext<'a>, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCanonAbiV1, SemanticExternAbiV1, SemanticLocalRoleV1, SemanticMirWireVersionV1,
    };
    budget.charge_work(16)?;
    let semantic = owner.source_semantic();
    if semantic.wire_version() != SemanticMirWireVersionV1::V36
        || semantic.target().architecture()
            != fe2o3_mir_model::semantic_mir_v1::SemanticTargetArchitectureV1::AmdGpuGfx942
        || semantic.functions().len() != 1
        || semantic.roots().len() != 1
        || semantic.callables().len() != 2
        || launch.roots().len() != 1
        || !semantic.allocations().is_empty()
        || !semantic.statics().is_empty()
        || !semantic.vtables().is_empty()
        || launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
    {
        return Err(complete_body_refusal(
            "complete-body exact source profile/roster differs",
        ));
    }
    let root = semantic.roots()[0];
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| complete_body_refusal("complete-body root missing"))?;
    let launch_root = launch.roots()[0];
    let geometry = launch_root.layout();
    if geometry.global_extents()[0] == dialect_kernel::DYNAMIC_EXTENT {
        return Err(complete_body_refusal(
            "complete-body source requires an explicit finite max_grid",
        ));
    }
    let entry = function
        .kernel_entry()
        .ok_or_else(|| complete_body_refusal("complete-body source export missing"))?;
    let launch_contract = entry
        .source_contract()
        .launch()
        .ok_or_else(|| complete_body_refusal("complete-body source launch missing"))?;
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || root.index() != 0
        || launch_root.selected_root() != root
        || launch_root.semantic_root_identity() != function.identity()
        || launch_root.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
        || launch_root.source_rank() != 1
        || geometry.workgroup_extents() != [64, 1, 1]
        || geometry.subgroup_size() != 64
        || !geometry.full_physical_workgroups()
        || launch_contract.required().map(|v| v.as_array()) != Some([64, 1, 1])
        || launch_contract.maximum().map(|v| v.as_array()) != Some([64, 1, 1])
    {
        return Err(complete_body_refusal(
            "complete-body current root/launch identity differs",
        ));
    }
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| complete_body_refusal("complete-body symbol is not UTF-8"))?;
    budget.charge_work(symbol.len())?;
    if !complete_body_symbol_vnext(symbol) {
        return Err(complete_body_refusal(
            "complete-body symbol must be unchanged ASCII identifier <=128 bytes",
        ));
    }
    let abi = function.abi();
    if abi.extern_abi() != SemanticExternAbiV1::GpuKernel
        || abi.canon_abi() != SemanticCanonAbiV1::GpuKernel
        || abi.c_variadic()
        || abi.can_unwind()
        || abi.fixed_count() != 5
        || abi.source_input_types().len() != 5
    {
        return Err(complete_body_refusal("complete-body source ABI differs"));
    }
    if !matches!(
        semantic
            .types()
            .get(abi.source_output_type().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Unit)
    ) || !matches!(semantic.callables().first(), Some(SemanticCallableDeclV1::Defined { function }) if *function == root)
    {
        return Err(complete_body_refusal(
            "complete-body source unit/root callable differs",
        ));
    }
    for ty in &abi.source_input_types()[1..] {
        if lower_scalar_type(semantic.types(), *ty)? != Type::Scalar(ScalarType::U32) {
            return Err(complete_body_refusal(
                "complete-body scalar source arguments must be u32",
            ));
        }
    }
    let locals = function.locals().len();
    let blocks = function.blocks().len();
    if !(6..=COMPLETE_BODY_SOURCE_LOCAL_LIMIT).contains(&locals)
        || !(1..=COMPLETE_BODY_SOURCE_BLOCK_LIMIT).contains(&blocks)
        || blocks > limits.max_blocks
        || semantic.functions().len() > limits.max_functions
    {
        return Err(complete_body_refusal(
            "complete-body source dimensions exceed limits",
        ));
    }
    let mut items = locals;
    let mut statements = 0usize;
    budget.charge_work(blocks)?;
    for block in function.blocks() {
        statements = statements
            .checked_add(block.statements().len())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        items = items
            .checked_add(block.statements().len())
            .and_then(|v| v.checked_add(1))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if items > COMPLETE_BODY_SOURCE_ITEM_LIMIT || statements > limits.max_statements {
            return Err(complete_body_refusal(
                "complete-body source item bound exceeded",
            ));
        }
    }
    // Debit fixed arrays and the complete bounded semantic census before traversal.
    budget.charge_work(argument_sum_v1(&[
        COMPLETE_BODY_SOURCE_LOCAL_LIMIT,
        COMPLETE_BODY_SOURCE_BLOCK_LIMIT,
        argument_product_v1(items, 16)?,
        512,
    ])?)?;
    // Semantic locals are identity-sorted, not raw rustc positions.
    let mut return_local = None;
    let mut argument_locals = [usize::MAX; 5];
    for (index, local) in function.locals().iter().enumerate() {
        match local.role() {
            SemanticLocalRoleV1::Return => {
                if return_local.replace(index).is_some() || local.ty() != abi.source_output_type() {
                    return Err(complete_body_refusal("complete-body return role differs"));
                }
            }
            SemanticLocalRoleV1::Argument(ordinal) if ordinal < 5 => {
                let slot = &mut argument_locals[ordinal as usize];
                if *slot != usize::MAX || local.ty() != abi.source_input_types()[ordinal as usize] {
                    return Err(complete_body_refusal(
                        "complete-body argument role/type differs",
                    ));
                }
                *slot = index;
            }
            SemanticLocalRoleV1::Temporary => {}
            _ => {
                return Err(complete_body_refusal(
                    "complete-body extra source argument role",
                ));
            }
        }
    }
    let return_local =
        return_local.ok_or_else(|| complete_body_refusal("complete-body return role absent"))?;
    if argument_locals.contains(&usize::MAX) {
        return Err(complete_body_refusal("complete-body argument role absent"));
    }
    if owner.plan_for_function(root).is_none() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let mut transport =
        CompleteBodyArgumentTransportVNext::with_roles(locals, return_local, argument_locals)?;
    let mut visited = [false; COMPLETE_BODY_SOURCE_BLOCK_LIMIT];
    let mut current = function.entry();
    let mut selected = None;
    loop {
        let index = current.index() as usize;
        let block = function
            .blocks()
            .get(index)
            .ok_or_else(|| complete_body_refusal("complete-body semantic edge leaves root"))?;
        if visited[index] {
            return Err(complete_body_refusal(
                "complete-body semantic transport cycle",
            ));
        }
        visited[index] = true;
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Nop => {}
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => {
                    transport.storage(local.index() as usize)?
                }
                SemanticStatementKindV1::Assign(assign) => {
                    let destination = assign.destination();
                    if !destination.projections().is_empty() {
                        return Err(complete_body_refusal(
                            "complete-body projected source write",
                        ));
                    }
                    let SemanticRvalueKindV1::Use(operand) = assign.value().kind() else {
                        return Err(complete_body_refusal(
                            "complete-body source computes outside body",
                        ));
                    };
                    if destination.local().index() as usize == return_local {
                        if destination.ty() != abi.source_output_type()
                            || !matches!(operand, SemanticOperandV1::Constant(value)
                                if value.ty() == abi.source_output_type()
                                && matches!(value.value(), SemanticConstantValueV1::ZeroSized))
                        {
                            return Err(complete_body_refusal("complete-body return is not unit"));
                        }
                    } else {
                        let (source, moved) = complete_body_local_operand_vnext(operand)?;
                        if destination.ty() != source.ty() {
                            return Err(complete_body_refusal(
                                "complete-body transport changes type",
                            ));
                        }
                        transport.assign(
                            destination.local().index() as usize,
                            source.local().index() as usize,
                            moved,
                        )?;
                    }
                }
                _ => {
                    return Err(complete_body_refusal(
                        "complete-body source statement effect",
                    ));
                }
            }
        }
        current = match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => edge.target(),
            SemanticTerminatorKindV1::Call(call) if selected.is_none() => {
                let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(packing),
                    ..
                }) = semantic.callables().get(call.callee().index() as usize)
                else {
                    return Err(complete_body_refusal(
                        "complete-body source has another call",
                    ));
                };
                if call.arguments().len() != 10
                    || call.callee().index() != 1
                    || !matches!(
                        call.unwind(),
                        SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
                    )
                    || call.inline_assembly_source_v30().is_some()
                    || call.ordered_region_source_v31().is_some()
                    || call.ordered_program_source_v32().is_some()
                {
                    return Err(complete_body_refusal("complete-body marker shape differs"));
                }
                let destination = call.destination().ok_or_else(|| {
                    complete_body_refusal("complete-body marker continuation missing")
                })?;
                if !destination.place().projections().is_empty()
                    || destination.place().ty() != abi.source_output_type()
                {
                    return Err(complete_body_refusal(
                        "complete-body marker destination differs",
                    ));
                }
                let source = call.complete_body_source_vnext().ok_or_else(|| {
                    complete_body_refusal("complete-body source observation missing")
                })?;
                if !source.matches_function(function)
                    || source.raw_block() as usize >= blocks
                    || source.block_identity() != *block.identity().as_bytes()
                {
                    return Err(complete_body_refusal(
                        "complete-body current source occurrence differs",
                    ));
                }
                let mut args = [0u32; 5];
                let mut moved = 0u8;
                for (ordinal, operand) in call.arguments()[..5].iter().enumerate() {
                    let (place, is_move) = complete_body_local_operand_vnext(operand)?;
                    if place.ty() != abi.source_input_types()[ordinal] {
                        return Err(complete_body_refusal(
                            "complete-body marker argument type differs",
                        ));
                    }
                    args[ordinal] = place.local().index();
                    moved |= u8::from(is_move) << ordinal;
                }
                transport.consume_marker(args, moved)?;
                let mut registers = [0u8; 5];
                for (ordinal, operand) in call.arguments()[5..].iter().enumerate() {
                    let SemanticOperandV1::Constant(constant) = operand else {
                        return Err(complete_body_refusal(
                            "complete-body registers must be literal u8",
                        ));
                    };
                    if lower_scalar_type(semantic.types(), constant.ty())?
                        != Type::Scalar(ScalarType::U8)
                    {
                        return Err(complete_body_refusal("complete-body register type differs"));
                    }
                    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                        return Err(complete_body_refusal(
                            "complete-body register value differs",
                        ));
                    };
                    if value.size_bytes() != 1 {
                        return Err(complete_body_refusal(
                            "complete-body register width differs",
                        ));
                    }
                    registers[ordinal] = u8::try_from(value.bits())
                        .map_err(|_| complete_body_refusal("complete-body register overflow"))?;
                }
                let registers = fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1::new(
                    registers[0],
                    registers[1],
                    [registers[2], registers[3], registers[4]],
                )
                .map_err(|_| complete_body_refusal("complete-body overlapping physical roles"))?;
                let packed = fe2o3_kernel_ir::Gfx942CompleteBodyPackedV1::from_words(
                    packing.block_count,
                    packing.instruction_count,
                    packing.block_words,
                    packing.instruction_words,
                )
                .map_err(|_| complete_body_refusal("complete-body exact packing refused"))?;
                let origin = fe2o3_kernel_ir::Gfx942CompleteBodyOriginVNext {
                    root_axes: source.root_axes(),
                    mir_body: source.mir_body(),
                    semantic_block: source.block_identity(),
                    source_signature: source.source_signature(),
                    rustc_fn_abi: source.rustc_fn_abi(),
                    frontend_bytes_sha256: source.frontend_bytes_sha256(),
                    raw_block: source.raw_block(),
                };
                selected = Some((
                    current,
                    crate::CompleteBodyCanonicalInputVNext {
                        origin,
                        export_name: symbol,
                        registers,
                        packed,
                    },
                ));
                destination.edge().target()
            }
            SemanticTerminatorKindV1::Return => {
                transport.require_marker()?;
                if visited[..blocks].iter().any(|seen| !seen) {
                    return Err(complete_body_refusal(
                        "complete-body source contains unaccounted blocks",
                    ));
                }
                let (marker, input) = selected
                    .ok_or_else(|| complete_body_refusal("complete-body marker missing"))?;
                return Ok(CompleteBodyContextVNext {
                    function,
                    symbol,
                    input,
                    marker,
                    root,
                    argument_locals,
                });
            }
            _ => {
                return Err(complete_body_refusal(
                    "complete-body extra call/control/effect",
                ));
            }
        };
    }
}

fn complete_body_local_operand_vnext(
    operand: &SemanticOperandV1,
) -> Result<(&SemanticPlaceV1, bool), ProductionSemanticKirErrorV1> {
    let (place, moved) = match operand {
        SemanticOperandV1::Copy(place) => (place, false),
        SemanticOperandV1::Move(place) => (place, true),
        _ => {
            return Err(complete_body_refusal(
                "complete-body input is not direct source transport",
            ));
        }
    };
    if !place.projections().is_empty() {
        return Err(complete_body_refusal(
            "complete-body source transport projects",
        ));
    }
    Ok((place, moved))
}
