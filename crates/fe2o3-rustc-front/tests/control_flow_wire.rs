use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_rustc_front::*;

fn id(value: u32) -> ControlFlowNodeIdV1 {
    ControlFlowNodeIdV1::new(value)
}

fn span(line: u32) -> FrontendSourceSpanV1 {
    FrontendSourceSpanV1::new("src/kernel.rs", line, 3, line, 17).unwrap()
}

fn node(value: u32, kind: ControlFlowNodeKindV1) -> ControlFlowNodeV1 {
    ControlFlowNodeV1::new(id(value), span(value + 1), kind)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fixture_with_order(reverse: bool) -> ControlFlowContractV1 {
    let switch = ControlFlowNodeKindV1::integer_switch(
        FrontendIntegerSwitchTypeV1::new(32, false).unwrap(),
        vec![
            FrontendIntegerSwitchCaseV1::from_unsigned(1, id(5)),
            FrontendIntegerSwitchCaseV1::from_unsigned(0, id(4)),
        ],
        id(6),
    )
    .unwrap();
    let mut nodes = vec![
        node(0, ControlFlowNodeKindV1::Entry { target: id(1) }),
        node(
            1,
            ControlFlowNodeKindV1::Loop {
                max_iterations: 32,
                body: id(2),
                exit: id(7),
            },
        ),
        node(
            2,
            ControlFlowNodeKindV1::Branch {
                then_target: id(3),
                else_target: id(6),
            },
        ),
        node(3, switch),
        node(
            4,
            ControlFlowNodeKindV1::Continue {
                loop_header: id(1),
                target: id(1),
            },
        ),
        node(5, ControlFlowNodeKindV1::Block { target: id(6) }),
        node(
            6,
            ControlFlowNodeKindV1::Break {
                loop_header: id(1),
                target: id(7),
            },
        ),
        node(7, ControlFlowNodeKindV1::Exit),
    ];
    if reverse {
        nodes.reverse();
    }
    ControlFlowContractV1::new(id(0), nodes).unwrap()
}

fn fixture() -> ControlFlowContractV1 {
    fixture_with_order(false)
}

#[test]
fn canonical_round_trip_preserves_spans_and_cfg_identity() {
    let contract = fixture();
    let encoded = encode_control_flow_contract_v1(&contract).unwrap();
    assert_eq!(
        hex(&encoded),
        include_str!("fixtures/control_flow_v1.hex").trim()
    );
    assert_eq!(
        hex(contract.cfg_identity().as_bytes()),
        include_str!("fixtures/control_flow_cfg_identity_v1.hex").trim()
    );
    let reordered = fixture_with_order(true);
    assert_eq!(
        encoded,
        encode_control_flow_contract_v1(&reordered).unwrap()
    );

    let decoded = decode_control_flow_contract_v1(&encoded).unwrap();
    assert_eq!(decoded, contract);
    assert_eq!(decoded.nodes()[3].span().file(), "src/kernel.rs");
    assert_eq!(decoded.nodes()[3].span().start(), (4, 3));
    assert_eq!(decoded.nodes()[3].span().end(), (4, 17));
    assert_eq!(decoded.cfg_identity(), contract.cfg_identity());

    let mut different_spans = contract.nodes().to_vec();
    different_spans[0] = ControlFlowNodeV1::new(
        id(0),
        FrontendSourceSpanV1::new("src/remapped.rs", 50, 1, 50, 9).unwrap(),
        different_spans[0].kind().clone(),
    );
    let different_spans = ControlFlowContractV1::new(id(0), different_spans).unwrap();
    assert_ne!(
        encode_control_flow_contract_v1(&different_spans).unwrap(),
        encoded
    );
    assert_eq!(different_spans.cfg_identity(), contract.cfg_identity());
}

#[test]
fn every_truncated_prefix_fails_without_panicking() {
    let encoded = encode_control_flow_contract_v1(&fixture()).unwrap();
    for end in 0..encoded.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_control_flow_contract_v1(&encoded[..end])
        }))
        .expect("decoder must be total");
        assert!(result.is_err(), "prefix ending at {end} decoded");
    }
}

#[test]
fn single_bit_mutations_are_rejected_or_canonical_alternatives() {
    let encoded = encode_control_flow_contract_v1(&fixture()).unwrap();
    for offset in 0..encoded.len() {
        let mut mutated = encoded.clone();
        mutated[offset] ^= 1;
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_control_flow_contract_v1(&mutated)
        }))
        .expect("decoder must be total");
        if let Ok(decoded) = result {
            assert_eq!(encode_control_flow_contract_v1(&decoded).unwrap(), mutated);
        }
    }
}

#[test]
fn malformed_headers_and_noncanonical_wire_fail_closed() {
    let encoded = encode_control_flow_contract_v1(&fixture()).unwrap();

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(
        decode_control_flow_contract_v1(&invalid),
        Err(ControlFlowDecodeErrorV1::InvalidMagic)
    );
    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_control_flow_contract_v1(&invalid),
        Err(ControlFlowDecodeErrorV1::UnknownVersion(2))
    );
    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_control_flow_contract_v1(&invalid),
        Err(ControlFlowDecodeErrorV1::UnsupportedFlags(1))
    );
    let mut invalid = encoded.clone();
    invalid[24] = 1;
    assert_eq!(
        decode_control_flow_contract_v1(&invalid),
        Err(ControlFlowDecodeErrorV1::NonzeroReserved(
            "control-flow header"
        ))
    );
    let mut invalid = encoded.clone();
    invalid.push(0);
    assert_eq!(
        decode_control_flow_contract_v1(&invalid),
        Err(ControlFlowDecodeErrorV1::TrailingBytes)
    );
}

#[test]
fn loops_and_structured_transfers_fail_closed() {
    let mut nodes = fixture().nodes().to_vec();
    nodes[1] = node(
        1,
        ControlFlowNodeKindV1::Loop {
            max_iterations: 0,
            body: id(2),
            exit: id(7),
        },
    );
    assert_eq!(
        ControlFlowContractV1::new(id(0), nodes),
        Err(ControlFlowValidationErrorV1::ZeroLoopBound(1))
    );

    let mut nodes = fixture().nodes().to_vec();
    nodes[4] = node(
        4,
        ControlFlowNodeKindV1::Continue {
            loop_header: id(1),
            target: id(2),
        },
    );
    assert_eq!(
        ControlFlowContractV1::new(id(0), nodes),
        Err(ControlFlowValidationErrorV1::ContinueTargetMismatch { node: 4 })
    );

    let mut nodes = fixture().nodes().to_vec();
    nodes[6] = node(
        6,
        ControlFlowNodeKindV1::Break {
            loop_header: id(1),
            target: id(2),
        },
    );
    assert_eq!(
        ControlFlowContractV1::new(id(0), nodes),
        Err(ControlFlowValidationErrorV1::BreakTargetMismatch { node: 6 })
    );
}

#[test]
fn integer_switches_require_fixed_types_and_unique_in_range_cases() {
    assert_eq!(
        FrontendIntegerSwitchTypeV1::new(24, false),
        Err(ControlFlowValidationErrorV1::UnsupportedIntegerWidth(24))
    );

    let mut nodes = fixture().nodes().to_vec();
    nodes[3] = node(
        3,
        ControlFlowNodeKindV1::integer_switch(
            FrontendIntegerSwitchTypeV1::new(8, true).unwrap(),
            vec![FrontendIntegerSwitchCaseV1::from_signed(128, id(4))],
            id(6),
        )
        .unwrap(),
    );
    assert_eq!(
        ControlFlowContractV1::new(id(0), nodes),
        Err(ControlFlowValidationErrorV1::IntegerCaseOutOfRange { node: 3, bits: 128 })
    );

    let mut nodes = fixture().nodes().to_vec();
    nodes[3] = node(
        3,
        ControlFlowNodeKindV1::integer_switch(
            FrontendIntegerSwitchTypeV1::new(32, false).unwrap(),
            vec![
                FrontendIntegerSwitchCaseV1::from_unsigned(7, id(4)),
                FrontendIntegerSwitchCaseV1::from_unsigned(7, id(5)),
            ],
            id(6),
        )
        .unwrap(),
    );
    assert_eq!(
        ControlFlowContractV1::new(id(0), nodes),
        Err(ControlFlowValidationErrorV1::DuplicateIntegerCase { node: 3, bits: 7 })
    );

    let u128_switch = ControlFlowNodeKindV1::integer_switch(
        FrontendIntegerSwitchTypeV1::new(128, false).unwrap(),
        vec![
            FrontendIntegerSwitchCaseV1::from_unsigned(u128::MAX, id(4)),
            FrontendIntegerSwitchCaseV1::from_unsigned(u128::MAX - 1, id(5)),
        ],
        id(6),
    )
    .unwrap();
    let mut nodes = fixture().nodes().to_vec();
    nodes[3] = node(3, u128_switch);
    assert!(ControlFlowContractV1::new(id(0), nodes).is_ok());
}

#[test]
fn unbounded_and_irreducible_cycles_are_rejected() {
    let unbounded = vec![
        node(0, ControlFlowNodeKindV1::Entry { target: id(1) }),
        node(
            1,
            ControlFlowNodeKindV1::Branch {
                then_target: id(1),
                else_target: id(2),
            },
        ),
        node(2, ControlFlowNodeKindV1::Exit),
    ];
    assert_eq!(
        ControlFlowContractV1::new(id(0), unbounded),
        Err(ControlFlowValidationErrorV1::IrreducibleControlFlow)
    );

    let irreducible = vec![
        node(0, ControlFlowNodeKindV1::Entry { target: id(1) }),
        node(
            1,
            ControlFlowNodeKindV1::Branch {
                then_target: id(2),
                else_target: id(3),
            },
        ),
        node(2, ControlFlowNodeKindV1::Block { target: id(3) }),
        node(
            3,
            ControlFlowNodeKindV1::Branch {
                then_target: id(2),
                else_target: id(4),
            },
        ),
        node(4, ControlFlowNodeKindV1::Exit),
    ];
    assert_eq!(
        ControlFlowContractV1::new(id(0), irreducible),
        Err(ControlFlowValidationErrorV1::IrreducibleControlFlow)
    );
}

#[test]
fn registration_and_wire_domains_are_distinct_and_frozen() {
    assert_ne!(CONTROL_FLOW_CONTRACT_MAGIC_V1, FRONTEND_UNIT_MAGIC_V1);
    assert_ne!(
        CONTROL_FLOW_CONTRACT_MAGIC_V1,
        FRONTEND_KERNEL_CONTRACT_MAGIC_V1
    );
    assert_eq!(CONTROL_FLOW_CONTRACT_VERSION_V1, 1);
    assert_eq!(
        CONTROL_FLOW_REGISTRATION_MAGIC_V1.to_le_bytes(),
        *b"FE2O3CFA"
    );
}
