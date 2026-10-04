#![cfg(target_os = "linux")]

use fe2o3_host_link_closure::{
    ArtifactProvenanceV1, ElfClassV1, ElfEndianV1, ElfProfileV1, ExecutableToolchainV1,
    HostArtifactKindV1, HostLinkErrorCodeV1, HostLinkHandoffV1, HostLinkPlanSpecV1, HostLinkPlanV1,
    LibraryPreferenceV1, MAX_HOST_LINK_PLAN_BYTES_V1, OutputTypeV1, PlanArgumentV1,
    ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1, RuntimeDsoClosureV1,
    Sha256Digest, TargetTripleV1,
};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use tempfile::TempDir;

fn nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x51; 32]).unwrap()
}

fn target() -> TargetTripleV1 {
    TargetTripleV1::new("x86_64-unknown-linux-gnu").unwrap()
}

fn minimal_elf(elf_type: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&elf_type.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes
}

fn source_file(root: &TempDir, name: &str, bytes: &[u8], mode: u32) -> File {
    let path = root.path().join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(mode)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .unwrap();
    file
}

fn artifact(
    root: &TempDir,
    label: &str,
    kind: HostArtifactKindV1,
    elf_type: u16,
) -> PublishedHostArtifactV1 {
    let file = source_file(root, label, &minimal_elf(elf_type), 0o755);
    PublishedHostArtifactV1::from_producer_fd(
        file,
        ProducerArtifactSpecV1::new(
            label,
            kind,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn profile(elf_type: u16) -> ElfProfileV1 {
    ElfProfileV1 {
        class: ElfClassV1::Elf64,
        endian: ElfEndianV1::Little,
        elf_type,
        machine: 62,
        interpreter: None,
        soname: None,
        needed: vec![],
        has_writable_executable_segment: false,
        has_executable_stack: false,
    }
}

#[test]
fn canonical_plan_round_trips_through_exact_sealed_descriptors() {
    let root = TempDir::new().unwrap();
    let wrapper = artifact(&root, "wrapper", HostArtifactKindV1::StaticWrapper, 2);
    let wrapper_id = wrapper.id();
    let lld = artifact(&root, "host-lld", HostArtifactKindV1::StaticHostLld, 2);
    let lld_id = lld.id();
    let object = artifact(&root, "object", HostArtifactKindV1::Object, 1);
    let object_id = object.id();
    let spec = HostLinkPlanSpecV1 {
        release_nonce: nonce(),
        target: target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-test".to_owned(),
        },
        output_type: OutputTypeV1::Executable,
        expected_output_mode: 0o555,
        expected_output_elf: profile(2),
        arguments: vec![
            PlanArgumentV1::Literal(b"--gc-sections".to_vec()),
            PlanArgumentV1::ProducerArtifact(object_id),
            PlanArgumentV1::SearchRoot("sysroot".to_owned()),
            PlanArgumentV1::Library {
                name: "c".to_owned(),
                preference: LibraryPreferenceV1::DynamicOnly,
            },
        ],
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![object, lld, wrapper]).unwrap();
    let bytes = handoff.manifest().encode_canonical().unwrap();
    let decoded =
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, *handoff.manifest());
    assert_ne!(
        decoded.plan_digest,
        fe2o3_host_link_closure::Sha256Digest::ZERO
    );
    assert_strict_wire_rejections(&decoded, &bytes);

    let (plan_fd, producer_fds) = handoff.into_parts();
    let plan = HostLinkPlanV1::from_sealed_fd(plan_fd, producer_fds).unwrap();
    assert_eq!(plan.plan_digest(), decoded.plan_digest);
    assert_eq!(plan.target(), &target());
    plan.revalidate().unwrap();
}

fn assert_strict_wire_rejections(
    manifest: &fe2o3_host_link_closure::HostLinkPlanManifestV1,
    canonical: &[u8],
) {
    let mut wrong_version = canonical.to_vec();
    wrong_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&wrong_version)
            .unwrap_err()
            .code(),
        HostLinkErrorCodeV1::InvalidVersion
    );

    let mut reserved = canonical.to_vec();
    reserved[10] = 1;
    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&reserved)
            .unwrap_err()
            .code(),
        HostLinkErrorCodeV1::NonCanonicalWire
    );

    let mut wrong_length = canonical.to_vec();
    let payload = u32::from_le_bytes(wrong_length[12..16].try_into().unwrap());
    wrong_length[12..16].copy_from_slice(&(payload + 1).to_le_bytes());
    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&wrong_length)
            .unwrap_err()
            .code(),
        HostLinkErrorCodeV1::InvalidWire
    );

    let mut wrong_digest = canonical.to_vec();
    *wrong_digest.last_mut().unwrap() ^= 1;
    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&wrong_digest)
            .unwrap_err()
            .code(),
        HostLinkErrorCodeV1::DigestMismatch
    );

    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&trailing)
            .unwrap_err()
            .code(),
        HostLinkErrorCodeV1::InvalidWire
    );

    let mut duplicate = manifest.clone();
    duplicate.producers.push(duplicate.producers[0].clone());
    assert_eq!(
        duplicate.encode_canonical().unwrap_err().code(),
        HostLinkErrorCodeV1::DuplicateRecord
    );

    let mut unordered = manifest.clone();
    unordered.producers.swap(0, 1);
    assert_eq!(
        unordered.encode_canonical().unwrap_err().code(),
        HostLinkErrorCodeV1::NonCanonicalOrder
    );

    let mut invalid_path = manifest.clone();
    invalid_path.spec.arguments = vec![PlanArgumentV1::FixedRootInput {
        root: "sysroot".to_owned(),
        relative_path: b"../escape.o".to_vec(),
        kind: fe2o3_host_link_closure::RootInputKindV1::Object,
    }];
    assert_eq!(
        invalid_path.encode_canonical().unwrap_err().code(),
        HostLinkErrorCodeV1::InvalidPath
    );

    let mut stale_digest = manifest.clone();
    stale_digest.plan_digest = Sha256Digest::from_bytes([0x5a; 32]);
    assert_eq!(
        stale_digest.encode_canonical().unwrap_err().code(),
        HostLinkErrorCodeV1::DigestMismatch
    );

    assert_eq!(
        fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&vec![
            0;
            MAX_HOST_LINK_PLAN_BYTES_V1
                + 1
        ])
        .unwrap_err()
        .code(),
        HostLinkErrorCodeV1::PlanTooLarge
    );

    for length in 0..512 {
        let hostile = (0..length)
            .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
            .collect::<Vec<_>>();
        assert!(
            std::panic::catch_unwind(|| {
                let _ = fe2o3_host_link_closure::HostLinkPlanManifestV1::decode_canonical(&hostile);
            })
            .is_ok(),
            "wire parser panicked for {length} hostile bytes"
        );
    }
}

#[test]
fn unsealed_plan_and_wrong_descriptor_count_fail_closed() {
    let root = TempDir::new().unwrap();
    let unsealed = source_file(&root, "plan", b"not a plan", 0o600);
    let error = HostLinkPlanV1::from_sealed_fd(unsealed, vec![])
        .err()
        .expect("unsealed plan must fail");
    assert_eq!(error.code(), HostLinkErrorCodeV1::DescriptorUnsealed);

    let wrapper = artifact(&root, "wrapper", HostArtifactKindV1::StaticWrapper, 2);
    let wrapper_id = wrapper.id();
    let lld = artifact(&root, "lld", HostArtifactKindV1::StaticHostLld, 2);
    let lld_id = lld.id();
    let spec = HostLinkPlanSpecV1 {
        release_nonce: nonce(),
        target: target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-test".to_owned(),
        },
        output_type: OutputTypeV1::Executable,
        expected_output_mode: 0o555,
        expected_output_elf: profile(2),
        arguments: vec![PlanArgumentV1::Literal(b"--gc-sections".to_vec())],
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![wrapper, lld]).unwrap();
    let (plan_fd, mut producer_fds) = handoff.into_parts();
    producer_fds.pop();
    let error = HostLinkPlanV1::from_sealed_fd(plan_fd, producer_fds)
        .err()
        .expect("missing producer descriptor must fail");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ReplayMismatch);
}

#[test]
fn publication_and_revalidation_do_not_change_shared_file_offsets() {
    let root = TempDir::new().unwrap();
    let source = source_file(&root, "offset.o", &minimal_elf(1), 0o644);
    let mut observer = source.try_clone().unwrap();
    observer.seek(SeekFrom::Start(17)).unwrap();
    let published = PublishedHostArtifactV1::from_producer_fd(
        source,
        ProducerArtifactSpecV1::new(
            "offset.o",
            HostArtifactKindV1::Object,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(observer.stream_position().unwrap(), 17);

    let mut sealed_observer = published.try_clone_file().unwrap();
    sealed_observer.seek(SeekFrom::Start(23)).unwrap();
    published.revalidate().unwrap();
    assert_eq!(sealed_observer.stream_position().unwrap(), 23);
}
