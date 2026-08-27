use fe2o3_kernel_analysis::{
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1,
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectAnalysisBasisV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    PhysicalMachineEffectEvidenceErrorV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectKindV1, PhysicalMachineEffectRequestErrorV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1, PhysicalMachineTargetV1,
    PhysicalMachineToolchainIdentityV1,
};

const CODE_OFFSET: u64 = 4;
const CODE_SIZE: u64 = 16;

#[derive(Clone)]
struct Function<'a> {
    symbol: &'a str,
    offset: u64,
    size: u64,
    callees: Vec<&'a str>,
}

#[derive(Clone)]
struct Effect<'a> {
    entry: &'a str,
    function: &'a str,
    offset: u64,
    kind: u8,
    width: u16,
}

fn budget() -> PhysicalMachineEffectBudgetV1 {
    PhysicalMachineEffectBudgetV1::new(8, 4, 4, 2, 2)
}

fn entry(
    symbol: &str,
    budget: PhysicalMachineEffectBudgetV1,
) -> PhysicalMachineEffectEntryRequestV1 {
    PhysicalMachineEffectEntryRequestV1::new(symbol, budget).unwrap()
}

fn request_with(
    payload: &[u8],
    entries: Vec<PhysicalMachineEffectEntryRequestV1>,
) -> PhysicalMachineEffectRequestV1 {
    PhysicalMachineEffectRequestV1::new(
        PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0x10; 32]),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0x11; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
        payload.to_vec(),
        entries,
    )
    .unwrap()
}

fn request() -> PhysicalMachineEffectRequestV1 {
    request_with(
        b"exact finalized gfx942 hsaco",
        vec![entry("arbitrary_entry", budget())],
    )
}

fn effects() -> Vec<Effect<'static>> {
    vec![
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET,
            kind: 2,
            width: 4,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 4,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 4,
            kind: 3,
            width: 4,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 8,
            kind: 4,
            width: 0,
        },
    ]
}

fn evidence(
    request: &PhysicalMachineEffectRequestV1,
    functions: &[Function<'_>],
    effects: &[Effect<'_>],
) -> Vec<u8> {
    evidence_with_entry_range(request, CODE_OFFSET, CODE_SIZE, functions, effects)
}

fn evidence_with_entry_range(
    request: &PhysicalMachineEffectRequestV1,
    entry_offset: u64,
    entry_size: u64,
    functions: &[Function<'_>],
    effects: &[Effect<'_>],
) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(&mut output, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
    output.extend_from_slice(&request.execution_challenge().as_bytes());
    output.extend_from_slice(&request.identity().sha256());
    push_u64(&mut output, request.identity().byte_len());
    output.extend_from_slice(&request.payload_identity().sha256());
    push_u64(&mut output, request.payload_identity().byte_len());
    output.extend_from_slice(&request.analyzer_identity().as_bytes());
    output.extend_from_slice(&request.toolchain_identity().as_bytes());
    push_u16(&mut output, 1);

    push_u16(&mut output, request.entries().len() as u16);
    for entry in request.entries() {
        push_text(&mut output, entry.symbol());
        output.extend_from_slice(&[0x33; 32]);
        push_u64(&mut output, entry_offset);
        push_u64(&mut output, entry_size);
    }

    push_u32(&mut output, functions.len() as u32);
    for function in functions {
        push_text(&mut output, function.symbol);
        push_u64(&mut output, function.offset);
        push_u64(&mut output, function.size);
        push_u16(&mut output, function.callees.len() as u16);
        for callee in &function.callees {
            push_text(&mut output, callee);
        }
    }

    push_u32(&mut output, effects.len() as u32);
    for effect in effects {
        push_text(&mut output, effect.entry);
        push_text(&mut output, effect.function);
        push_u64(&mut output, effect.offset);
        output.push(effect.kind);
        push_u16(&mut output, effect.width);
    }

    let length = output.len() as u32;
    let offset = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn entry_function() -> Function<'static> {
    Function {
        symbol: "arbitrary_entry",
        offset: CODE_OFFSET,
        size: CODE_SIZE,
        callees: Vec::new(),
    }
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_u16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}

#[test]
fn canonical_record_binds_exact_payload_worker_target_graph_and_effects() {
    let request = request();
    let bytes = evidence(&request, &[entry_function()], &effects());
    let decoded = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes).unwrap();

    assert_eq!(decoded.schema_version(), 1);
    assert_eq!(decoded.execution_challenge(), request.execution_challenge());
    assert_eq!(
        decoded.target(),
        PhysicalMachineTargetV1::Gfx942XnackMinusCov6
    );
    assert_eq!(
        decoded.analysis_basis(),
        PhysicalMachineEffectAnalysisBasisV1::FinalizedHsacoViaMeasuredLlvmObjectMc
    );
    assert_eq!(decoded.request_identity(), request.identity());
    assert_eq!(decoded.payload_identity(), request.payload_identity());
    assert_eq!(decoded.entry_points()[0].symbol(), "arbitrary_entry");
    assert_eq!(
        decoded.entry_points()[0].descriptor_identity().as_bytes(),
        [0x33; 32]
    );
    assert_eq!(decoded.entry_points()[0].code_offset(), CODE_OFFSET);
    assert_eq!(decoded.entry_points()[0].code_size(), CODE_SIZE);
    assert_eq!(decoded.functions()[0].symbol(), "arbitrary_entry");
    assert!(decoded.functions()[0].direct_callees().is_empty());
    assert_eq!(
        decoded.effects()[1].kind(),
        PhysicalMachineEffectKindV1::GlobalRead
    );
    assert_eq!(decoded.effects()[1].byte_width(), 4);
    assert!(decoded.is_derived_from_exact_payload());
    assert!(!decoded.authenticates_analyzer());
    assert!(!decoded.establishes_compiler_refinement());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(decoded.identity().byte_len(), bytes.len() as u64);
}

#[test]
fn request_and_evidence_are_deterministic_golden_records() {
    let first = request_with(
        b"golden-payload",
        vec![
            entry("_secondary.entry$1", budget()),
            entry("arbitrary_entry", budget()),
        ],
    );
    let second = request_with(
        b"golden-payload",
        vec![
            entry("arbitrary_entry", budget()),
            entry("_secondary.entry$1", budget()),
        ],
    );
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());

    let bytes = evidence(&request(), &[entry_function()], &effects());
    let first = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    let second = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.identity().sha256(),
        [
            0x0d, 0x67, 0x6d, 0x09, 0x8f, 0xb7, 0x97, 0xf3, 0xf3, 0xd2, 0x8e, 0x6c, 0x73, 0xd9,
            0x15, 0x8d, 0xcf, 0x89, 0x3d, 0xe5, 0x6a, 0x12, 0xae, 0xf8, 0x1f, 0x58, 0xb2, 0x68,
            0xc6, 0xe8, 0xd1, 0xc4,
        ]
    );
}

#[test]
fn payload_mutation_cannot_reuse_evidence() {
    let original = request();
    let bytes = evidence(&original, &[entry_function()], &effects());
    let mutated = request_with(
        b"exact finalized gfx942 hsacp",
        vec![entry("arbitrary_entry", budget())],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&mutated, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch)
    );
}

#[test]
fn symbol_and_identity_substitution_fail_closed() {
    let request = request();
    let mut bytes = evidence(&request, &[entry_function()], &effects());
    let entry_offset = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len()
        + 4
        + 2
        + 32
        + 32
        + 8
        + 32
        + 8
        + 32
        + 32
        + 2
        + 2
        + 2;
    bytes[entry_offset] = b'z';
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::EntrySetMismatch)
    );

    let mut bytes = evidence(&request, &[entry_function()], &effects());
    let analyzer_offset =
        PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() + 4 + 2 + 32 + 32 + 8 + 32 + 8;
    bytes[analyzer_offset] ^= 1;
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::AnalyzerIdentityMismatch)
    );
}

#[test]
fn open_call_edge_and_effect_expansion_fail_closed() {
    let request = request();
    let open = Function {
        symbol: "arbitrary_entry",
        offset: CODE_OFFSET,
        size: CODE_SIZE,
        callees: vec!["missing_helper"],
    };
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(&request, &[open], &effects())
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::OpenCallGraph)
    );

    let tight = request_with(
        b"exact finalized gfx942 hsaco",
        vec![entry(
            "arbitrary_entry",
            PhysicalMachineEffectBudgetV1::new(8, 0, 4, 2, 2),
        )],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &tight,
            &evidence(&tight, &[entry_function()], &effects())
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion)
    );
}

#[test]
fn code_ranges_may_end_exactly_at_the_payload_boundary() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);
    let function = Function {
        symbol: "arbitrary_entry",
        offset: 16,
        size: 16,
        callees: Vec::new(),
    };
    let effects = [
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: 16,
            kind: 4,
            width: 0,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: 31,
            kind: 4,
            width: 0,
        },
    ];
    let bytes = evidence_with_entry_range(&request, 16, 16, &[function], &effects);

    let decoded = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes).unwrap();
    assert_eq!(decoded.entry_points()[0].code_offset(), 16);
    assert_eq!(decoded.entry_points()[0].code_size(), 16);
    assert_eq!(decoded.effects()[0].instruction_offset(), 16);
    assert_eq!(decoded.effects()[1].instruction_offset(), 31);
}

#[test]
fn invalid_entry_ranges_fail_closed_without_panicking() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);

    for (offset, size) in [(32, 1), (31, 2), (u64::MAX, 1), (0, 0)] {
        let bytes =
            evidence_with_entry_range(&request, offset, size, &[entry_function()], &effects());
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "entry range ({offset}, {size}) panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange),
            "entry range ({offset}, {size}) was not rejected"
        );
    }
}

#[test]
fn invalid_function_ranges_fail_closed_without_panicking() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);

    for (offset, size) in [(32, 1), (31, 2), (u64::MAX, 1), (0, 0)] {
        let function = Function {
            symbol: "arbitrary_entry",
            offset,
            size,
            callees: Vec::new(),
        };
        let bytes = evidence_with_entry_range(&request, 16, 16, &[function], &[]);
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "function range ({offset}, {size}) panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange),
            "function range ({offset}, {size}) was not rejected"
        );
    }
}

#[test]
fn instruction_offsets_outside_the_function_fail_closed() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);
    let function = Function {
        symbol: "arbitrary_entry",
        offset: 16,
        size: 16,
        callees: Vec::new(),
    };

    for offset in [15, 32, u64::MAX] {
        let effects = [Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset,
            kind: 4,
            width: 0,
        }];
        let bytes =
            evidence_with_entry_range(&request, 16, 16, std::slice::from_ref(&function), &effects);
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "instruction offset {offset} panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::EffectOutsideClosure),
            "instruction offset {offset} was not rejected"
        );
    }
}

#[test]
fn entry_symbols_are_workload_neutral_and_bounded() {
    for valid in [
        "other",
        "_private_entry7",
        ".hidden.entry",
        "$generated$entry",
    ] {
        assert_eq!(
            PhysicalMachineEffectEntryRequestV1::new(valid, budget())
                .unwrap()
                .symbol(),
            valid
        );
    }

    let too_long = "k".repeat(257);
    for invalid in [
        "",
        "7leading_digit",
        "contains/slash",
        "non_ascii_\u{e9}",
        &too_long,
    ] {
        assert_eq!(
            PhysicalMachineEffectEntryRequestV1::new(invalid, budget()),
            Err(PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol {
                byte_len: invalid.len(),
            })
        );
    }
}

#[test]
fn request_rejects_reserved_identities() {
    assert_eq!(
        PhysicalMachineEffectRequestV1::new(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([1; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([2; 32]),
            vec![1],
            vec![entry("arbitrary_entry", budget())],
        ),
        Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
            "analyzer"
        ))
    );
    assert_eq!(
        PhysicalMachineEffectRequestV1::new(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([1; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([2; 32]),
            vec![1],
            vec![entry("arbitrary_entry", budget())],
        ),
        Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
            "execution challenge"
        ))
    );
}
