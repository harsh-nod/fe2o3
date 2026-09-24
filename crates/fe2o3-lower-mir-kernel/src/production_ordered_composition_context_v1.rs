// Exact source profile for the additive composition constructor. This is not
// rustc authentication: only the backend's private authenticated wrapper selects
// that constructor. Existing singleton and decoded-byte routes are unchanged.
#[derive(Clone, Copy, Debug)]
struct OrderedCompositionPermitV1 {
    semantic_sha256: [u8; 32],
    root: SemanticFunctionIdV1,
}
impl OrderedCompositionPermitV1 {
    fn matches(self, semantic: &AdmittedInertSemanticMirV1) -> bool {
        self.semantic_sha256 == *semantic.semantic_sha256().as_bytes()
            && semantic.roots() == [self.root]
    }
}

fn ordered_composition_refusal_v1(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn ordered_composition_u32_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    types.get(ty.index() as usize).is_some_and(|decl| {
        decl.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
            && matches!(
                lower_scalar_type(types, ty),
                Ok(Type::Scalar(ScalarType::U32))
            )
    })
}

fn ordered_composition_helper_abi_v1(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1, SemanticCanonAbiV1, SemanticExternAbiV1,
        SemanticSourceArgumentOwnershipV1,
    };
    let abi = function.abi();
    let result = abi.return_value();
    function.role() == SemanticFunctionRoleV1::InternalHelper
        && function.export().is_none()
        && function.kernel_entry().is_none()
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi.fixed_count() == 3
        && abi.arguments().len() == 3
        && abi.source_input_types().len() == 3
        && abi.source_argument_ownership().len() == 3
        && abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .zip(abi.source_argument_ownership())
            .all(|((argument, ty), ownership)| {
                argument.role() == SemanticAbiArgumentRoleV1::Source
                    && argument.ty() == *ty
                    && argument.value().adjusted().is_none()
                    && argument.value().pointee_override().is_none()
                    && matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
                    && *ownership == SemanticSourceArgumentOwnershipV1::ByValue
                    && ordered_composition_u32_v1(semantic.types(), *ty)
            })
        && result.source_ty() == abi.source_output_type()
        && result.adjusted().is_none()
        && result.pointee_override().is_none()
        && matches!(result.mode(), SemanticAbiPassModeV1::Direct(_))
        && ordered_composition_u32_v1(semantic.types(), abi.source_output_type())
}

fn ordered_composition_assignment_v1(
    types: &[SemanticTypeDeclV1],
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> bool {
    if !assignment.destination().projections().is_empty()
        || !ordered_composition_u32_v1(types, assignment.destination().ty())
        || !ordered_composition_u32_v1(types, assignment.value().result_type())
    {
        return false;
    }
    let operand = |v: &SemanticOperandV1| {
        ordered_composition_u32_v1(types, v.ty())
            && ordered_program_direct_scalar_operand_v32(types, v)
    };
    match assignment.value().kind() {
        SemanticRvalueKindV1::Use(value) => operand(value),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::Not,
            operand: value,
        } => operand(value),
        SemanticRvalueKindV1::Binary {
            operation:
                SemanticBinaryOpV1::Add
                | SemanticBinaryOpV1::Subtract
                | SemanticBinaryOpV1::BitAnd
                | SemanticBinaryOpV1::BitOr
                | SemanticBinaryOpV1::BitXor,
            left,
            right,
        } => operand(left) && operand(right),
        _ => false,
    }
}

// Marker=1, Defined helper=2, other=0. Other calls are admitted only in the
// post-composition root tail and retain ordinary lowering/formal requirements.
fn ordered_composition_call_kind_v1(
    semantic: &AdmittedInertSemanticMirV1,
    call: &SemanticDirectCallV1,
) -> u8 {
    match semantic.callables().get(call.callee().index() as usize) {
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_),
            ..
        }) => 1,
        Some(SemanticCallableDeclV1::Defined { .. }) => 2,
        _ => 0,
    }
}

fn validate_ordered_composition_context_v1(
    semantic: &AdmittedInertSemanticMirV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OrderedCompositionPermitV1, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticMirWireVersionV1, SemanticTargetArchitectureV1,
    };
    let fail = ordered_composition_refusal_v1;
    budget.charge_work(48)?;
    if semantic.wire_version() != SemanticMirWireVersionV1::V32
        || semantic.target().architecture() != SemanticTargetArchitectureV1::AmdGpuGfx942
        || !(1..=3).contains(&semantic.functions().len())
        || semantic.roots().len() != 1
        || launch.roots().len() != 1
        || launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
    {
        return Err(fail(
            "ordered composition requires exact MIR32 gfx942 and one source root",
        ));
    }
    let root = semantic.roots()[0];
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| fail("ordered composition root outside source"))?;
    let bounds = function
        .kernel_entry()
        .and_then(|entry| entry.source_contract().launch());
    let layout = launch.roots()[0].layout();
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || !bounds.is_some_and(|b| {
            b.required().map(|s| s.as_array()) == Some([64, 1, 1])
                && b.maximum().map(|s| s.as_array()) == Some([64, 1, 1])
        })
        || launch.roots()[0].selected_root() != root
        || layout.workgroup_extents() != [64, 1, 1]
        || layout.subgroup_size() != 64
        || !layout.full_physical_workgroups()
    {
        return Err(fail(
            "ordered composition requires exact source full Wave64 launch",
        ));
    }
    // Fixed three-function counters are stack scratch, not dynamic storage.
    let mut definitions = [0usize; 3];
    let mut instructions = [0usize; 3];
    let mut calls = [0usize; 3];
    let mut totals = [0usize; 2];
    for (index, source) in semantic.functions().iter().enumerate() {
        budget.charge_work(32)?;
        let id = SemanticFunctionIdV1::from_index(index as u32);
        if id != root && !ordered_composition_helper_abi_v1(semantic, source) {
            return Err(fail(
                "ordered composition helper requires exact direct three-u32 Rust ABI",
            ));
        }
        if source.blocks().len() > limits.max_blocks {
            return Err(fail("ordered composition source block bound"));
        }
        for (block_index, block) in source.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            match ordered_composition_call_kind_v1(semantic, call) {
                1 => {
                    budget.charge_work(4096)?;
                    semantic
                        .checked_gfx942_ordered_program_call_v32(
                            id,
                            SemanticBlockIdV1::from_index(block_index as u32),
                        )
                        .map_err(|_| fail("ordered composition immutable marker differs"))?;
                    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program),
                        ..
                    }) = semantic.callables().get(call.callee().index() as usize)
                    else {
                        return Err(fail("ordered composition marker census differs"));
                    };
                    definitions[index] = argument_sum_v1(&[definitions[index], 1])?;
                    instructions[index] =
                        argument_sum_v1(&[instructions[index], program.count() as usize])?;
                    totals[0] = argument_sum_v1(&[totals[0], 1])?;
                }
                2 => {
                    if id != root {
                        return Err(fail("ordered composition nested helper or recursion"));
                    }
                    let Some(SemanticCallableDeclV1::Defined { function: callee }) =
                        semantic.callables().get(call.callee().index() as usize)
                    else {
                        unreachable!();
                    };
                    let slot = calls
                        .get_mut(callee.index() as usize)
                        .ok_or_else(|| fail("ordered composition callee outside source"))?;
                    if *callee == root {
                        return Err(fail("ordered composition recursive root"));
                    }
                    *slot = argument_sum_v1(&[*slot, 1])?;
                    totals[1] = argument_sum_v1(&[totals[1], 1])?;
                    if call.arguments().len() != 3
                        || call.arguments().iter().any(|a| {
                            !ordered_composition_u32_v1(semantic.types(), a.ty())
                                || !ordered_program_direct_scalar_operand_v32(semantic.types(), a)
                        })
                        || call.destination().is_none_or(|d| {
                            !d.place().projections().is_empty()
                                || !ordered_composition_u32_v1(semantic.types(), d.place().ty())
                        })
                        || !matches!(
                            call.unwind(),
                            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
                        )
                    {
                        return Err(fail("ordered composition helper call transport differs"));
                    }
                }
                _ => {
                    // No old assembly marker may slip into the normal tail.
                    if call.inline_assembly_source_v30().is_some()
                        || call.ordered_region_source_v31().is_some()
                        || call.ordered_program_source_v32().is_some()
                    {
                        return Err(fail("ordered composition has a foreign assembly marker"));
                    }
                }
            }
        }
    }
    if !(1..=8).contains(&totals[0]) || totals[1] > 8 {
        return Err(fail("ordered composition static definition/call bound"));
    }
    let mut expanded = definitions[root.index() as usize];
    let mut expanded_instructions = instructions[root.index() as usize];
    for index in 0..semantic.functions().len() {
        if index == root.index() as usize {
            continue;
        }
        if calls[index] == 0 {
            return Err(fail("ordered composition unused helper"));
        }
        expanded = argument_sum_v1(&[
            expanded,
            argument_product_v1(calls[index], definitions[index])?,
        ])?;
        expanded_instructions = argument_sum_v1(&[
            expanded_instructions,
            argument_product_v1(calls[index], instructions[index])?,
        ])?;
    }
    if expanded > 8 || expanded_instructions > 128 {
        return Err(fail(
            "ordered composition expanded region/instruction bound",
        ));
    }
    for (index, source) in semantic.functions().iter().enumerate() {
        ordered_composition_prefix_v1(
            semantic,
            source,
            index == root.index() as usize,
            definitions[index]
                + if index == root.index() as usize {
                    totals[1]
                } else {
                    0
                },
            limits,
            budget,
        )?;
    }
    Ok(OrderedCompositionPermitV1 {
        root,
        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
    })
}

fn ordered_composition_prefix_v1(
    semantic: &AdmittedInertSemanticMirV1,
    source: &SemanticFunctionDeclV1,
    root: bool,
    expected: usize,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        let fail = ordered_composition_refusal_v1;
        let count = source.blocks().len();
        budget.charge_work(count)?;
        budget.reserve_storage(argument_product_v1(count, std::mem::size_of::<usize>())?)?;
        let mut prefix = argument_vec_v1(count)?;
        let extra = prefix
            .capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.reserve_storage(argument_product_v1(extra, std::mem::size_of::<usize>())?)?;
        prefix.resize(count, usize::MAX);
        let mut current = source.entry().index() as usize;
        let mut ordinal = 0usize;
        let mut found = 0usize;
        let mut statements = 0usize;
        loop {
            budget.charge_work(1)?;
            if root && found == expected {
                break;
            }
            let slot = prefix
                .get_mut(current)
                .ok_or_else(|| fail("ordered composition prefix leaves CFG"))?;
            if *slot != usize::MAX {
                return Err(fail("ordered composition cyclic prefix"));
            }
            *slot = ordinal;
            let block = &source.blocks()[current];
            for statement in block.statements() {
                budget.charge_work(12)?;
                statements = argument_sum_v1(&[statements, 1])?;
                enforce_limit(
                    ProductionSemanticKirResourceV1::Statements,
                    statements,
                    limits.max_statements,
                )?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(a)
                        if ordered_composition_assignment_v1(semantic.types(), a) => {}
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                    _ => {
                        return Err(fail(
                            "ordered composition prefix has unmodeled scalar/effect",
                        ));
                    }
                }
            }
            let edge = match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => *edge,
                SemanticTerminatorKindV1::Call(call)
                    if ordered_composition_call_kind_v1(semantic, call) != 0 =>
                {
                    found = argument_sum_v1(&[found, 1])?;
                    if found > expected {
                        return Err(fail("ordered composition prefix census differs"));
                    }
                    call.destination()
                        .ok_or_else(|| fail("ordered composition missing return edge"))?
                        .edge()
                }
                SemanticTerminatorKindV1::Return if !root && found == expected => break,
                _ => {
                    return Err(fail(
                        "ordered composition requires unconditional complete prefix",
                    ));
                }
            };
            current = edge.target().index() as usize;
            ordinal = argument_sum_v1(&[ordinal, 1])?;
        }
        if !root && prefix.contains(&usize::MAX) {
            return Err(fail(
                "ordered composition helper has unreachable or alternate blocks",
            ));
        }
        for (index, block) in source.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            block.terminator().kind().try_for_each_edge(|edge| {
                budget.charge_work(1)?;
                let destination = *prefix
                    .get(edge.target().index() as usize)
                    .ok_or_else(|| fail("ordered composition edge leaves CFG"))?;
                if destination != usize::MAX
                    && (destination == 0
                        || prefix[index] == usize::MAX
                        || prefix[index].checked_add(1) != Some(destination))
                {
                    return Err(fail("ordered composition alternate entry or backedge"));
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}
