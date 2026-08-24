use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_mir_kir_lineage::{
    BlockClassificationV4, BlockRecordV4, CANONICAL_KERNEL_IR_MAGIC_V6,
    CANONICAL_KERNEL_IR_V6_HEADER_BYTES, CANONICAL_KERNEL_IR_VERSION_V6,
    CHECKED_ARITHMETIC_EXTERNAL_OWNER_GATE_V4, CHECKED_ARITHMETIC_REFINEMENT_POLICY_VECTOR_V4,
    CanonicalKernelIrIdentityV4, CanonicalSemanticMirIdentityV4,
    CheckedArithmeticRefinementPolicyV4, DiagnosticTrapKindV4, F32IntrinsicV4,
    FunctionClassificationV4, FunctionRecordV4, InertCanonicalMirToKirLineageV4,
    KernelIrCanonicalWireVersionV4, KernelIrIdentitySchemeV4, KernelIrV6IdentityPreimageError,
    KernelRecordV4, LineageDecodeErrorV4, LineageDecodeLimitsV4, LineageEncodeErrorV4,
    LineageModelV4, LineagePolicyModeV4, LineageResourceV4, LineageValidationErrorV4,
    LineageWorkStageV4, LoweringConfigurationV4, LoweringResourceLimitsV4,
    MAX_CANONICAL_KERNEL_IR_BYTES_V4, MAX_CANONICAL_SEMANTIC_MIR_BYTES_V4, MAX_LINEAGE_BYTES_V4,
    RankedBoundsPolicyV4, SemanticMirCanonicalWireVersionV4, SemanticMirIdentitySchemeV4,
    SyntheticBlockRuleV4, VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1,
    VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1,
    recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1,
};

const DEFAULT_LINEAGE_INPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_STATEMENTS: usize = 1_048_576;

fn identity(byte: u8, length: u64) -> [u8; 32] {
    let mut value = [byte; 32];
    value[31] ^= u8::try_from(length & 0xff).unwrap();
    value
}

fn configuration() -> LoweringConfigurationV4 {
    LoweringConfigurationV4::new(
        RankedBoundsPolicyV4::RetainGenericChecks,
        LoweringResourceLimitsV4::default(),
    )
    .unwrap()
}

fn semantic_identity(byte: u8, length: u64) -> CanonicalSemanticMirIdentityV4 {
    CanonicalSemanticMirIdentityV4::new(
        SemanticMirCanonicalWireVersionV4::V3,
        identity(byte, length),
        length,
    )
    .unwrap()
}

fn kernel_ir_identity(byte: u8, length: u64) -> CanonicalKernelIrIdentityV4 {
    let bytes = canonical_kernel_ir_v6_fixture(byte, usize::try_from(length).unwrap());
    CanonicalKernelIrIdentityV4::new_v6(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&bytes).unwrap(),
    )
    .unwrap()
}

fn canonical_kernel_ir_v6_fixture(byte: u8, length: usize) -> Vec<u8> {
    assert!(length >= CANONICAL_KERNEL_IR_V6_HEADER_BYTES);
    let mut bytes = vec![byte; length];
    bytes[..8].copy_from_slice(&CANONICAL_KERNEL_IR_MAGIC_V6);
    bytes[8..10].copy_from_slice(&CANONICAL_KERNEL_IR_VERSION_V6.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
    bytes[16..20].copy_from_slice(&0_u32.to_le_bytes());
    bytes
}

fn semantic_block(
    kir_block: u64,
    semantic_block: u64,
    operation_count: u64,
    statement_operation_counts: &[u64],
    terminator_operation_count: u64,
) -> BlockRecordV4 {
    BlockRecordV4::semantic(
        kir_block,
        operation_count,
        semantic_block,
        statement_operation_counts.to_vec(),
        terminator_operation_count,
    )
    .unwrap()
}

fn sample_model() -> LineageModelV4 {
    let functions = vec![
        FunctionRecordV4::semantic_body(
            0,
            0,
            2,
            vec![
                semantic_block(0, 0, 3, &[0, 2], 1),
                BlockRecordV4::synthetic(2, 1, SyntheticBlockRuleV4::RuntimeAssertFailureTrap)
                    .unwrap(),
                semantic_block(1, 1, 1, &[1], 0),
            ],
        ),
        FunctionRecordV4::f32_intrinsic_declaration(1, F32IntrinsicV4::Sqrt),
        FunctionRecordV4::diagnostic_trap_declaration(
            2,
            DiagnosticTrapKindV4::RuntimeAssertFailure,
        ),
    ];
    LineageModelV4::new(
        semantic_identity(0x11, 123),
        kernel_ir_identity(0x22, 456),
        configuration(),
        functions,
        vec![KernelRecordV4::new(0, 0, 0)],
    )
    .unwrap()
}

fn sample_canonical() -> Vec<u8> {
    InertCanonicalMirToKirLineageV4::from_model(sample_model(), LineageDecodeLimitsV4::default())
        .unwrap()
        .canonical_bytes()
        .to_vec()
}

#[derive(Debug)]
struct WireOffsets {
    totals: [usize; 8],
    semantic_scheme: usize,
    semantic_version: usize,
    kernel_ir_scheme: usize,
    kernel_ir_version: usize,
    policy_version: usize,
    policy_mode: usize,
    target: usize,
    checked_arithmetic: usize,
    first_function_ordinal: usize,
    first_function_classification: usize,
    semantic_block_count: usize,
    first_statement_count: usize,
    second_statement_count: usize,
    first_terminator_count: usize,
}

fn take_varint(bytes: &[u8], cursor: &mut usize) -> (usize, u64) {
    let start = *cursor;
    let mut value = 0_u64;
    let mut shift = 0_u32;
    loop {
        let byte = bytes[*cursor];
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return (start, value);
        }
        shift += 7;
    }
}

fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let mut cursor = 8;
    take_varint(bytes, &mut cursor); // version
    take_varint(bytes, &mut cursor); // flags
    let semantic_scheme = take_varint(bytes, &mut cursor).0;
    let semantic_version = take_varint(bytes, &mut cursor).0;
    cursor += 32;
    take_varint(bytes, &mut cursor); // semantic length
    let kernel_ir_scheme = take_varint(bytes, &mut cursor).0;
    let kernel_ir_version = take_varint(bytes, &mut cursor).0;
    cursor += 32;
    take_varint(bytes, &mut cursor); // KIR length
    let policy_version = take_varint(bytes, &mut cursor).0;
    let policy_mode = take_varint(bytes, &mut cursor).0;
    let target = take_varint(bytes, &mut cursor).0;
    for _ in 0..4 {
        take_varint(bytes, &mut cursor); // remaining non-arithmetic policies
    }
    let checked_arithmetic = take_varint(bytes, &mut cursor).0;
    for _ in 0..7 {
        take_varint(bytes, &mut cursor); // lowering limits
    }
    let mut totals = [0_usize; 8];
    for offset in &mut totals {
        *offset = take_varint(bytes, &mut cursor).0;
    }

    let first_function_ordinal = take_varint(bytes, &mut cursor).0;
    let (first_function_classification, classification) = take_varint(bytes, &mut cursor);
    assert_eq!(classification, 0); // semantic body
    take_varint(bytes, &mut cursor); // semantic function
    let semantic_block_count = take_varint(bytes, &mut cursor).0;
    take_varint(bytes, &mut cursor); // KIR block count

    take_varint(bytes, &mut cursor); // KIR block
    take_varint(bytes, &mut cursor); // block operation count
    assert_eq!(take_varint(bytes, &mut cursor).1, 0); // semantic block
    take_varint(bytes, &mut cursor); // semantic block
    assert_eq!(take_varint(bytes, &mut cursor).1, 2); // statements
    let first_statement_count = take_varint(bytes, &mut cursor).0;
    let second_statement_count = take_varint(bytes, &mut cursor).0;
    let first_terminator_count = take_varint(bytes, &mut cursor).0;

    WireOffsets {
        totals,
        semantic_scheme,
        semantic_version,
        kernel_ir_scheme,
        kernel_ir_version,
        policy_version,
        policy_mode,
        target,
        checked_arithmetic,
        first_function_ordinal,
        first_function_classification,
        semantic_block_count,
        first_statement_count,
        second_statement_count,
        first_terminator_count,
    }
}

fn encode_varint(mut value: u64) -> Vec<u8> {
    let mut output = Vec::new();
    loop {
        let mut byte = u8::try_from(value & 0x7f).unwrap();
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            return output;
        }
    }
}

fn replace_varint(bytes: &mut Vec<u8>, offset: usize, value: u64) {
    let mut end = offset;
    loop {
        let byte = bytes[end];
        end += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    bytes.splice(offset..end, encode_varint(value));
}

fn permissive_limits(input_bytes: usize) -> LineageDecodeLimitsV4 {
    limits_with_work(input_bytes, u64::MAX)
}

fn limits_with_work(input_bytes: usize, max_work: u64) -> LineageDecodeLimitsV4 {
    LineageDecodeLimitsV4::new(
        u64::try_from(input_bytes).unwrap(),
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        max_work,
    )
}

fn zero_operation_statement_model(statement_count: usize) -> LineageModelV4 {
    zero_operation_statement_model_with_configuration(statement_count, configuration())
}

fn zero_operation_statement_model_with_configuration(
    statement_count: usize,
    configuration: LoweringConfigurationV4,
) -> LineageModelV4 {
    LineageModelV4::new(
        semantic_identity(0x31, 1),
        kernel_ir_identity(0x41, 20),
        configuration,
        vec![FunctionRecordV4::semantic_body(
            0,
            0,
            1,
            vec![BlockRecordV4::semantic(0, 0, 0, vec![0; statement_count], 0).unwrap()],
        )],
        vec![],
    )
    .unwrap()
}

#[test]
fn canonical_roundtrip_retains_every_v4_axis() {
    let model = sample_model();
    assert_eq!(
        model.semantic_mir().scheme(),
        SemanticMirIdentitySchemeV4::RawCanonicalSha256
    );
    assert_eq!(
        model.semantic_mir().wire_version(),
        SemanticMirCanonicalWireVersionV4::V3
    );
    assert_eq!(
        model.kernel_ir().scheme(),
        KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1
    );
    assert_eq!(
        model.kernel_ir().wire_version(),
        KernelIrCanonicalWireVersionV4::V6
    );
    assert_eq!(
        model.configuration().mode(),
        LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6
    );
    assert_eq!(
        model.configuration().checked_arithmetic(),
        CheckedArithmeticRefinementPolicyV4::SemanticMirV3ToKernelIrV6CheckedV1
    );
    let totals = model.totals();
    assert_eq!(totals.semantic_functions(), 1);
    assert_eq!(totals.kir_functions(), 3);
    assert_eq!(totals.kernels(), 1);
    assert_eq!(totals.semantic_blocks(), 2);
    assert_eq!(totals.synthetic_blocks(), 1);
    assert_eq!(totals.statements(), 3);
    assert_eq!(totals.terminators(), 2);
    assert_eq!(totals.operations(), 5);

    let encoded = InertCanonicalMirToKirLineageV4::from_model(
        model.clone(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    let decoded = InertCanonicalMirToKirLineageV4::decode_canonical(
        encoded.canonical_bytes(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    assert_eq!(decoded.model(), &model);
    assert_eq!(decoded.canonical_bytes(), encoded.canonical_bytes());
    decoded
        .revalidate(LineageDecodeLimitsV4::default())
        .unwrap();

    assert!(matches!(
        decoded.model().functions()[1].classification(),
        FunctionClassificationV4::F32IntrinsicDeclaration(F32IntrinsicV4::Sqrt)
    ));
    assert!(matches!(
        decoded.model().functions()[2].classification(),
        FunctionClassificationV4::DiagnosticTrapDeclaration(
            DiagnosticTrapKindV4::RuntimeAssertFailure
        )
    ));
    assert!(matches!(
        decoded.model().functions()[0].blocks()[1].classification(),
        BlockClassificationV4::SyntheticBlock {
            rule: SyntheticBlockRuleV4::RuntimeAssertFailureTrap,
            ..
        }
    ));
}

#[test]
fn zero_operation_statement_is_explicit_and_does_not_shift_following_spans() {
    let decoded = InertCanonicalMirToKirLineageV4::decode_canonical(
        &sample_canonical(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    let block = &decoded.model().functions()[0].blocks()[0];
    let statements = block
        .statement_operation_spans()
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(statements[0].operations().first_operation_ordinal(), 0);
    assert_eq!(statements[0].operations().operation_count(), 0);
    assert_eq!(statements[1].operations().first_operation_ordinal(), 0);
    assert_eq!(statements[1].operations().operation_count(), 2);
    assert_eq!(statements[0].statement_ordinal(), 0);
    assert_eq!(statements[1].statement_ordinal(), 1);
    let terminator = block.terminator_operation_span().unwrap().operations();
    assert_eq!(terminator.first_operation_ordinal(), 2);
    assert_eq!(terminator.operation_count(), 1);
}

#[test]
fn every_strict_prefix_is_rejected_as_truncated_or_incomplete() {
    let bytes = sample_canonical();
    for length in 0..bytes.len() {
        assert!(
            InertCanonicalMirToKirLineageV4::decode_canonical(
                &bytes[..length],
                LineageDecodeLimitsV4::default(),
            )
            .is_err(),
            "accepted strict prefix of length {length}"
        );
    }
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = sample_canonical();
    bytes.push(0);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, LineageDecodeLimitsV4::default()),
        Err(LineageDecodeErrorV4::TrailingBytes { trailing: 1, .. })
    ));
}

#[test]
fn non_shortest_and_overflowing_varints_are_rejected() {
    let canonical = sample_canonical();
    let mut non_shortest = canonical.clone();
    non_shortest.splice(8..9, [0x84, 0x00]);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &non_shortest,
            permissive_limits(non_shortest.len())
        ),
        Err(LineageDecodeErrorV4::NonShortestVarint { offset: 8 })
    ));

    let mut overflowing = canonical;
    overflowing.splice(8..9, [0xff; 10]);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &overflowing,
            permissive_limits(overflowing.len())
        ),
        Err(LineageDecodeErrorV4::VarintOverflow { offset: 8 })
    ));
}

#[test]
fn invalid_header_and_closed_tags_are_rejected() {
    let canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);

    let mut invalid_magic = canonical.clone();
    invalid_magic[0] ^= 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &invalid_magic,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::InvalidMagic)
    ));

    let mut unsupported_version = canonical.clone();
    unsupported_version[8] = 5;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &unsupported_version,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::UnsupportedVersion(5))
    ));

    let mut unsupported_flags = canonical.clone();
    unsupported_flags[9] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &unsupported_flags,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::UnsupportedFlags(1))
    ));

    let mut old_policy_version = canonical.clone();
    old_policy_version[offsets.policy_version] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &old_policy_version,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::UnsupportedPolicyVersion(1)
        ))
    ));

    for (offset, context) in [
        (offsets.semantic_scheme, "semantic MIR identity scheme"),
        (offsets.kernel_ir_scheme, "Kernel IR identity scheme"),
        (offsets.policy_mode, "lineage policy mode"),
        (
            offsets.checked_arithmetic,
            "checked-arithmetic refinement policy",
        ),
    ] {
        let mut invalid = canonical.clone();
        invalid[offset] = 9;
        assert!(matches!(
            InertCanonicalMirToKirLineageV4::decode_canonical(
                &invalid,
                LineageDecodeLimitsV4::default()
            ),
            Err(LineageDecodeErrorV4::InvalidTag {
                context: actual,
                value: 9,
                ..
            }) if actual == context
        ));
    }

    for (offset, value, context) in [
        (
            offsets.semantic_version,
            4,
            "semantic MIR canonical wire version",
        ),
        (
            offsets.kernel_ir_version,
            7,
            "Kernel IR canonical wire version",
        ),
    ] {
        let mut invalid = canonical.clone();
        invalid[offset] = value;
        assert!(matches!(
            InertCanonicalMirToKirLineageV4::decode_canonical(
                &invalid,
                LineageDecodeLimitsV4::default()
            ),
            Err(LineageDecodeErrorV4::InvalidTag {
                context: actual,
                value: actual_value,
                ..
            }) if actual == context && actual_value == u64::from(value)
        ));
    }

    let mut invalid_target = canonical.clone();
    invalid_target[offsets.target] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &invalid_target,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::InvalidTag {
            context: "lowering target",
            value: 1,
            ..
        })
    ));

    let mut legacy_arithmetic_in_production = canonical.clone();
    legacy_arithmetic_in_production[offsets.checked_arithmetic] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &legacy_arithmetic_in_production,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::ArtifactVersionPolicyMismatch {
                mode: LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6,
                ..
            }
        ))
    ));

    let mut invalid_function_class = canonical;
    invalid_function_class[offsets.first_function_classification] = 3;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &invalid_function_class,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::InvalidTag {
            context: "function classification",
            value: 3,
            ..
        })
    ));
}

#[test]
fn declared_totals_and_canonical_ordinals_are_exact() {
    let canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);

    let mut wrong_terminators = canonical.clone();
    wrong_terminators[offsets.totals[6]] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &wrong_terminators,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::DeclaredTotalsMismatch {
                context: "terminators",
                declared: 1,
                actual: 2,
            }
        ))
    ));

    let mut underclaimed_operations = canonical.clone();
    underclaimed_operations[offsets.totals[7]] = 4;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &underclaimed_operations,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::CountMismatch {
            context: "operations",
            declared: 4,
            observed: 5,
        })
    ));

    let mut noncanonical_function = canonical;
    noncanonical_function[offsets.first_function_ordinal] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &noncanonical_function,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::NonCanonicalOrdinal {
                context: "Kernel IR function",
                expected: 0,
                actual: 1,
            }
        ))
    ));
}

#[test]
fn input_count_and_work_limits_fail_before_record_allocation() {
    let bytes = sample_canonical();
    let too_short = LineageDecodeLimitsV4::new(
        u64::try_from(bytes.len() - 1).unwrap(),
        1_024,
        2_048,
        1_024,
        16_384,
        1_048_576,
        4_194_304,
        16_777_216,
    );
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, too_short),
        Err(LineageDecodeErrorV4::InputLimitExceeded { .. })
    ));

    let count_limited = LineageDecodeLimitsV4::new(
        u64::try_from(bytes.len()).unwrap(),
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    );
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, count_limited),
        Err(LineageDecodeErrorV4::CountLimitExceeded {
            resource: LineageResourceV4::SemanticFunctions,
            ..
        })
    ));

    let work_limited = LineageDecodeLimitsV4::new(
        u64::try_from(bytes.len()).unwrap(),
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        7,
    );
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, work_limited),
        Err(LineageDecodeErrorV4::WorkLimitExceeded { .. })
    ));
}

#[test]
fn hard_lineage_cap_is_independent_and_cannot_be_widened() {
    assert_eq!(MAX_LINEAGE_BYTES_V4, 4_194_304);
    let widened = LineageDecodeLimitsV4::new(
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    );
    assert_eq!(widened.max_input_bytes(), MAX_LINEAGE_BYTES_V4);

    let exact = vec![0; usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap()];
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&exact, widened),
        Err(LineageDecodeErrorV4::InvalidMagic)
    ));
    drop(exact);

    for length in [MAX_LINEAGE_BYTES_V4 + 1, 4_194_429] {
        let oversized = vec![0; usize::try_from(length).unwrap()];
        assert!(matches!(
            InertCanonicalMirToKirLineageV4::decode_canonical(&oversized, widened),
            Err(LineageDecodeErrorV4::InputLimitExceeded {
                actual,
                max: MAX_LINEAGE_BYTES_V4,
            }) if actual == length
        ));
    }

    let sample = sample_canonical();
    let tightened = u64::try_from(sample.len() - 1).unwrap();
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::from_model(
            sample_model(),
            limits_with_work(sample.len() - 1, u64::MAX),
        ),
        Err(LineageEncodeErrorV4::OutputLimitExceeded { max, .. }) if max == tightened
    ));
}

#[test]
fn from_model_accepts_the_exact_hard_cap_and_rejects_the_next_byte() {
    let max_statements = MAX_LINEAGE_BYTES_V4 + 1;
    let configuration = LoweringConfigurationV4::new(
        RankedBoundsPolicyV4::RetainGenericChecks,
        LoweringResourceLimitsV4::new(1, 1, 1, 1, max_statements, 1, u64::MAX).unwrap(),
    )
    .unwrap();
    let limits = LineageDecodeLimitsV4::new(u64::MAX, 1, 1, 0, 1, max_statements, 0, u64::MAX);

    let empty = InertCanonicalMirToKirLineageV4::from_model(
        zero_operation_statement_model_with_configuration(0, configuration),
        limits,
    )
    .unwrap();
    // Near 4 MiB, both the aggregate statement total and per-block statement
    // count use four-byte varints instead of the empty model's one-byte forms.
    let exact_count =
        usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap() - empty.canonical_bytes().len() - 6;
    assert_eq!(encode_varint(u64::try_from(exact_count).unwrap()).len(), 4);
    drop(empty);

    let exact = InertCanonicalMirToKirLineageV4::from_model(
        zero_operation_statement_model_with_configuration(exact_count, configuration),
        limits,
    )
    .unwrap();
    assert_eq!(
        u64::try_from(exact.canonical_bytes().len()).unwrap(),
        MAX_LINEAGE_BYTES_V4
    );
    drop(exact);

    assert!(matches!(
        InertCanonicalMirToKirLineageV4::from_model(
            zero_operation_statement_model_with_configuration(exact_count + 1, configuration),
            limits,
        ),
        Err(LineageEncodeErrorV4::OutputLimitExceeded {
            actual,
            max: MAX_LINEAGE_BYTES_V4,
        }) if actual == MAX_LINEAGE_BYTES_V4 + 1
    ));
}

#[test]
fn from_model_enforces_each_supplied_count_limit_at_exact_boundaries() {
    let exact = LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 3, 1, 3, 3, 5, u64::MAX);
    InertCanonicalMirToKirLineageV4::from_model(sample_model(), exact).unwrap();

    for (resource, limits, actual, max) in [
        (
            LineageResourceV4::SemanticFunctions,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 0, 3, 1, 3, 3, 5, u64::MAX),
            1,
            0,
        ),
        (
            LineageResourceV4::KirFunctions,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 2, 1, 3, 3, 5, u64::MAX),
            3,
            2,
        ),
        (
            LineageResourceV4::Kernels,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 3, 0, 3, 3, 5, u64::MAX),
            1,
            0,
        ),
        (
            LineageResourceV4::Blocks,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 3, 1, 2, 3, 5, u64::MAX),
            3,
            2,
        ),
        (
            LineageResourceV4::Statements,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 3, 1, 3, 2, 5, u64::MAX),
            3,
            2,
        ),
        (
            LineageResourceV4::Operations,
            LineageDecodeLimitsV4::new(MAX_LINEAGE_BYTES_V4, 1, 3, 1, 3, 3, 4, u64::MAX),
            5,
            4,
        ),
    ] {
        assert!(matches!(
            InertCanonicalMirToKirLineageV4::from_model(sample_model(), limits),
            Err(LineageEncodeErrorV4::CountLimitExceeded {
                resource: actual_resource,
                actual: actual_count,
                max: actual_max,
            }) if actual_resource == resource && actual_count == actual && actual_max == max
        ));
    }
}

#[test]
fn one_shared_work_budget_is_exact_across_decode_validation_and_reencoding() {
    let bytes = sample_canonical();
    let measured =
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, permissive_limits(bytes.len()))
            .unwrap();
    let work = measured.admission_work();
    let totals = measured.model().totals();
    let record_work = totals.kir_functions()
        + totals.kernels()
        + totals.semantic_blocks()
        + totals.synthetic_blocks()
        + totals.statements()
        + totals.terminators();
    assert_eq!(
        work.parse(),
        u64::try_from(bytes.len()).unwrap() + record_work
    );
    assert!(work.structural_validation() > 0);
    assert_eq!(
        work.canonical_encoding(),
        u64::try_from(bytes.len()).unwrap()
    );
    assert_eq!(
        work.total(),
        work.parse() + work.structural_validation() + work.canonical_encoding()
    );

    let exact = InertCanonicalMirToKirLineageV4::decode_canonical(
        &bytes,
        limits_with_work(bytes.len(), work.total()),
    )
    .unwrap();
    assert_eq!(exact.admission_work(), work);

    let one_short = work.total() - 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &bytes,
            limits_with_work(bytes.len(), one_short),
        ),
        Err(LineageDecodeErrorV4::WorkLimitExceeded {
            stage: LineageWorkStageV4::CanonicalEncoding,
            actual,
            max,
        }) if actual == work.total() && max == one_short
    ));
}

#[test]
fn from_model_uses_one_exact_structural_and_encoding_work_budget() {
    let measured = InertCanonicalMirToKirLineageV4::from_model(
        sample_model(),
        limits_with_work(usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap(), u64::MAX),
    )
    .unwrap();
    let work = measured.admission_work();
    assert_eq!(work.parse(), 0);
    assert!(work.structural_validation() > 0);
    assert_eq!(
        work.canonical_encoding(),
        u64::try_from(measured.canonical_bytes().len()).unwrap()
    );
    assert_eq!(
        work.total(),
        work.structural_validation() + work.canonical_encoding()
    );

    let exact = InertCanonicalMirToKirLineageV4::from_model(
        sample_model(),
        limits_with_work(usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap(), work.total()),
    )
    .unwrap();
    assert_eq!(exact.admission_work(), work);

    let one_short = work.total() - 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::from_model(
            sample_model(),
            limits_with_work(usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap(), one_short),
        ),
        Err(LineageEncodeErrorV4::WorkLimitExceeded {
            stage: LineageWorkStageV4::CanonicalEncoding,
            actual,
            max,
        }) if actual == work.total() && max == one_short
    ));
}

#[test]
fn aggregate_count_and_work_overflow_are_rejected_from_header_totals() {
    let canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);

    let mut count_overflow = canonical.clone();
    replace_varint(&mut count_overflow, offsets.totals[3], u64::MAX);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &count_overflow,
            permissive_limits(count_overflow.len())
        ),
        Err(LineageDecodeErrorV4::CountOverflow {
            resource: LineageResourceV4::Blocks
        })
    ));

    let mut work_overflow = canonical;
    replace_varint(&mut work_overflow, offsets.totals[1], u64::MAX);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &work_overflow,
            permissive_limits(work_overflow.len())
        ),
        Err(LineageDecodeErrorV4::WorkOverflow {
            stage: LineageWorkStageV4::Parse
        })
    ));
}

#[test]
fn nested_semantic_block_count_is_rejected_before_its_payload() {
    let mut bytes = sample_canonical();
    let offset = wire_offsets(&bytes).semantic_block_count;
    replace_varint(&mut bytes, offset, u64::MAX);
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(&bytes, permissive_limits(bytes.len())),
        Err(LineageDecodeErrorV4::CountMismatch {
            context: "declared semantic blocks",
            declared: 2,
            observed: u64::MAX,
        })
    ));
}

#[test]
fn compressed_operation_counts_reject_incomplete_or_excessive_coverage() {
    let canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);

    let mut first_statement_excess = canonical.clone();
    first_statement_excess[offsets.first_statement_count] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &first_statement_excess,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::OperationCoverageMismatch { .. }
        ))
    ));

    let mut second_statement_short = canonical.clone();
    second_statement_short[offsets.second_statement_count] = 1;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &second_statement_short,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::OperationCoverageMismatch { .. }
        ))
    ));

    let mut terminator_short = canonical;
    terminator_short[offsets.first_terminator_count] = 0;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &terminator_short,
            LineageDecodeLimitsV4::default()
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::OperationCoverageMismatch { .. }
        ))
    ));
}

#[test]
fn standalone_block_api_rejects_cumulative_span_overflow() {
    assert!(matches!(
        BlockRecordV4::semantic(0, u64::MAX, 0, vec![u64::MAX, 1], 0),
        Err(LineageValidationErrorV4::ArithmeticOverflow {
            resource: LineageResourceV4::Operations,
        })
    ));

    let block = BlockRecordV4::semantic(0, u64::MAX, 0, vec![u64::MAX], 0).unwrap();
    let mut spans = block.statement_operation_spans().unwrap();
    let span = spans.next().unwrap();
    assert_eq!(span.statement_ordinal(), 0);
    assert_eq!(span.operations().first_operation_ordinal(), 0);
    assert_eq!(span.operations().operation_count(), u64::MAX);
    assert!(spans.next().is_none());
    let terminator = block.terminator_operation_span().unwrap().operations();
    assert_eq!(terminator.first_operation_ordinal(), u64::MAX);
    assert_eq!(terminator.operation_count(), 0);
}

#[test]
fn only_the_closed_v3_to_v6_pair_is_production_eligible() {
    let base = sample_model();
    let semantic_v2 = CanonicalSemanticMirIdentityV4::new(
        SemanticMirCanonicalWireVersionV4::V2,
        identity(0x51, 2),
        2,
    )
    .unwrap();
    let kernel_ir_v5 =
        CanonicalKernelIrIdentityV4::new_legacy_v5_claimed_sha256_policy_v1(identity(0x61, 5), 5)
            .unwrap();

    let production_rejects_legacy = LineageModelV4::new(
        semantic_v2,
        kernel_ir_v5,
        configuration(),
        base.functions().to_vec(),
        base.kernels().to_vec(),
    );
    assert!(matches!(
        production_rejects_legacy,
        Err(LineageValidationErrorV4::ArtifactVersionPolicyMismatch {
            mode: LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6,
            ..
        })
    ));

    let legacy_configuration = LoweringConfigurationV4::legacy_inert(
        RankedBoundsPolicyV4::RetainGenericChecks,
        LoweringResourceLimitsV4::default(),
    )
    .unwrap();
    let legacy = LineageModelV4::new(
        semantic_v2,
        kernel_ir_v5,
        legacy_configuration,
        base.functions().to_vec(),
        base.kernels().to_vec(),
    )
    .unwrap();
    let encoded = InertCanonicalMirToKirLineageV4::from_model(
        legacy.clone(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    let decoded = InertCanonicalMirToKirLineageV4::decode_canonical(
        encoded.canonical_bytes(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    assert_eq!(decoded.model(), &legacy);
    assert_eq!(
        decoded.model().configuration().mode(),
        LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5
    );
    assert_eq!(
        decoded.model().configuration().checked_arithmetic(),
        CheckedArithmeticRefinementPolicyV4::LegacyInertNoRefinementAuthority
    );

    let legacy_rejects_production = LineageModelV4::new(
        base.semantic_mir(),
        base.kernel_ir(),
        legacy_configuration,
        base.functions().to_vec(),
        base.kernels().to_vec(),
    );
    assert!(matches!(
        legacy_rejects_production,
        Err(LineageValidationErrorV4::ArtifactVersionPolicyMismatch {
            mode: LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5,
            ..
        })
    ));
}

#[test]
fn known_legacy_versions_cannot_be_spliced_into_production_wire() {
    let canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);
    let mut semantic_v2 = canonical.clone();
    semantic_v2[offsets.semantic_version] = 2;
    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &semantic_v2,
            LineageDecodeLimitsV4::default(),
        ),
        Err(LineageDecodeErrorV4::Validation(
            LineageValidationErrorV4::ArtifactVersionPolicyMismatch {
                mode: LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6,
                ..
            }
        ))
    ));

    for (offset, value) in [
        (offsets.kernel_ir_version, 5_u8),
        (offsets.kernel_ir_scheme, 0_u8),
    ] {
        let mut mismatched_scheme = canonical.clone();
        mismatched_scheme[offset] = value;
        assert!(matches!(
            InertCanonicalMirToKirLineageV4::decode_canonical(
                &mismatched_scheme,
                LineageDecodeLimitsV4::default(),
            ),
            Err(LineageDecodeErrorV4::Validation(
                LineageValidationErrorV4::KernelIrIdentitySchemeVersionMismatch { .. }
            ))
        ));
    }
}

#[test]
fn semantic_mir_v4_cannot_be_claimed_by_the_legacy_kernel_ir_v6_lineage() {
    let mut canonical = sample_canonical();
    let offsets = wire_offsets(&canonical);
    canonical[offsets.semantic_version] = 4;

    assert!(matches!(
        InertCanonicalMirToKirLineageV4::decode_canonical(
            &canonical,
            LineageDecodeLimitsV4::default(),
        ),
        Err(LineageDecodeErrorV4::InvalidTag {
            context: "semantic MIR canonical wire version",
            value: 4,
            ..
        })
    ));
}

#[test]
fn model_rejects_trailing_operations_and_invalid_synthetic_rules() {
    let trailing = BlockRecordV4::semantic(0, 2, 0, vec![1], 0);
    assert!(matches!(
        trailing,
        Err(LineageValidationErrorV4::BlockOperationCoverageMismatch { .. })
    ));

    let invalid_synthetic =
        BlockRecordV4::synthetic(0, 2, SyntheticBlockRuleV4::RuntimeAssertFailureTrap);
    assert!(matches!(
        invalid_synthetic,
        Err(LineageValidationErrorV4::InvalidSyntheticBlockOperationCount { .. })
    ));
}

#[test]
fn model_rejects_duplicate_identities_declarations_and_kernel_splices() {
    let duplicate_blocks = LineageModelV4::new(
        semantic_identity(1, 1),
        kernel_ir_identity(2, 20),
        configuration(),
        vec![FunctionRecordV4::semantic_body(
            0,
            0,
            2,
            vec![
                semantic_block(0, 0, 0, &[], 0),
                semantic_block(0, 1, 0, &[], 0),
            ],
        )],
        vec![],
    );
    assert!(matches!(
        duplicate_blocks,
        Err(LineageValidationErrorV4::DuplicateOrdinal {
            context: "Kernel IR block",
            ..
        })
    ));

    let model = sample_model();
    let mut functions = model.functions().to_vec();
    functions.push(FunctionRecordV4::f32_intrinsic_declaration(
        3,
        F32IntrinsicV4::Sqrt,
    ));
    let duplicate_declaration = LineageModelV4::new(
        model.semantic_mir(),
        model.kernel_ir(),
        model.configuration(),
        functions,
        model.kernels().to_vec(),
    );
    assert!(matches!(
        duplicate_declaration,
        Err(LineageValidationErrorV4::DuplicateF32Declaration(
            F32IntrinsicV4::Sqrt
        ))
    ));
}

#[test]
fn synthetic_diagnostic_reference_requires_its_closed_declaration() {
    let model = sample_model();
    let mut functions = model.functions().to_vec();
    functions.remove(2);
    let missing_declaration = LineageModelV4::new(
        model.semantic_mir(),
        model.kernel_ir(),
        model.configuration(),
        functions,
        model.kernels().to_vec(),
    );
    assert!(matches!(
        missing_declaration,
        Err(LineageValidationErrorV4::DeclarationReferenceMismatch {
            context: "runtime-assert diagnostic trap",
            declaration_present: false,
            reference_present: true,
        })
    ));
}

#[test]
fn every_f32_declaration_classification_roundtrips() {
    let base = sample_model();
    let intrinsics = [
        F32IntrinsicV4::Sqrt,
        F32IntrinsicV4::FusedMultiplyAdd,
        F32IntrinsicV4::Floor,
        F32IntrinsicV4::Ceil,
        F32IntrinsicV4::Truncate,
        F32IntrinsicV4::RoundTiesEven,
        F32IntrinsicV4::Sin,
        F32IntrinsicV4::Cos,
        F32IntrinsicV4::Exp,
        F32IntrinsicV4::Exp2,
        F32IntrinsicV4::Ln,
        F32IntrinsicV4::Log2,
        F32IntrinsicV4::Log10,
    ];
    let mut functions = vec![base.functions()[0].clone()];
    for (index, intrinsic) in intrinsics.into_iter().enumerate() {
        functions.push(FunctionRecordV4::f32_intrinsic_declaration(
            u64::try_from(index + 1).unwrap(),
            intrinsic,
        ));
    }
    functions.push(FunctionRecordV4::diagnostic_trap_declaration(
        14,
        DiagnosticTrapKindV4::RuntimeAssertFailure,
    ));
    let model = LineageModelV4::new(
        base.semantic_mir(),
        base.kernel_ir(),
        base.configuration(),
        functions,
        base.kernels().to_vec(),
    )
    .unwrap();
    let decoded = InertCanonicalMirToKirLineageV4::from_model(
        model.clone(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    assert_eq!(decoded.model(), &model);
}

#[test]
fn canonical_artifact_lengths_are_nonzero_and_bounded() {
    assert!(matches!(
        CanonicalSemanticMirIdentityV4::new(
            SemanticMirCanonicalWireVersionV4::V3,
            identity(1, 0),
            0,
        ),
        Err(LineageValidationErrorV4::ZeroCanonicalLength { .. })
    ));
    assert!(matches!(
        CanonicalSemanticMirIdentityV4::new(
            SemanticMirCanonicalWireVersionV4::V3,
            identity(1, 1),
            MAX_CANONICAL_SEMANTIC_MIR_BYTES_V4 + 1,
        ),
        Err(LineageValidationErrorV4::CanonicalLengthLimitExceeded { .. })
    ));
    let maximum_kir = canonical_kernel_ir_v6_fixture(
        2,
        usize::try_from(MAX_CANONICAL_KERNEL_IR_BYTES_V4).unwrap(),
    );
    let maximum_identity =
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&maximum_kir).unwrap();
    assert_eq!(
        maximum_identity.canonical_length(),
        MAX_CANONICAL_KERNEL_IR_BYTES_V4
    );
    assert!(CanonicalKernelIrIdentityV4::new_v6(maximum_identity).is_ok());
    drop(maximum_kir);
    let too_large_kir = vec![0; usize::try_from(MAX_CANONICAL_KERNEL_IR_BYTES_V4 + 1).unwrap()];
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&too_large_kir),
        Err(KernelIrV6IdentityPreimageError::TooLarge { .. })
    ));
    assert_eq!(MAX_CANONICAL_KERNEL_IR_BYTES_V4, 16 * 1024 * 1024);
    assert_eq!(
        LineageDecodeLimitsV4::default().max_input_bytes(),
        DEFAULT_LINEAGE_INPUT_BYTES
    );
}

#[test]
fn exact_kernel_ir_v6_identity_scheme_has_frozen_vector_and_envelope_checks() {
    assert_eq!(
        VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1,
        b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V6\0"
    );
    assert_eq!(VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1, 1);
    let bytes = canonical_kernel_ir_v6_fixture(0xa5, 32);
    let identity = recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&bytes).unwrap();
    assert_eq!(identity.canonical_length(), 32);
    assert_eq!(
        identity.digest(),
        [
            0x5c, 0xab, 0xd3, 0x37, 0xff, 0xb4, 0x22, 0x44, 0x1a, 0x25, 0xbc, 0xd9, 0xca, 0x4e,
            0x4f, 0x03, 0xd6, 0xf3, 0x28, 0x16, 0xb2, 0x9a, 0x06, 0xd2, 0x9a, 0xdb, 0x4e, 0x47,
            0x15, 0xdf, 0xc4, 0x37,
        ]
    );
    let claim = CanonicalKernelIrIdentityV4::new_v6(identity).unwrap();
    assert_eq!(
        claim.scheme(),
        KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1
    );
    assert_eq!(claim.claimed_scheme_digest(), identity.digest());

    let mut mutation = bytes.clone();
    mutation[0] ^= 1;
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&mutation),
        Err(KernelIrV6IdentityPreimageError::InvalidMagic)
    ));
    mutation = bytes.clone();
    mutation[8..10].copy_from_slice(&5_u16.to_le_bytes());
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&mutation),
        Err(KernelIrV6IdentityPreimageError::NotVersion6 { actual: 5 })
    ));
    mutation = bytes.clone();
    mutation[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&mutation),
        Err(KernelIrV6IdentityPreimageError::UnsupportedFlags { actual: 1 })
    ));
    mutation = bytes.clone();
    mutation[12..16].copy_from_slice(&31_u32.to_le_bytes());
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&mutation),
        Err(KernelIrV6IdentityPreimageError::DeclaredLengthMismatch { .. })
    ));
    mutation = bytes;
    mutation[16..20].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1(&mutation),
        Err(KernelIrV6IdentityPreimageError::NonzeroReserved { actual: 1 })
    ));
}

#[test]
fn checked_arithmetic_policy_vector_is_frozen_but_grants_no_gate() {
    let actual = CHECKED_ARITHMETIC_REFINEMENT_POLICY_VECTOR_V4
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "4645324f334341310100030006000003000102050800100020004000800002000100400002000100000000"
    );
    assert_eq!(
        CHECKED_ARITHMETIC_EXTERNAL_OWNER_GATE_V4,
        "semantic-mir-v3-kernel-ir-v6-checked-arithmetic-owner-gate-v1"
    );
}

#[test]
fn kernel_mapping_to_a_declaration_is_rejected() {
    let model = sample_model();
    let result = LineageModelV4::new(
        model.semantic_mir(),
        model.kernel_ir(),
        model.configuration(),
        model.functions().to_vec(),
        vec![KernelRecordV4::new(0, 0, 1)],
    );
    assert!(matches!(
        result,
        Err(LineageValidationErrorV4::KernelFunctionMismatch { .. })
    ));
}

#[test]
fn represented_lowering_limits_are_enforced() {
    let limits = LoweringResourceLimitsV4::new(1, 2, 1, 3, 2, 5, 32).unwrap();
    let config =
        LoweringConfigurationV4::new(RankedBoundsPolicyV4::RetainGenericChecks, limits).unwrap();
    let model = sample_model();
    let result = LineageModelV4::new(
        model.semantic_mir(),
        model.kernel_ir(),
        config,
        model.functions().to_vec(),
        model.kernels().to_vec(),
    );
    assert!(matches!(
        result,
        Err(LineageValidationErrorV4::LimitExceeded {
            resource: LineageResourceV4::KirFunctions,
            actual: 3,
            limit: 2,
        })
    ));
}

#[test]
fn maximum_zero_operation_statement_set_has_proven_sub_4_mib_encoding() {
    let empty = InertCanonicalMirToKirLineageV4::from_model(
        zero_operation_statement_model(0),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    let maximum = InertCanonicalMirToKirLineageV4::from_model(
        zero_operation_statement_model(MAX_STATEMENTS),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();

    // Each zero-op statement is one byte. The statement-count field and the
    // aggregate statement total each grow from one byte to three bytes.
    let expected = empty.canonical_bytes().len() + MAX_STATEMENTS + 4;
    assert_eq!(maximum.canonical_bytes().len(), expected);
    assert!(u64::try_from(expected).unwrap() <= LineageDecodeLimitsV4::default().max_input_bytes());

    let decoded = InertCanonicalMirToKirLineageV4::decode_canonical(
        maximum.canonical_bytes(),
        LineageDecodeLimitsV4::default(),
    )
    .unwrap();
    assert_eq!(decoded.model().totals().statements(), 1_048_576);
    drop(decoded);
    drop(maximum);

    let over_limit = LineageModelV4::new(
        semantic_identity(0x71, 1),
        kernel_ir_identity(0x81, 20),
        configuration(),
        vec![FunctionRecordV4::semantic_body(
            0,
            0,
            1,
            vec![BlockRecordV4::semantic(0, 0, 0, vec![0; MAX_STATEMENTS + 1], 0).unwrap()],
        )],
        vec![],
    );
    assert!(matches!(
        over_limit,
        Err(LineageValidationErrorV4::LimitExceeded {
            resource: LineageResourceV4::Statements,
            actual: 1_048_577,
            limit: 1_048_576,
        })
    ));
}

#[test]
fn arbitrary_single_bit_mutations_are_panic_total_and_any_acceptance_is_exact() {
    let canonical = sample_canonical();
    for index in 0..canonical.len() {
        for bit in 0..8 {
            let mut mutated = canonical.clone();
            mutated[index] ^= 1 << bit;
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                InertCanonicalMirToKirLineageV4::decode_canonical(
                    &mutated,
                    LineageDecodeLimitsV4::default(),
                )
            }));
            let decoded =
                outcome.unwrap_or_else(|_| panic!("decoder panicked for byte {index}, bit {bit}"));
            if let Ok(decoded) = decoded {
                assert_eq!(decoded.canonical_bytes(), mutated);
                decoded
                    .revalidate(LineageDecodeLimitsV4::default())
                    .unwrap();
            }
        }
    }
}

#[test]
fn canonical_sample_has_frozen_golden_encoding() {
    const EXPECTED: &str = "4645324f334c340004000003111111111111111111111111111111111111111111111111111111111111116a7b0106a682a693511b5d91a3cf567972573f62d6cf8e26ff468f7c054d4e7de49fd3dec803020000000000000080088010800880800180804080808002808080080103010201030205000000020300030000020002010201010001010001010100010100020200000000";
    let actual = sample_canonical()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, EXPECTED);
}
