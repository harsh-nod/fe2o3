use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1,
    MeteredKernelIrVerificationErrorV1 as VerificationError, ScalarType, Signature, SwitchCase,
    ValueDef, verify_module,
};
use std::collections::BTreeMap;

const ENTRY: u32 = 91;
const HEADER: u32 = 7;
const BODY: u32 = 4_000_000;
const EXIT: u32 = u32::MAX;

fn execution(result: Option<(u32, Role)>, operation: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(operation),
    )
}

fn issue() -> Operation {
    execution(Some((10, Role::Context)), Execution::ContextIssue)
}

fn acquire(id: u32) -> Operation {
    execution(
        Some((id, Role::Workgroup)),
        Execution::WorkgroupDerive {
            context: ValueId(10),
        },
    )
}

fn end(workgroup: u32, discarded: &[u32]) -> Operation {
    execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(workgroup),
            discarded: discarded.iter().copied().map(ValueId).collect(),
        },
    )
}

#[derive(Clone, Copy, Debug)]
enum Cleanup {
    Empty,
    Parts,
    Tile,
    Fragment,
}

const CLEANUPS: [Cleanup; 4] = [
    Cleanup::Empty,
    Cleanup::Parts,
    Cleanup::Tile,
    Cleanup::Fragment,
];

fn descendants(workgroup: u32, base: u32, cleanup: Cleanup) -> (Vec<Operation>, Vec<u32>) {
    if matches!(cleanup, Cleanup::Empty) {
        return (vec![], vec![]);
    }
    let mut operations = vec![execution(
        Some((
            base,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 1,
            },
        )),
        Execution::MaskedTileLoadU32 {
            workgroup: ValueId(workgroup),
            input: ValueId(0),
            base: ValueId(1),
            lanes: 64,
            elements: 1,
        },
    )];
    if matches!(cleanup, Cleanup::Tile) {
        return (operations, vec![base]);
    }
    operations.push(execution(
        Some((
            base + 1,
            Role::LaneFragmentU32 {
                lanes: 64,
                elements: 1,
            },
        )),
        Execution::TileIntoFragmentU32 {
            tile: ValueId(base),
            lanes: 64,
            elements: 1,
        },
    ));
    if matches!(cleanup, Cleanup::Fragment) {
        return (operations, vec![base + 1]);
    }
    operations.push(Operation::new(
        vec![
            ValueDef::new(ValueId(base + 2), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(base + 3), Type::BOOL),
        ],
        OperationKind::Execution(Execution::FragmentIntoPartsU32 {
            fragment: ValueId(base + 1),
            lanes: 64,
            elements: 1,
        }),
    ));
    (operations, vec![])
}

fn iteration(base: u32, cleanup: Cleanup) -> Vec<Operation> {
    let (children, discarded) = descendants(base, base + 1, cleanup);
    let mut operations = vec![acquire(base)];
    operations.extend(children);
    operations.push(end(base, &discarded));
    operations
}

fn jump(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}

fn choose(then_target: u32, else_target: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(2),
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

fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = super::tests::fixture(1);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(2));
    body.blocks = blocks;
    module
}

fn loop_module(operations: Vec<Operation>, outer: bool) -> Module {
    let mut preheader = vec![issue()];
    if outer {
        preheader.push(acquire(11));
    }
    module(vec![
        block(ENTRY, preheader, jump(HEADER)),
        block(HEADER, vec![], choose(BODY, EXIT)),
        block(BODY, operations, jump(HEADER)),
        block(
            EXIT,
            if outer { vec![end(11, &[])] } else { vec![] },
            Terminator::Return { values: vec![] },
        ),
    ])
}

fn block_mut(module: &mut Module, id: u32) -> &mut BasicBlock {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == BlockId(id))
        .unwrap()
}

// This oracle executes each bounded path independently. Its fresh dynamic lease
// numbers deliberately differ from the verifier's static slots and join states.
// It models lifecycle transitions only; public verification checks types/SSA.
#[derive(Clone, Default)]
struct Trace {
    context: Option<ValueId>,
    scope: Option<(ValueId, usize)>,
    children: BTreeMap<ValueId, (usize, bool)>, // lease, is_fragment
    acquisitions: usize,
    loads: usize,
}

impl Trace {
    fn step(&mut self, operation: &Operation) -> Result<(), &'static str> {
        let result = || match operation.results.as_slice() {
            [result] => Ok(result.id),
            _ => Err("expected one execution result"),
        };
        match &operation.kind {
            OperationKind::Execution(Execution::ContextIssue) => {
                if self.context.is_some() {
                    return Err("context issued twice on one path");
                }
                self.context = Some(result()?);
            }
            OperationKind::Execution(Execution::WorkgroupDerive { context }) => {
                if self.context != Some(*context) || self.scope.is_some() {
                    return Err("context unavailable for acquisition");
                }
                self.acquisitions += 1;
                self.scope = Some((result()?, self.acquisitions));
            }
            OperationKind::Execution(Execution::MaskedTileLoadU32 { workgroup, .. }) => {
                let Some((owner, lease)) = self.scope else {
                    return Err("load outside scope");
                };
                if owner != *workgroup || self.children.contains_key(&result()?) {
                    return Err("wrong scope or live descendant overwritten");
                }
                self.children.insert(result()?, (lease, false));
                self.loads += 1;
            }
            OperationKind::Execution(Execution::TileIntoFragmentU32 { tile, .. }) => {
                let Some((lease, false)) = self.children.remove(tile) else {
                    return Err("tile unavailable");
                };
                if self.scope.map(|(_, active)| active) != Some(lease)
                    || self.children.contains_key(&result()?)
                {
                    return Err("stale tile or live fragment overwritten");
                }
                self.children.insert(result()?, (lease, true));
            }
            OperationKind::Execution(Execution::FragmentIntoPartsU32 { fragment, .. }) => {
                let Some((lease, true)) = self.children.remove(fragment) else {
                    return Err("fragment unavailable");
                };
                if self.scope.map(|(_, active)| active) != Some(lease) {
                    return Err("fragment outlived its scope");
                }
            }
            OperationKind::Execution(Execution::ScopeEnd {
                workgroup,
                discarded,
            }) => {
                let Some((owner, lease)) = self.scope else {
                    return Err("end without acquisition");
                };
                if owner != *workgroup {
                    return Err("ending a different scope");
                }
                for child in discarded {
                    if self.children.remove(child).map(|(epoch, _)| epoch) != Some(lease) {
                        return Err("discard is stale, repeated or foreign");
                    }
                }
                if !self.children.is_empty() {
                    return Err("end omits a live descendant");
                }
                self.scope = None;
            }
            OperationKind::Call { arguments, .. } => {
                if self.scope.is_some() || !self.children.is_empty() || !arguments.is_empty() {
                    return Err("call with live execution ownership");
                }
            }
            _ => return Err("operation outside trace oracle vocabulary"),
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Paths {
    exits: usize,
    bounded: usize,
    max_acquisitions: usize,
    max_loads: usize,
}

fn trace_paths(module: &Module, bound: usize) -> Result<Paths, &'static str> {
    let blocks = &module.functions[0].body.as_ref().unwrap().blocks;
    let mut paths = Paths::default();
    let mut pending = vec![(blocks[0].id, Trace::default(), bound)];
    while let Some((id, mut trace, remaining)) = pending.pop() {
        if remaining == 0 {
            paths.bounded += 1;
            continue;
        }
        let block = blocks
            .iter()
            .find(|block| block.id == id)
            .ok_or("missing block")?;
        if block
            .parameters
            .iter()
            .any(|value| matches!(value.ty, Type::Execution(_)))
        {
            return Err("execution phi");
        }
        for operation in &block.operations {
            trace.step(operation)?;
        }
        paths.max_acquisitions = paths.max_acquisitions.max(trace.acquisitions);
        paths.max_loads = paths.max_loads.max(trace.loads);
        match block.terminator.as_ref().ok_or("missing terminator")? {
            Terminator::Branch { target, arguments } => {
                if !arguments.is_empty() {
                    return Err("unexpected block arguments");
                }
                pending.push((*target, trace, remaining - 1));
            }
            Terminator::ConditionalBranch {
                then_target,
                else_target,
                then_arguments,
                else_arguments,
                ..
            } => {
                if !then_arguments.is_empty() || !else_arguments.is_empty() {
                    return Err("unexpected block arguments");
                }
                // Explore both CFG edges, including different choices on later iterations.
                pending.push((*else_target, trace.clone(), remaining - 1));
                pending.push((*then_target, trace, remaining - 1));
            }
            Terminator::Switch {
                cases,
                default_target,
                default_arguments,
                ..
            } => {
                if !default_arguments.is_empty()
                    || cases.iter().any(|case| !case.arguments.is_empty())
                {
                    return Err("unexpected switch arguments");
                }
                pending.push((*default_target, trace.clone(), remaining - 1));
                for case in cases {
                    pending.push((case.target, trace.clone(), remaining - 1));
                }
            }
            Terminator::Return { .. } | Terminator::Unreachable => {
                if trace.scope.is_some() || !trace.children.is_empty() {
                    return Err("live execution ownership at exit");
                }
                paths.exits += 1;
            }
            _ => return Err("terminator outside trace oracle vocabulary"),
        }
    }
    Ok(paths)
}

fn accepts(module: &Module) -> Paths {
    let paths = trace_paths(module, 18).expect("independent bounded lifecycle traces");
    assert!(paths.exits > 0 && paths.bounded > 0, "{paths:?}");
    verify_module(module).unwrap();
    paths
}

fn rejects(module: &Module, expected: &str) {
    let errors = verify_module(module).expect_err(expected);
    assert!(
        errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidSemanticOperation
                && diagnostic.message.contains(expected)
        }),
        "expected {expected:?}: {errors}"
    );
}

#[test]
fn balanced_iterations_consume_or_discard_fresh_descendants() {
    for cleanup in CLEANUPS {
        let candidate = loop_module(iteration(11, cleanup), false);
        let paths = accepts(&candidate);
        assert!(paths.max_acquisitions >= 3, "{cleanup:?}: {paths:?}");
        if !matches!(cleanup, Cleanup::Empty) {
            assert!(paths.max_loads >= 3, "{cleanup:?}: {paths:?}");
        }
    }
    let mut candidate = loop_module(iteration(11, Cleanup::Parts), false);
    // A do-while self-edge must compare the body's incoming state as well.
    block_mut(&mut candidate, HEADER).terminator = Some(jump(BODY));
    block_mut(&mut candidate, BODY).terminator = Some(choose(BODY, EXIT));
    assert!(accepts(&candidate).max_acquisitions >= 3);
}

#[test]
fn outer_workgroup_survives_loops_with_consumed_descendants() {
    for cleanup in [Cleanup::Empty, Cleanup::Parts] {
        let (operations, discarded) = descendants(11, 12, cleanup);
        assert!(discarded.is_empty());
        let paths = accepts(&loop_module(operations, true));
        assert_eq!(paths.max_acquisitions, 1);
        if matches!(cleanup, Cleanup::Parts) {
            assert!(paths.max_loads >= 3);
        }
    }
}

#[test]
fn invariant_live_scope_self_loop_does_not_require_a_termination_proof() {
    for cleanup in [Cleanup::Empty, Cleanup::Parts] {
        let candidate = module(vec![
            block(ENTRY, vec![issue(), acquire(11)], jump(HEADER)),
            block(HEADER, descendants(11, 12, cleanup).0, jump(HEADER)),
        ]);
        let paths = trace_paths(&candidate, 18).unwrap();
        assert_eq!(
            (paths.exits, paths.bounded, paths.max_acquisitions),
            (0, 1, 1)
        );
        assert_eq!(
            paths.max_loads,
            if matches!(cleanup, Cleanup::Parts) {
                17
            } else {
                0
            }
        );
        verify_module(&candidate).unwrap();
    }
}

fn diamond_loop(left: Cleanup, right: Cleanup) -> Module {
    let mut candidate = loop_module(vec![], false);
    block_mut(&mut candidate, BODY).terminator = Some(choose(21, 31));
    candidate.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .extend([
            block(21, iteration(100, left), jump(41)),
            block(31, iteration(200, right), jump(41)),
            block(41, vec![], jump(HEADER)),
        ]);
    candidate
}

#[test]
fn diamond_paths_and_sparse_block_orders_agree_with_dynamic_traces() {
    for left in CLEANUPS {
        for right in CLEANUPS {
            let mut candidate = diamond_loop(left, right);
            for _ in 0..2 {
                let paths = accepts(&candidate);
                assert!(paths.exits > 2 && paths.max_acquisitions >= 3, "{paths:?}");
                candidate.functions[0].body.as_mut().unwrap().blocks[1..].reverse();
            }
        }
    }
}

#[test]
fn nested_loops_keep_the_outer_scope_and_reuse_inner_descendant_slots() {
    let (children, _) = descendants(11, 12, Cleanup::Parts);
    let mut candidate = loop_module(vec![], true);
    block_mut(&mut candidate, BODY).terminator = Some(jump(21));
    candidate.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .extend([
            block(21, vec![], choose(31, 41)),
            block(31, children, jump(21)),
            block(41, vec![], jump(HEADER)),
        ]);
    let paths = accepts(&candidate);
    assert_eq!(paths.max_acquisitions, 1);
    assert!(paths.max_loads >= 3 && paths.exits > 2);
    // Also admit a fresh scope on each inner iteration, with no outer borrow.
    block_mut(&mut candidate, ENTRY).operations = vec![issue()];
    block_mut(&mut candidate, EXIT).operations.clear();
    block_mut(&mut candidate, 31).operations = iteration(11, Cleanup::Fragment);
    assert!(accepts(&candidate).max_acquisitions >= 3);
}

#[test]
fn multiple_entry_scc_and_switch_backedge_keep_invariant_ownership() {
    let mut candidate = loop_module(descendants(11, 12, Cleanup::Parts).0, true);
    block_mut(&mut candidate, HEADER).terminator = Some(choose(BODY, 21));
    block_mut(&mut candidate, BODY).terminator = Some(jump(21));
    candidate.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(block(
            21,
            descendants(11, 22, Cleanup::Parts).0,
            choose(BODY, EXIT),
        ));
    assert!(accepts(&candidate).max_loads >= 3);
    block_mut(&mut candidate, 21).terminator = Some(Terminator::Switch {
        selector: ValueId(1),
        cases: vec![SwitchCase {
            value: 0,
            target: BlockId(BODY),
            arguments: vec![],
        }],
        default_target: BlockId(EXIT),
        default_arguments: vec![],
    });
    assert!(accepts(&candidate).max_loads >= 3);
    // Removing consumption makes the same switch backedge fail equality.
    block_mut(&mut candidate, 21).operations.pop();
    rejects(&candidate, "states differ across a CFG join or backedge");
    assert!(trace_paths(&candidate, 18).is_err());
}

#[test]
fn context_issue_cannot_repeat_even_when_every_scope_is_balanced() {
    let mut candidate = loop_module(iteration(11, Cleanup::Empty), false);
    block_mut(&mut candidate, ENTRY).operations.clear();
    block_mut(&mut candidate, HEADER).operations.push(issue());
    assert_eq!(
        trace_paths(&candidate, 18).unwrap_err(),
        "context issued twice on one path"
    );
    rejects(&candidate, "states differ across a CFG join or backedge");
}

#[test]
fn scope_and_descendant_leaks_fail_on_backedges() {
    let mut scope = loop_module(vec![acquire(11)], false);
    // No exit is needed to detect the leak; the next iteration borrows twice.
    block_mut(&mut scope, HEADER).terminator = Some(jump(BODY));
    rejects(&scope, "states differ across a CFG join or backedge");
    assert_eq!(
        trace_paths(&scope, 18).unwrap_err(),
        "context unavailable for acquisition"
    );

    for cleanup in [Cleanup::Tile, Cleanup::Fragment] {
        let (children, _) = descendants(11, 12, cleanup);
        let mut candidate = loop_module(children, true);
        block_mut(&mut candidate, HEADER).terminator = Some(jump(BODY));
        // EXIT has execution operations, so keep it reachable from a later block.
        block_mut(&mut candidate, BODY).terminator = Some(choose(HEADER, EXIT));
        rejects(&candidate, "states differ across a CFG join or backedge");
        assert!(trace_paths(&candidate, 18).is_err());
    }
}

#[test]
fn divergent_diamond_and_consumed_outer_descendant_do_not_form_loop_phis() {
    let mut candidate = diamond_loop(Cleanup::Empty, Cleanup::Empty);
    block_mut(&mut candidate, 31).operations.pop();
    rejects(&candidate, "states differ across a CFG join or backedge");
    assert!(trace_paths(&candidate, 18).is_err());

    let (mut children, _) = descendants(11, 12, Cleanup::Parts);
    let consume = children.pop().unwrap();
    let mut candidate = loop_module(vec![], true);
    block_mut(&mut candidate, ENTRY).operations.extend(children);
    block_mut(&mut candidate, HEADER).operations.push(consume);
    rejects(&candidate, "states differ across a CFG join or backedge");
    assert_eq!(
        trace_paths(&candidate, 18).unwrap_err(),
        "fragment unavailable"
    );
}

#[test]
fn ending_then_reacquiring_does_not_revive_stale_descendants_or_early_uses() {
    let mut stale = loop_module(iteration(11, Cleanup::Tile), false);
    block_mut(&mut stale, BODY).operations.extend([
        acquire(100),
        descendants(11, 12, Cleanup::Parts).0.remove(1),
        end(100, &[]),
    ]);
    let mut early = loop_module(iteration(11, Cleanup::Parts), false);
    block_mut(&mut early, BODY).operations.swap(0, 1);
    for (candidate, expected) in [(&stale, "tile unavailable"), (&early, "load outside scope")] {
        assert_eq!(trace_paths(candidate, 18).unwrap_err(), expected);
        rejects(candidate, "execution operation violates");
    }
    let errors = verify_module(&early).unwrap_err();
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::NonDominatingUse)
    );
}

fn helper_call(candidate: &mut Module) -> Operation {
    candidate.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block(0, vec![], Terminator::Return { values: vec![] })],
    ));
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: "helper".into(),
            arguments: vec![],
        },
    )
}

#[test]
fn calls_and_exits_require_closed_scopes_on_every_loop_path() {
    let mut candidate = loop_module(iteration(11, Cleanup::Parts), false);
    let call = helper_call(&mut candidate);
    block_mut(&mut candidate, BODY)
        .operations
        .push(call.clone());
    accepts(&candidate);
    let body = block_mut(&mut candidate, BODY);
    body.operations.pop();
    body.operations.insert(1, call);
    rejects(&candidate, "execution operation violates");
    assert_eq!(
        trace_paths(&candidate, 18).unwrap_err(),
        "call with live execution ownership"
    );

    for terminator in [
        Terminator::Return { values: vec![] },
        Terminator::Unreachable,
    ] {
        for with_child in [false, true] {
            let mut candidate = loop_module(vec![], true);
            if with_child {
                block_mut(&mut candidate, ENTRY)
                    .operations
                    .extend(descendants(11, 12, Cleanup::Tile).0);
            }
            block_mut(&mut candidate, EXIT).operations.clear();
            block_mut(&mut candidate, EXIT).terminator = Some(terminator.clone());
            rejects(&candidate, "remains live at function exit");
            assert!(trace_paths(&candidate, 18).is_err());
        }
    }
    let mut candidate = loop_module(vec![], true);
    let trap = crate::AmdGpuDiagnosticOperation::Trap;
    candidate.functions.push(trap.declaration());
    block_mut(&mut candidate, EXIT)
        .operations
        .push(trap.operation(None));
    block_mut(&mut candidate, EXIT).terminator = Some(Terminator::Unreachable);
    accepts(&candidate);
    block_mut(&mut candidate, EXIT).operations.remove(0);
    rejects(&candidate, "execution operation violates");
}

#[test]
fn loop_carried_execution_parameters_remain_forbidden() {
    for (role, argument, outer) in [(Role::Context, 10, false), (Role::Workgroup, 11, true)] {
        let mut candidate = loop_module(vec![], outer);
        block_mut(&mut candidate, HEADER)
            .parameters
            .push(ValueDef::new(ValueId(100), Type::Execution(role)));
        block_mut(&mut candidate, ENTRY).terminator = Some(Terminator::Branch {
            target: BlockId(HEADER),
            arguments: vec![ValueId(argument)],
        });
        block_mut(&mut candidate, BODY).terminator = Some(Terminator::Branch {
            target: BlockId(HEADER),
            arguments: vec![ValueId(100)],
        });
        rejects(
            &candidate,
            "execution roles cannot be function or block parameters",
        );
    }
}

const FLOOR: usize = 37;
const WORK_PREFIX: usize = 19;

#[derive(Debug)]
struct Resources {
    result: Result<(), VerificationError>,
    work: usize,
    peak: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}

fn metered(module: &Module, work_limit: usize, storage_limit: usize) -> Resources {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(WORK_PREFIX).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    let result =
        crate::verify_depth_bounded_module_with_budget_v1(module, None, &mut budget).map(|_| ());
    assert_eq!(
        budget.storage(),
        FLOOR,
        "caller-owned floor must survive every outcome"
    );
    Resources {
        result,
        work: budget.work(),
        peak: budget.peak_storage(),
        denied_work: budget.work_budget_v1().failed_work(),
        denied_storage: budget.failed_storage(),
    }
}

#[test]
fn loops_have_exact_resource_boundaries_and_preserve_denied_prefixes() {
    let mut invalid = diamond_loop(Cleanup::Parts, Cleanup::Tile);
    block_mut(&mut invalid, 31).operations.pop();
    for (candidate, valid) in [
        (loop_module(iteration(11, Cleanup::Parts), false), true),
        (
            loop_module(descendants(11, 12, Cleanup::Parts).0, true),
            true,
        ),
        (diamond_loop(Cleanup::Tile, Cleanup::Fragment), true),
        (invalid, false),
    ] {
        let full = metered(&candidate, usize::MAX, usize::MAX);
        assert_eq!(full.result.is_ok(), valid, "{full:?}");
        if !valid {
            assert!(matches!(
                full.result,
                Err(VerificationError::Verification(_))
            ));
        }
        assert!(full.work > WORK_PREFIX && full.peak > FLOOR);
        assert_eq!((full.denied_work, full.denied_storage), (None, None));
        let exact = metered(&candidate, full.work, full.peak);
        assert_eq!(exact.result, full.result);
        assert_eq!((exact.work, exact.peak), (full.work, full.peak));
        assert_eq!((exact.denied_work, exact.denied_storage), (None, None));

        let short = metered(&candidate, full.work - 1, full.peak);
        let Err(VerificationError::Resource(ResourceError::Work(error))) = short.result else {
            panic!("expected one-short work denial: {short:?}");
        };
        assert_eq!((error.actual(), error.limit()), (full.work, full.work - 1));
        assert_eq!(short.denied_work, Some(full.work));
        assert!(short.work < error.actual() && short.work <= error.limit());
        assert!(short.peak <= full.peak);
        assert_eq!(short.denied_storage, None);

        let short = metered(&candidate, full.work, full.peak - 1);
        let Err(VerificationError::Resource(ResourceError::Storage(error))) = short.result else {
            panic!("expected one-short storage denial: {short:?}");
        };
        assert_eq!((error.actual(), error.limit()), (full.peak, full.peak - 1));
        assert_eq!(short.denied_storage, Some(full.peak));
        assert!(short.peak < error.actual() && short.work <= full.work);
        assert_eq!(short.denied_work, None);

        let denied = metered(&candidate, WORK_PREFIX, full.peak);
        assert!(matches!(
            denied.result,
            Err(VerificationError::Resource(ResourceError::Work(_)))
        ));
        assert_eq!((denied.work, denied.peak), (WORK_PREFIX, FLOOR));
        assert!(denied.denied_work.unwrap() > WORK_PREFIX);
        assert_eq!(denied.denied_storage, None);
        let denied = metered(&candidate, full.work, FLOOR);
        assert!(matches!(
            denied.result,
            Err(VerificationError::Resource(ResourceError::Storage(_)))
        ));
        assert_eq!(denied.peak, FLOOR);
        assert!(denied.denied_storage.unwrap() > FLOOR);
        assert_eq!(denied.denied_work, None);
    }
}

#[test]
fn rejected_backedge_keeps_diagnostic_storage_but_releases_lifecycle_scratch() {
    let candidate = loop_module(vec![acquire(11)], false);
    let function = &candidate.functions[0];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let flow = crate::analyze_control_flow_with_verification_budget_v1(
        function,
        crate::ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    let definitions = VerificationFunctionStateV1::build(function, &mut budget)
        .unwrap()
        .unwrap();
    let before_collector = budget.storage();
    let mut diagnostics = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
    let before_lifecycle = budget.storage();
    verify_execution_lifecycle_v15(
        &candidate,
        function,
        &definitions,
        Some(&flow),
        &mut diagnostics,
        &mut budget,
    )
    .unwrap();
    assert!(budget.storage() > before_lifecycle);
    let peak = budget.peak_storage();
    let diagnostics = diagnostics.finish_materialized(&mut budget).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("states differ across a CFG join or backedge")
    );
    assert_eq!(budget.storage(), before_collector);
    definitions.release(&mut budget).unwrap();
    flow.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), peak);
}
