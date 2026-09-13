#[derive(Clone)]
struct ContractV1 {
    location: HierarchicalOwnershipLocationV1,
    view: Value,
    view_name: String,
    view_op: RankedViewOp,
    coverage: OwnershipCoverageAttr,
    partition: OwnershipPartitionAttr,
}

#[cfg(test)]
pub(crate) fn run_pliron_hierarchical_ownership_check_v1(
    context: &Context,
    function: &FuncOp,
) -> HierarchicalOwnershipReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_hierarchical_ownership_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> HierarchicalOwnershipReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            return one(
                HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                    detail: format!(
                        "bounded function inventory {} count {} exceeds limit {}",
                        failure.resource(),
                        failure.actual(),
                        failure.limit(),
                    ),
                },
            );
        }
    };
    let contracts = match collect_contracts(context, &inventory) {
        Ok(contracts) if contracts.is_empty() => return clean(),
        Ok(contracts) => contracts,
        Err(finding) => return one(*finding),
    };
    let mut coverage_summary = declared_coverage_summary(&contracts);

    analyses.prepare_execution_layout(context, function);
    let layout = match analyses.execution_layout() {
        Ok(Some(layout)) => layout,
        Ok(None) => {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::ExecutionLayoutIncomplete {
                    detail: "kernel.ownership_contract requires gpu.execution_layout".to_owned(),
                },
                coverage_summary,
            );
        }
        Err(failure) => {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::ExecutionLayoutIncomplete {
                    detail: trace_failure_detail(failure),
                },
                coverage_summary,
            );
        }
    };
    analyses.prepare_sparse_indices(context, function);
    if let Err(failure) = analyses.sparse_indices() {
        return one_with_summary(
            HierarchicalOwnershipFindingV1::SparseIndexAnalysisIncomplete {
                detail: format!("{failure:?}"),
            },
            coverage_summary,
        );
    }
    let needs_effect_domain = contracts
        .iter()
        .any(|contract| contract.coverage == OwnershipCoverageAttr::ExactEffectDomain);
    let needs_mandatory_bounds = contracts.iter().any(|contract| {
        matches!(
            contract.coverage,
            OwnershipCoverageAttr::ExactEffectDomain
                | OwnershipCoverageAttr::TotalView
                | OwnershipCoverageAttr::CollectiveContributions
        )
    });
    let mut mandatory_bounds_failure = None;
    if needs_mandatory_bounds {
        let bounds = run_pliron_ranked_bounds_check_with_analyses_v1(context, function, analyses);
        if !bounds.is_clean() {
            let detail = bounded_nested_findings_detail_v1(
                "mandatory ranked bounds failed: ",
                bounds.findings(),
            );
            if needs_effect_domain {
                return one_with_summary(
                    HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail },
                    coverage_summary,
                );
            }
            mandatory_bounds_failure = Some(detail);
        }
    }
    if needs_effect_domain {
        let race = run_pliron_ranked_race_check_with_analyses_v1(context, function, analyses);
        if !race.is_clean() {
            return one_with_summary(
                HierarchicalOwnershipFindingV1::EffectDomainIncomplete {
                    detail: bounded_nested_findings_detail_v1("", race.findings()),
                },
                coverage_summary,
            );
        }
    }
    let needs_total_output = contracts
        .iter()
        .any(|contract| contract.coverage == OwnershipCoverageAttr::TotalView);
    if needs_total_output
        && let Some(finding) =
            first_unmodeled_or_aliasing_observable_write(context, &inventory, &contracts)
    {
        return one_with_summary(finding, coverage_summary);
    }
    let needs_exact_trace = contracts.iter().any(|contract| {
        matches!(
            contract.coverage,
            OwnershipCoverageAttr::ExactView
                | OwnershipCoverageAttr::TotalView
                | OwnershipCoverageAttr::CollectiveContributions
        )
    });
    if needs_exact_trace {
        analyses.prepare_exact_trace(context, function);
    }
    let traces = if needs_exact_trace {
        match analyses.exact_trace() {
            Ok(traces) => Some(traces),
            Err(failure) => {
                return one_with_summary(
                    HierarchicalOwnershipFindingV1::TraceIncomplete {
                        detail: trace_failure_detail(failure),
                    },
                    coverage_summary,
                );
            }
        }
    } else {
        None
    };
    let sparse = analyses
        .sparse_indices()
        .expect("sparse analysis was checked before ownership");

    let mut findings = Vec::new();
    let mut regions = Vec::new();
    for contract in contracts {
        let (extents, element_count) = match contract.coverage {
            OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::TotalView => {
                let extents = match resolve_extents(context, sparse, &contract) {
                    Ok(extents) => extents,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                let element_count = match bounded_element_count(&contract.view_name, &extents) {
                    Ok(count) => count,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                (Some(extents), Some(element_count))
            }
            OwnershipCoverageAttr::CollectiveContributions => {
                let extents = match resolve_extents(context, sparse, &contract) {
                    Ok(extents) => extents,
                    Err(finding) => {
                        findings.push(*finding);
                        continue;
                    }
                };
                (Some(extents), None)
            }
            OwnershipCoverageAttr::ExactEffectDomain => {
                if let Err(finding) = validate_effect_domain_site(context, &inventory, &contract) {
                    findings.push(*finding);
                }
                continue;
            }
        };
        let finding_count = findings.len();
        analyze_contract(
            context,
            &contract,
            extents.as_deref(),
            element_count,
            traces.expect("whole-domain contracts prepared exact traces"),
            layout.grid,
            &mut findings,
            &mut regions,
        );
        if findings.len() == finding_count && mandatory_bounds_failure.is_none() {
            match contract.coverage {
                OwnershipCoverageAttr::TotalView => coverage_summary.total_view_proved += 1,
                OwnershipCoverageAttr::CollectiveContributions => {
                    coverage_summary.collective_contributions_proved += 1;
                }
                OwnershipCoverageAttr::ExactView | OwnershipCoverageAttr::ExactEffectDomain => {}
            }
        }
    }
    if findings.is_empty()
        && let Some(detail) = mandatory_bounds_failure
    {
        findings.push(HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail });
    }
    HierarchicalOwnershipReportV1 {
        findings,
        regions,
        coverage_summary,
        conditional_prefix: None,
    }
}

fn first_unmodeled_or_aliasing_observable_write(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    contracts: &[ContractV1],
) -> Option<HierarchicalOwnershipFindingV1> {
    let modeled = contracts
        .iter()
        .map(|contract| contract.view)
        .collect::<HashSet<_>>();
    let total_outputs = contracts
        .iter()
        .filter(|contract| contract.coverage == OwnershipCoverageAttr::TotalView)
        .collect::<Vec<_>>();
    for site in inventory.operations() {
        let block = site.block();
        let operation = site.operation();
        let op = Operation::get_op_dyn(site.pointer(), context);
        if let Some(effect) = op.downcast_ref::<AllocationEffectOp>()
            && effect.memory_space(context) == Some(MemorySpaceAttr::Global)
            && effect
                .kind(context)
                .is_some_and(|kind| kind.writes_memory())
        {
            return Some(
                HierarchicalOwnershipFindingV1::UnmodeledObservableAllocationWrite {
                    allocation_origin: effect.allocation_origin(context).unwrap_or(0),
                    noalias_class: effect.noalias_class(context).unwrap_or(0),
                    location: HierarchicalOwnershipLocationV1 { block, operation },
                },
            );
        }
        let Some(access) = op.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        if !access
            .kind(context)
            .is_some_and(|kind| kind.writes_memory())
        {
            continue;
        }
        let view = access.view(context);
        let Some(view_op) = view
            .defining_op()
            .map(|definition| Operation::get_op_dyn(definition, context))
            .and_then(|definition| definition.downcast_ref::<RankedViewOp>().copied())
        else {
            continue;
        };
        if view_op.memory_space(context) != Some(MemorySpaceAttr::Global) {
            continue;
        }
        let location = HierarchicalOwnershipLocationV1 { block, operation };
        if !modeled.contains(&view) {
            return Some(HierarchicalOwnershipFindingV1::UnmodeledObservableWrite {
                view: view.unique_name(context).to_string(),
                location,
            });
        }
        let alias_noalias_class = view_op.noalias_class(context).unwrap_or(0);
        for contract in &total_outputs {
            if contract.view == view {
                continue;
            }
            let contracted_noalias_class = contract.view_op.noalias_class(context).unwrap_or(0);
            if contracted_noalias_class == 0
                || alias_noalias_class == 0
                || contracted_noalias_class == alias_noalias_class
            {
                return Some(HierarchicalOwnershipFindingV1::MayAliasObservableWrite {
                    contracted_view: contract.view_name.clone(),
                    alias_view: view.unique_name(context).to_string(),
                    contracted_noalias_class,
                    alias_noalias_class,
                    location,
                });
            }
        }
    }
    None
}

pub(crate) fn require_pliron_hierarchical_ownership_with_scoped_input_v1(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedOwnershipInputV1<'_>,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<
    Result<HierarchicalOwnershipReportV1, HierarchicalOwnershipCheckErrorV1>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let (context, function) = input.endpoints()?;
    let mut report =
        run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, analyses);
    if report.status() == KernelCheckStatusV1::Incomplete
        && report.coverage_summary.total_view_declared != 0
    {
        report.conditional_prefix =
            Some(conditional_prefix_v1::derive_with_scoped_input_v1(&input)?);
    }
    Ok(if report.is_clean() {
        Ok(report)
    } else {
        Err(HierarchicalOwnershipCheckErrorV1 { report })
    })
}
