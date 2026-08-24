use dialect_amdgcn::lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CodeObjectVersion, CompilerDescriptorSourceV1,
    CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ExactLdsGemmBufferRoleV1, ExactLdsGemmCompilerImportPinsV1, ExactLdsGemmElementV1,
    ExactLdsGemmProfileAdmissionErrorV1, ExactLdsGemmProfileAvailabilityV1,
    ExactLdsGemmProfileIdV1, exact_lds_gemm_profile_availability_v1,
    inspect_exact_lds_gemm_compiler_import_v1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest,
    CapabilityV1, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1, OwnershipSemantics,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
};
use fe2o3_kernel_ir::{
    KERNEL_IR_VERSION_V7, TILED_GEMM_LDS_EDGES_V1_KERNEL_ID, TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
    TILED_GEMM_LDS_K32_V2_KERNEL_ID, TILED_GEMM_LDS_V1_ALLOCATION_COUNT,
    TILED_GEMM_LDS_V1_KERNEL_ID, TILED_GEMM_LDS_V1_LDS_ALIGNMENT,
    TILED_GEMM_LDS_V1_STATIC_LDS_BYTES, TILED_GEMM_LDS_V1_TILE_BYTES, TiledGemmLdsV1Profile,
    encode_module_v7, tiled_gemm_lds_v1_module,
};
use sha2::{Digest, Sha256};

const TARGET: &str = "gfx942:xnack-";
const LOGICAL_NAME: &str = "tiled_gemm_lds_slice1";
const DESCRIPTOR_SYMBOL: &str = "tiled_gemm_lds_v1.kd";
const PRODUCER_VERSION: &str = "typed-tiled-gemm-lds-slice1-gfx942-cov6-v1";
const AUTHORITY_SECTION: &str = ".fe2o3.tiled-lds-slice1-auth.v1";
const RESOURCE_SECTION: &str = ".fe2o3.tiled-lds-slice1-resources.v1";
const RESOURCE_DOMAIN: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.worker-v2-resources.v1";
const CANONICAL_IR_DOMAIN: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.compiler-structural-ir.v1";
const SOURCE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-IDENTITY/V1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-DIGEST/V1\0";
const IR_IDENTITY_DOMAIN: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-IDENTITY/V1\0";
const IR_DIGEST_DOMAIN: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-DIGEST/V1\0";
const KERNEL_BINDING: [u8; 32] = [0x4c; 32];
const SOURCE_AUTHORITY: [u8; 32] = [0xa5; 32];

#[derive(Clone)]
struct Slice1Fixture {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    envelope: CompilerFfiEnvelopeV1,
    manifest: CompilerModuleSymbolManifestV1,
    llvm_body: String,
    descriptor: CompilerDescriptorSourceV1,
    source_authority: [u8; 32],
    resources: Vec<u8>,
}

impl Slice1Fixture {
    fn canonical() -> Self {
        let target = DeviceTargetV1::parse(TARGET).expect("exact target");
        let code_object_version = CodeObjectVersion::V6;
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, code_object_version)
                .expect("exact empty envelope");
        let llvm_body = canonical_llvm_body();
        let descriptor = descriptor_source(&envelope, &llvm_body, 0x51);
        let resources = resource_transcript(&SOURCE_AUTHORITY, descriptor.identity().sha256());
        Self {
            target,
            code_object_version,
            envelope,
            manifest: manifest_for(TILED_GEMM_LDS_V1_KERNEL_ID),
            llvm_body,
            descriptor,
            source_authority: SOURCE_AUTHORITY,
            resources,
        }
    }

    fn pins(&self) -> ExactLdsGemmCompilerImportPinsV1 {
        ExactLdsGemmCompilerImportPinsV1::new(self.descriptor.identity(), self.source_authority)
            .expect("nonzero exact pins")
    }

    fn canonical_module(&self) -> Vec<u8> {
        module_with_sections(
            &self.llvm_body,
            &[
                (
                    COMPILER_DESCRIPTOR_SECTION_NAME_V1,
                    self.descriptor.canonical_bytes(),
                ),
                (AUTHORITY_SECTION, &self.source_authority),
                (RESOURCE_SECTION, &self.resources),
            ],
        )
    }

    fn handoff(&self) -> CompilerModuleHandoffV2 {
        self.handoff_with(
            self.target,
            self.code_object_version,
            self.envelope.clone(),
            self.manifest.clone(),
            &self.canonical_module(),
        )
    }

    fn handoff_for_module(&self, module: &[u8]) -> CompilerModuleHandoffV2 {
        self.handoff_with(
            self.target,
            self.code_object_version,
            self.envelope.clone(),
            self.manifest.clone(),
            module,
        )
    }

    fn handoff_with(
        &self,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        envelope: CompilerFfiEnvelopeV1,
        manifest: CompilerModuleSymbolManifestV1,
        module: &[u8],
    ) -> CompilerModuleHandoffV2 {
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target,
            code_object_version,
            envelope,
            manifest,
            module,
        )
        .expect("structurally valid public V2 handoff")
    }
}

fn canonical_llvm_body() -> String {
    lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
        &tiled_gemm_lds_v1_module(),
        TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .expect("dedicated Slice 1 lowering")
    .as_str()
    .to_owned()
}

fn manifest_for(entry: &str) -> CompilerModuleSymbolManifestV1 {
    CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, entry.to_owned()),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            format!("{entry}.kd"),
        ),
    ])
    .expect("canonical exact two-symbol manifest")
}

fn descriptor_source(
    envelope: &CompilerFfiEnvelopeV1,
    llvm_body: &str,
    source_nonce: u8,
) -> CompilerDescriptorSourceV1 {
    let u16_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let u16_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16));
    let f32_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let f32_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let arguments = vec![
        LogicalArgumentV1::shared_slice(0, name("arg0"), &u16_source, &u16_layout, 0)
            .expect("A ABI"),
        LogicalArgumentV1::shared_slice(1, name("arg1"), &u16_source, &u16_layout, 16)
            .expect("B ABI"),
        LogicalArgumentV1::disjoint_slice(
            2,
            name("arg2"),
            &f32_source,
            &f32_layout,
            AccessMode::ReadWrite,
            32,
        )
        .expect("C ABI"),
    ];
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(KERNEL_BINDING),
        name(LOGICAL_NAME),
        name(TILED_GEMM_LDS_V1_KERNEL_ID),
        name(DESCRIPTOR_SYMBOL),
        source_evidence(source_nonce),
        executable_evidence(envelope, llvm_body),
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::WorkgroupMemory,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdMfma,
        ],
        KernelAbiLayoutV1::new(48, 304, 8).expect("Slice 1 ABI"),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).expect("WG64")),
            DimensionsV1::new(1, 1, 1).expect("one-workgroup grid"),
            64,
            TILED_GEMM_LDS_V1_STATIC_LDS_BYTES,
            0,
        )
        .expect("Slice 1 launch"),
        arguments,
    )
    .expect("exact Slice 1 kernel descriptor");
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            text("rustc-codegen-fe2o3"),
            text(env!("CARGO_PKG_VERSION")),
            [0; 20],
        ),
        ProducerIdentityV1::new(
            text("rustc-codegen-fe2o3-worker-v2"),
            text(PRODUCER_VERSION),
        ),
        DeviceTargetV1::parse(TARGET).expect("descriptor target"),
        vec![u16_source, f32_source],
        vec![u16_layout, f32_layout],
        vec![kernel],
    )
    .expect("exact Slice 1 descriptor table");
    CompilerDescriptorSourceV1::new(table).expect("zero-digest descriptor source")
}

fn source_evidence(nonce: u8) -> BuildEvidenceV1 {
    let nonce = [nonce];
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            SOURCE_IDENTITY_DOMAIN,
            &[
                &KERNEL_BINDING,
                LOGICAL_NAME.as_bytes(),
                TILED_GEMM_LDS_V1_KERNEL_ID.as_bytes(),
                &nonce,
            ],
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(
            SOURCE_DIGEST_DOMAIN,
            &[b"A:&[u16]", b"B:&[u16]", b"C:DisjointSlice<f32>", &nonce],
        )),
    )
}

fn executable_evidence(envelope: &CompilerFfiEnvelopeV1, llvm_body: &str) -> BuildEvidenceV1 {
    let envelope_identity = envelope.identity().as_bytes();
    let target = envelope.target().to_string();
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            IR_IDENTITY_DOMAIN,
            &[
                &KERNEL_BINDING,
                &envelope_identity,
                target.as_bytes(),
                TILED_GEMM_LDS_V1_KERNEL_ID.as_bytes(),
            ],
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(
            IR_DIGEST_DOMAIN,
            &[
                envelope.canonical_bytes(),
                llvm_body.as_bytes(),
                TILED_GEMM_LDS_V1_KERNEL_ID.as_bytes(),
            ],
        )),
    )
}

fn resource_transcript(authority: &[u8; 32], descriptor_identity: &[u8; 32]) -> Vec<u8> {
    let kernel_ir = encode_module_v7(&tiled_gemm_lds_v1_module()).expect("canonical Kernel IR");
    let canonical_ir = canonical_ir_commitment(&kernel_ir);
    let geometry = [64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
    let mut transcript = Vec::new();
    for field in [
        RESOURCE_DOMAIN,
        authority,
        TARGET.as_bytes(),
        &6u16.to_le_bytes(),
        &canonical_ir,
        descriptor_identity,
        &geometry,
        &0u32.to_le_bytes(),
        &TILED_GEMM_LDS_V1_STATIC_LDS_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_ALLOCATION_COUNT.to_le_bytes(),
        &TILED_GEMM_LDS_V1_TILE_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_LDS_ALIGNMENT.to_le_bytes(),
    ] {
        append_field(&mut transcript, field);
    }
    transcript
}

fn canonical_ir_commitment(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CANONICAL_IR_DOMAIN);
    hash_field(&mut digest, bytes);
    digest.finalize().into()
}

fn domain_hash(domain: &[u8], frames: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hash_field(&mut digest, frame);
    }
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn append_field(transcript: &mut Vec<u8>, field: &[u8]) {
    transcript.extend_from_slice(&(field.len() as u64).to_le_bytes());
    transcript.extend_from_slice(field);
}

fn module_with_sections(llvm_body: &str, sections: &[(&str, &[u8])]) -> Vec<u8> {
    let mut module = llvm_body.to_owned();
    module.push('\n');
    for (name, bytes) in sections {
        append_section(&mut module, name, bytes);
    }
    module.into_bytes()
}

fn append_section(module: &mut String, name: &str, bytes: &[u8]) {
    module.push_str(&format!(
        "module asm \".section {name},\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n"
    ));
    for chunk in bytes.chunks(16) {
        module.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                module.push_str(", ");
            }
            module.push_str(&format!("0x{byte:02x}"));
        }
        module.push_str("\"\n");
    }
}

fn name(value: &str) -> ValidName {
    ValidName::new(value).expect("valid descriptor name")
}

fn text(value: &str) -> Text {
    Text::new(value).expect("valid descriptor text")
}

#[test]
fn canonical_public_fixture_is_admitted_deterministically_without_authority() {
    let first_fixture = Slice1Fixture::canonical();
    let second_fixture = Slice1Fixture::canonical();
    assert_eq!(
        first_fixture.manifest.entries().collect::<Vec<_>>(),
        [
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                TILED_GEMM_LDS_V1_KERNEL_ID,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                DESCRIPTOR_SYMBOL,
            ),
        ]
    );
    assert_eq!(first_fixture.envelope.inspection().import_count(), 0);
    assert_eq!(first_fixture.envelope.inspection().export_count(), 0);
    assert_eq!(
        first_fixture
            .envelope
            .inspection()
            .requires_compiler_module_definition_count(),
        0
    );
    assert_eq!(
        first_fixture.descriptor.canonical_bytes(),
        second_fixture.descriptor.canonical_bytes()
    );
    assert_eq!(
        first_fixture.canonical_module(),
        second_fixture.canonical_module()
    );

    let first =
        inspect_exact_lds_gemm_compiler_import_v1(first_fixture.pins(), first_fixture.handoff())
            .expect("canonical public fixture must be admitted");
    let second =
        inspect_exact_lds_gemm_compiler_import_v1(second_fixture.pins(), second_fixture.handoff())
            .expect("the same public fixture must be admitted again");

    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.contract().identity(), second.contract().identity());
    assert_eq!(first.kernel_ir(), &tiled_gemm_lds_v1_module());
    assert_eq!(first.canonical_llvm_body(), canonical_llvm_body());
    let canonical_kernel_ir = encode_module_v7(&tiled_gemm_lds_v1_module()).unwrap();
    assert_eq!(
        canonical_kernel_ir[8..10],
        KERNEL_IR_VERSION_V7.to_le_bytes()
    );
    assert_eq!(
        first.kernel_ir_identity().byte_len(),
        canonical_kernel_ir.len() as u64
    );
    assert_eq!(
        first.llvm_body_identity().byte_len(),
        first.canonical_llvm_body().len() as u64
    );
    assert_eq!(
        first.resource_transcript_identity().byte_len(),
        first_fixture.resources.len() as u64
    );
    assert_eq!(first.descriptor_source(), &first_fixture.descriptor);
    assert_eq!(first.source_authority(), &SOURCE_AUTHORITY);

    assert!(!first.authenticates_compiler_origin());
    assert!(!first.grants_worker_authority());
    assert!(!first.grants_link_authority());
    assert!(!first.grants_publication_authority());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
    assert!(!first.proves_verus_verification());
    assert!(!first.handoff().authenticates_compiler_origin());
    assert!(!first.handoff().grants_compiler_authority());
    assert!(!first.handoff().grants_worker_authority());
    assert!(!first.handoff().grants_link_authority());
    assert!(!first.handoff().grants_load_authority());
    assert!(!first.handoff().grants_launch_authority());
    assert!(!first.descriptor_source().authenticates_compiler_origin());
    assert!(!first.descriptor_source().grants_link_authority());
    assert!(!first.descriptor_source().grants_load_authority());
    assert!(!first.descriptor_source().grants_launch_authority());
}

#[test]
fn admitted_contract_separates_exact_role_lengths_and_effects() {
    let fixture = Slice1Fixture::canonical();
    let admitted =
        inspect_exact_lds_gemm_compiler_import_v1(fixture.pins(), fixture.handoff()).unwrap();
    let contract = admitted.contract();
    let [a, b, c] = contract.buffers();

    assert_eq!(contract.profile(), ExactLdsGemmProfileIdV1::Slice1M16N16K16);
    assert_eq!(contract.target(), TARGET);
    assert_eq!(contract.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(contract.grid(), [1, 1, 1]);
    assert_eq!(contract.workgroup(), [64, 1, 1]);
    assert_eq!(contract.wavefront_size(), 64);
    assert_eq!(
        (a.role(), b.role(), c.role()),
        (
            ExactLdsGemmBufferRoleV1::A,
            ExactLdsGemmBufferRoleV1::B,
            ExactLdsGemmBufferRoleV1::C,
        )
    );
    assert_eq!((a.elements(), b.elements(), c.elements()), (256, 256, 256));
    assert_eq!((a.bytes(), b.bytes(), c.bytes()), (512, 512, 1024));
    assert_eq!(
        (a.element(), b.element(), c.element()),
        (
            ExactLdsGemmElementV1::Bf16BitsU16,
            ExactLdsGemmElementV1::Bf16BitsU16,
            ExactLdsGemmElementV1::F32,
        )
    );
    assert_ne!(a.length_identity(), b.length_identity());
    assert_ne!(a.length_identity(), c.length_identity());
    assert_ne!(b.length_identity(), c.length_identity());
    assert_eq!(
        (a.ownership(), a.access(), a.alias()),
        (
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
        )
    );
    assert_eq!(
        (b.ownership(), b.access(), b.alias()),
        (
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
        )
    );
    assert_eq!(
        (c.ownership(), c.access(), c.alias()),
        (
            OwnershipSemantics::UniqueBorrow,
            AccessMode::ReadWrite,
            AliasSemantics::Exclusive,
        )
    );
}

#[test]
fn reserved_k32_grid_and_edges_manifests_fail_closed() {
    let fixture = Slice1Fixture::canonical();
    for (entry, profile) in [
        (
            TILED_GEMM_LDS_K32_V2_KERNEL_ID,
            ExactLdsGemmProfileIdV1::KPhaseM16N16K32,
        ),
        (
            TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
            ExactLdsGemmProfileIdV1::GridM64N48K16,
        ),
        (
            TILED_GEMM_LDS_EDGES_V1_KERNEL_ID,
            ExactLdsGemmProfileIdV1::EdgesM17N19K18,
        ),
    ] {
        assert_eq!(
            exact_lds_gemm_profile_availability_v1(profile),
            ExactLdsGemmProfileAvailabilityV1::Reserved
        );
        let handoff = fixture.handoff_with(
            fixture.target,
            fixture.code_object_version,
            fixture.envelope.clone(),
            manifest_for(entry),
            &fixture.canonical_module(),
        );
        assert!(matches!(
            inspect_exact_lds_gemm_compiler_import_v1(fixture.pins(), handoff),
            Err(ExactLdsGemmProfileAdmissionErrorV1::ReservedProfile(actual))
                if actual == profile
        ));
    }
}

#[test]
fn self_consistent_hostile_llvm_with_regenerated_evidence_is_rejected() {
    let fixture = Slice1Fixture::canonical();
    let mut hostile_body = fixture.llvm_body.clone();
    hostile_body.push_str("; hostile producer-controlled LLVM body\n");
    let hostile_descriptor = descriptor_source(&fixture.envelope, &hostile_body, 0x51);
    let hostile_resources = resource_transcript(
        &fixture.source_authority,
        hostile_descriptor.identity().sha256(),
    );
    let hostile_module = module_with_sections(
        &hostile_body,
        &[
            (
                COMPILER_DESCRIPTOR_SECTION_NAME_V1,
                hostile_descriptor.canonical_bytes(),
            ),
            (AUTHORITY_SECTION, &fixture.source_authority),
            (RESOURCE_SECTION, &hostile_resources),
        ],
    );
    let hostile_pins = ExactLdsGemmCompilerImportPinsV1::new(
        hostile_descriptor.identity(),
        fixture.source_authority,
    )
    .unwrap();

    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(
            hostile_pins,
            fixture.handoff_for_module(&hostile_module),
        ),
        Err(ExactLdsGemmProfileAdmissionErrorV1::LlvmBodyMismatch)
    ));
}

#[test]
fn descriptor_bytes_and_descriptor_source_pin_substitution_are_rejected() {
    let fixture = Slice1Fixture::canonical();
    let substituted_descriptor = descriptor_source(&fixture.envelope, &fixture.llvm_body, 0x52);
    let substituted_resources = resource_transcript(
        &fixture.source_authority,
        substituted_descriptor.identity().sha256(),
    );
    let substituted_module = module_with_sections(
        &fixture.llvm_body,
        &[
            (
                COMPILER_DESCRIPTOR_SECTION_NAME_V1,
                substituted_descriptor.canonical_bytes(),
            ),
            (AUTHORITY_SECTION, &fixture.source_authority),
            (RESOURCE_SECTION, &substituted_resources),
        ],
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(
            fixture.pins(),
            fixture.handoff_for_module(&substituted_module),
        ),
        Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorPinMismatch)
    ));

    let substituted_pin = ExactLdsGemmCompilerImportPinsV1::new(
        substituted_descriptor.identity(),
        fixture.source_authority,
    )
    .unwrap();
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(substituted_pin, fixture.handoff()),
        Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorPinMismatch)
    ));
}

#[test]
fn source_authority_and_resource_substitution_are_rejected() {
    let fixture = Slice1Fixture::canonical();
    let substituted_authority = [0xa6; 32];
    let authority_bound_resources = resource_transcript(
        &substituted_authority,
        fixture.descriptor.identity().sha256(),
    );
    let authority_substituted_module = module_with_sections(
        &fixture.llvm_body,
        &[
            (
                COMPILER_DESCRIPTOR_SECTION_NAME_V1,
                fixture.descriptor.canonical_bytes(),
            ),
            (AUTHORITY_SECTION, &substituted_authority),
            (RESOURCE_SECTION, &authority_bound_resources),
        ],
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(
            fixture.pins(),
            fixture.handoff_for_module(&authority_substituted_module),
        ),
        Err(ExactLdsGemmProfileAdmissionErrorV1::SourceAuthorityMismatch)
    ));

    let mut substituted_resources = fixture.resources.clone();
    let last = substituted_resources.last_mut().expect("resource bytes");
    *last ^= 1;
    let resource_substituted_module = module_with_sections(
        &fixture.llvm_body,
        &[
            (
                COMPILER_DESCRIPTOR_SECTION_NAME_V1,
                fixture.descriptor.canonical_bytes(),
            ),
            (AUTHORITY_SECTION, &fixture.source_authority),
            (RESOURCE_SECTION, &substituted_resources),
        ],
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(
            fixture.pins(),
            fixture.handoff_for_module(&resource_substituted_module),
        ),
        Err(ExactLdsGemmProfileAdmissionErrorV1::ResourceTranscriptMismatch)
    ));
}

#[test]
fn duplicate_reordered_and_trailing_assembly_sections_are_rejected() {
    let fixture = Slice1Fixture::canonical();
    let descriptor = fixture.descriptor.canonical_bytes();
    let duplicate = module_with_sections(
        &fixture.llvm_body,
        &[
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor),
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor),
            (AUTHORITY_SECTION, &fixture.source_authority),
            (RESOURCE_SECTION, &fixture.resources),
        ],
    );
    let reordered = module_with_sections(
        &fixture.llvm_body,
        &[
            (AUTHORITY_SECTION, &fixture.source_authority),
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor),
            (RESOURCE_SECTION, &fixture.resources),
        ],
    );
    let trailing = module_with_sections(
        &fixture.llvm_body,
        &[
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, descriptor),
            (AUTHORITY_SECTION, &fixture.source_authority),
            (RESOURCE_SECTION, &fixture.resources),
            (".fe2o3.trailing.v1", &[0x7f]),
        ],
    );

    for module in [duplicate, reordered, trailing] {
        assert!(matches!(
            inspect_exact_lds_gemm_compiler_import_v1(
                fixture.pins(),
                fixture.handoff_for_module(&module),
            ),
            Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(_))
        ));
    }
}

#[test]
fn target_cov_and_manifest_substitution_are_rejected() {
    let fixture = Slice1Fixture::canonical();
    let module = fixture.canonical_module();

    let other_target = DeviceTargetV1::parse("gfx950:xnack-").expect("alternate target");
    let target_envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(other_target, CodeObjectVersion::V6)
            .unwrap();
    let target_handoff = fixture.handoff_with(
        other_target,
        CodeObjectVersion::V6,
        target_envelope,
        fixture.manifest.clone(),
        &module,
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(fixture.pins(), target_handoff),
        Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField("target"))
    ));

    let cov_envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(fixture.target, CodeObjectVersion::V5)
            .unwrap();
    let cov_handoff = fixture.handoff_with(
        fixture.target,
        CodeObjectVersion::V5,
        cov_envelope,
        fixture.manifest.clone(),
        &module,
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(fixture.pins(), cov_handoff),
        Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField(
            "code-object version"
        ))
    ));

    let substituted_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            TILED_GEMM_LDS_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "tiled_gemm_lds_v1.not-kd",
        ),
    ])
    .unwrap();
    let manifest_handoff = fixture.handoff_with(
        fixture.target,
        fixture.code_object_version,
        fixture.envelope.clone(),
        substituted_manifest,
        &module,
    );
    assert!(matches!(
        inspect_exact_lds_gemm_compiler_import_v1(fixture.pins(), manifest_handoff),
        Err(ExactLdsGemmProfileAdmissionErrorV1::UnsupportedManifest)
    ));
}
