use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingErrorV1 as ForwardError,
    CanonicalKirInductionRefinementErrorV1 as RefineError,
    check_canonical_kir_cross_block_forwarding_v1 as check_forward,
    check_canonical_kir_induction_refinement_v1 as check_refine,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError, Constant,
    KernelIrDecodeError, OperationKind as Kind, ValueId,
};
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingErrorV1 as OwnedForwardError,
    OwnedInductionRefinementErrorV1 as OwnedRefineError,
};

#[test]
fn malformed_literal_wire_is_refused_before_transformation() {
    let valid = hex(CASES[0].wire);
    for mode in 0..5 {
        let mut bytes = valid.clone();
        match mode {
            0 => bytes[0] ^= 1,
            1 => bytes[8] = 11,
            2 => bytes[10] = 1,
            3 => {
                bytes.pop();
            }
            4 => bytes.push(0),
            _ => unreachable!(),
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(43).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = Owner::from_canonical_bytes_with_verification_budget_v12(&bytes, &mut budget);
        let Err(AdmissionError::Decode(error)) = result else {
            panic!("exact wire decode refusal")
        };
        assert_eq!(
            error,
            match mode {
                0 => KernelIrDecodeError::InvalidMagic,
                1 => KernelIrDecodeError::UnknownVersion(11),
                2 => KernelIrDecodeError::UnsupportedFlags(1),
                3 => KernelIrDecodeError::Truncated,
                4 => KernelIrDecodeError::TrailingBytes,
                _ => unreachable!(),
            }
        );
        assert_eq!(budget.storage(), 43);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn actual_chain_rejects_wrong_input_and_incomplete_lineage() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, input_storage) = decode(&hex(CASES[0].wire), &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let (foreign, foreign_storage) = decode(&hex(CASES[1].wire), &mut budget);
    budget.reserve_storage(foreign_storage).unwrap();
    let refined = refine(&input, Default::default(), &mut budget).unwrap();
    budget.reserve_storage(refined.retained_storage()).unwrap();
    let final_owner = forward(refined.output(), Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(final_owner.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        refined.replay_against(&foreign, refined.limits(), &mut budget),
        Err(OwnedRefineError::ForeignInput)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        final_owner.replay_against(&input, &mut budget),
        Err(OwnedForwardError::ForeignInput)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        check_refine(&input, refined.output(), &[], refined.limits(), &mut budget),
        Err(RefineError::Mismatch("complete original operation roster"))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        check_forward(
            refined.output(),
            final_owner.output(),
            &[],
            final_owner.limits(),
            &mut budget
        ),
        Err(ForwardError::Mismatch("complete operation cardinality"))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let size = final_owner.retained_storage();
    drop(final_owner);
    budget.release_storage(size).unwrap();
    let size = refined.retained_storage();
    drop(refined);
    budget.release_storage(size).unwrap();
    drop(foreign);
    budget.release_storage(foreign_storage).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn fresh_admitted_hostile_outputs_fail_independent_actual_pair_checks() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, input_storage) = decode(&hex(CASES[0].wire), &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let refined = refine(&input, Default::default(), &mut budget).unwrap();
    budget.reserve_storage(refined.retained_storage()).unwrap();
    let final_owner = forward(refined.output(), Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(final_owner.retained_storage())
        .unwrap();
    let floor = budget.storage();
    for mutate_refinement in [true, false] {
        // Mutations are harness-owned candidate input. Actual hostile endpoints
        // must pass ordinary full admission before the independent pair check.
        let mut candidate = if mutate_refinement {
            refined.output()
        } else {
            final_owner.output()
        }
        .module()
        .clone();
        if mutate_refinement {
            let at = refined
                .origins()
                .iter()
                .find_map(|row| match row {
                    RefineRow::CheckedAddSplit { false_output, .. } => Some(*false_output),
                    _ => None,
                })
                .unwrap();
            candidate.functions[at.block.function.0 as usize]
                .body
                .as_mut()
                .unwrap()
                .blocks[at.block.block as usize]
                .operations[at.operation as usize]
                .kind = Kind::Constant(Constant::Bool(true));
        } else {
            let at = final_owner
                .origins()
                .iter()
                .find(|row| row.store.is_some())
                .unwrap()
                .output;
            // A genuinely admitted different operation on the available value.
            candidate.functions[at.block.function.0 as usize]
                .body
                .as_mut()
                .unwrap()
                .blocks[at.block.block as usize]
                .operations[at.operation as usize]
                .kind = Kind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(2),
                rhs: ValueId(2),
            };
        }
        let (hostile, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&candidate, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        if mutate_refinement {
            assert!(matches!(
                check_refine(
                    &input,
                    &hostile,
                    refined.origins(),
                    refined.limits(),
                    &mut budget
                ),
                Err(RefineError::Mismatch(
                    "exact sum and false payloads and result IDs"
                ))
            ));
        } else {
            assert!(matches!(
                check_forward(
                    refined.output(),
                    &hostile,
                    final_owner.origins(),
                    final_owner.limits(),
                    &mut budget
                ),
                Err(ForwardError::Mismatch("exact cross-block typed Load copy"))
            ));
        }
        assert_eq!(budget.storage(), floor + receipt.retained_storage());
        drop(hostile);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    let size = final_owner.retained_storage();
    drop(final_owner);
    budget.release_storage(size).unwrap();
    let size = refined.retained_storage();
    drop(refined);
    budget.release_storage(size).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn literal_ungrounded_phi_cannot_be_labeled_as_a_forwarding_proof() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, input_storage) = decode(&hex(CASES[4].wire), &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let actual = forward(&input, Default::default(), &mut budget).unwrap();
    budget.reserve_storage(actual.retained_storage()).unwrap();
    assert!(actual.origins().iter().all(|row| row.store.is_none()));
    let mut rows = actual.origins().to_vec();
    let store = rows
        .iter()
        .find(|row| {
            matches!(
                harness::operation(input.module(), row.input).kind,
                Kind::Store { .. }
            )
        })
        .unwrap()
        .input;
    let load = rows
        .iter_mut()
        .find(|row| {
            matches!(
                harness::operation(input.module(), row.input).kind,
                Kind::Load { .. }
            )
        })
        .unwrap();
    load.store = Some(store);
    let at = load.output;
    let mut candidate = input.module().clone();
    candidate.functions[at.block.function.0 as usize]
        .body
        .as_mut()
        .unwrap()
        .blocks[at.block.block as usize]
        .operations[at.operation as usize]
        .kind = Kind::Binary {
        op: BinaryOp::BitOr,
        lhs: ValueId(2),
        rhs: ValueId(2),
    };
    let (hostile, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&candidate, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        check_forward(&input, &hostile, &rows, actual.limits(), &mut budget),
        Err(ForwardError::Mismatch(
            "each phi needs an actual Store path"
        ))
    ));
    assert_eq!(budget.storage(), floor);
    drop(hostile);
    budget.release_storage(storage.retained_storage()).unwrap();
    let size = actual.retained_storage();
    drop(actual);
    budget.release_storage(size).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}
