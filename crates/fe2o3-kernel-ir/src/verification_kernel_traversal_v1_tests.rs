use super::*;
use crate::{
    Axis, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Diagnostic, DiagnosticLocation,
    IntrinsicKind, IntrinsicOperation, KernelId, LaunchDomain, ModuleId, Operation, Signature,
    Terminator, ValueDef, ValueId,
};

fn domain() -> LaunchDomain {
    LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }
}

fn return_block() -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn disconnected_kernels(count: usize) -> Module {
    let mut module = Module::new("m");
    for ordinal in 0..count {
        let id = format!("f{ordinal:04}");
        module.functions.push(Function::kernel_entry(
            id.clone(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![return_block()],
        ));
        module
            .kernels
            .push(Kernel::new(format!("k{ordinal:04}"), id, domain()));
    }
    module
}

#[test]
fn disconnected_kernel_closures_have_linear_exact_work_and_one_under_boundaries() {
    const FLOOR: usize = 7;
    for (count, height) in [(1, 0), (2, 1), (16, 4), (128, 7)] {
        let module = disconnected_kernels(count);
        // All IDs have five bytes. Census/fill costs 4*N; the three
        // in-place index sorts prepay 4*N*height*(7 + 7 + 6).
        let index_work = 4 * count + 80 * count * height;
        let index_storage = 5 * count;
        // One initial N-cell zero-fill, then generation/entry/publication3,
        // pop1 and one block visit1 for each disconnected closure.
        let traversal_work = count + 5 * count;
        let exact_work = index_work + traversal_work;
        let exact_storage = FLOOR + index_storage + 2 * count;
        for (work_limit, storage_limit) in [
            (exact_work, exact_storage),
            (exact_work - 1, exact_storage),
            (exact_work, exact_storage - 1),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let module_state = VerificationModuleStateV1::build(&module, &mut budget).unwrap();
            assert_eq!(budget.work(), index_work);
            assert_eq!(budget.storage(), FLOOR + index_storage);
            let mut traversal = VerificationKernelTraversalV1::new();
            let mut diagnostics = VerificationDiagnosticCollectorV1::count();
            let mut first_buffers = None;
            let result = (|| {
                for ordinal in 0..count {
                    verify_reachable_intrinsic_axes_v1(
                        &module,
                        &module.kernels[ordinal],
                        ordinal,
                        &module_state,
                        &mut traversal,
                        &mut diagnostics,
                        &mut budget,
                    )?;
                    let buffers = (
                        traversal.visited_generations.as_ptr(),
                        traversal.pending.as_ptr(),
                    );
                    if let Some(first_buffers) = first_buffers {
                        assert_eq!(buffers, first_buffers);
                    } else {
                        first_buffers = Some(buffers);
                    }
                    assert_eq!(traversal.generation, ordinal + 1);
                    assert!(traversal.pending.is_empty());
                    assert_eq!(traversal.visited_generations.capacity(), count);
                    assert_eq!(traversal.pending.capacity(), count);
                }
                Ok::<_, CanonicalKernelIrVerificationResourceErrorV1>(())
            })();
            if storage_limit < exact_storage {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == exact_storage && error.limit() == storage_limit
                ));
                assert_eq!(budget.work(), index_work + count + 3);
                assert_eq!(budget.storage(), FLOOR + index_storage);
                assert_eq!(budget.peak_storage(), FLOOR + index_storage);
                assert_eq!(budget.failed_storage(), Some(exact_storage));
                assert_eq!(traversal.generation, 0);
            } else {
                if work_limit < exact_work {
                    assert!(matches!(
                        result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                            if error.actual() == exact_work && error.limit() == work_limit
                    ));
                    assert_eq!(budget.work(), exact_work - 1);
                } else {
                    assert_eq!(result, Ok(()));
                    assert_eq!(budget.work(), exact_work);
                    assert_eq!(budget.work() - index_work, 6 * count);
                }
                assert_eq!(budget.storage(), exact_storage);
                assert_eq!(budget.peak_storage(), exact_storage);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(diagnostics.counted(), Some(0));
            traversal.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR + index_storage);
            module_state.release(&mut budget).unwrap();
            diagnostics.abandon(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(
                work.failed_work(),
                (work_limit < exact_work).then_some(exact_work)
            );
        }
    }
}

#[test]
fn traversal_setup_denial_does_not_allocate_or_advance_generation() {
    // Sixteen zero-fill actions plus generation/entry/publication3.
    for (work_limit, storage_limit, rejected) in [(18, 39, "work"), (19, 38, "storage")] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(7).unwrap();
        let mut traversal = VerificationKernelTraversalV1::new();
        let result = traversal.begin_kernel(16, 0, &mut budget);
        match rejected {
            "work" => {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == 19 && error.limit() == 18
                ));
                assert_eq!(budget.work(), 0);
            }
            "storage" => {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == 39 && error.limit() == 38
                ));
                assert_eq!(budget.work(), 19);
            }
            _ => unreachable!(),
        }
        assert_eq!(traversal.generation, 0);
        assert_eq!(traversal.retained_storage, 0);
        assert_eq!(traversal.visited_generations.capacity(), 0);
        assert_eq!(traversal.pending.capacity(), 0);
        traversal.release(&mut budget).unwrap();
        assert_eq!((budget.storage(), budget.peak_storage()), (7, 7));
    }
}

#[test]
fn complete_shared_pass_reuses_one_kernel_traversal_owner() {
    const FLOOR: usize = 7;
    // Function index2; kernel census/fill4; two sorts of widths4 and2.
    const MODULE_INDEX_WORK: usize = 2 + 2 + 2 + 4 * 2 * 4 + 4 * 2 * 2;
    const FUNCTION_HEADER_WORK: usize = 5 + 5 + 4 + 1 + 1;
    const CFG_WORK: usize = 29 + 7 + 14 + 9 + 24 + 20;
    const FUNCTION_STATE_WORK: usize = 1 + 3 + 1;
    const FUNCTION_PASS_WORK: usize = 5 + 1 + 1 + 1 + 5 + 1 + 1;
    // Kernel location5, capability location5, domain1, shape1, entry lookup5.
    const KERNEL_HEADER_WORK: usize = 5 + 5 + 1 + 1 + 5;
    const TRAVERSAL_WORK: usize = 1 + 2 * (3 + 1 + 1);
    const EXACT_WORK: usize = MODULE_INDEX_WORK
        + 5
        + 1
        + 1
        + FUNCTION_HEADER_WORK
        + CFG_WORK
        + FUNCTION_STATE_WORK
        + FUNCTION_PASS_WORK
        + (2 + 3)
        + 2
        + 2 * KERNEL_HEADER_WORK
        + TRAVERSAL_WORK
        + 1
        + 5;
    // The module index retains8; the single-block CFG reaches16. The
    // two-cell shared traversal owner is smaller and dies before orphan checks.
    const EXACT_STORAGE: usize = FLOOR + 8 + 16;
    assert_eq!(EXACT_WORK, 258);
    assert_eq!(EXACT_STORAGE, 31);

    let mut module = Module::new("m");
    module.functions.push(Function::kernel_entry(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![return_block()],
    ));
    module.kernels = vec![
        Kernel::new("k0", "f", domain()),
        Kernel::new("k1", "f", domain()),
    ];
    for work_limit in [EXACT_WORK, EXACT_WORK - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut diagnostics = VerificationDiagnosticCollectorV1::count();
        let result = run_verification_pass_v1(&module, None, &mut diagnostics, &mut budget);
        if work_limit < EXACT_WORK {
            assert!(matches!(
                result,
                Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                    if error.actual() == EXACT_WORK && error.limit() == work_limit
            ));
            // The last referenced-entry comparison admits two cells of work.
            assert_eq!(budget.work(), EXACT_WORK - 2);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.work(), EXACT_WORK);
        }
        assert_eq!(diagnostics.counted(), Some(0));
        diagnostics.abandon(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), EXACT_STORAGE);
        assert_eq!(
            work.failed_work(),
            (work_limit < EXACT_WORK).then_some(EXACT_WORK)
        );
    }
}

#[test]
fn exhausted_generation_fails_without_wrapping_or_losing_scratch_ownership() {
    let module = disconnected_kernels(1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(13);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 14);
    budget.reserve_storage(7).unwrap();
    // Singleton module index4, one closure6, failed generation advance3.
    let module_state = VerificationModuleStateV1::build(&module, &mut budget).unwrap();
    let mut diagnostics = VerificationDiagnosticCollectorV1::count();
    let mut traversal = VerificationKernelTraversalV1::new();
    verify_reachable_intrinsic_axes_v1(
        &module,
        &module.kernels[0],
        0,
        &module_state,
        &mut traversal,
        &mut diagnostics,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.work(), 10);
    traversal.generation = usize::MAX;
    assert_eq!(
        traversal.begin_kernel(1, 0, &mut budget),
        Err(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
    );
    assert_eq!(traversal.generation, usize::MAX);
    assert_eq!(traversal.visited_generations, [1]);
    assert!(traversal.pending.is_empty());
    assert_eq!((budget.work(), budget.storage()), (13, 14));
    traversal.release(&mut budget).unwrap();
    module_state.release(&mut budget).unwrap();
    diagnostics.abandon(&mut budget).unwrap();
    assert_eq!(budget.storage(), 7);
}

fn call(callee: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: vec![],
        },
    )
}

fn axis_query(axis: Axis) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(0), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::LaunchExtent { axis },
            Type::INDEX,
        )),
    )
}

#[test]
fn shared_closures_preserve_cycles_dead_blocks_and_per_kernel_diagnostics() {
    let mut module = Module::new("m");
    for (entry, kernel) in [("entry0", "k0"), ("entry1", "k1")] {
        let mut block = return_block();
        block.operations = vec![call("helper"), call("helper")];
        module.functions.push(Function::kernel_entry(
            entry,
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(kernel, entry, domain()));
    }
    let mut dead_block = return_block();
    dead_block.id = BlockId(1);
    dead_block.operations = vec![axis_query(Axis::Y), call("entry0")];
    module.functions.push(Function::definition(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![return_block(), dead_block],
    ));
    let mut unused_block = return_block();
    unused_block.operations = vec![axis_query(Axis::Z), call("missing")];
    module.functions.push(Function::definition(
        "unused",
        Signature::new(vec![], vec![]),
        vec![],
        vec![unused_block],
    ));

    let mut expected = ["k0", "k1"]
        .into_iter()
        .map(|kernel| Diagnostic {
            location: DiagnosticLocation {
                module: ModuleId::new("m"),
                function: Some(FunctionId::new("helper")),
                kernel: Some(KernelId::new(kernel)),
                block: Some(BlockId(1)),
                operation: Some(0),
            },
            code: DiagnosticCode::InvalidLaunchDomain,
            message: format!("axis Y is outside the 1D launch domain of kernel {kernel}"),
        })
        .collect::<Vec<_>>();
    // Unreachable functions still receive ordinary semantic checks. Their
    // axes are not attributed to a kernel whose closure cannot reach them.
    expected.push(Diagnostic {
        location: DiagnosticLocation {
            module: ModuleId::new("m"),
            function: Some(FunctionId::new("unused")),
            kernel: None,
            block: Some(BlockId(0)),
            operation: Some(1),
        },
        code: DiagnosticCode::UnknownCallee,
        message: "callee missing is not in the module".to_owned(),
    });
    assert_eq!(
        crate::verify_module(&module).unwrap_err().diagnostics(),
        expected
    );
    assert_eq!(
        crate::verify_module_ref(&module).unwrap_err().diagnostics(),
        expected
    );
    assert_eq!(
        crate::verify_module_with_capabilities(&module, &BTreeSet::new())
            .unwrap_err()
            .diagnostics(),
        expected
    );

    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(7).unwrap();
    let result = verify_exact_decoded_module_with_budget_v1(&module, None, &mut budget);
    let Err(MeteredKernelIrVerificationErrorV1::Verification(errors)) = result else {
        panic!("expected complete kernel and unused-function diagnostics");
    };
    assert_eq!(errors.diagnostics(), expected);
    assert_eq!(budget.storage(), 7);
}
