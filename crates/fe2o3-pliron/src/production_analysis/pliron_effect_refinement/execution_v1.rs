type Manager = PlironAnalysisManagerV1;
type Census = ProductionAnalysisInputCensusV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Bound = ProductionAnalysisResourceUpperBoundV1;
type Phase = ProductionAnalysisResourcePhaseV1;
type EffectObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;
type Inventory =
    crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;

#[derive(Debug, Eq, PartialEq)]
struct EffectBodyV1 {
    findings: Vec<PlironEffectRefinementFindingV1>,
    contracts: usize,
    proved_contracts: usize,
}

// Only this core constructs these private borrows. Modes are not raw-arena
// analysis entry points, and cannot be implemented by external callers.
struct EffectExecutionV1<'a> {
    context: &'a Context,
    function: &'a FuncOp,
    analyses: &'a mut Manager,
}

struct EffectWriteV1<'a> {
    context: &'a Context,
    inventory: &'a Inventory,
    contract: &'a EffectContractV1,
    location: EffectRefinementLocationV1,
}

trait EffectModeV1 {
    fn no_contracts(&mut self) {}
    fn ownership(
        &mut self,
        execution: EffectExecutionV1<'_>,
        contracts: &[EffectContractV1],
        findings: &mut Vec<PlironEffectRefinementFindingV1>,
        observer: EffectObserverV1<'_, '_, '_>,
    ) -> bool;
    fn credit(&mut self, _: EffectWriteV1<'_>, _: EffectObserverV1<'_, '_, '_>) -> bool {
        true
    }
}

struct OrdinaryEffectModeV1;

impl EffectModeV1 for OrdinaryEffectModeV1 {
    fn ownership(
        &mut self,
        execution: EffectExecutionV1<'_>,
        contracts: &[EffectContractV1],
        findings: &mut Vec<PlironEffectRefinementFindingV1>,
        observer: EffectObserverV1<'_, '_, '_>,
    ) -> bool {
        let EffectExecutionV1 {
            context,
            function,
            analyses,
        } = execution;
        let hierarchy = run_pliron_hierarchical_ownership_with_observation_v1(
            context, function, analyses, observer,
        );
        if hierarchy.is_clean() {
            return true;
        }
        let finding = hierarchy
            .findings()
            .first()
            .expect("non-clean hierarchy has finding");
        let dynamic = contracts.iter().find_map(|contract| {
            ranked_view_type(contract.view, context).and_then(|view| {
                view.deref(context)
                    .shape()
                    .iter()
                    .position(|extent| *extent == DYNAMIC_EXTENT)
                    .map(|dimension| (contract.view_name.clone(), dimension))
            })
        });
        *findings = vec![project_hierarchy_finding_v1(finding, dynamic)];
        false
    }
}

fn run_effect_core_v1<M: EffectModeV1>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    mode: &mut M,
    observer: EffectObserverV1<'_, '_, '_>,
) -> EffectBodyV1 {
    match observer {
        None => run_effect_inner_v1(context, function, analyses, mode, None),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            run_effect_inner_v1(context, function, analyses, mode, Some(nested))
        }),
    }
}

fn run_effect_inner_v1<M: EffectModeV1>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    mode: &mut M,
    observer: EffectObserverV1<'_, '_, '_>,
) -> EffectBodyV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            if let Some(observer) = observer {
                observer.deny(Limit {
                    phase: Phase::EffectRefinement,
                    resource: failure.resource(),
                });
            }
            return one(
                0,
                PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                    actual: failure.actual(),
                    limit: failure.limit(),
                },
            );
        }
    };
    if !inventory.operations().iter().any(|site| {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        is_effect_refinement_contract_v1(&*operation)
    }) {
        mode.no_contracts();
        return report(0, 0, Vec::new());
    }
    let (contracts, writes, ownership_views, obligations, evidence) = collect(context, &inventory);
    debug_assert!(!contracts.is_empty());
    if contracts.len() > MAX_EFFECT_REFINEMENT_CONTRACTS_V1 {
        if let Some(observer) = observer {
            observer.deny(Limit {
                phase: Phase::EffectRefinement,
                resource: "effect refinement contract limit",
            });
        }
        return one(
            contracts.len(),
            PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                actual: contracts.len(),
                limit: MAX_EFFECT_REFINEMENT_CONTRACTS_V1,
            },
        );
    }
    let mut findings = Vec::new();
    for contract in &contracts {
        if !ownership_views.contains(&contract.view) {
            findings.push(PlironEffectRefinementFindingV1::MissingOwnershipContract {
                view: contract.view_name.clone(),
                location: contract.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    if !mode.ownership(
        EffectExecutionV1 {
            context,
            function,
            analyses,
        },
        &contracts,
        &mut findings,
        observer,
    ) {
        return report(contracts.len(), 0, findings);
    }

    let mut writes_by_signature =
        HashMap::<(usize, Value, Vec<Value>), Vec<EffectRefinementLocationV1>>::new();
    for write in &writes {
        writes_by_signature
            .entry((write.location.block, write.view, write.indices.clone()))
            .or_default()
            .push(write.location);
    }
    let mut by_write = HashMap::<EffectRefinementLocationV1, usize>::new();
    let mut write_by_contract = vec![None; contracts.len()];
    for (contract_index, contract) in contracts.iter().enumerate() {
        let signature = (
            contract.location.block,
            contract.view,
            contract.indices.clone(),
        );
        let matching = writes_by_signature
            .get(&signature)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if matching.is_empty() {
            findings.push(PlironEffectRefinementFindingV1::OrphanEffectContract {
                view: contract.view_name.clone(),
                location: contract.location,
            });
            continue;
        }
        if matching.len() != 1 {
            findings.push(PlironEffectRefinementFindingV1::AmbiguousWriteSite {
                view: contract.view_name.clone(),
                location: contract.location,
                matches: matching.len(),
            });
            continue;
        }
        let write = matching[0];
        write_by_contract[contract_index] = Some(write);
        if let Some(first_index) = by_write.insert(write, contract_index) {
            findings.push(PlironEffectRefinementFindingV1::DuplicateEffectContract {
                view: contract.view_name.clone(),
                write,
                first: contracts[first_index].location,
                second: contract.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    for write in &writes {
        if !by_write.contains_key(&write.location) {
            findings.push(PlironEffectRefinementFindingV1::UnmodeledWriteSite {
                view: bounded_effect_diagnostic_v1(write.view.unique_name(context)),
                location: write.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    let expressions = match SemanticExpressionTableV1::from_inventory_with_observation_v1(
        context,
        &inventory,
        observer.map(|observer| (Phase::EffectRefinement, observer)),
    ) {
        Ok(expressions) => expressions,
        Err(_) => {
            return one(
                contracts.len(),
                PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                    actual: contracts.len(),
                    limit: MAX_EFFECT_REFINEMENT_CONTRACTS_V1,
                },
            );
        }
    };
    let mut proved = 0;
    for (index, contract) in contracts.iter().enumerate() {
        if !validate_proof(contract, &obligations, &evidence, &mut findings) {
            continue;
        }
        let write = write_by_contract[index].expect("correlated contract has write");
        let witness = None;
        let mut pairs = contract
            .coordinates
            .iter()
            .map(|(actual, expected)| ("coordinate", *actual, *expected))
            .collect::<Vec<_>>();
        pairs.extend([
            ("domain", contract.expressions[0], contract.expressions[1]),
            (
                "precondition",
                contract.expressions[2],
                contract.expressions[3],
            ),
            ("value", contract.expressions[4], contract.expressions[5]),
        ]);
        let mut valid = true;
        for (component, actual, expected) in pairs {
            let Some(actual_description) = expressions.describe_value(actual) else {
                findings.push(PlironEffectRefinementFindingV1::UnresolvedExpression {
                    view: contract.view_name.clone(),
                    location: contract.location,
                    component,
                    value: bounded_effect_diagnostic_v1(actual.unique_name(context)),
                });
                valid = false;
                continue;
            };
            let Some(expected_description) = expressions.describe_value(expected) else {
                findings.push(PlironEffectRefinementFindingV1::UnresolvedExpression {
                    view: contract.view_name.clone(),
                    location: contract.location,
                    component,
                    value: bounded_effect_diagnostic_v1(expected.unique_name(context)),
                });
                valid = false;
                continue;
            };
            if expressions.equivalent(actual, expected) != Some(true) {
                let finding = match component {
                    "domain" => PlironEffectRefinementFindingV1::DomainMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: bounded_effect_owned_diagnostic_v1(actual_description),
                        expected: bounded_effect_owned_diagnostic_v1(expected_description),
                        witness: witness.clone(),
                    },
                    "precondition" => PlironEffectRefinementFindingV1::PreconditionMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: bounded_effect_owned_diagnostic_v1(actual_description),
                        expected: bounded_effect_owned_diagnostic_v1(expected_description),
                        witness: witness.clone(),
                    },
                    _ => PlironEffectRefinementFindingV1::ValueMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: bounded_effect_owned_diagnostic_v1(actual_description),
                        expected: bounded_effect_owned_diagnostic_v1(expected_description),
                        witness: witness.clone(),
                    },
                };
                findings.push(finding);
                valid = false;
            }
        }
        if valid
            && mode.credit(
                EffectWriteV1 {
                    context,
                    inventory: &inventory,
                    contract,
                    location: write,
                },
                observer,
            )
        {
            proved += 1;
        }
    }
    report(contracts.len(), proved, findings)
}
