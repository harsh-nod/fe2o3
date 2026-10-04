//! Explicit isolated-root qualification; never run against a host installation.

use super::*;
use crate::deployment::{
    CONFIG_PATH, CONTROLLER_PATH, ProofCustodianDeploymentV1, RUNTIME_PATH, WORKER_PATH,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineWorkerExecutableIdentityV1, inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_protected_service_profile::ProofControllerCredentialProfileV1;
use std::{fs, io::Read, os::unix::fs::PermissionsExt, path::PathBuf};

fn path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).expect(name))
}

#[test]
#[ignore = "requires private pinned runtime, fixed installed worker/controller and an explicit fresh output path"]
fn prepare_fixed_deployment() {
    assert!(!rustix::process::getuid().is_root());
    let controller = fs::read(CONTROLLER_PATH).unwrap();
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let policy = inspect_physical_machine_effect_worker_candidate_v1(WORKER_PATH, limits)
        .unwrap()
        .policy();
    let runtime =
        fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(RUNTIME_PATH).unwrap();
    let config = ProofCustodianDeploymentV1::new(
        ProofControllerCredentialProfileV1::new(61000, 61000).unwrap(),
        wire::digest(&controller),
        controller.len() as u64,
        policy,
        runtime.identity().as_bytes(),
    )
    .unwrap();
    let output = path("FE2O3_CUSTODIAN_DEPLOYMENT_OUTPUT");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    use std::io::Write;
    file.write_all(config.canonical_bytes()).unwrap();
}

fn scopes() -> Vec<PathBuf> {
    let member = fs::read_to_string("/proc/thread-self/cgroup").unwrap();
    let relative = member
        .strip_prefix("0::/")
        .unwrap()
        .strip_suffix('\n')
        .unwrap();
    let parent = PathBuf::from("/sys/fs/cgroup").join(relative);
    let mut result = fs::read_dir(parent)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(b"fe2o3-proof-")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn rewrite_config(case: &str) {
    let old = ProofCustodianDeploymentV1::decode(&fs::read(CONFIG_PATH).unwrap()).unwrap();
    let measurement = old.controller_measurement().unwrap();
    let mut runtime = old.verus_identity();
    let mut policy = old.analyzer_policy().unwrap();
    let mut controller = measurement.sha256();
    match case {
        "worker" => {
            let mut hash = policy.executable().sha256();
            hash[0] ^= 1;
            policy = PhysicalMachineEffectWorkerPolicyV1::new(
                PhysicalMachineWorkerExecutableIdentityV1::from_parts(
                    hash,
                    policy.executable().byte_len(),
                ),
                policy.runtime_closure(),
                policy.analyzer(),
                policy.toolchain(),
            )
            .unwrap();
        }
        "runtime" => runtime[0] ^= 1,
        "controller" => controller[0] ^= 1,
        _ => panic!("unsupported replacement"),
    }
    let new = ProofCustodianDeploymentV1::new(
        old.credentials().unwrap(),
        controller,
        measurement.byte_len(),
        policy,
        runtime,
    )
    .unwrap();
    fs::write(CONFIG_PATH, new.canonical_bytes()).unwrap();
}

#[test]
#[ignore = "requires private real-root PID namespace, writable dedicated cgroup and private installed deployment"]
fn root_production_launcher() {
    assert!(rustix::process::getuid().is_root());
    assert_eq!(
        std::env::var("FE2O3_CUSTODIAN_PRIVATE_INSTALL").unwrap(),
        "1"
    );
    assert_ne!(
        fs::read_link("/proc/self/ns/pid").unwrap().as_os_str(),
        std::env::var_os("FE2O3_CUSTODIAN_HOST_PID_NAMESPACE").unwrap(),
        "qualification must not run in the outer PID namespace",
    );
    for path in ["/etc", "/usr/libexec"] {
        assert_eq!(rustix::fs::statfs(path).unwrap().f_type, libc::TMPFS_MAGIC);
    }
    assert!(
        fs::read_to_string("/proc/thread-self/cgroup")
            .unwrap()
            .contains("/fe2o3-custodian-qual-")
    );
    let case = std::env::var("FE2O3_CUSTODIAN_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "good"
            | "cancel"
            | "cancel-ready"
            | "worker"
            | "runtime"
            | "payload"
            | "controller"
            | "replace"
            | "fifo"
            | "poison"
            | "scope-drop"
    ));
    let output = path("FE2O3_CUSTODIAN_CAPTURE");
    fs::create_dir(&output).unwrap();
    fs::write(
        output.join("pid-namespace.txt"),
        fs::read_link("/proc/self/ns/pid")
            .unwrap()
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    let before = scopes();
    assert!(
        before.is_empty(),
        "qualification requires its own outer scope"
    );
    if case == "scope-drop" {
        let result: io::Result<()> = (|| {
            let _scope = Scope::create()?;
            assert_eq!(scopes().len(), 1);
            Err(io::Error::other("injected owner failure"))
        })();
        assert_eq!(result.unwrap_err().to_string(), "injected owner failure");
        assert_eq!(scopes(), before, "creation guard leaked its empty scope");
        fs::write(
            output.join("cleanup.txt"),
            b"empty scope removed on injected owner error\n",
        )
        .unwrap();
        return;
    }
    if matches!(case.as_str(), "worker" | "runtime" | "controller") {
        rewrite_config(&case);
    }
    if case == "fifo" {
        fs::remove_file(CONFIG_PATH).unwrap();
        rustix::fs::mknodat(
            rustix::fs::CWD,
            CONFIG_PATH,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR,
            0,
        )
        .unwrap();
    }
    let installed = ProductionProofCustodianDeploymentV1::open();
    if matches!(case.as_str(), "controller" | "fifo") {
        let error = installed.err().expect("malformed installation accepted");
        fs::write(output.join("rejection.txt"), error.to_string()).unwrap();
        assert_eq!(scopes(), before);
        return;
    }
    let installed = installed.unwrap();
    if case == "replace" {
        let bytes = fs::read(CONFIG_PATH).unwrap();
        fs::remove_file(CONFIG_PATH).unwrap();
        fs::write(CONFIG_PATH, bytes).unwrap();
        fs::set_permissions(CONFIG_PATH, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(installed.revalidate().is_err());
        assert_eq!(scopes(), before);
        return;
    }
    let source = path("FE2O3_CUSTODIAN_INPUTS");
    let envelope = fs::read(source.join("envelope.bin")).unwrap();
    let mut payload = fs::read(source.join("payload.hsaco")).unwrap();
    let kernel: [u8; 32] = fs::read(source.join("kernel.bin"))
        .unwrap()
        .try_into()
        .unwrap();
    if case == "payload" {
        payload.push(0);
    }
    let mut sentinel = std::process::Command::new("/bin/sleep")
        .arg("400")
        .spawn()
        .unwrap();
    let launched = installed.launch(&envelope, &payload, KernelId::from_bytes(kernel));
    let result = (|| -> io::Result<()> {
        if matches!(case.as_str(), "worker" | "runtime") {
            let error = launched.err().expect("wrong resource measurement accepted");
            fs::write(output.join("rejection.txt"), error.to_string())?;
            return Ok(());
        }
        let mut controller = launched?;
        let pid = controller.child_pid();
        let pidfd = controller.try_clone_pidfd()?;
        fs::write(output.join("controller-pid.txt"), pid.to_string())?;
        assert_eq!(scopes().len(), 1);
        if case == "cancel-ready" {
            controller.cancel()?;
            controller.cancel()?;
        } else if case == "cancel" {
            controller.send(wire::START)?;
            let verifier = wait_for_verifier(&controller)?;
            fs::write(output.join("verifier-command.bin"), verifier)?;
            controller.cancel()?;
        } else if case == "payload" {
            let error = controller.prove().err().expect("altered payload accepted");
            fs::write(output.join("rejection.txt"), error.to_string())?;
        } else {
            let mut proof = controller.prove()?;
            fs::write(output.join("subject.bin"), proof.subject_bytes())?;
            // Expire the original launch deadline: retained operations must use fresh bounds.
            proof.controller.deadline = Instant::now() - Duration::from_secs(1);
            proof.probe()?;
            if case == "poison" {
                proof.controller.send(wire::START)?;
                let error = proof
                    .controller
                    .receive()
                    .expect_err("illegal transition accepted");
                fs::write(output.join("rejection.txt"), error.to_string())?;
                assert!(
                    proof
                        .controller
                        .contained
                        .child()
                        .is_live()
                        .map_err(other)?
                );
                proof.controller.cancel()?;
            } else {
                std::thread::sleep(Duration::from_millis(50));
                proof.probe()?;
                proof.release()?;
            }
        }
        wire::wait(
            pidfd.as_fd(),
            rustix::event::PollFlags::IN,
            Instant::now() + Duration::from_secs(5),
        )?;
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "controller was not reaped"
        );
        Ok(())
    })();
    let sentinel_live = sentinel.try_wait().unwrap().is_none();
    sentinel.kill().unwrap();
    sentinel.wait().unwrap();
    assert!(sentinel_live, "unrelated sibling was killed");
    result.unwrap();
    assert_eq!(scopes(), before, "owned controller scope was not removed");
    fs::write(
        output.join("cleanup.txt"),
        b"controller reaped; owned scope absent; sibling survived\n",
    )
    .unwrap();
}

fn wait_for_verifier(controller: &RootManagedProofControllerV1) -> io::Result<Vec<u8>> {
    loop {
        controller.revalidate()?;
        require(
            Instant::now() < controller.deadline,
            "no live Verus process observed",
        )?;
        let mut pending = vec![controller.child_pid()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(pid) = pending.pop() {
            if !seen.insert(pid) {
                continue;
            }
            require(seen.len() <= 64, "unexpected proof process tree size")?;
            if pid != controller.child_pid()
                && let Ok(file) = fs::File::open(format!("/proc/{pid}/cmdline"))
            {
                let mut command = Vec::new();
                file.take(16 * 1024).read_to_end(&mut command)?;
                if command
                    .split(|b| *b == 0)
                    .any(|arg| arg == b"--no-cheating")
                {
                    return Ok(command);
                }
            }
            if let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) {
                for task in tasks.flatten() {
                    if let Ok(children) = fs::read_to_string(task.path().join("children")) {
                        for child in children.split_whitespace() {
                            pending.push(child.parse::<u32>().map_err(other)?);
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
