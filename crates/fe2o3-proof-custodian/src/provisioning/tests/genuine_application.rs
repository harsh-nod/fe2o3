//! Exercises shipped entrypoints; no receipt, signing seed, or application handoff is fabricated.

use super::*;
use std::io::{IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

mod capture;
mod hardware;

const COORDINATOR: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator";
const NOTIFY: &str = "/run/coordinator-ready.sock";
const HELPER: &str = "provisioning::tests::genuine_application::exec_installed_coordinator";
const DESCRIPTOR_NAMES: &str = "runtime-root:supervisor-root:anchor-root:supervisor:launcher:issuer:anchor-helper:anchor-daemon:supervisor-deployment:issuer-policy:anchor-deployment:anchor-provisioning:issuer-key-seed:anchor-key-seed";
const AUTHORITY_ENV: [&str; 13] = [
    "CARGO",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_BACKEND",
    "FE2O3_TARGET",
    "FE2O3_PRODUCTION_BUILD_CONFIG_V1",
    "LANG",
    "LC_ALL",
];

struct Service(Child);

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        // The enclosing private cgroup owns and drains every descendant, including quarantine.
    }
}

fn private_root() {
    assert!(rustix::process::getuid().is_root());
    assert_eq!(std::env::var("FE2O3_PROOF_INSTALL_PRIVATE").unwrap(), "1");
    assert_ne!(
        std::fs::read_link("/proc/self/ns/pid").unwrap().as_os_str(),
        std::env::var_os("FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE").unwrap()
    );
    for path in ["/etc", "/usr/libexec", "/run", "/var/lib"] {
        assert_eq!(rustix::fs::statfs(path).unwrap().f_type, libc::TMPFS_MAGIC);
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    // The shipped coordinator/manager profile, not an all-capabilities root surrogate.
    let expected_caps = (1 << 0) | (1 << 1) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 19);
    for field in ["CapEff:\t", "CapPrm:\t", "CapBnd:\t"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(field))
            .unwrap();
        assert_eq!(
            u64::from_str_radix(value, 16).unwrap(),
            expected_caps,
            "{field}"
        );
    }
    for field in ["CapInh:\t", "CapAmb:\t"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(field))
            .unwrap();
        assert_eq!(u64::from_str_radix(value, 16).unwrap(), 0, "{field}");
    }
    // bwrap adds no-new-privileges; this is namespace qualification, not a systemd boot test.
    for record in ["Seccomp:\t0", "NoNewPrivs:\t1", "Umask:\t0077"] {
        assert!(
            status.lines().any(|line| line == record),
            "missing {record}"
        );
    }
}

#[test]
#[ignore = "private exec-only helper for the genuine application campaign"]
fn exec_installed_coordinator() {
    private_root();
    // exec discards the test harness threads before the production single-thread admission.
    let error = Command::new(COORDINATOR)
        .env_clear()
        .env("LISTEN_PID", std::process::id().to_string())
        .env("LISTEN_FDS", "14")
        .env("LISTEN_FDNAMES", DESCRIPTOR_NAMES)
        .env("NOTIFY_SOCKET", NOTIFY)
        .exec();
    panic!("exec installed coordinator: {error}");
}

#[allow(unsafe_code)]
fn spawn_coordinator() -> Service {
    // Reserve low descriptors before Command creates its private exec-error pipe.
    let _reserved: Vec<_> = (0..32).map(|_| File::open("/dev/null").unwrap()).collect();
    let paths = [
        "/run/fe2o3",
        "/var/lib/fe2o3/compiler-execution",
        "/var/lib/fe2o3/external-anchor",
        "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor",
        "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher",
        "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer",
        "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper",
        "/usr/libexec/fe2o3/fe2o3-external-anchor-service",
        "/etc/fe2o3/compiler-execution/supervisor-deployment-v1",
        "/etc/fe2o3/compiler-execution/issuer-policy-v1",
        "/etc/fe2o3/compiler-execution/anchor-deployment-v1",
        "/etc/fe2o3/compiler-execution/anchor-provisioning-v1",
        "/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v1",
        "/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v1",
    ];
    let descriptors: Vec<OwnedFd> = paths
        .iter()
        .map(|path| {
            let file = File::open(path).unwrap_or_else(|error| panic!("{path}: {error}"));
            // Keep sources clear of every destination, including Command's exec-error pipe.
            rustix::io::fcntl_dupfd_cloexec(&file, 64).unwrap()
        })
        .collect();
    let raw: Vec<_> = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("FE2O3_PROOF_INSTALL_PRIVATE", "1")
        .env(
            "FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE",
            std::env::var_os("FE2O3_PROOF_INSTALL_HOST_PID_NAMESPACE").unwrap(),
        )
        .args([
            "--exact",
            HELPER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]);
    // Only async-signal-safe descriptor operations occur between fork and exec.
    unsafe {
        command.pre_exec(move || {
            for (offset, source) in raw.iter().enumerate() {
                if libc::dup2(*source, 3 + offset as i32) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Service(command.spawn().unwrap())
}

fn wait_ready(socket: &UnixDatagram, service: &mut Service) {
    use rustix::net::{self, RecvFlags, ReturnFlags};
    socket.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        assert!(
            service.0.try_wait().unwrap().is_none(),
            "coordinator exited before READY"
        );
        let mut bytes = [0_u8; 64];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1))];
        let mut ancillary = net::RecvAncillaryBuffer::new(&mut storage);
        match net::recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT,
        ) {
            Ok(message) => {
                assert!(
                    !message
                        .flags
                        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                );
                assert_eq!(&bytes[..message.bytes], b"READY=1");
                let credentials: Vec<_> = ancillary.drain().collect();
                match credentials.as_slice() {
                    [net::RecvAncillaryMessage::ScmCredentials(value)] => {
                        assert_eq!(value.pid.as_raw_pid(), service.0.id() as i32);
                        assert!(value.uid.is_root());
                        assert!(value.gid.is_root());
                    }
                    _ => panic!("readiness lacks exactly one kernel credential record"),
                }
                return;
            }
            Err(rustix::io::Errno::AGAIN) => (),
            Err(error) => panic!("coordinator readiness: {error}"),
        }
        assert!(Instant::now() < deadline, "coordinator readiness timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[allow(unsafe_code)]
fn inherit_standard_descriptors_only(command: &mut Command) {
    // Do not import namespace-launcher descriptors into the authority-release boundary.
    unsafe {
        command.pre_exec(|| {
            if libc::syscall(
                libc::SYS_close_range,
                3_u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[test]
fn authority_child_does_not_inherit_launcher_descriptors() {
    let file = File::open("/dev/null").unwrap();
    let descriptor = rustix::io::fcntl_dupfd_cloexec(&file, 512).unwrap();
    rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::empty()).unwrap();
    let path = format!("/proc/self/fd/{}", descriptor.as_raw_fd());
    let mut control = Command::new("/bin/sh");
    control
        .env_clear()
        .args(["-c", "test -e \"$1\"", "control", &path]);
    assert!(control.status().unwrap().success());
    let mut isolated = Command::new("/bin/sh");
    isolated
        .env_clear()
        .args(["-c", "test ! -e \"$1\"", "isolated", &path]);
    inherit_standard_descriptors_only(&mut isolated);
    assert!(isolated.status().unwrap().success());
    assert!(Path::new(&path).exists());
    assert_eq!(
        rustix::io::fcntl_getfd(&descriptor).unwrap(),
        rustix::io::FdFlags::empty()
    );
}

#[test]
#[ignore = "requires the genuine mode of scripts/qualify-proof-resource-inspection.sh"]
fn root_genuine_application_campaign() {
    run_genuine_application(None);
}

#[test]
#[ignore = "read-only hardware preflight for the genuine-two-gpu campaign"]
fn observe_two_gpu_selection() {
    let selection = hardware::discover(hardware::requested_ids().unwrap()).unwrap();
    selection.required_groups().unwrap();
    println!(
        "\n{}{}",
        hardware::SELECTION_TAG,
        serde_json::to_string(&selection).unwrap()
    );
}

#[test]
#[ignore = "requires the genuine-two-gpu mode of scripts/qualify-proof-resource-inspection.sh"]
fn root_genuine_two_gpu_application_campaign() {
    private_root();
    run_genuine_application(Some(hardware::requested_ids().unwrap()));
}

fn run_genuine_application(devices: Option<[u64; 2]>) {
    private_root();
    if let Some(ids) = devices {
        hardware::revalidate(ids).expect("final namespace GPU selection");
    }
    let output = Command::new("/usr/libexec/fe2o3/fe2o3-compiler-execution-provision")
        .env_clear()
        .arg("1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "compiler provisioning: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("actual compiler provisioner created fresh keys and bound client profile");

    let candidate_path = "/run/candidates/genuine";
    let provisioner = "/usr/libexec/fe2o3/fe2o3-proof-custodian-provision";
    let output = Command::new("/usr/bin/setpriv")
        .env_clear()
        .args([
            "--reuid=61002",
            "--regid=61003",
            "--clear-groups",
            "--inh-caps=-all",
            "--ambient-caps=-all",
            "--bounding-set=-all",
            provisioner,
            "inspect-fixed-resources",
            candidate_path,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "proof inspection: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let candidate = Candidate::decode(&std::fs::read(candidate_path).unwrap()).unwrap();
    let output = Command::new(provisioner)
        .env_clear()
        .args(["install", candidate_path, &candidate.sha256_hex()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "proof installation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!(
        "qualification-approved real candidate_sha256={}",
        candidate.sha256_hex()
    );

    let mut manager = Service(
        Command::new("/usr/libexec/fe2o3/fe2o3-proof-manager")
            .env_clear()
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let socket_path = "/run/fe2o3-proof-custodian/control.sock";
    while !Path::new(socket_path).exists() {
        assert!(
            manager.0.try_wait().unwrap().is_none(),
            "manager exited before listen"
        );
        assert!(Instant::now() < deadline, "manager listen timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    let metadata = std::fs::symlink_metadata(socket_path).unwrap();
    assert!(metadata.file_type().is_socket());
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.mode() & 0o7777),
        (0, 0, 0o600)
    );
    let notify = UnixDatagram::bind(NOTIFY).unwrap();
    rustix::net::sockopt::set_socket_passcred(&notify, true).unwrap();
    let mut coordinator = spawn_coordinator();
    wait_ready(&notify, &mut coordinator);
    println!(
        "actual coordinator entrypoint READY authenticated; original 14-FD activation retained"
    );

    let cwd = PathBuf::from(std::env::var_os("FE2O3_GENUINE_APPLICATION").unwrap());
    let mut command = Command::new("/usr/bin/setpriv");
    inherit_standard_descriptors_only(&mut command);
    command.env_clear().current_dir(&cwd);
    for name in AUTHORITY_ENV {
        command.env(
            name,
            std::env::var_os(name).unwrap_or_else(|| panic!("missing {name}")),
        );
    }
    command.env("TZ", "UTC");
    command.args([
        "--reuid=1000",
        "--regid=1000",
        "--inh-caps=-all",
        "--ambient-caps=-all",
        "--bounding-set=-all",
    ]);
    let groups = devices
        .map(hardware::revalidate)
        .transpose()
        .unwrap()
        .unwrap_or_default();
    if groups.is_empty() {
        command.arg("--clear-groups");
    } else {
        command.arg(format!(
            "--groups={}",
            groups
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    command.arg(std::env::var_os("FE2O3_GENUINE_CARGO_FE2O3").unwrap());
    command.args([
        "authority",
        "release",
        "run",
        "--application-proof-custodian",
        "--manifest-path",
    ]);
    command.arg(cwd.join("Cargo.toml"));
    command.args([
        "--target-dir",
        "/run/application-target",
        "--offline",
        "--frozen",
        "--bin",
        "fe2o3-conditional-custodian-application",
    ]);
    if let Some(ids) = devices {
        command.arg("--").args(ids.map(|id| format!("{id:#018x}")));
    }
    let output = capture::run(&mut command, Instant::now() + Duration::from_secs(1500))
        .expect("bounded genuine application capture");
    io::stdout().write_all(&output.stdout).unwrap();
    assert!(
        output.status.success(),
        "genuine compiler/application execution failed"
    );
    if let Some(ids) = devices {
        hardware::verify_report(&output.stdout, ids).expect("two-GPU execution report");
    }
    assert!(
        manager.0.try_wait().unwrap().is_none(),
        "proof manager lost custody"
    );
    assert!(
        coordinator.0.try_wait().unwrap().is_none(),
        "compiler coordinator lost continuity"
    );
    println!(
        "genuine selected-rustc -> issuer/anchor -> FD195 -> retained proof application passed"
    );
}
