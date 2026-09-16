use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Constant, Kernel,
    LaunchDomain, LaunchExtent, ScalarType, Signature, ValueDef, verify_module,
};

fn execution(result: Option<(u32, Role)>, operation: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(operation),
    )
}

fn end(discarded: &[u32]) -> Operation {
    execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(11),
            discarded: discarded.iter().copied().map(ValueId).collect(),
        },
    )
}

fn module(operations: Vec<Operation>) -> Module {
    let mut module = Module::new("execution");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

pub(crate) fn fixture(elements: u16) -> Module {
    let mut operations = vec![
        execution(Some((10, Role::Context)), Execution::ContextIssue),
        execution(
            Some((11, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        execution(
            Some((
                12,
                Role::MaskedTileU32 {
                    lanes: 64,
                    elements,
                },
            )),
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements,
            },
        ),
        execution(
            Some((
                13,
                Role::LaneFragmentU32 {
                    lanes: 64,
                    elements,
                },
            )),
            Execution::TileIntoFragmentU32 {
                tile: ValueId(12),
                lanes: 64,
                elements,
            },
        ),
    ];
    let results = (0..u32::from(elements))
        .map(|i| ValueDef::new(ValueId(20 + i), Type::Scalar(ScalarType::U32)))
        .chain(
            (0..u32::from(elements))
                .map(|i| ValueDef::new(ValueId(20 + u32::from(elements) + i), Type::BOOL)),
        )
        .collect();
    operations.push(Operation::new(
        results,
        OperationKind::Execution(Execution::FragmentIntoPartsU32 {
            fragment: ValueId(13),
            lanes: 64,
            elements,
        }),
    ));
    operations.push(end(&[]));
    module(operations)
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn rejects(module: &Module, message: &str) {
    let errors = verify_module(module).expect_err(message);
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidSemanticOperation),
        "{message}: {errors}"
    );
}

#[test]
fn execution_v15_accepts_complete_consumption_discard_and_repeated_scopes() {
    for elements in [1, 4, 125] {
        verify_module(&fixture(elements)).unwrap();
    }
    let mut discarded = fixture(4);
    operations(&mut discarded).truncate(3);
    operations(&mut discarded).push(end(&[12]));
    verify_module(&discarded).unwrap();
    let mut repeated = fixture(4);
    operations(&mut repeated).extend([
        execution(
            Some((100, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        execution(
            None,
            Execution::ScopeEnd {
                workgroup: ValueId(100),
                discarded: vec![],
            },
        ),
    ]);
    verify_module(&repeated).unwrap();
    let sparse = module(vec![execution(
        Some((u32::MAX, Role::Context)),
        Execution::ContextIssue,
    )]);
    verify_module(&sparse).unwrap();
}

#[test]
fn execution_v15_registered_checks_reject_operations_without_role_definitions() {
    for operation in [
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Execution(Execution::ContextIssue),
        ),
        execution(
            None,
            Execution::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            },
        ),
    ] {
        assert!(
            operation
                .results
                .iter()
                .all(|result| !matches!(result.ty, Type::Execution(_)))
        );
        let candidate = module(vec![operation]);
        assert_eq!(candidate.functions[0].signature.parameters[1], Type::INDEX);
        rejects(
            &candidate,
            "registered checks must reject malformed execution even without lifecycle roles",
        );
    }
}

#[test]
fn execution_v15_refuses_stale_foreign_missing_and_double_consumption() {
    let mut duplicate = fixture(4);
    operations(&mut duplicate).push(execution(
        Some((100, Role::Context)),
        Execution::ContextIssue,
    ));
    rejects(
        &duplicate,
        "distinct SSA result does not permit a second context issuer",
    );
    let mut open = fixture(4);
    operations(&mut open).pop();
    rejects(&open, "open scope");
    let mut missing = fixture(4);
    operations(&mut missing).truncate(3);
    operations(&mut missing).push(end(&[]));
    rejects(&missing, "omitted live tile");
    let mut consumed = fixture(4);
    *operations(&mut consumed).last_mut().unwrap() = end(&[12]);
    rejects(&consumed, "consumed tile is not a live discard");
    let mut double = fixture(4);
    operations(&mut double).insert(
        4,
        execution(
            Some((
                100,
                Role::LaneFragmentU32 {
                    lanes: 64,
                    elements: 4,
                },
            )),
            Execution::TileIntoFragmentU32 {
                tile: ValueId(12),
                lanes: 64,
                elements: 4,
            },
        ),
    );
    rejects(&double, "tile consumed twice");
    let mut stale = fixture(4);
    operations(&mut stale).extend([execution(
        Some((
            100,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 4,
            },
        )),
        Execution::MaskedTileLoadU32 {
            workgroup: ValueId(11),
            input: ValueId(0),
            base: ValueId(1),
            lanes: 64,
            elements: 4,
        },
    )]);
    rejects(&stale, "ended workgroup reused");
    let mut overlapping = fixture(4);
    operations(&mut overlapping).insert(
        2,
        execution(
            Some((100, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
    );
    rejects(&overlapping, "exclusive context acquired twice");
    let mut foreign = fixture(4);
    operations(&mut foreign).extend([
        execution(
            Some((101, Role::Workgroup)),
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        execution(
            None,
            Execution::ScopeEnd {
                workgroup: ValueId(101),
                discarded: vec![ValueId(12)],
            },
        ),
    ]);
    rejects(
        &foreign,
        "equal-role foreign acquisition cannot discard the first tile",
    );
}

fn diamond(close_both: bool) -> Module {
    let mut module = fixture(1);
    operations(&mut module).truncate(2);
    operations(&mut module).push(Operation::effect_free(
        ValueDef::new(ValueId(50), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(50),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    for (id, close) in [(20, true), (30, close_both)] {
        blocks.push(BasicBlock {
            id: BlockId(id),
            parameters: vec![],
            operations: if close { vec![end(&[])] } else { vec![] },
            terminator: Some(Terminator::Branch {
                target: BlockId(40),
                arguments: vec![],
            }),
        });
    }
    blocks.push(BasicBlock {
        id: BlockId(40),
        parameters: vec![],
        operations: vec![],
        terminator: Some(Terminator::Return { values: vec![] }),
    });
    module
}

#[test]
fn execution_v15_requires_exact_joins_and_acyclic_cfg_even_for_balanced_loops() {
    verify_module(&diamond(true)).unwrap();
    rejects(&diamond(false), "one branch leaves the acquisition open");
    let mut self_loop = fixture(1);
    self_loop.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    rejects(&self_loop, "balanced scope self-loop");
    let mut loop_with_exit = fixture(1);
    operations(&mut loop_with_exit).push(Operation::effect_free(
        ValueDef::new(ValueId(50), Type::BOOL),
        OperationKind::Constant(Constant::Bool(false)),
    ));
    let blocks = &mut loop_with_exit.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    blocks.push(BasicBlock {
        id: BlockId(20),
        parameters: vec![],
        operations: vec![],
        terminator: Some(Terminator::ConditionalBranch {
            condition: ValueId(50),
            then_target: BlockId(20),
            then_arguments: vec![],
            else_target: BlockId(30),
            else_arguments: vec![],
        }),
    });
    blocks.push(BasicBlock {
        id: BlockId(30),
        parameters: vec![],
        operations: vec![],
        terminator: Some(Terminator::Return { values: vec![] }),
    });
    rejects(
        &loop_with_exit,
        "reachable role-free cycle still violates the first lifecycle profile",
    );
}

#[test]
fn execution_v15_rejects_role_transport_storage_and_unreachable_islands() {
    let role = Type::Execution(Role::Context);
    assert!(!role.is_storable());
    for ty in [
        role.clone(),
        Type::pointer(role.clone(), AddressSpace::Private, AccessMode::ReadWrite),
        Type::slice(role.clone(), AddressSpace::Global, AccessMode::ReadOnly),
    ] {
        let mut candidate = fixture(1);
        candidate.functions[0].signature.parameters[0] = ty;
        rejects(&candidate, "execution role in signature");
    }
    let mut selected = fixture(1);
    operations(&mut selected).insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(50), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ),
    );
    operations(&mut selected).insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(51), role.clone()),
            OperationKind::Select {
                condition: ValueId(50),
                true_value: ValueId(10),
                false_value: ValueId(10),
            },
        ),
    );
    rejects(&selected, "select duplicates an execution producer");
    let mut stored = fixture(1);
    operations(&mut stored).push(Operation::effect_free(
        ValueDef::new(
            ValueId(52),
            Type::pointer(role.clone(), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        OperationKind::Alloca {
            element: role,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    ));
    rejects(&stored, "alloca role");
    let mut island = fixture(1);
    island.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(BasicBlock {
            id: BlockId(100),
            parameters: vec![],
            operations: vec![execution(
                Some((100, Role::Context)),
                Execution::ContextIssue,
            )],
            terminator: Some(Terminator::Return { values: vec![] }),
        });
    rejects(&island, "unreachable issuer");
    let mut phi = fixture(1);
    operations(&mut phi).truncate(1);
    let blocks = &mut phi.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![ValueId(10)],
    });
    blocks.push(BasicBlock {
        id: BlockId(20),
        parameters: vec![ValueDef::new(ValueId(100), Type::Execution(Role::Context))],
        operations: vec![],
        terminator: Some(Terminator::Return { values: vec![] }),
    });
    rejects(&phi, "phi cannot rename execution producer identity");
}

#[test]
fn execution_v15_requires_scope_end_before_retained_calls() {
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    let call = Operation::new(
        vec![],
        OperationKind::Call {
            callee: "helper".into(),
            arguments: vec![],
        },
    );
    let mut closed = fixture(1);
    closed.functions.push(helper.clone());
    operations(&mut closed).push(call.clone());
    verify_module(&closed).unwrap();
    let mut open = fixture(1);
    open.functions.push(helper);
    operations(&mut open).insert(2, call);
    rejects(&open, "retained callee has no same-function disposal proof");
}

#[test]
fn execution_v15_rejects_trapping_and_unreachable_exits_with_live_scopes() {
    let mut module = fixture(1);
    operations(&mut module).truncate(2);
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
    rejects(&module, "unreachable does not discard a workgroup");
    for diagnostic in [
        crate::AmdGpuDiagnosticOperation::Trap,
        crate::AmdGpuDiagnosticOperation::AssertFail {
            site_id: ValueId(50),
            line: ValueId(51),
        },
    ] {
        let mut module = fixture(1);
        operations(&mut module).truncate(2);
        for id in [50, 51] {
            operations(&mut module).push(Operation::effect_free(
                ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(id)),
            ));
        }
        operations(&mut module).push(end(&[]));
        operations(&mut module).push(diagnostic.operation(None));
        module.functions.push(diagnostic.declaration());
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Unreachable);
        verify_module(&module).unwrap();
        operations(&mut module).remove(4);
        rejects(&module, "diagnostic cannot implicitly dispose a scope");
    }
}

#[test]
fn execution_v15_lifecycle_uses_exact_shared_resource_limits() {
    fn run(
        module: &Module,
        work_limit: usize,
        storage_limit: usize,
    ) -> (
        Result<(), crate::MeteredKernelIrVerificationErrorV1>,
        usize,
        usize,
        usize,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = crate::verify_depth_bounded_module_with_budget_v1(module, None, &mut budget)
            .map(|_| ());
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.storage(),
        )
    }
    let module = diamond(true);
    let (result, work, storage, remaining) = run(&module, usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!(remaining, 0);
    assert!(work > 1 && storage > 1);
    assert!(run(&module, work, storage).0.is_ok());
    for (work_limit, storage_limit) in [
        (0, storage),
        (work - 1, storage),
        (work, 0),
        (work, storage - 1),
    ] {
        let (result, _, _, remaining) = run(&module, work_limit, storage_limit);
        assert!(matches!(
            result,
            Err(crate::MeteredKernelIrVerificationErrorV1::Resource(_))
        ));
        assert_eq!(remaining, 0);
    }
}

#[test]
fn execution_v15_lifecycle_keeps_materialized_diagnostics_above_existing_storage_floor() {
    let mut module = fixture(1);
    operations(&mut module).pop();
    let function = &module.functions[0];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(7).unwrap();
    let flow = crate::analyze_control_flow_with_verification_budget_v1(
        function,
        crate::ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    let state = VerificationFunctionStateV1::build(function, &mut budget)
        .unwrap()
        .unwrap();
    let before_collector = budget.storage();
    let mut diagnostics = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
    let before_lifecycle = budget.storage();
    verify_execution_lifecycle_v15(
        &module,
        function,
        &state,
        Some(&flow),
        &mut diagnostics,
        &mut budget,
    )
    .unwrap();
    assert!(budget.storage() > before_lifecycle);
    diagnostics.abandon(&mut budget).unwrap();
    assert_eq!(budget.storage(), before_collector);
    state.release(&mut budget).unwrap();
    flow.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 7);
}
