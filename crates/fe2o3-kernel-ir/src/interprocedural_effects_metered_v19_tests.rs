use super::*;
use crate::*;
use std::collections::BTreeSet;

const FLOOR: usize = 37;
const LIMIT: usize = 100_000_000;

fn function(name: &str, operations: Vec<Operation>, memory: bool) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let parameters = if memory {
        vec![
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::Scalar(ScalarType::U32),
        ]
    } else {
        vec![]
    };
    let arguments = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut function = Function::internal_helper(
        name,
        Signature::new(parameters, vec![]),
        arguments,
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    function
}
fn call(name: &str, memory: bool) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(name),
            arguments: if memory {
                vec![ValueId(0), ValueId(1)]
            } else {
                vec![]
            },
        },
    )
}
fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("metered_effects_v19");
    module.required_capabilities.extend(
        functions
            .iter()
            .flat_map(|function| function.required_capabilities.iter().cloned()),
    );
    module.functions = functions;
    module
}
fn owner(module: &Module) -> (Owner, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
        module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap();
    (owner, receipt.retained_storage())
}
fn write() -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn read() -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn parity(owner: &Owner, scope: &CanonicalEffectScopeV19<'_>, budget: &mut Budget<'_>) {
    let old = crate::interprocedural_effects::analyze_verified_effects_v1(owner.module()).unwrap();
    for function in &owner.module().functions {
        let expected = old.function(&function.id).unwrap();
        let actual = scope.function(owner, function, budget).unwrap();
        assert!(std::ptr::eq(actual.original_function(), function));
        assert_eq!(actual.is_complete(), expected.is_complete());
        assert_eq!(
            actual.is_complete_and_pure(),
            expected.is_complete_and_pure()
        );
        assert_eq!(
            actual.compiler_ordering(),
            expected.summary().compiler_ordering()
        );
        assert_eq!(
            actual
                .effects()
                .map(Effect::to_owned)
                .collect::<BTreeSet<_>>(),
            expected.summary().effects().clone()
        );
        let expected_reasons = expected
            .incomplete_reasons()
            .iter()
            .map(|reason| match reason {
                InterproceduralEffectIncompleteReasonV1::FunctionDeclaration { function } => {
                    CanonicalEffectReasonV19::Declaration(function)
                }
                InterproceduralEffectIncompleteReasonV1::RecursiveCallCycle { function } => {
                    CanonicalEffectReasonV19::Recursive(function)
                }
                InterproceduralEffectIncompleteReasonV1::InlineAssembly { function, location } => {
                    CanonicalEffectReasonV19::Assembly(function, *location)
                }
                InterproceduralEffectIncompleteReasonV1::ResourceLimit { .. } => {
                    panic!("small parity fixture must not reach a local cap")
                }
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual
                .incomplete_reasons()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
            expected_reasons
        );
    }
}

#[test]
fn actual_owner_pure_and_memory_call_dags_preserve_existing_effect_decisions() {
    for memory in [false, true] {
        let leaf = if memory {
            vec![write(), read(), write()]
        } else {
            vec![]
        };
        let module = module(vec![
            function(
                "root",
                vec![call("middle", memory), call("leaf", memory)],
                memory,
            ),
            function(
                "middle",
                vec![call("leaf", memory), call("leaf", memory)],
                memory,
            ),
            function("leaf", leaf, memory),
        ]);
        let (owner, retained) = owner(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            parity(&owner, scope, budget);
            let root = scope.function_named(&owner, &FunctionId::new("root"), budget)?;
            assert_eq!(root.is_complete_and_pure(), !memory);
            assert_eq!(root.effects().count(), if memory { 2 } else { 0 });
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn actual_owner_recursion_and_declaration_reasons_propagate_without_purity() {
    for recursive in [false, true] {
        let leaf = if recursive {
            function("leaf", vec![call("root", false)], false)
        } else {
            Function::declaration("leaf", Signature::new(vec![], vec![]))
        };
        let (owner, retained) = owner(&module(vec![
            function("root", vec![call("leaf", false)], false),
            leaf,
        ]));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            parity(&owner, scope, budget);
            let root = scope.function_named(&owner, &FunctionId::new("root"), budget)?;
            assert!(!root.is_complete_and_pure());
            assert_eq!(root.incomplete_reasons().len(), 1);
            assert!(
                matches!(
                    root.incomplete_reasons()[0],
                    CanonicalEffectReasonV19::Recursive(_)
                ) == recursive
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn exact_owner_and_actual_function_queries_reject_equal_byte_foreign_custody_stickily() {
    let module = module(vec![function("root", vec![], false)]);
    let (owner, retained) = owner(&module);
    let (foreign, _) = self::owner(&module);
    for use_foreign_owner in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            let result = if use_foreign_owner {
                scope.function_named(&foreign, &owner.module().functions[0].id, budget)
            } else {
                scope.function(&owner, &foreign.module().functions[0], budget)
            };
            assert!(matches!(result, Err(Error::ForeignOwner)));
            assert!(matches!(
                scope.function(&owner, &owner.module().functions[0], budget),
                Err(Error::ForeignOwner)
            ));
            completed.set(true);
            Ok(())
        });
        assert_eq!(result, Err(Error::ForeignOwner));
        assert!(completed.get());
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn actual_search_and_repeated_queries_have_exact_and_one_short_cumulative_boundaries() {
    let (owner, retained) = owner(&module(vec![
        function("root", vec![call("leaf", true)], true),
        function("leaf", vec![write(), read()], true),
    ]));
    let run = |work_limit: usize, storage_limit: usize| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            for _ in 0..3 {
                let summary = scope.function_named(&owner, &FunctionId::new("root"), budget)?;
                assert!(summary.is_complete());
                assert_eq!(summary.effects().count(), 2);
            }
            completed.set(true);
            Ok(())
        });
        assert_eq!(budget.storage(), FLOOR + retained);
        assert_eq!(result.is_ok(), completed.get());
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, peak) = run(LIMIT, LIMIT);
    result.unwrap();
    run(work, peak).0.unwrap();
    assert!(matches!(
        run(work - 1, peak).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn independent_empty_owner_entry_formula_and_capture_headers_are_exact() {
    let (owner, retained) = owner(&module(vec![]));
    fn consume(_: &CanonicalEffectScopeV19<'_>, _: &mut Budget<'_>) -> Result<()> {
        Ok(())
    }
    // Entry4, empty function census1, three typed vectors1 each. Empty bounded
    // sort performs no comparison and has no allocation or work charge.
    const WORK: usize = 8;
    let headers = size_of::<CanonicalEffectScopeV19<'_>>()
        + size_of::<Builder<'_>>()
        + size_of::<Result<CanonicalEffectDecisionV19<'_, '_>>>()
        + size_of::<Option<fn(&CanonicalEffectScopeV19<'_>, &mut Budget<'_>) -> Result<()>>>()
        + 2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>();
    let peak = FLOOR
        + retained
        + headers
        + size_of::<Vec<usize>>()
        + size_of::<Vec<Decision<'_>>>()
        + size_of::<Vec<Frame>>();
    for (work_limit, storage_limit) in [(WORK, peak), (WORK - 1, peak), (WORK, peak - 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let result = with_canonical_effects_v19(
            &owner,
            &mut budget,
            consume as fn(&CanonicalEffectScopeV19<'_>, &mut Budget<'_>) -> Result<()>,
        );
        assert_eq!(result.is_ok(), work_limit == WORK && storage_limit == peak);
        if result.is_ok() {
            assert_eq!((budget.work(), budget.peak_storage()), (WORK, peak));
        }
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn swallowed_budget_denial_and_callback_errors_preserve_first_failure_and_floor() {
    let (owner, retained) = owner(&module(vec![function("root", vec![], false)]));
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let completed = Cell::new(false);
        let expected = Cell::new(None);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            if mode == 0 {
                let error = budget.charge_work(LIMIT).unwrap_err();
                expected.set(Some(Error::Resource(error)));
                assert!(matches!(
                    scope.function(&owner, &owner.module().functions[0], budget),
                    Err(Error::Resource(_))
                ));
            } else {
                expected.set(Some(if mode == 1 {
                    Error::Consumer
                } else {
                    Error::Panicked
                }));
            }
            completed.set(true);
            if mode == 2 {
                std::panic::resume_unwind(Box::new(23u32));
            }
            if mode == 1 {
                Err(Error::Consumer)
            } else {
                Ok(())
            }
        });
        assert!(completed.get());
        assert_eq!(result, Err(expected.get().unwrap()));
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn actual_closed_and_nonclosed_assembly_keep_the_existing_typed_policy() {
    use Gfx942InlineAssemblyInstructionV1 as Instruction;
    for kind in [
        Instruction::VMovB32,
        Instruction::VAddU32,
        Instruction::VSubU32,
        Instruction::VAndB32,
        Instruction::VOrB32,
        Instruction::VXorB32,
        Instruction::SMovB32,
    ] {
        let mut operands = vec![AssemblyOperand::output(0, kind.constraint())];
        operands.extend(
            (0..kind.input_count())
                .map(|index| AssemblyOperand::input(ValueId(index as u32), kind.constraint())),
        );
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                mnemonic: kind.mnemonic().to_owned(),
                operands,
                options: BTreeSet::from([AssemblyOption::NoMemory]),
                declared_effects: BTreeSet::new(),
            }),
        );
        let mut function = function("assembly", vec![operation], false);
        function.signature.parameters = vec![Type::Scalar(ScalarType::U32); 2];
        function.body.as_mut().unwrap().parameters = vec![ValueId(0), ValueId(1)];
        function.required_capabilities = function.derived_capabilities();
        let (owner, retained) = owner(&module(vec![function]));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            parity(&owner, scope, budget);
            let decision = scope.function(&owner, &owner.module().functions[0], budget)?;
            assert_eq!(
                decision.is_complete_and_pure(),
                kind != Instruction::SMovB32
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn foreign_slot_and_changed_ledger_queries_leave_original_account_usable() {
    let (owner, retained) = owner(&module(vec![function("root", vec![], false)]));
    for replace_ledger in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        foreign.reserve_storage(19).unwrap();
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            if replace_ledger {
                std::mem::swap(budget, &mut foreign);
                assert!(matches!(
                    scope.function(&owner, &owner.module().functions[0], budget),
                    Err(Error::Resource(Resource::Accounting))
                ));
                std::mem::swap(budget, &mut foreign);
            } else {
                assert!(matches!(
                    scope.function(&owner, &owner.module().functions[0], &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
            }
            assert!(
                scope
                    .function(&owner, &owner.module().functions[0], budget)?
                    .is_complete_and_pure()
            );
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert_eq!(result, Ok(()));
        assert_eq!(budget.storage(), FLOOR + retained);
        assert_eq!(foreign.storage(), 19);
    }
}

#[test]
fn rejected_owned_capture_drop_cannot_mask_prior_entry_denial() {
    struct PanickingCapture(std::rc::Rc<Cell<usize>>);
    impl Drop for PanickingCapture {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            std::panic::resume_unwind(Box::new(101u32));
        }
    }
    let (owner, retained) = owner(&module(vec![]));
    let drops = std::rc::Rc::new(Cell::new(0));
    let capture = PanickingCapture(drops.clone());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + retained).unwrap();
    let result = with_canonical_effects_v19(&owner, &mut budget, move |_, _| {
        drop(capture);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR + retained);
}

#[test]
fn rejected_result_drop_and_later_floor_undercut_preserve_original_query_denial() {
    struct Output(std::rc::Rc<Cell<usize>>);
    impl Drop for Output {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            std::panic::resume_unwind(Box::new(103u32));
        }
    }
    let (owner, retained) = owner(&module(vec![function("root", vec![], false)]));
    let drops = std::rc::Rc::new(Cell::new(0));
    let completed = Cell::new(false);
    let expected = Cell::new(None);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + retained).unwrap();
    let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
        let error = budget.charge_work(LIMIT).unwrap_err();
        expected.set(Some(Error::Resource(error)));
        assert!(matches!(
            scope.function(&owner, &owner.module().functions[0], budget),
            Err(Error::Resource(_))
        ));
        budget.release_storage(1)?;
        assert!(matches!(
            scope.function(&owner, &owner.module().functions[0], budget),
            Err(Error::Resource(_))
        ));
        completed.set(true);
        Ok(Output(drops.clone()))
    });
    assert!(completed.get());
    assert!(matches!(result, Err(error) if Some(error) == expected.get()));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR + retained);
}

#[test]
fn aligned_owned_capture_is_paid_before_callback_entry() {
    #[repr(align(64))]
    struct Capture([u8; 129]);
    let (owner, retained) = owner(&module(vec![]));
    let reached = Cell::new(false);
    let observed = &reached;
    let capture = Capture([7; 129]);
    let consume = move |_: &CanonicalEffectScopeV19<'_>, _: &mut Budget<'_>| {
        std::hint::black_box(&capture);
        observed.set(true);
        Ok(())
    };
    fn header<T, F>(_: &F) -> usize {
        size_of::<CanonicalEffectScopeV19<'_>>()
            + size_of::<Builder<'_>>()
            + size_of::<Result<CanonicalEffectDecisionV19<'_, '_>>>()
            + size_of::<Option<F>>()
            + 2 * size_of::<std::thread::Result<Result<T>>>()
            + size_of::<std::thread::Result<()>>()
    }
    let bytes = header::<(), _>(&consume);
    assert!(std::mem::size_of_val(&consume) >= 192);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, FLOOR + retained + bytes - 1);
    budget.reserve_storage(FLOOR + retained).unwrap();
    let result = with_canonical_effects_v19(&owner, &mut budget, consume);
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
    assert_eq!(budget.work(), 4);
    assert_eq!(budget.peak_storage(), FLOOR + retained);
    assert_eq!(budget.storage(), FLOOR + retained);
    assert!(!reached.get());
}

#[test]
fn sorted_union_work_and_old_new_capacity_overlap_have_independent_boundaries() {
    for work_limit in [10, 9] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit + 1);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut original = allocate_vector_v2::<usize>(2, &mut budget).unwrap();
        original.extend([1, 3]);
        let old_capacity = original.capacity();
        let result = merge_sorted(&mut original, &[2, 3], 2, Ord::cmp, &mut budget);
        if work_limit == 10 {
            result.unwrap();
            assert_eq!(original, [1, 2, 3]);
            assert_eq!(budget.work(), 11);
            assert_eq!(
                budget.storage(),
                FLOOR + size_of::<Vec<usize>>() + original.capacity() * size_of::<usize>()
            );
            assert_eq!(
                budget.peak_storage(),
                FLOOR
                    + 2 * size_of::<Vec<usize>>()
                    + (old_capacity + original.capacity()) * size_of::<usize>()
            );
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
    }
}

#[test]
fn wide_independent_functions_share_one_index_and_logarithmic_allocation_free_queries() {
    for memory in [false, true] {
        for count in [1usize, 16, 64, 256] {
            let module = module(
                (0..count)
                    .map(|index| {
                        function(
                            &format!("f{index:04}"),
                            if memory { vec![write()] } else { vec![] },
                            memory,
                        )
                    })
                    .collect(),
            );
            let (owner, retained) = owner(&module);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + retained).unwrap();
            with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
                // Entry/census/three vectors: 8. Per function: one census unit,
                // six name units, and six DFS/empty-body units. The one bounded
                // heapsort prepays 4*N*ceil(log2 N)*7 for five-byte names.
                let ceil_log = if count < 2 {
                    0
                } else {
                    usize::BITS as usize - (count - 1).leading_zeros() as usize
                };
                // One Store adds one preflight visit, one DFS visit, one effect
                // row, one insertion, and one actual typed capacity allocation.
                assert_eq!(
                    budget.work(),
                    8 + (if memory { 18 } else { 13 }) * count + 28 * count * ceil_log
                );
                let live = (budget.storage(), budget.peak_storage());
                let comparisons = usize::BITS as usize - count.leading_zeros() as usize;
                for _ in 0..3 {
                    for function in owner.module().functions.iter().rev() {
                        let before = budget.work();
                        let decision = scope.function_named(&owner, &function.id, budget)?;
                        assert!(decision.is_complete());
                        assert_eq!(decision.is_complete_and_pure(), !memory);
                        assert_eq!(decision.effects().count(), usize::from(memory));
                        assert!(std::ptr::eq(decision.original_function(), function));
                        assert!(budget.work() - before <= 1 + 7 * comparisons);
                        assert_eq!((budget.storage(), budget.peak_storage()), live);
                    }
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), FLOOR + retained);
        }
    }
}

#[test]
fn actual_call_traversal_preserves_cumulative_local_cap_without_purity() {
    let (owner, retained) = owner(&module(vec![
        function(
            "root",
            vec![call("leaf", false), call("leaf", false)],
            false,
        ),
        function("leaf", vec![], false),
    ]));
    for preceding_calls in [
        MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 - 2,
        MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 - 1,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        // Seed only the cumulative local policy counter, representing previously
        // visited functions. The actual two owner-borrowed calls are traversed.
        let mut by_name = allocate_vector_v2(2, &mut budget).unwrap();
        by_name.extend([1, 0]);
        let mut decisions = allocate_vector_v2(2, &mut budget).unwrap();
        decisions.extend([Decision::new(), Decision::new()]);
        let frames = allocate_vector_v2(2, &mut budget).unwrap();
        let mut builder = Builder {
            owner: &owner,
            by_name,
            decisions,
            frames,
            call_edges: preceding_calls,
            name_work: 20,
            assembly: AssemblyTypeBudgetV1 {
                used: 0,
                limit: MAX_INTERPROCEDURAL_ASSEMBLY_TYPE_WORK_V1,
            },
        };
        builder.enter(0, &mut budget).unwrap();
        builder.walk(&mut budget).unwrap();
        let root = &builder.decisions[0];
        assert_eq!(root.state, 2);
        assert!(root.effects.is_empty());
        assert!(builder.decisions[1].reasons.is_empty());
        assert_eq!(builder.call_edges, preceding_calls + 2);
        if preceding_calls == MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 - 2 {
            assert!(root.reasons.is_empty());
        } else {
            assert_eq!(
                root.reasons,
                [CanonicalEffectReasonV19::CallLimit(
                    MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 + 1
                )]
            );
        }
        drop(builder);
        budget
            .release_storage(budget.storage() - FLOOR - retained)
            .unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}
