use super::*;
use crate::{
    DecodedLoopUnrollHistoryV1, LoopUnrollHistoryErrorV1 as SemanticError,
    encode_refined_forwarding_history_v1, materialize_loop_unroll_history_v1,
    refined_forwarding_history_wire_v1::tests::with_history_module, unroll_canonical_kir_loops_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollCopyV1 as CopyRole, CanonicalKirLoopUnrollOriginV1 as Origin,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, MemoryAccess, Module, Operation, OperationKind as Kind,
    ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
const WORK: usize = 1_000_000_000;
const STORAGE: usize = MAX_LOOP_UNROLL_HISTORY_STORAGE_V1;
#[path = "loop_unroll_history_resources_v1_tests.rs"]
mod resource_tests;

fn fixture(bound: Option<u32>) -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let literal = |id, n| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), ty.clone()),
            Kind::Constant(Constant::U32(n)),
        )
    };
    let block = |id, parameters, operations, terminator| BasicBlock {
        id: BlockId(id),
        parameters,
        operations,
        terminator: Some(terminator),
    };
    let jump = |id, value| Terminator::Branch {
        target: BlockId(id),
        arguments: vec![ValueId(value)],
    };
    let mut entry = vec![literal(10, 1), literal(11, 0)];
    if let Some(n) = bound {
        entry.push(literal(12, n));
    }
    let mut m = Module::new("complete-history-distinct-unroll");
    m.functions.push(Function::internal_helper(
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
            block(13, vec![], entry, jump(41, 11)),
            block(
                41,
                vec![ValueDef::new(ValueId(20), ty.clone())],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(21), Type::BOOL),
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
            block(
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
                jump(41, 30),
            ),
            block(
                701,
                vec![ValueDef::new(ValueId(40), ty)],
                vec![],
                Terminator::Return { values: vec![] },
            ),
        ],
    ));
    m
}
fn encoded(bound: Option<u32>) -> InertLoopUnrollHistoryBytesV1 {
    with_history_module(&fixture(bound), 0, |inputs, floor| {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(floor).unwrap();
        let prefix_bytes = encode_refined_forwarding_history_v1(inputs, &mut b).unwrap();
        b.reserve_storage(prefix_bytes.storage().retained_storage())
            .unwrap();
        let prefix =
            read_refined_forwarding_history_v1(prefix_bytes.canonical_bytes(), &mut b).unwrap();
        b.reserve_storage(prefix.storage().retained_storage())
            .unwrap();
        let (unrolled, storage) =
            unroll_canonical_kir_loops_v1(inputs.output, Limits::default(), &mut b).unwrap();
        b.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            unrolled.origins().selection.map(|s| s.iterations),
            bound.map(|n| n as u8)
        );
        if bound.is_some() {
            assert_ne!(
                inputs.output.canonical().canonical_bytes(),
                unrolled.output().canonical().canonical_bytes()
            );
        }
        let bytes = encode_loop_unroll_history_v1(
            LoopUnrollHistoryInputsV1 {
                prefix: &prefix,
                output: unrolled.output(),
                origins: unrolled.origins(),
                limits: unrolled.limits(),
            },
            &mut b,
        )
        .unwrap();
        b.reserve_storage(bytes.storage().retained_storage())
            .unwrap();
        drop(unrolled);
        drop(prefix);
        drop(prefix_bytes);
        b.release_storage(b.storage() - floor).unwrap();
        assert_eq!(b.storage(), floor);
        bytes
    })
}
fn read(bytes: &[u8]) -> Result<InertLoopUnrollHistoryRefV1<'_>, Error> {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(bytes.len()).unwrap();
    read_loop_unroll_history_v1(bytes, &mut b)
}
fn with_decoded<T>(
    bytes: &[u8],
    run: impl FnOnce(&DecodedLoopUnrollHistoryV1<'_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(bytes.len()).unwrap();
    let frame = read_loop_unroll_history_v1(bytes, &mut b).unwrap();
    b.reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let decoded = materialize_loop_unroll_history_v1(&frame, &mut b).unwrap();
    b.reserve_storage(decoded.storage().retained_storage())
        .unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let result = run(&decoded, &mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    drop(decoded);
    drop(frame);
    b.release_storage(floor - bytes.len()).unwrap();
    result
}
fn raw(fields: [&[u8]; 9]) -> Vec<u8> {
    let total = HEADER + fields.iter().map(|f| f.len()).sum::<usize>();
    let mut out = vec![0; HEADER];
    out[..16].copy_from_slice(b"F2LUH1\0\0\x01\0\x01\0\x68\0\0\0");
    out[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    out[24] = 9;
    for (i, field) in fields.iter().enumerate() {
        out[32 + i * 8..40 + i * 8].copy_from_slice(&(field.len() as u64).to_le_bytes());
        out.extend_from_slice(field);
    }
    out
}
fn replay(decoded: &DecodedLoopUnrollHistoryV1<'_, '_>, b: &mut Budget<'_>) -> (usize, usize) {
    let checked = decoded.check_semantics(b).unwrap();
    let retained = checked.storage().retained_storage();
    b.reserve_storage(retained).unwrap();
    assert!(std::ptr::eq(checked.output(), decoded.output()));
    assert!(std::ptr::eq(
        checked.prefix().output(),
        decoded
            .prefix()
            .graph(crate::RefinedForwardingHistoryRoleV1::F)
    ));
    assert!(!checked.grants_authority());
    assert!(!checked.authenticates_execution());
    checked.replay(b).unwrap();
    let result = (b.work(), b.peak_storage());
    drop(checked);
    b.release_storage(retained).unwrap();
    result
}
fn semantic_refusal(bytes: &[u8]) {
    with_decoded(bytes, |decoded, b| {
        assert!(matches!(
            decoded.check_semantics(b),
            Err(SemanticError::Unroll(_)) | Err(SemanticError::Prefix(_))
        ))
    });
}
fn row_round_trip<C: rows::Coordinate + std::fmt::Debug + Eq>(
    values: &[Origin<C>],
    family: u8,
) -> Vec<u8> {
    let mut bytes = vec![0; rows::extent(values).unwrap()];
    rows::encode(values, &mut bytes, family).unwrap();
    rows::validate::<C>(&bytes, family).unwrap();
    let mut c = rows::Reader {
        bytes: &bytes,
        pos: 8,
        family,
    };
    for expected in values {
        assert_eq!(&rows::row::<C>(&mut c).unwrap(), expected);
    }
    assert_eq!(c.pos, bytes.len());
    bytes
}

#[test]
fn unroll_history_literal_header_settings_and_all_coordinate_encodings() {
    let block = Block {
        function: FunctionCoordinate(7),
        block: 11,
    };
    let site = Site {
        block,
        operation: 13,
    };
    let edge = Edge {
        source: block,
        successor: 2,
    };
    let bytes = row_round_trip(
        &[Origin {
            input: block,
            output: Some(block),
            copy: CopyRole::Header(8),
        }],
        0,
    );
    assert_eq!(
        bytes,
        vec![
            1, 0, 0, 0, 0, 1, 0, 0, 1, 8, 1, 0, 7, 0, 0, 0, 11, 0, 0, 0, 7, 0, 0, 0, 11, 0, 0, 0
        ]
    );
    let definitions = [
        Definition::FunctionArgument {
            function: FunctionCoordinate(7),
            argument: 3,
        },
        Definition::BlockArgument { block, argument: 5 },
        Definition::Result {
            operation: site,
            result: 1,
        },
    ];
    for (tag, definition) in definitions.into_iter().enumerate() {
        let row = row_round_trip(
            &[Origin {
                input: definition,
                output: Some(definition),
                copy: CopyRole::Retained,
            }],
            1,
        );
        assert_eq!((row.len(), row[12]), (52, tag as u8));
        assert_eq!(&row[13..16], &[0; 3]);
        let expected = [[7u32, 3, 0, 0], [7, 11, 5, 0], [7, 11, 13, 1]][tag];
        let expected: Vec<_> = expected.into_iter().flat_map(u32::to_le_bytes).collect();
        assert_eq!(&row[16..32], expected);
        assert_eq!(&row[32..52], &row[12..32]);
    }
    assert_eq!(
        row_round_trip(
            &[Origin {
                input: site,
                output: Some(site),
                copy: CopyRole::Body(7)
            }],
            2
        )
        .len(),
        36
    );
    assert_eq!(
        row_round_trip(
            &[Origin {
                input: block,
                output: None,
                copy: CopyRole::OmittedBody
            }],
            3
        )
        .len(),
        28
    );
    assert_eq!(
        row_round_trip(
            &[Origin {
                input: edge,
                output: None,
                copy: CopyRole::Header(0)
            }],
            4
        )
        .len(),
        36
    );
    assert_eq!(
        row_round_trip(
            &[Origin {
                input: Argument { edge, argument: 4 },
                output: None,
                copy: CopyRole::Header(0)
            }],
            5
        )
        .len(),
        44
    );
    let op = row_round_trip(
        &[Origin {
            input: site,
            output: Some(site),
            copy: CopyRole::Body(7),
        }],
        2,
    );
    assert_eq!(&op[12..24], &[7, 0, 0, 0, 11, 0, 0, 0, 13, 0, 0, 0]);
    assert_eq!(&op[24..36], &op[12..24]);
    let edge_bytes = row_round_trip(
        &[Origin {
            input: edge,
            output: None,
            copy: CopyRole::Header(0),
        }],
        4,
    );
    assert_eq!(&edge_bytes[12..24], &[7, 0, 0, 0, 11, 0, 0, 0, 2, 0, 0, 0]);
    assert_eq!(&edge_bytes[24..36], &[0; 12]);
    let arg = row_round_trip(
        &[Origin {
            input: Argument { edge, argument: 4 },
            output: None,
            copy: CopyRole::Header(0),
        }],
        5,
    );
    assert_eq!(
        &arg[12..28],
        &[7, 0, 0, 0, 11, 0, 0, 0, 2, 0, 0, 0, 4, 0, 0, 0]
    );
    assert_eq!(&arg[28..44], &[0; 16]);
    let limits = Limits {
        loops: fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 {
            functions: 1,
            blocks: 2,
            edges: 3,
            definitions: 4,
            operations: 5,
            loops: 6,
            rows: 7,
        },
        max_iterations: 8,
        max_output_operand_uses: 9,
        max_output_edge_arguments: 10,
        max_origin_rows: 11,
    };
    let selection = Some(Selection {
        fact: 19,
        iterations: 0,
    });
    let mut settings = [0; 104];
    rows::write_settings(limits, selection, &mut settings).unwrap();
    for (at, n) in [
        (0, 1u64),
        (8, 2),
        (16, 3),
        (24, 4),
        (32, 5),
        (40, 6),
        (48, 7),
        (64, 9),
        (72, 10),
        (80, 11),
        (96, 19),
    ] {
        assert_eq!(&settings[at..at + 8], &n.to_le_bytes());
    }
    assert_eq!((settings[56], settings[88], settings[89]), (8, 1, 0));
    assert_eq!(rows::settings(&settings).unwrap(), (limits, selection));
}
#[test]
fn unroll_history_nonzero_full_prefix_round_trip_keeps_f_and_u_distinct() {
    let bytes = encoded(Some(3));
    with_decoded(bytes.canonical_bytes(), |decoded, b| {
        assert_eq!(decoded.origins().selection.unwrap().iterations, 3);
        assert_ne!(
            decoded
                .prefix()
                .graph(crate::RefinedForwardingHistoryRoleV1::F)
                .canonical()
                .canonical_bytes(),
            decoded.output().canonical().canonical_bytes()
        );
        assert_eq!(
            decoded
                .origins()
                .blocks
                .iter()
                .filter(|r| matches!(r.copy, CopyRole::Header(_)))
                .count(),
            4
        );
        assert_eq!(
            decoded
                .origins()
                .blocks
                .iter()
                .filter(|r| matches!(r.copy, CopyRole::Body(_)))
                .count(),
            3
        );
        replay(decoded, b);
        let second = encode_loop_unroll_history_v1(
            LoopUnrollHistoryInputsV1 {
                prefix: decoded.frame().prefix(),
                output: decoded.output(),
                origins: decoded.origins(),
                limits: decoded.limits(),
            },
            b,
        )
        .unwrap();
        assert_eq!(second.canonical_bytes(), bytes.canonical_bytes());
    });
}
#[test]
fn unroll_history_no_selection_round_trip_keeps_equal_roles_distinct() {
    let bytes = encoded(None);
    with_decoded(bytes.canonical_bytes(), |decoded, b| {
        let f = decoded
            .prefix()
            .graph(crate::RefinedForwardingHistoryRoleV1::F);
        assert_eq!(decoded.origins().selection, None);
        assert_eq!(
            f.canonical().canonical_bytes(),
            decoded.output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(f, decoded.output()));
        replay(decoded, b);
    });
}
#[test]
fn unroll_history_selected_zero_preserves_omitted_body_and_header_edges() {
    // Component selected-zero is deliberately not source-prefix eligibility.
    let module = fixture(Some(0));
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    let (input, storage) =
        Graph::from_module_ref_with_verification_budget_v12(&module, &mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let (u, storage) = unroll_canonical_kir_loops_v1(&input, Limits::default(), &mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(u.origins().selection.unwrap().iterations, 0);
    let origins = u.origins();
    assert!(
        origins
            .blocks
            .iter()
            .any(|r| r.copy == CopyRole::OmittedBody)
    );
    assert!(
        origins
            .edges
            .iter()
            .any(|r| matches!(r.copy, CopyRole::Header(0)) && r.output.is_none())
    );
    assert!(
        origins
            .arguments
            .iter()
            .any(|r| matches!(r.copy, CopyRole::Header(0)) && r.output.is_none())
    );
    row_round_trip(origins.blocks, 0);
    row_round_trip(origins.definitions, 1);
    row_round_trip(origins.operations, 2);
    row_round_trip(origins.terminators, 3);
    row_round_trip(origins.edges, 4);
    row_round_trip(origins.arguments, 5);
    let (pair, storage) = fe2o3_kernel_analysis::check_canonical_kir_loop_unroll_pair_v1(
        &input,
        u.output(),
        origins,
        u.limits(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    drop(pair);
    drop(u);
    drop(input);
    b.release_storage(b.storage()).unwrap();
}
#[test]
fn unroll_history_fixed_header_and_reserved_mutations_refuse() {
    let bytes = encoded(Some(3));
    let wire = bytes.canonical_bytes();
    for len in [0, 7, 31, 103, wire.len() - 1] {
        assert!(read(&wire[..len]).is_err());
    }
    for at in 0..32 {
        let mut bad = wire.to_vec();
        bad[at] ^= 0x80;
        assert!(read(&bad).is_err(), "header {at}");
    }
    for field in 0..9 {
        let mut bad = wire.to_vec();
        bad[32 + 8 * field..40 + 8 * field].fill(255);
        assert!(read(&bad).is_err());
    }
    let mut bad = wire.to_vec();
    bad.push(0);
    assert!(read(&bad).is_err());
    let frame = read(wire).unwrap();
    assert!(read(frame.prefix().canonical_bytes()).is_err());
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(wire.len()).unwrap();
    assert!(read_refined_forwarding_history_v1(wire, &mut b).is_err());
}
#[test]
fn unroll_history_all_six_row_family_tags_and_padding_are_closed() {
    let bytes = encoded(Some(3));
    let frame = read(bytes.canonical_bytes()).unwrap();
    for family in 0..6 {
        assert!(rows::header(frame.fields[2 + family], family as u8).unwrap() > 0);
        for (at, value) in [(4, 99), (5, 0), (6, 1), (8, 9), (10, 2), (11, 1)] {
            let mut row = frame.fields[2 + family].to_vec();
            row[at] = value;
            let mut fields = frame.fields;
            fields[2 + family] = &row;
            assert!(
                matches!(read(&raw(fields)), Err(Error::Tag { family: n }) if n == family as u8)
            );
        }
        let mut row = frame.fields[2 + family].to_vec();
        let coordinate_width = (rows::WIDTHS[family] - 4) / 2;
        row[10] = 0;
        row[12 + coordinate_width..12 + 2 * coordinate_width].fill(0);
        row[12 + coordinate_width] = 1;
        let mut fields = frame.fields;
        fields[2 + family] = &row;
        assert!(matches!(read(&raw(fields)), Err(Error::Tag { family: n }) if n == family as u8));
    }
    for (tag, index) in [(1, 9), (2, 8), (3, 1), (0, 1)] {
        let mut row = frame.fields[2].to_vec();
        row[8] = tag;
        row[9] = index;
        let mut fields = frame.fields;
        fields[2] = &row;
        assert!(matches!(read(&raw(fields)), Err(Error::Tag { family: 0 })));
    }
    let mut row = frame.fields[3].to_vec();
    row[12] = 9;
    let mut fields = frame.fields;
    fields[3] = &row;
    assert!(matches!(read(&raw(fields)), Err(Error::Tag { family: 1 })));
    for at in [13, 24] {
        let mut row = frame.fields[3].to_vec();
        assert_eq!(row[12], 0);
        row[at] = 1;
        let mut fields = frame.fields;
        fields[3] = &row;
        assert!(matches!(read(&raw(fields)), Err(Error::Tag { family: 1 })));
    }
}
#[test]
fn unroll_history_aggregate_nested_caps_precede_u_allocation() {
    let bytes = encoded(Some(3));
    let frame = read(bytes.canonical_bytes()).unwrap();
    assert!(matches!(
        aggregate(frame.prefix(), MAX_LOOP_UNROLL_HISTORY_GRAPH_BYTES_V1, 0, 0),
        Err(Error::Limit)
    ));
    assert!(matches!(
        aggregate(frame.prefix(), 0, MAX_LOOP_UNROLL_HISTORY_ROW_BYTES_V1, 0),
        Err(Error::Limit)
    ));
    assert!(matches!(
        aggregate(frame.prefix(), 0, 0, MAX_LOOP_UNROLL_HISTORY_ROWS_V1),
        Err(Error::Limit)
    ));
    assert!(matches!(
        aggregate(frame.prefix(), usize::MAX, 0, 0),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE + 1);
    assert!(matches!(
        read_loop_unroll_history_v1(bytes.canonical_bytes(), &mut b),
        Err(Error::Limit)
    ));
    assert_eq!((b.work(), b.storage(), b.peak_storage()), (0, 0, 0));
}
#[test]
fn unroll_history_all_six_missing_duplicate_or_reordered_rosters_refuse() {
    let bytes = encoded(Some(3));
    let frame = read(bytes.canonical_bytes()).unwrap();
    for family in 0..6 {
        let original = frame.fields[2 + family];
        let width = rows::WIDTHS[family];
        let count = rows::header(original, family as u8).unwrap();
        assert!(count > 1);
        for mode in 0..3 {
            let mut row = original.to_vec();
            if mode == 0 {
                row.drain(8..8 + width);
                row[..4].copy_from_slice(&((count - 1) as u32).to_le_bytes());
            } else if mode == 1 {
                row.extend_from_slice(&original[8..8 + width]);
                row[..4].copy_from_slice(&((count + 1) as u32).to_le_bytes());
            } else {
                row[8..8 + width].copy_from_slice(&original[8 + width..8 + 2 * width]);
                row[8 + width..8 + 2 * width].copy_from_slice(&original[8..8 + width]);
            }
            let mut fields = frame.fields;
            fields[2 + family] = &row;
            semantic_refusal(&raw(fields));
        }
    }
}
#[test]
fn unroll_history_selection_and_exact_limits_are_not_replaced() {
    let bytes = encoded(Some(3));
    let frame = read(bytes.canonical_bytes()).unwrap();
    for (at, value) in [(89, 2), (96, 1), (56, 2), (88, 0)] {
        let mut settings = frame.fields[8].to_vec();
        settings[at] = value;
        let mut fields = frame.fields;
        fields[8] = &settings;
        let wire = raw(fields);
        if at == 56 || at == 88 {
            assert!(matches!(read(&wire), Err(Error::Settings)));
        } else {
            semantic_refusal(&wire);
        }
    }
    for at in [64, 72, 80] {
        let mut settings = frame.fields[8].to_vec();
        settings[at..at + 8].fill(0);
        let mut fields = frame.fields;
        fields[8] = &settings;
        semantic_refusal(&raw(fields));
    }
    with_decoded(bytes.canonical_bytes(), |decoded, b| {
        let mut limits = decoded.limits();
        limits.max_origin_rows += 1;
        limits.max_output_operand_uses += 7;
        let wire = encode_loop_unroll_history_v1(
            LoopUnrollHistoryInputsV1 {
                prefix: decoded.frame().prefix(),
                output: decoded.output(),
                origins: decoded.origins(),
                limits,
            },
            b,
        )
        .unwrap();
        let retained = wire.storage().retained_storage();
        b.reserve_storage(retained).unwrap();
        let vs = {
            let view = read_loop_unroll_history_v1(wire.canonical_bytes(), b).unwrap();
            let vs = view.storage().retained_storage();
            b.reserve_storage(vs).unwrap();
            assert_eq!(view.limits(), limits);
            vs
        };
        b.release_storage(vs).unwrap();
        drop(wire);
        b.release_storage(retained).unwrap();
    });
}
#[test]
fn unroll_history_graph_and_prefix_donors_fail_independent_replay() {
    let a = encoded(Some(3));
    let b = encoded(Some(2));
    let fa = read(a.canonical_bytes()).unwrap();
    let fb = read(b.canonical_bytes()).unwrap();
    for field in [0, 1] {
        let mut fields = fa.fields;
        fields[field] = fb.fields[field];
        semantic_refusal(&raw(fields));
    }
    let mut fields = fa.fields;
    fields[1] = fa
        .prefix()
        .graph_bytes(crate::RefinedForwardingHistoryRoleV1::F);
    semantic_refusal(&raw(fields));
}
#[test]
fn unroll_history_decode_preserves_typed_v12_and_prefix_refusals() {
    let bytes = encoded(Some(3));
    let frame = read(bytes.canonical_bytes()).unwrap();
    let mut fields = frame.fields;
    fields[1] = &[0xff];
    let bad = raw(fields);
    let view = read(&bad).unwrap();
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(bad.len() + view.storage().retained_storage())
        .unwrap();
    assert!(matches!(
        materialize_loop_unroll_history_v1(&view, &mut b),
        Err(Error::Admission(_))
    ));
    let mut prefix = frame.fields[0].to_vec();
    prefix[0] ^= 1;
    let mut fields = frame.fields;
    fields[0] = &prefix;
    assert!(matches!(
        read(&raw(fields)),
        Err(Error::Prefix(PrefixError::Header))
    ));
}
#[test]
fn unroll_history_fresh_materializations_and_replay_are_deterministic() {
    let bytes = encoded(Some(3));
    let mut observations = Vec::new();
    for _ in 0..2 {
        with_decoded(bytes.canonical_bytes(), |decoded, b| {
            observations.push((
                decoded.output().canonical().canonical_bytes().to_vec(),
                decoded.origins().blocks.to_vec(),
                decoded.limits(),
                replay(decoded, b),
            ));
        });
    }
    assert_eq!(observations[0], observations[1]);
}

fn wire_source_resource_cases() -> [Resource; 5] {
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, 5);
    [
        budget.charge_work(4).unwrap_err(),
        budget.reserve_storage(6).unwrap_err(),
        budget.release_storage(1).unwrap_err(),
        // Typed leaf fixtures, not attempts to exhaust the host allocator.
        Resource::Allocation,
        Resource::Arithmetic,
    ]
}

fn assert_wire_borrowed_source<T: std::error::Error + 'static>(
    error: &dyn std::error::Error,
    child: &T,
) {
    let actual = error.source().unwrap().downcast_ref::<T>().unwrap();
    assert!(std::ptr::eq(actual, child));
}

fn assert_wire_resource_source_chain(
    error: &(dyn std::error::Error + 'static),
    expected: Resource,
    depth: usize,
) {
    let mut cause = error;
    for _ in 0..depth {
        cause = cause.source().expect("complete typed resource cause chain");
    }
    assert_eq!(cause.downcast_ref::<Resource>(), Some(&expected));
    match expected {
        Resource::Work(expected) => {
            let leaf = cause.source().unwrap();
            assert_eq!(
                leaf.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>(),
                Some(&expected)
            );
            assert!(leaf.source().is_none());
        }
        Resource::Storage(expected) => {
            let leaf = cause.source().unwrap();
            assert_eq!(
                leaf.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1>(),
                Some(&expected)
            );
            assert!(leaf.source().is_none());
        }
        Resource::Accounting | Resource::Allocation | Resource::Arithmetic => {
            assert!(cause.source().is_none());
        }
    }
}

#[test]
fn wire_error_sources_preserve_direct_prefix_and_tail_resources() {
    use fe2o3_kernel_ir::CanonicalKirTransitionReceiptErrorV1 as TailError;
    for resource in wire_source_resource_cases() {
        let direct = Error::Resource(resource);
        let Error::Resource(child) = &direct else {
            unreachable!()
        };
        assert_wire_borrowed_source(&direct, child);
        assert_wire_resource_source_chain(&direct, resource, 1);

        let prefix = Error::Prefix(PrefixError::Resource(resource));
        let Error::Prefix(child) = &prefix else {
            unreachable!()
        };
        assert_wire_borrowed_source(&prefix, child);
        assert_wire_resource_source_chain(&prefix, resource, 2);

        let nested = Error::Prefix(PrefixError::Tail(TailError::Resource(resource)));
        assert_wire_resource_source_chain(&nested, resource, 3);
    }
}

#[test]
fn wire_error_sources_preserve_admission_and_nested_policy_wrappers() {
    for resource in wire_source_resource_cases() {
        let admission = Error::Admission(Admission::Resource(resource));
        let Error::Admission(child) = &admission else {
            unreachable!()
        };
        assert_wire_borrowed_source(&admission, child);
        assert!(matches!(child, Admission::Resource(actual) if *actual == resource));

        let policy = Error::Prefix(PrefixError::Policy7(
            crate::CanonicalPolicy7SemanticErrorV1::Resource(resource),
        ));
        let Error::Prefix(prefix) = &policy else {
            unreachable!()
        };
        assert_wire_borrowed_source(&policy, prefix);
        let PrefixError::Policy7(child) = prefix else {
            unreachable!()
        };
        assert_wire_borrowed_source(prefix, child);
        assert!(matches!(
            child,
            crate::CanonicalPolicy7SemanticErrorV1::Resource(actual) if *actual == resource
        ));
    }
}

#[test]
fn wire_error_sources_distinguish_markers_from_nonresource_children() {
    use std::error::Error as _;
    for error in [
        Error::Length,
        Error::Header,
        Error::Reserved,
        Error::Limit,
        Error::Field(7),
        Error::Rows { family: 2 },
        Error::Tag { family: 4 },
        Error::Settings,
        Error::Panicked,
    ] {
        assert!(error.source().is_none());
    }
    let prefix = Error::Prefix(PrefixError::Header);
    let Error::Prefix(child) = &prefix else {
        unreachable!()
    };
    assert_wire_borrowed_source(&prefix, child);
    assert!(prefix.source().unwrap().source().is_none());
    let admission = Error::Admission(Admission::CanonicalMismatch);
    let Error::Admission(child) = &admission else {
        unreachable!()
    };
    assert_wire_borrowed_source(&admission, child);
    assert!(admission.source().unwrap().source().is_none());
}
