use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_compiler_ffi::{
    CodeObjectVersion, DeviceTargetV1, EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1,
    EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1, ExternalDeviceAddressSpaceV1,
    ExternalDeviceBlobIdentityV1, ExternalDeviceCallingConventionV1,
    ExternalDeviceCapabilityIdentityV1, ExternalDeviceConvergenceV1,
    ExternalDeviceLibraryContentIdentityV1, ExternalDeviceLibraryContentKindV1,
    ExternalDeviceLibraryDependencyV1, ExternalDeviceLibraryManifestErrorV1,
    ExternalDeviceLibraryManifestIdentityV1, ExternalDeviceLibraryManifestV1,
    ExternalDeviceLibraryProvenanceKindV1, ExternalDeviceLibraryProvenanceV1,
    ExternalDeviceLibraryTrustClassV1, ExternalDeviceLibraryTrustV1, ExternalDeviceLlvmIdentityV1,
    ExternalDeviceSemanticIdentityV1, ExternalDeviceSymbolRoleV1, ExternalDeviceSymbolV1,
    MAX_EXTERNAL_DEVICE_LIBRARY_CAPABILITIES_V1, MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1,
    MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1, MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1,
};
use sha2::{Digest, Sha256};

const IMPORT_ABI: &str = "C(const_ptr<global,u32>[size=8,align=8,as=global])->u32[size=4,align=4]";
const SCALAR_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

fn digest(label: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(label.as_ref()).into()
}

fn blob(label: &str, len: u64) -> ExternalDeviceBlobIdentityV1 {
    ExternalDeviceBlobIdentityV1::new(digest(label), len).unwrap()
}

fn content(label: &str) -> ExternalDeviceLibraryContentIdentityV1 {
    ExternalDeviceLibraryContentIdentityV1::new(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        blob(label, 4_096),
    )
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn llvm(version: &str) -> ExternalDeviceLlvmIdentityV1 {
    ExternalDeviceLlvmIdentityV1::new(
        18,
        version,
        [0x18; 20],
        blob("llvm-executable", 1_048_576),
        EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
        EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1,
    )
    .unwrap()
}

fn capability(label: &str) -> ExternalDeviceCapabilityIdentityV1 {
    ExternalDeviceCapabilityIdentityV1::new(digest(label)).unwrap()
}

fn semantic(label: &str) -> ExternalDeviceSemanticIdentityV1 {
    ExternalDeviceSemanticIdentityV1::new(digest(label)).unwrap()
}

fn symbol(
    role: ExternalDeviceSymbolRoleV1,
    name: &str,
    abi: &str,
    effects: &str,
    convergence: ExternalDeviceConvergenceV1,
    capabilities: Vec<ExternalDeviceCapabilityIdentityV1>,
) -> ExternalDeviceSymbolV1 {
    ExternalDeviceSymbolV1::new(
        role,
        name,
        abi,
        effects,
        convergence,
        semantic(&format!("semantic:{role:?}:{name}")),
        capabilities,
    )
    .unwrap()
}

fn symbols() -> Vec<ExternalDeviceSymbolV1> {
    vec![
        symbol(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "external_add",
            IMPORT_ABI,
            "read_global",
            ExternalDeviceConvergenceV1::Unconstrained,
            vec![capability("cap:global-memory")],
        ),
        symbol(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "external_mul",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            vec![],
        ),
        symbol(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "rust_helper",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Convergent,
            vec![capability("cap:device-call")],
        ),
    ]
}

fn manifest_id(label: &str) -> ExternalDeviceLibraryManifestIdentityV1 {
    ExternalDeviceLibraryManifestIdentityV1::new(digest(label), 2_048).unwrap()
}

fn dependency(
    manifest_label: &str,
    content_label: &str,
    imports: &[&str],
) -> ExternalDeviceLibraryDependencyV1 {
    ExternalDeviceLibraryDependencyV1::new(
        manifest_id(manifest_label),
        content(content_label),
        imports.iter().map(|value| (*value).to_owned()).collect(),
    )
    .unwrap()
}

fn dependencies() -> Vec<ExternalDeviceLibraryDependencyV1> {
    let mut values = vec![
        dependency("provider:add", "provider-content:add", &["external_add"]),
        dependency("provider:mul", "provider-content:mul", &["external_mul"]),
    ];
    values.sort_by_key(|value| value.manifest_identity());
    values
}

fn build(
    own_content: ExternalDeviceLibraryContentIdentityV1,
    code_object_version: CodeObjectVersion,
    llvm: ExternalDeviceLlvmIdentityV1,
    trust: ExternalDeviceLibraryTrustV1,
    symbols: Vec<ExternalDeviceSymbolV1>,
    dependencies: Vec<ExternalDeviceLibraryDependencyV1>,
) -> Result<ExternalDeviceLibraryManifestV1, ExternalDeviceLibraryManifestErrorV1> {
    ExternalDeviceLibraryManifestV1::new(
        own_content,
        target(),
        code_object_version,
        llvm,
        ExternalDeviceLibraryProvenanceV1::new(
            ExternalDeviceLibraryProvenanceKindV1::VendorSdk,
            digest("rocm-sdk:7.2.4"),
        )
        .unwrap(),
        trust,
        symbols,
        dependencies,
    )
}

fn manifest() -> ExternalDeviceLibraryManifestV1 {
    build(
        content("library-content"),
        CodeObjectVersion::V6,
        llvm("18.1.8"),
        ExternalDeviceLibraryTrustV1::declared_specification(digest("rocwmma-contract-v1"))
            .unwrap(),
        symbols(),
        dependencies(),
    )
    .unwrap()
}

#[test]
fn public_round_trip_binds_every_contract_layer_without_authority() {
    let original = manifest();
    let decoded = ExternalDeviceLibraryManifestV1::decode(original.canonical_bytes()).unwrap();
    let via_try_from =
        ExternalDeviceLibraryManifestV1::try_from(original.canonical_bytes()).unwrap();
    let bound = ExternalDeviceLibraryManifestV1::decode_for(
        original.identity(),
        original.canonical_bytes(),
    )
    .unwrap();

    assert_eq!(decoded, original);
    assert_eq!(via_try_from, original);
    assert_eq!(bound, original);
    assert_eq!(original.target(), target());
    assert_eq!(original.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        original.content().kind(),
        ExternalDeviceLibraryContentKindV1::LlvmBitcode
    );
    assert_eq!(original.llvm().major(), 18);
    assert_eq!(original.llvm().version(), "18.1.8");
    assert_eq!(
        original.llvm().target_triple(),
        EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1
    );
    assert_eq!(
        original.provenance().kind(),
        ExternalDeviceLibraryProvenanceKindV1::VendorSdk
    );
    assert_eq!(
        original.trust().class(),
        ExternalDeviceLibraryTrustClassV1::DeclaredSpecification
    );
    assert_eq!(original.symbols().len(), 3);
    assert_eq!(original.imports().count(), 2);
    assert_eq!(original.exports().count(), 1);
    assert_eq!(original.dependencies().len(), 2);
    assert_eq!(
        original.symbols()[0].calling_convention(),
        ExternalDeviceCallingConventionV1::C
    );
    assert_eq!(
        original.symbols()[0].address_spaces(),
        &[ExternalDeviceAddressSpaceV1::Global]
    );
    assert_eq!(original.dependencies()[0].resolved_imports().count(), 1);
    assert!(original.identity().matches(original.canonical_bytes()));
    assert!(!original.trust().authenticates_evidence());
    assert!(!original.authenticates_provenance());
    assert!(!original.authenticates_verification());
    assert!(!original.grants_link_authority());
    assert!(!original.grants_load_authority());
    assert!(!original.grants_launch_authority());
}

#[test]
fn exact_blob_identity_checks_content_bytes_and_representation() {
    let bytes = b"exact llvm bitcode bytes";
    let exact = ExternalDeviceBlobIdentityV1::calculate(bytes).unwrap();
    let content = ExternalDeviceLibraryContentIdentityV1::new(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        exact,
    );
    assert!(content.matches(bytes));
    assert!(!content.matches(b"exact llvm bitcode byteS"));
    assert_eq!(content.blob().byte_len(), bytes.len() as u64);
    assert_eq!(
        content.kind(),
        ExternalDeviceLibraryContentKindV1::LlvmBitcode
    );
}

#[test]
fn stable_manifest_identity_and_separate_wire_domain_are_golden() {
    let value = manifest();
    assert!(
        value
            .canonical_bytes()
            .starts_with(b"FE2O3/EXTERNAL-DEVICE-LIBRARY-MANIFEST/V1\0")
    );
    assert!(
        !value
            .canonical_bytes()
            .starts_with(b"FE2O3/COMPILER-MODULE-HANDOFF/V1\0")
    );
    assert_eq!(
        (value.identity().byte_len(), *value.identity().sha256()),
        (
            1_135,
            [
                0xfa, 0x3f, 0xdb, 0x08, 0xc7, 0xed, 0x8e, 0x5d, 0x67, 0xf3, 0x8f, 0x8d, 0xe7, 0x87,
                0x03, 0x52, 0x47, 0x32, 0x5a, 0x4b, 0xef, 0xa5, 0xea, 0x01, 0xd6, 0xf1, 0x2b, 0x40,
                0xef, 0xa1, 0x3a, 0x07,
            ]
        )
    );
}

#[test]
fn identities_llvm_text_and_trust_evidence_are_strictly_bounded() {
    assert_eq!(
        ExternalDeviceBlobIdentityV1::new(digest("empty"), 0),
        Err(ExternalDeviceLibraryManifestErrorV1::EmptyBlob)
    );
    assert_eq!(
        ExternalDeviceBlobIdentityV1::new([0; 32], 1),
        Err(ExternalDeviceLibraryManifestErrorV1::MissingIdentity)
    );
    assert_eq!(
        ExternalDeviceSemanticIdentityV1::new([0; 32]),
        Err(ExternalDeviceLibraryManifestErrorV1::MissingIdentity)
    );
    assert_eq!(
        ExternalDeviceLibraryTrustV1::external_attestation([0; 32]),
        Err(ExternalDeviceLibraryManifestErrorV1::MissingIdentity)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            0,
            "18.1.8",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmIdentity)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "x".repeat(MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1 + 1),
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidText)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "18.1.8 with space",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidText)
    );

    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "19.0.0git",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::LlvmVersionMajorMismatch)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "18vendor",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmIdentity)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "18.0.0git",
            [1; 20],
            blob("llvm", 1),
            "nvptx64-nvidia-cuda",
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmTargetTriple)
    );
    assert_eq!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "18.0.0git",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            "e-p:32:32"
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmDataLayout)
    );
    assert!(
        ExternalDeviceLlvmIdentityV1::new(
            18,
            "18.99.123git",
            [1; 20],
            blob("llvm", 1),
            EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
            EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        )
        .is_ok()
    );
}

#[test]
fn reviewed_target_content_and_code_object_profiles_fail_closed() {
    let make = |content, target, cov| {
        ExternalDeviceLibraryManifestV1::new(
            content,
            target,
            cov,
            llvm("18.1.8"),
            ExternalDeviceLibraryProvenanceV1::new(
                ExternalDeviceLibraryProvenanceKindV1::VendorSdk,
                digest("profile-provenance"),
            )
            .unwrap(),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            dependencies(),
        )
    };
    assert_eq!(
        make(content("library-content"), target(), CodeObjectVersion::V4),
        Err(ExternalDeviceLibraryManifestErrorV1::UnsupportedContentCodeObjectCombination)
    );
    assert_eq!(
        make(
            ExternalDeviceLibraryContentIdentityV1::new(
                ExternalDeviceLibraryContentKindV1::CodeObject,
                blob("library-content", 4_096),
            ),
            target(),
            CodeObjectVersion::V6,
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::UnsupportedContentCodeObjectCombination)
    );
    assert_eq!(
        make(
            content("library-content"),
            DeviceTargetV1::parse("gfx950:xnack-").unwrap(),
            CodeObjectVersion::V6,
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::UnsupportedTargetProfile)
    );
}

#[test]
fn ffi_grammar_capability_order_and_address_spaces_fail_closed() {
    assert!(matches!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "invalid symbol",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("invalid-symbol"),
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::FfiGrammar(_))
    ));
    assert!(matches!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "read_without_pointer",
            SCALAR_ABI,
            "read_global",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("effect-mismatch"),
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::FfiGrammar(_))
    ));

    let mut reversed = vec![capability("cap:z"), capability("cap:a")];
    reversed.sort();
    reversed.reverse();
    assert_eq!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "helper",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("helper"),
            reversed
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalCapabilityOrder)
    );
    let duplicate = capability("same");
    assert_eq!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "helper",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("helper"),
            vec![duplicate, duplicate]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalCapabilityOrder)
    );
}

#[test]
fn workgroup_barrier_effects_require_convergent_contracts() {
    assert_eq!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "barrier",
            SCALAR_ABI,
            "barrier_workgroup",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("barrier"),
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::BarrierRequiresConvergence)
    );
    assert!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "barrier",
            SCALAR_ABI,
            "barrier_workgroup",
            ExternalDeviceConvergenceV1::Convergent,
            semantic("barrier"),
            vec![]
        )
        .is_ok()
    );
}

#[test]
fn symbol_order_names_and_semantics_are_unique() {
    let mut reversed = symbols();
    reversed.reverse();
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            reversed,
            dependencies()
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalSymbolOrder)
    );

    let duplicate_name = vec![
        symbol(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "same",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            vec![],
        ),
        symbol(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "same",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            vec![],
        ),
    ];
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            duplicate_name,
            vec![dependency(
                "provider:same",
                "provider-content:same",
                &["same"]
            )]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::DuplicateSymbol)
    );

    let shared_semantic = semantic("shared");
    let duplicate_semantic = vec![
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "a",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            shared_semantic,
            vec![],
        )
        .unwrap(),
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "b",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            shared_semantic,
            vec![],
        )
        .unwrap(),
    ];
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            duplicate_semantic,
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::DuplicateSemanticIdentity)
    );
}

#[test]
fn dependency_closure_is_exact_unique_and_deterministic() {
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::MissingResolvedImport)
    );

    let unexpected = vec![dependency(
        "provider:unexpected",
        "provider-content:unexpected",
        &["not_an_import"],
    )];
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            unexpected
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::UnexpectedResolvedImport)
    );

    let mut duplicate_provider = vec![
        dependency("provider:1", "provider-content:1", &["external_add"]),
        dependency(
            "provider:2",
            "provider-content:2",
            &["external_add", "external_mul"],
        ),
    ];
    duplicate_provider.sort_by_key(|value| value.manifest_identity());
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            duplicate_provider
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::DuplicateResolvedImport)
    );

    let self_content = vec![
        ExternalDeviceLibraryDependencyV1::new(
            manifest_id("self"),
            ExternalDeviceLibraryContentIdentityV1::new(
                ExternalDeviceLibraryContentKindV1::RelocatableObject,
                content("library-content").blob(),
            ),
            vec!["external_add".into(), "external_mul".into()],
        )
        .unwrap(),
    ];
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            self_content
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::SelfDependencyContent)
    );
}

#[test]
fn duplicate_and_reordered_dependencies_are_rejected() {
    let first = dependency("provider:shared", "provider-content:a", &["external_add"]);
    let duplicate_manifest = ExternalDeviceLibraryDependencyV1::new(
        first.manifest_identity(),
        content("provider-content:b"),
        vec!["external_mul".into()],
    )
    .unwrap();
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            vec![first, duplicate_manifest]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::DuplicateDependencyManifest)
    );

    let shared_blob = content("shared-provider-content").blob();
    let mut duplicate_content = vec![
        ExternalDeviceLibraryDependencyV1::new(
            manifest_id("provider:a"),
            ExternalDeviceLibraryContentIdentityV1::new(
                ExternalDeviceLibraryContentKindV1::LlvmBitcode,
                shared_blob,
            ),
            vec!["external_add".into()],
        )
        .unwrap(),
        ExternalDeviceLibraryDependencyV1::new(
            manifest_id("provider:b"),
            ExternalDeviceLibraryContentIdentityV1::new(
                ExternalDeviceLibraryContentKindV1::RelocatableObject,
                shared_blob,
            ),
            vec!["external_mul".into()],
        )
        .unwrap(),
    ];
    duplicate_content.sort_by_key(|value| value.manifest_identity());
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            duplicate_content
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::DuplicateDependencyContent)
    );

    let mut reversed = dependencies();
    reversed.reverse();
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            symbols(),
            reversed
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalDependencyOrder)
    );
}

#[test]
fn count_bounds_are_checked_before_canonical_allocation() {
    let capabilities = (0..=MAX_EXTERNAL_DEVICE_LIBRARY_CAPABILITIES_V1)
        .map(|index| capability(&format!("capability:{index:04}")))
        .collect();
    assert_eq!(
        ExternalDeviceSymbolV1::new(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "too_many_capabilities",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            semantic("too-many-capabilities"),
            capabilities
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::TooManyCapabilities)
    );

    let too_many_symbols = (0..=MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1)
        .map(|index| {
            ExternalDeviceSymbolV1::new(
                ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
                format!("symbol_{index:04}"),
                SCALAR_ABI,
                "none",
                ExternalDeviceConvergenceV1::Unconstrained,
                semantic(&format!("symbol-semantic:{index}")),
                vec![],
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            too_many_symbols,
            vec![]
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::TooManySymbols)
    );

    let mut too_many_dependencies = (0..=MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1)
        .map(|index| {
            ExternalDeviceLibraryDependencyV1::new(
                ExternalDeviceLibraryManifestIdentityV1::new(
                    digest(format!("manifest:{index}")),
                    1_024,
                )
                .unwrap(),
                content(&format!("dependency-content:{index}")),
                vec![],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    too_many_dependencies.sort_by_key(|value| value.manifest_identity());
    let export_only = vec![symbol(
        ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
        "only_export",
        SCALAR_ABI,
        "none",
        ExternalDeviceConvergenceV1::Unconstrained,
        vec![],
    )];
    assert_eq!(
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::unverified(),
            export_only,
            too_many_dependencies
        ),
        Err(ExternalDeviceLibraryManifestErrorV1::TooManyDependencies)
    );
}

#[test]
fn every_truncation_trailing_byte_and_expected_identity_substitution_is_rejected() {
    let value = manifest();
    let bytes = value.canonical_bytes();
    for length in 0..bytes.len() {
        assert!(
            ExternalDeviceLibraryManifestV1::decode(&bytes[..length]).is_err(),
            "accepted prefix of length {length}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        ExternalDeviceLibraryManifestV1::decode(&trailing),
        Err(ExternalDeviceLibraryManifestErrorV1::TrailingBytes)
    );
    let wrong_identity = ExternalDeviceLibraryManifestIdentityV1::new(digest("wrong"), 1).unwrap();
    assert_eq!(
        ExternalDeviceLibraryManifestV1::decode_for(wrong_identity, bytes),
        Err(ExternalDeviceLibraryManifestErrorV1::ManifestIdentityMismatch)
    );

    let oversized = vec![0; fe2o3_compiler_ffi::MAX_EXTERNAL_DEVICE_LIBRARY_MANIFEST_BYTES_V1 + 1];
    assert_eq!(
        ExternalDeviceLibraryManifestV1::decode_for(value.identity(), &oversized),
        Err(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)
    );
}

#[test]
fn every_single_byte_mutation_is_total_canonical_when_accepted_and_loses_binding() {
    let value = manifest();
    let bytes = value.canonical_bytes().to_vec();
    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0x01;
        assert_eq!(
            ExternalDeviceLibraryManifestV1::decode_for(value.identity(), &mutated),
            Err(ExternalDeviceLibraryManifestErrorV1::ManifestIdentityMismatch),
            "mutation at byte {index} retained the original identity"
        );
        let result = catch_unwind(AssertUnwindSafe(|| {
            ExternalDeviceLibraryManifestV1::decode(&mutated)
        }));
        assert!(
            result.is_ok(),
            "decoder panicked for mutation at byte {index}"
        );
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(decoded.canonical_bytes(), mutated);
            assert_ne!(decoded.identity(), value.identity());
        }
    }
}

#[test]
fn encoded_address_spaces_are_cross_checked_against_c_abi() {
    let value = manifest();
    let mut bytes = value.canonical_bytes().to_vec();
    let name = b"external_add";
    let name_start = bytes
        .windows(name.len())
        .position(|window| window == name)
        .unwrap();
    let abi_len_start = name_start + name.len();
    let abi_len =
        u32::from_le_bytes(bytes[abi_len_start..abi_len_start + 4].try_into().unwrap()) as usize;
    let address_count = abi_len_start + 4 + abi_len;
    assert_eq!(bytes[address_count], 1);
    assert_eq!(
        bytes[address_count + 1],
        ExternalDeviceAddressSpaceV1::Global as u8
    );
    bytes[address_count + 1] = ExternalDeviceAddressSpaceV1::Workgroup as u8;
    assert_eq!(
        ExternalDeviceLibraryManifestV1::decode(&bytes),
        Err(ExternalDeviceLibraryManifestErrorV1::AddressSpaceMismatch)
    );
}

#[test]
fn kernel_entry_role_tag_is_not_part_of_v1() {
    let value = manifest();
    let mut bytes = value.canonical_bytes().to_vec();
    let name = b"external_add";
    let name_start = bytes
        .windows(name.len())
        .position(|window| window == name)
        .unwrap();
    let role_offset = name_start - 7;
    assert_eq!(
        bytes[role_offset],
        ExternalDeviceSymbolRoleV1::DeviceFunctionImport as u8
    );
    bytes[role_offset] = 3;
    assert_eq!(
        ExternalDeviceLibraryManifestV1::decode(&bytes),
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag)
    );
}

#[test]
fn meaningful_valid_field_mutations_change_the_manifest_identity() {
    let original = manifest();
    let mutations = [
        build(
            ExternalDeviceLibraryContentIdentityV1::new(
                ExternalDeviceLibraryContentKindV1::RelocatableObject,
                blob("library-content", 4_096),
            ),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::declared_specification(digest("rocwmma-contract-v1"))
                .unwrap(),
            symbols(),
            dependencies(),
        )
        .unwrap(),
        build(
            content("library-content"),
            CodeObjectVersion::V5,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::declared_specification(digest("rocwmma-contract-v1"))
                .unwrap(),
            symbols(),
            dependencies(),
        )
        .unwrap(),
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.9"),
            ExternalDeviceLibraryTrustV1::declared_specification(digest("rocwmma-contract-v1"))
                .unwrap(),
            symbols(),
            dependencies(),
        )
        .unwrap(),
        build(
            content("library-content"),
            CodeObjectVersion::V6,
            llvm("18.1.8"),
            ExternalDeviceLibraryTrustV1::external_attestation(digest("attestation-v1")).unwrap(),
            symbols(),
            dependencies(),
        )
        .unwrap(),
    ];
    for mutation in mutations {
        assert_ne!(mutation.identity(), original.identity());
        assert_ne!(mutation.canonical_bytes(), original.canonical_bytes());
    }
}

#[test]
fn debug_output_does_not_expose_symbols_abi_or_tool_paths() {
    let debug = format!("{:?}", manifest());
    for sensitive in [
        "external_add",
        "external_mul",
        "rust_helper",
        "const_ptr",
        EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
        "/opt/rocm",
        "--library-path",
    ] {
        assert!(
            !debug.contains(sensitive),
            "debug output exposed `{sensitive}`: {debug}"
        );
    }
}
