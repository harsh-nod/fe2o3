use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1, IndexKind, IntrinsicKind, IntrinsicOperation,
    Kernel, Signature, StorageLayoutLimitsV1, ValueDef,
};

#[path = "canonical_selected_slice_resources_v30_tests.rs"]
mod resources;

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn pointer(space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(scalar(), space, access)
}

fn branch(target: u32, argument: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![ValueId(argument)],
    })
}

pub(super) fn fixture(
    generic_root: bool,
    writing: bool,
    parallel: bool,
    recurrence: bool,
) -> Module {
    let permission = if writing {
        AccessMode::ReadWrite
    } else {
        AccessMode::ReadOnly
    };
    let second = if generic_root {
        AddressSpace::Generic
    } else {
        AddressSpace::Global
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations.push(op(
        4,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut a = BasicBlock::new(BlockId(20));
    a.operations = vec![
        op(
            5,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            6,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
    ];
    a.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(40),
        then_arguments: vec![],
        else_target: BlockId(80),
        else_arguments: vec![],
    });
    let mut b = BasicBlock::new(BlockId(30));
    b.operations = vec![
        op(
            7,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(1) },
        ),
        op(
            8,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(7),
            },
        ),
    ];
    b.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(8),
        then_target: BlockId(50),
        then_arguments: vec![],
        else_target: BlockId(80),
        else_arguments: vec![],
    });
    let mut inject_a = BasicBlock::new(BlockId(40));
    inject_a.operations = vec![
        op(
            9,
            pointer(AddressSpace::Global, permission),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            10,
            pointer(AddressSpace::Global, permission),
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(4),
            },
        ),
        op(
            11,
            pointer(AddressSpace::Generic, permission),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(10),
                to: pointer(AddressSpace::Generic, permission),
            },
        ),
    ];
    inject_a.terminator = if parallel {
        Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(60),
            then_arguments: vec![ValueId(11)],
            else_target: BlockId(60),
            else_arguments: vec![ValueId(11)],
        })
    } else {
        branch(60, 11)
    };
    let mut inject_b = BasicBlock::new(BlockId(50));
    inject_b.operations = vec![
        op(
            12,
            pointer(second, permission),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        op(
            13,
            pointer(second, permission),
            OperationKind::GetElementPointer {
                base: ValueId(12),
                offset: ValueId(4),
            },
        ),
    ];
    if !generic_root {
        inject_b.operations.push(op(
            14,
            pointer(AddressSpace::Generic, permission),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(13),
                to: pointer(AddressSpace::Generic, permission),
            },
        ));
    }
    inject_b.terminator = branch(60, if generic_root { 13 } else { 14 });
    let mut joined = BasicBlock::new(BlockId(60));
    joined.parameters.push(ValueDef::new(
        ValueId(15),
        pointer(AddressSpace::Generic, permission),
    ));
    joined.operations.push(op(
        16,
        scalar(),
        OperationKind::Load {
            pointer: ValueId(15),
            access: MemoryAccess::new(AddressSpace::Generic, 4),
        },
    ));
    if writing {
        joined.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(15),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ));
    }
    joined.terminator = if recurrence {
        Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(60),
            then_arguments: vec![ValueId(15)],
            else_target: BlockId(80),
            else_arguments: vec![],
        })
    } else {
        Some(Terminator::Return { values: vec![] })
    };
    let mut exit = BasicBlock::new(BlockId(80));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("selected-conditional-v30");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar(), AddressSpace::Global, permission),
                Type::slice(scalar(), second, permission),
                Type::BOOL,
                scalar(),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, a, b, inject_a, inject_b, joined, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 128,
                containment_depth: 8,
                object_bytes: 1024,
            },
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 3,
        extents: [64, 1, 1],
    }
}

fn run<T>(
    module: &Module,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalSelectedSliceDomainsV30<'s, 'g>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> (Result<Option<T>>, usize, usize) {
    let (owner, credit) = owner(module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_selected_slice_domains_v30(
        &owner,
        &[launch()],
        FormalIndexWidth::Bits64,
        Default::default(),
        &mut budget,
        consume,
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

fn at(operation: u32) -> Coordinate {
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(0),
            block: 5,
        },
        operation,
    }
}

fn inspect(
    batch: &CheckedCanonicalSelectedSliceDomainsV30<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    let accesses = batch.accesses(FunctionCoordinate(0), budget)?;
    let incoming = batch.incoming_edges(FunctionCoordinate(0), budget)?;
    let nodes = batch.pointer_nodes(FunctionCoordinate(0), budget)?;
    let mut total = 0;
    for access in accesses {
        let choices = batch.choices_at(access.operation(), budget)?.unwrap();
        assert!(!choices.is_empty());
        for choice in choices {
            assert_eq!(choice.domain.writing(), access.writing());
            assert_eq!(nodes[choice.leaf].value, choice.domain.pointer());
            let CanonicalSelectedSliceInjectionV30::Incoming(edge) = choice.injection else {
                panic!("joined access has no direct leaf");
            };
            assert!(incoming[edge].reachable);
            assert!(
                !choice.at_access,
                "arm-specific guard must not become unconditional"
            );
            total += 1;
        }
    }
    assert!(!batch.grants_artifact_or_launch_authority());
    assert!(!batch.source_and_runtime_requirements_are_discharged());
    Ok(total)
}

#[test]
fn selected_slice_domains_keep_ordered_diamond_parallel_and_loop_choices() {
    for generic in [false, true] {
        for parallel in [false, true] {
            for recurrence in [false, true] {
                let module = fixture(generic, true, parallel, recurrence);
                let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
                    assert_eq!(batch.function_count(budget)?, 1);
                    assert_eq!(
                        batch.function_conditions(FunctionCoordinate(0), budget)?,
                        (launch(), FormalIndexWidth::Bits64, 1, 1)
                    );
                    assert_eq!(inspect(batch, budget)?, 2 * (2 + usize::from(parallel)));
                    let incoming = batch.incoming_edges(FunctionCoordinate(0), budget)?;
                    assert_eq!(
                        incoming.len(),
                        2 + usize::from(parallel) + usize::from(recurrence)
                    );
                    let choices = batch.choices_at(at(0), budget)?.unwrap();
                    let roots = choices
                        .iter()
                        .map(|row| row.domain.allocation().parameter_index())
                        .collect::<Vec<_>>();
                    assert_eq!(roots, if parallel { vec![0, 0, 1] } else { vec![0, 1] });
                    assert_eq!(choices[0].root_space, AddressSpace::Global);
                    assert_eq!(
                        choices.last().unwrap().root_space,
                        if generic {
                            AddressSpace::Generic
                        } else {
                            AddressSpace::Global
                        }
                    );
                    if parallel {
                        let ordinals = incoming
                            .iter()
                            .take(2)
                            .map(|row| row.occurrence.edge.successor)
                            .collect::<Vec<_>>();
                        assert_eq!(ordinals, [0, 1]);
                    }
                    for row in batch
                        .parameters(FunctionCoordinate(0), budget)?
                        .iter()
                        .flatten()
                    {
                        assert!(row.requires_valid_aligned_extent());
                        assert!(row.requires_initialized_extent());
                        assert!(row.requires_exclusive_runtime_binding());
                        assert!(row.requires_exact_launch_binding());
                        assert_eq!(row.invocation_axis(), Some(Axis::X));
                    }
                    Ok(())
                });
                assert!(
                    matches!(result, Ok(Some(()))),
                    "generic={generic} parallel={parallel} recurrence={recurrence}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn selected_slice_domains_do_not_promote_seeded_cycles_or_guard_copies_to_bounds() {
    for fault in 0..6 {
        let mut module = fixture(true, true, true, true);
        let body = module.functions[0].body.as_mut().unwrap();
        match fault {
            0 => {
                let OperationKind::Compare { predicate, .. } =
                    &mut body.blocks[2].operations[1].kind
                else {
                    unreachable!()
                };
                *predicate = ComparePredicate::GreaterThan;
            }
            1 => {
                let Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                } = body.blocks[2].terminator.as_mut().unwrap()
                else {
                    unreachable!()
                };
                std::mem::swap(then_target, else_target);
            }
            2 => {
                body.blocks[0].operations.push(op(
                    17,
                    Type::INDEX,
                    OperationKind::Intrinsic(IntrinsicOperation::new(
                        IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Global,
                            axis: Axis::Y,
                        },
                        Type::INDEX,
                    )),
                ));
                let OperationKind::GetElementPointer { offset, .. } =
                    &mut body.blocks[4].operations[1].kind
                else {
                    unreachable!()
                };
                *offset = ValueId(17);
            }
            3 => {
                let OperationKind::Load { access, .. } = &mut body.blocks[5].operations[0].kind
                else {
                    unreachable!()
                };
                access.alignment = 8;
            }
            4 => {
                let OperationKind::Store { access, .. } = &mut body.blocks[5].operations[1].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            5 => {
                body.blocks[2].operations[0].kind =
                    OperationKind::SliceLength { slice: ValueId(0) };
            }
            _ => unreachable!(),
        }
        let reached = Cell::new(false);
        let (result, _, _) = run(&module, 100_000_000, 100_000_000, |_, _| {
            reached.set(true);
            Ok(())
        });
        assert!(matches!(result, Ok(None)), "fault={fault}: {result:?}");
        assert!(!reached.get());
    }
}

#[test]
fn selected_slice_domains_preserve_both_permission_restriction_and_widening_orders() {
    for restrict_first in [false, true] {
        let mut module = fixture(false, true, true, true);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[5].operations.remove(1);
        body.blocks[5].parameters[0].ty = pointer(AddressSpace::Generic, AccessMode::ReadOnly);
        for (block, source, intermediate, output) in [(3, 10, 17, 11), (4, 13, 18, 14)] {
            body.blocks[block].operations.truncate(2);
            let middle = pointer(
                if restrict_first {
                    AddressSpace::Global
                } else {
                    AddressSpace::Generic
                },
                if restrict_first {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                },
            );
            let target = pointer(AddressSpace::Generic, AccessMode::ReadOnly);
            body.blocks[block].operations.extend([
                op(
                    intermediate,
                    middle.clone(),
                    OperationKind::Cast {
                        kind: if restrict_first {
                            CastKind::RestrictPointerAccess
                        } else {
                            CastKind::PointerToGeneric
                        },
                        value: ValueId(source),
                        to: middle,
                    },
                ),
                op(
                    output,
                    target.clone(),
                    OperationKind::Cast {
                        kind: if restrict_first {
                            CastKind::PointerToGeneric
                        } else {
                            CastKind::RestrictPointerAccess
                        },
                        value: ValueId(intermediate),
                        to: target,
                    },
                ),
            ]);
        }
        let reached = Cell::new(false);
        let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
            assert_eq!(inspect(batch, budget)?, 3);
            assert_eq!(
                batch.function_conditions(FunctionCoordinate(0), budget)?,
                (launch(), FormalIndexWidth::Bits64, 1, 0)
            );
            for parameter in batch
                .parameters(FunctionCoordinate(0), budget)?
                .iter()
                .flatten()
            {
                assert_eq!(parameter.access(), AccessMode::ReadWrite);
                assert!(!parameter.requires_exclusive_runtime_binding());
            }
            reached.set(true);
            Ok(())
        });
        assert!(
            matches!(result, Ok(Some(()))),
            "restrict_first={restrict_first}: {result:?}"
        );
        assert!(reached.get());
    }
}

#[test]
fn selected_slice_domains_reject_lossy_index_width_and_an_unproved_incoming_pointer() {
    for missing_path in [false, true] {
        let mut module = fixture(true, false, false, false);
        let function = &mut module.functions[0];
        let body = function.body.as_mut().unwrap();
        if missing_path {
            function
                .signature
                .parameters
                .push(pointer(AddressSpace::Generic, AccessMode::ReadOnly));
            body.parameters.push(ValueId(17));
            body.blocks[4].terminator = branch(60, 17);
        } else {
            body.blocks[0].operations.extend([
                op(
                    17,
                    scalar(),
                    OperationKind::Cast {
                        kind: CastKind::Truncate,
                        value: ValueId(4),
                        to: scalar(),
                    },
                ),
                op(
                    18,
                    Type::INDEX,
                    OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value: ValueId(17),
                        to: Type::INDEX,
                    },
                ),
            ]);
            for block in [3, 4] {
                let OperationKind::GetElementPointer { offset, .. } =
                    &mut body.blocks[block].operations[1].kind
                else {
                    unreachable!()
                };
                *offset = ValueId(18);
            }
        }
        let reached = Cell::new(false);
        let (result, _, _) = run(&module, 100_000_000, 100_000_000, |_, _| {
            reached.set(true);
            Ok(())
        });
        assert!(
            matches!(result, Ok(None)),
            "missing_path={missing_path}: {result:?}"
        );
        assert!(!reached.get());
    }
}

#[test]
fn selected_slice_domains_preserve_noop_and_unconditional_read_profiles() {
    let mut module = fixture(false, false, false, false);
    let body = module.functions[0].body.as_mut().unwrap();
    for block in &mut body.blocks {
        block
            .operations
            .retain(|operation| !matches!(operation.kind, OperationKind::Load { .. }));
    }
    let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
        assert_eq!(
            batch.function_conditions(FunctionCoordinate(0), budget)?,
            (launch(), FormalIndexWidth::Bits64, 0, 0)
        );
        assert!(batch.accesses(FunctionCoordinate(0), budget)?.is_empty());
        Ok(())
    });
    assert!(matches!(result, Ok(Some(()))), "{result:?}");
    let module = fixture(false, false, false, false);
    let (result, _, _) = run(&module, 100_000_000, 100_000_000, inspect);
    assert!(matches!(result, Ok(Some(2))), "{result:?}");

    let mut module = fixture(false, false, false, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[5].operations.clear();
    body.blocks[3].operations.push(op(
        16,
        scalar(),
        OperationKind::Load {
            pointer: ValueId(10),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
        let operation = Coordinate {
            block: BlockCoordinate {
                function: FunctionCoordinate(0),
                block: 3,
            },
            operation: 3,
        };
        let choices = batch.choices_at(operation, budget)?.unwrap();
        assert_eq!(choices.len(), 1);
        assert_eq!(
            choices[0].injection,
            CanonicalSelectedSliceInjectionV30::Access
        );
        assert!(choices[0].at_access);
        assert_eq!(choices[0].domain.pointer(), ValueId(10));
        assert_eq!(choices[0].root_space, AddressSpace::Global);
        assert_eq!(
            batch.function_conditions(FunctionCoordinate(0), budget)?,
            (launch(), FormalIndexWidth::Bits64, 1, 0)
        );
        Ok(())
    });
    assert!(matches!(result, Ok(Some(()))), "{result:?}");
}

#[test]
fn selected_slice_domains_reject_an_unseeded_disconnected_pointer_recurrence() {
    let mut module = fixture(true, false, false, false);
    let body = module.functions[0].body.as_mut().unwrap();
    let mut disconnected = BasicBlock::new(BlockId(90));
    disconnected.parameters.push(ValueDef::new(
        ValueId(18),
        pointer(AddressSpace::Generic, AccessMode::ReadOnly),
    ));
    disconnected.operations.push(op(
        19,
        scalar(),
        OperationKind::Load {
            pointer: ValueId(18),
            access: MemoryAccess::new(AddressSpace::Generic, 4),
        },
    ));
    disconnected.terminator = branch(90, 18);
    body.blocks.push(disconnected);
    let reached = Cell::new(false);
    let (result, _, _) = run(&module, 100_000_000, 100_000_000, |_, _| {
        reached.set(true);
        Ok(())
    });
    assert!(matches!(result, Ok(None)), "{result:?}");
    assert!(!reached.get());
}

#[test]
fn selected_slice_domains_have_exact_full_transaction_and_one_short_resources() {
    let module = fixture(true, true, true, true);
    let (result, work, peak) = run(&module, 100_000_000, 100_000_000, inspect);
    assert!(matches!(result, Ok(Some(6))), "{result:?}");
    let (exact, exact_work, exact_peak) = run(&module, work, peak, inspect);
    assert!(matches!(exact, Ok(Some(6))), "{exact:?}");
    assert_eq!((exact_work, exact_peak), (work, peak));
    let (short, _, _) = run(&module, work - 1, peak, inspect);
    assert!(
        matches!(short, Err(Failure::Resource(ResourceError::Work(error))) if error.actual() == work && error.limit() == work - 1)
    );
    let (short, _, _) = run(&module, work, peak - 1, inspect);
    assert!(
        matches!(short, Err(Failure::Resource(ResourceError::Storage { actual, limit })) if actual == peak && limit == peak - 1)
    );
}

#[test]
fn selected_slice_domain_queries_latch_foreign_budget_and_coordinate_refusals() {
    for foreign in [false, true] {
        let module = fixture(true, false, true, true);
        let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
            let error = if foreign {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                let mut other = Budget::new(&mut work, 100_000_000);
                other.reserve_storage(budget.storage()).unwrap();
                batch.owner(&mut other).unwrap_err()
            } else {
                match batch.accesses(FunctionCoordinate(u32::MAX), budget) {
                    Err(error) => error,
                    Ok(_) => panic!("foreign function accepted"),
                }
            };
            if foreign {
                assert_eq!(error, Failure::Resource(ResourceError::Accounting));
            } else {
                assert!(matches!(error, Failure::Coordinate(_)));
            }
            assert_eq!(batch.function_count(budget).unwrap_err(), error);
            Ok(())
        });
        if foreign {
            assert_eq!(result, Err(Failure::Resource(ResourceError::Accounting)));
        } else {
            assert!(matches!(result, Err(Failure::Coordinate(_))));
        }
    }
}
