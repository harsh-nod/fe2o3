// Independent full-participation check on the actual emitted caller. Kernel
// ABI parameters are uniform; memory values and unknown results stay varying.
// This does not turn an unknown helper result into a uniform value.
fn bf16_full_wave_v1(
    module: &Module,
    root: &Function,
    helper: &Function,
    call_block: BlockId,
    call_ordinal: usize,
    launch: RetainedRankedLaunchRootV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if launch.launch_rank != 1
        || launch.global_extents != [64, 1, 1]
        || launch.workgroup_extents != [64, 1, 1]
        || !launch.full_physical_workgroups
    {
        return Err(bf16_emission_refusal_v1(
            "BF16 exact full Wave64 launch required",
        ));
    }
    let body = root
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    bf16_acyclic_cfg_v1(root, budget)?;
    bf16_acyclic_cfg_v1(helper, budget)?;
    // The analysis uses ordinary allocating containers. Its complete finite
    // envelope is paid by the outer original-ledger emission reservation.
    // Prepay all fixed-point block/value scans before calling it.
    let values = body
        .blocks
        .iter()
        .try_fold(root.signature.parameters.len(), |n, block| {
            let n = n
                .checked_add(block.parameters.len())
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            block.operations.iter().try_fold(n, |n, op| {
                n.checked_add(op.results.len())
                    .ok_or(ArgumentResourceV1::Arithmetic)
            })
        })?;
    let b = body.blocks.len();
    let work = (b + 1)
        .checked_mul(b + 1)
        .and_then(|n| n.checked_mul(values + 1))
        .and_then(|n| n.checked_mul(64))
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    budget.charge_work(work)?;
    let report = fe2o3_kernel_analysis::analyze_kernel_entry(module, root);
    if report.function() != &root.id
        || report.block_control(call_block) > fe2o3_kernel_analysis::Variation::WorkgroupUniform
    {
        return Err(bf16_emission_refusal_v1(
            "BF16 call lacks uniform full-wave control",
        ));
    }
    for diagnostic in report.diagnostics() {
        budget.charge_work(1)?;
        // The sole nominal call has no generic value-uniformity summary. That
        // absence is not granted a summary: its results remain Varying. Only
        // the PRE-call control fact is consumed by this typed checker.
        match diagnostic {
            fe2o3_kernel_analysis::Diagnostic::Unsupported {
                block: Some(block),
                operation_index: Some(index),
                reason: fe2o3_kernel_analysis::UnsupportedReason::CallWithoutSummary { callee },
            } if *block == call_block && *index == call_ordinal && *callee == helper.id => {}
            _ => {
                return Err(bf16_emission_refusal_v1(
                    "BF16 caller control analysis incomplete",
                ));
            }
        }
    }
    Ok(())
}

fn bf16_each_edge_v1(
    terminator: &Terminator,
    mut visit: impl FnMut(BlockId, &[ValueId]) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    match terminator {
        Terminator::Branch { target, arguments } => visit(*target, arguments)?,
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            visit(*then_target, then_arguments)?;
            visit(*else_target, else_arguments)?;
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            if cases.len() > 8 {
                return Err(bf16_emission_refusal_v1("BF16 switch fanout"));
            }
            for case in cases {
                visit(case.target, &case.arguments)?;
            }
            visit(*default_target, default_arguments)?;
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            if cases.len() > 8 {
                return Err(bf16_emission_refusal_v1("BF16 switch fanout"));
            }
            for case in cases {
                visit(case.target, &case.arguments)?;
            }
            visit(*default_target, default_arguments)?;
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}

fn bf16_acyclic_cfg_v1(
    function: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let blocks = &function
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .blocks;
    if blocks.is_empty() || blocks.len() > 32 {
        return Err(bf16_emission_refusal_v1("BF16 canonical block bound"));
    }
    // Allocation-free Kahn traversal. All CFG edges (including dead blocks)
    // count; no cycle or hidden helper dispatch is waived by this profile.
    let mut incoming = [0usize; 32];
    let mut removed = [false; 32];
    for block in blocks {
        let term = block
            .terminator
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        bf16_each_edge_v1(term, |target, _| {
            budget.charge_work(blocks.len())?;
            let index = blocks
                .iter()
                .position(|b| b.id == target)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            incoming[index] = incoming[index]
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            Ok(())
        })?;
    }
    for _ in 0..blocks.len() {
        budget.charge_work(blocks.len())?;
        let index = (0..blocks.len())
            .find(|i| !removed[*i] && incoming[*i] == 0)
            .ok_or_else(|| bf16_emission_refusal_v1("BF16 cyclic control unavailable"))?;
        removed[index] = true;
        bf16_each_edge_v1(blocks[index].terminator.as_ref().unwrap(), |target, _| {
            budget.charge_work(blocks.len())?;
            let next = blocks
                .iter()
                .position(|b| b.id == target)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            incoming[next] = incoming[next]
                .checked_sub(1)
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(())
        })?;
    }
    Ok(())
}

// Resolve only literal identity transport through a unique actual predecessor.
// No value equivalence, constant folding, arithmetic or numeric oracle is used.
fn bf16_transport_origin_v1(
    function: &Function,
    mut value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    let blocks = &function
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .blocks;
    for _ in 0..=32 {
        let mut parameter = None;
        for block in blocks {
            budget.charge_work(block.parameters.len() + 1)?;
            if let Some(index) = block.parameters.iter().position(|p| p.id == value) {
                if parameter.replace((block.id, index)).is_some() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
        }
        let Some((target, index)) = parameter else {
            return Ok(value);
        };
        let mut incoming = None;
        for block in blocks {
            bf16_each_edge_v1(
                block
                    .terminator
                    .as_ref()
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                |actual, args| {
                    budget.charge_work(1)?;
                    if actual == target {
                        let next = *args
                            .get(index)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if incoming.replace(next).is_some() {
                            return Err(bf16_emission_refusal_v1(
                                "BF16 multi-predecessor nominal transport",
                            ));
                        }
                    }
                    Ok(())
                },
            )?;
        }
        value = incoming.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    }
    Err(bf16_emission_refusal_v1("BF16 nominal edge depth"))
}
