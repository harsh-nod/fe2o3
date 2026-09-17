use super::*;
use slice_view_v1::with_checked_borrowed_slice_access_v1 as query;

include!("production_borrowed_call_effects_v1_fixtures.rs");

fn use_asserted_index_for_read(module: &mut Module, _: &mut SemanticKirCorrespondenceV1) {
    let body = module.functions[1].body.as_mut().unwrap();
    let lhs = body.blocks[0]
        .operations
        .iter()
        .find_map(|operation| {
            if let OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                ..
            } = operation.kind
            {
                Some(lhs)
            } else {
                None
            }
        })
        .unwrap();
    let lhs_is_index = body.blocks[0]
        .operations
        .iter()
        .flat_map(|operation| &operation.results)
        .find(|result| result.id == lhs)
        .unwrap()
        .ty
        == Type::INDEX;
    let mut changed = false;
    for operation in &mut body.blocks[1].operations {
        if lhs_is_index {
            if let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind {
                *offset = lhs;
                changed = true;
            }
            continue;
        }
        if let OperationKind::Cast {
            kind: CastKind::Bitcast,
            value,
            to,
        } = &mut operation.kind
            && *to == Type::INDEX
        {
            *value = lhs;
            changed = true;
        }
    }
    assert!(
        changed,
        "fixture must replace the actual indexed address input"
    );
}

#[test]
fn sealed_helper_cannot_be_relabelled_as_an_empty_path_root() {
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |_, _| {},
        |subject, assertions, inventory, budget| {
            query(
                subject,
                assertions,
                inventory,
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                budget,
                |_| Ok(()),
            )
            .unwrap();
            for reparent in [false, true] {
                let mut correspondence = subject.correspondence.clone();
                let helper = correspondence
                    .lowered_functions
                    .iter_mut()
                    .find(|row| row.semantic_function == SemanticFunctionIdV1::from_index(1))
                    .unwrap();
                helper.role = SemanticKirFunctionRoleV1::KernelEntry;
                if reparent {
                    helper.correspondence_owner = SemanticFunctionIdV1::from_index(1);
                }
                let site = ProductionSliceAccessSiteV1::new(
                    SemanticFunctionIdV1::from_index(u32::from(reparent)),
                    SemanticFunctionIdV1::from_index(1),
                    SemanticBlockIdV1::from_index(1),
                    Some(0),
                    0,
                    SemanticBlockIdV1::from_index(0),
                );
                let floor = budget.storage();
                let result: Result<(), _> = query(
                    CanonicalCallSubjectV1 {
                        correspondence: &correspondence,
                        ..subject
                    },
                    assertions,
                    inventory,
                    site.with_call_path(&[]),
                    budget,
                    |_| panic!("relabelled helper bypassed its source and call-path checks"),
                );
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert_eq!(budget.storage(), floor);
            }
        },
    );
}

#[test]
fn sealed_same_valued_index_cannot_borrow_another_definition_span() {
    with_borrowed_subject(
        BorrowedCase::SameValueIndex,
        use_asserted_index_for_read,
        |subject, assertions, inventory, budget| {
            let mut correspondence = subject.correspondence.clone();
            let (block, first, count) = correspondence
                .statement_operation_spans
                .iter()
                .find(|row| {
                    row.semantic_function == SemanticFunctionIdV1::from_index(1)
                        && row.semantic_block == SemanticBlockIdV1::from_index(0)
                        && row.statement_ordinal == 1
                })
                .map(|row| {
                    (
                        row.kernel_ir_block,
                        row.first_operation_ordinal,
                        row.operation_count,
                    )
                })
                .unwrap();
            let forged = correspondence
                .statement_operation_spans
                .iter_mut()
                .find(|row| {
                    row.semantic_function == SemanticFunctionIdV1::from_index(1)
                        && row.semantic_block == SemanticBlockIdV1::from_index(1)
                        && row.statement_ordinal == 0
                })
                .unwrap();
            forged.kernel_ir_block = block;
            forged.first_operation_ordinal = first;
            forged.operation_count = count;
            let floor = budget.storage();
            let result: Result<(), _> = query(
                CanonicalCallSubjectV1 {
                    correspondence: &correspondence,
                    ..subject
                },
                assertions,
                inventory,
                helper_read_site(BorrowedCase::SameValueIndex).with_call_path(&[0]),
                budget,
                |_| panic!("equal-valued source definition inherited a forged native span"),
            );
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn source_index_rebinding_cannot_inherit_a_stale_physical_bound() {
    for case in [BorrowedCase::ChangedIndex, BorrowedCase::SameValueIndex] {
        with_borrowed_subject(
            case,
            use_asserted_index_for_read,
            |subject, assertions, inventory, budget| {
                let result: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(case).with_call_path(&[0]),
                    budget,
                    |_| panic!("source index mismatch received a view"),
                );
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
            },
        );
    }
}

#[test]
fn plain_explicit_load_and_exact_source_index_copy_remain_checked() {
    for case in [BorrowedCase::PlainLoad, BorrowedCase::CopiedIndex] {
        // Indexed explicit Load has no emitter path yet. Check its actual source
        // against the equivalent plain read emitted by the supported Copy path.
        with_borrowed_source_and_emission(
            case,
            if matches!(case, BorrowedCase::PlainLoad) {
                BorrowedCase::Distinct
            } else {
                case
            },
            |_, _| {},
            |subject, assertions, inventory, budget| {
                query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(case).with_call_path(&[0]),
                    budget,
                    |view| {
                        assert_eq!(view.source().source_argument(), 0);
                        assert_eq!(view.loaded_type(), &Type::Scalar(ScalarType::U32));
                        assert!(!view.memory().volatile);
                        Ok(())
                    },
                )
                .unwrap();
            },
        );
    }
}

#[test]
fn source_load_qualifiers_cannot_be_discarded_by_plain_native_reads() {
    for case in [BorrowedCase::VolatileLoad, BorrowedCase::AtomicLoad] {
        with_borrowed_source_and_emission(
            case,
            BorrowedCase::Distinct,
            |_, _| {},
            |subject, assertions, inventory, budget| {
                let result: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(case).with_call_path(&[0]),
                    budget,
                    |_| panic!("qualified source load received a plain-read view"),
                );
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ));
            },
        );
    }
}

#[test]
fn nested_helper_read_requires_every_actual_formal_call_edge() {
    with_borrowed_subject(
        BorrowedCase::Nested,
        |_, _| {},
        |subject, assertions, inventory, budget| {
            for outer in 0..2 {
                let path = [outer, 2];
                query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(BorrowedCase::Nested).with_call_path(&path),
                    budget,
                    |view| {
                        assert_eq!(view.call_path(), path);
                        assert_eq!(view.source().source_argument(), outer as u32);
                        assert!(matches!(
                            view.input(),
                            Definition::FunctionArgument { argument: 0, .. }
                        ));
                        assert!(matches!(view.root_input(), Definition::FunctionArgument { argument, .. }
                            if argument == outer as u32));
                        Ok(())
                    },
                )
                .unwrap();
            }
            for path in [&[0][..], &[2][..], &[2, 0][..], &[0, 2, 2][..]] {
                let result: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(BorrowedCase::Nested).with_call_path(path),
                    budget,
                    |_| panic!("broken path received a view"),
                );
                assert!(result.is_err());
            }
        },
    );
}

#[test]
fn real_helper_data_read_keeps_call_and_distinct_root_input_identities() {
    for case in [BorrowedCase::Distinct, BorrowedCase::Aliased] {
        with_borrowed_subject(
            case,
            |_, _| {},
            |subject, assertions, inventory, budget| {
                let mut roots = Vec::new();
                let mut read = None;
                for call in 0..2 {
                    let floor = budget.storage();
                    query(
                        subject,
                        assertions,
                        inventory,
                        helper_read_site(case).with_call_path(&[call]),
                        budget,
                        |view| {
                            assert_eq!(view.call_path(), [call]);
                            assert_eq!(
                                view.source().source_argument(),
                                if call == 1 && matches!(case, BorrowedCase::Distinct) {
                                    1
                                } else {
                                    0
                                }
                            );
                            assert!(view.source().source_path().is_empty());
                            assert_ne!(view.input(), view.root_input());
                            assert_eq!(view.loaded_type(), &Type::Scalar(ScalarType::U32));
                            assert_eq!(view.memory().address_space, AddressSpace::Global);
                            assert!(!view.memory().volatile);
                            assert!(matches!(
                                view.input(),
                                Definition::FunctionArgument { argument: 0, .. }
                            ));
                            assert_eq!(
                                view.access().operation.block.function,
                                inventory.calls()[call].target.unwrap()
                            );
                            if let Some(previous) = read {
                                assert_eq!(previous, view.access());
                            }
                            read = Some(view.access());
                            roots.push(view.root_input());
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                assert_eq!(roots[0] == roots[1], matches!(case, BorrowedCase::Aliased));
                use fe2o3_kernel_analysis::{
                    CanonicalKirCallEffectDecisionV1 as Decision,
                    CanonicalKirCallEffectErrorV1 as Error, CanonicalKirCallEffectKindV1 as Kind,
                    CanonicalKirCallEffectsV1,
                };
                let (effects, storage) =
                    CanonicalKirCallEffectsV1::derive(inventory, budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let root = inventory.calls()[0].coordinate.block.function;
                assert_eq!(
                    effects.decision(root, budget).unwrap(),
                    Decision::CompleteNonempty
                );
                assert_eq!(
                    effects
                        .decision(inventory.calls()[0].target.unwrap(), budget)
                        .unwrap(),
                    Decision::CompleteNonempty
                );
                let mut paths = Vec::new();
                effects.try_visit(root, budget, |event| {
                if matches!(event.kind(), Kind::Physical(effect) if Some(effect.coordinate) == read) {
                    paths.push(event.call_path().to_vec());
                }
                Ok::<_, Error>(())
            }).unwrap();
                assert_eq!(paths, [vec![0], vec![1]]);
                drop(effects);
                budget.release_storage(storage.retained_storage()).unwrap();
            },
        );
    }
}

#[test]
fn helper_read_rejects_wrong_extent_and_reassigned_index() {
    for case in [BorrowedCase::WrongExtent, BorrowedCase::ChangedIndex] {
        with_borrowed_subject(
            case,
            |_, _| {},
            |subject, assertions, inventory, budget| {
                let result: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(case).with_call_path(&[0]),
                    budget,
                    |_| panic!("invalid bounds received a view"),
                );
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
            },
        );
    }
}

#[test]
fn helper_read_rejects_rebound_descriptors_and_substituted_helper_carriers() {
    with_borrowed_subject(
        BorrowedCase::ReboundDescriptor,
        |_, _| {},
        |subject, assertions, inventory, budget| {
            let result: Result<(), _> = query(
                subject,
                assertions,
                inventory,
                helper_read_site(BorrowedCase::ReboundDescriptor).with_call_path(&[0]),
                budget,
                |_| panic!("rebound descriptor received a view"),
            );
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        },
    );
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |module, _| {
            let body = module.functions[1].body.as_mut().unwrap();
            let other = body.parameters[1];
            for block in &mut body.blocks {
                for operation in &mut block.operations {
                    match &mut operation.kind {
                        OperationKind::SliceData { slice }
                        | OperationKind::SliceLength { slice } => *slice = other,
                        _ => {}
                    }
                }
            }
        },
        |subject, assertions, inventory, budget| {
            let result: Result<(), _> = query(
                subject,
                assertions,
                inventory,
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                budget,
                |_| panic!("substituted helper carrier received a view"),
            );
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        },
    );
}

#[test]
fn helper_query_rejects_foreign_inventory_and_assertion_attachments() {
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |_, _| {},
        |subject, assertions, inventory, budget| {
            with_borrowed_subject(
                BorrowedCase::Distinct,
                |_, _| {},
                |_, foreign_assertions, foreign_inventory, _| {
                    for (assertions, inventory) in [
                        (assertions, foreign_inventory),
                        (foreign_assertions, inventory),
                    ] {
                        let result: Result<(), _> = query(
                            subject,
                            assertions,
                            inventory,
                            helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                            budget,
                            |_| panic!("foreign attachment received a view"),
                        );
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                        ));
                    }
                },
            );
        },
    );
}

#[test]
fn helper_read_rejects_call_source_and_access_substitutions() {
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |_, _| {},
        |subject, assertions, inventory, budget| {
            let f = SemanticFunctionIdV1::from_index;
            let b = SemanticBlockIdV1::from_index;
            for request in [
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[]),
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[usize::MAX]),
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[0, 0]),
                ProductionSliceAccessSiteV1::new(f(0), f(1), b(1), Some(0), 1, b(0))
                    .with_call_path(&[0]),
                ProductionSliceAccessSiteV1::new(f(0), f(1), b(1), Some(1), 0, b(0))
                    .with_call_path(&[0]),
                ProductionSliceAccessSiteV1::new(f(0), f(1), b(1), Some(0), 0, b(1))
                    .with_call_path(&[0]),
                ProductionSliceAccessSiteV1::new(f(1), f(1), b(1), Some(0), 0, b(0))
                    .with_call_path(&[0]),
                ProductionSliceAccessSiteV1::new(f(0), f(0), b(1), Some(0), 0, b(0))
                    .with_call_path(&[0]),
            ] {
                let result: Result<(), _> =
                    query(subject, assertions, inventory, request, budget, |_| {
                        panic!("substituted occurrence received a view")
                    });
                assert!(result.is_err());
            }
        },
    );
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |module, _| {
            let body = module.functions[0].body.as_mut().unwrap();
            let call = body.blocks[0]
                .operations
                .iter_mut()
                .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .unwrap();
            let OperationKind::Call { arguments, .. } = &mut call.kind else {
                unreachable!()
            };
            arguments.swap(0, 1);
        },
        |subject, assertions, inventory, budget| {
            let result: Result<(), _> = query(
                subject,
                assertions,
                inventory,
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                budget,
                |_| panic!("same-typed source substitution received a view"),
            );
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        },
    );
}

#[test]
fn helper_read_rejects_volatile_access_and_access_widening() {
    for widen in [false, true] {
        with_borrowed_subject(
            BorrowedCase::Distinct,
            |module, _| {
                let change_type = |ty: &mut Type| match ty {
                    Type::Slice(slice) => slice.access = AccessMode::ReadWrite,
                    Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global => {
                        pointer.access = AccessMode::ReadWrite
                    }
                    _ => {}
                };
                for function in &mut module.functions {
                    if widen {
                        for ty in &mut function.signature.parameters {
                            change_type(ty);
                        }
                    }
                    if let Some(body) = &mut function.body {
                        for block in &mut body.blocks {
                            if widen {
                                for parameter in &mut block.parameters {
                                    change_type(&mut parameter.ty);
                                }
                            }
                            for operation in &mut block.operations {
                                if widen {
                                    for result in &mut operation.results {
                                        change_type(&mut result.ty);
                                    }
                                } else if let OperationKind::Load { access, .. } =
                                    &mut operation.kind
                                {
                                    access.volatile = true;
                                }
                            }
                        }
                    }
                }
            },
            |subject, assertions, inventory, budget| {
                let result: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                    budget,
                    |_| panic!("invalid access contract received a view"),
                );
                assert!(result.is_err());
            },
        );
    }
}

#[test]
fn helper_query_exact_budgets_callback_failure_and_unwind_restore_storage() {
    with_borrowed_subject(
        BorrowedCase::Distinct,
        |_, _| {},
        |subject, assertions, inventory, outer| {
            let run = |budget: &mut AssertOriginBudgetV1<'_>| {
                query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(BorrowedCase::Distinct).with_call_path(&[1]),
                    budget,
                    |_| Ok(()),
                )
            };
            let floor = outer.storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            run(&mut budget).unwrap();
            let required = (budget.work(), budget.peak_storage());
            for (work_limit, storage_limit, accepted) in [
                (required.0, required.1, true),
                (required.0 - 1, required.1, false),
                (required.0, required.1 - 1, false),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result = run(&mut budget);
                assert_eq!(result.is_ok(), accepted);
                if !accepted {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                Resource::Work(_) | Resource::Storage(_)
                            )
                        )
                    ));
                }
                assert_eq!(budget.storage(), floor);
            }
            let result: Result<(), _> = query(
                subject,
                assertions,
                inventory,
                helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                outer,
                |_| Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
            );
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(outer.storage(), floor);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), _> = query(
                    subject,
                    assertions,
                    inventory,
                    helper_read_site(BorrowedCase::Distinct).with_call_path(&[0]),
                    outer,
                    |_| panic!("consumer unwind"),
                );
            }));
            assert!(result.is_err());
            assert_eq!(outer.storage(), floor);
        },
    );
}
