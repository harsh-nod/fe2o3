use fe2o3_kernel_analysis::{
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1,
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectAnalysisBasisV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    PhysicalMachineEffectEvidenceErrorV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectKindV1, PhysicalMachineEffectRequestErrorV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1, PhysicalMachineTargetV1,
    PhysicalMachineToolchainIdentityV1,
};

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
        vec![entry("alpha", budget())],
    )
}

fn effects() -> Vec<Effect<'static>> {
    vec![
        Effect {
            entry: "alpha",
            function: "alpha",
            offset: 0x100,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "alpha",
            function: "alpha",
            offset: 0x100,
            kind: 2,
            width: 4,
        },
        Effect {
            entry: "alpha",
            function: "alpha",
            offset: 0x104,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "alpha",
            function: "alpha",
            offset: 0x104,
            kind: 3,
            width: 4,
        },
        Effect {
            entry: "alpha",
            function: "alpha",
            offset: 0x108,
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
        push_u64(&mut output, 0x100);
        push_u64(&mut output, 0x40);
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

fn alpha_function() -> Function<'static> {
    Function {
        symbol: "alpha",
        offset: 0x100,
        size: 0x40,
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
    let bytes = evidence(&request, &[alpha_function()], &effects());
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
    assert_eq!(decoded.entry_points()[0].symbol(), "alpha");
    assert_eq!(
        decoded.entry_points()[0].descriptor_identity().as_bytes(),
        [0x33; 32]
    );
    assert_eq!(decoded.entry_points()[0].code_offset(), 0x100);
    assert_eq!(decoded.entry_points()[0].code_size(), 0x40);
    assert_eq!(decoded.functions()[0].symbol(), "alpha");
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
        vec![entry("zeta", budget()), entry("alpha", budget())],
    );
    let second = request_with(
        b"golden-payload",
        vec![entry("alpha", budget()), entry("zeta", budget())],
    );
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());

    let bytes = evidence(&request(), &[alpha_function()], &effects());
    let first = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    let second = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.identity().sha256(),
        [
            0xea, 0xf2, 0xb7, 0xa7, 0x4f, 0xfb, 0xe2, 0x77, 0x0e, 0x1d, 0x0a, 0x96, 0xa9, 0xe9,
            0x39, 0xc1, 0x04, 0x23, 0x8d, 0xf6, 0xbe, 0x72, 0xd0, 0x30, 0x79, 0x90, 0xbf, 0xfb,
            0x63, 0xa1, 0xdb, 0xd9,
        ]
    );
}

#[test]
fn payload_mutation_cannot_reuse_evidence() {
    let original = request();
    let bytes = evidence(&original, &[alpha_function()], &effects());
    let mutated = request_with(
        b"exact finalized gfx942 hsacp",
        vec![entry("alpha", budget())],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&mutated, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch)
    );
}

#[test]
fn symbol_and_identity_substitution_fail_closed() {
    let request = request();
    let mut bytes = evidence(&request, &[alpha_function()], &effects());
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

    let mut bytes = evidence(&request, &[alpha_function()], &effects());
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
        symbol: "alpha",
        offset: 0x100,
        size: 0x40,
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
            "alpha",
            PhysicalMachineEffectBudgetV1::new(8, 0, 4, 2, 2),
        )],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &tight,
            &evidence(&tight, &[alpha_function()], &effects())
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion)
    );
}

#[test]
fn slice_rejects_unprofiled_entries_and_reserved_identities() {
    assert_eq!(
        PhysicalMachineEffectEntryRequestV1::new("other", budget()),
        Err(PhysicalMachineEffectRequestErrorV1::UnsupportedEntry(
            "other".to_string()
        ))
    );
    assert_eq!(
        PhysicalMachineEffectRequestV1::new(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([1; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([2; 32]),
            vec![1],
            vec![entry("alpha", budget())],
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
            vec![entry("alpha", budget())],
        ),
        Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
            "execution challenge"
        ))
    );
}
