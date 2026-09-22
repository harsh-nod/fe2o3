use super::*;
use fe2o3_kernel_descriptor as kd;
use fe2o3_rustc_invocation as invocation;
use sha2::{Digest, Sha256};
use std::{convert::Infallible, ffi::OsString};

// These are inert framing fixtures, not authenticated source or semantically
// admitted optimizer histories. Full B1.a/B1.c replay is a separate obligation.
pub(crate) fn free(_: usize) -> Result<(), Infallible> {
    Ok(())
}
pub(crate) fn identity(domain: &[u8], bytes: &[u8]) -> ExpandedContentIdentityV4 {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    ExpandedContentIdentityV4::from_declared(h.finalize().into(), bytes.len() as u64).unwrap()
}
pub(crate) fn invocation_bytes(target: &str) -> Vec<u8> {
    let pins = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]];
    let closure = fe2o3_build_authority::CompilerClosureV2::new(
        pins[0], pins[1], pins[2], pins[3], pins[4], pins[5],
    )
    .unwrap();
    let unit = invocation::RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "expanded_frame".into(),
            "src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .unwrap();
    let env = invocation::CompileEnvironmentV2::from_child_environment(
        [
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
            ("FE2O3_TARGET", target),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]
        .into_iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    )
    .unwrap();
    let old = invocation::RustcInvocationDescriptorV2::new(pins[3], pins[5], unit, env).unwrap();
    invocation::encode_descriptor_v3(
        &invocation::RustcInvocationDescriptorV3::new(old, closure).unwrap(),
    )
    .unwrap()
}
pub(crate) fn descriptor(target: &str, count: usize) -> Vec<u8> {
    let compiler = kd::CompilerIdentityV1::new(
        kd::Text::new("rustc").unwrap(),
        kd::Text::new("nightly").unwrap(),
        [7; 20],
    );
    let producer = kd::ProducerIdentityV1::new(
        kd::Text::new("fe2o3").unwrap(),
        kd::Text::new("inert-frame-test").unwrap(),
    );
    let launch = kd::LaunchConstraintsV1::new(
        1,
        kd::BlockSizeV1::Any,
        kd::DimensionsV1::new(1024, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let evidence = kd::BuildEvidenceV1::new(
        kd::EvidenceIdentity::from_opaque_bytes([11; 32]),
        kd::EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let names = ["maple", "zebra", "amber"];
    let symbols = ["maple.kd", "zebra.kd", "amber.kd"];
    let kernels: Vec<_> = (0..count)
        .map(|i| kd::KernelDescriptorInputV3 {
            kernel_id: kd::KernelId::from_bytes([(i + 1) as u8; 32]),
            logical_name: names[i],
            entry_name: names[i],
            descriptor_symbol: symbols[i],
            source_evidence: evidence,
            executable_ir_evidence: evidence,
            capabilities: &[kd::CapabilityV1::AmdWave],
            abi_layout: kd::KernelAbiLayoutV1::new(0, 0, 1).unwrap(),
            launch: &launch,
            arguments: &[],
        })
        .collect();
    let requirements: Vec<_> = kernels
        .iter()
        .map(|k| {
            kd::KernelTargetRequirementsV2::new(
                k.kernel_id,
                kd::LdsRequirementsV2::new(0, 0).unwrap(),
                kd::RequiredWavefrontWidthV2::Wave64,
                false,
                kd::SynchronizationRequirementsV2::empty(),
                kd::AtomicRequirementsV2::empty(),
            )
        })
        .collect();
    let input = kd::DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: kd::CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: kd::CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: kd::DeviceTargetV1::parse(target).unwrap(),
        type_records: &[],
        layout_records: &[],
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut out = vec![0; kd::encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    kd::encode_device_descriptor_table_v3(&input, &mut out, &mut free).unwrap();
    out
}
pub(crate) fn graph(seed: u8) -> Vec<u8> {
    let mut out = vec![0; 25];
    out[..8].copy_from_slice(b"FE2O3KI\0");
    out[8..10].copy_from_slice(&12u16.to_le_bytes());
    out[12..20].copy_from_slice(&25u64.to_le_bytes());
    out[24] = seed;
    out
}
pub(crate) fn history(final_graph: &[u8], padding: usize, rounds: usize) -> Vec<u8> {
    let prior = graph(3);
    let mut u = vec![0x61; 48 + padding];
    u[..8].copy_from_slice(b"F2LUH1\0\0");
    let lengths = [prior.len(), final_graph.len(), 416, 64, 808];
    let scalar_len = 160 + 48 * rounds + lengths.iter().sum::<usize>() * rounds;
    let mut scalar = vec![0; 160 + 48 * rounds];
    scalar[..8].copy_from_slice(b"F2SPH1\0\0");
    scalar[8..10].copy_from_slice(&1u16.to_le_bytes());
    scalar[10..12].copy_from_slice(&1u16.to_le_bytes());
    scalar[12..16].copy_from_slice(&160u32.to_le_bytes());
    scalar[16..24].copy_from_slice(&(scalar_len as u64).to_le_bytes());
    scalar[24..26].copy_from_slice(&(rounds as u16).to_le_bytes());
    scalar[32..40].copy_from_slice(b"F2SFP1\0\0");
    scalar[40..42].copy_from_slice(&1u16.to_le_bytes());
    scalar[42..44].copy_from_slice(&16u16.to_le_bytes());
    scalar[44..46].copy_from_slice(&(rounds as u16).to_le_bytes());
    scalar[46..48].copy_from_slice(&2u16.to_le_bytes());
    scalar[48..80].fill(3);
    scalar[80..88].copy_from_slice(&(prior.len() as u64).to_le_bytes());
    scalar[88..120].fill(4);
    scalar[120..128].copy_from_slice(&(final_graph.len() as u64).to_le_bytes());
    scalar[128..136].copy_from_slice(&[6, 0, 2, 0, 0, 0, 0, 0]);
    scalar[136..144].copy_from_slice(&[3, 0, 8, 0, 1, 0, 0, 0]);
    for index in 0..rounds {
        let row = 160 + index * 48;
        scalar[row..row + 2].copy_from_slice(&(index as u16).to_le_bytes());
        scalar[row + 2..row + 4].copy_from_slice(&1u16.to_le_bytes());
        for (field, length) in lengths.iter().enumerate() {
            let at = row + 8 + field * 8;
            scalar[at..at + 8].copy_from_slice(&(*length as u64).to_le_bytes());
        }
        scalar.extend_from_slice(&prior);
        scalar.extend_from_slice(final_graph);
        scalar.extend_from_slice(&[0; 416]);
        scalar.extend_from_slice(&[0; 64]);
        scalar.extend_from_slice(&[0; 808]);
    }
    let total = 48 + u.len() + scalar.len();
    let mut out = vec![0; 48];
    out[..8].copy_from_slice(b"F2EPH1\0\0");
    out[8..10].copy_from_slice(&1u16.to_le_bytes());
    out[10..12].copy_from_slice(&1u16.to_le_bytes());
    out[12..16].copy_from_slice(&48u32.to_le_bytes());
    out[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    out[24..26].copy_from_slice(&1u16.to_le_bytes());
    out[32..40].copy_from_slice(&(u.len() as u64).to_le_bytes());
    out[40..48].copy_from_slice(&(scalar.len() as u64).to_le_bytes());
    out.extend_from_slice(&u);
    out.extend_from_slice(&scalar);
    out
}
pub(crate) struct Fixture {
    pub(crate) target: String,
    pub(crate) invocation: Vec<u8>,
    pub(crate) receipts: [Vec<u8>; 16],
    pub(crate) roots: Vec<NativeOutputTransitionRootV1>,
    pub(crate) kind: ExpandedSourceKindV1,
    pub(crate) native_axes: [ExpandedContentIdentityV4; 4],
}
impl Fixture {
    pub(crate) fn new(
        target: &str,
        kind: ExpandedSourceKindV1,
        count: usize,
        padding: usize,
    ) -> Self {
        let mut receipts: [Vec<u8>; 16] = std::array::from_fn(|i| vec![i as u8 + 1; i + 1]);
        receipts[4] = graph(4);
        receipts[10] = descriptor(target, count);
        receipts[15] = history(&receipts[4], padding, 2);
        let roots = if count == 1 {
            vec![NativeOutputTransitionRootV1::new(7, 0, 0, 0)]
        } else {
            vec![
                NativeOutputTransitionRootV1::new(7, 2, 1, 0),
                NativeOutputTransitionRootV1::new(31, 0, 2, 2),
                NativeOutputTransitionRootV1::new(88, 1, 0, 1),
            ]
        };
        let marker = ExpandedContentIdentityV4::from_declared([9; 32], 17).unwrap();
        let mut out = Self {
            target: target.into(),
            invocation: invocation_bytes(target),
            receipts,
            roots,
            kind,
            native_axes: [marker; 4],
        };
        out.refresh();
        out
    }
    pub(crate) fn input(&self) -> ExpandedCapsuleInputsV4<'_> {
        ExpandedCapsuleInputsV4 {
            invocation: &self.invocation,
            target: &self.target,
            receipts: std::array::from_fn(|i| self.receipts[i].as_slice()),
        }
    }
    pub(crate) fn refresh(&mut self) {
        let marker = ExpandedContentIdentityV4::from_declared([9; 32], 17).unwrap();
        let mut axes = [marker; EXPANDED_OUTPUT_AXIS_COUNT_V1];
        let source = b"opaque-original-source-packet-not-authenticated";
        let catalog = b"opaque-final-catalog-not-admitted";
        axes[ExpandedOutputAxisV1::OriginalSource as usize] =
            identity(EXPANDED_ORIGINAL_SOURCE_DOMAIN_V1, source);
        axes[ExpandedOutputAxisV1::FinalCatalog as usize] =
            identity(EXPANDED_FINAL_CATALOG_DOMAIN_V1, catalog);
        for (axis, slot) in [
            (ExpandedOutputAxisV1::SemanticMir, 2),
            (ExpandedOutputAxisV1::Correspondence, 5),
            (ExpandedOutputAxisV1::History, 15),
            (ExpandedOutputAxisV1::FinalGraph, 4),
            (ExpandedOutputAxisV1::FinalFormal, 6),
            (ExpandedOutputAxisV1::TargetBinding, 8),
            (ExpandedOutputAxisV1::DataLayout, 9),
            (ExpandedOutputAxisV1::AmdgpuLowering, 12),
            (ExpandedOutputAxisV1::FinalCommitment, 14),
        ] {
            axes[axis as usize] = identity(
                ExpandedReceiptSlotV4::ALL[slot].identity_domain(),
                &self.receipts[slot],
            );
        }
        for (axis, value) in [
            ExpandedOutputAxisV1::Descriptor,
            ExpandedOutputAxisV1::NativeModule,
            ExpandedOutputAxisV1::SymbolManifest,
            ExpandedOutputAxisV1::FfiEnvelope,
        ]
        .into_iter()
        .zip(self.native_axes)
        {
            axes[axis as usize] = value;
        }
        let input = ExpandedOutputAssociationInputsV1 {
            source_kind: self.kind,
            target: &self.target,
            axes,
            roots: &self.roots,
            original_source: source,
            final_catalog: catalog,
        };
        self.receipts[7] =
            encode_expanded_output_association_v1(&input, usize::MAX, &mut free).unwrap();
        let association = ExpandedOutputAssociationRefV1::read(
            &self.receipts[7],
            EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
            &mut free,
        )
        .unwrap();
        self.receipts[13] =
            encode_expanded_semantic_to_llvm_v1(&association, usize::MAX, &mut free).unwrap();
    }
    pub(crate) fn capsule(&self) -> InertProductionSemanticCapsuleV4 {
        InertProductionSemanticCapsuleV4::new(&self.input(), usize::MAX, &mut free).unwrap()
    }
}

#[test]
fn expanded_capsule_inert_profiles_branches_and_full_nonlexical_roster() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for kind in [ExpandedSourceKindV1::Direct, ExpandedSourceKindV1::Erased] {
            let f = Fixture::new(target, kind, 3, 0);
            let capsule = f.capsule();
            assert!(!capsule.grants_authority());
            let view = capsule.view(usize::MAX, &mut free).unwrap();
            assert_eq!(
                view.identity(),
                identity(
                    INERT_PRODUCTION_SEMANTIC_CAPSULE_DOMAIN_V4,
                    capsule.canonical_bytes()
                )
            );
            assert_eq!(view.invocation_bytes(), f.invocation);
            assert_eq!(view.target_text(), target);
            for slot in ExpandedReceiptSlotV4::ALL {
                assert_eq!(
                    view.receipt(slot).canonical_preimage(),
                    f.receipts[slot as usize]
                );
            }
            let a = ExpandedOutputAssociationRefV1::read(
                view.receipt(ExpandedReceiptSlotV4::ProofBinding)
                    .canonical_preimage(),
                usize::MAX,
                &mut free,
            )
            .unwrap();
            assert_eq!(a.source_kind(), kind);
            for (i, (semantic, descriptor, input, output)) in
                [(7, 2, 1, 0), (31, 0, 2, 2), (88, 1, 0, 1)]
                    .into_iter()
                    .enumerate()
            {
                let row = a.root(i, &mut free).unwrap();
                assert_eq!(
                    (
                        row.semantic_root(),
                        row.descriptor_ordinal(),
                        row.input_kernel_ordinal(),
                        row.output_kernel_ordinal()
                    ),
                    (semantic, descriptor, input, output)
                );
            }
        }
    }
}
#[test]
fn owned_capsule_decode_preserves_transferred_capacity_and_pointer() {
    let bytes = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0)
        .capsule()
        .canonical_bytes()
        .to_vec();
    let ptr = bytes.as_ptr();
    let capacity = bytes.capacity();
    let got = InertProductionSemanticCapsuleV4::decode_owned(bytes, usize::MAX, &mut free).unwrap();
    assert_eq!(got.canonical_bytes().as_ptr(), ptr);
    assert_eq!(
        got.retained_storage(),
        std::mem::size_of::<InertProductionSemanticCapsuleV4>() + capacity
    );
}
#[test]
fn directory_corruption_and_cross_version_frames_are_typed_refusals() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    let good = f.capsule();
    for at in [0, 8, 10, 12, 20, 60, 62, 64] {
        let mut bytes = good.canonical_bytes().to_vec();
        bytes[at] ^= 1;
        assert!(
            ExpandedCapsuleRefV4::read(&bytes, usize::MAX, &mut free).is_err(),
            "offset {at}"
        );
    }
    let mut old = good.canonical_bytes().to_vec();
    old[..8].copy_from_slice(b"F2O3ISV3");
    old[8..10].copy_from_slice(&3u16.to_le_bytes());
    assert!(matches!(
        ExpandedCapsuleRefV4::read(&old, usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format("capsule header"))
    ));
    assert!(InertProductionSemanticCapsuleV3::decode(good.canonical_bytes()).is_err());
}
#[test]
fn each_receipt_requires_exact_content_not_just_directory_lengths() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    let good = f.capsule();
    let mut cursor = 60 + 16 * 44 + f.invocation.len() + f.target.len();
    for payload in &f.receipts {
        let mut bytes = good.canonical_bytes().to_vec();
        bytes[cursor + payload.len() - 1] ^= 1;
        assert!(matches!(
            ExpandedCapsuleRefV4::read(&bytes, usize::MAX, &mut free),
            Err(ExpandedPublicationErrorV4::Identity("receipt content"))
        ));
        cursor += payload.len();
    }
}
#[test]
fn final_graph_is_resolved_by_directory_and_compared_byte_for_byte() {
    let mut f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    f.receipts[4][24] ^= 1;
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format(
            "complete final V12 graph"
        ))
    ));
    f.receipts[4] = vec![7; 96];
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format(
            "complete final V12 graph"
        ))
    ));
}
#[test]
fn actual_invocation_target_and_nominal_descriptor_cannot_be_substituted() {
    let mut f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    f.invocation = invocation_bytes("gfx950:xnack-");
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format(
            "actual invocation target"
        ))
    ));
    f.invocation = invocation_bytes(&f.target);
    f.receipts[10] = descriptor("gfx950:xnack-", 1);
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Identity(
            "nominal descriptor target/digest/roster"
        ))
    ));
    f.receipts[10] = descriptor(&f.target, 1);
    f.receipts[10][8..10].copy_from_slice(&1u16.to_le_bytes());
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Descriptor(_))
    ));
}
#[test]
fn complete_opaque_history_can_exceed_old_receipt_cap_but_does_not_claim_semantics() {
    let f = Fixture::new(
        "gfx950:xnack-",
        ExpandedSourceKindV1::Erased,
        1,
        4 * 1024 * 1024,
    );
    assert!(f.receipts[15].len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3);
    let capsule = f.capsule();
    assert!(capsule.canonical_bytes().len() > 4 * 1024 * 1024);
    assert!(InertProofBindingReceiptV3::from_canonical_preimage(f.receipts[15].clone()).is_err());
    let history =
        InertExpandedHistoryReceiptV4::from_vec(f.receipts[15].clone(), usize::MAX, &mut free)
            .unwrap();
    assert!(!history.grants_authority());
    assert_eq!(history.canonical_bytes(), f.receipts[15]);
}
#[test]
fn malformed_scalar_directory_policy_and_terminal_extent_refuse() {
    let bytes = history(&graph(4), 0, 2);
    let scalar = 48 + 48;
    for at in [
        8,
        10,
        12,
        16,
        24,
        26,
        32,
        scalar + 8,
        scalar + 10,
        scalar + 12,
        scalar + 24,
        scalar + 26,
        scalar + 40,
        scalar + 42,
        scalar + 46,
        scalar + 120,
        scalar + 128,
        scalar + 136,
        scalar + 144,
        scalar + 160,
    ] {
        let mut bad = bytes.clone();
        bad[at] ^= 1;
        assert!(
            expanded_history_final_graph_range_v4(&bad, usize::MAX, &mut free).is_err(),
            "offset {at}"
        );
    }
}
#[test]
fn old_slot_domains_and_four_mebibyte_limit_are_preserved() {
    let data = vec![3; MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3];
    let old = InertKernelIrReceiptV3::from_canonical_preimage(data.clone()).unwrap();
    assert_eq!(
        identity(
            ExpandedReceiptSlotV4::FinalKernelIr.identity_domain(),
            &data
        )
        .sha256(),
        old.identity().sha256()
    );
    let mut excess = data;
    excess.push(3);
    assert!(InertKernelIrReceiptV3::from_canonical_preimage(excess).is_err());
    assert_eq!(
        ExpandedReceiptSlotV4::FinalKernelIr.maximum_bytes(),
        4 * 1024 * 1024
    );
    assert_eq!(
        ExpandedReceiptSlotV4::NominalAbi.maximum_bytes(),
        4 * 1024 * 1024
    );
}
#[test]
fn capsule_aggregate_is_independent_of_individual_slot_caps() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    let backing = vec![1; MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3];
    let mut input = f.input();
    input.receipts[2] = &backing;
    let without_history = 60
        + 16 * 44
        + input.invocation.len()
        + input.target.len()
        + input
            .receipts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 15)
            .map(|(_, b)| b.len())
            .sum::<usize>();
    let remaining = 160 * 1024 * 1024 - without_history;
    input.receipts[15] = &backing[..remaining];
    assert_eq!(expanded_capsule_length_v4(&input), Some(160 * 1024 * 1024));
    input.receipts[15] = &backing[..remaining + 1];
    assert_eq!(expanded_capsule_length_v4(&input), None);
}

#[test]
fn golden_directory_order_and_full_root_coordinates_are_not_name_sets() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Erased, 3, 0);
    let owner = f.capsule();
    let bytes = owner.canonical_bytes();
    assert_eq!(&bytes[..8], b"F2O3ISV4");
    assert_eq!(&bytes[8..12], &[4, 0, 0, 0]);
    assert_eq!(
        u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
        bytes.len() as u64
    );
    assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 16);
    for slot in 0..16 {
        let at = 60 + slot * 44;
        assert_eq!(
            u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
            slot as u16
        );
        assert_eq!(&bytes[at + 2..at + 4], &[0, 0]);
        assert_eq!(
            u64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap()),
            f.receipts[slot].len() as u64
        );
        assert_eq!(
            &bytes[at + 12..at + 44],
            identity(
                ExpandedReceiptSlotV4::ALL[slot].identity_domain(),
                &f.receipts[slot]
            )
            .sha256()
        );
        for delta in [0, 2, 4, 12] {
            let mut bad = bytes.to_vec();
            bad[at + delta] ^= 1;
            assert!(
                ExpandedCapsuleRefV4::read(&bad, usize::MAX, &mut free).is_err(),
                "slot {slot} field {delta}"
            );
        }
    }
    let base = 40 + 19 * 40 + f.target.len();
    for (i, values) in [[7u32, 2, 1, 0], [31, 0, 2, 2], [88, 1, 0, 1]]
        .into_iter()
        .enumerate()
    {
        for (field, value) in values.into_iter().enumerate() {
            let at = base + i * 16 + field * 4;
            assert_eq!(&f.receipts[7][at..at + 4], &value.to_le_bytes());
        }
    }
    for field in 0..4 {
        let mut bad = f.receipts[7].clone();
        let duplicate: [u8; 4] = bad[base + field * 4..base + field * 4 + 4]
            .try_into()
            .unwrap();
        bad[base + 16 + field * 4..base + 16 + field * 4 + 4].copy_from_slice(&duplicate);
        assert!(matches!(
            ExpandedOutputAssociationRefV1::read(&bad, usize::MAX, &mut free),
            Err(ExpandedPublicationErrorV4::Format(
                "semantic root order" | "root permutation"
            ))
        ));
    }
    let mut reordered = f.receipts[7].clone();
    for offset in 0..16 {
        reordered.swap(base + offset, base + 16 + offset);
    }
    assert!(matches!(
        ExpandedOutputAssociationRefV1::read(&reordered, usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format("semantic root order"))
    ));
}

#[test]
fn independent_rehashed_nominal_and_association_donors_keep_typed_refusals() {
    let mut f = Fixture::new("gfx950:xnack-", ExpandedSourceKindV1::Direct, 3, 0);
    f.receipts[10][kd::CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3] = 1;
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Identity(
            "nominal descriptor target/digest/roster"
        ))
    ));
    f.receipts[10] = descriptor(&f.target, 1);
    f.refresh();
    assert!(matches!(
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Identity(
            "nominal descriptor target/digest/roster"
        ))
    ));
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    let mut bad = f.receipts[7].clone();
    bad[40 + ExpandedOutputAxisV1::PreBind as usize * 40] ^= 1;
    assert!(matches!(
        ExpandedOutputAssociationRefV1::read(&bad, usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Identity(
            "direct pre-bind input"
        ))
    ));
    for axis in [
        ExpandedOutputAxisV1::OriginalSource,
        ExpandedOutputAxisV1::FinalCatalog,
    ] {
        let mut bad = f.receipts[7].clone();
        bad[40 + axis as usize * 40] ^= 1;
        assert!(matches!(
            ExpandedOutputAssociationRefV1::read(&bad, usize::MAX, &mut free),
            Err(ExpandedPublicationErrorV4::Identity(
                "embedded association content"
            ))
        ));
    }
    let mut foreign = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    foreign.receipts[8].push(3);
    foreign.refresh();
    let association =
        ExpandedOutputAssociationRefV1::read(&foreign.receipts[7], usize::MAX, &mut free).unwrap();
    let derivation =
        ExpandedSemanticToLlvmRefV1::read(&f.receipts[13], usize::MAX, &mut free).unwrap();
    assert!(matches!(
        derivation.check_association(&association, usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Identity(_))
    ));
}

#[test]
fn hash_domain_length_and_zero_declarations_are_distinct_and_inert() {
    let a = expanded_content_identity_v4(b"one\0", b"abc", usize::MAX, &mut free).unwrap();
    let b = expanded_content_identity_v4(b"two\0", b"abc", usize::MAX, &mut free).unwrap();
    let c = expanded_content_identity_v4(b"one\0", b"abc\0", usize::MAX, &mut free).unwrap();
    assert_eq!(a, identity(b"one\0", b"abc"));
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert!(ExpandedContentIdentityV4::from_declared([0; 32], 3).is_none());
    assert!(ExpandedContentIdentityV4::from_declared([1; 32], 0).is_none());
    assert!(matches!(
        expanded_content_identity_v4(b"one\0", b"", usize::MAX, &mut free),
        Err(ExpandedPublicationErrorV4::Format(
            "empty identity preimage"
        ))
    ));
}
