//! Inert grammar/codec controls, not authenticated source-produced evidence.

use super::*;

const PROFILE: SemanticGfx942OrderedRegionProfileV31 =
    SemanticGfx942OrderedRegionProfileV31::XorAddU32E32;
const REGISTERS: [u8; 5] = [32, 33, 34, 35, 36];

fn source(function: SemanticFunctionIdentityV1) -> SemanticOrderedRegionSourceV31 {
    SemanticOrderedRegionSourceV31::new([0xb1; 32], function, [0xb3; 32], [0xb4; 32]).unwrap()
}

fn old_source(function: SemanticFunctionIdentityV1) -> SemanticInlineAssemblySourceV30 {
    SemanticInlineAssemblySourceV30::new([0xa1; 32], function, [0xa3; 32], [0xa4; 32]).unwrap()
}

fn literal(value: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(u128::from(value), 1).unwrap()),
    ))
}

fn request(registers: [u8; 5]) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let mut types = request.types.into_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(40)),
        SemanticLayoutIdentityV1(identity(41)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, u128::from(u8::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8,
        }),
    ));
    request.types = types.into_boxed_slice();
    let abi = &request.functions[0].abi;
    let mut arguments = vec![abi.arguments()[0].value().clone(); 3];
    arguments.extend(vec![
        SemanticAbiValueV1::new(
            SemanticTypeIdV1(1),
            abi.return_value().mode().clone()
        );
        5
    ]);
    let intrinsic_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(20)),
        SemanticLayoutIdentityV1(identity(21)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        abi.return_value().clone(),
    )
    .unwrap();
    let mut callables = request.callables.into_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(identity(22)),
            SemanticItemDefinitionIdentityV1(identity(23)),
            SemanticMonomorphizationIdentityV1(identity(24)),
            SemanticGenericTypeArgumentsIdentityV1(identity(25)),
            SemanticConstGenericArgumentsIdentityV1(identity(26)),
            SemanticSourceProvenanceV1::unavailable(),
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(PROFILE),
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(0xa7)),
    });
    request.callables = callables.into_boxed_slice();
    let mut arguments = vec![
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1(1), vec![], SemanticTypeIdV1(0)).unwrap()
        );
        3
    ];
    arguments.extend(registers.into_iter().map(literal));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        arguments,
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], SemanticTypeIdV1(0)).unwrap(),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_ordered_region_source_v31(source(request.functions[0].identity));
    request.functions[0].blocks = vec![
        block(30, SemanticTerminatorKindV1::Call(call)),
        block(31, SemanticTerminatorKindV1::Return),
    ]
    .into_boxed_slice();
    request
}

fn block(identity_tag: u8, kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1(identity(identity_tag)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn call_mut(request: &mut InertSemanticMirRequestV1) -> &mut SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call
}

fn binding_mut(request: &mut InertSemanticMirRequestV1) -> &mut SemanticNonBodyCallableBindingV1 {
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    binding
}

fn unique_offset(bytes: &[u8], needle: &[u8]) -> usize {
    let offsets: Vec<_> = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, value)| (value == needle).then_some(index))
        .collect();
    assert_eq!(offsets.len(), 1);
    offsets[0]
}

fn admitted(request: InertSemanticMirRequestV1) -> AdmittedInertSemanticMirV1 {
    request
        .admit_exact_v31(SemanticMirLimitsV1::default())
        .unwrap()
}

fn assert_invalid(request: InertSemanticMirRequestV1) {
    assert_eq!(
        request
            .admit_exact_v31(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidOrderedRegionV31
    );
}

#[test]
fn v31_round_trip_preserves_literal_bindings_and_borrowed_exact_call() {
    for registers in [REGISTERS, [0, 63, 1, 62, 2], [63, 0, 62, 1, 61]] {
        let original = request(registers);
        let admitted = admitted(original.clone());
        assert_eq!(
            minimum_wire_version(&original),
            SemanticMirWireVersionV1::V31
        );
        let bytes = admitted.canonical_encoding();
        let offset = unique_offset(bytes, &[0xa7; 32]) - 2;
        assert_eq!(&bytes[offset..offset + 2], &[88, 0]);
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.request, original);
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        let checked = decoded
            .checked_gfx942_ordered_region_call_v31(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
            .unwrap();
        assert_eq!(checked.profile(), PROFILE);
        assert_eq!(checked.source(), source(original.functions[0].identity));
        assert_eq!(checked.registers().scratch(), registers[0]);
        assert_eq!(checked.registers().output(), registers[1]);
        assert_eq!(checked.registers().inputs(), registers[2..]);
        let SemanticTerminatorKindV1::Call(call) =
            original.functions[0].blocks[0].terminator.kind()
        else {
            unreachable!()
        };
        assert_eq!(checked.inputs(), &call.arguments[..3]);
        for (function, block) in [(0, 1), (0, 999), (999, 0)] {
            assert!(
                decoded
                    .checked_gfx942_ordered_region_call_v31(
                        SemanticFunctionIdV1(function),
                        SemanticBlockIdV1(block),
                    )
                    .is_err()
            );
        }
        for current in [
            original
                .clone()
                .admit(SemanticMirLimitsV1::default())
                .unwrap(),
            original
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap(),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bytes,
                SemanticMirLimitsV1::default(),
            )
            .unwrap(),
        ] {
            assert_eq!(current.canonical_encoding(), bytes);
        }
    }
    // Different per-call bindings do not change the shared intrinsic identity.
    let first = admitted(request(REGISTERS));
    let second = admitted(request([0, 1, 2, 3, 4]));
    assert_eq!(first.callables(), second.callables());
    assert_ne!(first.semantic_sha256(), second.semantic_sha256());
}

#[test]
fn source_options_are_exclusive_required_and_bound_to_actual_caller() {
    let mut missing = request(REGISTERS);
    call_mut(&mut missing).ordered_region_source_v31 = None;
    assert_invalid(missing);
    let mut wrong = request(REGISTERS);
    call_mut(&mut wrong).ordered_region_source_v31 =
        Some(source(SemanticFunctionIdentityV1(identity(99))));
    assert_invalid(wrong);
    let mut both = request(REGISTERS);
    let old = old_source(both.functions[0].identity);
    call_mut(&mut both).inline_assembly_source_v30 = Some(old);
    assert_invalid(both);
    let mut only_old = request(REGISTERS);
    let old = old_source(only_old.functions[0].identity);
    call_mut(&mut only_old).ordered_region_source_v31 = None;
    call_mut(&mut only_old).inline_assembly_source_v30 = Some(old);
    assert_invalid(only_old);
    let mut ordinary = request(REGISTERS);
    call_mut(&mut ordinary).callee = SemanticCallableIdV1(0);
    assert_invalid(ordinary);
    // Source refs are inert. A nonzero non-caller reference changes bytes, not authority.
    let mut changed = request(REGISTERS);
    call_mut(&mut changed).ordered_region_source_v31 = Some(
        SemanticOrderedRegionSourceV31::new(
            [0xc1; 32],
            changed.functions[0].identity,
            [0xb3; 32],
            [0xb4; 32],
        )
        .unwrap(),
    );
    assert_ne!(
        admitted(changed).semantic_sha256(),
        admitted(request(REGISTERS)).semantic_sha256()
    );
}

#[test]
fn mixed_v30_and_v31_calls_keep_independent_source_options_in_one_document() {
    let mut fixture = request(REGISTERS);
    let function = fixture.functions[0].identity;
    let root_abi = &fixture.functions[0].abi;
    let old_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(60)),
        SemanticLayoutIdentityV1(identity(61)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![root_abi.arguments()[0].value().clone()],
        root_abi.return_value().clone(),
    )
    .unwrap();
    let mut callables = fixture.callables.to_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(identity(62)),
            SemanticItemDefinitionIdentityV1(identity(63)),
            SemanticMonomorphizationIdentityV1(identity(64)),
            SemanticGenericTypeArgumentsIdentityV1(identity(65)),
            SemanticConstGenericArgumentsIdentityV1(identity(66)),
            SemanticSourceProvenanceV1::unavailable(),
            old_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VMovB32, 1)
                .unwrap(),
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(0xa8)),
    });
    fixture.callables = callables.into_boxed_slice();
    let old_call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(2),
        vec![SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], SemanticTypeIdV1(0)).unwrap(),
        )],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], SemanticTypeIdV1(0)).unwrap(),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(2)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_inline_assembly_source_v30(old_source(function));
    let mut blocks = fixture.functions[0].blocks.to_vec();
    blocks[1] = block(31, SemanticTerminatorKindV1::Call(old_call));
    blocks.push(block(32, SemanticTerminatorKindV1::Return));
    fixture.functions[0].blocks = blocks.into_boxed_slice();
    let admitted = admitted(fixture.clone());
    let bytes = admitted.canonical_encoding();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
        bytes,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.request, fixture);
    assert_eq!(
        minimum_wire_version(&fixture),
        SemanticMirWireVersionV1::V31
    );
    assert!(
        decoded
            .checked_gfx942_ordered_region_call_v31(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
            .is_ok()
    );
    assert!(
        decoded
            .checked_gfx942_ordered_region_call_v31(SemanticFunctionIdV1(0), SemanticBlockIdV1(1))
            .is_err()
    );
    let new_source = unique_offset(bytes, &[0xb1; 32]);
    let old_source = unique_offset(bytes, &[0xa1; 32]);
    assert_eq!(&bytes[new_source - 2..new_source], &[0, 1]);
    assert_eq!(bytes[old_source - 1], 1);
    assert_eq!(bytes[old_source + 128], 0);
    let SemanticTerminatorKindV1::Call(old_call) =
        &mut fixture.functions[0].blocks[1].terminator.kind
    else {
        unreachable!()
    };
    old_call.ordered_region_source_v31 = Some(source(function));
    assert_invalid(fixture.clone());
    let SemanticTerminatorKindV1::Call(old_call) =
        &mut fixture.functions[0].blocks[1].terminator.kind
    else {
        unreachable!()
    };
    old_call.inline_assembly_source_v30 = None;
    assert_invalid(fixture);
}

#[test]
fn call_bindings_require_five_literal_u8_values_and_every_pair_disjoint() {
    for left in 0..5 {
        for right in left + 1..5 {
            let mut registers = REGISTERS;
            registers[right] = registers[left];
            assert_invalid(request(registers));
        }
        for value in 64..=255 {
            let mut registers = REGISTERS;
            registers[left] = value;
            assert_invalid(request(registers));
        }
        for operand in [
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(2), vec![], SemanticTypeIdV1(1)).unwrap(),
            ),
            SemanticOperandV1::Move(
                SemanticPlaceV1::new(SemanticLocalIdV1(2), vec![], SemanticTypeIdV1(1)).unwrap(),
            ),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(1),
                SemanticConstantValueV1::ZeroSized,
            )),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(1),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(32, 2).unwrap()),
            )),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1(0),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(32, 4).unwrap()),
            )),
        ] {
            let mut fixture = request(REGISTERS);
            let mut locals = fixture.functions[0].locals.to_vec();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(50)),
                SemanticTypeIdV1(1),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ));
            fixture.functions[0].locals = locals.into_boxed_slice();
            call_mut(&mut fixture).arguments[left + 3] = operand;
            assert_invalid(fixture);
        }
    }
    for count in [0, 3, 7, 9] {
        let mut fixture = request(REGISTERS);
        call_mut(&mut fixture).arguments = vec![literal(32); count].into_boxed_slice();
        assert_invalid(fixture);
    }
    for unwind in [
        SemanticUnwindActionV1::Terminate,
        SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallUnwind,
            SemanticBlockIdV1(1),
        )),
    ] {
        let mut fixture = request(REGISTERS);
        call_mut(&mut fixture).unwind = unwind;
        assert_invalid(fixture);
    }
    let mut continuing = request(REGISTERS);
    call_mut(&mut continuing).unwind = SemanticUnwindActionV1::Continue;
    admitted(continuing);
}

#[test]
fn exact_signature_is_checked_even_for_an_unused_region_callable() {
    for case in 0..8 {
        let mut fixture = request(REGISTERS);
        let abi = &mut binding_mut(&mut fixture).abi;
        match case {
            0 => abi.can_unwind = true,
            1 => abi.source_signature.c_variadic = true,
            2 => abi.canon_abi = SemanticCanonAbiV1::C,
            3 => abi.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false },
            4 => abi.arguments[3].value.mode = SemanticAbiPassModeV1::Ignore,
            5 => abi.return_value.mode = SemanticAbiPassModeV1::Ignore,
            6 => abi.source_signature.inputs[3] = SemanticTypeIdV1(0),
            7 => abi.source_signature.inputs = vec![SemanticTypeIdV1(0); 1024].into_boxed_slice(),
            _ => unreachable!(),
        }
        fixture.functions[0].blocks =
            vec![block(30, SemanticTerminatorKindV1::Return)].into_boxed_slice();
        assert_eq!(
            fixture
                .admit_exact_v31(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
    for (index, signed, bits) in [(0, true, 32), (0, false, 64), (1, true, 8), (1, false, 16)] {
        let mut fixture = request(REGISTERS);
        fixture.types[index].shape =
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits });
        let abi = &binding_mut(&mut fixture).abi.clone();
        assert!(!gfx942_ordered_region_v31::signature_matches(&fixture, abi));
        assert!(
            fixture
                .admit_exact_v31(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
    let mut wrong_destination = request(REGISTERS);
    call_mut(&mut wrong_destination).destination = None;
    assert!(
        wrong_destination
            .admit_exact_v31(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn profile_and_source_byte_mutations_fail_closed() {
    let admitted = admitted(request(REGISTERS));
    let bytes = admitted.canonical_encoding();
    let operation = unique_offset(bytes, &[0xa7; 32]) - 2;
    for profile in 1..=255 {
        let mut altered = bytes.to_vec();
        altered[operation + 1] = profile;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "gfx942 ordered region profile",
                ..
            })
        ));
    }
    for tag in (69..=86).chain(89..=255) {
        let mut altered = bytes.to_vec();
        altered[operation] = tag;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                ..
            })
        ));
    }
    let start = unique_offset(bytes, &[0xb1; 32]);
    assert_eq!(&bytes[start - 2..start], &[0, 1]); // old source absent, new source present
    for tag in [2, 255] {
        let mut altered = bytes.to_vec();
        altered[start - 1] = tag;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "ordered region source",
                ..
            })
        ));
    }
    for field in 0..4 {
        let mut altered = bytes.to_vec();
        altered[start + field * 32..start + (field + 1) * 32].fill(0);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidOrderedRegionV31
            ))
        ));
    }
    let mut wrong_caller = bytes.to_vec();
    wrong_caller[start + 32..start + 64].fill(99);
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
            &wrong_caller,
            SemanticMirLimitsV1::default()
        ),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::InvalidOrderedRegionV31
        ))
    ));
}

#[test]
fn frozen_v31_scalar_frames_and_both_source_options_stay_unchanged() {
    let ordinary = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1(7),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let ordinary_bytes = vec![2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut writer, &ordinary, SemanticMirWireVersionV1::V30).unwrap();
    assert_eq!(writer.finish(), ordinary_bytes);
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut writer, &ordinary, SemanticMirWireVersionV1::V31).unwrap();
    let mut frozen_v31 = ordinary_bytes;
    frozen_v31.extend([0, 0]);
    assert_eq!(writer.finish(), frozen_v31);
    for (tag, instruction) in [
        SemanticGfx942InlineInstructionV30::VMovB32,
        SemanticGfx942InlineInstructionV30::VAddU32,
        SemanticGfx942InlineInstructionV30::VSubU32,
        SemanticGfx942InlineInstructionV30::VAndB32,
        SemanticGfx942InlineInstructionV30::VOrB32,
        SemanticGfx942InlineInstructionV30::VXorB32,
    ]
    .into_iter()
    .enumerate()
    {
        let operation = SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(instruction, 1).unwrap(),
        );
        let mut old = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        let mut new = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut old, operation, SemanticMirWireVersionV1::V34)
            .unwrap();
        encode_compiler_intrinsic_operation(&mut new, operation, SemanticMirWireVersionV1::V31)
            .unwrap();
        assert_eq!(old.finish(), [91, tag as u8, 1, 0]);
        assert_eq!(new.finish(), [87, tag as u8, 1, 0]);
    }
    let mut fixture = request(REGISTERS);
    let function = fixture.functions[0].identity;
    let old = call_mut(&mut fixture);
    old.ordered_region_source_v31 = None;
    old.inline_assembly_source_v30 = Some(old_source(function));
    let old = SemanticTerminatorKindV1::Call(old.clone());
    let mut previous = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    let mut current = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut previous, &old, SemanticMirWireVersionV1::V34).unwrap();
    encode_terminator(&mut current, &old, SemanticMirWireVersionV1::V31).unwrap();
    let mut previous = previous.finish();
    previous.push(0);
    assert_eq!(current.finish(), previous);
}

#[test]
fn old_schemas_reject_region_tag_and_never_consume_the_new_source_option() {
    for version in [
        SemanticMirWireVersionV1::V2,
        SemanticMirWireVersionV1::V3,
        SemanticMirWireVersionV1::V4,
        SemanticMirWireVersionV1::V5,
        SemanticMirWireVersionV1::V6,
        SemanticMirWireVersionV1::V7,
        SemanticMirWireVersionV1::V8,
        SemanticMirWireVersionV1::V9,
        SemanticMirWireVersionV1::V10,
        SemanticMirWireVersionV1::V11,
        SemanticMirWireVersionV1::V12,
        SemanticMirWireVersionV1::V13,
        SemanticMirWireVersionV1::V14,
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
    ] {
        assert!(matches!(
            request(REGISTERS).admit_for_wire_version(version, SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V31,
                ..
            })
        ));
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(PROFILE),
                version
            )
            .is_err()
        );
        let mut decoder = CanonicalDecoderV1::new(&[88, 0], SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert!(decoder.compiler_intrinsic().is_err());
    }
    let mut fixture = request(REGISTERS);
    let call = SemanticTerminatorKindV1::Call(call_mut(&mut fixture).clone());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(encode_terminator(&mut writer, &call, SemanticMirWireVersionV1::V30).is_err());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut writer, &call, SemanticMirWireVersionV1::V31).unwrap();
    let bytes = writer.finish();
    let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V30;
    let SemanticTerminatorKindV1::Call(old) = decoder.terminator().unwrap() else {
        unreachable!()
    };
    assert_eq!(old.inline_assembly_source_v30(), None);
    assert_eq!(old.ordered_region_source_v31(), None);
    assert!(decoder.finish().is_err());
}

#[test]
fn v31_never_accepts_hidden_or_unused_v29_execution_types_or_intrinsics() {
    let expected = SemanticMirErrorV1::WireVersionCannotRepresent {
        requested: SemanticMirWireVersionV1::V31,
        required: SemanticMirWireVersionV1::V29,
    };
    for role in [
        SemanticExecutionRoleV29::KernelContext,
        SemanticExecutionRoleV29::Workgroup,
        SemanticExecutionRoleV29::MaskedTileU32 {
            lanes: 64,
            elements: 1,
        },
        SemanticExecutionRoleV29::LaneFragmentU32 {
            lanes: 64,
            elements: 1,
        },
    ] {
        let mut fixture = request(REGISTERS);
        let mut types = fixture.types.to_vec();
        let mut hidden = types[0].clone();
        hidden.rust_type_kind = SemanticRustTypeKindV1::Execution(role);
        types.push(hidden);
        fixture.types = types.into_boxed_slice();
        assert_eq!(
            fixture
                .clone()
                .admit_exact_v31(SemanticMirLimitsV1::default())
                .unwrap_err(),
            expected
        );
        assert_eq!(
            fixture
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V28,
                required: SemanticMirWireVersionV1::V29,
            }
        );
    }
    let mut fixture = request(REGISTERS);
    let mut callables = fixture.callables.to_vec();
    let mut hidden = callables[1].clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut hidden else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Execution(
        SemanticExecutionOperationV29::ContextIssue {
            context: SemanticTypeIdV1(0),
        },
    );
    callables.push(hidden);
    fixture.callables = callables.into_boxed_slice();
    assert_eq!(
        fixture
            .admit_exact_v31(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    for tag in 14..=17 {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_type(
            &mut writer,
            &minimal_request().types[0],
            SemanticMirWireVersionV1::V31,
        )
        .unwrap();
        let mut bytes = writer.finish();
        let shape = bytes.len() - 5;
        assert_eq!(bytes[shape], 2);
        bytes[shape] = tag;
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V31;
        assert!(matches!(
            decoder.ty(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "type shape",
                ..
            })
        ));
    }
}

#[test]
fn canonical_bytes_and_cumulative_validation_work_have_exact_limits() {
    let fixture = request(REGISTERS);
    let admitted = admitted(fixture.clone());
    let bytes = admitted.canonical_encoding();
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, bytes.len() as u64)
        .unwrap();
    let short = SemanticMirLimitsV1::default()
        .with_limit(
            SemanticMirResourceV1::CanonicalBytes,
            bytes.len() as u64 - 1,
        )
        .unwrap();
    assert!(fixture.clone().admit_exact_v31(exact).is_ok());
    assert!(fixture.clone().admit_exact_v31(short).is_err());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v31_canonical(bytes, exact).is_ok());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v31_canonical(bytes, short),
        Err(SemanticMirDecodeErrorV1::InputLimitExceeded { .. })
    ));
    for length in [0, MAGIC.len(), bytes.len() - 1] {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v31_canonical(&bytes[..length], exact)
                .is_err()
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v31_canonical(
            &trailing,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    // Binary search the actual cumulative traversal budget, not an independent
    // per-call allowance. At most fourteen small inert admissions are performed.
    let mut low = 0;
    let mut high = 8192;
    let work_limit = |value| {
        SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, value)
            .unwrap()
    };
    assert!(fixture.clone().admit_exact_v31(work_limit(high)).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        if fixture.clone().admit_exact_v31(work_limit(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low >= 96); // explicit callable+call checks, in addition to existing work
    assert!(fixture.clone().admit_exact_v31(work_limit(low)).is_ok());
    assert!(matches!(
        fixture.admit_exact_v31(work_limit(low - 1)),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}
