#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TypedRootFactV1 {
    scalar: SemanticTypedScalarV1,
    contract: SemanticNumericalContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveContractKindV1 {
    Fold,
    Recurrence,
    Permutation,
}

#[derive(Clone, Copy, Debug)]
struct CollectiveContractV1 {
    kind: CollectiveContractKindV1,
    block: usize,
    operation: usize,
    view: pliron::value::Value,
    actual: pliron::value::Value,
    expected: pliron::value::Value,
    coverage: Option<SemanticCoverageBindingAttr>,
    numerical_policy: Option<SemanticNumericalPolicyAttr>,
    witness0: pliron::value::Value,
    witness1: pliron::value::Value,
}

#[cfg(test)]
pub(crate) fn run_pliron_semantic_refinement_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironSemanticRefinementReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(PlironSemanticRefinementFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_semantic_refinement_check_after_bounds_v1(context, function, &mut analyses)
}

#[cfg(test)]
pub(crate) fn run_pliron_semantic_refinement_check_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironSemanticRefinementReportV1 {
    let progress = run_pliron_progress_check_v1(context, function);
    run_pliron_semantic_refinement_after_progress_v1(context, function, analyses, progress)
}

#[cfg(test)]
fn run_pliron_semantic_refinement_after_progress_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
) -> PlironSemanticRefinementReportV1 {
    run_pliron_semantic_refinement_after_progress_with_observation_v1(
        context, function, analyses, progress, None,
    )
}

fn run_pliron_semantic_refinement_after_progress_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    observer: SemanticObserverV1<'_, '_, '_>,
) -> PlironSemanticRefinementReportV1 {
    run_semantic_core_v1(
        context,
        function,
        analyses,
        progress,
        OrdinarySemanticModeV1,
        observer,
    )
    .into_ordinary()
}

fn run_semantic_core_v1<M: SemanticModeV1>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    mode: M,
    observer: SemanticObserverV1<'_, '_, '_>,
) -> SemanticBodyV1<M::Effect> {
    match observer {
        None => run_semantic_inner_v1(context, function, analyses, progress, mode, None),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            run_semantic_inner_v1(context, function, analyses, progress, mode, Some(nested))
        }),
    }
}

fn run_semantic_inner_v1<M: SemanticModeV1>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    progress: PlironProgressReportV1,
    mode: M,
    observer: SemanticObserverV1<'_, '_, '_>,
) -> SemanticBodyV1<M::Effect> {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            if let Some(observer) = observer {
                observer.deny(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                    resource: failure.resource(),
                });
            }
            return semantic_early_v1(
                vec![PlironSemanticRefinementFindingV1::ResourceLimitExceeded],
                progress,
            );
        }
    };
    let mut definitions = Vec::new();
    let mut requirements = Vec::new();
    let mut reference_requirements = Vec::new();
    let mut numerical_requirements = Vec::new();
    let mut tensor_requirements = Vec::new();
    let mut tensor_components = HashMap::new();
    let mut effect_requirement_ids = HashSet::new();
    let mut obligations = Vec::new();
    let mut evidence = Vec::new();
    let mut ownership_contracts = Vec::new();
    let mut collective_contracts = Vec::new();
    for site in inventory.operations() {
        let block_index = site.block();
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if is_semantic_refinement_definition_v1(&*operation) {
            definitions.push(operation.get_operation());
        } else if let Some(component) = operation.downcast_ref::<TensorResultComponentOp>() {
            tensor_components.insert(
                component.result(context),
                (
                    component.result_root(context),
                    component.component(context),
                    component.scalar(context),
                ),
            );
        } else if let Some(requirement) = operation.downcast_ref::<RequireEquivalentOp>() {
            requirements.push((
                block_index,
                operation_index,
                requirement.actual(context),
                requirement.expected(context),
            ));
        } else if let Some(requirement) = operation.downcast_ref::<RequireRefinementOp>() {
            reference_requirements.push((
                block_index,
                operation_index,
                requirement.obligation_id(context),
                requirement.actual(context),
                requirement.expected(context),
            ));
        } else if let Some(requirement) = operation.downcast_ref::<RequireTensorRefinementOp>() {
            tensor_requirements.push((
                block_index,
                operation_index,
                requirement.obligation_id(context),
                requirement.result_root(context),
                requirement.view(context),
                requirement.actual(context),
                requirement.reference(context),
                requirement.components(context),
            ));
        } else if let Some(requirement) = operation.downcast_ref::<RequireEffectRefinementOp>() {
            effect_requirement_ids.insert(requirement.obligation_id(context).unwrap_or([0; 4]));
        } else if let Some(requirement) = operation.downcast_ref::<RequireNumericalRefinementOp>() {
            numerical_requirements.push((
                block_index,
                operation_index,
                requirement.obligation_id(context),
                operation.get_operation().deref(context).get_operand(0),
                operation.get_operation().deref(context).get_operand(1),
                operation.get_operation().deref(context).get_operand(2),
                operation.get_operation().deref(context).get_operand(3),
                requirement.absolute_error_f64_bits(context),
                requirement.relative_error_f64_bits(context),
            ));
        } else if let Some(obligation) = operation.downcast_ref::<ObligationOp>() {
            obligations.push((
                block_index,
                operation_index,
                obligation.obligation_id(context),
                obligation.subject_id(context),
                obligation.model_id(context),
                obligation.property(context),
            ));
        } else if let Some(record) = operation.downcast_ref::<EvidenceRefOp>() {
            evidence.push((
                block_index,
                operation_index,
                record.evidence_id(context),
                record.obligation_id(context),
                record.property(context),
                record.status(context),
                record.covered_boundary(context),
            ));
        } else if let Some(ownership) = operation.downcast_ref::<OwnershipContractOp>() {
            ownership_contracts.push((ownership.view(context), ownership.coverage(context)));
        } else if let Some(contract) = operation.downcast_ref::<RequireFiniteFoldOp>() {
            collective_contracts.push(CollectiveContractV1 {
                kind: CollectiveContractKindV1::Fold,
                block: block_index,
                operation: operation_index,
                view: contract.view(context),
                actual: contract.actual(context),
                expected: contract.expected(context),
                coverage: contract.coverage(context),
                numerical_policy: contract.numerical_policy(context),
                witness0: contract.identity(context),
                witness1: contract.operator(context),
            });
        } else if let Some(contract) = operation.downcast_ref::<RequireFiniteRecurrenceOp>() {
            collective_contracts.push(CollectiveContractV1 {
                kind: CollectiveContractKindV1::Recurrence,
                block: block_index,
                operation: operation_index,
                view: contract.view(context),
                actual: contract.actual(context),
                expected: contract.expected(context),
                coverage: contract.coverage(context),
                numerical_policy: contract.numerical_policy(context),
                witness0: contract.initial(context),
                witness1: contract.transition(context),
            });
        } else if let Some(contract) = operation.downcast_ref::<RequirePermutationGatherOp>() {
            collective_contracts.push(CollectiveContractV1 {
                kind: CollectiveContractKindV1::Permutation,
                block: block_index,
                operation: operation_index,
                view: contract.view(context),
                actual: contract.actual(context),
                expected: contract.expected(context),
                coverage: contract.coverage(context),
                numerical_policy: contract.numerical_policy(context),
                witness0: contract.mapping(context),
                witness1: contract.inverse(context),
            });
        }
    }
    if definitions.is_empty()
        && requirements.is_empty()
        && reference_requirements.is_empty()
        && numerical_requirements.is_empty()
        && tensor_requirements.is_empty()
        && tensor_components.is_empty()
        && effect_requirement_ids.is_empty()
        && obligations.is_empty()
        && evidence.is_empty()
        && ownership_contracts.is_empty()
        && collective_contracts.is_empty()
    {
        // Progress and effect refinement are nested report owners and remain
        // mandatory. The semantic-local path has no expression, contract, or
        // finding payload to allocate after the authenticated inventory scan.
        let effect_refinement = mode.effect(
            SemanticExecutionV1 {
                context,
                function,
                analyses,
            },
            observer,
        );
        return SemanticBodyV1 {
            findings: Vec::new(),
            reference_obligations: 0,
            policy_checked_reference_obligations: 0,
            numerical_obligations: 0,
            policy_checked_numerical_obligations: 0,
            collective_contracts: 0,
            policy_checked_collective_contracts: 0,
            typed_root_commitments: Vec::new(),
            numerical_certificates: Vec::new(),
            progress,
            effect_refinement: EffectRunV1::Executed(effect_refinement),
        };
    }
    if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
        expression_quota_v1(
            observer.map(|observer| {
                (
                    ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                    observer,
                )
            }),
            "semantic expression definition limit",
        );
        return semantic_early_v1(
            vec![PlironSemanticRefinementFindingV1::ResourceLimitExceeded],
            progress,
        );
    }

    let reference_count = reference_requirements.len() + tensor_requirements.len();
    let numerical_count = numerical_requirements.len();
    let policy_checked_requirements = reference_requirements
        .iter()
        .map(|&(block, operation, identity, actual, expected)| {
            (block, operation, identity, actual, expected)
        })
        .chain(numerical_requirements.iter().map(
            |&(block, operation, identity, actual, reference, ..)| {
                (block, operation, identity, actual, reference)
            },
        ))
        .chain(tensor_requirements.iter().map(
            |&(block, operation, identity, _, _, actual, reference, _)| {
                (block, operation, identity, actual, reference)
            },
        ))
        .collect::<Vec<_>>();
    let mut findings = Vec::new();
    let mut contract_valid = HashSet::new();
    let mut used_obligations = HashSet::new();
    for (block, operation, identity, _, _) in &policy_checked_requirements {
        let identity = identity.unwrap_or([0; 4]);
        let matching = obligations
            .iter()
            .filter(|record| record.2 == Some(identity))
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            push(
                &mut findings,
                if matching.is_empty() {
                    PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the exact proof.obligation record is missing",
                    }
                } else {
                    PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the proof obligation identity is duplicated",
                    }
                },
            );
            continue;
        }
        let obligation = matching[0];
        used_obligations.insert(identity);
        if obligation.3.is_none() || obligation.4.is_none() {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the obligation lacks an exact subject or reference-model identity",
                },
            );
            continue;
        }
        if obligation.5 != Some(PropertyAttr::FunctionalRefinement) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the referenced obligation is not FunctionalRefinement",
                },
            );
            continue;
        }
        let matching_evidence = evidence
            .iter()
            .filter(|record| record.3 == Some(identity))
            .collect::<Vec<_>>();
        if matching_evidence.len() != 1 {
            push(
                &mut findings,
                if matching_evidence.is_empty() {
                    PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the exact proof.evidence_ref record is missing",
                    }
                } else {
                    PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "more than one evidence record claims the obligation",
                    }
                },
            );
            continue;
        }
        let record = matching_evidence[0];
        if record.2.is_none() || record.4 != Some(PropertyAttr::FunctionalRefinement) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the evidence identity or property does not match functional refinement",
                },
            );
            continue;
        }
        if record.5 != Some(EvidenceStatusAttr::Checked) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "policy-checked staging requires exact Checked evidence",
                },
            );
            continue;
        }
        if record.6 != Some(CoveredBoundaryAttr::Mir) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the Verus reference evidence must cover the exact MIR boundary",
                },
            );
            continue;
        }
        contract_valid.insert((*block, *operation));
    }
    for (block, operation, identity, _, _, property) in &obligations {
        if *property == Some(PropertyAttr::FunctionalRefinement)
            && identity.is_none_or(|identity| {
                !used_obligations.contains(&identity) && !effect_requirement_ids.contains(&identity)
            })
        {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity.unwrap_or([0; 4]),
                    reason: "functional-refinement proof obligation has no semantic equality",
                },
            );
        }
    }

    let expressions = match SemanticExpressionTableV1::build_with_observation_v1(
        context,
        &definitions,
        observer.map(|observer| {
            (
                ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                observer,
            )
        }),
    ) {
        Ok(expressions) => expressions,
        Err(SemanticExpressionBuildErrorV1::ResourceLimit) => {
            return semantic_early_v1(
                vec![PlironSemanticRefinementFindingV1::ResourceLimitExceeded],
                progress,
            );
        }
        Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(reason)) => {
            return semantic_early_v1(
                vec![PlironSemanticRefinementFindingV1::TypedExpressionRejected { reason }],
                progress,
            );
        }
    };

    for (block, operation, actual, expected) in requirements {
        let Some(actual_node) = expressions.identity(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: actual.unique_name(context).to_string(),
                },
            );
            continue;
        };
        let Some(expected_node) = expressions.identity(expected) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: expected.unique_name(context).to_string(),
                },
            );
            continue;
        };
        if actual_node != expected_node {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ExpressionMismatch {
                    block,
                    operation,
                    actual: expressions.describe(actual_node),
                    expected: expressions.describe(expected_node),
                },
            );
        }
    }
    let mut policy_checked_reference_obligations = 0;
    let mut proved_reference_pairs = HashSet::new();
    analyses.prepare_tensor_layout_dataflow(context, function);
    let tensor_layout_dataflow = analyses.tensor_layout_dataflow().ok();
    for (block, operation, _, result_root, _, actual, reference, components) in tensor_requirements
    {
        let actual_fact = expressions.typed_root_fact(actual);
        let reference_fact = expressions.typed_root_fact(reference);
        let valid_aggregate = actual_fact
            .zip(reference_fact)
            .is_some_and(|(actual, reference)| actual == reference);
        let root_component_count = result_root.map_or(0, |root| {
            tensor_components
                .values()
                .filter(|(candidate, ..)| *candidate == Some(root))
                .count()
        });
        let mut seen = HashSet::new();
        let propagated_layout = result_root
            .and_then(|root| tensor_layout_dataflow.and_then(|dataflow| dataflow.fact(root)));
        let valid_layout = propagated_layout.is_some_and(|fact| {
            fact.layout.role == TensorOperandRoleV1::Accumulator
                && usize::from(fact.layout.fragment_elements) == components.len()
                && tensor_element_scalar(fact.layout.element)
                    .zip(actual_fact.map(|actual| actual.scalar))
                    .is_some_and(|(layout, actual)| layout == actual)
        });
        let valid_components = result_root.is_some()
            && !components.is_empty()
            && root_component_count == components.len()
            && components
                .iter()
                .enumerate()
                .all(|(ordinal, (gpu, sequential))| {
                    seen.insert(*gpu)
                        && tensor_components.get(gpu).is_some_and(
                            |(component_root, component_ordinal, scalar)| {
                                *component_root == result_root
                                    && *component_ordinal == u32::try_from(ordinal).ok()
                                    && actual_fact.is_some_and(|fact| *scalar == Some(fact.scalar))
                            },
                        )
                        && reference_fact
                            .zip(expressions.typed_root_fact(*sequential))
                            .is_some_and(|(aggregate, component)| aggregate == component)
                });
        if !valid_layout {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "tensor refinement result is not the exact propagated accumulator layout and scalar contract named by the authenticated tensor site",
                },
            );
        } else if !valid_aggregate || !valid_components {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "tensor refinement lacks an exact result-root/component SSA mapping or compatible typed aggregate roots",
                },
            );
        } else if contract_valid.contains(&(block, operation)) {
            policy_checked_reference_obligations += 1;
            proved_reference_pairs.insert((actual, reference));
        }
    }
    for (block, operation, _, actual, expected) in reference_requirements {
        let Some(actual_node) = expressions.identity(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: actual.unique_name(context).to_string(),
                },
            );
            continue;
        };
        let Some(expected_node) = expressions.identity(expected) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: expected.unique_name(context).to_string(),
                },
            );
            continue;
        };
        if actual_node != expected_node {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ExpressionMismatch {
                    block,
                    operation,
                    actual: expressions.describe(actual_node),
                    expected: expressions.describe(expected_node),
                },
            );
        } else if contract_valid.contains(&(block, operation)) {
            policy_checked_reference_obligations += 1;
            proved_reference_pairs.insert((actual, expected));
        }
    }
    let mut policy_checked_numerical_obligations = 0;
    let mut numerical_certificates = Vec::new();
    for (
        block,
        operation,
        _,
        actual,
        reference,
        domain,
        precondition,
        absolute_error,
        relative_error,
    ) in numerical_requirements
    {
        let Some(actual_fact) = expressions.typed_root_fact(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "the numerical actual value is not a reconstructed typed root",
                },
            );
            continue;
        };
        let Some(reference_fact) = expressions.typed_root_fact(reference) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "the numerical reference value is not a reconstructed typed root",
                },
            );
            continue;
        };
        let domain_fact = expressions.typed_root_fact(domain);
        let precondition_fact = expressions.typed_root_fact(precondition);
        if actual_fact != reference_fact || !actual_fact.scalar.is_float() {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical actual and reference roots must share one floating scalar contract",
                },
            );
            continue;
        }
        let actual_identity = expressions.identity(actual);
        let reference_identity = expressions.identity(reference);
        if actual_identity != reference_identity {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::NumericalProofIncomplete {
                    block,
                    operation,
                    actual: expressions
                        .describe_value(actual)
                        .unwrap_or_else(|| actual.unique_name(context).to_string()),
                    reference: expressions
                        .describe_value(reference)
                        .unwrap_or_else(|| reference.unique_name(context).to_string()),
                    reason: "V1 derives a finite bound only from identical typed IEEE operator trees",
                },
            );
            continue;
        }
        if domain_fact.is_none_or(|fact| !fact.scalar.is_bool())
            || precondition_fact.is_none_or(|fact| !fact.scalar.is_bool())
        {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical domain and precondition must be typed Boolean roots",
                },
            );
            continue;
        }
        let bounds = absolute_error
            .zip(relative_error)
            .map(|(absolute, relative)| (f64::from_bits(absolute), f64::from_bits(relative)));
        if bounds.is_none_or(|(absolute, relative)| {
            !absolute.is_finite()
                || !relative.is_finite()
                || absolute < 0.0
                || relative < 0.0
                || (absolute == 0.0 && relative == 0.0)
        }) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical refinement requires finite nonnegative nonzero bounds",
                },
            );
            continue;
        }
        if contract_valid.contains(&(block, operation)) {
            policy_checked_numerical_obligations += 1;
            numerical_certificates.push(PlironNumericalBoundCertificateV1 {
                block,
                operation,
                requested_absolute_error_f64_bits: absolute_error.expect("validated bound"),
                requested_relative_error_f64_bits: relative_error.expect("validated bound"),
            });
        }
    }
    let collective_count = collective_contracts.len();
    let mut policy_checked_collective_contracts = 0;
    let mut used_reference_pairs = HashSet::new();
    for collective in collective_contracts {
        let block = collective.block;
        let operation = collective.operation;
        let view = collective.view;
        let actual = collective.actual;
        let expected = collective.expected;
        let coverage = collective.coverage;
        let Some(actual_fact) = expressions.typed_root_fact(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the actual value is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(expected_fact) = expressions.typed_root_fact(expected) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the expected value is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(witness0_fact) = expressions.typed_root_fact(collective.witness0) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the first collective witness is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(witness1_fact) = expressions.typed_root_fact(collective.witness1) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the second collective witness is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(numerical_policy) = collective.numerical_policy else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the declared collective numerical policy is absent",
                },
            );
            continue;
        };
        if actual_fact != expected_fact {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "actual and expected roots do not share one scalar and numerical contract",
                },
            );
            continue;
        }
        if actual_fact.contract.policy != numerical_policy {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the declared collective policy does not match the actual and expected typed roots",
                },
            );
            continue;
        }
        match collective.kind {
            CollectiveContractKindV1::Fold | CollectiveContractKindV1::Recurrence => {
                if witness0_fact != actual_fact || witness1_fact != actual_fact {
                    push(
                        &mut findings,
                        PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                            block,
                            operation,
                            reason: "a fold or recurrence witness scalar or numerical contract does not match its result",
                        },
                    );
                    continue;
                }
            }
            CollectiveContractKindV1::Permutation => {
                if witness0_fact != witness1_fact
                    || !witness0_fact.scalar.is_integer()
                    || witness0_fact.contract.policy
                        != SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
                {
                    push(
                        &mut findings,
                        PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                            block,
                            operation,
                            reason: "permutation mapping and inverse must share one integer bitvector contract",
                        },
                    );
                    continue;
                }
            }
        }
        let required_coverage = match coverage {
            Some(SemanticCoverageBindingAttr::TotalView) => OwnershipCoverageAttr::TotalView,
            Some(SemanticCoverageBindingAttr::CollectiveContributions) => {
                OwnershipCoverageAttr::CollectiveContributions
            }
            None => {
                push(
                    &mut findings,
                    PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                        block,
                        operation,
                        reason: "the coverage binding is absent",
                    },
                );
                continue;
            }
        };
        if mode.selected_view(view) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "conditional selected coverage is not an ordinary collective ownership theorem",
                },
            );
            continue;
        }
        let matching_coverage = ownership_contracts
            .iter()
            .filter(|(candidate_view, _)| *candidate_view == view)
            .collect::<Vec<_>>();
        if matching_coverage.len() != 1 {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "exactly one independently verified ownership contract is required for the output view",
                },
            );
            continue;
        }
        if matching_coverage[0].1 != Some(required_coverage) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the output ownership theorem does not match the declared coverage binding",
                },
            );
            continue;
        }
        if !proved_reference_pairs.contains(&(actual, expected)) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "coverage never proves a final value; an independently proved MIR functional-refinement equality is required",
                },
            );
            continue;
        }
        if !used_reference_pairs.insert((actual, expected)) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "one scalar proof equality cannot discharge multiple finite collective contracts",
                },
            );
            continue;
        }
        policy_checked_collective_contracts += 1;
    }
    let effect_refinement = mode.effect(
        SemanticExecutionV1 {
            context,
            function,
            analyses,
        },
        observer,
    );
    let typed_root_commitments = expressions.typed_root_commitments().to_vec();
    SemanticBodyV1 {
        findings,
        reference_obligations: reference_count,
        policy_checked_reference_obligations,
        numerical_obligations: numerical_count,
        policy_checked_numerical_obligations,
        collective_contracts: collective_count,
        policy_checked_collective_contracts,
        typed_root_commitments,
        numerical_certificates,
        progress,
        effect_refinement: EffectRunV1::Executed(effect_refinement),
    }
}

fn tensor_element_scalar(element: MatrixElement) -> Option<SemanticTypedScalarV1> {
    match element {
        MatrixElement::F32 => {
            SemanticTypedScalarV1::new(dialect_kernel::SemanticScalarKindAttr::Float, 32)
        }
        // BF16 is currently an operand storage scalar, not a semantic result
        // scalar in the typed-reference dialect.
        MatrixElement::Bf16 | MatrixElement::Fp4E2M1 | MatrixElement::Fp8E4M3 => None,
    }
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_pliron_semantic_refinement_with_scoped_input_v1(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<'_>,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<
    Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    require_pliron_semantic_refinement_with_scoped_observation_v1(input, analyses, None)
}

#[allow(clippy::result_large_err)]
pub(crate) fn require_pliron_semantic_refinement_with_scoped_observation_v1(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<'_>,
    analyses: &mut PlironAnalysisManagerV1,
    observer: SemanticObserverV1<'_, '_, '_>,
) -> Result<
    Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let run = || {
        let scoped =
            crate::production_analysis::pliron_progress::run_pliron_progress_with_scoped_observation_v1(
                input, observer,
            )?;
        let report = run_pliron_semantic_refinement_after_progress_with_observation_v1(
            scoped.context,
            scoped.function,
            analyses,
            scoped.report,
            observer,
        );
        Ok(if report.is_clean() {
            Ok(report)
        } else {
            Err(PlironSemanticRefinementCheckErrorV1 { report })
        })
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
pub(crate) fn require_pliron_semantic_refinement_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1> {
    let report = run_pliron_semantic_refinement_check_after_bounds_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironSemanticRefinementCheckErrorV1 { report })
    }
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_pliron_semantic_refinement_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1> {
    let report = run_pliron_semantic_refinement_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironSemanticRefinementCheckErrorV1 { report })
    }
}

fn push(
    findings: &mut Vec<PlironSemanticRefinementFindingV1>,
    finding: PlironSemanticRefinementFindingV1,
) {
    if findings.len() < MAX_PLIRON_SEMANTIC_FINDINGS_V1 {
        findings.push(finding);
    } else if !matches!(
        findings.last(),
        Some(PlironSemanticRefinementFindingV1::ResourceLimitExceeded)
    ) {
        findings.push(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
    }
}

#[cfg(test)]
fn one(finding: PlironSemanticRefinementFindingV1) -> PlironSemanticRefinementReportV1 {
    PlironSemanticRefinementReportV1 {
        findings: vec![finding],
        reference_obligations: 0,
        policy_checked_reference_obligations: 0,
        numerical_obligations: 0,
        policy_checked_numerical_obligations: 0,
        collective_contracts: 0,
        policy_checked_collective_contracts: 0,
        typed_root_commitments: Vec::new(),
        numerical_certificates: Vec::new(),
        progress: PlironProgressReportV1::clean(),
        effect_refinement: clean_effect_refinement_report_v1(),
    }
}

fn proof_identity(words: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        words[0], words[1], words[2], words[3]
    )
}
