//! Diagnostic concrete helper execution, not approved sibling bootstrap or receipts.
use super::*;
use rustix::{
    net::{self, AddressFamily, SocketFlags, SocketType},
    process::Pid,
};
use std::{
    os::fd::{AsFd, OwnedFd},
    process::{Child, Command, Stdio},
    thread,
};

const ROOT: &str = "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const DEADLINE_ENV: &str = "FE2O3_TEST_EXECUTOR_ABSOLUTE_DEADLINE_NS";
const CHILD: &str = "retained_functional_refinement_runtime_v1::executor_channel::protected_tests::protected_executor_helper_fixture";
const SESSION: [u8; 32] = [101; 32];

#[test]
fn command_handoff_releases_parent_endpoint_so_child_exit_reaches_eof() {
    let (client, helper) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&client, true).unwrap();
    let creator = net::sockopt::socket_peercred(&client).unwrap();
    let mut command = Command::new("/bin/true");
    command.env_clear().stdin(Stdio::from(helper));
    let mut child =
        Helper(crate::executor::spawn_artifact_coordinated_child(&mut command).unwrap());
    drop(command);
    let socket = Socket::new(client, creator).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    assert!(child.wait(deadline).success());
    assert_eq!(socket.receive_packet(&mut [0; 1], deadline).unwrap(), None);
}

struct Helper(Child);
impl Helper {
    fn wait(&mut self, deadline: Instant) -> std::process::ExitStatus {
        loop {
            if let Some(status) = self.0.try_wait().expect("observe helper") {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "helper did not terminate before deadline"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            // The protected runner owns the whole attempt cgroup and its outer
            // deadline. The proof tracer also arms EXITKILL before releasing work.
            let _ = self.0.wait();
        }
    }
}

#[test]
#[ignore = "private helper entry for the protected executor-channel diagnostic"]
fn protected_executor_helper_fixture() {
    let deadline = std::env::var(DEADLINE_ENV)
        .expect("private fixture deadline")
        .parse()
        .unwrap();
    let descriptor: OwnedFd = std::io::stdin().as_fd().try_clone_to_owned().unwrap();
    // The fixture's peer is its direct parent, which created this unnamed pair.
    // Production sibling bootstrap must instead commit the actual other child.
    let parent = net::sockopt::socket_peercred(&descriptor).unwrap();
    assert_eq!(Some(parent.pid), rustix::process::getppid());
    let socket = Socket::new(descriptor, parent).unwrap();
    let runtime =
        super::super::open_retained_generated_verus_runtime_v1(std::path::Path::new(ROOT))
            .expect("open a fresh process-local runtime lease after helper exec");
    let channel = ExecutorChannelV1::new(
        socket,
        SessionBindingV1 {
            session: SESSION,
            runtime: runtime.identity(),
        },
        deadline,
    )
    .unwrap();
    channel
        .serve(&runtime)
        .expect("serve the concrete retained backend");
    runtime
        .revalidate()
        .expect("revalidate helper-local runtime after close");
}

#[test]
#[ignore = "requires the installed protected runtime and a disposable bounded process tree"]
fn protected_executor_channel_runs_true_and_false_proofs_in_fresh_helper() {
    let runtime =
        super::super::open_retained_generated_verus_runtime_v1(std::path::Path::new(ROOT))
            .expect("open the caller-local lease");
    let (client, helper) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&client, true).unwrap();
    net::sockopt::set_socket_passcred(&helper, true).unwrap();
    let creator = net::sockopt::socket_peercred(&client).unwrap();
    assert_eq!(creator.pid, rustix::process::getpid());
    let deadline = Instant::now() + Duration::from_secs(240);
    let absolute = absolute_deadline(deadline).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            CHILD,
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env(DEADLINE_ENV, absolute.to_string())
        .env(
            "LD_LIBRARY_PATH",
            std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        )
        .current_dir("/")
        .stdin(Stdio::from(helper))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    let mut child =
        Helper(crate::executor::spawn_artifact_coordinated_child(&mut command).unwrap());
    drop(command);
    let sender = net::UCred {
        pid: Pid::from_raw(child.0.id().try_into().unwrap()).unwrap(),
        ..creator
    };
    assert_ne!(sender.pid, creator.pid);
    let socket = Socket::new(client, sender).unwrap();
    let mut channel = ExecutorChannelV1::new(
        socket,
        SessionBindingV1 {
            session: SESSION,
            runtime: runtime.identity(),
        },
        absolute,
    )
    .unwrap();
    for (assertion, code, stdout) in [
        (
            "value + 1 > value",
            0,
            b"verification results:: 1 verified, 0 errors\n".as_slice(),
        ),
        (
            "value + 1 > value + 1",
            1,
            b"verification results:: 0 verified, 1 errors\n".as_slice(),
        ),
    ] {
        let source = Source::new(format!("use vstd::prelude::*;\nverus! {{\n    pub proof fn sample(value: int) {{\n        assert({assertion});\n    }}\n}}\n").into_bytes()).unwrap();
        let reply = channel.exchange(&source, deadline, 64 * 1024).unwrap();
        assert_eq!((reply.exit_code, reply.signal), (Some(code), None));
        assert_eq!(reply.stdout, stdout);
        if code == 0 {
            assert!(reply.stderr.is_empty());
        } else {
            assert!(String::from_utf8_lossy(&reply.stderr).contains("assertion failed"));
        }
    }
    channel.finish().unwrap();
    assert!(child.wait(deadline).success());
    runtime
        .revalidate()
        .expect("caller lease never crossed the process boundary");
    eprintln!(
        "diagnostic helper transport only; production bootstrap=false; receipt=false; launch=false"
    );
}
