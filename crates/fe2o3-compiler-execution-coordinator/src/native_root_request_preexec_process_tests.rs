//! Genuine transport peers and an outside cgroup custodian. No approval producer.
use super::*;
use std::{
    io::{Read, Seek},
    os::{
        fd::{AsRawFd, BorrowedFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Stdio},
};

const CONTROL_FD: i32 = 240;
const CLIENT_UID: &str = "FE2O3_NATIVE_ROOT_CLIENT_UID";
const CLIENT_GID: &str = "FE2O3_NATIVE_ROOT_CLIENT_GID";
const CGROUP_PARENT: &str = "FE2O3_NATIVE_ROOT_CGROUP_PARENT";
const CGROUP2_MAGIC: fs::FsWord = 0x6367_7270;
const LIMIT: Duration = Duration::from_secs(240);

pub(super) fn identity_from_environment() -> (u32, u32) {
    let uid = std::env::var(CLIENT_UID)
        .expect("provisioned client UID required")
        .parse()
        .unwrap();
    let gid = std::env::var(CLIENT_GID)
        .expect("provisioned client GID required")
        .parse()
        .unwrap();
    assert!(
        uid != 0 && gid != 0,
        "peer must be a real distinct non-root process"
    );
    (uid, gid)
}

fn until<T>(mut operation: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(value) = operation() {
            return value;
        }
        assert!(Instant::now() < deadline, "bounded test transport deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(super) struct Sender {
    child: Child,
    control: OwnedFd,
    expected: MessageSender,
}

impl Sender {
    #[allow(unsafe_code)]
    pub(super) fn connect((uid, gid): (u32, u32)) -> (Self, OwnedFd) {
        assert!(uid != 0 && gid != 0);
        let (control, child_control) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        for fd in [&control, &child_control] {
            net::sockopt::set_socket_passcred(fd, true).unwrap();
        }
        // Separate high source avoids dup2(old == new) retaining CLOEXEC.
        let inherited = rustix::io::fcntl_dupfd_cloexec(&child_control, CONTROL_FD + 1).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                &test_name("preexec::original_client_process"),
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CLIENT_UID, uid.to_string())
            .env(CLIENT_GID, gid.to_string())
            .uid(uid)
            .gid(gid);
        // SAFETY: this isolated test owns the source. The post-fork callback only
        // performs dup2; the dedicated re-exec role adopts destination 240 once.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(inherited.as_raw_fd(), CONTROL_FD) != CONTROL_FD {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command
            .spawn()
            .expect("start actual non-root intake sender");
        let expected = MessageSender::new(child.id() as i32, uid, gid);
        let sender = Self {
            child,
            control,
            expected,
        };
        let (_, client) = until(|| {
            launch_io::receive_authenticated_descriptor::<1>(sender.control.as_fd(), expected)
                .unwrap()
        });
        (sender, client)
    }

    pub(super) fn send(&self, bytes: &[u8; N], file: Option<BorrowedFd<'_>>) {
        let kind = [u8::from(file.is_some())];
        until(|| launch_io::send_packet(self.control.as_fd(), &kind).unwrap());
        if let Some(file) = file {
            until(|| {
                launch_io::send_packet_with_descriptor(self.control.as_fd(), bytes, file).unwrap()
            });
        } else {
            until(|| launch_io::send_packet(self.control.as_fd(), bytes).unwrap());
        }
        // The actual child sent the packet before the root advances its Receiver.
        let ack = until(|| {
            launch_io::receive_authenticated_packet::<1>(self.control.as_fd(), self.expected)
                .unwrap()
        });
        assert_eq!(ack, [1]);
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "private distinct-UID transport role; original root matrix only"]
#[allow(unsafe_code)]
fn original_client_process() {
    let (uid, gid) = identity_from_environment();
    assert_eq!(
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw()
        ),
        (uid, gid)
    );
    // SAFETY: dedicated re-exec role; the parent installed the unique inherited FD.
    let control = unsafe { OwnedFd::from_raw_fd(CONTROL_FD) };
    let peer = net::sockopt::socket_peercred(&control).unwrap();
    assert_eq!((peer.uid.as_raw(), peer.gid.as_raw()), (0, 0));
    let expected = MessageSender::new(peer.pid.as_raw_pid(), 0, 0);
    let client = net::socket_with(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&client, true).unwrap();
    net::connect(&client, &net::SocketAddrUnix::new(SOCKET).unwrap()).unwrap();
    until(|| {
        launch_io::send_packet_with_descriptor(control.as_fd(), &[0], client.as_fd()).unwrap()
    });
    loop {
        let kind = until(|| {
            launch_io::receive_authenticated_packet::<1>(control.as_fd(), expected).unwrap()
        });
        match kind {
            [0] => {
                let bytes = until(|| {
                    launch_io::receive_authenticated_packet::<N>(control.as_fd(), expected).unwrap()
                });
                until(|| launch_io::send_packet(client.as_fd(), &bytes).unwrap());
            }
            [1] => {
                let (bytes, file) = until(|| {
                    launch_io::receive_authenticated_descriptor::<N>(control.as_fd(), expected)
                        .unwrap()
                });
                until(|| {
                    launch_io::send_packet_with_descriptor(client.as_fd(), &bytes, file.as_fd())
                        .unwrap()
                });
            }
            _ => panic!("unexpected test transport command"),
        }
        until(|| launch_io::send_packet(control.as_fd(), &[1]).unwrap());
    }
}

// This controller stays OUTSIDE the case domain. It is actual filesystem/domain
// custody, not an environment flag credited as successful external cleanup.
struct OutsideDomain {
    path: PathBuf,
    child: Option<Child>,
}

impl OutsideDomain {
    fn empty(&self) -> bool {
        disk::read_to_string(self.path.join("cgroup.events"))
            .unwrap()
            .lines()
            .any(|line| line == "populated 0")
    }

    fn kill_and_remove(&mut self) {
        disk::write(self.path.join("cgroup.kill"), b"1").unwrap();
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            until(|| child.try_wait().unwrap());
        }
        self.child = None;
        let deadline = Instant::now() + TIMEOUT;
        while !self.empty() {
            assert!(
                Instant::now() < deadline,
                "outside domain remains populated"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        remove_empty_domains(&self.path).unwrap();
        assert!(
            !self.path.exists(),
            "outside custodian must retire its actual domain"
        );
    }
}

fn remove_empty_domains(path: &Path) -> std::io::Result<()> {
    for entry in disk::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_empty_domains(&entry.path())?;
        }
    }
    disk::remove_dir(path)
}

impl Drop for OutsideDomain {
    fn drop(&mut self) {
        // The outer isolated harness must fail closed even when an assertion fails.
        // Avoid a second panic during unwinding; no successful cleanup is credited.
        if self.path.exists() {
            let _ = disk::write(self.path.join("cgroup.kill"), b"1");
            if let Some(child) = &mut self.child {
                let _ = child.kill();
            }
            let deadline = Instant::now() + TIMEOUT;
            while Instant::now() < deadline {
                let reaped = self
                    .child
                    .as_mut()
                    .is_none_or(|child| child.try_wait().is_ok_and(|status| status.is_some()));
                let empty = disk::read_to_string(self.path.join("cgroup.events"))
                    .is_ok_and(|events| events.lines().any(|line| line == "populated 0"));
                if reaped && empty {
                    let _ = remove_empty_domains(&self.path);
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

#[allow(unsafe_code)]
pub(super) fn run_case(case: &str) -> std::process::Output {
    identity_from_environment();
    let parent = PathBuf::from(
        std::env::var(CGROUP_PARENT).expect("isolated root-controlled cgroup parent required"),
    );
    let canonical = disk::canonicalize(&parent).unwrap();
    assert_eq!(canonical, parent);
    assert!(parent.starts_with("/sys/fs/cgroup") && parent != Path::new("/sys/fs/cgroup"));
    let directory = disk::File::open(&parent).unwrap();
    assert_eq!(fs::fstatfs(&directory).unwrap().f_type, CGROUP2_MAGIC);
    assert_eq!(
        disk::read_to_string(parent.join("cgroup.type")).unwrap(),
        "domain\n"
    );
    let metadata = disk::metadata(&parent).unwrap();
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.mode() & 0o022),
        (0, 0, 0)
    );
    let path = parent.join(format!("fe2o3-preexec-{}-{case}", std::process::id()));
    disk::create_dir(&path).expect("fresh external custody domain, never adopt an existing one");
    let mut outside = OutsideDomain { path, child: None };
    let procs = disk::OpenOptions::new()
        .write(true)
        .open(outside.path.join("cgroup.procs"))
        .unwrap();
    let mut stderr = tempfile::tempfile().unwrap();
    let mut stdout = tempfile::tempfile().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &test_name("complete_root_request_case"),
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CASE_ENV, case)
        .stdout(Stdio::from(stdout.try_clone().unwrap()))
        .stderr(Stdio::from(stderr.try_clone().unwrap()));
    // SAFETY: move only the new child into the fresh external domain before exec.
    // The parent/controller remains outside. Only an async-signal-safe write runs.
    unsafe {
        command.pre_exec(move || {
            if libc::write(procs.as_raw_fd(), b"0".as_ptr().cast(), 1) != 1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    outside.child = Some(command.spawn().unwrap());
    let deadline = Instant::now() + LIMIT;
    let status = loop {
        if let Some(status) = outside.child.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "case exceeded bounded external lifetime"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    if status.success() {
        assert!(
            outside.empty(),
            "normal return requires native aggregate cleanup, not external killing"
        );
    }
    outside.kill_and_remove();
    assert!(stderr.metadata().unwrap().len() <= 256 * 1024);
    assert!(stdout.metadata().unwrap().len() <= 256 * 1024);
    stderr.rewind().unwrap();
    stdout.rewind().unwrap();
    let mut err = Vec::new();
    let mut out = Vec::new();
    stderr.read_to_end(&mut err).unwrap();
    stdout.read_to_end(&mut out).unwrap();
    eprintln!("ROOT_REQUEST_OUTSIDE_DOMAIN_RETIRED case={case}");
    std::process::Output {
        status,
        stdout: out,
        stderr: err,
    }
}
