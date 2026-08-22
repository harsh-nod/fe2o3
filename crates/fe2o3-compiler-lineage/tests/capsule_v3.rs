use std::ffi::OsString;

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::DeviceTargetV1;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV2,
};

const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const TARGET: &str = "gfx942:sramecc+:xnack-";

fn os_entries(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    entries
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

fn invocation(seed: u8) -> RustcInvocationDescriptorV3 {
    let pins = [
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
        [seed.wrapping_add(6); 32],
    ];
    let closure = CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5])
        .expect("nonzero fixture closure");
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "lineage_fixture".into(),
            "crates/lineage-fixture/src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .expect("valid rustc unit");
    let environment = CompileEnvironmentV2::from_child_environment(os_entries(&[
        ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
        ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
        ("FE2O3_TARGET", TARGET),
        ("FE2O3_VERIFY_KERNEL_IR", "1"),
    ]))
    .expect("valid exact environment");
    let v2 = RustcInvocationDescriptorV2::new(pins[3], pins[5], rustc, environment)
        .expect("valid V2 invocation");
    RustcInvocationDescriptorV3::new(v2, closure).expect("matching closure")
}

fn payload(label: &str, seed: u8) -> Vec<u8> {
    format!("fe2o3-lineage/{label}/seed-{seed:03}").into_bytes()
}

fn receipts(seed: u8) -> OrderedInertSemanticLineageReceiptsV3 {
    receipts_with_stage_seeds(seed, seed, seed, seed)
}

fn receipts_with_stage_seeds(
    base_seed: u8,
    semantic_mir_seed: u8,
    kernel_ir_seed: u8,
    final_compiler_module_commitment_seed: u8,
) -> OrderedInertSemanticLineageReceiptsV3 {
    OrderedInertSemanticLineageReceiptsV3::new(
        InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(payload(
            "inventory",
            base_seed,
        ))
        .unwrap(),
        InertRustcPreflightPlanReceiptV3::from_canonical_preimage(payload("preflight", base_seed))
            .unwrap(),
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(payload(
            "semantic-mir",
            semantic_mir_seed,
        ))
        .unwrap(),
        InertMiddleEndReceiptV3::from_canonical_preimage(payload("middle-end", base_seed)).unwrap(),
        InertKernelIrReceiptV3::from_canonical_preimage(payload("kernel-ir", kernel_ir_seed))
            .unwrap(),
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(payload(
            "mir-to-kir",
            base_seed,
        ))
        .unwrap(),
        InertFormalMemoryReceiptV3::from_canonical_preimage(payload("formal-memory", base_seed))
            .unwrap(),
        InertProofBindingReceiptV3::from_canonical_preimage(payload("proof-binding", base_seed))
            .unwrap(),
        InertTargetBindingReceiptV3::from_canonical_preimage(payload("target-binding", base_seed))
            .unwrap(),
        InertDataLayoutReceiptV3::from_canonical_preimage(payload("data-layout", base_seed))
            .unwrap(),
        InertAbiReceiptV3::from_canonical_preimage(payload("abi", base_seed)).unwrap(),
        InertExportManifestReceiptV3::from_canonical_preimage(payload(
            "export-manifest",
            base_seed,
        ))
        .unwrap(),
        InertAmdgpuLoweringReceiptV3::from_canonical_preimage(payload(
            "amdgpu-lowering",
            base_seed,
        ))
        .unwrap(),
        InertSemanticToLlvmReceiptV3::from_canonical_preimage(payload(
            "semantic-to-llvm",
            base_seed,
        ))
        .unwrap(),
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(payload(
            "final-compiler-module-commitment",
            final_compiler_module_commitment_seed,
        ))
        .unwrap(),
    )
}

fn capsule(seed: u8) -> InertProductionSemanticCapsuleV3 {
    capsule_with_receipts(seed, receipts(seed))
}

fn capsule_with_receipts(
    invocation_seed: u8,
    receipts: OrderedInertSemanticLineageReceiptsV3,
) -> InertProductionSemanticCapsuleV3 {
    InertProductionSemanticCapsuleV3::new(
        invocation(invocation_seed),
        DeviceTargetV1::parse(TARGET).unwrap(),
        receipts,
    )
    .expect("valid inert capsule")
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

#[derive(Debug)]
struct WireLayout {
    invocation: std::ops::Range<usize>,
    invocation_identity: std::ops::Range<usize>,
    target: std::ops::Range<usize>,
    receipts: Vec<(std::ops::Range<usize>, std::ops::Range<usize>)>,
    capsule_identity: std::ops::Range<usize>,
}

fn layout(bytes: &[u8]) -> WireLayout {
    let mut offset = HEADER_BYTES;
    let invocation_len = u32_at(bytes, offset);
    offset += 4;
    let invocation_range = offset..offset + invocation_len;
    offset += invocation_len;
    let invocation_identity = offset..offset + SHA256_BYTES;
    offset += SHA256_BYTES;
    let target_len = usize::from(u16_at(bytes, offset));
    offset += 2;
    let target = offset..offset + target_len;
    offset += target_len;
    let mut receipt_ranges = Vec::new();
    for _ in 0..15 {
        let payload_len = u32_at(bytes, offset);
        offset += 4;
        let payload = offset..offset + payload_len;
        offset += payload_len;
        let identity = offset..offset + SHA256_BYTES;
        offset += SHA256_BYTES;
        receipt_ranges.push((payload, identity));
    }
    let capsule_identity = offset..offset + SHA256_BYTES;
    assert_eq!(capsule_identity.end, bytes.len());
    WireLayout {
        invocation: invocation_range,
        invocation_identity,
        target,
        receipts: receipt_ranges,
        capsule_identity,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn golden_encoding_round_trips_and_retains_complete_preimages() {
    let capsule = capsule(0x10);
    let encoded = capsule.canonical_bytes();
    let decoded = InertProductionSemanticCapsuleV3::decode(encoded).expect("strict roundtrip");

    assert_eq!(decoded, capsule);
    assert_eq!(&encoded[..8], &INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3);
    assert_eq!(
        u16_at(encoded, 8),
        INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3
    );
    assert_eq!(decoded.target().to_string(), TARGET);
    assert_eq!(
        decoded.compiler_closure().identity_sha256(),
        invocation(0x10).compiler_closure().identity_sha256()
    );
    assert_eq!(
        decoded.receipts().semantic_mir().canonical_preimage(),
        payload("semantic-mir", 0x10)
    );
    assert_eq!(
        decoded
            .receipts()
            .final_compiler_module_commitment()
            .canonical_preimage(),
        payload("final-compiler-module-commitment", 0x10)
    );
    assert_eq!(decoded.identity().byte_len(), encoded.len() as u64);
    assert!(decoded.identity().matches_canonical_bytes(encoded));
    assert_eq!(encoded.len(), 1_838);
    assert_eq!(
        hex(decoded.identity().sha256()),
        "26925c1d98ab888b5a540538246d678d48d36bcfe35d0c7158e5bb00b0589ae9"
    );
}

#[test]
fn identities_are_deterministic_and_stage_domain_separated() {
    let first = capsule(0x20);
    let repeated = capsule(0x20);
    let changed = capsule(0x21);
    assert_eq!(first.canonical_bytes(), repeated.canonical_bytes());
    assert_eq!(first.identity(), repeated.identity());
    assert_ne!(first.identity(), changed.identity());

    let same = b"one exact transcript".to_vec();
    let middle = InertMiddleEndReceiptV3::from_canonical_preimage(same.clone()).unwrap();
    let kir = InertKernelIrReceiptV3::from_canonical_preimage(same.clone()).unwrap();
    let final_commitment =
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(same).unwrap();
    assert_ne!(middle.identity().sha256(), kir.identity().sha256());
    assert_ne!(
        kir.identity().sha256(),
        final_commitment.identity().sha256()
    );
    assert_eq!(middle.identity().byte_len(), kir.identity().byte_len());
    assert_eq!(
        kir.identity().byte_len(),
        final_commitment.identity().byte_len()
    );
}

#[test]
fn same_final_commitment_with_different_semantic_content_changes_capsule_identity() {
    let first = capsule(0x22);
    let changed_semantic =
        capsule_with_receipts(0x22, receipts_with_stage_seeds(0x22, 0x23, 0x22, 0x22));

    assert_eq!(
        first
            .receipts()
            .final_compiler_module_commitment()
            .identity(),
        changed_semantic
            .receipts()
            .final_compiler_module_commitment()
            .identity()
    );
    assert_ne!(
        first.receipts().semantic_mir().identity(),
        changed_semantic.receipts().semantic_mir().identity()
    );
    assert_ne!(first.identity(), changed_semantic.identity());
}

#[test]
fn fully_rehashed_cross_stage_splice_is_accepted_only_as_inert_content() {
    let recipient = capsule(0x24);
    let rehashed_splice =
        capsule_with_receipts(0x24, receipts_with_stage_seeds(0x24, 0x25, 0x25, 0x24));
    let decoded = InertProductionSemanticCapsuleV3::decode(rehashed_splice.canonical_bytes())
        .expect("internally consistent inert content is accepted");

    assert_eq!(decoded, rehashed_splice);
    assert_ne!(recipient.identity(), decoded.identity());
    assert_ne!(
        recipient.receipts().semantic_mir().identity(),
        decoded.receipts().semantic_mir().identity()
    );
    assert_ne!(
        recipient.receipts().kernel_ir().identity(),
        decoded.receipts().kernel_ir().identity()
    );
    assert_eq!(
        recipient.receipts().semantic_to_llvm().identity(),
        decoded.receipts().semantic_to_llvm().identity()
    );
    assert_eq!(
        recipient
            .receipts()
            .final_compiler_module_commitment()
            .identity(),
        decoded
            .receipts()
            .final_compiler_module_commitment()
            .identity()
    );
    assert!(!decoded.authenticates_producer());
}

#[test]
fn every_payload_and_identity_mutation_is_rejected() {
    let original = capsule(0x30);
    let encoded = original.canonical_bytes();
    let layout = layout(encoded);

    let mut mutated = encoded.to_vec();
    mutated[layout.invocation.start + 24] ^= 1;
    assert!(InertProductionSemanticCapsuleV3::decode(&mutated).is_err());

    let mut mutated = encoded.to_vec();
    mutated[layout.invocation_identity.start] ^= 1;
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&mutated),
        Err(LineageDecodeErrorV3::InvocationDigestMismatch)
    );

    let mut mutated = encoded.to_vec();
    mutated[layout.target.start] = b'h';
    assert!(InertProductionSemanticCapsuleV3::decode(&mutated).is_err());

    for (index, (payload, identity)) in layout.receipts.iter().enumerate() {
        let mut mutated_payload = encoded.to_vec();
        mutated_payload[payload.start] ^= 1;
        assert!(
            matches!(
                InertProductionSemanticCapsuleV3::decode(&mutated_payload),
                Err(LineageDecodeErrorV3::ReceiptIdentityMismatch { .. })
            ),
            "receipt payload {index}"
        );

        let mut mutated_identity = encoded.to_vec();
        mutated_identity[identity.start] ^= 1;
        assert!(
            matches!(
                InertProductionSemanticCapsuleV3::decode(&mutated_identity),
                Err(LineageDecodeErrorV3::ReceiptIdentityMismatch { .. })
            ),
            "receipt identity {index}"
        );
    }

    let mut mutated = encoded.to_vec();
    mutated[layout.capsule_identity.start] ^= 1;
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&mutated),
        Err(LineageDecodeErrorV3::CapsuleIdentityMismatch)
    );
}

#[test]
fn receipt_splicing_without_the_capsule_preimage_is_rejected() {
    let first = capsule(0x40);
    let second = capsule(0x41);
    let first_layout = layout(first.canonical_bytes());
    let second_layout = layout(second.canonical_bytes());
    let (first_payload, first_identity) = &first_layout.receipts[4];
    let (second_payload, second_identity) = &second_layout.receipts[4];
    assert_eq!(first_payload.len(), second_payload.len());

    let mut spliced = first.canonical_bytes().to_vec();
    spliced[first_payload.clone()]
        .copy_from_slice(&second.canonical_bytes()[second_payload.clone()]);
    spliced[first_identity.clone()]
        .copy_from_slice(&second.canonical_bytes()[second_identity.clone()]);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&spliced),
        Err(LineageDecodeErrorV3::CapsuleIdentityMismatch)
    );
}

#[test]
fn header_version_flags_reserved_and_lengths_are_strict() {
    let encoded = capsule(0x50).canonical_bytes().to_vec();

    let mut bad_magic = encoded.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bad_magic),
        Err(LineageDecodeErrorV3::InvalidMagic)
    );

    let mut bad_version = encoded.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bad_version),
        Err(LineageDecodeErrorV3::UnsupportedVersion(2))
    );

    let mut bad_flags = encoded.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bad_flags),
        Err(LineageDecodeErrorV3::UnsupportedFlags(1))
    );

    let mut bad_reserved = encoded.clone();
    bad_reserved[20..24].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bad_reserved),
        Err(LineageDecodeErrorV3::NonzeroReserved)
    );

    let mut impossible_length = encoded.clone();
    impossible_length[12..20].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&impossible_length),
        Err(LineageDecodeErrorV3::InvalidLength(1))
    );

    let mut short_declared = encoded.clone();
    short_declared[12..20].copy_from_slice(&((encoded.len() - 1) as u64).to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&short_declared),
        Err(LineageDecodeErrorV3::TrailingBytes)
    );

    let mut long_declared = encoded.clone();
    long_declared[12..20].copy_from_slice(&((encoded.len() + 1) as u64).to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&long_declared),
        Err(LineageDecodeErrorV3::Truncated)
    );
}

#[test]
fn truncation_and_physical_trailing_bytes_are_rejected() {
    let encoded = capsule(0x60).canonical_bytes().to_vec();
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&encoded[..encoded.len() - 1]),
        Err(LineageDecodeErrorV3::Truncated)
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&trailing),
        Err(LineageDecodeErrorV3::TrailingBytes)
    );
}

#[test]
fn every_encoded_zero_identity_is_rejected() {
    let encoded = capsule(0x70).canonical_bytes().to_vec();
    let layout = layout(&encoded);

    let mut zero = encoded.clone();
    zero[layout.invocation_identity.clone()].fill(0);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&zero),
        Err(LineageDecodeErrorV3::ZeroIdentity {
            field: "rustc invocation"
        })
    );

    for (index, (_, identity)) in layout.receipts.iter().enumerate() {
        let mut zero = encoded.clone();
        zero[identity.clone()].fill(0);
        assert!(
            matches!(
                InertProductionSemanticCapsuleV3::decode(&zero),
                Err(LineageDecodeErrorV3::ZeroIdentity { .. })
            ),
            "receipt identity {index}"
        );
    }

    let mut zero = encoded;
    zero[layout.capsule_identity].fill(0);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&zero),
        Err(LineageDecodeErrorV3::ZeroIdentity {
            field: "inert production semantic capsule"
        })
    );
}

#[test]
fn empty_and_max_plus_one_receipts_are_rejected_before_capsule_allocation() {
    assert_eq!(
        InertKernelIrReceiptV3::from_canonical_preimage(Vec::new()),
        Err(LineageErrorV3::EmptyPreimage {
            field: "canonical Kernel IR"
        })
    );
    assert_eq!(
        InertKernelIrReceiptV3::from_canonical_preimage(vec![
            1;
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
                + 1
        ]),
        Err(LineageErrorV3::PreimageTooLarge {
            field: "canonical Kernel IR",
            max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
        })
    );

    let encoded = capsule(0x80).canonical_bytes().to_vec();
    let layout = layout(&encoded);
    let first_receipt_len_offset = layout.target.end;
    let mut declared_too_large = encoded;
    declared_too_large[first_receipt_len_offset..first_receipt_len_offset + 4]
        .copy_from_slice(&((MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 1) as u32).to_le_bytes());
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&declared_too_large),
        Err(LineageDecodeErrorV3::PreimageTooLarge {
            field: "rustc identity inventory",
            max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
        })
    );

    let mut aggregate_too_large = capsule(0x81).canonical_bytes().to_vec();
    aggregate_too_large[12..20].copy_from_slice(
        &((MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + 1) as u64).to_le_bytes(),
    );
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&aggregate_too_large),
        Err(LineageDecodeErrorV3::TooLarge {
            max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
        })
    );
}

#[test]
fn noncanonical_target_spelling_and_target_substitution_are_rejected() {
    let encoded = capsule(0x90).canonical_bytes().to_vec();
    let layout = layout(&encoded);
    let noncanonical = b"gfx942:xnack-:sramecc+";
    assert_eq!(noncanonical.len(), layout.target.len());
    let mut bytes = encoded.clone();
    bytes[layout.target.clone()].copy_from_slice(noncanonical);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bytes),
        Err(LineageDecodeErrorV3::NonCanonical)
    );

    let other_canonical = b"gfx942:sramecc-:xnack-";
    assert_eq!(other_canonical.len(), layout.target.len());
    let mut bytes = encoded;
    bytes[layout.target].copy_from_slice(other_canonical);
    assert_eq!(
        InertProductionSemanticCapsuleV3::decode(&bytes),
        Err(LineageDecodeErrorV3::TargetMismatch)
    );

    assert_eq!(
        InertProductionSemanticCapsuleV3::new(
            invocation(0x90),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            receipts(0x90),
        ),
        Err(LineageErrorV3::TargetMismatch)
    );
}

#[test]
fn inert_content_records_make_no_authority_claims() {
    let capsule = capsule(0xa0);
    assert!(!capsule.authenticates_producer());
    assert!(!capsule.grants_compiler_authority());
    assert!(!capsule.grants_artifact_authority());
    assert!(!capsule.grants_publication_authority());
    assert!(!capsule.grants_load_authority());
    assert!(!capsule.grants_launch_authority());
}

#[test]
fn exported_decode_resource_bound_is_consistent_with_wire_limits() {
    assert_eq!(
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_DECODE_OWNED_BYTES_V3,
        2 * MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + MAX_DESCRIPTOR_BYTES_V3
    );
    const {
        assert!(
            MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_DECODE_OWNED_BYTES_V3
                > 2 * MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
        );
    }
    assert!(
        capsule(0xa1).canonical_bytes().len() <= MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
    );
}
