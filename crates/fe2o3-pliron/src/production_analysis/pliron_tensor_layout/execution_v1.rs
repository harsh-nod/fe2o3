#[cfg(test)]
pub(crate) fn run_pliron_tensor_layout_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironTensorLayoutReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_tensor_layout_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_tensor_layout_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironTensorLayoutReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(_) => return report(vec![PlironTensorLayoutFindingV1::ResourceLimitExceeded]),
    };
    let mut findings = Vec::new();
    let mut operation_count = 0;
    let mut tensor_sites = Vec::new();
    for site in inventory.operations() {
        let block_index = site.block();
        let operation_index = site.operation();
        operation_count += 1;
        if operation_count > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1
            || findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1
        {
            findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
            return report(findings);
        }
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(layout) = operation.downcast_ref::<TensorLayoutOp>() else {
            continue;
        };
        let contract = layout.contract(context);
        tensor_sites.push((
            block_index,
            operation_index,
            layout.active_lanes(context),
            contract
                .as_ref()
                .ok()
                .map(|contract| u64::from(contract.subgroup_width)),
        ));
        let Ok(contract) = contract else {
            findings.push(PlironTensorLayoutFindingV1::MalformedContract {
                block: block_index,
                operation: operation_index,
            });
            continue;
        };
        if layout.convergence(context) != Some(TensorConvergenceAttr::UniformSubgroup) {
            findings.push(PlironTensorLayoutFindingV1::ConvergenceMismatch {
                block: block_index,
                operation: operation_index,
                actual: layout
                    .convergence(context)
                    .unwrap_or(TensorConvergenceAttr::Opaque),
            });
        }
        for finding in verify_tensor_layout_contract_v1(&contract) {
            if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                return report(findings);
            }
            findings.push(PlironTensorLayoutFindingV1::Contract {
                block: block_index,
                operation: operation_index,
                finding,
            });
        }
    }
    analyses.prepare_tensor_layout_dataflow(context, function);
    match analyses.tensor_layout_dataflow() {
        Ok(dataflow) => {
            for issue in dataflow.issues() {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::Dataflow(Box::new(
                    issue.clone(),
                )));
            }
        }
        Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit) => {
            findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
            return report(findings);
        }
        Err(PlironTensorLayoutDataflowFailureV1::MalformedSite { block, operation }) => {
            findings.push(PlironTensorLayoutFindingV1::MalformedContract { block, operation });
            return report(findings);
        }
    }
    if !tensor_sites.is_empty() {
        analyses.prepare_execution_layout(context, function);
        let layout = match analyses.execution_layout() {
            Ok(Some(layout)) => layout,
            Ok(None) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "tensor instructions require one authenticated gpu.execution_layout in the entry block"
                        .to_owned(),
                });
                return report(findings);
            }
            Err(failure) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
                return report(findings);
            }
        };
        for (block, operation, active_lanes, contract_width) in &tensor_sites {
            if let Some(required) = contract_width
                && layout.subgroup_size != *required
            {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::ExecutionLayoutMismatch {
                    block: *block,
                    operation: *operation,
                    declared: layout.subgroup_size,
                    required: *required,
                });
            }
            if let Some(actual) = active_lanes
                && u64::from(*actual) != layout.subgroup_size
            {
                if findings.len() >= MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
                    findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    return report(findings);
                }
                findings.push(PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                    block: *block,
                    operation: *operation,
                    expected: layout.subgroup_size,
                    actual: *actual,
                });
            }
        }
        analyses.prepare_exact_trace(context, function);
        match analyses.exact_trace() {
            Ok(traces) => {
                if let Some(finding) = exact_subgroup_trace_finding(traces, layout.subgroup_size) {
                    findings.push(finding);
                }
            }
            Err(PlironTraceFailureV1::Sparse(SparseIndexFailureV1::ResourceLimit { .. })) => {
                findings.push(PlironTensorLayoutFindingV1::ResourceLimitExceeded)
            }
            Err(
                PlironTraceFailureV1::DynamicLaunch { .. }
                | PlironTraceFailureV1::LaunchTooLarge { .. }
                | PlironTraceFailureV1::UnresolvedBranch { .. }
                | PlironTraceFailureV1::CyclicControlFlow { .. }
                | PlironTraceFailureV1::UnsupportedTerminator { .. },
            ) => match symbolic_subgroup_convergence(
                context,
                function,
                &inventory,
                layout,
                analyses,
                &tensor_sites
                    .iter()
                    .map(|(block, operation, _, _)| (*block, *operation))
                    .collect::<Vec<_>>(),
            ) {
                Ok(()) => {}
                Err(finding) => findings.push(finding),
            },
            Err(failure) => {
                findings.push(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
            }
        }
    }
    report(findings)
}

fn exact_subgroup_trace_finding(
    traces: &[PlironInvocationTraceV1],
    subgroup_size: u64,
) -> Option<PlironTensorLayoutFindingV1> {
    let mut groups = BTreeMap::<(u64, u64, u64), Vec<&PlironInvocationTraceV1>>::new();
    for trace in traces {
        groups
            .entry((trace.grid, trace.workgroup, trace.subgroup))
            .or_default()
            .push(trace);
    }
    for ((grid, workgroup, subgroup), group) in groups {
        if !group.iter().any(|trace| !tensor_trace(trace).is_empty()) {
            continue;
        }
        let lanes = group.iter().map(|trace| trace.lane).collect::<HashSet<_>>();
        let complete = lanes.len() == subgroup_size as usize
            && group.len() == subgroup_size as usize
            && (0..subgroup_size).all(|lane| lanes.contains(&lane));
        if !complete {
            return Some(PlironTensorLayoutFindingV1::PartialSubgroupParticipation {
                grid,
                workgroup,
                subgroup,
                expected: subgroup_size,
                actual: lanes.len(),
            });
        }
        let Some(first) = group.first().copied() else {
            continue;
        };
        let first_tensor = tensor_trace(first);
        for trace in group.iter().copied().skip(1) {
            let tensor = tensor_trace(trace);
            if tensor != first_tensor {
                return Some(PlironTensorLayoutFindingV1::DivergentInstructionTrace {
                    first_invocation: first.invocation.clone(),
                    first_trace: first_tensor
                        .iter()
                        .map(|location| (location.block, location.operation))
                        .collect(),
                    second_invocation: trace.invocation.clone(),
                    second_trace: tensor
                        .iter()
                        .map(|location| (location.block, location.operation))
                        .collect(),
                });
            }
        }
    }
    None
}
