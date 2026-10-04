#![cfg(target_os = "linux")]

use fe2o3_host_link_closure::{
    ArtifactIdV1, ArtifactProvenanceV1, ElfClassV1, ElfEndianV1, ElfProfileV1,
    ExecutableToolchainV1, FixedRootSetV1, FixedRootV1, HostArtifactCatalogV1, HostArtifactKindV1,
    HostLinkClosureV1, HostLinkError, HostLinkErrorCodeV1, HostLinkHandoffV1, HostLinkPlanSpecV1,
    HostLinkPlanV1, MAX_HOST_LINK_INPUT_BYTES_V1, OutputTypeV1, PlanArgumentV1,
    ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1, RootInputKindV1,
    RuntimeDsoClosureV1, Sha256Digest, TargetTripleV1,
};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use tempfile::TempDir;

fn nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x2a; 32]).unwrap()
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

fn archive(members: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut bytes = b"!<arch>\n".to_vec();
    for (name, data) in members {
        assert!(!name.is_empty() && name.len() <= 15);
        let mut header = [b' '; 60];
        header[..name.len()].copy_from_slice(name);
        header[name.len()] = b'/';
        header[16..28].copy_from_slice(b"0           ");
        header[28..34].copy_from_slice(b"0     ");
        header[34..40].copy_from_slice(b"0     ");
        header[40..48].copy_from_slice(b"100644  ");
        let size = format!("{:<10}", data.len());
        header[48..58].copy_from_slice(size.as_bytes());
        header[58..].copy_from_slice(b"`\n");
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(data);
        if data.len() % 2 != 0 {
            bytes.push(b'\n');
        }
    }
    bytes
}

fn elf_with_llvmbc(flags: u64) -> Vec<u8> {
    const STRINGS: &[u8] = b"\0.llvmbc\0.shstrtab\0";
    const STRINGS_OFFSET: usize = 64;
    const DATA_OFFSET: usize = STRINGS_OFFSET + STRINGS.len();
    const SECTION_HEADERS_OFFSET: usize = 88;
    let mut bytes = vec![0_u8; SECTION_HEADERS_OFFSET + 3 * 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&1_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[40..48].copy_from_slice(&(SECTION_HEADERS_OFFSET as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes[60..62].copy_from_slice(&3_u16.to_le_bytes());
    bytes[62..64].copy_from_slice(&2_u16.to_le_bytes());
    bytes[STRINGS_OFFSET..DATA_OFFSET].copy_from_slice(STRINGS);
    bytes[DATA_OFFSET..DATA_OFFSET + 4].copy_from_slice(b"data");

    let llvmbc = SECTION_HEADERS_OFFSET + 64;
    bytes[llvmbc..llvmbc + 4].copy_from_slice(&1_u32.to_le_bytes());
    bytes[llvmbc + 4..llvmbc + 8].copy_from_slice(&1_u32.to_le_bytes());
    bytes[llvmbc + 8..llvmbc + 16].copy_from_slice(&flags.to_le_bytes());
    bytes[llvmbc + 24..llvmbc + 32].copy_from_slice(&(DATA_OFFSET as u64).to_le_bytes());
    bytes[llvmbc + 32..llvmbc + 40].copy_from_slice(&4_u64.to_le_bytes());
    bytes[llvmbc + 48..llvmbc + 56].copy_from_slice(&1_u64.to_le_bytes());

    let shstrtab = SECTION_HEADERS_OFFSET + 2 * 64;
    bytes[shstrtab..shstrtab + 4].copy_from_slice(&10_u32.to_le_bytes());
    bytes[shstrtab + 4..shstrtab + 8].copy_from_slice(&3_u32.to_le_bytes());
    bytes[shstrtab + 24..shstrtab + 32].copy_from_slice(&(STRINGS_OFFSET as u64).to_le_bytes());
    bytes[shstrtab + 32..shstrtab + 40].copy_from_slice(&(STRINGS.len() as u64).to_le_bytes());
    bytes[shstrtab + 48..shstrtab + 56].copy_from_slice(&1_u64.to_le_bytes());
    bytes
}

fn elf_with_dependent_libraries_section() -> Vec<u8> {
    let mut bytes = elf_with_llvmbc(0);
    bytes[65..73].copy_from_slice(b".deplibs");
    let section = 88 + 64;
    bytes[section + 4..section + 8]
        .copy_from_slice(&object::elf::SHT_LLVM_DEPENDENT_LIBRARIES.to_le_bytes());
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

fn source(root: &TempDir, name: &str, bytes: &[u8], mode: u32) -> File {
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(mode)
        .open(root.path().join(name))
        .unwrap();
    file.write_all(bytes).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .unwrap();
    file
}

fn published_with(
    root: &TempDir,
    label: &str,
    kind: HostArtifactKindV1,
    provenance: ArtifactProvenanceV1,
    artifact_nonce: ReleaseNonceV1,
    artifact_target: TargetTripleV1,
) -> Result<PublishedHostArtifactV1, HostLinkError> {
    PublishedHostArtifactV1::from_producer_fd(
        source(root, label, &minimal_elf(1), 0o644),
        ProducerArtifactSpecV1::new(label, kind, provenance, artifact_nonce, artifact_target)
            .unwrap(),
    )
}

fn tool(root: &TempDir, label: &str, kind: HostArtifactKindV1) -> PublishedHostArtifactV1 {
    PublishedHostArtifactV1::from_producer_fd(
        source(root, label, &minimal_elf(2), 0o755),
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

fn decoded_plan(arguments: Vec<PlanArgumentV1>) -> HostLinkPlanV1 {
    try_decoded_plan(arguments).unwrap()
}

fn try_decoded_plan(arguments: Vec<PlanArgumentV1>) -> Result<HostLinkPlanV1, HostLinkError> {
    try_decoded_plan_with_output_policy(arguments, OutputTypeV1::Executable, 0o555, profile(2))
}

fn try_decoded_plan_with_output_policy(
    arguments: Vec<PlanArgumentV1>,
    output_type: OutputTypeV1,
    expected_output_mode: u32,
    expected_output_elf: ElfProfileV1,
) -> Result<HostLinkPlanV1, HostLinkError> {
    let root = TempDir::new().unwrap();
    let wrapper = tool(&root, "wrapper", HostArtifactKindV1::StaticWrapper);
    let wrapper_id = wrapper.id();
    let lld = tool(&root, "lld", HostArtifactKindV1::StaticHostLld);
    let lld_id = lld.id();
    let spec = HostLinkPlanSpecV1 {
        release_nonce: nonce(),
        target: target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-test".to_owned(),
        },
        output_type,
        expected_output_mode,
        expected_output_elf,
        arguments,
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![wrapper, lld])?;
    let (plan, producers) = handoff.into_parts();
    HostLinkPlanV1::from_sealed_fd(plan, producers)
}

#[test]
fn output_policy_rejects_every_dynamic_or_mutable_profile() {
    let mut interpreter = profile(2);
    interpreter.interpreter = Some(b"/lib64/ld-linux-x86-64.so.2".to_vec());
    let mut needed = profile(2);
    needed.needed = vec![b"libc.so.6".to_vec()];
    let mut soname = profile(2);
    soname.soname = Some(b"host-output.so".to_vec());
    let mut writable_executable = profile(2);
    writable_executable.has_writable_executable_segment = true;
    let mut executable_stack = profile(2);
    executable_stack.has_executable_stack = true;

    for (output_type, mode, output_profile) in [
        (OutputTypeV1::SharedObject, 0o555, profile(3)),
        (OutputTypeV1::Relocatable, 0o555, profile(1)),
        (OutputTypeV1::Executable, 0o755, profile(2)),
        (OutputTypeV1::Executable, 0o555, interpreter),
        (OutputTypeV1::Executable, 0o555, needed),
        (OutputTypeV1::Executable, 0o555, soname),
        (OutputTypeV1::Executable, 0o555, writable_executable),
        (OutputTypeV1::Executable, 0o555, executable_stack),
    ] {
        let error = try_decoded_plan_with_output_policy(
            vec![PlanArgumentV1::Literal(b"--static".to_vec())],
            output_type,
            mode,
            output_profile,
        )
        .err()
        .expect("non-static output policy must reject");
        assert_eq!(error.code(), HostLinkErrorCodeV1::ElfPolicy);
    }
}

#[test]
fn raw_search_library_unknown_and_alternate_options_reject() {
    for (argument, expected) in [
        (b"-L/tmp".as_slice(), HostLinkErrorCodeV1::UnresolvedSearch),
        (b"-lfoo".as_slice(), HostLinkErrorCodeV1::UnresolvedLibrary),
        (
            b"--unknown-option".as_slice(),
            HostLinkErrorCodeV1::UnsupportedArgument,
        ),
        (
            b"-znow".as_slice(),
            HostLinkErrorCodeV1::UnsupportedArgument,
        ),
        (
            b"-uentry".as_slice(),
            HostLinkErrorCodeV1::UnsupportedArgument,
        ),
    ] {
        let error = try_decoded_plan(vec![PlanArgumentV1::Literal(argument.to_vec())])
            .err()
            .expect("hostile raw option must reject");
        assert_eq!(error.code(), expected);
    }
}

#[test]
fn thin_archive_plugin_lto_and_unpublished_build_script_reject() {
    let root = TempDir::new().unwrap();
    let thin = PublishedHostArtifactV1::from_producer_fd(
        source(&root, "thin.a", b"!<thin>\n", 0o644),
        ProducerArtifactSpecV1::new(
            "thin.a",
            HostArtifactKindV1::RegularArchive,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("thin archive must reject");
    assert_eq!(thin.code(), HostLinkErrorCodeV1::ThinArchive);

    for kind in [HostArtifactKindV1::Plugin, HostArtifactKindV1::LtoCache] {
        let error = published_with(
            &root,
            if kind == HostArtifactKindV1::Plugin {
                "plugin.so"
            } else {
                "lto-cache"
            },
            kind,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .err()
        .expect("plugin or LTO publication must reject");
        assert_eq!(
            error.code(),
            if kind == HostArtifactKindV1::Plugin {
                HostLinkErrorCodeV1::Plugin
            } else {
                HostLinkErrorCodeV1::Lto
            }
        );
    }

    let unpublished = published_with(
        &root,
        "native.o",
        HostArtifactKindV1::BuildScriptNative,
        ArtifactProvenanceV1::Compiler,
        nonce(),
        target(),
    )
    .err()
    .expect("unpublished build-script native input must reject");
    assert_eq!(
        unpublished.code(),
        HostLinkErrorCodeV1::UnpublishedBuildScript
    );
}

#[test]
fn oversized_sparse_input_rejects_before_content_allocation() {
    let root = TempDir::new().unwrap();
    let oversized = root.path().join("oversized.o");
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(0o644)
        .open(&oversized)
        .unwrap();
    file.set_len(MAX_HOST_LINK_INPUT_BYTES_V1 + 1).unwrap();
    let error = PublishedHostArtifactV1::from_producer_fd(
        file,
        ProducerArtifactSpecV1::new(
            "oversized.o",
            HostArtifactKindV1::Object,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("oversized sparse input must reject from metadata");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ArtifactTooLarge);
}

#[test]
fn archive_members_are_structurally_bounded_and_lto_free() {
    let root = TempDir::new().unwrap();
    let valid = archive(&[(b"member.o", &minimal_elf(1))]);
    assert!(
        PublishedHostArtifactV1::from_producer_fd(
            source(&root, "valid.a", &valid, 0o644),
            ProducerArtifactSpecV1::new(
                "valid.a",
                HostArtifactKindV1::RegularArchive,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .is_ok()
    );

    let metadata_rlib = archive(&[
        (b"member.o", minimal_elf(1).as_slice()),
        (b"lib.rmeta", minimal_elf(1).as_slice()),
    ]);
    assert!(
        PublishedHostArtifactV1::from_producer_fd(
            source(&root, "metadata.rlib", &metadata_rlib, 0o644),
            ProducerArtifactSpecV1::new(
                "metadata.rlib",
                HostArtifactKindV1::Rlib,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .is_ok()
    );

    let arbitrary_rmeta = archive(&[(b"lib.rmeta", b"arbitrary metadata bytes")]);
    let error = PublishedHostArtifactV1::from_producer_fd(
        source(&root, "arbitrary.rlib", &arbitrary_rmeta, 0o644),
        ProducerArtifactSpecV1::new(
            "arbitrary.rlib",
            HostArtifactKindV1::Rlib,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("an rlib member name cannot exempt arbitrary bytes");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ArtifactKind);

    let excluded_llvmbc = archive(&[(
        b"member.o",
        elf_with_llvmbc(u64::from(object::elf::SHF_EXCLUDE)).as_slice(),
    )]);
    assert!(
        PublishedHostArtifactV1::from_producer_fd(
            source(&root, "excluded-llvmbc.rlib", &excluded_llvmbc, 0o644),
            ProducerArtifactSpecV1::new(
                "excluded-llvmbc.rlib",
                HostArtifactKindV1::Rlib,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .is_ok()
    );

    for (label, flags) in [
        ("not-excluded", 0_u64),
        (
            "alloc",
            u64::from(object::elf::SHF_EXCLUDE | object::elf::SHF_ALLOC),
        ),
        (
            "write",
            u64::from(object::elf::SHF_EXCLUDE | object::elf::SHF_WRITE),
        ),
        (
            "exec",
            u64::from(object::elf::SHF_EXCLUDE | object::elf::SHF_EXECINSTR),
        ),
    ] {
        let member = elf_with_llvmbc(flags);
        let hostile = archive(&[(b"member.o", member.as_slice())]);
        let name = format!("{label}-llvmbc.rlib");
        let error = PublishedHostArtifactV1::from_producer_fd(
            source(&root, &name, &hostile, 0o644),
            ProducerArtifactSpecV1::new(
                &name,
                HostArtifactKindV1::Rlib,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .err()
        .expect("active .llvmbc section must reject");
        assert_eq!(error.code(), HostLinkErrorCodeV1::Lto, "{label}");
    }

    let many_members = (0..8193)
        .map(|index| (format!("m{index:05}.o").into_bytes(), Vec::<u8>::new()))
        .collect::<Vec<_>>();
    let member_refs = many_members
        .iter()
        .map(|(name, data)| (name.as_slice(), data.as_slice()))
        .collect::<Vec<_>>();
    let oversized_member_table = archive(&member_refs);
    let error = PublishedHostArtifactV1::from_producer_fd(
        source(&root, "too-many.a", &oversized_member_table, 0o644),
        ProducerArtifactSpecV1::new(
            "too-many.a",
            HostArtifactKindV1::RegularArchive,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("archive traversal count must be bounded before member parsing");
    assert_eq!(error.code(), HostLinkErrorCodeV1::FieldTooLarge);

    for (name, bytes, expected) in [
        (
            "nested.a",
            archive(&[(b"nested.a", b"!<arch>\n")]),
            HostLinkErrorCodeV1::ArtifactKind,
        ),
        (
            "raw-bitcode.a",
            archive(&[(b"member.o", b"BC\xc0\xdehostile")]),
            HostLinkErrorCodeV1::Lto,
        ),
        (
            "path-member.a",
            archive(&[(b"../member.o", &minimal_elf(1))]),
            HostLinkErrorCodeV1::InvalidPath,
        ),
        (
            "non-object.a",
            archive(&[(b"member.o", b"not an object")]),
            HostLinkErrorCodeV1::ArtifactKind,
        ),
    ] {
        let error = PublishedHostArtifactV1::from_producer_fd(
            source(&root, name, &bytes, 0o644),
            ProducerArtifactSpecV1::new(
                name,
                HostArtifactKindV1::RegularArchive,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .err()
        .expect("hostile archive member must reject");
        assert_eq!(error.code(), expected, "archive case {name}");
    }

    let malformed = [b"!<arch>\n".as_slice(), &[b'x'; 59]].concat();
    let error = PublishedHostArtifactV1::from_producer_fd(
        source(&root, "malformed.a", &malformed, 0o644),
        ProducerArtifactSpecV1::new(
            "malformed.a",
            HostArtifactKindV1::RegularArchive,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("malformed archive must reject");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ArtifactKind);
}

#[test]
fn direct_and_archived_relocatables_reject_wrong_machine_lto_and_deplibs() {
    let root = TempDir::new().unwrap();
    let mut wrong_machine = minimal_elf(1);
    wrong_machine[18..20].copy_from_slice(&object::elf::EM_AARCH64.to_le_bytes());
    let dependent_libraries = elf_with_dependent_libraries_section();
    for (name, bytes, expected) in [
        (
            "wrong-machine.o",
            wrong_machine,
            HostLinkErrorCodeV1::ArtifactKind,
        ),
        (
            "raw-bitcode.o",
            b"BC\xc0\xdehostile".to_vec(),
            HostLinkErrorCodeV1::Lto,
        ),
        (
            "active-llvmbc.o",
            elf_with_llvmbc(0),
            HostLinkErrorCodeV1::Lto,
        ),
        (
            "dependent-libraries.o",
            dependent_libraries.clone(),
            HostLinkErrorCodeV1::Lto,
        ),
    ] {
        let error = PublishedHostArtifactV1::from_producer_fd(
            source(&root, name, &bytes, 0o644),
            ProducerArtifactSpecV1::new(
                name,
                HostArtifactKindV1::Object,
                ArtifactProvenanceV1::Compiler,
                nonce(),
                target(),
            )
            .unwrap(),
        )
        .err()
        .expect("hostile direct relocatable must reject");
        assert_eq!(error.code(), expected, "direct input {name}");
    }

    let archived = archive(&[(b"member.o", dependent_libraries.as_slice())]);
    let error = PublishedHostArtifactV1::from_producer_fd(
        source(&root, "deplibs.a", &archived, 0o644),
        ProducerArtifactSpecV1::new(
            "deplibs.a",
            HostArtifactKindV1::RegularArchive,
            ArtifactProvenanceV1::Compiler,
            nonce(),
            target(),
        )
        .unwrap(),
    )
    .err()
    .expect("dependent-library archive member must reject recursively");
    assert_eq!(error.code(), HostLinkErrorCodeV1::Lto);
}

#[test]
fn expanded_argument_count_and_byte_budgets_fail_incrementally() {
    let count_error = HostLinkClosureV1::prepare(
        decoded_plan(vec![
            PlanArgumentV1::ZPolicy(
                fe2o3_host_link_closure::LinkerZPolicyV1::Now,
            );
            2047
        ]),
        FixedRootSetV1::new(vec![]).unwrap(),
        HostArtifactCatalogV1::new(nonce(), target()),
    )
    .err()
    .expect("expanded argument count must reject");
    assert_eq!(count_error.code(), HostLinkErrorCodeV1::FieldTooLarge);

    let long_symbol = "x".repeat(512);
    let byte_error = HostLinkClosureV1::prepare(
        decoded_plan(vec![PlanArgumentV1::UndefinedSymbol(long_symbol); 2002]),
        FixedRootSetV1::new(vec![]).unwrap(),
        HostArtifactCatalogV1::new(nonce(), target()),
    )
    .err()
    .expect("expanded argument bytes must reject");
    assert_eq!(byte_error.code(), HostLinkErrorCodeV1::FieldTooLarge);
}

#[test]
fn response_and_script_escape_families_reject() {
    for (name, bytes, argument, expected) in [
        (
            "nested.rsp",
            b"@other.rsp".as_slice(),
            PlanArgumentV1::ResponseFile {
                root: "sysroot".to_owned(),
                relative_path: b"nested.rsp".to_vec(),
            },
            HostLinkErrorCodeV1::NestedResponseFile,
        ),
        (
            "search.ld",
            b"SEARCH_DIR(/tmp)".as_slice(),
            PlanArgumentV1::FixedRootInput {
                root: "sysroot".to_owned(),
                relative_path: b"search.ld".to_vec(),
                kind: RootInputKindV1::LinkerScript,
            },
            HostLinkErrorCodeV1::ScriptSearchDir,
        ),
        (
            "include.ld",
            b"INCLUDE other.ld".as_slice(),
            PlanArgumentV1::FixedRootInput {
                root: "sysroot".to_owned(),
                relative_path: b"include.ld".to_vec(),
                kind: RootInputKindV1::LinkerScript,
            },
            HostLinkErrorCodeV1::ScriptInclude,
        ),
        (
            "absolute.ld",
            b"INPUT(/tmp/input.o)".as_slice(),
            PlanArgumentV1::FixedRootInput {
                root: "sysroot".to_owned(),
                relative_path: b"absolute.ld".to_vec(),
                kind: RootInputKindV1::LinkerScript,
            },
            HostLinkErrorCodeV1::AbsoluteNestedPath,
        ),
    ] {
        let root = TempDir::new().unwrap();
        source(&root, name, bytes, 0o644);
        let fixed = FixedRootV1::open("sysroot", root.path()).unwrap();
        let error = HostLinkClosureV1::prepare(
            decoded_plan(vec![argument]),
            FixedRootSetV1::new(vec![fixed]).unwrap(),
            HostArtifactCatalogV1::new(nonce(), target()),
        )
        .err()
        .expect("hostile control file must reject");
        assert_eq!(error.code(), expected);
    }
}

#[test]
fn catalog_binding_replay_and_digest_substitution_reject() {
    let root = TempDir::new().unwrap();
    let wrong_nonce = ReleaseNonceV1::new([0x31; 32]).unwrap();
    let artifact = published_with(
        &root,
        "wrong-nonce.o",
        HostArtifactKindV1::Object,
        ArtifactProvenanceV1::Compiler,
        wrong_nonce,
        target(),
    )
    .unwrap();
    let mut catalog = HostArtifactCatalogV1::new(nonce(), target());
    assert_eq!(
        catalog.insert(artifact).unwrap_err().code(),
        HostLinkErrorCodeV1::WrongNonce
    );

    let artifact = published_with(
        &root,
        "wrong-target.o",
        HostArtifactKindV1::Object,
        ArtifactProvenanceV1::Compiler,
        nonce(),
        TargetTripleV1::new("aarch64-unknown-linux-gnu").unwrap(),
    )
    .unwrap();
    assert_eq!(
        catalog.insert(artifact).unwrap_err().code(),
        HostLinkErrorCodeV1::WrongTarget
    );

    let absent = ArtifactIdV1::from_sha256(Sha256Digest::from_bytes([0x82; 32]));
    let error = HostLinkClosureV1::prepare(
        decoded_plan(vec![PlanArgumentV1::CatalogArtifact(absent)]),
        FixedRootSetV1::new(vec![]).unwrap(),
        catalog,
    )
    .err()
    .expect("absent catalog identity must reject");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ReplayMismatch);

    let expected = Sha256Digest::from_bytes([0x91; 32]);
    let spec = ProducerArtifactSpecV1::new(
        "digest-substitution.o",
        HostArtifactKindV1::Object,
        ArtifactProvenanceV1::Compiler,
        nonce(),
        target(),
    )
    .unwrap()
    .with_expected_sha256(expected);
    assert_eq!(
        PublishedHostArtifactV1::from_producer_fd(
            source(&root, "digest-substitution.o", &minimal_elf(1), 0o644,),
            spec,
        )
        .err()
        .expect("producer digest substitution must reject")
        .code(),
        HostLinkErrorCodeV1::DigestMismatch
    );
}
