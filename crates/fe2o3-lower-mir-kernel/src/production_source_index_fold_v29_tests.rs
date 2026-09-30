// These immutable graph equations do not authenticate an original Rust use.
// The repeated original array relocation corpus independently exercises that
// source-to-actual join and the complete consuming path.
#[derive(Clone, Copy, Debug)]
enum IndexFoldCaseV29 {
    Literal(ScalarType),
    NarrowThenWiden,
    RuntimeCast,
    WrongLiteral,
    WrongWidth,
    Signed,
    UnknownRuntime,
    ForeignValue,
    ForeignOrigin,
    OutsideBound,
}

fn folded_index_module_v29(case: IndexFoldCaseV29) -> (Module, PendingSourceIndexV29) {
    let mut module = literal_array_module(1024, 0);
    let function = &mut module.functions[0];
    let body = function.body.as_mut().unwrap();
    let block = &mut body.blocks[0];
    let mut prefix = Vec::new();
    let mut original = ValueId(30);
    let mut scalar = ScalarType::U32;
    let mut expected = 7;
    match case {
        IndexFoldCaseV29::Literal(ty) => {
            scalar = ty;
            let constant = match ty {
                ScalarType::U8 => Constant::U8(7),
                ScalarType::U16 => Constant::U16(7),
                ScalarType::U32 => Constant::U32(7),
                ScalarType::U64 => Constant::U64(7),
                ScalarType::Index => Constant::Index(7),
                _ => unreachable!(),
            };
            prefix.push(Operation::effect_free(
                ValueDef::new(original, Type::Scalar(ty)),
                OperationKind::Constant(constant),
            ));
        }
        IndexFoldCaseV29::NarrowThenWiden => {
            prefix.push(Operation::effect_free(
                ValueDef::new(ValueId(30), Type::Scalar(ScalarType::U64)),
                OperationKind::Constant(Constant::U64(0x1_0000_0107)),
            ));
            prefix.push(Operation::effect_free(
                ValueDef::new(ValueId(31), Type::Scalar(ScalarType::U8)),
                OperationKind::Cast {
                    kind: CastKind::Truncate,
                    value: ValueId(30),
                    to: Type::Scalar(ScalarType::U8),
                },
            ));
            original = ValueId(32);
            prefix.push(Operation::effect_free(
                ValueDef::new(original, Type::Scalar(scalar)),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(31),
                    to: Type::Scalar(scalar),
                },
            ));
        }
        IndexFoldCaseV29::RuntimeCast | IndexFoldCaseV29::UnknownRuntime => {
            scalar = ScalarType::U8;
            original = ValueId(1);
            function.signature.parameters.push(Type::Scalar(scalar));
            body.parameters.push(original);
        }
        IndexFoldCaseV29::Signed => {
            scalar = ScalarType::I32;
            prefix.push(Operation::effect_free(
                ValueDef::new(original, Type::Scalar(scalar)),
                OperationKind::Constant(Constant::I32(7)),
            ));
        }
        _ => {
            if matches!(case, IndexFoldCaseV29::OutsideBound) {
                expected = 1024;
            }
            prefix.push(Operation::effect_free(
                ValueDef::new(original, Type::Scalar(scalar)),
                OperationKind::Constant(Constant::U32(expected as u32)),
            ));
        }
    }
    if matches!(case, IndexFoldCaseV29::RuntimeCast) {
        let mut value = original;
        let path: Vec<_> = plan_integer_cast_v1(scalar, ScalarType::Index)
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        for (ordinal, (kind, target)) in path.iter().copied().enumerate() {
            let output = if ordinal + 1 == path.len() {
                OFFSET
            } else {
                ValueId(40 + ordinal as u32)
            };
            prefix.push(Operation::effect_free(
                ValueDef::new(output, Type::Scalar(target)),
                OperationKind::Cast {
                    kind,
                    value,
                    to: Type::Scalar(target),
                },
            ));
            value = output;
        }
        assert_eq!(value, OFFSET);
        block.operations.remove(2);
    } else {
        block.operations[2].kind = OperationKind::Constant(Constant::Index(
            expected + u64::from(matches!(case, IndexFoldCaseV29::WrongLiteral)),
        ));
    }
    block.operations.splice(2..2, prefix);
    let operation = block
        .operations
        .iter()
        .position(|row| row.results.iter().any(|result| result.id == ELEMENT))
        .unwrap();
    if matches!(case, IndexFoldCaseV29::WrongWidth) {
        scalar = ScalarType::U64;
    }
    if matches!(case, IndexFoldCaseV29::ForeignValue) {
        original = ValueId(9999);
    }
    let source = PendingSourceIndexV29 {
        instance: ProductionCallInstanceIdV1(0),
        event: 0,
        canonical: 0,
        load_anchor: None,
        index_slot: None,
        array_slot: 0,
        original,
        scalar,
        length: 1024,
        base: if matches!(case, IndexFoldCaseV29::ForeignOrigin) {
            UNUSED
        } else {
            ARRAY
        },
        offset: OFFSET,
        pointer: ELEMENT,
        block: BlockId(0),
        operation,
    };
    (module, source)
}

fn check_folded_index_v29(
    case: IndexFoldCaseV29,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let (module, row) = folded_index_module_v29(case);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = ArgumentBudgetV1::new(&mut setup_work, LIMIT);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut setup,
        )
        .unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut setup).unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let (versions, receipt) = fe2o3_kernel_analysis::CanonicalKirMemorySsaV18::derive_v18(
        &inventory,
        Default::default(),
        &mut setup,
    )
    .unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let function = inventory.functions()[0].function;
    let slots = [literal_array_slot(1024)];
    let accesses: Vec<_> = function.body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            )
        })
        .map(|(operation, _)| SourceAddressAccessV29 {
            block: BlockId(0),
            operation,
            slot: 0,
        })
        .collect();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_inventory(
            &inventory,
            CanonicalKirFunctionCoordinateV1(0),
            &slots,
            &accesses,
            budget,
        )?
        .solve_pending_indices(&slots, &accesses, &[], budget)?
        .graph;
        let result = scoped_slot_uses_v29::check_expanded_index_addresses_v29(
            function,
            &graph,
            &slots,
            &accesses,
            &[],
            &[],
            &[],
            &[SourceIndexLocationV29 {
                source: row,
                load: None,
            }],
            &[],
            &inventory,
            Some(&versions),
            CanonicalKirFunctionCoordinateV1(0),
            budget,
        );
        drop(graph);
        result
    });
    assert_eq!(budget.storage(), FLOOR, "{case:?}: {result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn final_index_normalization_reuses_exact_unsigned_folding_and_preserves_runtime_casts() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::Index,
    ] {
        let result = check_folded_index_v29(IndexFoldCaseV29::Literal(scalar), LIMIT, LIMIT).0;
        assert!(result.is_ok(), "{scalar:?}: {result:?}");
    }
    for case in [
        IndexFoldCaseV29::NarrowThenWiden,
        IndexFoldCaseV29::RuntimeCast,
    ] {
        let result = check_folded_index_v29(case, LIMIT, LIMIT).0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
    }
}

#[test]
fn final_index_fold_rejects_changed_literals_types_origins_and_unknown_values() {
    for case in [
        IndexFoldCaseV29::WrongLiteral,
        IndexFoldCaseV29::WrongWidth,
        IndexFoldCaseV29::Signed,
        IndexFoldCaseV29::UnknownRuntime,
        IndexFoldCaseV29::ForeignValue,
        IndexFoldCaseV29::ForeignOrigin,
        IndexFoldCaseV29::OutsideBound,
    ] {
        assert!(
            check_folded_index_v29(IndexFoldCaseV29::NarrowThenWiden, LIMIT, LIMIT)
                .0
                .is_ok()
        );
        let error = check_folded_index_v29(case, LIMIT, LIMIT).0.unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "source raw address differs from its actual formation or memory history"
                        | "source index needs exact current memory or successful guard bounds"
                }
            ),
            "{case:?}: {error:?}"
        );
    }
}

#[test]
fn final_index_fold_exact_and_short_scope_limits_keep_the_caller_floor() {
    let case = IndexFoldCaseV29::NarrowThenWiden;
    let (positive, work, peak) = check_folded_index_v29(case, LIMIT, LIMIT);
    assert!(positive.is_ok(), "{positive:?}");
    assert!(check_folded_index_v29(case, work, peak).0.is_ok());
    assert!(matches!(
        check_folded_index_v29(case, work - 1, peak).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        check_folded_index_v29(case, work, peak - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
