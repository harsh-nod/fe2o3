#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use fe2o3_host_link_closure::{
    ApprovedStaticHostLldV1, ArtifactProvenanceV1, ElfClassV1, ElfEndianV1, ElfProfileV1,
    ExecutableToolchainV1, FixedRootSetV1, HostArtifactCatalogV1, HostArtifactKindV1,
    HostLinkClosureV1, HostLinkErrorCodeV1, HostLinkHandoffV1, HostLinkPlanSpecV1, HostLinkPlanV1,
    OutputTypeV1, PlanArgumentV1, ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1,
    RuntimeDsoClosureV1, Sha256Digest, TargetTripleV1,
    authenticated_host_link_available_capacity_v1, sha256_bytes,
};
use rustix::fs::SealFlags;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const TOOL_ENV: &str = "FE2O3_TEST_STATIC_HOST_LLD";
const ACCEPTED_TOOL_SHA256: &str =
    "7c1a7429e93896393eb743ed54ead78ec6d492e3ed887183e67737b3872d7bf9";
const ACCEPTED_TOOL_SIZE: u64 = 85_597_472;
const EXPECTED_EXIT_CODE: i32 = 37;
const EXACT_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

struct VerifiedStaticHostLld {
    path: PathBuf,
    file: File,
    sha256: Sha256Digest,
}

struct PreparedClosure {
    closure: HostLinkClosureV1,
    request_argument: Vec<u8>,
}

struct LinkedOutput {
    file: File,
    bytes: Vec<u8>,
    sha256: Sha256Digest,
    size: u64,
    plan_digest: Sha256Digest,
    closure_digest: Sha256Digest,
    nonce_sha256: Sha256Digest,
}

fn release_nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x47; 32]).expect("test release nonce is nonzero")
}

fn target() -> TargetTripleV1 {
    TargetTripleV1::new("x86_64-unknown-linux-gnu").expect("test target is canonical")
}

fn expected_output_profile() -> ElfProfileV1 {
    ElfProfileV1 {
        class: ElfClassV1::Elf64,
        endian: ElfEndianV1::Little,
        elf_type: object::elf::ET_EXEC,
        machine: object::elf::EM_X86_64,
        interpreter: None,
        soname: None,
        needed: vec![],
        has_writable_executable_segment: false,
        has_executable_stack: false,
    }
}

fn verify_external_tool() -> VerifiedStaticHostLld {
    let path = std::env::var_os(TOOL_ENV)
        .unwrap_or_else(|| panic!("{TOOL_ENV} must be set to the exact accepted static host LLD"));
    assert!(!path.is_empty(), "{TOOL_ENV} must not be empty");
    let path = PathBuf::from(path);
    assert!(
        path.is_absolute(),
        "{TOOL_ENV} must name an absolute path: {}",
        path.display()
    );
    let file = File::open(&path).unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    let metadata = file
        .metadata()
        .unwrap_or_else(|error| panic!("fstat {}: {error}", path.display()));
    assert_tool_metadata(&path, &metadata);

    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = rustix::io::pread(&file, &mut buffer, offset)
            .unwrap_or_else(|error| panic!("hash {} at {offset}: {error}", path.display()));
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        offset = offset
            .checked_add(count as u64)
            .expect("accepted tool hash offset overflowed");
    }
    assert_eq!(
        offset,
        ACCEPTED_TOOL_SIZE,
        "{} changed size while it was hashed",
        path.display()
    );
    let sha256 = Sha256Digest::from_bytes(hasher.finalize().into());
    assert_eq!(
        sha256.to_string(),
        ACCEPTED_TOOL_SHA256,
        "{} is not the accepted static fe2o3-host-lld",
        path.display()
    );
    let after = file
        .metadata()
        .unwrap_or_else(|error| panic!("re-fstat {}: {error}", path.display()));
    assert_eq!(
        metadata.dev(),
        after.dev(),
        "{} changed device while it was hashed",
        path.display()
    );
    assert_eq!(
        metadata.ino(),
        after.ino(),
        "{} changed inode while it was hashed",
        path.display()
    );
    assert_eq!(
        metadata.len(),
        after.len(),
        "{} changed length while it was hashed",
        path.display()
    );
    assert_eq!(
        metadata.mtime(),
        after.mtime(),
        "{} changed mtime while it was hashed",
        path.display()
    );
    assert_eq!(
        metadata.mtime_nsec(),
        after.mtime_nsec(),
        "{} changed mtime while it was hashed",
        path.display()
    );

    VerifiedStaticHostLld { path, file, sha256 }
}

fn assert_tool_metadata(path: &Path, metadata: &Metadata) {
    assert!(
        metadata.is_file(),
        "{} is not a regular file",
        path.display()
    );
    assert_eq!(
        metadata.len(),
        ACCEPTED_TOOL_SIZE,
        "{} has the wrong accepted-tool size",
        path.display()
    );
    assert_ne!(
        metadata.mode() & 0o111,
        0,
        "{} has no executable mode bit",
        path.display()
    );
}

fn assemble_start(directory: &TempDir, stem: &str, exit_code: u8) -> File {
    let source = directory.path().join(format!("{stem}.s"));
    let object = directory.path().join(format!("{stem}.o"));
    let mut source_file = File::create(&source)
        .unwrap_or_else(|error| panic!("create {}: {error}", source.display()));
    writeln!(
        source_file,
        ".global _start\n.type _start,@function\n.text\n_start:\n  mov $60, %rax\n  mov ${exit_code}, %rdi\n  syscall\n.size _start, .-_start\n.section .note.GNU-stack,\"\",@progbits"
    )
    .unwrap_or_else(|error| panic!("write {}: {error}", source.display()));
    source_file
        .sync_all()
        .unwrap_or_else(|error| panic!("sync {}: {error}", source.display()));

    let result = Command::new("/usr/bin/as")
        .env_clear()
        .args(["--64", "-o"])
        .arg(&object)
        .arg(&source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("launch /usr/bin/as for {}: {error}", source.display()));
    assert!(
        result.status.success(),
        "/usr/bin/as failed for {}: {}",
        source.display(),
        String::from_utf8_lossy(&result.stderr)
    );

    let file =
        File::open(&object).unwrap_or_else(|error| panic!("open {}: {error}", object.display()));
    let mut header = [0_u8; 20];
    assert_eq!(
        rustix::io::pread(&file, &mut header, 0).expect("read ET_REL header"),
        header.len()
    );
    assert_eq!(&header[..4], b"\x7fELF");
    assert_eq!(header[4], object::elf::ELFCLASS64);
    assert_eq!(header[5], object::elf::ELFDATA2LSB);
    assert_eq!(
        u16::from_le_bytes([header[16], header[17]]),
        object::elf::ET_REL
    );
    assert_eq!(
        u16::from_le_bytes([header[18], header[19]]),
        object::elf::EM_X86_64
    );
    file
}

fn publish_tool(
    tool: &VerifiedStaticHostLld,
    label: &str,
    kind: HostArtifactKindV1,
) -> PublishedHostArtifactV1 {
    let spec = ProducerArtifactSpecV1::new(
        label,
        kind,
        ArtifactProvenanceV1::Compiler,
        release_nonce(),
        target(),
    )
    .expect("accepted tool spec is canonical")
    .with_expected_sha256(tool.sha256);
    PublishedHostArtifactV1::from_producer_fd(
        tool.file
            .try_clone()
            .unwrap_or_else(|error| panic!("clone verified {}: {error}", tool.path.display())),
        spec,
    )
    .unwrap_or_else(|error| {
        panic!(
            "publish verified {} as {label}: {error}",
            tool.path.display()
        )
    })
}

fn publish_object(object: &File) -> PublishedHostArtifactV1 {
    let artifact = PublishedHostArtifactV1::from_producer_fd(
        object.try_clone().expect("clone assembled ET_REL"),
        ProducerArtifactSpecV1::new(
            "input.o",
            HostArtifactKindV1::Object,
            ArtifactProvenanceV1::Compiler,
            release_nonce(),
            target(),
        )
        .expect("object spec is canonical"),
    )
    .expect("publish assembled ET_REL as a sealed typed input");
    let profile = artifact
        .identity()
        .elf_profile
        .as_ref()
        .expect("object publication parsed an ELF profile");
    assert_eq!(profile.elf_type, object::elf::ET_REL);
    assert_eq!(profile.machine, object::elf::EM_X86_64);
    artifact
}

fn prepare_closure(tool: &VerifiedStaticHostLld, object: &File) -> PreparedClosure {
    let wrapper = publish_tool(tool, "static-wrapper", HostArtifactKindV1::StaticWrapper);
    let wrapper_id = wrapper.id();
    let lld = publish_tool(tool, "static-host-lld", HostArtifactKindV1::StaticHostLld);
    let lld_id = lld.id();
    let object = publish_object(object);
    let object_id = object.id();
    let spec = HostLinkPlanSpecV1 {
        release_nonce: release_nonce(),
        target: target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-static-accepted".to_owned(),
        },
        output_type: OutputTypeV1::Executable,
        expected_output_mode: 0o555,
        expected_output_elf: expected_output_profile(),
        arguments: vec![PlanArgumentV1::ProducerArtifact(object_id)],
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![object, lld, wrapper])
        .expect("construct canonical real-tool handoff");
    let (plan, producer_fds) = handoff.into_parts();
    let plan = HostLinkPlanV1::from_sealed_fd(plan, producer_fds)
        .expect("decode canonical sealed real-tool plan");
    let mut closure = HostLinkClosureV1::prepare(
        plan,
        FixedRootSetV1::new(vec![]).expect("empty fixed-root set is canonical"),
        HostArtifactCatalogV1::new(release_nonce(), target()),
    )
    .expect("prepare real static-LLD closure");
    closure
        .prevalidate()
        .expect("prevalidate real static-LLD closure");

    let arguments = closure
        .lld_argv()
        .expect("prevalidated closure exposes canonical argv")
        .canonical_arguments();
    assert_eq!(
        arguments.first().map(Vec::as_slice),
        Some(b"fe2o3-host-lld".as_slice())
    );
    assert_eq!(
        arguments.get(1).map(Vec::as_slice),
        Some(b"--fe2o3-host-lld-elf-v2".as_slice())
    );
    assert_eq!(
        arguments
            .iter()
            .filter(|argument| argument.starts_with(b"--fe2o3-input-v1="))
            .count(),
        1
    );
    let request_argument = arguments
        .iter()
        .find(|argument| argument.starts_with(b"--fe2o3-request-v1="))
        .expect("canonical argv carries a request binding")
        .clone();
    assert_request_binding(&closure, &request_argument);
    PreparedClosure {
        closure,
        request_argument,
    }
}

fn assert_request_binding(closure: &HostLinkClosureV1, request: &[u8]) {
    let expected = format!(
        "--fe2o3-request-v1={}:{}:{}",
        closure.plan_digest(),
        closure.closure_digest(),
        closure.nonce_sha256()
    );
    assert_eq!(request, expected.as_bytes());
}

#[allow(unsafe_code)]
fn approve_verified_tool(
    closure: &HostLinkClosureV1,
    evidence: &VerifiedStaticHostLld,
) -> ApprovedStaticHostLldV1 {
    assert_eq!(evidence.sha256.to_string(), ACCEPTED_TOOL_SHA256);
    assert_eq!(
        evidence
            .file
            .metadata()
            .expect("re-fstat approved tool")
            .len(),
        ACCEPTED_TOOL_SIZE
    );
    // SAFETY: this test crosses the production authority boundary only after hashing and sizing
    // the already-open executable descriptor against the accepted static-tool evidence. The
    // closure's tool artifacts were captured from clones of that exact descriptor with an
    // expected-digest check, and `from_verified_evidence` revalidates the complete sealed plan.
    unsafe { ApprovedStaticHostLldV1::from_verified_evidence(closure) }
        .expect("mint test-only approval for the digest-verified static tool")
}

fn run_real_link(prepared: PreparedClosure, tool: &VerifiedStaticHostLld) -> LinkedOutput {
    let approval = approve_verified_tool(&prepared.closure, tool);
    let plan_digest = prepared.closure.plan_digest();
    let closure_digest = prepared.closure.closure_digest();
    let nonce_sha256 = prepared.closure.nonce_sha256();
    let mut execution = prepared
        .closure
        .launch(approval)
        .expect("launch the exact sealed static LLD descriptor");
    assert_eq!(execution.plan_digest(), plan_digest);
    assert_eq!(execution.closure_digest(), closure_digest);
    assert_eq!(execution.nonce_sha256(), nonce_sha256);

    let outer_deadline = Instant::now() + Duration::from_secs(35);
    loop {
        let poll_started = Instant::now();
        match execution.try_admit_output() {
            Ok(admitted) => {
                assert!(
                    poll_started.elapsed() < Duration::from_secs(1),
                    "successful admission poll exceeded its bounded work quantum"
                );
                assert_eq!(admitted.mode(), 0o555);
                assert_eq!(admitted.elf_profile(), &expected_output_profile());
                assert_eq!(admitted.plan_digest(), plan_digest);
                assert_eq!(admitted.closure_digest(), closure_digest);
                let file = admitted
                    .try_clone_file()
                    .expect("clone admitted receiver-owned output");
                let metadata = file.metadata().expect("fstat admitted output");
                assert!(metadata.is_file());
                assert_eq!(metadata.nlink(), 0, "admitted output is namespace-linked");
                assert_eq!(metadata.mode() & 0o7777, 0o555);
                assert_eq!(metadata.len(), admitted.size());
                assert_eq!(
                    rustix::fs::fcntl_get_seals(&file).expect("read admitted output seals"),
                    EXACT_SEALS
                );
                let bytes = read_exact_descriptor(&file, admitted.size());
                assert_eq!(&bytes[..4], b"\x7fELF");
                assert_eq!(
                    u16::from_le_bytes([bytes[16], bytes[17]]),
                    object::elf::ET_EXEC
                );
                assert_eq!(sha256_bytes(&bytes), admitted.sha256());
                return LinkedOutput {
                    file,
                    bytes,
                    sha256: admitted.sha256(),
                    size: admitted.size(),
                    plan_digest,
                    closure_digest,
                    nonce_sha256,
                };
            }
            Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {
                assert!(
                    poll_started.elapsed() < Duration::from_secs(1),
                    "pending admission poll exceeded its bounded work quantum"
                );
                assert!(
                    Instant::now() < outer_deadline,
                    "real static LLD exceeded the integration test deadline"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("real static LLD protocol/admission failed: {error}"),
        }
    }
}

fn read_exact_descriptor(file: &File, size: u64) -> Vec<u8> {
    let size = usize::try_from(size).expect("admitted output fits this process");
    let mut bytes = vec![0_u8; size];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let count = rustix::io::pread(file, &mut bytes[offset..], offset as u64)
            .expect("pread admitted output");
        assert_ne!(count, 0, "admitted output ended before its bound size");
        offset += count;
    }
    let mut extra = [0_u8; 1];
    assert_eq!(
        rustix::io::pread(file, &mut extra, size as u64).expect("probe admitted output end"),
        0
    );
    bytes
}

fn assert_output_behavior(output: &LinkedOutput) {
    let path = format!("/proc/self/fd/{}", output.file.as_raw_fd());
    let status = Command::new(&path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|error| panic!("execute admitted descriptor {path}: {error}"));
    assert_eq!(
        status.code(),
        Some(EXPECTED_EXIT_CODE),
        "admitted ET_EXEC did not implement the linked _start behavior"
    );
}

#[test]
#[ignore = "requires exact accepted static host LLD via FE2O3_TEST_STATIC_HOST_LLD"]
fn real_static_lld_closure_vertical_slice() {
    let tool = verify_external_tool();
    let directory = TempDir::new().expect("create real-link test directory");
    let object = assemble_start(&directory, "exit-37", EXPECTED_EXIT_CODE as u8);
    let different_object = assemble_start(&directory, "exit-38", 38);

    let first = prepare_closure(&tool, &object);
    let stale_approval = approve_verified_tool(&first.closure, &tool);
    let different = prepare_closure(&tool, &different_object);
    assert_ne!(first.closure.plan_digest(), different.closure.plan_digest());
    let capacity_before = authenticated_host_link_available_capacity_v1();
    let error = different
        .closure
        .launch(stale_approval)
        .err()
        .expect("approval for another real-tool plan must fail before launch");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ToolApproval);
    assert_eq!(
        authenticated_host_link_available_capacity_v1(),
        capacity_before,
        "rejected approval reserved authenticated child capacity"
    );

    let second = prepare_closure(&tool, &object);
    assert_eq!(first.closure.plan_digest(), second.closure.plan_digest());
    assert_eq!(
        first.closure.closure_digest(),
        second.closure.closure_digest()
    );
    assert_ne!(first.closure.nonce_sha256(), second.closure.nonce_sha256());
    assert_ne!(first.request_argument, second.request_argument);
    assert_request_binding(&first.closure, &first.request_argument);
    assert_request_binding(&second.closure, &second.request_argument);

    let first_output = run_real_link(first, &tool);
    let second_output = run_real_link(second, &tool);
    assert_eq!(first_output.plan_digest, second_output.plan_digest);
    assert_eq!(first_output.closure_digest, second_output.closure_digest);
    assert_ne!(first_output.nonce_sha256, second_output.nonce_sha256);
    assert_eq!(first_output.sha256, second_output.sha256);
    assert_eq!(first_output.size, second_output.size);
    assert_eq!(first_output.bytes, second_output.bytes);
    assert_output_behavior(&first_output);
    assert_output_behavior(&second_output);
}
