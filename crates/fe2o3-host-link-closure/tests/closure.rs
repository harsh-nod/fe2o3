#![cfg(target_os = "linux")]

use fe2o3_host_link_closure::{
    ArtifactProvenanceV1, ElfClassV1, ElfEndianV1, ElfProfileV1, ExecutableToolchainV1,
    FixedRootSetV1, FixedRootV1, HostArtifactCatalogV1, HostArtifactKindV1, HostLinkClosureV1,
    HostLinkErrorCodeV1, HostLinkHandoffV1, HostLinkPlanSpecV1, HostLinkPlanV1, OutputTypeV1,
    PlanArgumentV1, ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1,
    RootInputKindV1, RuntimeDsoClosureV1, TargetTripleV1,
};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use tempfile::TempDir;

fn nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x73; 32]).unwrap()
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

fn single_object_archive(name: &[u8]) -> Vec<u8> {
    let object = minimal_elf(object::elf::ET_REL);
    let mut bytes = b"!<arch>\n".to_vec();
    let mut header = [b' '; 60];
    header[..name.len()].copy_from_slice(name);
    header[name.len()] = b'/';
    header[16..28].copy_from_slice(b"0           ");
    header[28..34].copy_from_slice(b"0     ");
    header[34..40].copy_from_slice(b"0     ");
    header[40..48].copy_from_slice(b"100644  ");
    header[48..58].copy_from_slice(b"64        ");
    header[58..].copy_from_slice(b"`\n");
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&object);
    bytes
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

fn write_file(root: &TempDir, name: &str, bytes: &[u8], mode: u32) -> File {
    let path = root.path().join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(mode)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .unwrap();
    file
}

fn published(
    root: &TempDir,
    label: &str,
    kind: HostArtifactKindV1,
    bytes: &[u8],
    mode: u32,
) -> PublishedHostArtifactV1 {
    PublishedHostArtifactV1::from_producer_fd(
        write_file(root, label, bytes, mode),
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

fn decoded_plan(producer_root: &TempDir, arguments: Vec<PlanArgumentV1>) -> HostLinkPlanV1 {
    let wrapper = published(
        producer_root,
        "wrapper",
        HostArtifactKindV1::StaticWrapper,
        &minimal_elf(2),
        0o755,
    );
    let wrapper_id = wrapper.id();
    let lld = published(
        producer_root,
        "host-lld",
        HostArtifactKindV1::StaticHostLld,
        &minimal_elf(2),
        0o755,
    );
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
        arguments,
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![wrapper, lld]).unwrap();
    let (plan, producers) = handoff.into_parts();
    HostLinkPlanV1::from_sealed_fd(plan, producers).unwrap()
}

#[test]
fn closure_expands_control_files_and_produces_deterministic_fd_only_argv() {
    let producer_root = TempDir::new().unwrap();
    let fixed_root = TempDir::new().unwrap();
    write_file(
        &fixed_root,
        "libfoo.a",
        &single_object_archive(b"foo.o"),
        0o644,
    );
    write_file(
        &fixed_root,
        "libbar.a",
        &single_object_archive(b"bar.o"),
        0o644,
    );
    write_file(&fixed_root, "input.o", &minimal_elf(1), 0o644);
    write_file(
        &fixed_root,
        "args.rsp",
        b"-L@sysroot -Bstatic -lfoo --gc-sections -z now -u entry_symbol",
        0o644,
    );
    write_file(
        &fixed_root,
        "inputs.ld",
        b"INPUT(input.o, libbar.a);",
        0o644,
    );
    let fixed = FixedRootV1::open("sysroot", fixed_root.path()).unwrap();
    let plan = decoded_plan(
        &producer_root,
        vec![
            PlanArgumentV1::ResponseFile {
                root: "sysroot".to_owned(),
                relative_path: b"args.rsp".to_vec(),
            },
            PlanArgumentV1::FixedRootInput {
                root: "sysroot".to_owned(),
                relative_path: b"inputs.ld".to_vec(),
                kind: RootInputKindV1::LinkerScript,
            },
        ],
    );
    let catalog = HostArtifactCatalogV1::new(nonce(), target());
    let mut closure =
        HostLinkClosureV1::prepare(plan, FixedRootSetV1::new(vec![fixed]).unwrap(), catalog)
            .unwrap();
    let error = closure
        .lld_argv()
        .err()
        .expect("argv before prevalidation must fail");
    assert_eq!(error.code(), HostLinkErrorCodeV1::InvalidState);
    closure.prevalidate().unwrap();
    let argv = closure.lld_argv().unwrap();
    let arguments = argv.canonical_arguments();
    assert_eq!(arguments[0], b"fe2o3-host-lld");
    assert_eq!(arguments[1], b"--fe2o3-host-lld-elf-v2");
    assert!(arguments[2].starts_with(b"--fe2o3-result-socket-v1=91:"));
    assert!(arguments[3].starts_with(b"--fe2o3-request-v1="));
    assert!(arguments[4].starts_with(b"--fe2o3-input-v1=100:archive:"));
    assert_eq!(arguments[5], b"--gc-sections");
    assert_eq!(arguments[6], b"-z");
    assert_eq!(arguments[7], b"now");
    assert_eq!(arguments[8], b"--undefined=entry_symbol");
    assert!(arguments[9].starts_with(b"--fe2o3-input-v1=101:elf-rel:"));
    assert!(arguments[10].starts_with(b"--fe2o3-input-v1=102:archive:"));
    assert!(
        arguments
            .iter()
            .all(|argument| !argument.starts_with(b"/proc/self/fd/"))
    );
    assert_ne!(
        closure.closure_digest(),
        fe2o3_host_link_closure::Sha256Digest::ZERO
    );
}

#[test]
fn fixed_root_mutation_and_symlink_roots_fail_closed() {
    let producer_root = TempDir::new().unwrap();
    let fixed_root = TempDir::new().unwrap();
    write_file(&fixed_root, "input.o", &minimal_elf(1), 0o644);
    let fixed = FixedRootV1::open("sysroot", fixed_root.path()).unwrap();
    let plan = decoded_plan(
        &producer_root,
        vec![PlanArgumentV1::FixedRootInput {
            root: "sysroot".to_owned(),
            relative_path: b"input.o".to_vec(),
            kind: RootInputKindV1::Object,
        }],
    );
    let mut closure = HostLinkClosureV1::prepare(
        plan,
        FixedRootSetV1::new(vec![fixed]).unwrap(),
        HostArtifactCatalogV1::new(nonce(), target()),
    )
    .unwrap();
    closure.prevalidate().unwrap();
    OpenOptions::new()
        .append(true)
        .open(fixed_root.path().join("input.o"))
        .unwrap()
        .write_all(b"mutation")
        .unwrap();
    let error = closure.revalidate().unwrap_err();
    assert!(matches!(
        error.code(),
        HostLinkErrorCodeV1::RootMutation | HostLinkErrorCodeV1::RootChanged
    ));

    let alias_root = TempDir::new().unwrap();
    let alias = alias_root.path().join("alias");
    symlink(fixed_root.path(), &alias).unwrap();
    let error = FixedRootV1::open("alias", &alias)
        .err()
        .expect("symlink root must fail");
    assert_eq!(error.code(), HostLinkErrorCodeV1::Symlink);
}

#[test]
fn fixed_root_replacement_and_pre_copy_inode_substitution_fail_closed() {
    let parent = TempDir::new().unwrap();
    let root_path = parent.path().join("root");
    std::fs::create_dir(&root_path).unwrap();
    std::fs::write(root_path.join("input.o"), minimal_elf(1)).unwrap();
    let retained = FixedRootV1::open("sysroot", &root_path).unwrap();
    std::fs::rename(&root_path, parent.path().join("moved-root")).unwrap();
    std::fs::create_dir(&root_path).unwrap();
    std::fs::write(root_path.join("input.o"), minimal_elf(1)).unwrap();
    assert!(matches!(
        retained.revalidate().unwrap_err().code(),
        HostLinkErrorCodeV1::RootChanged | HostLinkErrorCodeV1::RootMutation
    ));

    let producer_root = TempDir::new().unwrap();
    let fixed_root = TempDir::new().unwrap();
    write_file(&fixed_root, "input.o", &minimal_elf(1), 0o644);
    std::fs::hard_link(
        fixed_root.path().join("input.o"),
        fixed_root.path().join("alternate.o"),
    )
    .unwrap();
    let error = FixedRootV1::open("sysroot", fixed_root.path())
        .err()
        .expect("fixed roots with multiply linked regular files must reject");
    assert_eq!(error.code(), HostLinkErrorCodeV1::RootChanged);
    std::fs::remove_file(fixed_root.path().join("input.o")).unwrap();
    std::fs::hard_link(
        fixed_root.path().join("alternate.o"),
        fixed_root.path().join("input.o"),
    )
    .unwrap();
    let _ = producer_root;
}

#[test]
fn external_hardlink_mutation_after_root_admission_fails_closed() {
    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write_file(&root, "input.o", &minimal_elf(1), 0o644);
    let retained = FixedRootV1::open("sysroot", root.path()).unwrap();
    std::fs::hard_link(root.path().join("input.o"), outside.path().join("alias.o")).unwrap();
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(outside.path().join("alias.o"))
        .unwrap()
        .write_all(&minimal_elf(1))
        .unwrap();
    assert_eq!(
        retained.revalidate().unwrap_err().code(),
        HostLinkErrorCodeV1::RootChanged
    );
}

#[test]
fn group_linker_scripts_reject_instead_of_losing_group_semantics() {
    let producer_root = TempDir::new().unwrap();
    let fixed_root = TempDir::new().unwrap();
    write_file(&fixed_root, "input.o", &minimal_elf(1), 0o644);
    write_file(&fixed_root, "group.ld", b"GROUP(input.o);", 0o644);
    let fixed = FixedRootV1::open("sysroot", fixed_root.path()).unwrap();
    let plan = decoded_plan(
        &producer_root,
        vec![PlanArgumentV1::FixedRootInput {
            root: "sysroot".to_owned(),
            relative_path: b"group.ld".to_vec(),
            kind: RootInputKindV1::LinkerScript,
        }],
    );
    let error = HostLinkClosureV1::prepare(
        plan,
        FixedRootSetV1::new(vec![fixed]).unwrap(),
        HostArtifactCatalogV1::new(nonce(), target()),
    )
    .err()
    .expect("GROUP must not be flattened into ordinary positional inputs");
    assert_eq!(error.code(), HostLinkErrorCodeV1::LinkerScript);
}

#[test]
fn nested_symlinks_are_never_admitted_as_fixed_root_content() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("outside.o"), minimal_elf(1)).unwrap();
    symlink(root.path().join("outside.o"), root.path().join("input.o")).unwrap();
    let error = FixedRootV1::open("sysroot", root.path())
        .err()
        .expect("symlinked root input must fail the snapshot");
    assert_eq!(error.code(), HostLinkErrorCodeV1::Symlink);
}
