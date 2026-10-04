// Included by production_physical_entry_source_v20.rs in the sole importer.
// Exact MIR37 source grammar; no older-profile source is relabeled.
const PHYSICAL_ENTRY_SOURCE_BLOCK_LIMIT: usize = 4096;
const PHYSICAL_ENTRY_SOURCE_LOCAL_LIMIT: usize = 4096;
const PHYSICAL_ENTRY_SOURCE_ITEM_LIMIT: usize = 65_536;

struct PhysicalEntryContextV20<'a> {
    function: &'a SemanticFunctionDeclV1,
    symbol: &'a str,
    origin: fe2o3_kernel_ir::Gfx942PhysicalEntryOriginVNext,
    begin: fe2o3_kernel_ir::Gfx942PhysicalEntrySourceSiteVNext,
    events: [crate::physical_entry_materialization_v20::Event; 72],
    event_count: usize,
    root: SemanticFunctionIdV1,
    argument_locals: [usize; 5],
}

fn physical_entry_refusal(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}
fn physical_entry_symbol_v20(symbol: &str) -> bool {
    (1..=128).contains(&symbol.len())
        && symbol
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && symbol
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn physical_entry_context_v20<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PhysicalEntryContextV20<'a>, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCanonAbiV1, SemanticExternAbiV1, SemanticLocalRoleV1, SemanticMirWireVersionV1,
    };
    budget.charge_work(16)?;
    let semantic = owner.source_semantic();
    if semantic.wire_version() != SemanticMirWireVersionV1::V37
        || semantic.target().architecture()
            != fe2o3_mir_model::semantic_mir_v1::SemanticTargetArchitectureV1::AmdGpuGfx942
        || semantic.functions().len() != 1
        || semantic.roots().len() != 1
        || !(2..=74).contains(&semantic.callables().len())
        || launch.roots().len() != 1
        || !semantic.allocations().is_empty()
        || !semantic.statics().is_empty()
        || !semantic.vtables().is_empty()
        || launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
    {
        return Err(physical_entry_refusal(
            "physical-entry exact source profile/roster differs",
        ));
    }
    let root = semantic.roots()[0];
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| physical_entry_refusal("physical-entry root missing"))?;
    let launch_root = launch.roots()[0];
    let geometry = launch_root.layout();
    if geometry.global_extents()[0] == dialect_kernel::DYNAMIC_EXTENT {
        return Err(physical_entry_refusal(
            "physical-entry source requires an explicit finite max_grid",
        ));
    }
    let entry = function
        .kernel_entry()
        .ok_or_else(|| physical_entry_refusal("physical-entry source export missing"))?;
    let launch_contract = entry
        .source_contract()
        .launch()
        .ok_or_else(|| physical_entry_refusal("physical-entry source launch missing"))?;
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || root.index() != 0
        || launch_root.selected_root() != root
        || launch_root.semantic_root_identity() != function.identity()
        || launch_root.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
        || launch_root.source_rank() != 1
        || geometry.workgroup_extents() != [64, 1, 1]
        || geometry.global_extents() != [128, 1, 1]
        || launch_root.source_launch().max_grid() != [2, 1, 1]
        || geometry.subgroup_size() != 64
        || !geometry.full_physical_workgroups()
        || launch_contract.required().map(|v| v.as_array()) != Some([64, 1, 1])
        || launch_contract.maximum().map(|v| v.as_array()) != Some([64, 1, 1])
    {
        return Err(physical_entry_refusal(
            "physical-entry current root/launch identity differs",
        ));
    }
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| physical_entry_refusal("physical-entry symbol is not UTF-8"))?;
    budget.charge_work(symbol.len())?;
    if !physical_entry_symbol_v20(symbol) {
        return Err(physical_entry_refusal(
            "physical-entry symbol must be unchanged ASCII identifier <=128 bytes",
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
        return Err(physical_entry_refusal("physical-entry source ABI differs"));
    }
    if !matches!(
        semantic
            .types()
            .get(abi.source_output_type().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Unit)
    ) || !matches!(semantic.callables().first(), Some(SemanticCallableDeclV1::Defined { function }) if *function == root)
    {
        return Err(physical_entry_refusal(
            "physical-entry source unit/root callable differs",
        ));
    }
    for ty in &abi.source_input_types()[1..] {
        if lower_scalar_type(semantic.types(), *ty)? != Type::Scalar(ScalarType::U32) {
            return Err(physical_entry_refusal(
                "physical-entry scalar source arguments must be u32",
            ));
        }
    }
    let locals = function.locals().len();
    let blocks = function.blocks().len();
    if !(6..=PHYSICAL_ENTRY_SOURCE_LOCAL_LIMIT).contains(&locals)
        || !(1..=PHYSICAL_ENTRY_SOURCE_BLOCK_LIMIT).contains(&blocks)
        || blocks > limits.max_blocks
        || semantic.functions().len() > limits.max_functions
    {
        return Err(physical_entry_refusal(
            "physical-entry source dimensions exceed limits",
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
        if items > PHYSICAL_ENTRY_SOURCE_ITEM_LIMIT || statements > limits.max_statements {
            return Err(physical_entry_refusal(
                "physical-entry source item bound exceeded",
            ));
        }
    }
    // Debit fixed arrays and the complete bounded semantic census before traversal.
    budget.charge_work(argument_sum_v1(&[
        PHYSICAL_ENTRY_SOURCE_LOCAL_LIMIT,
        PHYSICAL_ENTRY_SOURCE_BLOCK_LIMIT,
        argument_product_v1(items, 16)?,
        512,
    ])?)?;
    // Every callable after the actual root must be one exact new marker.
    for callable in &semantic.callables()[1..] {
        if !matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
                    | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(_)
                    | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(_),
                ..
            }
        ) {
            return Err(physical_entry_refusal(
                "physical-entry has a foreign callable",
            ));
        }
    }
    // Semantic locals are identity-sorted, not raw rustc positions.
    let mut return_local = None;
    let mut argument_locals = [usize::MAX; 5];
    for (index, local) in function.locals().iter().enumerate() {
        match local.role() {
            SemanticLocalRoleV1::Return => {
                if return_local.replace(index).is_some() || local.ty() != abi.source_output_type() {
                    return Err(physical_entry_refusal("physical-entry return role differs"));
                }
            }
            SemanticLocalRoleV1::Argument(ordinal) if ordinal < 5 => {
                let slot = &mut argument_locals[ordinal as usize];
                if *slot != usize::MAX || local.ty() != abi.source_input_types()[ordinal as usize] {
                    return Err(physical_entry_refusal(
                        "physical-entry argument role/type differs",
                    ));
                }
                *slot = index;
            }
            SemanticLocalRoleV1::Temporary => {}
            _ => {
                return Err(physical_entry_refusal(
                    "physical-entry extra source argument role",
                ));
            }
        }
    }
    let return_local =
        return_local.ok_or_else(|| physical_entry_refusal("physical-entry return role absent"))?;
    if argument_locals.contains(&usize::MAX) {
        return Err(physical_entry_refusal(
            "physical-entry argument role absent",
        ));
    }
    if owner.plan_for_function(root).is_none() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let mut transport =
        PhysicalEntryArgumentTransportV20::with_roles(locals, return_local, argument_locals)?;
    let mut visited = [false; PHYSICAL_ENTRY_SOURCE_BLOCK_LIMIT];
    let mut current = function.entry();
    use crate::physical_entry_materialization_v20::{Event, EventKind};
    use fe2o3_kernel_ir::{
        Gfx942PhysicalEntryOriginVNext as Origin, Gfx942PhysicalEntrySourceSiteVNext as Site,
    };
    let mut events = [Event {
        site: Site::ZERO,
        kind: EventKind::Label(0),
    }; 72];
    let mut event_count = 0;
    let mut origin = None;
    let mut begin = None;
    let mut calls = 0u8;
    let mut used_callables = [false; 74];
    used_callables[0] = true;
    loop {
        let index = current.index() as usize;
        let block = function
            .blocks()
            .get(index)
            .ok_or_else(|| physical_entry_refusal("physical-entry semantic edge leaves root"))?;
        if visited[index] {
            return Err(physical_entry_refusal(
                "physical-entry semantic marker chain cycles",
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
                        return Err(physical_entry_refusal(
                            "physical-entry projected source write",
                        ));
                    }
                    let SemanticRvalueKindV1::Use(operand) = assign.value().kind() else {
                        return Err(physical_entry_refusal(
                            "physical-entry source computes outside native instructions",
                        ));
                    };
                    if destination.local().index() as usize == return_local {
                        if destination.ty() != abi.source_output_type()
                            || !matches!(operand,SemanticOperandV1::Constant(value) if value.ty()==abi.source_output_type()&&matches!(value.value(),SemanticConstantValueV1::ZeroSized))
                        {
                            return Err(physical_entry_refusal(
                                "physical-entry source return is not unit",
                            ));
                        }
                    } else {
                        if calls != 0 {
                            return Err(physical_entry_refusal(
                                "physical-entry source transport after begin",
                            ));
                        }
                        let (source, moved) = physical_entry_local_operand_v20(operand)?;
                        if destination.ty() != source.ty() {
                            return Err(physical_entry_refusal(
                                "physical-entry transport changes type",
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
                    return Err(physical_entry_refusal(
                        "physical-entry source statement has an unadmitted effect",
                    ));
                }
            }
        }
        current = match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => edge.target(),
            SemanticTerminatorKindV1::Call(call) => {
                if calls >= 73 {
                    return Err(physical_entry_refusal(
                        "physical-entry source exceeds 73 occurrences",
                    ));
                }
                let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                    semantic.callables().get(call.callee().index() as usize)
                else {
                    return Err(physical_entry_refusal(
                        "physical-entry source has a foreign call",
                    ));
                };
                if !matches!(
                    call.unwind(),
                    SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
                ) || !call.variadic_argument_abis().is_empty()
                    || call.inline_assembly_source_v30().is_some()
                    || call.ordered_region_source_v31().is_some()
                    || call.ordered_program_source_v32().is_some()
                    || call.complete_body_source_vnext().is_some()
                {
                    return Err(physical_entry_refusal(
                        "physical-entry source marker shape differs",
                    ));
                }
                let destination = call.destination().ok_or_else(|| {
                    physical_entry_refusal("physical-entry source continuation absent")
                })?;
                if !destination.place().projections().is_empty()
                    || destination.place().ty() != abi.source_output_type()
                {
                    return Err(physical_entry_refusal(
                        "physical-entry source marker destination differs",
                    ));
                }
                let source = call.physical_entry_source_v37().ok_or_else(|| {
                    physical_entry_refusal("physical-entry source occurrence absent")
                })?;
                if !source.matches_function(function)
                    || source.occurrence() != calls
                    || source.raw_block() as usize >= blocks
                    || source.block_identity() != *block.identity().as_bytes()
                {
                    return Err(physical_entry_refusal(
                        "physical-entry current source occurrence differs",
                    ));
                }
                let current_origin = Origin {
                    root_axes: source.root_axes(),
                    mir_body: source.mir_body(),
                    source_signature: source.source_signature(),
                    rustc_fn_abi: source.rustc_fn_abi(),
                    frontend_bytes_sha256: source.frontend_bytes_sha256(),
                };
                let site = Site {
                    occurrence: calls,
                    raw_block: source.raw_block(),
                    semantic_block_index: current.index(),
                    semantic_block_identity: source.block_identity(),
                    semantic_callable_index: call.callee().index(),
                };
                if origin.is_some_and(|old| old != current_origin) {
                    return Err(physical_entry_refusal(
                        "physical-entry occurrence root/body/ABI lineage differs",
                    ));
                }
                let event = match operation {
                    SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
                        if calls == 0 =>
                    {
                        if call.arguments().len() != 5 {
                            return Err(physical_entry_refusal(
                                "physical-entry begin arity differs",
                            ));
                        }
                        let mut args = [0; 5];
                        let mut moved = 0u8;
                        for (ordinal, operand) in call.arguments().iter().enumerate() {
                            let (place, is_move) = physical_entry_local_operand_v20(operand)?;
                            if place.ty() != abi.source_input_types()[ordinal] {
                                return Err(physical_entry_refusal(
                                    "physical-entry begin source operand type differs",
                                ));
                            }
                            args[ordinal] = place.local().index();
                            moved |= u8::from(is_move) << ordinal;
                        }
                        transport.consume_marker(args, moved)?;
                        origin = Some(current_origin);
                        begin = Some(site);
                        None
                    }
                    SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(label)
                        if calls > 0 && call.arguments().is_empty() =>
                    {
                        Some(EventKind::Label(*label))
                    }
                    SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(step)
                        if calls > 1 && call.arguments().is_empty() =>
                    {
                        let instruction =
                            fe2o3_kernel_ir::Gfx942PhysicalEntryInstructionVNext::from_descriptor(
                                step.descriptor(),
                            )
                            .map_err(|_| {
                                physical_entry_refusal(
                                    "physical-entry primitive differs from exact KIR20 grammar",
                                )
                            })?;
                        Some(EventKind::Step(instruction))
                    }
                    _ => {
                        return Err(physical_entry_refusal(
                            "physical-entry begin/label/step ordering or runtime arity differs",
                        ));
                    }
                };
                if let Some(kind) = event {
                    events[event_count] = Event { site, kind };
                    event_count += 1;
                }
                used_callables[call.callee().index() as usize] = true;
                calls += 1;
                destination.edge().target()
            }
            SemanticTerminatorKindV1::Return => {
                transport.require_marker()?;
                if visited[..blocks].iter().any(|seen| !*seen)
                    || used_callables[..semantic.callables().len()]
                        .iter()
                        .any(|used| !*used)
                {
                    return Err(physical_entry_refusal(
                        "physical-entry source has unaccounted blocks or callable declarations",
                    ));
                }
                return Ok(PhysicalEntryContextV20 {
                    function,
                    symbol,
                    root,
                    argument_locals,
                    origin: origin.ok_or_else(|| {
                        physical_entry_refusal("physical-entry begin origin absent")
                    })?,
                    begin: begin.ok_or_else(|| {
                        physical_entry_refusal("physical-entry begin site absent")
                    })?,
                    events,
                    event_count,
                });
            }
            _ => {
                return Err(physical_entry_refusal(
                    "physical-entry extra source call/control/effect",
                ));
            }
        };
    }
}

impl PhysicalEntryContextV20<'_> {
    fn input(&self) -> crate::physical_entry_materialization_v20::Input<'_> {
        crate::physical_entry_materialization_v20::Input {
            origin: self.origin,
            export_name: self.symbol,
            begin: self.begin,
            events: &self.events[..self.event_count],
        }
    }
}
fn physical_entry_local_operand_v20(
    operand: &SemanticOperandV1,
) -> Result<(&SemanticPlaceV1, bool), ProductionSemanticKirErrorV1> {
    let (place, moved) = match operand {
        SemanticOperandV1::Copy(place) => (place, false),
        SemanticOperandV1::Move(place) => (place, true),
        _ => {
            return Err(physical_entry_refusal(
                "physical-entry source input is not direct argument transport",
            ));
        }
    };
    if !place.projections().is_empty() {
        return Err(physical_entry_refusal(
            "physical-entry source transport projects",
        ));
    }
    Ok((place, moved))
}
