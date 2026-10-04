use super::*;

// Fixed offsets independently follow the frozen grammar for fixture(), not
// encoder instrumentation. The 702-byte assertion detects accidental drift.
const SIG: usize = 498;
const IR: usize = 552;
const BLOCKS: usize = 579;
const ASSIGNMENTS: usize = 587;
const VALUE: usize = 604;
const LOOPS: usize = 625;
const EFFECTS: usize = 629;
const RHS: usize = 663;

fn wire() -> Vec<u8> {
    let bytes = encode(fixture().input()).unwrap().0;
    assert_eq!(bytes.len(), 702);
    bytes
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn unknown_tags_header_bool_bits_and_utf8_reject() {
    for offset in [
        0,
        8,
        10,
        16,
        SIG,
        SIG + 8,
        SIG + 9,
        SIG + 10,
        SIG + 17,
        SIG + 18,
        SIG + 19,
        SIG + 20,
        SIG + 21,
        IR + 12,
        603,
        VALUE,
        VALUE + 1,
        VALUE + 2,
        VALUE + 3,
        624,
        645,
        650,
        RHS,
        RHS + 1,
        RHS + 2,
    ] {
        let mut bytes = wire();
        bytes[offset] = 255;
        assert!(decode(&bytes).is_err(), "tag/header offset {offset}");
    }
    let mut bytes = wire();
    bytes[57] = 255;
    assert_eq!(decode(&bytes), Err(Error::Wire("UTF-8")));
    for offset in [VALUE + 4 + 4, RHS + 3 + 4, 686 + 4] {
        let mut bytes = wire();
        bytes[offset] = 1;
        assert_eq!(decode(&bytes), Err(Error::Wire("noncanonical scalar bits")));
    }
    let mut bytes = wire();
    bytes[16] = 32;
    assert_eq!(decode(&bytes), Err(Error::Wire("pointer width")));
}

#[test]
fn count_remaining_bytes_limits_and_loop_tag_reject_before_exposure() {
    for offset in [
        53,
        SIG + 4,
        SIG + 11,
        IR + 8,
        BLOCKS,
        ASSIGNMENTS,
        599,
        EFFECTS,
        646,
        655,
        659,
    ] {
        let mut bytes = wire();
        put_u32(&mut bytes, offset, u32::MAX);
        assert!(decode(&bytes).is_err(), "count offset {offset}");
    }
    // Admissible hard count but impossible remaining minimum bytes.
    let mut bytes = wire();
    put_u32(&mut bytes, BLOCKS, MAX_REFERENCE_BLOCKS_V1 as u32);
    assert_eq!(decode(&bytes), Err(Error::Wire("count or remaining bytes")));
    let mut bytes = wire();
    put_u32(&mut bytes, LOOPS, 1);
    assert_eq!(
        decode(&bytes),
        Err(Error::Wire("loop summaries unsupported"))
    );
    let bytes = vec![0; MAX_NATIVE_CPU_INPUT_BYTES_V1 + 1];
    assert_eq!(decode(&bytes), Err(Error::Wire("frame limit")));
}

#[test]
fn invalid_ordinals_locals_relations_and_occurrences_reject() {
    for (offset, value) in [
        (49, HARD_MAX_FUNCTIONS_V1 as u32),
        (IR, 3),
        (IR + 4, 2),
        (IR + 4, HARD_MAX_LOCALS_V1 as u32 + 1),
        (565, 1),
        (569, 1),
        (574, 1),
        (583, 1),
        (591, 65536),
        (595, 3),
        (633, 1),
        (637, 1),
        (641, 1),
        (651, 1),
    ] {
        let mut bytes = wire();
        put_u32(&mut bytes, offset, value);
        assert!(decode(&bytes).is_err(), "ordinal {offset}={value}");
    }
}

#[test]
fn invalid_boolean_and_constant_index_reject() {
    let mut f = fixture();
    f.ir.observable_output_effects[0].coordinate = ReferenceOutputCoordinateV1::Constant {
        offset: 0,
        minimum_length: 1,
        from_end: false,
    };
    f.refresh();
    let mut bytes = encode(f.input()).unwrap().0;
    assert_eq!(bytes[645], 3);
    bytes[662] = 2;
    assert_eq!(decode(&bytes), Err(Error::Wire("boolean")));
    f.ir.observable_output_effects[0].coordinate = ReferenceOutputCoordinateV1::Constant {
        offset: 0,
        minimum_length: 1,
        from_end: true,
    };
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("constant index")
    );
}

#[test]
fn edges_cycles_including_unreachable_and_duplicate_switch_values_reject() {
    for target in [0, 1] {
        let mut f = fixture();
        f.ir.blocks[0].terminator = ReferenceTerminatorV1::Goto { target };
        f.refresh();
        assert!(encode(f.input()).is_err());
    }
    let mut f = fixture();
    let mut blocks = f.ir.blocks.to_vec();
    blocks.push(ReferenceBlockV1 {
        block: 1,
        assignments: Box::default(),
        terminator: ReferenceTerminatorV1::Goto { target: 1 },
    });
    f.ir.blocks = blocks.into_boxed_slice();
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("cyclic CPU body")
    );
    f.ir.blocks[1].terminator = ReferenceTerminatorV1::Return;
    f.ir.blocks[0].terminator = ReferenceTerminatorV1::Switch {
        discriminant: ReferenceOperandV1::Constant(ReferenceConstantV1::ZeroSized),
        values: vec![(1, 1), (1, 1)].into_boxed_slice(),
        otherwise: 1,
    };
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("duplicate switch value")
    );
    if let ReferenceTerminatorV1::Switch { values, .. } = &mut f.ir.blocks[0].terminator {
        values[0].0 = 2;
    }
    f.refresh();
    let bytes = encode(f.input()).unwrap().0;
    decode(&bytes).unwrap(); // Source order 2,1 is deliberately not sorted.
}

fn unary(depth: usize) -> ReferenceEffectExpressionV1 {
    let mut expression = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized);
    for _ in 1..depth {
        expression = ReferenceEffectExpressionV1::Unary {
            operation: ReferenceUnaryOpV1::Not,
            operand: Box::new(expression),
        };
    }
    expression
}

#[test]
fn recursive_depth_is_bounded_in_both_live_lists_before_digest() {
    let mut f = fixture();
    f.ir.observable_output_effects[0].rhs = unary(DEPTH);
    f.refresh();
    decode(&encode(f.input()).unwrap().0).unwrap();
    // Do not hash hostile retained data in the test setup either.
    f.writes[0].rhs = unary(DEPTH + 1);
    f.digest[0] ^= 1;
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("expression depth/node limit")
    );
    f.ir.observable_output_effects[0].rhs = unary(DEPTH + 1);
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("expression depth/node limit")
    );
}

#[test]
fn recursive_wire_depth_rejected_while_parsing_not_after_allocation() {
    let bytes = wire();
    let mut hostile = bytes[..RHS].to_vec();
    for _ in 0..DEPTH {
        hostile.extend_from_slice(&[4, 0]);
    }
    hostile.extend_from_slice(&bytes[RHS..]);
    let length = hostile.len() as u32;
    put_u32(&mut hostile, 12, length);
    assert_eq!(
        decode(&hostile),
        Err(Error::Wire("expression depth/node limit"))
    );
}

#[test]
fn noncanonical_predicates_and_unsupported_helpers_reject() {
    let mut f = fixture();
    let atom = ReferenceGuardAtomV1::Assert {
        condition: ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Bool,
            bits: 1,
        }),
        expected: true,
    };
    f.ir.observable_output_effects[0].guard.clauses[0].atoms =
        vec![atom.clone(), atom].into_boxed_slice();
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("guard atom order")
    );
    let mut f = fixture();
    f.ir.blocks[0].assignments[0].value = ReferenceValueV1::SafeHelperCall {
        helper: identity(7),
        parameters: Box::default(),
        result: ReferenceScalarTypeV1::F32,
        arguments: Box::default(),
        summary: Box::new(unary(1)),
    };
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("helper calls unsupported")
    );
    let mut bytes = wire();
    bytes[VALUE] = 5;
    assert_eq!(
        decode(&bytes),
        Err(Error::Wire("value tag (helpers unsupported)"))
    );
}

#[test]
fn live_loops_projection_indices_and_bad_slice_expression_reject() {
    let mut f = fixture();
    f.ir.loop_summaries = vec![ReferenceLoopSummaryV2 {
        header: 0,
        latch: 0,
        exit: 0,
        exact_iterations: Some(0),
        maximum_iterations: 0,
        carried_locals: Box::default(),
        initial_state_sha256: [0; 32],
        transition_sha256: [0; 32],
        variant_sha256: [0; 32],
    }]
    .into_boxed_slice();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("loop summaries unsupported")
    );
    for projection in [
        ReferencePlaceProjectionV1::Field(2),
        ReferencePlaceProjectionV1::Index(3),
    ] {
        let mut f = fixture();
        f.ir.blocks[0].assignments[0].destination.projection = vec![projection].into_boxed_slice();
        f.refresh();
        assert!(encode(f.input()).is_err());
    }
    for expr in [
        ReferenceEffectExpressionV1::InputLength {
            reference_argument: 0,
        },
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 1,
            index: Box::new(unary(1)),
        },
        ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 },
    ] {
        let mut f = fixture();
        f.ir.observable_output_effects[0].rhs = expr;
        f.refresh();
        assert!(encode(f.input()).is_err());
    }
}

fn tree(level: usize) -> ReferenceEffectExpressionV1 {
    if level == 0 {
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized)
    } else {
        ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            checked: false,
            lhs: Box::new(tree(level - 1)),
            rhs: Box::new(tree(level - 1)),
        }
    }
}
fn wrap(expr: ReferenceEffectExpressionV1) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Unary {
        operation: ReferenceUnaryOpV1::Not,
        operand: Box::new(expr),
    }
}

#[test]
fn per_expression_node_limit_exact_and_one_over_live_and_wire() {
    let mut f = fixture();
    f.ir.observable_output_effects[0].rhs = wrap(tree(12)); // 8,192 nodes.
    f.refresh();
    let bytes = encode(f.input()).unwrap().0;
    decode(&bytes).unwrap();
    f.writes[0].rhs = wrap(f.writes[0].rhs.clone());
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("expression depth/node limit")
    );
    let mut hostile = bytes[..RHS].to_vec();
    hostile.extend_from_slice(&[4, 0]);
    hostile.extend_from_slice(&bytes[RHS..]);
    let length = hostile.len() as u32;
    put_u32(&mut hostile, 12, length);
    assert_eq!(
        decode(&hostile),
        Err(Error::Wire("expression depth/node limit"))
    );
}

#[test]
fn aggregate_node_limit_exact_and_one_over_live_and_wire() {
    let mut f = fixture();
    f.ir.observable_output_effects[0].rhs = tree(12); // 8,191 plus one point coordinate.
    let mut effects = Vec::new();
    let mut assignments = Vec::new();
    for statement in 0..32 {
        let mut effect = f.ir.observable_output_effects[0].clone();
        effect.statement = statement;
        let mut assignment = f.ir.blocks[0].assignments[0].clone();
        assignment.statement = statement;
        effects.push(effect);
        assignments.push(assignment);
    }
    f.ir.blocks[0].assignments = assignments.into_boxed_slice();
    f.ir.observable_output_effects = effects.into_boxed_slice();
    f.refresh();
    let bytes = encode(f.input()).unwrap().0;
    decode(&bytes).unwrap();
    f.ir.observable_output_effects[0].rhs = wrap(f.ir.observable_output_effects[0].rhs.clone());
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("expression depth/node limit")
    );
    let first_rhs = RHS + 31 * 33; // Each extra fixed-fixture assignment is 33 bytes.
    assert_eq!(bytes[first_rhs], 3);
    let mut hostile = bytes[..first_rhs].to_vec();
    hostile.extend_from_slice(&[4, 0]);
    hostile.extend_from_slice(&bytes[first_rhs..]);
    let length = hostile.len() as u32;
    put_u32(&mut hostile, 12, length);
    assert_eq!(
        decode(&hostile),
        Err(Error::Wire("expression depth/node limit"))
    );
}

#[test]
fn frame_cap_exact_and_one_over() {
    let f = fixture();
    let fixed = wire().len() - f.input().association.registration_path.len();
    let name = "x".repeat(MAX_NATIVE_CPU_INPUT_BYTES_V1 - fixed);
    let mut input = f.input();
    input.association.registration_path = &name;
    let bytes = encode(input).unwrap().0;
    assert_eq!(bytes.len(), MAX_NATIVE_CPU_INPUT_BYTES_V1);
    decode(&bytes).unwrap();
    let too_long = format!("{name}x");
    let mut input = f.input();
    input.association.registration_path = &too_long;
    assert_eq!(encode(input).unwrap_err(), Error::Wire("frame limit"));
}

#[test]
fn recursive_guards_in_second_list_are_checked_before_order_or_hash() {
    let mut f = fixture();
    f.writes[0].guard.clauses[0].atoms = vec![ReferenceGuardAtomV1::Assert {
        condition: unary(DEPTH + 1),
        expected: true,
    }]
    .into_boxed_slice();
    f.digest[0] ^= 1;
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("expression depth/node limit")
    );
}

#[test]
fn duplicate_effects_statement_order_and_relation_type_reject() {
    let mut f = fixture();
    f.ir.observable_output_effects =
        vec![f.writes[0].clone(), f.writes[0].clone()].into_boxed_slice();
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("effect occurrence order")
    );
    let mut f = fixture();
    f.ir.blocks[0].assignments = vec![
        f.ir.blocks[0].assignments[0].clone(),
        f.ir.blocks[0].assignments[0].clone(),
    ]
    .into_boxed_slice();
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("statement ordinal")
    );
    let mut f = fixture();
    f.ir.relations[1] = ReferenceArgumentRelationV1::ScalarInput {
        argument: 0,
        scalar: ReferenceScalarTypeV1::F32,
    };
    f.refresh();
    assert_eq!(
        encode(f.input()).unwrap_err(),
        Error::Wire("effect output relation")
    );
}
