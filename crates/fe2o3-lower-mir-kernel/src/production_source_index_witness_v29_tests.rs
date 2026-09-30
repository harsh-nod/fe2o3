use super::*;

#[path = "production_source_index_computation_v35_tests.rs"]
mod computation_tests;

const INDEX: SemanticTypeIdV1 = WORD;
const THREAD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const THREAD_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const DISJOINT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const DISJOINT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn index_abi(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        8,
        Some(8),
    )
    .unwrap();
    let kernel = tag == 10;
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        inputs.len() as u32,
        inputs
            .iter()
            .map(|&ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Direct(if ty == THREAD_REF || ty == DISJOINT_REF {
                        shared
                    } else {
                        plain
                    }),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(
            output,
            if output == UNIT {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(plain)
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|&ty| {
                if ty == THREAD_REF || ty == DISJOINT_REF {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::ByValue
                }
            })
            .collect(),
    )
    .unwrap()
}

fn index_intrinsic(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source(),
            index_abi(tag, inputs, output),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

fn index_call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    result: u32,
    ty: SemanticTypeIdV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(result, ty),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn index_reader_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = owner(Case::Shared);
    let mut types = base.source_semantic().types()[..2].to_vec();
    for (tag, witness) in [(3, THREAD), (5, DISJOINT)] {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                types[INDEX.index() as usize]
                    .layout()
                    .backend_repr()
                    .clone(),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![INDEX, UNIT]).unwrap(),
            ),
        ));
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
                base.source_semantic().types()[REFERENCE.index() as usize]
                    .layout()
                    .clone(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        witness,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            8,
                            8,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
    }
    let blocks = vec![
        block(10, vec![], index_call(1, vec![], 1, THREAD, 1)),
        block(
            11,
            vec![assign(
                place(2, THREAD_REF),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, THREAD),
                },
            )],
            index_call(
                2,
                vec![SemanticOperandV1::Copy(place(2, THREAD_REF))],
                3,
                INDEX,
                2,
            ),
        ),
        block(
            12,
            vec![],
            index_call(
                3,
                vec![SemanticOperandV1::Copy(place(1, THREAD))],
                4,
                DISJOINT,
                3,
            ),
        ),
        block(
            13,
            vec![assign(
                place(5, DISJOINT_REF),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(4, DISJOINT),
                },
            )],
            index_call(
                4,
                vec![SemanticOperandV1::Copy(place(5, DISJOINT_REF))],
                6,
                INDEX,
                4,
            ),
        ),
        block(14, vec![unit()], SemanticTerminatorKindV1::Return),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([10; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([10; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([10; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([10; 32]),
        source(),
        index_abi(10, &[], UNIT),
        [
            UNIT,
            THREAD,
            THREAD_REF,
            INDEX,
            DISJOINT,
            DISJOINT_REF,
            INDEX,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            local(
                10 + index as u8,
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(
        base.source_semantic().functions()[0]
            .kernel_entry()
            .unwrap()
            .clone(),
    );
    let callables = vec![
        SemanticCallableDeclV1::defined(ROOT),
        index_intrinsic(
            20,
            &[],
            THREAD,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: THREAD,
                raw_index: INDEX,
            },
        ),
        index_intrinsic(
            21,
            &[THREAD_REF],
            INDEX,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: THREAD,
                raw_index: INDEX,
            },
        ),
        index_intrinsic(
            22,
            &[THREAD],
            DISJOINT,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                input_witness: THREAD,
                output_witness: DISJOINT,
                raw_index: INDEX,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
            },
        ),
        index_intrinsic(
            23,
            &[DISJOINT_REF],
            INDEX,
            SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
                index_witness: DISJOINT,
                raw_index: INDEX,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
            },
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut ArgumentBudgetV1::new(&mut work, usize::MAX))
        .unwrap();
    owner
}

thread_local! {
    static READER_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BORROW_FAULT: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) };
}

struct ObserverGuard;
impl Drop for ObserverGuard {
    fn drop(&mut self) {
        SOURCE_INDEX_BORROW_OBSERVER_V29.set(None);
        SOURCE_INDEX_READER_OBSERVER_V29.set(None);
        BORROW_FAULT.set(usize::MAX);
    }
}

fn reader_checks(
    references: &SourceReferenceEmissionV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    expected: (SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let loan = binding.origin.single_loan()?;
    let original = references.index_witnesses[loan].get().unwrap();
    let actual =
        references.index_reader_value_v29(binding, binding.source_type, expected, budget)?;
    assert_eq!(actual, binding.values[0].id);
    for fault in 0..6 {
        let mut altered = original;
        match fault {
            0 => references.index_witnesses[loan].set(None),
            1 => {
                altered.origin = usize::MAX;
                references.index_witnesses[loan].set(Some(altered));
            }
            2 => {
                altered.site.statement = Some(usize::MAX);
                references.index_witnesses[loan].set(Some(altered));
            }
            3 => {
                altered.ty = INDEX;
                references.index_witnesses[loan].set(Some(altered));
            }
            4 => {
                altered.index_space = SemanticDisjointIndexSpaceV1::GridExclusive;
                references.index_witnesses[loan].set(Some(altered));
            }
            5 => {
                altered.disjoint = !altered.disjoint;
                references.index_witnesses[loan].set(Some(altered));
            }
            _ => unreachable!(),
        }
        let refused =
            references.index_reader_value_v29(binding, binding.source_type, expected, budget);
        references.index_witnesses[loan].set(Some(original));
        assert!(
            matches!(
                refused,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "source index reader differs from its original witness loan",
                })
            ),
            "receipt fault {fault}: {refused:?}"
        );
        assert_eq!(
            references.index_reader_value_v29(binding, binding.source_type, expected, budget)?,
            actual
        );
    }
    let claimed = references.claimed[loan].replace(false);
    let refused = references.index_reader_value_v29(binding, binding.source_type, expected, budget);
    references.claimed[loan].set(claimed);
    assert!(matches!(
        refused,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "source index reader differs from its original witness loan",
        })
    ));
    assert_eq!(
        references.index_reader_value_v29(binding, binding.source_type, expected, budget)?,
        actual
    );
    for fault in 0..3 {
        let mut altered = binding.clone();
        let detail = match fault {
            0 => {
                altered.owner ^= 1;
                "source reference binding belongs to another owner"
            }
            1 => {
                altered.origin = SourceReferenceBindingOriginV29::SingleLoan(1 - loan);
                "source reference binding belongs to another owner"
            }
            2 => {
                altered.values[0].ty = Type::Scalar(ScalarType::U64);
                "source reference physical payload differs"
            }
            _ => unreachable!(),
        };
        let refused =
            references.index_reader_value_v29(&altered, binding.source_type, expected, budget);
        assert!(
            matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None, detail: actual_detail,
        }) if actual_detail == detail),
            "binding fault {fault}: {refused:?}"
        );
        assert_eq!(
            references.index_reader_value_v29(binding, binding.source_type, expected, budget)?,
            actual
        );
    }
    READER_CHECKS.set(READER_CHECKS.get() + 1);
    Ok(())
}

#[test]
fn source_shared_index_origin_preservation_requires_original_c1_loan_and_keeps_legacy_invalidation()
{
    cells_tests::run_cells(index_reader_owner(), |plan, budget| {
        let source = plan.instances.owner().source_semantic();
        let function = plan
            .instances
            .instance(plan.instances.root())
            .unwrap()
            .declaration();
        let dominance = SemanticOptionDominanceV1::analyze(function, &[]).unwrap();
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        with_source_reference_availability_v29(
            plan.instances,
            plan.instances.root(),
            Some(&emission),
            budget,
            |cursor, budget| {
                let certified = cursor
                    .ssa
                    .plan()
                    .promoted_variables()
                    .iter()
                    .map(|local| local.get())
                    .collect::<BTreeSet<_>>();
                assert!(certified.contains(&1) && certified.contains(&4));
                let mut legacy = SemanticCapabilityOriginResolverV1::new(
                    source.types(),
                    source.callables(),
                    function,
                    &dominance,
                    &certified,
                    usize::MAX,
                    usize::MAX,
                )?;
                for local in [1, 4] {
                    assert!(legacy.invalidated_locals.contains(&local));
                    assert_eq!(legacy.resolve(SemanticLocalIdV1::from_index(local))?, None);
                }
                budget.reserve_storage(std::mem::size_of::<
                    Option<(
                        &ExecutionAvailabilityV29<'_>,
                        &mut dyn SemanticEmissionBudgetV1,
                    )>,
                >())?;
                let mut checked = SemanticCapabilityOriginResolverV1::new_with_source_v29(
                    source.types(),
                    source.callables(),
                    function,
                    &dominance,
                    &certified,
                    usize::MAX,
                    usize::MAX,
                    Some((&cursor, budget)),
                )?;
                for (local, disjoint) in [(1, false), (4, true)] {
                    assert!(!checked.invalidated_locals.contains(&local));
                    assert_eq!(
                        checked.resolve(SemanticLocalIdV1::from_index(local))?,
                        Some(SemanticPromotedBindingV1::IndexWitness {
                            index_space: SemanticDisjointIndexSpaceV1::Index1d,
                            disjoint,
                            availability: None,
                        })
                    );
                }
                for loan in &plan.loans {
                    let original = function.blocks()[loan.site.block.index() as usize].statements()
                        [loan.site.statement.unwrap()]
                    .kind();
                    assert!(source_shared_index_borrow_preserves_origin_v29(
                        &emission, loan.site, original, budget,
                    )?);
                    let cloned = original.clone();
                    let refused = source_shared_index_borrow_preserves_origin_v29(
                        &emission, loan.site, &cloned, budget,
                    );
                    assert!(matches!(
                        refused,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "source index reader differs from its original witness loan",
                        })
                    ));
                    assert!(source_shared_index_borrow_preserves_origin_v29(
                        &emission, loan.site, original, budget,
                    )?);
                }
                let mut missing = SourceReferenceEmissionV29::new(plan, budget)?;
                let saved = missing.sites.pop().unwrap();
                let site = plan.loans[saved.1].site;
                let original = function.blocks()[site.block.index() as usize].statements()
                    [site.statement.unwrap()]
                .kind();
                assert!(!source_shared_index_borrow_preserves_origin_v29(
                    &missing, site, original, budget,
                )?);
                missing.sites.push(saved);
                assert!(source_shared_index_borrow_preserves_origin_v29(
                    &missing, site, original, budget,
                )?);
                missing.abort_scope(plan.instances, budget)?;
                let shared = function.blocks()[1].statements()[0].kind();
                checked.index_statement(shared)?;
                assert!(checked.invalidated_locals.contains(&1));
                let mutable = assign(
                    place(5, DISJOINT_REF),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(4, DISJOINT),
                    },
                );
                // Even a mistakenly supplied preservation bit cannot waive a mutable borrow.
                let mut mutable_probe = SemanticCapabilityOriginResolverV1::new(
                    source.types(),
                    source.callables(),
                    function,
                    &dominance,
                    &certified,
                    usize::MAX,
                    usize::MAX,
                )?;
                mutable_probe.invalidated_locals.clear();
                mutable_probe.index_statement_with_shared_origin_v29(mutable.kind(), true)?;
                assert!(mutable_probe.invalidated_locals.contains(&4));
                Ok(())
            },
        )
    })
    .unwrap();
}

#[test]
fn original_shared_index_readers_emit_checked_scalar_values_and_reject_forged_receipts() {
    let _guard = ObserverGuard;
    READER_CHECKS.set(0);
    SOURCE_INDEX_READER_OBSERVER_V29.set(Some(reader_checks));
    cell_emission_tests::with_cell_source_lowered(
        index_reader_owner(),
        |plan, references, emitted, _| {
            assert_eq!(plan.loans.len(), 2);
            assert_eq!(references.index_witnesses.len(), plan.loans.len());
            assert!(
                references
                    .index_witnesses
                    .iter()
                    .all(|proof| proof.get().is_some())
            );
            assert_eq!(emitted.len(), 1);
            let body = emitted[0].1.function.body.as_ref().unwrap();
            assert!(!body.blocks.iter().flat_map(|block| &block.operations).any(
                |operation| matches!(
                    operation.kind,
                    OperationKind::Call { .. } | OperationKind::Load { .. }
                )
            ));
            assert_eq!(READER_CHECKS.get(), 2);
            Ok(())
        },
    )
    .unwrap();
}

fn corrupt_borrow(
    this: &mut SemanticFunctionLoweringV1<'_, '_>,
    _: SemanticBlockIdV1,
    _: Option<u32>,
    loan: usize,
) {
    if loan != 0 {
        return;
    }
    match BORROW_FAULT.get() {
        0 => {
            let SemanticValueBindingV1::IndexWitness { id, .. } = this.locals[1].as_ref().unwrap()
            else {
                panic!("original witness missing");
            };
            this.locals[1] = Some(SemanticValueBindingV1::Value {
                id: *id,
                ty: Type::INDEX,
            });
        }
        1 => this
            .control_flow_ssa
            .cfg_carriers
            .locals
            .retain(|(local, _)| *local != 1),
        2 => {
            let row = this
                .control_flow_ssa
                .cfg_carriers
                .locals
                .iter_mut()
                .find(|(local, _)| *local == 1)
                .unwrap();
            let SemanticPromotedBindingV1::IndexWitness { index_space, .. } = &mut row.1.binding
            else {
                panic!("original recipe missing");
            };
            *index_space = SemanticDisjointIndexSpaceV1::GridExclusive;
        }
        _ => panic!("unknown fixture fault"),
    }
}

#[test]
fn original_shared_index_borrow_rejects_integer_substitution_missing_recipe_and_changed_space() {
    let _guard = ObserverGuard;
    SOURCE_INDEX_BORROW_OBSERVER_V29.set(Some(corrupt_borrow));
    for fault in 0..3 {
        BORROW_FAULT.set(fault);
        let refused =
            cell_emission_tests::with_cell_source_lowered(index_reader_owner(), |_, _, _, _| {
                panic!("forged borrow must not reach completed emission");
            });
        assert!(
            matches!(
                refused,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution CFG transport differs from its captured SSA state",
                })
            ),
            "borrow fault {fault}: {refused:?}"
        );
    }
}

#[test]
fn original_index_reader_contract_requires_original_call_and_independent_exact_work() {
    let owner = index_reader_owner();
    let source = owner.source_semantic();
    let function = &source.functions()[0];
    let block = SemanticBlockIdV1::from_index(1);
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[1].terminator().kind() else {
        panic!("reader missing");
    };
    for limit in [16, 15] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_index_reader_contract_v29(
            function,
            source.callables(),
            block,
            call,
            &mut budget,
        );
        if limit == 16 {
            assert_eq!(
                result.unwrap(),
                (THREAD, SemanticDisjointIndexSpaceV1::Index1d, false)
            );
            assert_eq!(budget.work(), 16);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error)
                    )
                ) if error.actual() == 16 && error.limit() == 15
            ));
        }
        assert_eq!(budget.storage(), 0);
    }
    let cloned = call.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let refused =
        source_index_reader_contract_v29(function, source.callables(), block, &cloned, &mut budget);
    assert!(matches!(
        refused,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "source index reader differs from its original witness loan",
        })
    ));
}

#[test]
fn index_witness_receipt_backing_has_independent_exact_and_short_storage() {
    use std::mem::size_of;
    type Row = std::cell::Cell<Option<SourceIndexWitnessBorrowV29>>;
    // The owner pays its Vec header separately. This is the actual fixed-array
    // allocator used by source_reference_owned_vec_v29, including return frames.
    let headers =
        size_of::<Vec<Row>>() + 2 * size_of::<Result<Vec<Row>, ProductionSemanticKirErrorV1>>();
    let bytes = 2 * size_of::<Row>();
    for short in [false, true] {
        let limit = headers + bytes - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = source_reference_emission_vec_v29::<Row>(2, &mut budget);
        assert_eq!(budget.work(), 3);
        if short {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error)
            )) if error.actual() == headers + bytes && error.limit() == limit)
            );
            assert_eq!(budget.storage(), headers);
        } else {
            let rows = result.unwrap();
            assert_eq!(rows.capacity(), 2);
            assert_eq!(budget.storage(), headers + bytes);
            drop(rows);
        }
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn foreign_reader_ledger(
    references: &SourceReferenceEmissionV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    expected: (SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
    foreign.reserve_storage(budget.storage())?;
    let before = (foreign.work(), foreign.storage());
    let refused =
        references.index_reader_value_v29(binding, binding.source_type, expected, &mut foreign);
    assert!(matches!(
        refused,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!((foreign.work(), foreign.storage()), before);
    let original =
        references.index_reader_value_v29(binding, binding.source_type, expected, budget);
    assert!(matches!(
        original,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    READER_CHECKS.set(READER_CHECKS.get() + 1);
    // Deliberately swallow the immediate refusal. The enclosing source owner
    // must retain its first failure and deny the actual reader's next check.
    Ok(())
}

#[test]
fn original_index_reader_foreign_ledger_refusal_is_sticky_at_actual_emission() {
    let _guard = ObserverGuard;
    READER_CHECKS.set(0);
    SOURCE_INDEX_READER_OBSERVER_V29.set(Some(foreign_reader_ledger));
    let refused =
        cell_emission_tests::with_cell_source_lowered(index_reader_owner(), |_, _, _, _| {
            panic!("foreign ledger must not complete emission");
        });
    assert_eq!(READER_CHECKS.get(), 1);
    assert!(matches!(
        refused,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}
