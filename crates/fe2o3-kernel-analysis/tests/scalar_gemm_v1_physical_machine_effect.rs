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

#[derive(Clone)]
struct Function<'a> {
    symbol: &'a str,
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
    b"finalized scalar GEMM V1 gfx942 HSACO fixture".to_vec()
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
        callees: Vec::new(),
    }
}

fn scalar_effects(extra_write: bool) -> Vec<Effect<'static>> {
    let mut effects = Vec::new();
    for (site, width, kind) in [
        (0, 8, 2),
        (4, 8, 2),
        (8, 8, 2),
        (12, 4, 2),
        (16, 4, 2),
        (20, 4, 2),
        (24, 4, 2),
        (28, 4, 2),
        (32, 4, 3),
    ] {
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x100 + site,
            kind: 1,
            width: 8,
        });
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x100 + site,
            kind,
            width,
        });
    }
    let return_offset = if extra_write {
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x124,
            kind: 1,
            width: 8,
        });
        effects.push(Effect {
            entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
            offset: 0x124,
            kind: 3,
            width: 4,
        });
        0x128
    } else {
        0x124
    };
    effects.push(Effect {
        entry: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        function: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
        offset: return_offset,
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
        push_u64(&mut output, 0x100 + index as u64 * 0x100);
        push_u64(&mut output, 0x40);
    }

    push_u32(&mut output, functions.len() as u32);
    for function in functions {
        push_text(&mut output, function.symbol);
        push_u64(
            &mut output,
            if function.symbol == SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL {
                0x100
            } else {
                0x180
            },
        );
        push_u64(&mut output, 0x40);
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
        callees: Vec::new(),
    };
    let calling = Function {
        symbol: SCALAR_GEMM_V1_PHYSICAL_ENTRY_SYMBOL,
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

#[cfg(target_os = "linux")]
#[test]
fn authenticated_worker_result_is_profile_bound_and_inert() {
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
    let authenticated = profile(hsaco(), descriptor(0x33))
        .analyze(&worker, limits)
        .unwrap();

    assert!(authenticated.authenticates_analyzer_execution());
    assert!(!authenticated.establishes_compiler_refinement());
    assert!(!authenticated.establishes_logical_buffer_address_refinement());
    assert!(!authenticated.establishes_memory_safety());
    assert!(!authenticated.establishes_out_of_bounds_absence());
    assert!(!authenticated.establishes_race_freedom());
    assert!(!authenticated.grants_publication_authority());
    assert!(!authenticated.grants_load_authority());
    assert!(!authenticated.grants_launch_authority());
    assert_eq!(authenticated.descriptor_identity(), descriptor(0x33));
    assert_eq!(authenticated.evidence().entry_points().len(), 1);
}
