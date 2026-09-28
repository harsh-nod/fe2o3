use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrReplayAdmissionErrorV18 as AdmissionError,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Signature, StorageLayoutLimitsV1, ValueDef,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};
const FLOOR: usize = 17;

fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}

fn conditional(then_target: u32, else_target: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(1000),
        then_target: BlockId(then_target),
        then_arguments: vec![],
        else_target: BlockId(else_target),
        else_arguments: vec![],
    }
}

fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        parameters: vec![],
        operations,
        terminator: Some(terminator),
    }
}

fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}

fn loop_module(outer_scope: bool, elements: u16) -> Module {
    let mut module = tests::fixture(elements);
    let entry = &mut blocks(&mut module)[0];
    let mut body = entry.operations.split_off(if outer_scope { 2 } else { 1 });
    let exit = if outer_scope {
        vec![body.pop().unwrap()]
    } else {
        vec![]
    };
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1000), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    entry.terminator = Some(branch(20));
    blocks(&mut module).extend([
        block(20, body, conditional(20, 30)),
        block(30, exit, Terminator::Return { values: vec![] }),
    ]);
    module
}

fn run(
    module: &Module,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<Owner, AdmissionError>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = Owner::from_module_ref_with_verification_budget_v18(module, LIMITS, &mut budget)
        .map(|(owner, _)| owner);
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

fn admit(module: &Module) -> Owner {
    let owner = run(module, usize::MAX, usize::MAX).0.unwrap();
    assert_eq!(owner.module(), module);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let (replayed, _) = Owner::from_canonical_bytes_with_verification_budget_v18(
        owner.canonical_bytes(),
        LIMITS,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(replayed.module(), module);
    assert_eq!(replayed.canonical_bytes(), owner.canonical_bytes());
    assert_eq!(replayed.identity(), owner.identity());
    owner
}

fn reject(module: &Module, expected: &str) {
    let error = run(module, usize::MAX, usize::MAX).0.unwrap_err();
    assert!(matches!(error, AdmissionError::Verification(_)), "{error}");
    assert!(error.to_string().contains(expected), "{error}");
}

#[test]
fn execution_v18_accepts_balanced_iterations_and_scopes_around_loops() {
    for outer in [false, true] {
        for elements in [1, 4, 125] {
            let module = loop_module(outer, elements);
            admit(&module);
            let legacy = crate::verify_module(&module).unwrap_err();
            assert!(legacy.to_string().contains("acyclic same-function CFG"));
        }
    }
}

#[test]
fn execution_v18_accepts_multiple_latches_and_irreducible_exact_state_cycles() {
    let mut multiple = loop_module(false, 4);
    blocks(&mut multiple)[1].terminator = Some(conditional(40, 50));
    blocks(&mut multiple).extend([
        block(40, vec![], branch(20)),
        block(50, vec![], conditional(20, 30)),
    ]);
    admit(&multiple);

    let mut irreducible = loop_module(false, 4);
    blocks(&mut irreducible)[0].terminator = Some(conditional(20, 40));
    blocks(&mut irreducible)[1].terminator = Some(conditional(40, 30));
    blocks(&mut irreducible).push(block(40, vec![], conditional(20, 30)));
    admit(&irreducible);
}

#[test]
fn execution_v18_accepts_nested_loops_and_balanced_nonreturning_loops() {
    let mut nested = loop_module(true, 4);
    blocks(&mut nested)[1].terminator = Some(branch(40));
    blocks(&mut nested).extend([
        block(40, vec![], conditional(40, 50)),
        block(50, vec![], conditional(20, 30)),
    ]);
    admit(&nested);

    let mut nonreturning = loop_module(false, 4);
    blocks(&mut nonreturning)[1].terminator = Some(branch(20));
    blocks(&mut nonreturning).pop();
    admit(&nonreturning);
}

#[test]
fn execution_v18_rejects_issuer_reentry_and_changed_backedge_ownership() {
    let source = loop_module(false, 4);
    admit(&source);
    let mut reentry = source.clone();
    blocks(&mut reentry)[1].terminator = Some(conditional(7, 30));
    reject(&reentry, "states differ across a CFG join or backedge");

    let mut acquisition = source;
    blocks(&mut acquisition)[1].operations.pop();
    reject(&acquisition, "states differ across a CFG join or backedge");

    let mut descendant = loop_module(true, 4);
    admit(&descendant);
    blocks(&mut descendant)[1].operations.truncate(1);
    reject(&descendant, "states differ across a CFG join or backedge");
}

#[test]
fn execution_v18_rejects_unmatched_joins_and_open_exits() {
    let mut module = loop_module(true, 4);
    let end = blocks(&mut module)[2].operations[0].clone();
    blocks(&mut module)[1].terminator = Some(conditional(40, 50));
    blocks(&mut module)[2].operations.clear();
    blocks(&mut module).extend([
        block(40, vec![end.clone()], branch(30)),
        block(50, vec![end], branch(30)),
    ]);
    admit(&module);
    blocks(&mut module)[4].operations.clear();
    reject(&module, "states differ across a CFG join or backedge");

    for unreachable in [false, true] {
        let mut open = loop_module(true, 4);
        if unreachable {
            blocks(&mut open)[2].terminator = Some(Terminator::Unreachable);
        }
        admit(&open);
        blocks(&mut open)[2].operations.clear();
        reject(&open, "remains live at function exit");
    }
}

#[test]
fn execution_v18_rejects_stale_and_foreign_acquisitions_in_loops() {
    let mut stale = loop_module(false, 4);
    admit(&stale);
    let mut load = blocks(&mut stale)[1].operations[1].clone();
    load.results[0].id = ValueId(300);
    blocks(&mut stale)[1].operations.push(load);
    reject(
        &stale,
        "violates exact producer, acquisition or consumption state",
    );

    let mut foreign = loop_module(false, 4);
    admit(&foreign);
    let mut derive = blocks(&mut foreign)[1].operations[0].clone();
    derive.results[0].id = ValueId(300);
    blocks(&mut foreign)[1].operations.extend([
        derive,
        Operation::new(
            vec![],
            OperationKind::Execution(Execution::ScopeEnd {
                workgroup: ValueId(300),
                discarded: vec![ValueId(12)],
            }),
        ),
    ]);
    reject(
        &foreign,
        "violates exact producer, acquisition or consumption state",
    );
}

#[test]
fn execution_v18_rejects_role_parameters_and_unreachable_capability_islands() {
    let mut parameter = loop_module(false, 4);
    admit(&parameter);
    blocks(&mut parameter)[1]
        .parameters
        .push(ValueDef::new(ValueId(300), Type::Execution(Role::Context)));
    let Some(Terminator::Branch { arguments, .. }) = &mut blocks(&mut parameter)[0].terminator
    else {
        unreachable!()
    };
    arguments.push(ValueId(10));
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut blocks(&mut parameter)[1].terminator
    else {
        unreachable!()
    };
    then_arguments.push(ValueId(10));
    reject(
        &parameter,
        "execution roles cannot be function or block parameters",
    );

    let mut island = loop_module(false, 4);
    admit(&island);
    blocks(&mut island).push(block(
        100,
        vec![Operation::new(
            vec![ValueDef::new(ValueId(300), Type::Execution(Role::Context))],
            OperationKind::Execution(Execution::ContextIssue),
        )],
        Terminator::Return { values: vec![] },
    ));
    reject(&island, "unreachable execution capability island");
}

#[test]
fn execution_v18_retained_calls_still_require_closed_scopes() {
    for outer in [false, true] {
        let mut module = loop_module(outer, 4);
        admit(&module);
        module.functions.push(Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block(0, vec![], Terminator::Return { values: vec![] })],
        ));
        blocks(&mut module)[1].operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![],
            },
        ));
        if outer {
            reject(
                &module,
                "violates exact producer, acquisition or consumption state",
            );
        } else {
            admit(&module);
        }
    }
}

#[test]
fn execution_v18_diagnostics_do_not_implicitly_close_scopes() {
    for diagnostic in [
        crate::AmdGpuDiagnosticOperation::Trap,
        crate::AmdGpuDiagnosticOperation::AssertFail {
            site_id: ValueId(300),
            line: ValueId(301),
        },
    ] {
        let mut module = loop_module(true, 4);
        for id in [300, 301] {
            blocks(&mut module)[2]
                .operations
                .push(Operation::effect_free(
                    ValueDef::new(ValueId(id), Type::Scalar(crate::ScalarType::U32)),
                    OperationKind::Constant(Constant::U32(id)),
                ));
        }
        blocks(&mut module)[2]
            .operations
            .push(diagnostic.operation(None));
        blocks(&mut module)[2].terminator = Some(Terminator::Unreachable);
        module.functions.push(diagnostic.declaration());
        admit(&module);
        blocks(&mut module)[2].operations.remove(0);
        reject(
            &module,
            "violates exact producer, acquisition or consumption state",
        );
    }
}

#[test]
fn execution_v18_cyclic_admission_has_exact_resource_limits_and_preserves_floor() {
    for outer in [false, true] {
        let module = loop_module(outer, 4);
        let (result, work, storage) = run(&module, usize::MAX, usize::MAX);
        result.unwrap();
        assert!(work > 1 && storage > FLOOR);
        let (exact, exact_work, exact_storage) = run(&module, work, storage);
        exact.unwrap();
        assert_eq!((exact_work, exact_storage), (work, storage));
        for (work_limit, storage_limit) in [
            (0, storage),
            (work - 1, storage),
            (work, FLOOR),
            (work, storage - 1),
        ] {
            let error = run(&module, work_limit, storage_limit).0.unwrap_err();
            let resource = match error {
                AdmissionError::Resource(error)
                | AdmissionError::Layout(crate::StorageLayoutErrorV1::Resource(error))
                | AdmissionError::Verification(
                    crate::BorrowedKernelIrVerificationErrorV1::Resource(error),
                )
                | AdmissionError::Decode(crate::KernelIrDecodeError::Resource(error)) => error,
                AdmissionError::Decode(crate::KernelIrDecodeError::WorkLimit(error))
                | AdmissionError::Encode(crate::KernelIrEncodeError::WorkLimit(error)) => {
                    ResourceError::Work(error)
                }
                other => panic!("expected a typed resource denial, got {other}"),
            };
            if work_limit < work {
                assert!(matches!(resource, ResourceError::Work(_)), "{resource}");
            } else {
                assert!(matches!(resource, ResourceError::Storage(_)), "{resource}");
            }
        }
    }
}
