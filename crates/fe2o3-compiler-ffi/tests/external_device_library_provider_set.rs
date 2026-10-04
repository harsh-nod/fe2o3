use fe2o3_compiler_ffi::{
    CodeObjectVersion, DeviceTargetV1, EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1,
    EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1, ExternalDeviceBlobIdentityV1,
    ExternalDeviceCapabilityIdentityV1, ExternalDeviceConvergenceV1,
    ExternalDeviceLibraryContentIdentityV1, ExternalDeviceLibraryContentKindV1,
    ExternalDeviceLibraryDependencyV1, ExternalDeviceLibraryManifestV1,
    ExternalDeviceLibraryProvenanceKindV1, ExternalDeviceLibraryProvenanceV1,
    ExternalDeviceLibraryProviderSetErrorV1, ExternalDeviceLibraryProviderV1,
    ExternalDeviceLibraryTrustV1, ExternalDeviceLlvmIdentityV1, ExternalDeviceSemanticIdentityV1,
    ExternalDeviceSymbolRoleV1, ExternalDeviceSymbolV1,
    MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1,
};
use sha2::{Digest, Sha256};

const GLOBAL_ABI: &str = "C(const_ptr<global,u32>[size=8,align=8,as=global])->u32[size=4,align=4]";
const WORKGROUP_ABI: &str =
    "C(const_ptr<workgroup,u32>[size=8,align=8,as=workgroup])->u32[size=4,align=4]";
const SCALAR_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

struct OwnedProvider {
    manifest: ExternalDeviceLibraryManifestV1,
    bytes: Vec<u8>,
}

fn digest(value: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(value.as_ref()).into()
}

fn bitcode(label: &str) -> Vec<u8> {
    let mut bytes = vec![b'B', b'C', 0xc0, 0xde];
    bytes.extend_from_slice(label.as_bytes());
    bytes
}

fn relocatable(label: &str) -> Vec<u8> {
    let mut bytes = vec![0; 64 + label.len()];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[16..18].copy_from_slice(&1_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&224_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[64..].copy_from_slice(label.as_bytes());
    bytes
}

fn content(
    kind: ExternalDeviceLibraryContentKindV1,
    bytes: &[u8],
) -> ExternalDeviceLibraryContentIdentityV1 {
    ExternalDeviceLibraryContentIdentityV1::new(
        kind,
        ExternalDeviceBlobIdentityV1::calculate(bytes).unwrap(),
    )
}

fn target(value: &str) -> DeviceTargetV1 {
    DeviceTargetV1::parse(value).unwrap()
}

fn llvm(major: u16, version: &str, label: &str) -> ExternalDeviceLlvmIdentityV1 {
    ExternalDeviceLlvmIdentityV1::new(
        major,
        version,
        [label.as_bytes()[0]; 20],
        ExternalDeviceBlobIdentityV1::new(digest(format!("llvm:{label}")), 10_000).unwrap(),
        EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1,
        EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1,
    )
    .unwrap()
}

fn capability(label: &str) -> ExternalDeviceCapabilityIdentityV1 {
    ExternalDeviceCapabilityIdentityV1::new(digest(format!("capability:{label}"))).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn function(
    role: ExternalDeviceSymbolRoleV1,
    name: &str,
    abi: &str,
    effects: &str,
    convergence: ExternalDeviceConvergenceV1,
    semantic_label: &str,
    capabilities: &[&str],
) -> ExternalDeviceSymbolV1 {
    let mut capabilities = capabilities
        .iter()
        .map(|label| capability(label))
        .collect::<Vec<_>>();
    capabilities.sort_unstable();
    ExternalDeviceSymbolV1::new(
        role,
        name,
        abi,
        effects,
        convergence,
        ExternalDeviceSemanticIdentityV1::new(digest(format!("semantic:{semantic_label}")))
            .unwrap(),
        capabilities,
    )
    .unwrap()
}

fn baseline_function(role: ExternalDeviceSymbolRoleV1, name: &str) -> ExternalDeviceSymbolV1 {
    function(
        role,
        name,
        GLOBAL_ABI,
        "read_global",
        ExternalDeviceConvergenceV1::Unconstrained,
        name,
        &["global-memory"],
    )
}

#[allow(clippy::too_many_arguments)]
fn manifest_for(
    kind: ExternalDeviceLibraryContentKindV1,
    bytes: &[u8],
    target: DeviceTargetV1,
    cov: CodeObjectVersion,
    llvm: ExternalDeviceLlvmIdentityV1,
    mut symbols: Vec<ExternalDeviceSymbolV1>,
    mut dependencies: Vec<ExternalDeviceLibraryDependencyV1>,
) -> ExternalDeviceLibraryManifestV1 {
    symbols
        .sort_by(|left, right| (left.role(), left.symbol()).cmp(&(right.role(), right.symbol())));
    dependencies.sort_by_key(ExternalDeviceLibraryDependencyV1::manifest_identity);
    ExternalDeviceLibraryManifestV1::new(
        content(kind, bytes),
        target,
        cov,
        llvm,
        ExternalDeviceLibraryProvenanceV1::new(
            ExternalDeviceLibraryProvenanceKindV1::SourceBuild,
            digest("provider-set-provenance"),
        )
        .unwrap(),
        ExternalDeviceLibraryTrustV1::unverified(),
        symbols,
        dependencies,
    )
    .unwrap()
}

fn dependency(
    provider: &ExternalDeviceLibraryManifestV1,
    imports: &[&str],
) -> ExternalDeviceLibraryDependencyV1 {
    ExternalDeviceLibraryDependencyV1::new(
        provider.identity(),
        provider.content(),
        imports.iter().map(|name| (*name).to_owned()).collect(),
    )
    .unwrap()
}

fn provider(
    label: &str,
    export: ExternalDeviceSymbolV1,
    target_value: &str,
    cov: CodeObjectVersion,
    llvm_identity: ExternalDeviceLlvmIdentityV1,
) -> OwnedProvider {
    let bytes = bitcode(label);
    let manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &bytes,
        target(target_value),
        cov,
        llvm_identity,
        vec![export],
        vec![],
    );
    OwnedProvider { manifest, bytes }
}

fn direct_fixture() -> (ExternalDeviceLibraryManifestV1, Vec<OwnedProvider>) {
    let add = provider(
        "provider-add",
        baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external_add",
        ),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.99.0git", "add"),
    );
    let mul = provider(
        "provider-mul",
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external_mul",
            SCALAR_ABI,
            "none",
            ExternalDeviceConvergenceV1::Convergent,
            "external_mul",
            &[],
        ),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.0.0git", "mul"),
    );
    let root_bytes = bitcode("root");
    let root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "root"),
        vec![
            baseline_function(
                ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
                "external_add",
            ),
            function(
                ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
                "external_mul",
                SCALAR_ABI,
                "none",
                ExternalDeviceConvergenceV1::Convergent,
                "external_mul",
                &[],
            ),
        ],
        vec![
            dependency(&add.manifest, &["external_add"]),
            dependency(&mul.manifest, &["external_mul"]),
        ],
    );
    (root, vec![add, mul])
}

fn provider_views(values: &[OwnedProvider]) -> Vec<ExternalDeviceLibraryProviderV1<'_>> {
    values
        .iter()
        .map(|value| ExternalDeviceLibraryProviderV1::new(&value.manifest, &value.bytes).unwrap())
        .collect()
}

#[test]
fn actual_provider_set_validates_exact_content_and_complete_contracts() {
    let (root, providers) = direct_fixture();
    let mut views = provider_views(&providers);
    views.reverse();
    let validation = root.validate_provider_set(&views).unwrap();

    assert_eq!(validation.root_manifest_identity(), root.identity());
    assert_eq!(validation.providers().count(), 2);
    assert!(!validation.authenticates_provider_origin());
    assert!(!validation.authenticates_verification());
    assert!(!validation.grants_link_authority());
    assert!(!validation.grants_load_authority());
    assert!(!validation.grants_launch_authority());
    assert!(!views[0].authenticates_provider_origin());
    assert!(!views[0].grants_link_authority());
    assert_eq!(views[0].content_bytes(), providers[1].bytes);
}

#[test]
fn transitive_provider_imports_are_checked_against_the_same_actual_closure() {
    let leaf = provider(
        "leaf",
        baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "leaf_fn"),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.2.0git", "leaf"),
    );
    let mid_bytes = bitcode("mid");
    let mid_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &mid_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.3.4", "mid"),
        vec![
            baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionImport, "leaf_fn"),
            baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "mid_fn"),
        ],
        vec![dependency(&leaf.manifest, &["leaf_fn"])],
    );
    let mid = OwnedProvider {
        manifest: mid_manifest,
        bytes: mid_bytes,
    };
    let root_bytes = bitcode("transitive-root");
    let root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.1", "transitive-root"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "mid_fn",
        )],
        vec![
            dependency(&mid.manifest, &["mid_fn"]),
            dependency(&leaf.manifest, &[]),
        ],
    );
    let providers = vec![mid, leaf];
    assert!(
        root.validate_provider_set(&provider_views(&providers))
            .is_ok()
    );
}

#[test]
fn provider_set_checks_exact_digest_after_construction_preflight() {
    let (root, providers) = direct_fixture();
    let mut mutated = providers[0].bytes.clone();
    *mutated.last_mut().unwrap() ^= 1;
    let mutated_provider =
        ExternalDeviceLibraryProviderV1::new(&providers[0].manifest, &mutated).unwrap();
    let valid_provider =
        ExternalDeviceLibraryProviderV1::new(&providers[1].manifest, &providers[1].bytes).unwrap();
    assert_eq!(
        root.validate_provider_set(&[mutated_provider, valid_provider]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentDigestMismatch)
    );
}

#[test]
fn provider_construction_is_only_a_representation_header_preflight() {
    let header_only = bitcode("arbitrary-tail-not-reader-validated");
    let header_only_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &header_only,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "header-only"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "header_only",
        )],
        vec![],
    );
    assert!(
        ExternalDeviceLibraryProviderV1::new(&header_only_manifest, &header_only).is_ok(),
        "the constructor intentionally checks only the recognizable bitcode header"
    );

    let invalid = b"not llvm bitcode".to_vec();
    let invalid_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &invalid,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "invalid"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "invalid",
        )],
        vec![],
    );
    assert!(matches!(
        ExternalDeviceLibraryProviderV1::new(&invalid_manifest, &invalid),
        Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentHeaderMismatch)
    ));

    let object = relocatable("valid-object");
    let object_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::RelocatableObject,
        &object,
        target("gfx942:xnack-"),
        CodeObjectVersion::V5,
        llvm(18, "18.0.0git", "object"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "object_export",
        )],
        vec![],
    );
    assert!(ExternalDeviceLibraryProviderV1::new(&object_manifest, &object).is_ok());

    for (index, replacement) in [
        (0, b'N'),
        (4, 1),
        (5, 2),
        (6, 0),
        (7, 0),
        (16, 2),
        (18, 62),
        (20, 0),
        (52, 0),
    ] {
        let mut malformed = object.clone();
        malformed[index] = replacement;
        let malformed_manifest = manifest_for(
            ExternalDeviceLibraryContentKindV1::RelocatableObject,
            &malformed,
            target("gfx942:xnack-"),
            CodeObjectVersion::V5,
            llvm(18, "18.0.0git", &format!("malformed-{index}")),
            vec![baseline_function(
                ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
                &format!("malformed_{index}"),
            )],
            vec![],
        );
        assert!(matches!(
            ExternalDeviceLibraryProviderV1::new(&malformed_manifest, &malformed),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentHeaderMismatch)
        ));
    }

    let truncated = object[..63].to_vec();
    let truncated_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::RelocatableObject,
        &truncated,
        target("gfx942:xnack-"),
        CodeObjectVersion::V5,
        llvm(18, "18.0.0git", "truncated"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "truncated",
        )],
        vec![],
    );
    assert!(matches!(
        ExternalDeviceLibraryProviderV1::new(&truncated_manifest, &truncated),
        Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentHeaderMismatch)
    ));
}

#[test]
fn missing_fabricated_duplicate_and_extra_providers_are_rejected() {
    let (root, providers) = direct_fixture();
    let views = provider_views(&providers);
    assert_eq!(
        root.validate_provider_set(&views[..1]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::MissingProvider)
    );
    assert_eq!(
        root.validate_provider_set(&[views[0], views[0], views[1]]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderManifest)
    );

    let fake = provider(
        "fabricated",
        baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external_mul",
        ),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.0.0git", "fabricated"),
    );
    let fake_view = ExternalDeviceLibraryProviderV1::new(&fake.manifest, &fake.bytes).unwrap();
    assert_eq!(
        root.validate_provider_set(&[views[0], fake_view]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::MissingProvider)
    );
    assert_eq!(
        root.validate_provider_set(&[views[0], views[1], fake_view]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::UnexpectedProvider)
    );

    let excessive = vec![views[0]; MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1 + 1];
    assert_eq!(
        root.validate_provider_set(&excessive),
        Err(ExternalDeviceLibraryProviderSetErrorV1::TooManyProviders)
    );
}

#[test]
fn root_repetition_and_reused_provider_blobs_are_rejected() {
    let root_bytes = bitcode("standalone-root");
    let standalone_root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "standalone-root"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "root_export",
        )],
        vec![],
    );
    let root_view = ExternalDeviceLibraryProviderV1::new(&standalone_root, &root_bytes).unwrap();
    assert_eq!(
        standalone_root.validate_provider_set(&[root_view]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::RootRepeatedAsProvider)
    );

    let (root, providers) = direct_fixture();
    let alias_manifest = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &providers[0].bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "blob-alias"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "aliased_export",
        )],
        vec![],
    );
    let alias = ExternalDeviceLibraryProviderV1::new(&alias_manifest, &providers[0].bytes).unwrap();
    let mut views = provider_views(&providers);
    views.push(alias);
    assert_eq!(
        root.validate_provider_set(&views),
        Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderBlob)
    );
}

#[test]
fn dependency_content_target_cov_and_llvm_profiles_are_exact() {
    let cases = [
        (
            provider(
                "wrong-target",
                baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "external"),
                "gfx942:xnack+",
                CodeObjectVersion::V6,
                llvm(18, "18.1.8", "wrong-target"),
            ),
            ExternalDeviceLibraryProviderSetErrorV1::TargetMismatch,
        ),
        (
            provider(
                "wrong-cov",
                baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "external"),
                "gfx942:xnack-",
                CodeObjectVersion::V5,
                llvm(18, "18.1.8", "wrong-cov"),
            ),
            ExternalDeviceLibraryProviderSetErrorV1::CodeObjectVersionMismatch,
        ),
        (
            provider(
                "wrong-llvm",
                baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "external"),
                "gfx942:xnack-",
                CodeObjectVersion::V6,
                llvm(19, "19.0.0git", "wrong-llvm"),
            ),
            ExternalDeviceLibraryProviderSetErrorV1::LlvmProfileMismatch,
        ),
    ];
    for (provider, expected) in cases {
        let root_bytes = bitcode("compatibility-root");
        let root = manifest_for(
            ExternalDeviceLibraryContentKindV1::LlvmBitcode,
            &root_bytes,
            target("gfx942:xnack-"),
            CodeObjectVersion::V6,
            llvm(18, "18.7.9", "compatibility-root"),
            vec![baseline_function(
                ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
                "external",
            )],
            vec![dependency(&provider.manifest, &["external"])],
        );
        let view =
            ExternalDeviceLibraryProviderV1::new(&provider.manifest, &provider.bytes).unwrap();
        assert_eq!(root.validate_provider_set(&[view]), Err(expected));
    }

    let (root, providers) = direct_fixture();
    let wrong_kind = ExternalDeviceLibraryDependencyV1::new(
        providers[0].manifest.identity(),
        ExternalDeviceLibraryContentIdentityV1::new(
            ExternalDeviceLibraryContentKindV1::RelocatableObject,
            providers[0].manifest.content().blob(),
        ),
        vec!["external_add".into()],
    )
    .unwrap();
    let root_bytes = bitcode("kind-root");
    let kind_root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        root.target(),
        root.code_object_version(),
        root.llvm().clone(),
        root.symbols().to_vec(),
        vec![
            wrong_kind,
            dependency(&providers[1].manifest, &["external_mul"]),
        ],
    );
    assert_eq!(
        kind_root.validate_provider_set(&provider_views(&providers)),
        Err(ExternalDeviceLibraryProviderSetErrorV1::DependencyContentMismatch)
    );
}

#[test]
fn every_import_export_contract_field_is_compared() {
    let mutations = vec![
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external",
            WORKGROUP_ABI,
            "read_workgroup",
            ExternalDeviceConvergenceV1::Unconstrained,
            "external",
            &["global-memory"],
        ),
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external",
            GLOBAL_ABI,
            "none",
            ExternalDeviceConvergenceV1::Unconstrained,
            "external",
            &["global-memory"],
        ),
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external",
            GLOBAL_ABI,
            "read_global",
            ExternalDeviceConvergenceV1::Convergent,
            "external",
            &["global-memory"],
        ),
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external",
            GLOBAL_ABI,
            "read_global",
            ExternalDeviceConvergenceV1::Unconstrained,
            "different-semantic",
            &["global-memory"],
        ),
        function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionExport,
            "external",
            GLOBAL_ABI,
            "read_global",
            ExternalDeviceConvergenceV1::Unconstrained,
            "external",
            &["different-capability"],
        ),
    ];
    for (index, export) in mutations.into_iter().enumerate() {
        let provider = provider(
            &format!("contract-mutation-{index}"),
            export,
            "gfx942:xnack-",
            CodeObjectVersion::V6,
            llvm(18, "18.2.3", &format!("contract-{index}")),
        );
        let root_bytes = bitcode(&format!("contract-root-{index}"));
        let root = manifest_for(
            ExternalDeviceLibraryContentKindV1::LlvmBitcode,
            &root_bytes,
            target("gfx942:xnack-"),
            CodeObjectVersion::V6,
            llvm(18, "18.1.8", &format!("root-{index}")),
            vec![baseline_function(
                ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
                "external",
            )],
            vec![dependency(&provider.manifest, &["external"])],
        );
        let view =
            ExternalDeviceLibraryProviderV1::new(&provider.manifest, &provider.bytes).unwrap();
        assert_eq!(
            root.validate_provider_set(&[view]),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ImportExportContractMismatch),
            "accepted contract mutation {index}"
        );
    }
}

#[test]
fn missing_and_duplicate_exports_are_rejected_before_linking() {
    let missing = provider(
        "missing-export",
        baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "other"),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "missing-export"),
    );
    let root_bytes = bitcode("missing-root");
    let root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "missing-root"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "external",
        )],
        vec![dependency(&missing.manifest, &["external"])],
    );
    let view = ExternalDeviceLibraryProviderV1::new(&missing.manifest, &missing.bytes).unwrap();
    assert_eq!(
        root.validate_provider_set(&[view]),
        Err(ExternalDeviceLibraryProviderSetErrorV1::MissingProviderExport)
    );

    let first = provider(
        "duplicate-export-1",
        baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "external"),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "duplicate-1"),
    );
    let second = provider(
        "duplicate-export-2",
        baseline_function(ExternalDeviceSymbolRoleV1::DeviceFunctionExport, "external"),
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "duplicate-2"),
    );
    let root_bytes = bitcode("duplicate-root");
    let root = manifest_for(
        ExternalDeviceLibraryContentKindV1::LlvmBitcode,
        &root_bytes,
        target("gfx942:xnack-"),
        CodeObjectVersion::V6,
        llvm(18, "18.1.8", "duplicate-root"),
        vec![baseline_function(
            ExternalDeviceSymbolRoleV1::DeviceFunctionImport,
            "external",
        )],
        vec![
            dependency(&first.manifest, &["external"]),
            dependency(&second.manifest, &[]),
        ],
    );
    let providers = vec![first, second];
    assert_eq!(
        root.validate_provider_set(&provider_views(&providers)),
        Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderExport)
    );
}

#[test]
fn provider_debug_output_does_not_expose_content_or_symbol_text() {
    let (_, providers) = direct_fixture();
    let provider =
        ExternalDeviceLibraryProviderV1::new(&providers[0].manifest, &providers[0].bytes).unwrap();
    let debug = format!("{provider:?}");
    assert!(!debug.contains("provider-add"));
    assert!(!debug.contains("external_add"));
}
