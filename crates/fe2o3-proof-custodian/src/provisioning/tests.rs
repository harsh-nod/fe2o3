use super::*;
use fe2o3_kernel_analysis::{
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineRuntimeClosureIdentityV1, PhysicalMachineToolchainIdentityV1,
};
use std::os::unix::fs::{FileTypeExt, PermissionsExt, symlink};

mod genuine_application;

#[test]
#[ignore = "run through scripts/build-static-proof-custodian.sh"]
fn release_images_satisfy_production_static_elf_profile() {
    let directory = PathBuf::from(
        std::env::var_os("FE2O3_STATIC_PROOF_CUSTODIAN_DIR").expect("static proof image directory"),
    );
    for name in [
        "fe2o3-proof-manager",
        "fe2o3-native-application-manager",
        "fe2o3-application-proof-controller",
        "fe2o3-native-application-proof-controller",
        "fe2o3-proof-custodian-provision",
    ] {
        let bytes = std::fs::read(directory.join(name)).unwrap();
        assert!(bytes.len() <= 128 * 1024 * 1024, "{name}");
        assert_ne!(
            fe2o3_runtime_protocol::sealed_static_application_identity_v1(&bytes)
                .unwrap_or_else(|error| panic!("{name}: {error}")),
            [0; 32]
        );
    }
}

fn compiler_profile() -> CompilerExecutionClientProfileV1 {
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };
    let policy = CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 100).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([2; 32], 200).unwrap(),
        ed25519_dalek::SigningKey::from_bytes(&[3; 32])
            .verifying_key()
            .to_bytes(),
        ed25519_dalek::SigningKey::from_bytes(&[4; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    CompilerExecutionClientProfileV1::new(
        61000,
        62000,
        CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 62001).unwrap(),
        policy,
    )
    .unwrap()
}

#[test]
fn proof_uid_and_gid_must_each_be_separate_from_signing_services() {
    let profile = compiler_profile();
    assert!(separate_credentials(candidate().application.credentials().unwrap(), &profile).is_ok());
    for (uid, gid) in [
        (61000, 63000),
        (61001, 63000),
        (63000, 62000),
        (63000, 62001),
    ] {
        assert!(
            separate_credentials(
                ProofControllerCredentialProfileV1::new(uid, gid).unwrap(),
                &profile
            )
            .is_err()
        );
    }
    for (uid, gid) in [(0, 63000), (63000, 0), (u32::MAX, 63000), (63000, u32::MAX)] {
        assert!(ProofControllerCredentialProfileV1::new(uid, gid).is_err());
    }
}

fn candidate() -> Candidate {
    let policy = PhysicalMachineEffectWorkerPolicyV1::new(
        PhysicalMachineWorkerExecutableIdentityV1::calculate(b"worker"),
        PhysicalMachineRuntimeClosureIdentityV1::from_parts([4; 32], 5),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([6; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([7; 32]),
    )
    .unwrap();
    let application = ProofCustodianDeploymentV1::new(
        ProofControllerCredentialProfileV1::new(61002, 61003).unwrap(),
        [3; 32],
        9,
        policy,
        [8; 32],
    )
    .unwrap();
    let manager =
        ProofManagerDeploymentV1::new(([9; 32], 10), ([10; 32], 11), application.identity())
            .unwrap();
    Candidate {
        application,
        manager,
    }
}

#[test]
fn candidate_is_exact_canonical_pair_with_matching_link() {
    let bytes = candidate().bytes();
    assert_eq!(Candidate::decode(&bytes).unwrap().bytes(), bytes);
    for i in 0..bytes.len() {
        let mut changed = bytes;
        changed[i] ^= 1;
        assert!(Candidate::decode(&changed).is_err(), "mutation {i}");
    }
    for len in 0..bytes.len() {
        assert!(Candidate::decode(&bytes[..len]).is_err());
    }
    assert!(Candidate::decode(&[bytes.as_slice(), &[0]].concat()).is_err());
    let mut wrong_link = candidate();
    wrong_link.manager =
        ProofManagerDeploymentV1::new(([9; 32], 10), ([10; 32], 11), [42; 32]).unwrap();
    assert!(Candidate::decode(&wrong_link.bytes()).is_err());
    assert!(
        Candidate::decode(&[&bytes[DEPLOYMENT_BYTES..], &bytes[..DEPLOYMENT_BYTES]].concat())
            .is_err()
    );
}

#[test]
fn cli_is_closed_and_pin_is_independent_lowercase_hex() {
    let parse_text = |args: &[&str]| parse(&args.iter().map(OsString::from).collect::<Vec<_>>());
    assert!(matches!(
        parse_text(&["inspect-fixed-resources", "out"]).unwrap(),
        Command::Inspect(_)
    ));
    assert!(matches!(
        parse_text(&["install", "in", &"ab".repeat(32)]).unwrap(),
        Command::Install(_, [0xab, ..])
    ));
    for args in [
        vec![],
        vec!["inspect-fixed-resources"],
        vec!["inspect-fixed-resources", "out", "extra"],
        vec!["install", "in"],
        vec!["install", "in", "hash"],
        vec!["other", "out"],
    ] {
        assert!(parse_text(&args).is_err());
    }
    for hash in [
        "AB".repeat(32),
        "gg".repeat(32),
        "ab".repeat(31),
        "ab".repeat(33),
    ] {
        assert!(parse_text(&["install", "in", &hash]).is_err());
    }
}

#[test]
fn candidate_capture_rejects_wrong_pin_and_nonregular_sources() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("candidate");
    let bytes = candidate().bytes();
    let pin = Sha256::digest(bytes).into();
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(read_candidate(&path, pin).unwrap().bytes(), bytes);
    assert!(read_candidate(&path, [0; 32]).is_err());
    let alias = dir.path().join("alias");
    symlink(&path, &alias).unwrap();
    assert!(read_candidate(&alias, pin).is_err());
    std::fs::remove_file(&alias).unwrap();
    std::fs::hard_link(&path, &alias).unwrap();
    assert!(read_candidate(&path, pin).is_err());
    std::fs::remove_file(&alias).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &alias,
        rustix::fs::FileType::Fifo,
        Mode::RUSR,
        0,
    )
    .unwrap();
    assert!(read_candidate(&alias, pin).is_err());
    assert!(read_candidate(dir.path(), pin).is_err());
    std::fs::write(&path, [bytes.as_slice(), &[0]].concat()).unwrap();
    assert!(read_candidate(&path, pin).is_err());
}

fn install_test(parent: &File, candidate: &Candidate) -> io::Result<()> {
    publish(
        parent,
        candidate,
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
        || Ok(()),
    )
}

#[test]
fn atomic_pair_installs_and_reacquires_without_replacing_objects() {
    let dir = tempfile::tempdir().unwrap();
    let parent = File::open(dir.path()).unwrap();
    let candidate = candidate();
    install_test(&parent, &candidate).unwrap();
    let installed = open_directory(&parent, CONFIG_NAME).unwrap();
    let first = std::fs::metadata(dir.path().join(CONFIG_NAME).join(RECORD_NAMES[0]))
        .unwrap()
        .ino();
    install_test(&parent, &candidate).unwrap();
    assert!(same_object(&installed, &open_directory(&parent, CONFIG_NAME).unwrap()).unwrap());
    assert_eq!(
        std::fs::metadata(dir.path().join(CONFIG_NAME).join(RECORD_NAMES[0]))
            .unwrap()
            .ino(),
        first
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn malformed_existing_installations_are_preserved_and_rejected() {
    for case in [
        "partial",
        "extra",
        "content",
        "writable",
        "symlink",
        "hardlink",
        "fifo",
        "xattr",
        "directory",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let parent = File::open(dir.path()).unwrap();
        let candidate = candidate();
        install_test(&parent, &candidate).unwrap();
        let path = dir.path().join(CONFIG_NAME).join(RECORD_NAMES[1]);
        match case {
            "partial" => std::fs::remove_file(&path).unwrap(),
            "extra" => std::fs::write(
                dir.path().join(CONFIG_NAME).join("unrelated"),
                b"leave alone",
            )
            .unwrap(),
            "content" => {
                let mut bytes = std::fs::read(&path).unwrap();
                bytes[0] ^= 1;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
                std::fs::write(&path, bytes).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
            }
            "writable" => {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap()
            }
            "symlink" | "fifo" => {
                std::fs::remove_file(&path).unwrap();
                if case == "symlink" {
                    symlink(RECORD_NAMES[0], &path).unwrap();
                } else {
                    rustix::fs::mknodat(
                        rustix::fs::CWD,
                        &path,
                        rustix::fs::FileType::Fifo,
                        Mode::RUSR,
                        0,
                    )
                    .unwrap();
                }
            }
            "hardlink" => std::fs::hard_link(&path, dir.path().join("outside-link")).unwrap(),
            "xattr" => {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
                rustix::fs::setxattr(
                    &path,
                    "user.fe2o3-test",
                    b"x",
                    rustix::fs::XattrFlags::empty(),
                )
                .unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
            }
            "directory" => std::fs::set_permissions(
                dir.path().join(CONFIG_NAME),
                std::fs::Permissions::from_mode(0o777),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        assert!(install_test(&parent, &candidate).is_err(), "case {case}");
        assert!(dir.path().join(CONFIG_NAME).exists());
    }
}

#[test]
fn interruption_never_publishes_partial_pair_and_postrename_failure_is_explicit() {
    for fail_at in 0..5 {
        let dir = tempfile::tempdir().unwrap();
        let parent = File::open(dir.path()).unwrap();
        let candidate = candidate();
        let mut calls = 0;
        let result = publish(
            &parent,
            &candidate,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
            || {
                let current = calls;
                calls += 1;
                require(current != fail_at, "injected resource invalidation")
            },
        );
        let error = result.unwrap_err();
        if fail_at < 4 {
            assert_eq!(
                std::fs::read_dir(dir.path()).unwrap().count(),
                0,
                "failure {fail_at}"
            );
        } else {
            assert!(error.to_string().contains("may already be installed"));
            install_test(&parent, &candidate).unwrap();
        }
    }
}

#[test]
fn stale_staging_is_not_deleted_or_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let stale = dir.path().join(format!("{STAGING_PREFIX}unrecognized"));
    std::fs::write(&stale, b"must preserve").unwrap();
    let parent = File::open(dir.path()).unwrap();
    assert!(install_test(&parent, &candidate()).is_err());
    assert_eq!(std::fs::read(&stale).unwrap(), b"must preserve");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "requires scripts/qualify-proof-resource-inspection.sh private installed runtime"]
fn root_fixed_resource_inspection_campaign() {
    use std::process::Command;
    assert!(rustix::process::getuid().is_root());
    assert_eq!(std::env::var("FE2O3_PROOF_INSTALL_PRIVATE").unwrap(), "1");
    assert_ne!(
        std::fs::read_link("/proc/self/ns/pid").unwrap().as_os_str(),
        std::env::var_os("FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE").unwrap()
    );
    assert!(std::env::var_os("HOME").is_none());
    assert_eq!(std::env::current_dir().unwrap(), Path::new("/"));
    for path in [
        "/etc",
        "/usr/libexec",
        "/tmp",
        "/home",
        "/root",
        "/run/host-lib",
        "/run/setup",
    ] {
        assert_eq!(rustix::fs::statfs(path).unwrap().f_type, libc::TMPFS_MAGIC);
    }
    for path in ["/home", "/root", "/run/host-lib", "/run/setup"] {
        assert_eq!(std::fs::read_dir(path).unwrap().count(), 0, "{path}");
    }
    for path in [
        "/etc/ld.so.cache",
        "/etc/ld.so.preload",
        "/usr/lib/x86_64-linux-gnu/glibc-hwcaps",
    ] {
        assert!(!Path::new(path).exists(), "unexpected loader input: {path}");
    }
    for path in ["/etc/fe2o3", "/etc/fe2o3/compiler-execution"] {
        std::fs::create_dir(path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        compiler_profile().canonical_bytes(),
    )
    .unwrap();
    std::fs::set_permissions(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();
    let output_dir = Path::new("/run/candidates");
    std::fs::create_dir(output_dir).unwrap();
    std::os::unix::fs::chown(output_dir, Some(61002), Some(61003)).unwrap();
    std::fs::set_permissions(output_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let provisioner = "/usr/libexec/fe2o3/fe2o3-proof-custodian-provision";
    // Nested root binds must not turn the private device mounts into unusable nodev aliases.
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .unwrap();
    assert!(null.metadata().unwrap().file_type().is_char_device());
    assert_eq!(null.metadata().unwrap().rdev(), rustix::fs::makedev(1, 3));
    assert_eq!(
        std::fs::read_link("/lib64").unwrap(),
        Path::new("usr/lib64")
    );
    assert_eq!(
        std::fs::read_link("/usr/lib64/ld-linux-x86-64.so.2").unwrap(),
        Path::new("../lib/x86_64-linux-gnu/ld-linux-x86-64.so.2")
    );
    let inspect = |name: &str, success: bool| {
        let path = output_dir.join(name);
        let output = Command::new("/usr/bin/setpriv")
            .env_clear()
            .args([
                "--reuid=61002",
                "--regid=61003",
                "--clear-groups",
                "--inh-caps=-all",
                "--ambient-caps=-all",
                "--bounding-set=-all",
            ])
            .arg(provisioner)
            .arg("inspect-fixed-resources")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "inspection {name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !success {
            assert_eq!(output.status.code(), Some(98));
            assert!(!path.exists());
            println!(
                "negative {name}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return None;
        }
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert_eq!(
            (
                metadata.uid(),
                metadata.gid(),
                metadata.mode() & 0o7777,
                metadata.nlink()
            ),
            (61002, 61003, 0o400, 1)
        );
        let bytes = std::fs::read(&path).unwrap();
        let candidate = Candidate::decode(&bytes).unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("candidate_sha256={}\n", candidate.sha256_hex())
        );
        Some(candidate)
    };
    rustix::process::umask(Mode::from_raw_mode(0o077));
    // Discovery executes the real sealed worker, then admits the pinned Verus closure.
    let first = inspect("first", true).unwrap();
    let second = inspect("second", true).unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(
        first.application.credentials().unwrap(),
        ProofControllerCredentialProfileV1::new(61002, 61003).unwrap()
    );
    let resources = Resources::open().unwrap();
    resources.validate_candidate(&first).unwrap();
    for path in [
        WORKER_PATH,
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        RUNTIME_PATH,
        "/usr/lib/x86_64-linux-gnu/libzstd.so.1.5.5",
    ] {
        let hidden = format!("{path}.qualification-absent");
        std::fs::rename(path, &hidden).unwrap();
        inspect("missing-resource", false);
        std::fs::rename(&hidden, path).unwrap();
    }
    let output = Command::new(provisioner)
        .env_clear()
        .arg("install")
        .arg(output_dir.join("first"))
        .arg(first.sha256_hex())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let owner = crate::ProductionApplicationProofCustodianDeploymentV1::open().unwrap();
    owner.revalidate().unwrap();
    assert_eq!(owner.deployment(), &first.application);
    println!(
        "real fixed resource inspection: twice-identical candidate_sha256={}, proof_uid=61002 proof_gid=61003, home-hidden, installed and reacquired; compiler profile is a routing fixture, not a compiler receipt",
        first.sha256_hex()
    );
    println!("candidate_bytes={:02x?}", first.bytes());
    // Reuse the actual controller protocol campaign, not another resource-opening surrogate.
    for case in ["app-good", "app-payload", "app-duplicate", "app-stale"] {
        let capture = Path::new("/tmp").join(case);
        let status = Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("FE2O3_CUSTODIAN_PRIVATE_INSTALL", "1")
            .env(
                "FE2O3_CUSTODIAN_HOST_PID_NAMESPACE",
                std::env::var_os("FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE").unwrap(),
            )
            .env("FE2O3_CUSTODIAN_CASE", case)
            .env("FE2O3_CUSTODIAN_INPUTS", "/run/proof-inputs")
            .env("FE2O3_CUSTODIAN_CAPTURE", &capture)
            .args([
                "--exact",
                "launch::application::qualification::root_application_controller",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .status()
            .unwrap();
        assert!(status.success(), "controller case {case}");
        for name in ["preactivation.txt", "cleanup.txt"] {
            println!(
                "{case}/{name}: {}",
                std::fs::read_to_string(capture.join(name)).unwrap().trim()
            );
        }
        if case == "app-good" {
            let subject = std::fs::read(capture.join("subject.bin")).unwrap();
            assert!(!subject.is_empty());
            println!("{case}/subject_sha256={:02x?}", Sha256::digest(&subject));
            println!(
                "{case}/quarantine: {}",
                std::fs::read_to_string(capture.join("quarantine.txt"))
                    .unwrap()
                    .trim()
            );
        } else {
            println!(
                "{case}/rejection: {}",
                std::fs::read_to_string(capture.join("rejection.txt"))
                    .unwrap()
                    .trim()
            );
        }
    }
}

#[test]
#[ignore = "requires scripts/qualify-proof-deployment-install.sh private real-root namespace"]
fn root_fixed_install_campaign() {
    use std::process::Command;
    assert!(rustix::process::getuid().is_root());
    assert_eq!(std::env::var("FE2O3_PROOF_INSTALL_PRIVATE").unwrap(), "1");
    assert_ne!(
        std::fs::read_link("/proc/self/ns/pid").unwrap().as_os_str(),
        std::env::var_os("FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE").unwrap()
    );
    for path in ["/etc", "/usr/libexec", "/tmp"] {
        assert_eq!(rustix::fs::statfs(path).unwrap().f_type, libc::TMPFS_MAGIC);
    }
    let images = PathBuf::from(std::env::var_os("FE2O3_STATIC_PROOF_CUSTODIAN_DIR").unwrap());
    let provisioner = PathBuf::from("/usr/libexec/fe2o3/fe2o3-proof-custodian-provision");
    for path in [
        "/etc/fe2o3",
        "/etc/fe2o3/compiler-execution",
        "/usr/libexec/fe2o3",
    ] {
        std::fs::create_dir(path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for path in [
        APPLICATION_CONTROLLER_PATH,
        MANAGER_PATH,
        COORDINATOR_PATH,
        WORKER_PATH,
        provisioner.to_str().unwrap(),
    ] {
        let source = if path == WORKER_PATH {
            PathBuf::from(std::env::var_os("FE2O3_PROOF_INSTALL_WORKER").unwrap())
        } else if path == COORDINATOR_PATH {
            PathBuf::from(std::env::var_os("FE2O3_PROOF_INSTALL_COORDINATOR").unwrap())
        } else {
            images.join(Path::new(path).file_name().unwrap())
        };
        std::fs::copy(source, path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    std::fs::write(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        compiler_profile().canonical_bytes(),
    )
    .unwrap();
    std::fs::set_permissions(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();

    let resources = Resources::open().unwrap();
    let inert = candidate();
    let old_policy = inert.application.analyzer_policy().unwrap();
    let policy = PhysicalMachineEffectWorkerPolicyV1::new(
        PhysicalMachineWorkerExecutableIdentityV1::calculate(&resources.worker.bytes),
        old_policy.runtime_closure(),
        old_policy.analyzer(),
        old_policy.toolchain(),
    )
    .unwrap();
    let (hash, len) = resources.controller.measurement();
    let application = ProofCustodianDeploymentV1::new(
        inert.application.credentials().unwrap(),
        hash,
        len,
        policy,
        inert.application.verus_identity(),
    )
    .unwrap();
    let manager = ProofManagerDeploymentV1::new(
        resources.manager.measurement(),
        resources.coordinator.measurement(),
        application.identity(),
    )
    .unwrap();
    let good = Candidate {
        application,
        manager,
    };
    drop(resources);
    let candidate_path = Path::new("/tmp/candidate");
    let invoke = |candidate: &Candidate, pin: &str, success: bool| {
        std::fs::write(candidate_path, candidate.bytes()).unwrap();
        let output = Command::new(&provisioner)
            .env_clear()
            .arg("install")
            .arg(candidate_path)
            .arg(pin)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if success {
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("installed_candidate_sha256={}\n", candidate.sha256_hex())
            );
        } else {
            assert_eq!(output.status.code(), Some(98));
        }
    };
    let absent = || assert!(!Path::new("/etc/fe2o3/proof-custodian").exists());
    let inspect = Command::new(&provisioner)
        .env_clear()
        .args(["inspect-fixed-resources", "/tmp/root-candidate"])
        .output()
        .unwrap();
    assert_eq!(inspect.status.code(), Some(98));
    assert!(!Path::new("/tmp/root-candidate").exists());
    invoke(&good, &"00".repeat(32), false);
    absent();

    for credentials in [
        ProofControllerCredentialProfileV1::new(61000, 61003).unwrap(),
        ProofControllerCredentialProfileV1::new(61002, 62001).unwrap(),
    ] {
        let application = ProofCustodianDeploymentV1::new(
            credentials,
            hash,
            len,
            policy,
            good.application.verus_identity(),
        )
        .unwrap();
        let resources = Resources::open().unwrap();
        let manager = ProofManagerDeploymentV1::new(
            resources.manager.measurement(),
            resources.coordinator.measurement(),
            application.identity(),
        )
        .unwrap();
        let bad = Candidate {
            application,
            manager,
        };
        invoke(&bad, &bad.sha256_hex(), false);
        absent();
    }
    invoke(&inert, &inert.sha256_hex(), false);
    absent();
    let resources = Resources::open().unwrap();
    let (raw_hash, worker_len) = resources.worker.measurement();
    let wrong_policy = PhysicalMachineEffectWorkerPolicyV1::new(
        PhysicalMachineWorkerExecutableIdentityV1::from_parts(raw_hash, worker_len),
        policy.runtime_closure(),
        policy.analyzer(),
        policy.toolchain(),
    )
    .unwrap();
    let application = ProofCustodianDeploymentV1::new(
        good.application.credentials().unwrap(),
        hash,
        len,
        wrong_policy,
        good.application.verus_identity(),
    )
    .unwrap();
    let manager = ProofManagerDeploymentV1::new(
        resources.manager.measurement(),
        resources.coordinator.measurement(),
        application.identity(),
    )
    .unwrap();
    let wrong_worker = Candidate {
        application,
        manager,
    };
    invoke(&wrong_worker, &wrong_worker.sha256_hex(), false);
    absent();
    drop(resources);
    for mode in [0o755, 0o555] {
        std::fs::set_permissions(COORDINATOR_PATH, std::fs::Permissions::from_mode(mode)).unwrap();
        if mode == 0o755 {
            invoke(&good, &good.sha256_hex(), false);
            absent();
        }
    }
    let alias = Path::new("/usr/libexec/fe2o3/coordinator-test-link");
    std::fs::hard_link(COORDINATOR_PATH, alias).unwrap();
    invoke(&good, &good.sha256_hex(), false);
    absent();
    std::fs::remove_file(alias).unwrap();
    let output = Command::new("/usr/bin/setpriv")
        .args([
            "--reuid=61002",
            "--regid=61003",
            "--clear-groups",
            "--inh-caps=-all",
            "--ambient-caps=-all",
            "--bounding-set=-all",
        ])
        .arg(&provisioner)
        .arg("install")
        .arg(candidate_path)
        .arg(good.sha256_hex())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(98),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    absent();

    invoke(&good, &good.sha256_hex(), true);
    let owner = crate::ProductionApplicationProofCustodianDeploymentV1::open().unwrap();
    owner.revalidate().unwrap();
    assert_eq!(owner.deployment(), &good.application);
    let inode = |name| {
        std::fs::metadata(Path::new("/etc/fe2o3/proof-custodian").join(name))
            .unwrap()
            .ino()
    };
    let before = RECORD_NAMES.map(inode);
    invoke(&good, &good.sha256_hex(), true);
    assert_eq!(RECORD_NAMES.map(inode), before);
    invoke(&inert, &inert.sha256_hex(), false);
    assert_eq!(RECORD_NAMES.map(inode), before);
    owner.revalidate().unwrap();
    std::fs::remove_file("/etc/fe2o3/proof-custodian/manager-deployment-v1").unwrap();
    invoke(&good, &good.sha256_hex(), false);
    assert_eq!(inode(RECORD_NAMES[0]), before[0]);
    println!(
        "root installation: fixed images, independent pin, credential/metadata rejection, atomic pair, unchanged reinstallation passed; analyzer/runtime identities are inert fixture facts"
    );
}
