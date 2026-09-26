// Actual source/Function/Call/Return joins. Detached rows cannot construct an
// owner: the only producer below takes the live checked source and emitter map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SealedBf16CallRelationV1 {
    root: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    call_block: BlockId,
    call_ordinal: u32,
    matrix_block: BlockId,
    matrix_ordinal: u32,
    source_call_block: SemanticBlockIdV1,
    source_matrix_block: SemanticBlockIdV1,
    helper_return: [ValueId; 4],
    permutation: [u8; 4],
    capture: Bf16CallEmissionCaptureV1,
}

fn bf16_source_span_v1<'a>(
    correspondence: &'a SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a SemanticKirTerminatorOperationSpanV1, ProductionSemanticKirErrorV1> {
    let mut found = None;
    for span in &correspondence.terminator_operation_spans {
        budget.charge_work(1)?;
        if span.correspondence_owner == root
            && span.semantic_function == function
            && span.semantic_block == block
        {
            if found.replace(span).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
    }
    found.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}
fn bf16_inside_span_v1(
    span: &SemanticKirTerminatorOperationSpanV1,
    block: BlockId,
    ordinal: usize,
) -> bool {
    block == span.kernel_ir_block
        && ordinal >= span.first_operation_ordinal as usize
        && ordinal
            .checked_sub(span.first_operation_ordinal as usize)
            .is_some_and(|n| n < span.operation_count as usize)
}
fn bf16_function_v1<'a>(
    module: &'a Module,
    correspondence: &SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    role: SemanticKirFunctionRoleV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a Function, ProductionSemanticKirErrorV1> {
    let mut found = None;
    for row in &correspondence.lowered_functions {
        budget.charge_work(1)?;
        if row.correspondence_owner == root && row.semantic_function == function && row.role == role
        {
            if found.replace(&row.kernel_ir_function).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
    }
    let id = found.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(module.functions.len())?;
    module
        .function(id)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}

fn bf16_seal_call_relation_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    capture: Bf16CallEmissionCaptureV1,
    launch: RetainedRankedLaunchRootV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SealedBf16CallRelationV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(64)?;
    if capture.seen != [true, true]
        || correspondence.semantic_sha256
            != *source
                .owner()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
        || correspondence.function_count != 2
        || correspondence.lowered_functions.len() != 2
        || correspondence.private_arrays.active
        || !correspondence.synthetic_operation_spans.is_empty()
        || module.kernels.len() != 1
        || !(2..=3).contains(&module.functions.len())
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let root = bf16_function_v1(
        module,
        correspondence,
        source.root(),
        source.root(),
        SemanticKirFunctionRoleV1::KernelEntry,
        budget,
    )?;
    let helper = bf16_function_v1(
        module,
        correspondence,
        source.root(),
        source.helper(),
        SemanticKirFunctionRoleV1::InternalHelper,
        budget,
    )?;
    if root.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
        || helper.role != fe2o3_kernel_ir::FunctionRole::InternalHelper
        || module.kernels[0].entry != root.id
        || root.id == helper.id
        || helper.signature.parameters.len() != 12
        || helper.signature.results.len() != 4
        || helper
            .body
            .as_ref()
            .is_none_or(|body| body.parameters.len() != 12)
    {
        return Err(bf16_emission_refusal_v1(
            "BF16 actual function/signature roster",
        ));
    }
    for index in 0..12 {
        let ty = if index < 8 {
            ScalarType::Bf16
        } else {
            ScalarType::F32
        };
        if helper.signature.parameters[index] != Type::Scalar(ty)
            || helper
                .body
                .as_ref()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                .parameters[index]
                != capture.formals[1 + index / 4][index % 4]
        {
            return Err(bf16_emission_refusal_v1(
                "BF16 positional canonical formal identity",
            ));
        }
    }
    if helper.signature.results.iter().any(|t| *t != Type::F32) {
        return Err(bf16_emission_refusal_v1("BF16 canonical result type"));
    }
    let call_span = bf16_source_span_v1(
        correspondence,
        source.root(),
        source.root(),
        source.call_block(),
        budget,
    )?;
    let result_site = source.producer(Bf16CallInstanceRoleV1::Result);
    let matrix_span = bf16_source_span_v1(
        correspondence,
        source.root(),
        source.helper(),
        result_site.block(),
        budget,
    )?;
    let mut call_site = None;
    let mut matrix_site = None;
    let mut traps = 0usize;
    let mut returns = [0usize; 2];
    let mut helper_return = [ValueId(0); 4];
    for (slot, function) in [root, helper].into_iter().enumerate() {
        let body = function
            .body
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                budget.charge_work(operation.results.len() + 2)?;
                match &operation.kind {
                    OperationKind::Call { callee, arguments } if *callee == helper.id => {
                        if slot != 0
                            || !bf16_inside_span_v1(call_span, block.id, ordinal)
                            || arguments.len() != 12
                            || operation.results.len() != 4
                            || call_site.replace((block.id, ordinal)).is_some()
                        {
                            return Err(bf16_emission_refusal_v1("BF16 sole actual Call site"));
                        }
                        for (i, argument) in arguments.iter().enumerate() {
                            if *argument != capture.arguments[1 + i / 4][i % 4]
                                || bf16_transport_origin_v1(root, *argument, budget)?
                                    != bf16_transport_origin_v1(
                                        root,
                                        capture.producers[2 + i / 4][i % 4],
                                        budget,
                                    )?
                            {
                                return Err(bf16_emission_refusal_v1(
                                    "BF16 Call operand SSA identity",
                                ));
                            }
                        }
                        if operation
                            .results
                            .iter()
                            .enumerate()
                            .any(|(i, r)| r.ty != Type::F32 || r.id != capture.call_result[i])
                        {
                            return Err(bf16_emission_refusal_v1(
                                "BF16 Call result type or exact SSA identity",
                            ));
                        }
                    }
                    OperationKind::Call { callee, arguments } => {
                        if slot != 0
                            || !operation.results.is_empty()
                            || !matches!(
                                AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                                Some(AmdGpuDiagnosticOperation::Trap)
                            )
                        {
                            return Err(bf16_emission_refusal_v1("BF16 extra or non-Trap call"));
                        }
                        bf16_trap_source_v1(source, correspondence, block.id, ordinal, budget)?;
                        traps = traps.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                    OperationKind::Matrix(matrix) => {
                        let fe2o3_kernel_ir::MatrixOperationKind::MultiplyAccumulate {
                            lhs,
                            rhs,
                            accumulator,
                            ..
                        } = &matrix.kind
                        else {
                            return Err(bf16_emission_refusal_v1("BF16 different Matrix kind"));
                        };
                        if slot != 1
                            || !bf16_inside_span_v1(matrix_span, block.id, ordinal)
                            || matrix_site.replace((block.id, ordinal)).is_some()
                            || operation.results.len() != 4
                        {
                            return Err(bf16_emission_refusal_v1("BF16 exact Matrix source site"));
                        }
                        let expected =
                            MatrixOperation::multiply_accumulate(*lhs, *rhs, *accumulator)
                                .with_declared_tensor_layout(
                                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                                        .with_zero_filled_predicate_inputs(),
                                );
                        if *matrix != expected {
                            return Err(bf16_emission_refusal_v1("BF16 exact Matrix contract"));
                        }
                        for (group, values) in [lhs, rhs, accumulator].into_iter().enumerate() {
                            for (index, value) in values.iter().enumerate() {
                                if bf16_transport_origin_v1(helper, *value, budget)?
                                    != capture.formals[group + 1][index]
                                {
                                    return Err(bf16_emission_refusal_v1(
                                        "BF16 Matrix formal role or SSA identity",
                                    ));
                                }
                            }
                        }
                        for (index, result) in operation.results.iter().enumerate() {
                            if result.id != capture.producers[5][index] || result.ty != Type::F32 {
                                return Err(bf16_emission_refusal_v1(
                                    "BF16 Matrix result SSA identity",
                                ));
                            }
                        }
                    }
                    _ if slot == 1 => {
                        // The closed helper consists solely of the real MFMA;
                        // conversion and array permutation transport values.
                        return Err(bf16_emission_refusal_v1("BF16 extra helper operation"));
                    }
                    _ => {}
                }
            }
            match block.terminator.as_ref() {
                Some(Terminator::Return { values }) => {
                    returns[slot] += 1;
                    if slot == 0 {
                        if !values.is_empty() {
                            return Err(bf16_emission_refusal_v1("BF16 root return"));
                        }
                    } else {
                        if values.len() != 4 {
                            return Err(bf16_emission_refusal_v1("BF16 helper return width"));
                        }
                        for (i, value) in values.iter().enumerate() {
                            helper_return[i] = *value;
                            let expected =
                                capture.producers[5][source.return_permutation()[i] as usize];
                            if bf16_transport_origin_v1(helper, *value, budget)? != expected {
                                return Err(bf16_emission_refusal_v1(
                                    "BF16 exact returned component permutation",
                                ));
                            }
                        }
                    }
                }
                Some(Terminator::Branch { .. }) => {}
                Some(_) if slot == 0 => {}
                _ => return Err(bf16_emission_refusal_v1("BF16 helper control changed")),
            }
        }
    }
    if returns != [1, 1] {
        return Err(bf16_emission_refusal_v1("BF16 complete return roster"));
    }
    for function in &module.functions {
        budget.charge_work(1)?;
        if function.id != root.id
            && function.id != helper.id
            && (traps == 0 || *function != AmdGpuDiagnosticOperation::Trap.declaration())
        {
            return Err(bf16_emission_refusal_v1("BF16 external declaration roster"));
        }
    }
    if (module.functions.len() == 3) != (traps != 0) {
        return Err(bf16_emission_refusal_v1(
            "BF16 Trap declaration cardinality",
        ));
    }
    for i in 0..4 {
        if bf16_transport_origin_v1(helper, capture.producers[6][i], budget)?
            != capture.producers[5][i]
        {
            return Err(bf16_emission_refusal_v1(
                "BF16 accumulator conversion identity",
            ));
        }
    }
    let (call_block, call_ordinal) =
        call_site.ok_or_else(|| bf16_emission_refusal_v1("BF16 Call absent"))?;
    let (matrix_block, matrix_ordinal) =
        matrix_site.ok_or_else(|| bf16_emission_refusal_v1("BF16 Matrix absent"))?;
    bf16_full_wave_v1(
        module,
        root,
        helper,
        call_block,
        call_ordinal,
        launch,
        budget,
    )?;
    bf16_complete_source_coverage_v1(source, module, correspondence, budget)?;
    Ok(SealedBf16CallRelationV1 {
        root: source.root(),
        helper: source.helper(),
        call_block,
        call_ordinal: u32::try_from(call_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        matrix_block,
        matrix_ordinal: u32::try_from(matrix_ordinal)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        source_call_block: source.call_block(),
        source_matrix_block: result_site.block(),
        helper_return,
        permutation: source.return_permutation(),
        capture,
    })
}

fn bf16_trap_source_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    correspondence: &SemanticKirCorrespondenceV1,
    block: BlockId,
    ordinal: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let semantic = source.owner().source_semantic();
    let mut found = 0usize;
    for span in &correspondence.terminator_operation_spans {
        budget.charge_work(1)?;
        if span.correspondence_owner != source.root()
            || span.semantic_function != source.root()
            || !bf16_inside_span_v1(span, block, ordinal)
        {
            continue;
        }
        let statement = semantic.functions()[source.root().index() as usize]
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if let SemanticTerminatorKindV1::Call(call) = statement.terminator().kind()
            && call.arguments().is_empty()
            && matches!(
                semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Trap,
                    ..
                })
            )
        {
            found += 1;
        }
    }
    if found != 1 {
        return Err(bf16_emission_refusal_v1(
            "BF16 Trap lacks exact source origin",
        ));
    }
    Ok(())
}
