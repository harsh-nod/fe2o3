use super::*;
use crate::scalar_fixed_point_history_v1::tests::*;

fn with_decoded<T>(
    input: &Owner,
    input_paid: usize,
    bytes: &[u8],
    run: impl FnOnce(&DecodedScalarFixedPointHistoryV1<'_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(input_paid + bytes.len() + SIBLING)
        .unwrap();
    let framed = crate::read_scalar_fixed_point_history_v1(bytes, &mut b).unwrap();
    b.reserve_storage(framed.storage().retained_storage())
        .unwrap();
    let decoded = materialize_scalar_fixed_point_history_v1(&framed, &mut b).unwrap();
    b.reserve_storage(decoded.storage().retained_storage())
        .unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let result = run(&decoded, &mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(
        input.canonical().canonical_bytes().len(),
        input.canonical().identity().canonical_length() as usize
    );
    drop(decoded);
    drop(framed);
    b.release_storage(floor - input_paid - bytes.len() - SIBLING)
        .unwrap();
    result
}

#[test]
fn decoded_complete_rounds_replay_without_aliasing_roles_or_granting_execution() {
    for module in [Module::new("fixed"), reverse_chain(1), reverse_chain(8)] {
        let (input, paid, wire) = produced(&module);
        with_decoded(&input, paid, wire.canonical_bytes(), |decoded, b| {
            let checked = decoded.check_semantics(&input, b).unwrap();
            let retained = checked.storage().retained_storage();
            b.reserve_storage(retained).unwrap();
            assert_eq!(checked.rounds().len(), decoded.frame().rounds().len());
            let mut before = &input;
            for (i, round) in checked.rounds().iter().enumerate() {
                assert!(std::ptr::eq(round.input(), before));
                assert!(!std::ptr::eq(round.integer_output(), round.output()));
                assert_eq!(
                    before.canonical().canonical_bytes()
                        == round.output().canonical().canonical_bytes(),
                    i + 1 == checked.rounds().len()
                );
                assert!(!round.integer_claim().grants_authority());
                assert!(!round.scalar_relation().grants_authority());
                before = round.output();
            }
            assert!(std::ptr::eq(checked.output(), decoded.output()));
            assert!(!checked.grants_authority());
            assert!(!checked.authenticates_execution());
            assert!(!decoded.grants_authority());
            assert!(!decoded.authenticates_execution());
            checked.replay(b).unwrap();
            drop(checked);
            b.release_storage(retained).unwrap();
        });
    }
}

#[test]
fn missing_terminal_and_early_terminal_are_typed_semantic_refusals() {
    let (input, paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    assert_eq!(framed.rounds().len(), 3);
    let rows = fields(&framed);
    let shortened = raw(framed.execution_claim(), &rows[..2]);
    with_decoded(&input, paid, &shortened, |d, b| {
        assert!(matches!(
            d.check_semantics(&input, b),
            Err(Error::Terminal { ordinal: 1 })
        ))
    });
    let mut extra = rows.clone();
    extra.push(rows[2]);
    let extended = raw(framed.execution_claim(), &extra);
    with_decoded(&input, paid, &extended, |d, b| {
        assert!(matches!(
            d.check_semantics(&input, b),
            Err(Error::Terminal { ordinal: 2 })
        ))
    });
}

#[test]
fn exact_endpoint_middle_adjacency_child_domains_and_foreign_input_are_checked() {
    let (input, paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    let mut swapped = fields(&framed);
    swapped.swap(0, 1);
    let hostile = raw(framed.execution_claim(), &swapped);
    with_decoded(&input, paid, &hostile, |d, b| {
        assert!(matches!(
            d.check_semantics(&input, b),
            Err(Error::IntegerClaim(_))
        ))
    });
    for (field, expected) in [(2, 0), (3, 1), (4, 2)] {
        let mut changed = framed.rounds()[1].fields[field].to_vec();
        changed[0] ^= 0x80;
        let mut rows = fields(&framed);
        rows[1][field] = &changed;
        let hostile = raw(framed.execution_claim(), &rows);
        with_decoded(&input, paid, &hostile, |d, b| {
            match (d.check_semantics(&input, b), expected) {
                (Err(Error::IntegerClaim(_)), 0)
                | (Err(Error::Semantic(_)), 1)
                | (Err(Error::Policy3(_)), 2) => {}
                (Err(e), _) => panic!("wrong child refusal: {e}"),
                _ => panic!("hostile middle admitted"),
            }
        });
    }
    let (foreign, foreign_paid) =
        crate::scalar_fixed_point_history_v1::tests::admit(&reverse_chain(2));
    with_decoded(&foreign, foreign_paid, wire.canonical_bytes(), |d, b| {
        assert!(matches!(
            d.check_semantics(&foreign, b),
            Err(Error::Composition)
        ))
    });
    let mut endpoint = wire.canonical_bytes().to_vec();
    endpoint[48] ^= 1;
    with_decoded(&input, paid, &endpoint, |d, b| {
        assert!(matches!(
            d.check_semantics(&input, b),
            Err(Error::Composition)
        ))
    });
}

#[test]
fn all_graph_roles_are_freshly_admitted_including_terminal() {
    let (input, paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    for ordinal in 0..framed.rounds().len() {
        for field in 0..2 {
            let mut bad = framed.rounds()[ordinal].fields[field].to_vec();
            bad[0] ^= 0x80;
            let mut rows = fields(&framed);
            rows[ordinal][field] = &bad;
            let wire = raw(framed.execution_claim(), &rows);
            let frame = frame(&wire);
            let mut work = Work::new(W);
            let mut b = Budget::new(&mut work, S);
            let floor = paid + wire.len() + frame.storage().retained_storage() + SIBLING;
            b.reserve_storage(floor).unwrap();
            assert!(
                matches!(materialize_scalar_fixed_point_history_v1(&frame, &mut b), Err(Error::Admission { ordinal: actual, integer, .. }) if actual == ordinal && integer == (field == 0))
            );
            assert_eq!(b.storage(), floor);
        }
    }
    assert!(!input.canonical().canonical_bytes().is_empty());
}

#[test]
fn strict_ieee_and_effect_changes_refuse_even_with_refreshed_integer_endpoint_receipts() {
    use fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1 as Transition;
    for constant in [
        Constant::F32Bits(0),
        Constant::F32Bits(0x8000_0000),
        Constant::F32Bits(0x7fc0_0017),
        Constant::F64Bits(0),
        Constant::F64Bits(0x8000_0000_0000_0000),
        Constant::F64Bits(0x7ff8_0000_0000_0017),
    ] {
        let module = effect_module(constant);
        let (input, paid, wire) = produced(&module);
        let framed = frame(wire.canonical_bytes());
        with_decoded(&input, paid, wire.canonical_bytes(), |d, b| {
            let checked = d.check_semantics(&input, b).unwrap();
            let operations = &checked.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0]
                .operations;
            assert!(operations.iter().any(|op| matches!(
                op.kind,
                Kind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )));
            assert!(
                operations
                    .iter()
                    .any(|op| matches!(op.kind, Kind::Store { .. }))
            );
        });
        for change_effect in [false, true] {
            let mut hostile_module = module.clone();
            let operations =
                &mut hostile_module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            if change_effect {
                operations.pop();
            } else {
                operations[1].kind = Kind::Binary {
                    op: BinaryOp::Subtract,
                    lhs: ValueId(0),
                    rhs: ValueId(2),
                };
            }
            let (donor, donor_paid) =
                crate::scalar_fixed_point_history_v1::tests::admit(&hostile_module);
            let mut work = Work::new(W);
            let mut b = Budget::new(&mut work, S);
            b.reserve_storage(paid + donor_paid + wire.storage().retained_storage() + SIBLING)
                .unwrap();
            let (old, old_paid) =
                Transition::decode_with_budget(framed.rounds()[0].integer_transition(), &mut b)
                    .unwrap();
            b.reserve_storage(old_paid.retained_storage()).unwrap();
            let (refreshed, _) = Transition::from_candidate_with_budget(
                input.canonical().identity(),
                donor.canonical().identity(),
                old.candidate(),
                &mut b,
            )
            .unwrap();
            let mut claim = framed.rounds()[0].integer_record().to_vec();
            claim[48..80].copy_from_slice(donor.canonical().identity().digest());
            claim[80..88].copy_from_slice(
                &donor
                    .canonical()
                    .identity()
                    .canonical_length()
                    .to_le_bytes(),
            );
            let mut rows = fields(&framed);
            rows[0][0] = donor.canonical().canonical_bytes();
            rows[0][2] = &claim;
            rows[0][3] = refreshed.canonical_bytes();
            let hostile = raw(framed.execution_claim(), &rows);
            with_decoded(&input, paid, &hostile, |d, b| {
                assert!(matches!(
                    d.check_semantics(&input, b),
                    Err(Error::Semantic(_))
                ))
            });
        }
    }
}

#[test]
fn checked_integer_overflow_and_both_effects_survive_full_history() {
    for constant in [
        Constant::U8(1),
        Constant::I8(1),
        Constant::U32(1),
        Constant::I32(1),
        Constant::U64(1),
        Constant::I64(1),
    ] {
        let ty = constant.ty();
        let width = match ty {
            Type::Scalar(ScalarType::I8 | ScalarType::U8) => 1,
            Type::Scalar(ScalarType::I64 | ScalarType::U64) => 8,
            _ => 4,
        };
        let mut entry = block(17, Terminator::Return { values: vec![] });
        entry.operations = vec![
            op(3, ty.clone(), Kind::Constant(constant)),
            Operation::checked_binary(
                ValueDef::new(ValueId(4), ty.clone()),
                ValueDef::new(ValueId(5), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(0),
                ValueId(3),
            ),
            Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(1),
                    value: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, width),
                },
            ),
            Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(2),
                    value: ValueId(5),
                    access: MemoryAccess::new(AddressSpace::Global, 1),
                },
            ),
        ];
        let mut module = Module::new("checked-overflow-history");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(
                vec![
                    ty.clone(),
                    Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite),
                    Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![entry],
        ));
        let (input, paid, wire) = produced(&module);
        with_decoded(&input, paid, wire.canonical_bytes(), |d, b| {
            let checked = d.check_semantics(&input, b).unwrap();
            let operations = &checked.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0]
                .operations;
            assert!(operations.iter().any(|op| matches!(
                op.kind,
                Kind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    ..
                }
            ) && op.results.len() == 2));
            assert_eq!(
                operations
                    .iter()
                    .filter(|op| matches!(op.kind, Kind::Store { .. }))
                    .count(),
                2
            );
        });
    }
}

#[test]
fn dynamic_report_claims_are_not_misrepresented_as_observed_execution() {
    let (input, paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    let mut claim = framed.rounds()[0].integer_record().to_vec();
    claim[96..104].copy_from_slice(&u64::MAX.to_le_bytes());
    claim[264..296].fill(0x57);
    let mut rows = fields(&framed);
    rows[0][2] = &claim;
    let inert = raw(framed.execution_claim(), &rows);
    with_decoded(&input, paid, &inert, |d, b| {
        let checked = d.check_semantics(&input, b).unwrap();
        assert_eq!(
            checked.rounds()[0].integer_claim().declared_profile_work(),
            u64::MAX
        );
        assert_eq!(
            checked.rounds()[0].integer_claim().declared_map_digest(),
            &[0x57; 32]
        );
        assert!(!checked.authenticates_execution());
        assert!(!checked.grants_authority());
    });
}

fn materialize_trace(framed: &Frame<'_>, floor: usize) -> (Trace, usize) {
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    let mut retained = size_of::<DecodedScalarFixedPointHistoryV1<'_, '_>>();
    trace.reserve(retained);
    retained += trace.table::<DecodedScalarFixedPointRoundV1>(framed.rounds().len());
    for round in framed.rounds() {
        trace.work(1);
        for bytes in [round.integer_output_bytes(), round.output_bytes()] {
            let child = child(trace.live, |b| {
                Owner::from_canonical_bytes_with_verification_budget_v12(bytes, b)
                    .unwrap()
                    .1
                    .retained_storage()
            });
            retained += child.retained;
            trace.child(child);
        }
        trace.work(1);
    }
    trace.work(1);
    (trace, retained)
}
fn semantic_trace(
    decoded: &DecodedScalarFixedPointHistoryV1<'_, '_>,
    input: &Owner,
    floor: usize,
) -> (Trace, usize) {
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    let mut retained = size_of::<ReplayedScalarFixedPointHistoryV1<'_, '_, '_>>();
    trace.reserve(retained);
    trace.reserve(128 + size_of::<[usize; 3]>() + size_of::<bool>());
    trace.work(4 + 256 + 257);
    retained += trace.table::<ReplayedScalarFixedPointRoundV1<'_, '_>>(decoded.rounds().len());
    let mut before = input;
    for (round, wire) in decoded.rounds().iter().zip(decoded.frame().rounds()) {
        trace.work(1 + 2 + 416);
        let integer = child(trace.live, |b| {
            decode_and_check_canonical_optimization_receipt_v1(
                before,
                round.integer_output(),
                wire.integer_transition(),
                b,
            )
            .unwrap()
            .storage()
            .retained_storage()
        });
        retained += integer.retained;
        trace.child(integer);
        let scalar = child(trace.live, |b| {
            decode_and_check_published_policy3_semantic_relation_v1(
                round.integer_output(),
                round.output(),
                wire.policy3_receipt(),
                b,
            )
            .unwrap()
            .storage()
            .retained_storage()
        });
        retained += scalar.retained;
        trace.child(scalar);
        trace.work(
            before.canonical().canonical_bytes().len()
                + round.output().canonical().canonical_bytes().len()
                + 1,
        );
        trace.work(1);
        before = round.output();
    }
    trace.work(1);
    (trace, retained)
}

#[test]
fn materializer_and_semantic_replay_have_independent_child_composed_resources() {
    let (input, input_paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    let floor = input_paid
        + wire.storage().retained_storage()
        + framed.storage().retained_storage()
        + SIBLING;
    let (expected, retained) = materialize_trace(&framed, floor);
    for limit in [expected.work, expected.work - 1] {
        let mut work = Work::new(limit);
        {
            let mut b = Budget::new(&mut work, expected.peak);
            b.reserve_storage(floor).unwrap();
            match materialize_scalar_fixed_point_history_v1(&framed, &mut b) {
                Ok(decoded) => {
                    assert_eq!(limit, expected.work);
                    assert_eq!(decoded.storage().retained_storage(), retained);
                }
                Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, expected.work - 1),
                Err(e) => panic!("{e}"),
            }
            assert_eq!(
                (b.work(), b.peak_storage(), b.storage()),
                (limit, expected.peak, floor)
            );
        }
        assert_eq!(
            work.failed_work(),
            if limit == expected.work {
                None
            } else {
                Some(expected.work)
            }
        );
    }
    with_decoded(&input, input_paid, wire.canonical_bytes(), |decoded, b| {
        let floor = b.storage();
        let (expected, retained) = semantic_trace(decoded, &input, floor);
        for limit in [expected.work, expected.work - 1] {
            let mut work = Work::new(limit);
            {
                let mut b = Budget::new(&mut work, expected.peak);
                b.reserve_storage(floor).unwrap();
                match decoded.check_semantics(&input, &mut b) {
                    Ok(checked) => {
                        assert_eq!(limit, expected.work);
                        assert_eq!(checked.storage().retained_storage(), retained);
                    }
                    Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, expected.work - 1),
                    Err(e) => panic!("{e}"),
                }
                assert_eq!(
                    (b.work(), b.peak_storage(), b.storage()),
                    (limit, expected.peak, floor)
                );
            }
            assert_eq!(
                work.failed_work(),
                if limit == expected.work {
                    None
                } else {
                    Some(expected.work)
                }
            );
        }
    });
}

#[test]
fn materializer_initial_header_and_slot_denials_are_independently_reachable() {
    let (_, input_paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    let floor = input_paid
        + wire.storage().retained_storage()
        + framed.storage().retained_storage()
        + SIBLING;
    let meter = size_of::<Meter<'_, '_>>();
    let owner = size_of::<DecodedScalarFixedPointHistoryV1<'_, '_>>();
    let requested = framed.rounds().len() * size_of::<DecodedScalarFixedPointRoundV1>();
    for (required, accepted) in [
        (floor + meter, 0),
        (floor + meter + owner, 0),
        (floor + meter + owner + requested, 4),
    ] {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, required - 1);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            materialize_scalar_fixed_point_history_v1(&framed, &mut b),
            Err(Error::Resource(Resource::Storage(_)))
        ));
        assert_eq!(
            (b.work(), b.failed_storage(), b.storage()),
            (accepted, Some(required), floor)
        );
    }
}

#[test]
fn scoped_codec_resource_errors_panic_payload_and_foreign_ledger_cleanup_are_exact() {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind, panic_any},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("codec payload destructor");
        }
    }
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(SIBLING).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
            m.reserve(19)?;
            m.work(7)?;
            m.derive(|b| {
                b.reserve_storage(11)?;
                panic_any(Payload(dropped.clone()));
            })
        });
    }));
    assert!(caught.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!((budget.storage(), budget.work()), (SIBLING, 7));
    let mut foreign_work = Work::new(W);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(37).unwrap();
    foreign.charge_work(13).unwrap();
    let old = budget.work_ledger_identity_v1();
    let result: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
        m.reserve(23)?;
        m.work(5)?;
        std::mem::swap(m.budget_for_test(), &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() != old);
    assert!(foreign.work_ledger_identity_v1() == old);
    assert_eq!((budget.storage(), budget.work()), (37, 13));
    assert_eq!(
        (foreign.storage(), foreign.work()),
        (SIBLING + size_of::<Meter<'_, '_>>() + 23, 12)
    );
    foreign
        .release_storage(size_of::<Meter<'_, '_>>() + 23)
        .unwrap();
    assert_eq!(foreign.storage(), SIBLING);
}

#[test]
fn decoded_and_replayed_backing_shortfalls_never_mint_storage_credit() {
    let (input, paid, wire) = produced(&reverse_chain(1));
    let framed = frame(wire.canonical_bytes());
    let minimum = framed.storage().retained_storage() + wire.canonical_bytes().len();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        materialize_scalar_fixed_point_history_v1(&framed, &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!((b.work(), b.storage()), (0, minimum - 1));
    with_decoded(&input, paid, wire.canonical_bytes(), |d, b| {
        let checked = d.check_semantics(&input, b).unwrap();
        let floor = b.storage();
        let retained = checked.storage().retained_storage();
        b.reserve_storage(retained - 1).unwrap();
        assert!(matches!(
            checked.replay(b),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(b.storage(), floor + retained - 1);
        drop(checked);
        b.release_storage(retained - 1).unwrap();
    });
}
