use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1,
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineDescriptorIdentityV1,
    PhysicalMachineEffectEvidenceErrorV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1,
    PhysicalMachinePayloadIdentityV1, PhysicalMachineToolchainIdentityV1,
    SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET, SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
    ScalarGemmV1PhysicalMachineEffectErrorV1, ScalarGemmV1PhysicalMachineEffectProfileV1,
    inspect_physical_machine_effect_worker_candidate_v1,
};
use std::{path::Path, time::Duration};

const SCALAR_CODE_OFFSET: u64 = 0x1b00;
const SCALAR_CODE_SIZE: u64 = 0x0ad0;
const HSACO_FIXTURE_BYTES: usize = 0x2640;

#[derive(Clone)]
struct Function<'a> {
    symbol: &'a str,
    code_offset: u64,
    code_size: u64,
    callees: Vec<&'a str>,
}

#[derive(Clone, Copy)]
struct Effect<'a> {
    entry: &'a str,
    function: &'a str,
    offset: u64,
    kind: u8,
    width: u16,
}

fn hsaco() -> Vec<u8> {
    let label = b"finalized scalar GEMM V1 gfx942 HSACO fixture";
    let mut payload = vec![0; HSACO_FIXTURE_BYTES];
    payload[..label.len()].copy_from_slice(label);
    payload
}

fn descriptor(byte: u8) -> PhysicalMachineDescriptorIdentityV1 {
    PhysicalMachineDescriptorIdentityV1::from_sha256_bytes([byte; 32])
}

fn profile(
    payload: Vec<u8>,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
) -> ScalarGemmV1PhysicalMachineEffectProfileV1 {
    ScalarGemmV1PhysicalMachineEffectProfileV1::new(
        PhysicalMachinePayloadIdentityV1::calculate(&payload),
        payload,
        descriptor_identity,
    )
    .unwrap()
}

fn request(profile: &ScalarGemmV1PhysicalMachineEffectProfileV1) -> PhysicalMachineEffectRequestV1 {
    profile
        .request_v1(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0x10; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0x11; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
        )
        .unwrap()
}

fn scalar_function() -> Function<'static> {
    Function {
        symbol: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        code_offset: SCALAR_CODE_OFFSET,
        code_size: SCALAR_CODE_SIZE,
        callees: Vec::new(),
    }
}

fn scalar_effects(extra_write: bool) -> Vec<Effect<'static>> {
    let mut effects = Vec::new();
    for (offset, width, kind) in [
        (0x1b0c, 8, 2),
        (0x1b14, 8, 2),
        (0x1b1c, 8, 2),
        (0x1b24, 4, 2),
        (0x1b2c, 4, 2),
        (0x1b34, 4, 2),
        (0x2490, 4, 2),
        (0x24a4, 4, 2),
        (0x25c0, 4, 3),
    ] {
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset,
            kind: 1,
            width: 8,
        });
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset,
            kind,
            width,
        });
    }
    if extra_write {
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x25c4,
            kind: 1,
            width: 8,
        });
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x25c4,
            kind: 3,
            width: 4,
        });
    }
    effects.push(Effect {
        entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        offset: 0x25cc,
        kind: 4,
        width: 0,
    });
    effects
}

fn evidence(
    request: &PhysicalMachineEffectRequestV1,
    target: u16,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    functions: &[Function<'_>],
    effects: &[Effect<'_>],
) -> Vec<u8> {
    evidence_with_entry_range(
        request,
        target,
        descriptor_identity,
        (SCALAR_CODE_OFFSET, SCALAR_CODE_SIZE),
        functions,
        effects,
    )
}

fn evidence_with_entry_range(
    request: &PhysicalMachineEffectRequestV1,
    target: u16,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    entry_range: (u64, u64),
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
    push_u16(&mut output, target);

    push_u16(&mut output, request.entries().len() as u16);
    for (index, entry) in request.entries().iter().enumerate() {
        push_text(&mut output, entry.symbol());
        output.extend_from_slice(&descriptor_identity.as_bytes());
        assert_eq!(index, 0);
        push_u64(&mut output, entry_range.0);
        push_u64(&mut output, entry_range.1);
    }

    push_u32(&mut output, functions.len() as u32);
    for function in functions {
        push_text(&mut output, function.symbol);
        push_u64(&mut output, function.code_offset);
        push_u64(&mut output, function.code_size);
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
fn derives_exact_single_entry_request_and_effect_budget() {
    let bytes = hsaco();
    let identity = PhysicalMachinePayloadIdentityV1::calculate(&bytes);
    let profile =
        ScalarGemmV1PhysicalMachineEffectProfileV1::new(identity, bytes.clone(), descriptor(0x33))
            .unwrap();
    let request = request(&profile);

    assert_eq!(profile.finalized_hsaco_identity(), identity);
    assert!(identity.matches(&bytes));
    assert_eq!(profile.exact_finalized_hsaco(), bytes);
    assert_eq!(request.payload_identity(), identity);
    assert_eq!(request.exact_payload_bytes(), bytes);
    assert_eq!(request.entries().len(), 1);
    assert_eq!(
        request.entries()[0].symbol(),
        SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL
    );
    assert_eq!(
        request.entries()[0].budget(),
        SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET
    );
    assert_eq!(
        SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_addresses(),
        9
    );
    assert_eq!(SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_reads(), 8);
    assert_eq!(SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_global_writes(), 1);
    assert_eq!(SCALAR_GEMM_V1_PHYSICAL_EFFECT_BUDGET.max_direct_calls(), 0);
}

#[test]
fn exact_profile_evidence_is_accepted_but_remains_non_authoritative() {
    let profile = profile(hsaco(), descriptor(0x33));
    let request = request(&profile);
    let evidence = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
        &request,
        &evidence(
            &request,
            1,
            descriptor(0x33),
            &[scalar_function()],
            &scalar_effects(false),
        ),
    )
    .unwrap();
    profile.validate_evidence(&evidence).unwrap();
    assert!(!evidence.establishes_compiler_refinement());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
}

#[test]
fn payload_descriptor_and_target_substitution_fail_closed() {
    let original = profile(hsaco(), descriptor(0x33));
    let request = request(&original);
    let exact = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
        &request,
        &evidence(
            &request,
            1,
            descriptor(0x33),
            &[scalar_function()],
            &scalar_effects(false),
        ),
    )
    .unwrap();

    let mut changed_payload = hsaco();
    changed_payload[0] ^= 1;
    assert!(matches!(
        profile(changed_payload, descriptor(0x33)).validate_evidence(&exact),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::PayloadSubstitution)
    ));
    assert!(matches!(
        profile(hsaco(), descriptor(0x44)).validate_evidence(&exact),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::DescriptorSubstitution)
    ));
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(
                &request,
                2,
                descriptor(0x33),
                &[scalar_function()],
                &scalar_effects(false),
            ),
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::TargetMismatch)
    );

    let mut changed = hsaco();
    changed.push(0);
    assert!(matches!(
        ScalarGemmV1PhysicalMachineEffectProfileV1::new(
            original.finalized_hsaco_identity(),
            changed,
            descriptor(0x33),
        ),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::FinalizedHsacoIdentityMismatch { .. })
    ));
}

#[test]
fn extra_entry_write_expansion_and_open_calls_fail_closed() {
    let profile = profile(hsaco(), descriptor(0x33));
    let request = request(&profile);

    let expanded = evidence(
        &request,
        1,
        descriptor(0x33),
        &[scalar_function()],
        &scalar_effects(true),
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &expanded),
        Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion)
    );

    let open = Function {
        symbol: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        code_offset: SCALAR_CODE_OFFSET,
        code_size: SCALAR_CODE_SIZE,
        callees: vec!["missing"],
    };
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(
                &request,
                1,
                descriptor(0x33),
                &[open],
                &scalar_effects(false),
            ),
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::OpenCallGraph)
    );

    let helper = Function {
        symbol: "helper",
        code_offset: 0x2600,
        code_size: 0x40,
        callees: Vec::new(),
    };
    let calling = Function {
        symbol: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        code_offset: SCALAR_CODE_OFFSET,
        code_size: SCALAR_CODE_SIZE,
        callees: vec!["helper"],
    };
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(
                &request,
                1,
                descriptor(0x33),
                &[helper, calling],
                &scalar_effects(false),
            ),
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion)
    );

    let mut extra_entry = evidence(
        &request,
        1,
        descriptor(0x33),
        &[scalar_function()],
        &scalar_effects(false),
    );
    let entry_count_offset = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len()
        + 4
        + 2
        + 32
        + 32
        + 8
        + 32
        + 8
        + 32
        + 32
        + 2;
    extra_entry[entry_count_offset..entry_count_offset + 2].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &extra_entry),
        Err(PhysicalMachineEffectEvidenceErrorV1::EntrySetMismatch)
    );
}

#[test]
fn underreported_effects_and_zero_descriptor_identity_fail_closed() {
    let profile = profile(hsaco(), descriptor(0x33));
    let request = request(&profile);
    let mut effects = scalar_effects(false);
    effects.remove(1);
    let underreported = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
        &request,
        &evidence(
            &request,
            1,
            descriptor(0x33),
            &[scalar_function()],
            &effects,
        ),
    )
    .unwrap();
    assert!(matches!(
        profile.validate_evidence(&underreported),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EffectSet)
    ));

    let bytes = hsaco();
    assert!(matches!(
        ScalarGemmV1PhysicalMachineEffectProfileV1::new(
            PhysicalMachinePayloadIdentityV1::calculate(&bytes),
            bytes,
            descriptor(0),
        ),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::ZeroDescriptorIdentity)
    ));
}

#[test]
fn same_budget_site_relocations_and_broken_pairs_fail_closed() {
    let profile = profile(hsaco(), descriptor(0x33));
    let request = request(&profile);

    let assert_effect_set_rejected = |effects: Vec<Effect<'static>>| {
        let evidence = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(
                &request,
                1,
                descriptor(0x33),
                &[scalar_function()],
                &effects,
            ),
        )
        .unwrap();
        assert!(matches!(
            profile.validate_evidence(&evidence),
            Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EffectSet)
        ));
    };

    let mut relocated_pair = scalar_effects(false);
    relocated_pair[0].offset = 0x1b10;
    relocated_pair[1].offset = 0x1b10;
    assert_effect_set_rejected(relocated_pair);

    let mut swapped_pair_offsets = scalar_effects(false);
    swapped_pair_offsets[0].offset = 0x1b14;
    swapped_pair_offsets[1].offset = 0x1b14;
    swapped_pair_offsets[2].offset = 0x1b0c;
    swapped_pair_offsets[3].offset = 0x1b0c;
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(
                &request,
                1,
                descriptor(0x33),
                &[scalar_function()],
                &swapped_pair_offsets,
            ),
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::NonCanonicalOrder)
    );

    let mut broken_pair = scalar_effects(false);
    broken_pair[1].offset = 0x1b10;
    assert_effect_set_rejected(broken_pair);

    let mut swapped_read_widths = scalar_effects(false);
    swapped_read_widths[1].width = 4;
    swapped_read_widths[7].width = 8;
    assert_effect_set_rejected(swapped_read_widths);
}

#[test]
fn entry_and_function_range_substitutions_fail_closed() {
    let profile = profile(hsaco(), descriptor(0x33));
    let request = request(&profile);

    let changed_entry = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
        &request,
        &evidence_with_entry_range(
            &request,
            1,
            descriptor(0x33),
            (SCALAR_CODE_OFFSET - 4, SCALAR_CODE_SIZE + 4),
            &[scalar_function()],
            &scalar_effects(false),
        ),
    )
    .unwrap();
    assert!(matches!(
        profile.validate_evidence(&changed_entry),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EntryRange)
    ));

    let changed_function = Function {
        symbol: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        code_offset: SCALAR_CODE_OFFSET,
        code_size: SCALAR_CODE_SIZE + 4,
        callees: Vec::new(),
    };
    let changed_function = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
        &request,
        &evidence(
            &request,
            1,
            descriptor(0x33),
            &[changed_function],
            &scalar_effects(false),
        ),
    )
    .unwrap();
    assert!(matches!(
        profile.validate_evidence(&changed_function),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::FunctionRange)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn authenticated_fixture_with_nonproduction_layout_is_rejected() {
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(30),
        1024 * 1024,
        16 * 1024,
    )
    .unwrap();
    let fixture = Path::new(env!("CARGO_BIN_EXE_fe2o3-machine-effect-worker-fixture"));
    let candidate = inspect_physical_machine_effect_worker_candidate_v1(fixture, limits).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture, candidate.policy(), limits)
            .unwrap();
    assert!(matches!(
        profile(hsaco(), descriptor(0x33)).analyze(&worker, limits),
        Err(ScalarGemmV1PhysicalMachineEffectErrorV1::EntryRange)
    ));
}
