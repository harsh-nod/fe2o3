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
    prepare_deployment(CONTROLLER_PATH);
}

#[test]
#[ignore = "requires private pinned runtime and installed application controller"]
fn prepare_application_deployment() {
    prepare_deployment(crate::deployment::APPLICATION_CONTROLLER_PATH);
}

fn prepare_deployment(controller_path: &str) {
    assert!(!rustix::process::getuid().is_root());
    let controller = fs::read(controller_path).unwrap();
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
            | "poll-pair"
            | "poll-cancel-proof"
            | "poll-cancel-gated"
            | "poll-drop-gated"
            | "poll-deadline"
            | "poll-proof-deadline"
            | "poll-probe-timeout"
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
    let result = (|| -> io::Result<()> {
        if case.starts_with("poll-") {
            return qualify_polling(
                installed,
                &case,
                &envelope,
                &payload,
                KernelId::from_bytes(kernel),
                &output,
            );
        }
        let launched = installed.launch(&envelope, &payload, KernelId::from_bytes(kernel));
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

fn drive(mut poll: impl FnMut() -> io::Result<bool>) -> io::Result<usize> {
    let deadline = Instant::now() + EXECUTION_TIMEOUT;
    let mut count = 0;
    loop {
        count += 1;
        if poll()? {
            return Ok(count);
        }
        check_deadline(deadline)?;
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_reaped(pid: u32, pidfd: &OwnedFd) -> io::Result<()> {
    wire::wait(
        pidfd.as_fd(),
        rustix::event::PollFlags::IN,
        Instant::now() + Duration::from_secs(5),
    )?;
    require(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "controller was not reaped",
    )
}

fn qualify_polling(
    installed: ProductionProofCustodianDeploymentV1,
    case: &str,
    envelope: &[u8],
    payload: &[u8],
    kernel: KernelId,
    output: &std::path::Path,
) -> io::Result<()> {
    let mut first = installed.begin_launch(envelope, payload, kernel)?;
    let original = first.controller.as_ref().unwrap();
    let pid = original.child_pid();
    let pidfd = original
        .contained
        .child()
        .try_clone_pidfd()
        .map_err(other)?;
    fs::write(output.join("controller-pid.txt"), pid.to_string())?;
    assert!(first.take_ready()?.is_none());
    assert_eq!(scopes().len(), 1);
    let mut counts = String::new();
    if matches!(
        case,
        "poll-cancel-gated" | "poll-drop-gated" | "poll-deadline"
    ) {
        if case == "poll-deadline" {
            first.controller.as_mut().unwrap().deadline = Instant::now() - Duration::from_secs(1);
            assert_eq!(first.poll().unwrap_err().kind(), io::ErrorKind::TimedOut);
            assert!(first.poll().is_err());
            assert!(first.take_ready().is_err());
            assert!(
                first
                    .controller
                    .as_ref()
                    .unwrap()
                    .contained
                    .child()
                    .is_live()
                    .map_err(other)?
            );
            assert_eq!(scopes().len(), 1);
        }
        if case == "poll-drop-gated" {
            drop(first);
        } else {
            let n = drive(|| first.poll_cancel())?;
            counts.push_str(&format!("gated cancellation polls: {n}\n"));
            assert!(first.poll_cancel()?);
            assert!(first.poll().is_err());
            drop(first);
        }
        assert_reaped(pid, &pidfd)?;
        fs::write(output.join("polls.txt"), counts)?;
        return Ok(());
    }
    let mut second =
        ProductionProofCustodianDeploymentV1::open()?.begin_launch(envelope, payload, kernel)?;
    let second_original = second.controller.as_ref().unwrap();
    let second_pid = second_original.child_pid();
    let second_pidfd = second_original
        .contained
        .child()
        .try_clone_pidfd()
        .map_err(other)?;
    assert_ne!(pid, second_pid);
    assert_eq!(scopes().len(), 2);
    fs::write(
        output.join("second-controller-pid.txt"),
        second_pid.to_string(),
    )?;
    let n = drive(|| {
        let a = first.poll()?;
        let b = second.poll()?;
        assert_eq!(first.controller.as_ref().unwrap().child_pid(), pid);
        assert_eq!(second.controller.as_ref().unwrap().child_pid(), second_pid);
        Ok(a && b)
    })?;
    counts.push_str(&format!("alternating startup polls: {n}\n"));
    let first_owner = first.take_ready()?.unwrap();
    let second_owner = second.take_ready()?.unwrap();
    assert!(first.poll_cancel().is_err());
    assert!(second.poll_cancel().is_err());
    first_owner.revalidate()?;
    second_owner.revalidate()?;
    assert!(first.take_ready().is_err());
    assert!(second.poll().is_err());
    let mut first_proof = first_owner.begin_proof();
    let mut second_proof = second_owner.begin_proof();
    assert!(first_proof.take_ready()?.is_none());
    assert!(second_proof.take_ready()?.is_none());
    if case == "poll-proof-deadline" {
        second_proof.controller.as_mut().unwrap().deadline =
            Instant::now() - Duration::from_secs(1);
        assert_eq!(
            second_proof.poll().unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(second_proof.poll().is_err());
        assert!(second_proof.take_ready().is_err());
    } else {
        assert!(!second_proof.poll()?);
    }
    assert!(!first_proof.poll()?);
    let cancel_second = matches!(case, "poll-cancel-proof" | "poll-proof-deadline");
    if case == "poll-cancel-proof" {
        let command = wait_for_verifier(second_proof.controller.as_ref().unwrap())?;
        fs::write(output.join("verifier-command.bin"), command)?;
    }
    let n = drive(|| {
        let a = first_proof.poll()?;
        let b = if cancel_second {
            second_proof.poll_cancel()?
        } else {
            second_proof.poll()?
        };
        Ok(a && b)
    })?;
    counts.push_str(&format!("alternating proof/cancellation polls: {n}\n"));
    // Only completed proof custody survives the old execution deadline.
    first_proof.controller.as_mut().unwrap().deadline = Instant::now() - Duration::from_secs(1);
    assert!(first_proof.poll()?);
    let mut a = first_proof.take_ready()?.unwrap();
    assert!(first_proof.poll_cancel().is_err());
    a.controller.revalidate()?;
    assert_eq!(a.child_pid(), pid);
    assert!(first_proof.take_ready().is_err());
    fs::write(output.join("subject.bin"), a.subject_bytes())?;
    a.probe()?;
    if cancel_second {
        assert!(second_proof.poll_cancel()?);
        assert!(second_proof.take_ready().is_err());
        assert_reaped(second_pid, &second_pidfd)?;
        assert_eq!(scopes().len(), 1);
        a.probe()?;
        a.release()?;
    } else {
        let mut b = second_proof.take_ready()?.unwrap();
        fs::write(output.join("second-subject.bin"), b.subject_bytes())?;
        // Independent executions have fresh analyzer/proof roots. Each probe must
        // match its own original subject, not another run's serialized identity.
        b.probe()?;
        if case == "poll-probe-timeout" {
            // Send a genuine Probe, then force the wait to expire. Its reply must
            // never be reused as a response to another transaction.
            a.controller.send(wire::PROBE)?;
            a.controller.deadline = Instant::now() - Duration::from_secs(1);
            assert_eq!(
                a.controller.receive().unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
            assert!(a.probe().is_err());
            let n = drive(|| a.controller.poll_cancel())?;
            counts.push_str(&format!("poisoned cancellation polls: {n}\n"));
        } else {
            a.release()?;
        }
        assert_reaped(pid, &pidfd)?;
        assert_eq!(scopes().len(), 1);
        b.probe()?;
        b.release()?;
        assert_reaped(second_pid, &second_pidfd)?;
    }
    assert_reaped(pid, &pidfd)?;
    fs::write(output.join("polls.txt"), counts)?;
    Ok(())
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
