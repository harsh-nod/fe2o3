//! Synthetic graph/ledger controls, not live source authenticity or normal
//! compilation. Genuine Identity/Swap01 positives run in the frontend ladder.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const FLOOR: usize = 23;
const WORK: usize = 10_000_000;

fn synthetic(permutation: [u8; 4]) -> (SealedBf16CallRelationV1, Function, Function) {
    let types = (0..12)
        .map(|i| {
            Type::Scalar(if i < 8 {
                ScalarType::Bf16
            } else {
                ScalarType::F32
            })
        })
        .collect::<Vec<_>>();
    let mut call_block = BasicBlock::new(BlockId(41));
    call_block.operations.push(Operation::new(
        (200..204)
            .map(|i| ValueDef::new(ValueId(i), Type::F32))
            .collect(),
        OperationKind::Call {
            callee: FunctionId::new("query.synthetic.helper"),
            arguments: (100..112).map(ValueId).collect(),
        },
    ));
    call_block.terminator = Some(Terminator::Branch {
        target: BlockId(77),
        arguments: vec![],
    });
    let mut end = BasicBlock::new(BlockId(77));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let root = Function::kernel_entry(
        "query.synthetic.root",
        Signature::new(types.clone(), vec![]),
        (100..112).map(ValueId).collect(),
        vec![call_block, end],
    );
    let mut matrix_block = BasicBlock::new(BlockId(91));
    matrix_block.operations.push(Operation::new(
        (300..304)
            .map(|i| ValueDef::new(ValueId(i), Type::F32))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                    .with_zero_filled_predicate_inputs(),
            ),
        ),
    ));
    matrix_block.terminator = Some(Terminator::Return {
        values: permutation.map(|i| ValueId(300 + u32::from(i))).to_vec(),
    });
    let helper = Function::internal_helper(
        "query.synthetic.helper",
        Signature::new(types, vec![Type::F32; 4]),
        (0..12).map(ValueId).collect(),
        vec![matrix_block],
    );
    let mut capture = Bf16CallEmissionCaptureV1::new();
    capture.seen = [true, true];
    for group in 0..3 {
        for i in 0..4 {
            capture.arguments[group + 1][i] = ValueId(100 + (4 * group + i) as u32);
            capture.formals[group + 1][i] = ValueId((4 * group + i) as u32);
            capture.producers[group + 2][i] = capture.arguments[group + 1][i];
        }
    }
    capture.call_result = [ValueId(200), ValueId(201), ValueId(202), ValueId(203)];
    capture.producers[5] = [ValueId(300), ValueId(301), ValueId(302), ValueId(303)];
    capture.producers[6] = capture.producers[5];
    (
        SealedBf16CallRelationV1 {
            root: SemanticFunctionIdV1::from_index(9),
            helper: SemanticFunctionIdV1::from_index(3),
            call_block: BlockId(41),
            call_ordinal: 0,
            matrix_block: BlockId(91),
            matrix_ordinal: 0,
            source_call_block: SemanticBlockIdV1::from_index(7),
            source_matrix_block: SemanticBlockIdV1::from_index(5),
            helper_return: permutation.map(|i| ValueId(300 + u32::from(i))),
            permutation,
            capture,
        },
        root,
        helper,
    )
}
fn components(
    relation: &SealedBf16CallRelationV1,
    root: &Function,
    helper: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<()> {
    bf16_query_components_v1(
        relation,
        root,
        helper,
        &root.body.as_ref().unwrap().blocks[0].operations[0],
        &helper.body.as_ref().unwrap().blocks[0].operations[0],
        budget,
    )
}

#[test]
fn synthetic_identity_and_swap01_exact_component_transport() {
    for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
        let (relation, root, helper) = synthetic(permutation);
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        bf16_call_query_scope_v1(&mut budget, |b| components(&relation, &root, &helper, b))
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn synthetic_changed_missing_duplicate_components_and_return_maps_refuse() {
    for which in 0..22 {
        let (mut relation, mut root, mut helper) = synthetic([0, 1, 2, 3]);
        match which {
            0 => relation.capture.seen[0] = false,
            1 => relation.permutation = [2, 1, 0, 3],
            2 => relation.permutation = [1, 0, 2, 3],
            3 => relation.capture.arguments[1][1] = relation.capture.arguments[1][0],
            4 => relation.capture.formals[1][1] = relation.capture.formals[1][0],
            5 => relation.capture.producers[2][0] = ValueId(999),
            6 => relation.capture.call_result[1] = relation.capture.call_result[0],
            7 => relation.capture.producers[5][1] = relation.capture.producers[5][0],
            8 => relation.capture.producers[6][0] = ValueId(999),
            9 => relation.helper_return.swap(0, 1),
            10 => helper.signature.parameters[0] = Type::F32,
            11 => {
                helper.signature.parameters.pop();
            }
            12 => helper.signature.results[0] = Type::BOOL,
            13 => {
                helper.signature.results.pop();
            }
            14 => {
                helper.body.as_mut().unwrap().parameters.pop();
            }
            15 => {
                let OperationKind::Call { arguments, .. } =
                    &mut root.body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.pop();
            }
            16 => {
                let OperationKind::Call { arguments, .. } =
                    &mut root.body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                arguments[1] = arguments[0];
            }
            17 => {
                root.body.as_mut().unwrap().blocks[0].operations[0]
                    .results
                    .pop();
            }
            18 => root.body.as_mut().unwrap().blocks[0].operations[0].results[0].ty = Type::BOOL,
            19 => {
                helper.body.as_mut().unwrap().blocks[0].operations[0]
                    .results
                    .pop();
            }
            20 => {
                helper.body.as_mut().unwrap().blocks[0].operations[0].results[0].id = ValueId(999)
            }
            _ => {
                helper.body.as_mut().unwrap().blocks[0].terminator =
                    Some(Terminator::Return { values: vec![] })
            }
        }
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            bf16_call_query_scope_v1(&mut budget, |b| components(&relation, &root, &helper, b))
                .is_err(),
            "synthetic mutation {which}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn synthetic_exact_and_one_short_work_storage_keep_original_prefix() {
    let (relation, root, helper) = synthetic([0, 1, 2, 3]);
    let run = |limit, storage| {
        let mut work = Work::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            bf16_call_query_scope_v1(&mut budget, |b| components(&relation, &root, &helper, b));
        assert_eq!(budget.storage(), FLOOR);
        (
            result,
            budget.work(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let measured = run(WORK, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
    assert!(measured.0.is_ok());
    assert_eq!(
        run(measured.1, FLOOR + BF16_CALL_QUERY_SCRATCH_V1),
        measured
    );
    for limit in [0, BF16_CALL_QUERY_ENTRY_WORK_V1 - 1, measured.1 - 1] {
        let refused = run(limit, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
        assert!(matches!(
            refused.0,
            Err(Bf16NominalCallQueryErrorV1::Resource(
                ArgumentResourceV1::Work(_)
            ))
        ));
        assert!(refused.2.is_some());
    }
    let refused = run(WORK, FLOOR + BF16_CALL_QUERY_SCRATCH_V1 - 1);
    assert!(matches!(
        refused.0,
        Err(Bf16NominalCallQueryErrorV1::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert!(refused.3.is_some());
}

#[test]
fn scope_retains_callback_charges_on_success_error_and_panic() {
    for case in 0..3 {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(
            &mut work,
            FLOOR + bf16_call_query_scratch_v1::<u32>().unwrap() + 7,
        );
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = bf16_call_query_scope_v1(&mut budget, |b| {
            b.reserve_storage(7)?;
            b.charge_work(11)?;
            match case {
                0 => Ok(19u32),
                1 => Err(Bf16NominalCallQueryErrorV1::Unavailable("callback control")),
                _ => panic!("query callback panic control"),
            }
        });
        match case {
            0 => assert_eq!(result, Ok(19)),
            1 => assert_eq!(
                result,
                Err(Bf16NominalCallQueryErrorV1::Unavailable("callback control"))
            ),
            _ => assert_eq!(result, Err(Bf16NominalCallQueryErrorV1::CallbackPanicked)),
        }
        assert_eq!(budget.storage(), FLOOR + 7);
        assert_eq!(budget.work(), BF16_CALL_QUERY_ENTRY_WORK_V1 + 11);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn scope_drops_panic_payload_and_does_not_hide_floor_debit() {
    struct Payload<'a>(&'a Cell<bool>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    // A panic payload must be 'static; retain the observer separately.
    struct PanicPayload(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for PanicPayload {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
    budget.reserve_storage(FLOOR).unwrap();
    let out: Bf16CallQueryResultV1<()> = bf16_call_query_scope_v1(&mut budget, |_| {
        std::panic::panic_any(PanicPayload(dropped.clone()));
    });
    assert_eq!(out, Err(Bf16NominalCallQueryErrorV1::CallbackPanicked));
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(budget.storage(), FLOOR);
    let scratch_dropped = Cell::new(false);
    let out = bf16_call_query_scope_v1(&mut budget, |b| {
        let _temporary = Payload(&scratch_dropped);
        b.release_storage(1)?;
        Ok(())
    });
    assert_eq!(
        out,
        Err(Bf16NominalCallQueryErrorV1::Resource(
            ArgumentResourceV1::Accounting
        ))
    );
    assert!(scratch_dropped.get());
    // Invalid debit is visible; no fabricated refund repairs the protected floor.
    assert_eq!(budget.storage(), FLOOR + BF16_CALL_QUERY_SCRATCH_V1 - 1);
}

#[test]
fn scope_refuses_ignored_denials_and_replaced_work_ledger() {
    for storage_failure in [false, true] {
        let mut work = Work::new(BF16_CALL_QUERY_ENTRY_WORK_V1);
        let mut budget = ArgumentBudgetV1::new(&mut work, BF16_CALL_QUERY_SCRATCH_V1);
        let out = bf16_call_query_scope_v1(&mut budget, |b| {
            if storage_failure {
                let _ = b.reserve_storage(1);
            } else {
                let _ = b.charge_work(1);
            }
            Ok(())
        });
        assert_eq!(
            out,
            Err(Bf16NominalCallQueryErrorV1::Resource(
                ArgumentResourceV1::Accounting
            ))
        );
        assert_eq!(budget.storage(), 0);
        let entered = Cell::new(false);
        assert!(
            bf16_call_query_scope_v1(&mut budget, |_| {
                entered.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!entered.get());
    }
    let mut first = Work::new(WORK);
    let mut second = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut first, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
    budget.reserve_storage(FLOOR).unwrap();
    let foreign = ArgumentBudgetV1::new(&mut second, FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
    let out = bf16_call_query_scope_v1(&mut budget, move |b| {
        *b = foreign;
        b.reserve_storage(FLOOR + BF16_CALL_QUERY_SCRATCH_V1)?;
        Ok(())
    });
    assert_eq!(
        out,
        Err(Bf16NominalCallQueryErrorV1::Resource(
            ArgumentResourceV1::Accounting
        ))
    );
    assert_eq!(budget.storage(), FLOOR + BF16_CALL_QUERY_SCRATCH_V1);
}

#[test]
fn semantic_resource_errors_are_not_collapsed_to_unavailable() {
    for error in [
        ArgumentResourceV1::Accounting,
        ArgumentResourceV1::Arithmetic,
        ArgumentResourceV1::Allocation,
    ] {
        assert_eq!(
            bf16_query_semantic_error_v1(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)
            ),
            Bf16NominalCallQueryErrorV1::Resource(error)
        );
        assert_eq!(
            bf16_query_inventory_error_v1(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error)
            ),
            Bf16NominalCallQueryErrorV1::Resource(error)
        );
    }
}

#[test]
fn only_nominal_policy_enters_the_query_and_large_copy_results_are_prepaid() {
    assert!(bf16_query_policy_v1(ProductionHelperSourcePolicyV1::Bf16Nominal).is_ok());
    for policy in [
        ProductionHelperSourcePolicyV1::RawEmpty,
        ProductionHelperSourcePolicyV1::UnitLocal,
    ] {
        assert!(bf16_query_policy_v1(policy).is_err());
    }
    type Output = [u64; 512];
    let scratch = bf16_call_query_scratch_v1::<Output>().unwrap();
    assert_eq!(
        scratch,
        BF16_CALL_QUERY_SCRATCH_V1 + 2 * std::mem::size_of::<Output>()
    );
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, scratch - 1);
    let entered = Cell::new(false);
    let result = bf16_call_query_scope_v1(&mut budget, |_| {
        entered.set(true);
        Ok([0u64; 512])
    });
    assert!(result.is_err());
    assert!(!entered.get());
}
