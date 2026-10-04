//! Inert private predicate controls, not a constructed source inspection owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
fn paid<T>(run: impl FnOnce(&mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = run(&mut budget);
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 17);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}
fn module(trap: bool) -> Module {
    let mut module = Module::new("inert-seal-control");
    module.kernels.push(Kernel::new(
        "kernel",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![], vec![]),
        vec![],
        vec![],
    ));
    if trap {
        module
            .functions
            .push(AmdGpuDiagnosticOperation::Trap.declaration());
    }
    module
}
fn relation() -> SemanticKirFunctionCorrespondenceV1 {
    SemanticKirFunctionCorrespondenceV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        kernel_ir_function: FunctionId::new("root"),
        role: SemanticKirFunctionRoleV1::KernelEntry,
    }
}
fn source_block(kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([7; 32]),
        source,
        vec![],
        SemanticTerminatorV1::new(source, kind),
    )
    .unwrap()
}
fn source_call(
    arguments: Vec<SemanticOperandV1>,
    destination: Option<SemanticCallDestinationV1>,
    unwind: SemanticUnwindActionV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            arguments,
            destination,
            unwind,
        )
        .unwrap(),
    )
}
fn source_trap() -> SemanticBasicBlockV1 {
    source_block(source_call(
        vec![],
        None,
        SemanticUnwindActionV1::Unreachable,
    ))
}
fn callable() -> SemanticCallableDeclV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([8; 32]),
        SemanticLayoutIdentityV1::from_sha256([8; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([8; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([8; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([8; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([8; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([8; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Trap,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([8; 32]),
    }
}
fn trap_block(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block
        .operations
        .push(AmdGpuDiagnosticOperation::Trap.operation(None));
    block.terminator = Some(Terminator::Unreachable);
    block
}
fn term(block: u32) -> SemanticKirTerminatorOperationSpanV1 {
    SemanticKirTerminatorOperationSpanV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        semantic_block: SemanticBlockIdV1::from_index(block),
        kernel_ir_block: BlockId(block),
        first_operation_ordinal: 0,
        operation_count: 1,
    }
}
fn synthetic(block: u32) -> SemanticKirSyntheticOperationSpanV1 {
    SemanticKirSyntheticOperationSpanV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        rule: SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
        kernel_ir_block: BlockId(block),
        first_operation_ordinal: 0,
        operation_count: 1,
    }
}

#[test]
fn original_singleton_and_exact_appended_trap_preserve_slot_zero() {
    for trap in [false, true] {
        let module = module(trap);
        paid(|b| {
            let (root, declared) = select_root(&module, &[relation()], ROOT, b).unwrap();
            assert!(std::ptr::eq(root, &module.functions[0]));
            assert_eq!(declared, trap);
        });
    }
}
#[test]
fn extra_or_absent_kernel_function_and_relation_rosters_refuse() {
    for which in 0..6 {
        let mut module = module(true);
        let mut relations = vec![relation()];
        match which {
            0 => module.kernels.clear(),
            1 => module.kernels.push(module.kernels[0].clone()),
            2 => module.functions.clear(),
            3 => module.functions.push(module.functions[0].clone()),
            4 => relations.clear(),
            _ => relations.push(relation()),
        }
        paid(|b| assert!(select_root(&module, &relations, ROOT, b).is_err()));
    }
}
#[test]
fn reordered_or_cross_owned_root_is_not_recovered_by_search() {
    for which in 0..8 {
        let mut module = module(true);
        let mut relation = relation();
        match which {
            0 => module.functions.swap(0, 1),
            1 => module.functions[0].body = None,
            2 => module.functions[0].role = FunctionRole::InternalHelper,
            3 => module.kernels[0].entry = FunctionId::new("foreign"),
            4 => relation.kernel_ir_function = FunctionId::new("foreign"),
            5 => relation.semantic_function = SemanticFunctionIdV1::from_index(1),
            6 => relation.correspondence_owner = SemanticFunctionIdV1::from_index(1),
            _ => relation.role = SemanticKirFunctionRoleV1::InternalHelper,
        }
        paid(|b| assert!(select_root(&module, &[relation], ROOT, b).is_err()));
    }
}
#[test]
fn every_appended_declaration_field_is_checked_including_legacy_capability() {
    for which in 0..10 {
        let mut f = AmdGpuDiagnosticOperation::Trap.declaration();
        match which {
            0 => f.id = FunctionId::new("foreign"),
            1 => f.role = FunctionRole::InternalHelper,
            2 => {
                f.body = Some(FunctionBody {
                    parameters: vec![],
                    blocks: vec![],
                })
            }
            3 => f.signature.parameters.push(Type::Scalar(ScalarType::U32)),
            4 => f.signature.results.push(Type::Scalar(ScalarType::U32)),
            5 => f.required_capabilities.clear(),
            6 => {
                f.required_capabilities.insert(TargetCapability::BFloat16);
            }
            7 => f.required_capabilities = [TargetCapability::BFloat16].into(),
            8 => {
                f.required_capabilities = [TargetCapability::Extension {
                    namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
                    name: fe2o3_kernel_ir::AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
                }]
                .into()
            }
            _ => {
                f.required_capabilities = [TargetCapability::Extension {
                    namespace: "foreign".to_owned(),
                    name: AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
                }]
                .into()
            }
        }
        paid(|b| assert!(exact_trap_declaration(&f, b).is_err()));
    }
}
#[test]
fn unused_declaration_or_undeclared_call_cannot_pass() {
    paid(|b| {
        require_declaration_use(false, 0, b).unwrap();
        require_declaration_use(true, 2, b).unwrap();
        assert!(require_declaration_use(true, 0, b).is_err());
        assert!(require_declaration_use(false, 1, b).is_err());
    });
}
#[test]
fn actual_source_trap_shape_and_repeated_sites_share_one_declaration() {
    let source = [source_trap(), source_trap()];
    let calls = [callable()];
    let terms = [term(0), term(1)];
    paid(|b| {
        let origins = Origins::new(ROOT, &source, &calls, &terms, &[], b).unwrap();
        origins.validate_call(&trap_block(0), 0, b).unwrap();
        origins.validate_call(&trap_block(1), 0, b).unwrap();
        require_declaration_use(true, 2, b).unwrap();
    });
}
#[test]
fn same_owner_synthetic_assert_trap_has_its_own_exact_origin() {
    paid(|b| {
        let rows = [synthetic(2)];
        let origins = Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap();
        origins.validate_call(&trap_block(2), 0, b).unwrap();
    });
}
#[test]
fn missing_duplicate_or_dual_source_synthetic_origins_refuse() {
    let source = [source_trap()];
    let calls = [callable()];
    for which in 0..4 {
        let terms = match which {
            1 => vec![term(0), term(0)],
            3 => vec![term(0)],
            _ => vec![],
        };
        let synth = match which {
            2 => vec![synthetic(0), synthetic(0)],
            3 => vec![synthetic(0)],
            _ => vec![],
        };
        paid(|b| {
            let origins = Origins::new(ROOT, &source, &calls, &terms, &synth, b).unwrap();
            assert!(origins.validate_call(&trap_block(0), 0, b).is_err());
        });
    }
}
#[test]
fn source_origin_owner_coordinate_and_overlapping_coverage_refuse() {
    let source = [source_trap()];
    let calls = [callable()];
    for which in 0..6 {
        let mut span = term(0);
        match which {
            0 => span.correspondence_owner = SemanticFunctionIdV1::from_index(1),
            1 => span.semantic_function = SemanticFunctionIdV1::from_index(1),
            2 => span.semantic_block = SemanticBlockIdV1::from_index(1),
            3 => span.kernel_ir_block = BlockId(1),
            4 => span.operation_count = 2,
            _ => span.operation_count = 0,
        }
        paid(|b| {
            let rows = [span];
            let origins = Origins::new(ROOT, &source, &calls, &rows, &[], b).unwrap();
            assert!(origins.validate_call(&trap_block(0), 0, b).is_err());
        });
    }
    // A second wider span that covers the same call cannot hide beside a valid one.
    paid(|b| {
        let mut wider = term(0);
        wider.operation_count = 2;
        let rows = [term(0), wider];
        let origins = Origins::new(ROOT, &source, &calls, &rows, &[], b).unwrap();
        assert!(origins.validate_call(&trap_block(0), 0, b).is_err());
    });
}
#[test]
fn synthetic_owner_rule_and_coverage_are_not_inferred_from_trap_spelling() {
    for which in 0..6 {
        let mut span = synthetic(0);
        match which {
            0 => span.correspondence_owner = SemanticFunctionIdV1::from_index(1),
            1 => span.semantic_function = SemanticFunctionIdV1::from_index(1),
            2 => span.rule = SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
            3 => span.first_operation_ordinal = 1,
            4 => span.operation_count = 2,
            _ => span.kernel_ir_block = BlockId(1),
        }
        paid(|b| {
            let rows = [span];
            let origins = Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap();
            assert!(origins.validate_call(&trap_block(0), 0, b).is_err());
        });
    }
}
#[test]
fn source_call_must_be_intrinsic_no_argument_no_destination_no_cleanup() {
    let place = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![],
            SemanticTypeIdV1::from_index(0),
        )
        .unwrap()
    };
    for which in 0..6 {
        let mut callables = vec![callable()];
        let kind = match which {
            0 => {
                callables[0] = SemanticCallableDeclV1::defined(ROOT);
                source_call(vec![], None, SemanticUnwindActionV1::Unreachable)
            }
            1 => {
                callables.clear();
                source_call(vec![], None, SemanticUnwindActionV1::Unreachable)
            }
            2 => SemanticTerminatorKindV1::Return,
            3 => source_call(
                vec![SemanticOperandV1::Copy(place())],
                None,
                SemanticUnwindActionV1::Unreachable,
            ),
            4 => source_call(
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(0),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            ),
            _ => source_call(
                vec![],
                None,
                SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallUnwind,
                    SemanticBlockIdV1::from_index(0),
                )),
            ),
        };
        paid(|b| {
            let source = [source_block(kind)];
            let terms = [term(0)];
            let origins = Origins::new(ROOT, &source, &callables, &terms, &[], b).unwrap();
            assert!(origins.validate_call(&trap_block(0), 0, b).is_err());
        });
    }
}
#[test]
fn canonical_call_operand_result_tail_and_unreachable_checks_are_exact() {
    for which in 0..6 {
        let mut block = trap_block(0);
        match which {
            0 => {
                block.operations[0].kind = OperationKind::Call {
                    callee: FunctionId::new("foreign"),
                    arguments: vec![],
                }
            }
            1 => {
                let OperationKind::Call { arguments, .. } = &mut block.operations[0].kind else {
                    unreachable!()
                };
                arguments.push(ValueId(3));
            }
            2 => block.operations[0]
                .results
                .push(ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))),
            3 => block
                .operations
                .push(AmdGpuDiagnosticOperation::Trap.operation(None)),
            4 => block.terminator = Some(Terminator::Return { values: vec![] }),
            _ => block.terminator = None,
        }
        paid(|b| {
            let rows = [synthetic(0)];
            let origins = Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap();
            assert!(origins.validate_call(&block, 0, b).is_err());
        });
    }
}
#[test]
fn finite_source_and_trace_bounds_accept_exact_and_refuse_one_over() {
    let blocks = vec![source_trap(); SOURCE_BLOCKS];
    let terms = vec![term(0); SOURCE_BLOCKS];
    let synth = vec![synthetic(0); SYNTHETIC_SPANS];
    paid(|b| {
        assert!(Origins::new(ROOT, &blocks, &[], &terms, &synth, b).is_ok());
    });
    for which in 0..3 {
        let mut blocks = blocks.clone();
        let mut terms = terms.clone();
        let mut synth = synth.clone();
        match which {
            0 => blocks.push(source_trap()),
            1 => terms.push(term(0)),
            _ => synth.push(synthetic(0)),
        }
        paid(|b| assert!(Origins::new(ROOT, &blocks, &[], &terms, &synth, b).is_err()));
    }
}
#[test]
fn span_arithmetic_overflow_refuses_instead_of_wrapping_coverage() {
    assert!(covers(u32::MAX, 1, 0).is_err());
    assert!(!covers(0, 0, 0).unwrap());
    assert!(covers(0, 1, 0).unwrap());
    paid(|b| {
        let mut span = synthetic(0);
        span.first_operation_ordinal = u32::MAX;
        let rows = [span];
        let origins = Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap();
        assert!(matches!(
            origins.validate_call(&trap_block(0), 0, b),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    });
}
#[test]
fn work_denial_precedes_header_and_origin_inspection_without_touching_floor() {
    let module = module(true);
    let rows = [synthetic(0)];
    let block = trap_block(0);
    let origins = paid(|b| Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap());
    for header in [true, false] {
        let mut work = Work::new(0);
        let mut b = Budget::new(&mut work, 17);
        b.reserve_storage(17).unwrap();
        let result = if header {
            select_root(&module, &[relation()], ROOT, &mut b).map(|_| ())
        } else {
            origins.validate_call(&block, 0, &mut b)
        };
        assert!(matches!(result, Err(Error::Resource(_))));
        assert!(b.failed_work().is_some());
        assert_eq!(b.work(), 0);
        assert_eq!(b.storage(), 17);
    }
}
#[test]
fn exact_paid_header_work_and_one_short_share_the_original_ledger() {
    let module = module(true);
    let needed = paid(|b| {
        select_root(&module, &[relation()], ROOT, b).unwrap();
        b.work()
    });
    for limit in [needed - 1, needed] {
        let mut work = Work::new(limit);
        let mut b = Budget::new(&mut work, 17);
        b.reserve_storage(17).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let ok = select_root(&module, &[relation()], ROOT, &mut b).is_ok();
        assert_eq!(ok, limit == needed);
        assert_eq!(b.storage(), 17);
        assert_eq!(b.peak_storage(), 17);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.failed_work().is_some(), limit < needed);
    }
}
#[test]
fn exact_origin_work_and_one_short_keep_consumed_work_without_allocating() {
    let rows = [synthetic(0)];
    let block = trap_block(0);
    let origins = paid(|b| Origins::new(ROOT, &[], &[], &[], &rows, b).unwrap());
    let needed = paid(|b| {
        origins.validate_call(&block, 0, b).unwrap();
        b.work()
    });
    for limit in [needed - 1, needed] {
        let mut work = Work::new(limit);
        let mut b = Budget::new(&mut work, 17);
        b.reserve_storage(17).unwrap();
        assert_eq!(
            origins.validate_call(&block, 0, &mut b).is_ok(),
            limit == needed
        );
        assert_eq!(b.storage(), 17);
        assert_eq!(b.peak_storage(), 17);
        assert!(b.work() > 0);
        assert_eq!(b.failed_work().is_some(), limit < needed);
    }
}

#[test]
fn authored_trap_span_may_follow_statements_but_synthetic_trap_may_not() {
    let mut block = trap_block(0);
    block.operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(0)),
        ),
    );
    let source = [source_trap()];
    let calls = [callable()];
    let mut span = term(0);
    span.first_operation_ordinal = 1;
    paid(|b| {
        let terms = [span];
        let origins = Origins::new(ROOT, &source, &calls, &terms, &[], b).unwrap();
        origins.validate_call(&block, 1, b).unwrap();
    });
    paid(|b| {
        let mut span = synthetic(0);
        span.first_operation_ordinal = 1;
        let rows = [span];
        let origins = Origins::new(ROOT, &source, &calls, &[], &rows, b).unwrap();
        assert!(origins.validate_call(&block, 1, b).is_err());
    });
}
#[test]
fn zero_origin_header_work_refuses_before_even_the_oversize_roster_predicate() {
    let blocks = vec![source_trap(); SOURCE_BLOCKS + 1];
    let mut work = Work::new(0);
    let mut b = Budget::new(&mut work, 17);
    b.reserve_storage(17).unwrap();
    assert!(matches!(
        Origins::new(ROOT, &blocks, &[], &[], &[], &mut b),
        Err(Error::Resource(_))
    ));
    assert_eq!(b.storage(), 17);
    assert_eq!(b.work(), 0);
    assert!(b.failed_work().is_some());
}
