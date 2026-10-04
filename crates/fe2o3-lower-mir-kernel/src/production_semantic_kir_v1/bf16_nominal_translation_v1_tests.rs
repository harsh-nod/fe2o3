//! Inert graph/resource controls, not a genuine rustc nominal-owner qualification.
use super::super::bf16_call_query_tests_v1::synthetic;
use super::super::bf16_nominal_translation_context_v1 as nominal;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 23;

#[test]
fn nominal_translation_trap_declaration_is_exact_and_allocation_free() {
    for mutation in 0..10 {
        let mut declaration = AmdGpuDiagnosticOperation::Trap.declaration();
        match mutation {
            0 => {}
            1 => declaration.id = AmdGpuDiagnosticOperation::DebugTrap.intrinsic_function_id(),
            2 => declaration.role = fe2o3_kernel_ir::FunctionRole::InternalHelper,
            3 => {
                declaration.body = Some(fe2o3_kernel_ir::FunctionBody {
                    parameters: Vec::new(),
                    blocks: Vec::new(),
                })
            }
            4 => declaration
                .signature
                .parameters
                .push(Type::Scalar(ScalarType::U32)),
            5 => declaration
                .signature
                .results
                .push(Type::Scalar(ScalarType::F32)),
            6 => declaration.required_capabilities.clear(),
            7 => {
                declaration.required_capabilities.insert(
                    fe2o3_kernel_ir::TargetCapability::Extension {
                        namespace: "other".into(),
                        name: "other".into(),
                    },
                );
            }
            8 | 9 => {
                declaration.required_capabilities.clear();
                declaration.required_capabilities.insert(
                    fe2o3_kernel_ir::TargetCapability::Extension {
                        namespace: if mutation == 8 {
                            "other"
                        } else {
                            fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                        }
                        .into(),
                        name: if mutation == 9 {
                            "other"
                        } else {
                            fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAME
                        }
                        .into(),
                    },
                );
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: None,
            failed: false,
            resource_error: None,
        };
        assert_eq!(
            nominal::exact_trap_declaration(&declaration, &mut meter).unwrap(),
            mutation == 0
        );
        assert_eq!(meter.budget.storage(), FLOOR);
        assert!(!meter.exhausted());
    }
}

#[test]
fn nominal_translation_roster_retains_two_bodies_and_only_optional_trap() {
    let (_, root, helper) = synthetic([0, 1, 2, 3]);
    for mutation in 0..7 {
        let mut graph = Module::new("nominal-roster-control");
        graph.functions = vec![root.clone(), helper.clone()];
        if mutation != 0 {
            graph
                .functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration());
        }
        match mutation {
            0 | 1 => {}
            2 => graph
                .functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration()),
            3 => graph.functions[2] = root.clone(),
            4 => graph.functions[2] = AmdGpuDiagnosticOperation::DebugTrap.declaration(),
            5 => graph.functions[1].body = None,
            6 => {}
            _ => unreachable!(),
        }
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: None,
            failed: false,
            resource_error: None,
        };
        // A byte-identical detached root is not the selected actual owner row.
        let selected = if mutation == 6 {
            &root
        } else {
            &graph.functions[0]
        };
        let result =
            nominal::closed_function_roster(&graph, selected, &graph.functions[1], &mut meter);
        assert_eq!(
            result.ok(),
            match mutation {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            }
        );
        assert!(
            nominal::closed_function_roster(
                &graph,
                &graph.functions[0],
                &graph.functions[0],
                &mut meter
            )
            .is_err()
        );
        assert_eq!(meter.budget.storage(), FLOOR);
        assert!(!meter.exhausted());
    }
    assert!(nominal::trap_presence(false, 0).is_ok());
    assert!(nominal::trap_presence(true, 1).is_ok());
    assert!(nominal::trap_presence(true, 2).is_ok());
    assert!(nominal::trap_presence(true, 0).is_err()); // unused declaration
    assert!(nominal::trap_presence(false, 1).is_err()); // unresolved Trap
}

fn nominal_trap_roster_work_trial(limit: usize) -> (bool, usize, bool) {
    let (_, root, helper) = synthetic([0, 1, 2, 3]);
    let mut graph = Module::new("nominal-roster-work-control");
    graph.functions = vec![root, helper, AmdGpuDiagnosticOperation::Trap.declaration()];
    let mut work = Work::new(limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        allowance: None,
        failed: false,
        resource_error: None,
    };
    let result = nominal::closed_function_roster(
        &graph,
        &graph.functions[0],
        &graph.functions[1],
        &mut meter,
    )
    .and_then(|declaration| {
        assert!(declaration);
        nominal::prepay_graph_visits(&graph, &mut meter)
    });
    assert_eq!(meter.budget.storage(), FLOOR);
    assert!(meter.budget.work_ledger_identity_v1() == ledger);
    if result.is_err() {
        assert!(matches!(
            meter.resource_error,
            Some(ArgumentResourceV1::Work(_))
        ));
        assert!(meter.budget.check_prior_denials_v1().is_err());
    }
    (
        result.is_ok(),
        meter.budget.work(),
        meter.budget.failed_work().is_some(),
    )
}

#[test]
fn nominal_translation_optional_trap_roster_work_exact_and_one_short_are_typed() {
    let measured = nominal_trap_roster_work_trial(WORK);
    assert!(measured.0 && measured.1 > 0 && !measured.2);
    let exact = nominal_trap_roster_work_trial(measured.1);
    assert_eq!(exact, (true, measured.1, false));
    let short = nominal_trap_roster_work_trial(measured.1 - 1);
    assert!(!short.0 && short.2);
    let zero = nominal_trap_roster_work_trial(0);
    assert_eq!(zero, (false, 0, true));
}

fn trial(
    permutation: [u8; 4],
    mutation: usize,
    work_limit: usize,
    storage_limit: usize,
    local: Option<(usize, usize)>,
) -> (
    Result<ProductionMirPlironTranslationValidationV1, NativeTranslationErrorV1>,
    bool,
    usize,
    usize,
    bool,
    bool,
) {
    let (relation, mut root, mut helper) = synthetic(permutation);
    match mutation {
        0 => {}
        1 => root.body.as_mut().unwrap().blocks[0].operations[0].results[0].id = ValueId(999),
        2 => {
            let OperationKind::Call { arguments, .. } =
                &mut root.body.as_mut().unwrap().blocks[0].operations[0].kind
            else {
                unreachable!()
            };
            arguments.swap(0, 1);
        }
        3 => {
            let OperationKind::Matrix(matrix) =
                &mut helper.body.as_mut().unwrap().blocks[0].operations[0].kind
            else {
                unreachable!()
            };
            matrix.active_lanes = 32;
        }
        4 => helper.body.as_mut().unwrap().blocks[0].operations[0].results[0].id = ValueId(999),
        5 => {
            let Some(Terminator::Return { values }) =
                &mut helper.body.as_mut().unwrap().blocks[0].terminator
            else {
                unreachable!()
            };
            values.swap(0, 1);
        }
        _ => unreachable!(),
    }
    let mut work = Work::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let mut allowance =
        local.map(|(work, storage)| TranslationAllowanceV1::new(&budget, work, storage));
    let mut reached = false;
    let result = with_nominal_meter(&mut budget, allowance.as_mut(), |meter| {
        let accepted = meter
            .check_original_bounded(|budget| {
                bf16_query_components_v1(
                    &relation,
                    &root,
                    &helper,
                    &root.body.as_ref().unwrap().blocks[0].operations[0],
                    &helper.body.as_ref().unwrap().blocks[0].operations[0],
                    budget,
                )
                .map_err(nominal::component_error)
            })
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        reached = accepted;
        // Deliberate stop at the component boundary. No fabricated translation
        // validation, ranked receipt, full owner or normal success is returned.
        Err(ProductionMirPlironTranslationErrorV1::KernelShape)
    });
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == identity);
    (
        result,
        reached,
        budget.work(),
        budget.peak_storage(),
        budget.failed_work().is_some(),
        budget.failed_storage().is_some(),
    )
}

#[test]
fn nominal_translation_bridge_checks_identity_swap_and_component_mutations() {
    for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
        let (result, reached, ..) = trial(permutation, 0, WORK, STORAGE, None);
        assert!(reached);
        assert_eq!(
            result,
            Err(NativeTranslationErrorV1::Translation(
                ProductionMirPlironTranslationErrorV1::KernelShape
            ))
        );
        for mutation in 1..=5 {
            let (result, reached, ..) = trial(permutation, mutation, WORK, STORAGE, None);
            assert!(!reached);
            assert_eq!(
                result,
                Err(NativeTranslationErrorV1::Translation(
                    ProductionMirPlironTranslationErrorV1::KernelShape
                ))
            );
        }
    }
}
#[test]
fn nominal_translation_original_exact_and_one_short_accounts_are_typed() {
    let p = [0, 1, 2, 3];
    let (_, reached, work, peak, ..) = trial(p, 0, WORK, STORAGE, None);
    assert!(reached && work > 0 && peak > FLOOR);
    assert!(trial(p, 0, work, peak, None).1);
    let short_work = trial(p, 0, work - 1, peak, None);
    assert!(!short_work.1 && short_work.4);
    assert!(matches!(
        short_work.0,
        Err(NativeTranslationErrorV1::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
    let short_storage = trial(p, 0, work, peak - 1, None);
    assert!(!short_storage.1 && short_storage.5);
    assert!(matches!(
        short_storage.0,
        Err(NativeTranslationErrorV1::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
}
#[test]
fn nominal_translation_preserves_local_allowance_refusal_without_global_relabel() {
    let p = [0, 1, 2, 3];
    let (_, reached, work, peak, ..) = trial(p, 0, WORK, STORAGE, None);
    assert!(reached);
    assert!(trial(p, 0, WORK, STORAGE, Some((work, peak - FLOOR))).1);
    for local in [(work - 1, peak - FLOOR), (work, peak - FLOOR - 1)] {
        let result = trial(p, 0, WORK, STORAGE, Some(local));
        assert!(!result.1);
        assert_eq!(
            result.0,
            Err(NativeTranslationErrorV1::Translation(
                ProductionMirPlironTranslationErrorV1::ResourceLimit
            ))
        );
    }
}
#[test]
fn nominal_translation_tensor_contract_and_singleton_roster_are_exact() {
    use dialect_kernel::TensorConvergenceAttr as Scope;
    let (_, _, helper) = synthetic([0, 1, 2, 3]);
    let OperationKind::Matrix(matrix) = &helper.body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let contract = matrix.tensor_layout.unwrap();
    let mut count = 0;
    assert!(nominal::tensor_count(count).is_err());
    nominal::tensor_row(&mut count, matrix, contract, Scope::UniformSubgroup, 64).unwrap();
    assert_eq!(nominal::tensor_count(count).unwrap(), 1);
    assert!(nominal::tensor_row(&mut count, matrix, contract, Scope::UniformSubgroup, 64).is_err());
    for (contract, scope, lanes) in [
        (contract, Scope::UniformWorkgroup, 64),
        (contract, Scope::Divergent, 64),
        (contract, Scope::Opaque, 64),
        (contract, Scope::UniformSubgroup, 32),
        (
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            Scope::UniformSubgroup,
            64,
        ),
    ] {
        let mut count = 0;
        assert!(nominal::tensor_row(&mut count, matrix, contract, scope, lanes).is_err());
        assert_eq!(count, 0);
    }
}
#[test]
fn nominal_translation_frame_preserves_callback_surplus_and_refuses_hidden_denial() {
    for mode in 0..3 {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_nominal_meter(&mut budget, None, |meter| {
            meter.reserve(7).unwrap();
            match mode {
                0 => Err(ProductionMirPlironTranslationErrorV1::KernelShape),
                1 => panic!("inert nominal translation callback"),
                _ => {
                    let _ = meter.budget.charge_work(WORK);
                    Err(ProductionMirPlironTranslationErrorV1::KernelShape)
                }
            }
        });
        assert_eq!(budget.storage(), FLOOR + 7);
        if mode == 2 {
            assert!(matches!(
                result,
                Err(NativeTranslationErrorV1::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
            assert!(budget.failed_work().is_some());
        } else {
            assert_eq!(
                result,
                Err(NativeTranslationErrorV1::Translation(
                    ProductionMirPlironTranslationErrorV1::KernelShape
                ))
            );
        }
        budget.release_storage(7).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn nominal_translation_graph_header_pass_and_later_scans_are_prepaid_exactly() {
    let (_, root, helper) = synthetic([0, 1, 2, 3]);
    let mut graph = Module::new("inert_nominal_scan_control");
    graph.functions.extend([root, helper]);
    let blocks: usize = graph
        .functions
        .iter()
        .map(|f| f.body.as_ref().unwrap().blocks.len())
        .sum();
    let operations: usize = graph
        .functions
        .iter()
        .flat_map(|f| &f.body.as_ref().unwrap().blocks)
        .map(|b| b.operations.len())
        .sum();
    // A separate function/header counting pass, then four later census/lookups.
    let required = graph.functions.len() + blocks + 4 * (blocks + operations);
    for (limit, accepted) in [(required, true), (required - 1, false), (0, false)] {
        let mut work = Work::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            allowance: None,
            failed: false,
            resource_error: None,
        };
        assert_eq!(
            nominal::prepay_graph_visits(&graph, &mut meter).is_ok(),
            accepted
        );
        assert_eq!(meter.budget.storage(), FLOOR);
        if accepted {
            assert_eq!(meter.budget.work(), required);
            assert!(!meter.exhausted());
        } else {
            assert!(meter.exhausted());
            assert!(matches!(
                meter.resource_error,
                Some(ArgumentResourceV1::Work(_))
            ));
            assert!(meter.budget.failed_work().is_some());
        }
    }
}

#[test]
fn private_nominal_entry_refuses_actual_legacy_owner_and_preserves_original_denial() {
    use super::super::assert_origins_v1_tests::{Fixture, materialize};
    use super::super::require_private_bf16_nominal_translation_owner_v1 as check;
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(Fixture::Literal(true), true, &mut budget);
    let retained = owner.unit_local_source_storage_floor_v1().unwrap();
    budget.reserve_storage(retained).unwrap();
    let selected = owner.semantic_ssa().source_semantic().roots()[0];
    let floor = budget.storage();
    let start = budget.work();
    assert!(matches!(
        check(&owner, selected, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "private nominal translation",
            }
        )
    ));
    assert_eq!(budget.work(), start + 1);
    assert_eq!(budget.storage(), floor);
    // Exhaust the SAME original Work, not a replacement lookup budget.
    budget.charge_work(WORK - budget.work()).unwrap();
    assert!(matches!(
        check(&owner, selected, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        check(&owner, selected, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn private_nominal_phase_header_uses_original_exact_and_local_first_limits() {
    use super::super::with_private_nominal_translation_phase_v1 as phase;
    let header = std::mem::size_of::<TranslationAllowanceV1>();
    assert!(header > 0);
    for (global_work, global_storage, local_work, local_storage, expected) in [
        (header, FLOOR + header, header, header, 0),
        (header, FLOOR + header, header - 1, header, 1),
        (header, FLOOR + header, header, header - 1, 1),
        (header - 1, FLOOR + header, header, header, 2),
        (header, FLOOR + header - 1, header, header, 3),
        // Tie is local-first and never poisons an untouched original account.
        (header - 1, FLOOR + header - 1, header - 1, header - 1, 1),
    ] {
        let mut work = Work::new(global_work);
        let mut budget = ArgumentBudgetV1::new(&mut work, global_storage);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut reached = false;
        let result = phase(
            &mut budget,
            local_work,
            local_storage,
            |budget, allowance| {
                reached = true;
                assert_eq!(budget.storage(), FLOOR + header);
                assert_eq!(allowance.floor, FLOOR);
                assert_eq!(allowance.work, header);
                assert!(allowance.ledger == budget.work_ledger_identity_v1());
                // Exact cap leaves no refill for a later query.
                assert!(allowance.work(budget, 1).is_err());
                Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                    ProductionMirPlironTranslationErrorV1::KernelShape,
                ))
            },
        );
        match expected {
            0 => {
                assert!(reached);
                // The swallowed local one-extra-work denial remains sticky.
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::ResourceLimit
                    ))
                ));
                assert_eq!(budget.work(), header);
            }
            1 => {
                assert!(!reached);
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::ResourceLimit
                    ))
                ));
                assert_eq!(budget.work(), 0);
                assert!(budget.failed_work().is_none() && budget.failed_storage().is_none());
            }
            2 => {
                assert!(!reached);
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert!(budget.failed_work().is_some());
            }
            3 => {
                assert!(!reached);
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
                assert!(budget.failed_storage().is_some());
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn private_nominal_phase_header_cleanup_retains_surplus_and_sticky_history() {
    use super::super::with_private_nominal_translation_phase_v1 as phase;
    let header = std::mem::size_of::<TranslationAllowanceV1>();
    for mode in 0..3 {
        let mut work = Work::new(header);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + header + 7);
        budget.reserve_storage(FLOOR).unwrap();
        let result = phase(&mut budget, header, header + 7, |budget, _| {
            match mode {
                0 => panic!("private phase deliberate control"),
                1 => {
                    budget.reserve_storage(7).unwrap();
                }
                2 => {
                    assert!(budget.charge_work(1).is_err());
                }
                _ => unreachable!(),
            }
            Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::KernelShape,
            ))
        });
        match mode {
            0 => {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::KernelShape
                    ))
                ));
                assert_eq!(budget.storage(), FLOOR);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                // No broad-delta refund erases another live reservation.
                assert_eq!(budget.storage(), FLOOR + header + 7);
            }
            2 => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert!(budget.failed_work().is_some());
                assert_eq!(budget.storage(), FLOOR);
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.work(), header);
    }
}
