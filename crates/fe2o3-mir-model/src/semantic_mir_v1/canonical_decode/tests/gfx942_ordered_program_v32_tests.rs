//! Inert grammar/codec controls, not authenticated source-produced evidence.

use super::*;

#[test]
fn exact_v32_framing_round_trip_and_current_selection() {
    for registers in [REGISTERS, [0, 63, 1, 62, 2], [63, 0, 62, 1, 61]] {
        let original = request(registers);
        let owner = admitted(original.clone());
        let bytes = owner.canonical_encoding();
        assert_eq!(&bytes[MAGIC.len()..MAGIC.len() + 2], &[32, 0]);
        let offset = unique_offset(bytes, &[0xa7; 32]) - 35;
        // Independent literal frame, not the model's packer or encoder.
        let mut expected = vec![89, 0, 3, 0x85, 0, 0x33, 1, 0x9d, 1];
        expected.extend([0; 26]);
        assert_eq!(&bytes[offset..offset + 35], expected);
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.request, original);
        assert_eq!(decoded.semantic_sha256(), owner.semantic_sha256());
        let checked = decoded
            .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
            .unwrap();
        assert_eq!(checked.program(), program());
        assert_eq!(checked.source(), source(original.functions[0].identity));
        assert_eq!(checked.registers().scratch(), registers[0]);
        assert_eq!(checked.registers().output(), registers[1]);
        assert_eq!(checked.registers().inputs(), registers[2..]);
        let SemanticTerminatorKindV1::Call(call) =
            &decoded.request.functions[0].blocks[0].terminator.kind
        else {
            unreachable!()
        };
        assert!(std::ptr::eq(
            checked.inputs().as_ptr(),
            call.arguments.as_ptr()
        ));
        assert!(call.ordered_region_source_v31.is_none());
        let occurrence = unique_offset(bytes, &[0xb1; 32]);
        assert_eq!(&bytes[occurrence - 2..occurrence], &[0, 1]);
        for selected in [
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
            assert_eq!(selected.wire_version(), SemanticMirWireVersionV1::V32);
            assert_eq!(selected.canonical_encoding(), bytes);
        }
    }
}

fn decode_program_frame(
    bytes: &[u8],
) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
    let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V32;
    let value = decoder.compiler_intrinsic()?;
    decoder.finish()?;
    Ok(value)
}

#[test]
fn revision_count_padding_and_descriptor_corruption_are_rejected() {
    let mut frame = vec![89, 0, 1, 8, 0]; // mov output,input0
    frame.extend([0; 30]);
    assert_eq!(
        decode_program_frame(&frame).unwrap(),
        SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program_from(&[8]))
    );
    for revision in 1..=255 {
        let mut changed = frame.clone();
        changed[1] = revision;
        assert!(matches!(
            decode_program_frame(&changed),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "gfx942 ordered program revision",
                ..
            })
        ));
    }
    for count in [0, 17, 255] {
        let mut changed = frame.clone();
        changed[2] = count;
        assert!(matches!(
            decode_program_frame(&changed),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidOrderedProgramV32
            ))
        ));
    }
    for slot in 1..16 {
        for bit in 0..16 {
            let mut changed = frame.clone();
            let word = (1_u16 << bit).to_le_bytes();
            changed[3 + 2 * slot..5 + 2 * slot].copy_from_slice(&word);
            assert!(
                decode_program_frame(&changed).is_err(),
                "inactive slot={slot}, bit={bit}"
            );
        }
    }
    // Reserved bits, illegal opcodes/roles, unary's nonzero unused source,
    // reads of uninitialized scratch/output, and no final output definition.
    for word in [
        0x408_u16, 0x8008, 0x000e, 0x000f, 0x0058, 0x0389, 0x0088, 0x0038, 0x0048, 0,
    ] {
        let mut changed = frame.clone();
        changed[3..5].copy_from_slice(&word.to_le_bytes());
        assert!(
            decode_program_frame(&changed).is_err(),
            "descriptor={word:04x}"
        );
    }
    for length in 0..frame.len() {
        assert!(decode_program_frame(&frame[..length]).is_err());
    }
    frame.push(0);
    assert!(decode_program_frame(&frame).is_err());
}

#[test]
fn v32_has_an_explicit_intrinsic_allowlist_not_an_ordinal_promotion() {
    for tag in (69..=86).chain([88]).chain(90..=255) {
        assert!(matches!(decode_program_frame(&[tag]),
            Err(SemanticMirDecodeErrorV1::InvalidTag { context: "compiler intrinsic", value, .. }) if value == tag));
    }
    let old_scalar = SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
        SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VXorB32, 1).unwrap(),
    );
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_compiler_intrinsic_operation(&mut writer, old_scalar, SemanticMirWireVersionV1::V32)
        .unwrap();
    assert_eq!(decode_program_frame(&writer.finish()).unwrap(), old_scalar);
}

#[test]
fn old_versions_and_header_substitution_never_admit_program_bytes() {
    let owner = admitted(request(REGISTERS));
    for number in (2..=15).chain([28, 29, 30, 31]) {
        let version = SemanticMirWireVersionV1::from_u16(number).unwrap();
        assert!(matches!(
            request(REGISTERS).admit_for_wire_version(version, SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V32,
                ..
            })
        ));
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program()),
                version
            )
            .is_err()
        );
        let mut decoder = CanonicalDecoderV1::new(&[89], SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert!(matches!(
            decoder.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                value: 89,
                ..
            })
        ));
        let mut substituted = owner.canonical_encoding().to_vec();
        substituted[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&number.to_le_bytes());
        assert!(
            AdmittedInertSemanticMirV1::decode_with_policy(
                &substituted,
                SemanticMirLimitsV1::default(),
                CanonicalDecodePolicyV1::Exact(version)
            )
            .is_err()
        );
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
                &substituted,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
}

#[test]
fn v31_fixed_pair_is_rejected_even_when_hidden_or_mixed_with_v32() {
    let expected = SemanticMirErrorV1::WireVersionCannotRepresent {
        requested: SemanticMirWireVersionV1::V32,
        required: SemanticMirWireVersionV1::V31,
    };
    let mut old = request(REGISTERS);
    let function = old.functions[0].identity;
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut old.callables[1] else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(
        SemanticGfx942OrderedRegionProfileV31::XorAddU32E32,
    );
    call_mut(&mut old).ordered_program_source_v32 = None;
    call_mut(&mut old).ordered_region_source_v31 = Some(
        SemanticOrderedRegionSourceV31::new([0xb1; 32], function, [0xb3; 32], [0xb4; 32]).unwrap(),
    );
    let old_owner = old
        .clone()
        .admit_exact_v31(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(
        old.clone()
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    let mut substituted = old_owner.canonical_encoding().to_vec();
    substituted[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&32_u16.to_le_bytes());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
            &substituted,
            SemanticMirLimitsV1::default()
        ),
        Err(SemanticMirDecodeErrorV1::InvalidTag {
            context: "compiler intrinsic",
            value: 88,
            ..
        })
    ));
    let mut mixed = request(REGISTERS);
    let mut callables = mixed.callables.to_vec();
    callables.push(old.callables[1].clone()); // unused fixed-pair declaration still excludes V32
    mixed.callables = callables.into_boxed_slice();
    assert_eq!(
        mixed
            .clone()
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    assert_eq!(
        mixed
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    // Merely attaching the sibling source option is enough to refuse the sibling grammar.
    let mut source_only = request(REGISTERS);
    call_mut(&mut source_only).ordered_region_source_v31 =
        call_mut(&mut old).ordered_region_source_v31;
    assert_eq!(
        source_only
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
}

#[test]
fn source_wire_options_and_each_identity_are_checked_without_authentication_claims() {
    let owner = admitted(request(REGISTERS));
    let bytes = owner.canonical_encoding();
    let start = unique_offset(bytes, &[0xb1; 32]);
    for tag in [2, 255] {
        let mut changed = bytes.to_vec();
        changed[start - 1] = tag;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
                &changed,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "ordered program source",
                ..
            })
        ));
    }
    for field in 0..4 {
        let mut changed = bytes.to_vec();
        changed[start + field * 32..start + (field + 1) * 32].fill(0);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
                &changed,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidOrderedProgramV32
            ))
        ));
    }
    let mut wrong_caller = bytes.to_vec();
    wrong_caller[start + 32..start + 64].fill(99);
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
            &wrong_caller,
            SemanticMirLimitsV1::default()
        ),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::InvalidOrderedProgramV32
        ))
    ));
}

#[test]
fn canonical_identity_retains_dead_repeated_and_self_steps() {
    let mut identities = std::collections::BTreeSet::new();
    for descriptors in [vec![8], vec![0, 8], vec![8, 8], vec![8, 72], vec![8; 16]] {
        let declared = program_from(&descriptors);
        let mut fixture = request(REGISTERS);
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut fixture.callables[1]
        else {
            unreachable!()
        };
        *operation = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(declared);
        let owner = admitted(fixture);
        assert!(identities.insert(*owner.semantic_sha256().as_bytes()));
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
            owner.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        let actual = decoded
            .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
            .unwrap()
            .program();
        assert_eq!(actual, declared);
        assert_eq!(actual.active_descriptors(), descriptors);
        assert_eq!(actual.evaluate([19, 23, 42]), 19);
    }
    assert_eq!(identities.len(), 5);
}

#[test]
fn frozen_v30_v31_encodings_and_new_sibling_call_layout_are_exact() {
    let ordinary = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1(7),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let frozen_v30 = vec![2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0];
    for version in [
        SemanticMirWireVersionV1::V30,
        SemanticMirWireVersionV1::V31,
        SemanticMirWireVersionV1::V32,
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_terminator(&mut writer, &ordinary, version).unwrap();
        let mut expected = frozen_v30.clone();
        if version != SemanticMirWireVersionV1::V30 {
            expected.push(0);
        }
        assert_eq!(writer.finish(), expected);
    }
    let fixed = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(
        SemanticGfx942OrderedRegionProfileV31::XorAddU32E32,
    );
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_compiler_intrinsic_operation(&mut writer, fixed, SemanticMirWireVersionV1::V31).unwrap();
    assert_eq!(writer.finish(), [88, 0]);
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(
        encode_compiler_intrinsic_operation(&mut writer, fixed, SemanticMirWireVersionV1::V32)
            .is_err()
    );
    let mut fixture = request(REGISTERS);
    let program_call = SemanticTerminatorKindV1::Call(call_mut(&mut fixture).clone());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert!(encode_terminator(&mut writer, &program_call, SemanticMirWireVersionV1::V31).is_err());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut writer, &program_call, SemanticMirWireVersionV1::V32).unwrap();
    let bytes = writer.finish();
    let mut old_decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
    old_decoder.wire_version = SemanticMirWireVersionV1::V30;
    let SemanticTerminatorKindV1::Call(old) = old_decoder.terminator().unwrap() else {
        unreachable!()
    };
    assert_eq!(old.ordered_program_source_v32(), None);
    assert!(old_decoder.finish().is_err());
}

const REGISTERS: [u8; 5] = [32, 33, 34, 35, 36];

fn source(function: SemanticFunctionIdentityV1) -> SemanticOrderedProgramSourceV32 {
    SemanticOrderedProgramSourceV32::new([0xb1; 32], function, [0xb3; 32], [0xb4; 32]).unwrap()
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
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program()),
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
    .with_ordered_program_source_v32(source(request.functions[0].identity));
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
        .admit_exact_v32(SemanticMirLimitsV1::default())
        .unwrap()
}

fn assert_invalid(request: InertSemanticMirRequestV1) {
    assert_eq!(
        request
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidOrderedProgramV32
    );
}

fn program() -> SemanticGfx942U32ProgramV32 {
    program_from(&[0x85, 0x133, 0x19d])
}

fn program_from(active: &[u16]) -> SemanticGfx942U32ProgramV32 {
    let mut descriptors = [0; 16];
    descriptors[..active.len()].copy_from_slice(active);
    SemanticGfx942U32ProgramV32::from_descriptors(active.len() as u8, descriptors).unwrap()
}
#[test]
fn source_options_are_exclusive_required_and_bound_to_actual_caller() {
    let mut missing = request(REGISTERS);
    call_mut(&mut missing).ordered_program_source_v32 = None;
    assert_invalid(missing);
    let mut wrong = request(REGISTERS);
    call_mut(&mut wrong).ordered_program_source_v32 =
        Some(source(SemanticFunctionIdentityV1(identity(99))));
    assert_invalid(wrong);
    let mut both = request(REGISTERS);
    let old = old_source(both.functions[0].identity);
    call_mut(&mut both).inline_assembly_source_v30 = Some(old);
    assert_invalid(both);
    let mut only_old = request(REGISTERS);
    let old = old_source(only_old.functions[0].identity);
    call_mut(&mut only_old).ordered_program_source_v32 = None;
    call_mut(&mut only_old).inline_assembly_source_v30 = Some(old);
    assert_invalid(only_old);
    let mut ordinary = request(REGISTERS);
    call_mut(&mut ordinary).callee = SemanticCallableIdV1(0);
    assert_invalid(ordinary);
    // Source refs are inert. A nonzero non-caller reference changes bytes, not authority.
    let mut changed = request(REGISTERS);
    call_mut(&mut changed).ordered_program_source_v32 = Some(
        SemanticOrderedProgramSourceV32::new(
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
fn mixed_v30_and_v32_calls_keep_independent_source_options_in_one_document() {
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
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
        bytes,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.request, fixture);
    assert_eq!(
        minimum_wire_version(&fixture),
        SemanticMirWireVersionV1::V32
    );
    assert!(
        decoded
            .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(0))
            .is_ok()
    );
    assert!(
        decoded
            .checked_gfx942_ordered_program_call_v32(SemanticFunctionIdV1(0), SemanticBlockIdV1(1))
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
    old_call.ordered_program_source_v32 = Some(source(function));
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
fn exact_signature_is_checked_even_for_an_unused_program_callable() {
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
                .admit_exact_v32(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
    for (index, signed, bits) in [(0, true, 32), (0, false, 64), (1, true, 8), (1, false, 16)] {
        let mut fixture = request(REGISTERS);
        fixture.types[index].shape =
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits });
        let abi = &binding_mut(&mut fixture).abi.clone();
        assert!(!gfx942_ordered_program_v32::signature_matches(
            &fixture, abi
        ));
        assert!(
            fixture
                .admit_exact_v32(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
    let mut wrong_destination = request(REGISTERS);
    call_mut(&mut wrong_destination).destination = None;
    assert!(
        wrong_destination
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn v32_never_accepts_hidden_or_unused_v29_execution_types_or_intrinsics() {
    let expected = SemanticMirErrorV1::WireVersionCannotRepresent {
        requested: SemanticMirWireVersionV1::V32,
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
                .admit_exact_v32(SemanticMirLimitsV1::default())
                .unwrap_err(),
            expected
        );
        assert_eq!(
            fixture
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            expected
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
            .admit_exact_v32(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    for tag in 14..=17 {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_type(
            &mut writer,
            &minimal_request().types[0],
            SemanticMirWireVersionV1::V32,
        )
        .unwrap();
        let mut bytes = writer.finish();
        let shape = bytes.len() - 5;
        assert_eq!(bytes[shape], 2);
        bytes[shape] = tag;
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V32;
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
    assert!(fixture.clone().admit_exact_v32(exact).is_ok());
    assert!(fixture.clone().admit_exact_v32(short).is_err());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v32_canonical(bytes, exact).is_ok());
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v32_canonical(bytes, short),
        Err(SemanticMirDecodeErrorV1::InputLimitExceeded { .. })
    ));
    for length in [0, MAGIC.len(), bytes.len() - 1] {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v32_canonical(&bytes[..length], exact)
                .is_err()
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v32_canonical(
            &trailing,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    // Binary search the actual cumulative traversal budget, not an independent
    // per-call allowance. At most sixteen small inert admissions are performed.
    let mut low = 0;
    let mut high = 32768;
    let work_limit = |value| {
        SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, value)
            .unwrap()
    };
    assert!(fixture.clone().admit_exact_v32(work_limit(high)).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        if fixture.clone().admit_exact_v32(work_limit(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low >= 8192); // explicit callable+call checks, in addition to existing work
    assert!(fixture.clone().admit_exact_v32(work_limit(low)).is_ok());
    assert!(matches!(
        fixture.admit_exact_v32(work_limit(low - 1)),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}
