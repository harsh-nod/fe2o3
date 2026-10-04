use super::{binary_count, identity, reverse_layout_chain};
use crate::checked_load_forwarding_v1::tests::{STORAGE, WORK};
use crate::*;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirTransitionCandidateV1 as Candidate,
    Constant, InertCanonicalKirTransitionReceiptV1 as Wire, Module, Operation,
    OperationKind as Kind, ScalarType, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::read_unauthenticated_integer_continuation_claim_v1 as read_claim;
use std::mem::size_of_val;
type Error = CanonicalPolicy6SemanticErrorV1;
const PREFIX: usize = 29;

struct Prepared {
    input: Owner,
    input_storage: usize,
    checked: CheckedCanonicalKernelIrOwnerPolicy6V1,
    p4: InertCanonicalPolicy4ExecutionReceiptV1,
    transition: Wire,
    transition_storage: usize,
}
impl Prepared {
    fn inputs(&self) -> CanonicalPolicy6SemanticInputsV1<'_> {
        let p5 = self.checked.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        CanonicalPolicy6SemanticInputsV1 {
            prefix: CanonicalPolicy5SemanticInputsV1 {
                input: &self.input,
                intermediate: p4.intermediate_policy3().owner(),
                stored: p4.owner(),
                output: p5.owner(),
                policy4_wire: self.p4.canonical_bytes(),
                policy5_record: p5.execution().canonical_bytes(),
                load_rows: p5.load_forwarding_rows(),
            },
            output: self.checked.owner(),
            continuation: CanonicalPolicy6ContinuationClaimsV1 {
                composition_record: self.checked.execution().canonical_bytes(),
                integer_record: self.checked.continuation().execution().canonical_bytes(),
                transition_wire: self.transition.canonical_bytes(),
            },
        }
    }
    fn floor(&self) -> usize {
        PREFIX
            + self.input_storage
            + self.checked.retained_storage()
            + self.p4.storage().retained_storage()
            + self.transition_storage
    }
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}

fn prepared(module: &Module) -> Prepared {
    let (input, input_storage) = admit(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_storage).unwrap();
    let p5 = optimize_checked_canonical_kernel_ir_policy5_v1(&input, &mut budget).unwrap();
    budget.reserve_storage(p5.retained_storage()).unwrap();
    let checked = continue_checked_canonical_kernel_ir_policy6_v1(&input, p5, &mut budget).unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    let p4 = encode_checked_canonical_policy4_execution_receipt_v1(
        &input,
        checked.intermediate_policy5().intermediate_policy4(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(p4.storage().retained_storage())
        .unwrap();
    let (transition, storage) = Wire::from_candidate_with_budget(
        checked
            .intermediate_policy5()
            .owner()
            .canonical()
            .identity(),
        checked.owner().canonical().identity(),
        checked.continuation().occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        budget.storage(),
        input_storage + checked.retained_storage() + p4.storage().retained_storage()
    );
    Prepared {
        input,
        input_storage,
        checked,
        p4,
        transition,
        transition_storage: storage.retained_storage(),
    }
}

fn semantic(
    inputs: CanonicalPolicy6SemanticInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    let receipt = check_published_policy6_semantic_relation_v1(inputs, budget)?;
    assert!(std::ptr::eq(
        receipt.policy5_relation().output(),
        inputs.prefix.output
    ));
    assert!(std::ptr::eq(
        receipt.continuation().input(),
        inputs.prefix.output
    ));
    assert!(std::ptr::eq(receipt.continuation().output(), inputs.output));
    assert!(!receipt.authenticates_execution());
    assert!(!receipt.grants_authority());
    let storage = receipt.storage().retained_storage();
    assert_eq!(
        storage,
        receipt.policy5_relation().storage().retained_storage()
            + receipt.continuation().storage().retained_storage()
            + std::mem::size_of::<ReplayedPolicy6SemanticRelationV1<'_>>()
            - std::mem::size_of::<ReplayedPolicy5SemanticRelationV1<'_>>()
            - std::mem::size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>()
    );
    budget.reserve_storage(storage)?;
    drop(receipt);
    budget.release_storage(storage)?;
    Ok(storage)
}

fn authenticated(
    prepared: &Prepared,
    claims: CanonicalPolicy6ContinuationClaimsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    let input = prepared.inputs();
    let receipt = check_canonical_policy6_execution_relation_v1(
        &prepared.input,
        &prepared.checked,
        input.prefix.policy4_wire,
        input.prefix.policy5_record,
        input.prefix.load_rows,
        claims,
        budget,
    )?;
    assert!(std::ptr::eq(receipt.execution_owner(), &prepared.checked));
    assert!(std::ptr::eq(
        receipt.policy5_execution().execution_owner(),
        prepared.checked.intermediate_policy5()
    ));
    assert!(std::ptr::eq(
        receipt.continuation().input(),
        input.prefix.output
    ));
    assert!(std::ptr::eq(receipt.continuation().output(), input.output));
    assert_eq!(
        receipt.authenticated_integer_record(),
        prepared
            .checked
            .continuation()
            .execution()
            .canonical_bytes()
    );
    assert!(receipt.authenticates_execution());
    assert!(!receipt.grants_authority());
    let storage = receipt.storage().retained_storage();
    assert_eq!(
        storage,
        receipt.policy5_execution().storage().retained_storage()
            + receipt.continuation().storage().retained_storage()
            + std::mem::size_of::<CheckedCanonicalPolicy6ExecutionRelationV1<'_>>()
            - std::mem::size_of::<CheckedCanonicalPolicy5ExecutionRelationV1<'_>>()
            - std::mem::size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>()
    );
    budget.reserve_storage(storage)?;
    drop(receipt);
    budget.release_storage(storage)?;
    Ok(storage)
}

fn mutation() -> Module {
    let mut module = identity(Constant::U32(0), true);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let ty = Type::Scalar(ScalarType::U32);
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(7), ty.clone()),
        Kind::Constant(Constant::U32(3)),
    ));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(8), ty),
        Kind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(5),
            rhs: ValueId(7),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(8), ValueId(4)],
    });
    module
}

#[test]
fn portable_policy6_mutation_noop_and_all_integer_flag_profiles_use_real_owners() {
    let mut modules = vec![
        Module::new("no-op"),
        mutation(),
        identity(Constant::U32(3), true),
    ];
    for zero in [
        Constant::I8(0),
        Constant::I16(0),
        Constant::I32(0),
        Constant::I64(0),
        Constant::U8(0),
        Constant::U16(0),
        Constant::U32(0),
        Constant::U64(0),
    ] {
        modules.push(identity(zero.clone(), false));
        modules.push(identity(zero, true));
    }
    for module in modules {
        let prepared = prepared(&module);
        let inputs = prepared.inputs();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(prepared.floor()).unwrap();
        budget.charge_work(13).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        semantic(inputs, &mut budget).unwrap();
        let before = budget.work();
        authenticated(&prepared, inputs.continuation, &mut budget).unwrap();
        assert!(budget.work() > before && before > 13);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), prepared.floor());
    }
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    assert_ne!(
        inputs.prefix.output.canonical().canonical_bytes(),
        inputs.output.canonical().canonical_bytes()
    );
    assert_eq!(binary_count(inputs.output), 1, "neighboring xor3 survives");
    assert_ne!(
        prepared.checked.continuation().map().digest(),
        prepared.transition.digest(),
        "observed map and F2NTR have different domains and meanings"
    );
}

#[test]
fn mutually_consistent_changed_map_and_work_claims_are_semantic_not_execution() {
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    for change_work in [false, true] {
        let mut composition: [u8; 256] = inputs.continuation.composition_record.try_into().unwrap();
        let mut integer: [u8; 416] = inputs.continuation.integer_record.try_into().unwrap();
        if change_work {
            integer[96] ^= 1;
        } else {
            composition[216] ^= 1;
            integer[264] ^= 1;
        }
        let claims = CanonicalPolicy6ContinuationClaimsV1 {
            composition_record: &composition,
            integer_record: &integer,
            ..inputs.continuation
        };
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + size_of_val(&composition) + size_of_val(&integer);
        budget.reserve_storage(floor).unwrap();
        semantic(
            CanonicalPolicy6SemanticInputsV1 {
                continuation: claims,
                ..inputs
            },
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            authenticated(&prepared, claims, &mut budget),
            Err(Error::ExecutionWitness)
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn composition_checks_every_byte_and_all_five_real_subjects() {
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    for offset in 0..256 {
        let mut composition: [u8; 256] = inputs.continuation.composition_record.try_into().unwrap();
        composition[offset] ^= 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + size_of_val(&composition);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(
                semantic(
                    CanonicalPolicy6SemanticInputsV1 {
                        continuation: CanonicalPolicy6ContinuationClaimsV1 {
                            composition_record: &composition,
                            ..inputs.continuation
                        },
                        ..inputs
                    },
                    &mut budget
                ),
                Err(Error::Composition)
            ),
            "byte {offset}"
        );
        assert_eq!(budget.storage(), floor);
    }
    let (foreign, storage) = admit(&Module::new("foreign"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = prepared.floor() + storage;
    budget.reserve_storage(floor).unwrap();
    for axis in 0..5 {
        let mut changed = inputs;
        match axis {
            0 => changed.prefix.input = &foreign,
            1 => changed.prefix.intermediate = &foreign,
            2 => changed.prefix.stored = &foreign,
            3 => changed.prefix.output = &foreign,
            _ => changed.output = &foreign,
        }
        assert!(semantic(changed, &mut budget).is_err(), "subject {axis}");
        assert_eq!(budget.storage(), floor);
    }
    assert!(
        semantic(
            CanonicalPolicy6SemanticInputsV1 {
                output: inputs.prefix.output,
                prefix: CanonicalPolicy5SemanticInputsV1 {
                    output: inputs.output,
                    ..inputs.prefix
                },
                ..inputs
            },
            &mut budget
        )
        .is_err()
    );
}

#[test]
fn exact_lengths_nested_syntax_and_transition_subjects_remain_mandatory() {
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    let mut long = [0; 257];
    long[..256].copy_from_slice(inputs.continuation.composition_record);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = prepared.floor() + long.len();
    budget.reserve_storage(floor).unwrap();
    for record in [&long[..], &long[..255], &[]] {
        assert!(matches!(
            semantic(
                CanonicalPolicy6SemanticInputsV1 {
                    continuation: CanonicalPolicy6ContinuationClaimsV1 {
                        composition_record: record,
                        ..inputs.continuation
                    },
                    ..inputs
                },
                &mut budget
            ),
            Err(Error::Composition)
        ));
    }
    assert!(matches!(
        semantic(
            CanonicalPolicy6SemanticInputsV1 {
                prefix: CanonicalPolicy5SemanticInputsV1 {
                    policy4_wire: &[],
                    ..inputs.prefix
                },
                ..inputs
            },
            &mut budget
        ),
        Err(Error::Policy5(_))
    ));
    let mut wire = inputs.continuation.transition_wire.to_vec();
    let extra = wire.capacity();
    budget.reserve_storage(extra).unwrap();
    for offset in [0, 8, 20, 52] {
        wire[offset] ^= 1;
        assert!(matches!(
            semantic(
                CanonicalPolicy6SemanticInputsV1 {
                    continuation: CanonicalPolicy6ContinuationClaimsV1 {
                        transition_wire: &wire,
                        ..inputs.continuation
                    },
                    ..inputs
                },
                &mut budget
            ),
            Err(Error::Transition(_))
        ));
        wire[offset] ^= 1;
    }
    drop(wire);
    budget.release_storage(extra).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn reframed_wrong_i_and_missing_duplicate_reordered_occurrences_fail_semantic_replay() {
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    let original = prepared.checked.continuation().occurrences().candidate();
    assert!(original.operations.len() >= 2);
    for mode in 0..4 {
        let mut operations = original.operations.to_vec();
        match mode {
            0 => {
                operations.pop();
            }
            1 => operations.push(operations[0]),
            2 => operations.swap(0, 1),
            _ => operations[0].output.operation = 999,
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + operations.capacity() * size_of_val(&operations[0]);
        budget.reserve_storage(floor).unwrap();
        let (wire, storage) = Wire::from_candidate_with_budget(
            inputs.prefix.output.canonical().identity(),
            inputs.output.canonical().identity(),
            Candidate {
                operations: &operations,
                ..original
            },
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(matches!(
            semantic(
                CanonicalPolicy6SemanticInputsV1 {
                    continuation: CanonicalPolicy6ContinuationClaimsV1 {
                        transition_wire: wire.canonical_bytes(),
                        ..inputs.continuation
                    },
                    ..inputs
                },
                &mut budget
            ),
            Err(Error::Transition(_))
        ));
        drop(wire);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    let mut module = inputs.output.module().clone();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let op = block
        .operations
        .iter_mut()
        .find(|op| {
            matches!(
                op.kind,
                Kind::Binary {
                    op: BinaryOp::BitXor,
                    ..
                }
            )
        })
        .unwrap();
    let (lhs, rhs) = match op.kind {
        Kind::Binary { lhs, rhs, .. } => Some((lhs, rhs)),
        _ => None,
    }
    .unwrap();
    op.kind = Kind::Binary {
        op: BinaryOp::BitOr,
        lhs,
        rhs,
    };
    let (wrong, wrong_storage) = admit(&module);
    let mut composition: [u8; 256] = inputs.continuation.composition_record.try_into().unwrap();
    let mut integer: [u8; 416] = inputs.continuation.integer_record.try_into().unwrap();
    for (bytes, offset) in [(&mut composition[..], 176), (&mut integer[..], 48)] {
        bytes[offset..offset + 32].copy_from_slice(wrong.canonical().identity().digest());
        bytes[offset + 32..offset + 40].copy_from_slice(
            &wrong
                .canonical()
                .identity()
                .canonical_length()
                .to_le_bytes(),
        );
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(prepared.floor() + wrong_storage + 672)
        .unwrap();
    let (wire, storage) = Wire::from_candidate_with_budget(
        inputs.prefix.output.canonical().identity(),
        wrong.canonical().identity(),
        original,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        semantic(
            CanonicalPolicy6SemanticInputsV1 {
                output: &wrong,
                continuation: CanonicalPolicy6ContinuationClaimsV1 {
                    composition_record: &composition,
                    integer_record: &integer,
                    transition_wire: wire.canonical_bytes()
                },
                ..inputs
            },
            &mut budget
        ),
        Err(Error::Transition(_))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn independently_admitted_equal_bytes_keep_actual_supplied_output_borrow() {
    let prepared = prepared(&mutation());
    let inputs = prepared.inputs();
    let (other, storage) = admit(inputs.output.module());
    assert!(!std::ptr::eq(inputs.output, &other));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(prepared.floor() + storage).unwrap();
    semantic(
        CanonicalPolicy6SemanticInputsV1 {
            output: &other,
            ..inputs
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), prepared.floor() + storage);
}

#[test]
fn reverse_physical_order_continuations_remain_single_sweep_claims_not_a_fixpoint() {
    let (input, input_storage) = admit(&reverse_layout_chain());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_storage).unwrap();
    let first = fe2o3_pliron::optimize_native_neutral_kernel_ir_integer_continuation_v1(
        &input,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(first.storage().retained_storage())
        .unwrap();
    let first = first.try_check_and_finish_v1(&mut budget).unwrap();
    budget
        .reserve_storage(first.storage().retained_storage())
        .unwrap();
    assert_eq!(binary_count(first.owner()), 1);
    read_claim(
        &input,
        first.owner(),
        first.execution().canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    let second = fe2o3_pliron::optimize_native_neutral_kernel_ir_integer_continuation_v1(
        first.owner(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(second.storage().retained_storage())
        .unwrap();
    let second = second.try_check_and_finish_v1(&mut budget).unwrap();
    budget
        .reserve_storage(second.storage().retained_storage())
        .unwrap();
    assert_eq!(binary_count(second.owner()), 0);
    read_claim(
        first.owner(),
        second.owner(),
        second.execution().canonical_bytes(),
        &mut budget,
    )
    .unwrap();
}

#[test]
fn exact_and_one_short_resource_bounds_preserve_same_ledger_floor_and_determinism() {
    for module in [mutation(), Module::new("no-op")] {
        let prepared = prepared(&module);
        for auth in [false, true] {
            let run = |budget: &mut Budget<'_>| {
                if auth {
                    authenticated(&prepared, prepared.inputs().continuation, budget)
                } else {
                    semantic(prepared.inputs(), budget)
                }
            };
            let (spent, peak, receipt) = {
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(prepared.floor()).unwrap();
                budget.charge_work(13).unwrap();
                let receipt = run(&mut budget).unwrap();
                (budget.work(), budget.peak_storage(), receipt)
            };
            for (limit, ceiling, succeeds) in [
                (spent, peak, true),
                (spent - 1, peak, false),
                (spent, peak - 1, false),
            ] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, ceiling);
                budget.reserve_storage(prepared.floor()).unwrap();
                budget.charge_work(13).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = run(&mut budget);
                assert_eq!(result.is_ok(), succeeds);
                if succeeds {
                    assert_eq!(result.unwrap(), receipt);
                    assert_eq!(budget.work(), spent);
                }
                assert_eq!(budget.storage(), prepared.floor());
                assert!(budget.work_ledger_identity_v1() == ledger);
            }
        }
    }
    let a = prepared(&mutation());
    let b = prepared(&mutation());
    assert_eq!(
        a.transition.canonical_bytes(),
        b.transition.canonical_bytes()
    );
    assert_eq!(
        a.checked.execution().canonical_bytes(),
        b.checked.execution().canonical_bytes()
    );
    assert_eq!(
        a.checked.continuation().execution().canonical_bytes(),
        b.checked.continuation().execution().canonical_bytes()
    );
}
