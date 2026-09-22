use super::*;
use crate::{
    LoopUnrollHistoryInputsV1, encode_loop_unroll_history_v1, encode_refined_forwarding_history_v1,
    encode_scalar_fixed_point_history_v1, prepare_checked_scalar_fixed_point_v1,
    read_refined_forwarding_history_v1,
    refined_forwarding_history_wire_v1::tests::with_history_module,
    scalar_fixed_point_history_v1::tests::{self as fixture, *},
    unroll_canonical_kir_loops_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 as Limits;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;

fn loop_module(bound: Option<u32>) -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let literal = |id, n| op(id, ty.clone(), Kind::Constant(Constant::U32(n)));
    let row = |id, parameters, operations, terminator| BasicBlock {
        id: BlockId(id),
        parameters,
        operations,
        terminator: Some(terminator),
    };
    let mut entry = vec![literal(10, 1), literal(11, 0)];
    if let Some(n) = bound {
        entry.push(literal(12, n));
    }
    let mut module = Module::new("expanded-history-loop");
    module.functions.push(Function::internal_helper(
        "loop_body",
        Signature::new(
            vec![
                Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                ty.clone(),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![
            row(13, vec![], entry, branch(41, &[11])),
            row(
                41,
                vec![ValueDef::new(ValueId(20), ty.clone())],
                vec![op(
                    21,
                    Type::BOOL,
                    Kind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(20),
                        rhs: ValueId(if bound.is_some() { 12 } else { 1 }),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(21),
                    then_target: BlockId(97),
                    then_arguments: vec![ValueId(20)],
                    else_target: BlockId(701),
                    else_arguments: vec![ValueId(20)],
                },
            ),
            row(
                97,
                vec![ValueDef::new(ValueId(25), ty.clone())],
                vec![
                    Operation::new(
                        vec![],
                        Kind::Store {
                            pointer: ValueId(0),
                            value: ValueId(25),
                            access: MemoryAccess::new(AddressSpace::Global, 4),
                        },
                    ),
                    Operation::checked_binary(
                        ValueDef::new(ValueId(30), ty.clone()),
                        ValueDef::new(ValueId(31), Type::BOOL),
                        CheckedBinaryOperator::Add,
                        ValueId(20),
                        ValueId(10),
                    ),
                ],
                branch(41, &[30]),
            ),
            row(
                701,
                vec![ValueDef::new(ValueId(40), ty)],
                vec![],
                Terminator::Return { values: vec![] },
            ),
        ],
    ));
    module
}
fn produced(bound: Option<u32>) -> InertExpandedHistoryBytesV1 {
    with_history_module(&loop_module(bound), 0, |inputs, inherited| {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let floor = inherited + SIBLING;
        b.reserve_storage(floor).unwrap();
        let f_bytes = encode_refined_forwarding_history_v1(inputs, &mut b).unwrap();
        b.reserve_storage(f_bytes.storage().retained_storage())
            .unwrap();
        let f = read_refined_forwarding_history_v1(f_bytes.canonical_bytes(), &mut b).unwrap();
        b.reserve_storage(f.storage().retained_storage()).unwrap();
        let (u, u_paid) =
            unroll_canonical_kir_loops_v1(inputs.output, Limits::default(), &mut b).unwrap();
        b.reserve_storage(u_paid.retained_storage()).unwrap();
        assert_eq!(
            u.origins().selection.map(|s| s.iterations),
            bound.map(|n| n as u8)
        );
        if bound.is_some() {
            assert_ne!(
                inputs.output.canonical().canonical_bytes(),
                u.output().canonical().canonical_bytes()
            );
        }
        let u_bytes = encode_loop_unroll_history_v1(
            LoopUnrollHistoryInputsV1 {
                prefix: &f,
                output: u.output(),
                origins: u.origins(),
                limits: u.limits(),
            },
            &mut b,
        )
        .unwrap();
        b.reserve_storage(u_bytes.storage().retained_storage())
            .unwrap();
        let uf = read_loop_unroll_history_v1(u_bytes.canonical_bytes(), &mut b).unwrap();
        b.reserve_storage(uf.storage().retained_storage()).unwrap();
        let scalar = prepare_checked_scalar_fixed_point_v1(u.output(), &mut b).unwrap();
        b.reserve_storage(scalar.retained_storage()).unwrap();
        let scalar_bytes =
            encode_scalar_fixed_point_history_v1(u.output(), &scalar, &mut b).unwrap();
        b.reserve_storage(scalar_bytes.storage().retained_storage())
            .unwrap();
        let sf =
            read_scalar_fixed_point_history_v1(scalar_bytes.canonical_bytes(), &mut b).unwrap();
        b.reserve_storage(sf.storage().retained_storage()).unwrap();
        let bytes = encode_expanded_history_v1(&uf, &sf, &mut b).unwrap();
        drop(sf);
        drop(scalar_bytes);
        drop(scalar);
        drop(uf);
        drop(u_bytes);
        drop(u);
        drop(f);
        drop(f_bytes);
        b.release_storage(b.storage() - floor).unwrap();
        assert_eq!(b.storage(), floor);
        bytes
    })
}
fn raw(u: &[u8], scalar: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 48];
    bytes[..16].copy_from_slice(b"F2EPH1\0\0\x01\0\x01\0\x30\0\0\0");
    bytes[16..24].copy_from_slice(&((48 + u.len() + scalar.len()) as u64).to_le_bytes());
    bytes[24..26].copy_from_slice(&1u16.to_le_bytes());
    bytes[32..40].copy_from_slice(&(u.len() as u64).to_le_bytes());
    bytes[40..48].copy_from_slice(&(scalar.len() as u64).to_le_bytes());
    bytes.extend_from_slice(u);
    bytes.extend_from_slice(scalar);
    bytes
}
fn frame(bytes: &[u8]) -> InertExpandedHistoryRefV1<'_> {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(bytes.len() + SIBLING).unwrap();
    let frame = read_expanded_history_v1(bytes, &mut b).unwrap();
    assert_eq!(b.storage(), bytes.len() + SIBLING);
    frame
}
fn with_decoded<T>(
    bytes: &[u8],
    run: impl FnOnce(&DecodedExpandedHistoryV1<'_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(bytes.len() + SIBLING).unwrap();
    let frame = read_expanded_history_v1(bytes, &mut b).unwrap();
    b.reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let decoded = materialize_expanded_history_v1(&frame, &mut b).unwrap();
    b.reserve_storage(decoded.storage().retained_storage())
        .unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let result = run(&decoded, &mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(decoded);
    drop(frame);
    b.release_storage(floor - bytes.len() - SIBLING).unwrap();
    result
}

#[test]
fn actual_unroll_and_dynamic_cycle_roundtrip_one_closed_expanded_history() {
    for bound in [None, Some(2), Some(3)] {
        let wire = produced(bound);
        let frame = frame(wire.canonical_bytes());
        assert_eq!(
            raw(
                frame.prefix().canonical_bytes(),
                frame.scalar().canonical_bytes()
            ),
            wire.canonical_bytes()
        );
        assert_eq!(
            &wire.canonical_bytes()[frame.final_graph_range()],
            frame.output_bytes()
        );
        assert_eq!(
            frame.policy_identity(),
            b"FE2O3/EXPANDED-PRODUCTION-POLICY/V1\0"
        );
        assert!(!frame.grants_authority());
        assert!(!wire.grants_authority());
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let floor =
            wire.storage().retained_storage() + frame.storage().retained_storage() + SIBLING;
        b.reserve_storage(floor).unwrap();
        assert_eq!(
            encode_expanded_history_v1(frame.prefix(), frame.scalar(), &mut b)
                .unwrap()
                .canonical_bytes(),
            wire.canonical_bytes()
        );
        assert_eq!(b.storage(), floor);
        with_decoded(wire.canonical_bytes(), |decoded, b| {
            let checked = decoded.check_semantics(b).unwrap();
            let paid = checked.storage().retained_storage();
            b.reserve_storage(paid).unwrap();
            assert!(std::ptr::eq(
                checked.prefix().output(),
                checked.scalar().input()
            ));
            assert!(std::ptr::eq(checked.output(), decoded.output()));
            assert!(!checked.grants_authority());
            assert!(!checked.authenticates_execution());
            assert!(!decoded.grants_authority());
            assert!(!decoded.authenticates_execution());
            if bound.is_some() {
                assert_ne!(
                    checked
                        .prefix()
                        .prefix()
                        .output()
                        .canonical()
                        .canonical_bytes(),
                    checked.prefix().output().canonical().canonical_bytes()
                );
            }
            checked.replay(b).unwrap();
            drop(checked);
            b.release_storage(paid).unwrap();
        });
    }
}

#[test]
fn expanded_policy_domain_node_count_order_and_child_frames_are_closed() {
    let wire = produced(Some(2));
    let good = frame(wire.canonical_bytes());
    let mut cases = vec![raw(
        good.scalar().canonical_bytes(),
        good.prefix().canonical_bytes(),
    )];
    for at in [0, 8, 10, 12, 16, 24, 26, 32, 40, 48] {
        let mut bad = wire.canonical_bytes().to_vec();
        bad[at] ^= 0x80;
        cases.push(bad);
    }
    let mut bad = wire.canonical_bytes().to_vec();
    bad[48 + good.prefix().canonical_bytes().len()] ^= 0x80;
    cases.push(bad);
    cases.push(wire.canonical_bytes()[..47].to_vec());
    cases.push(wire.canonical_bytes()[..wire.canonical_bytes().len() - 1].to_vec());
    let mut bad = wire.canonical_bytes().to_vec();
    bad.push(0);
    cases.push(bad);
    for bad in cases {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(bad.len() + SIBLING).unwrap();
        assert!(read_expanded_history_v1(&bad, &mut b).is_err());
        assert_eq!(b.storage(), bad.len() + SIBLING);
    }
}

#[test]
fn foreign_but_individually_valid_scalar_history_cannot_join_actual_u() {
    let wire = produced(Some(2));
    let good = frame(wire.canonical_bytes());
    let (_, _, foreign) = fixture::produced(&fixture::reverse_chain(1));
    let hostile = raw(good.prefix().canonical_bytes(), foreign.canonical_bytes());
    with_decoded(&hostile, |decoded, b| {
        assert!(matches!(
            decoded.check_semantics(b),
            Err(Error::Scalar(ScalarFixedPointHistoryErrorV1::Composition))
        ))
    });
}

#[test]
fn actual_prefix_role_and_scalar_terminal_mutations_survive_framing_but_not_semantics() {
    let wire = produced(Some(2));
    let good = frame(wire.canonical_bytes());
    let mut claim = good.scalar().canonical_bytes().to_vec();
    claim[48] ^= 1;
    let hostile = raw(good.prefix().canonical_bytes(), &claim);
    with_decoded(&hostile, |d, b| {
        assert!(matches!(
            d.check_semantics(b),
            Err(Error::Scalar(ScalarFixedPointHistoryErrorV1::Composition))
        ))
    });
    let mut rows = fixture::fields(good.scalar());
    rows.push(*rows.last().unwrap());
    let scalar = fixture::raw(good.scalar().execution_claim(), &rows);
    let hostile = raw(good.prefix().canonical_bytes(), &scalar);
    with_decoded(&hostile, |d, b| {
        assert!(matches!(
            d.check_semantics(b),
            Err(Error::Scalar(
                ScalarFixedPointHistoryErrorV1::Terminal { .. }
            ))
        ))
    });
    // A full valid but different U prefix leaves both child frames canonical.
    let other = produced(Some(3));
    let other = frame(other.canonical_bytes());
    let hostile = raw(
        other.prefix().canonical_bytes(),
        good.scalar().canonical_bytes(),
    );
    with_decoded(&hostile, |d, b| {
        assert!(matches!(
            d.check_semantics(b),
            Err(Error::Scalar(ScalarFixedPointHistoryErrorV1::Composition))
        ))
    });
}

fn read_trace(frame: &InertExpandedHistoryRefV1<'_>, floor: usize) -> Trace {
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    trace.reserve(
        size_of::<InertExpandedHistoryRefV1<'_>>()
            + size_of::<[&[u8]; 2]>()
            + size_of::<[usize; 3]>(),
    );
    trace.work(48);
    trace.child(child(trace.live, |b| {
        read_loop_unroll_history_v1(frame.prefix().canonical_bytes(), b)
            .unwrap()
            .storage()
            .retained_storage()
    }));
    trace.child(fixture::read_cost(frame.scalar().rounds().len()));
    trace.work(1);
    trace
}
fn materialize_trace(frame: &InertExpandedHistoryRefV1<'_>, floor: usize) -> (Trace, usize) {
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    let mut retained = size_of::<DecodedExpandedHistoryV1<'_, '_>>();
    trace.reserve(retained);
    let u = child(trace.live, |b| {
        materialize_loop_unroll_history_v1(frame.prefix(), b)
            .unwrap()
            .storage()
            .retained_storage()
    });
    retained += u.retained;
    trace.child(u);
    let scalar = child(trace.live, |b| {
        materialize_scalar_fixed_point_history_v1(frame.scalar(), b)
            .unwrap()
            .storage()
            .retained_storage()
    });
    retained += scalar.retained;
    trace.child(scalar);
    trace.work(1);
    (trace, retained)
}
fn semantic_trace(decoded: &DecodedExpandedHistoryV1<'_, '_>, floor: usize) -> (Trace, usize) {
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    let mut retained = size_of::<ReplayedExpandedHistoryV1<'_, '_, '_>>();
    trace.reserve(retained);
    let u = child(trace.live, |b| {
        decoded
            .prefix()
            .check_semantics(b)
            .unwrap()
            .storage()
            .retained_storage()
    });
    retained += u.retained;
    trace.child(u);
    let scalar = child(trace.live, |b| {
        decoded
            .scalar()
            .check_semantics(decoded.prefix().output(), b)
            .unwrap()
            .storage()
            .retained_storage()
    });
    retained += scalar.retained;
    trace.child(scalar);
    trace.work(1);
    (trace, retained)
}

#[test]
fn expanded_read_encode_materialize_replay_have_independent_child_resource_composition() {
    let wire = produced(Some(2));
    let framed = frame(wire.canonical_bytes());
    let floor = wire.storage().retained_storage() + framed.storage().retained_storage() + SIBLING;
    let read = read_trace(&framed, floor);
    for limit in [read.work, read.work - 1] {
        let mut work = Work::new(limit);
        {
            let mut b = Budget::new(&mut work, read.peak);
            b.reserve_storage(floor).unwrap();
            match read_expanded_history_v1(wire.canonical_bytes(), &mut b) {
                Ok(_) => assert_eq!(limit, read.work),
                Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, read.work - 1),
                Err(e) => panic!("{e}"),
            }
            assert_eq!(
                (b.work(), b.peak_storage(), b.storage()),
                (limit, read.peak, floor)
            );
        }
        assert_eq!(
            work.failed_work(),
            if limit == read.work {
                None
            } else {
                Some(read.work)
            }
        );
    }
    let mut encode = Trace::new(floor, size_of::<Meter<'_, '_>>());
    encode.reserve(
        size_of::<InertExpandedHistoryBytesV1>()
            + size_of::<[&[u8]; 2]>()
            + size_of::<[usize; 3]>(),
    );
    encode.work(3);
    let capacity = encode.table::<u8>(wire.canonical_bytes().len());
    encode.work(wire.canonical_bytes().len() * 2);
    let reader = read_trace(&framed, encode.live);
    encode.child(Child {
        work: reader.work,
        peak: reader.peak - encode.live,
        retained: size_of::<InertExpandedHistoryRefV1<'_>>(),
    });
    encode.work(1);
    let mut work = Work::new(encode.work);
    let mut b = Budget::new(&mut work, encode.peak);
    b.reserve_storage(floor).unwrap();
    let value = encode_expanded_history_v1(framed.prefix(), framed.scalar(), &mut b).unwrap();
    assert_eq!(value.canonical_bytes(), wire.canonical_bytes());
    assert_eq!(
        value.storage().retained_storage(),
        size_of::<InertExpandedHistoryBytesV1>() + capacity
    );
    assert_eq!(
        (b.work(), b.peak_storage(), b.storage()),
        (encode.work, encode.peak, floor)
    );
    let (mat, retained) = materialize_trace(&framed, floor);
    for limit in [mat.work, mat.work - 1] {
        let mut work = Work::new(limit);
        {
            let mut b = Budget::new(&mut work, mat.peak);
            b.reserve_storage(floor).unwrap();
            match materialize_expanded_history_v1(&framed, &mut b) {
                Ok(d) => {
                    assert_eq!(limit, mat.work);
                    assert_eq!(d.storage().retained_storage(), retained);
                }
                Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, mat.work - 1),
                Err(e) => panic!("{e}"),
            }
            assert_eq!(
                (b.work(), b.peak_storage(), b.storage()),
                (limit, mat.peak, floor)
            );
        }
        assert_eq!(
            work.failed_work(),
            if limit == mat.work {
                None
            } else {
                Some(mat.work)
            }
        );
    }
    with_decoded(wire.canonical_bytes(), |decoded, b| {
        let floor = b.storage();
        let (sem, retained) = semantic_trace(decoded, floor);
        for limit in [sem.work, sem.work - 1] {
            let mut work = Work::new(limit);
            {
                let mut b = Budget::new(&mut work, sem.peak);
                b.reserve_storage(floor).unwrap();
                match decoded.check_semantics(&mut b) {
                    Ok(checked) => {
                        assert_eq!(limit, sem.work);
                        assert_eq!(checked.storage().retained_storage(), retained);
                    }
                    Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, sem.work - 1),
                    Err(e) => panic!("{e}"),
                }
                assert_eq!(
                    (b.work(), b.peak_storage(), b.storage()),
                    (limit, sem.peak, floor)
                );
            }
            assert_eq!(
                work.failed_work(),
                if limit == sem.work {
                    None
                } else {
                    Some(sem.work)
                }
            );
        }
    });
}

#[test]
fn expanded_initial_denial_floor_and_caps_do_not_relax_child_or_publication_limits() {
    let wire = produced(None);
    let floor = wire.canonical_bytes().len() + SIBLING;
    let meter = size_of::<Meter<'_, '_>>();
    let header = size_of::<InertExpandedHistoryRefV1<'_>>()
        + size_of::<[&[u8]; 2]>()
        + size_of::<[usize; 3]>();
    for required in [floor + meter, floor + meter + header] {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, required - 1);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            read_expanded_history_v1(wire.canonical_bytes(), &mut b),
            Err(Error::Resource(Resource::Storage(_)))
        ));
        assert_eq!(
            (b.work(), b.failed_storage(), b.storage()),
            (0, Some(required), floor)
        );
    }
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S + 1);
    assert!(matches!(
        read_expanded_history_v1(wire.canonical_bytes(), &mut b),
        Err(Error::Limit)
    ));
    assert_eq!(MAX_LOOP_UNROLL_HISTORY_BYTES_V1, 16 * 1024 * 1024);
    assert_eq!(MAX_LOOP_UNROLL_HISTORY_STORAGE_V1, 256 * 1024 * 1024);
    assert_eq!(
        MAX_EXPANDED_HISTORY_BYTES_V1,
        48 + 16 * 1024 * 1024 + MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1
    );
    assert!(MAX_EXPANDED_HISTORY_BYTES_V1 > 160 * 1024 * 1024);
}

#[test]
fn expanded_scoped_unwind_released_backing_and_result_destructor_do_not_refund_foreign_credits() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct BadDrop(std::rc::Rc<std::cell::Cell<bool>>);
    impl Drop for BadDrop {
        fn drop(&mut self) {
            self.0.set(true);
            panic!("expanded rejected result destructor");
        }
    }
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
    let caught = catch_unwind(AssertUnwindSafe(|| {
        resources::scoped(&mut b, |m: &mut Meter<'_, '_>| {
            m.reserve(19)?;
            m.work(7)?;
            m.budget_for_test().reserve_storage(11)?;
            Ok(BadDrop(dropped.clone()))
        })
    }));
    assert!(matches!(
        caught,
        Ok(Err(Error::Resource(Resource::Accounting)))
    ));
    assert!(dropped.get());
    assert_eq!((b.storage(), b.work()), (SIBLING + 11, 7));
    b.release_storage(11).unwrap();
    let result: Result<()> = resources::scoped(&mut b, |m: &mut Meter<'_, '_>| {
        m.reserve(19)?;
        m.budget_for_test().release_storage(1)?;
        m.work(1)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(b.storage(), SIBLING + size_of::<Meter<'_, '_>>() + 18);
    b.release_storage(size_of::<Meter<'_, '_>>() + 18).unwrap();
    assert_eq!(b.storage(), SIBLING);
}
