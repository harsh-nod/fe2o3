use super::*;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1 as LOCK,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1 as SOCKET,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use rustix::process::{Gid, Uid};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
};

pub(super) const ISSUER: u32 = 61001;
pub(super) const ANCHOR: u32 = 61002;
pub(super) const CLIENT: u32 = 61003;
const MAX_IMAGE: u64 = 32 * 1024 * 1024;

pub(super) fn path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("missing {name}")));
    assert!(
        path.is_absolute(),
        "{name} must be an explicit absolute path"
    );
    path
}

pub(super) fn require_environment() {
    assert_eq!(
        std::env::var("FE2O3_RUN_NATIVE_ROOT_ISSUER").as_deref(),
        Ok("1")
    );
    fe2o3_protected_service_spawn::require_exact_root_identity_v1().unwrap();
    assert_eq!(path("FE2O3_NATIVE_ROOT_SCRATCH"), Path::new("/tmp"));
    assert_eq!(
        path("FE2O3_NATIVE_ROOT_RUNTIME"),
        Path::new(SOCKET).parent().unwrap()
    );
    let runtime = disk::symlink_metadata(path("FE2O3_NATIVE_ROOT_RUNTIME"))
        .expect("private /run/fe2o3 mount required");
    assert!(runtime.is_dir());
    assert_eq!(
        (runtime.uid(), runtime.gid(), runtime.mode() & 0o7777),
        (0, 0, 0o755)
    );
    for name in [
        "FE2O3_NATIVE_ROOT_ISSUER_V3",
        "FE2O3_NATIVE_ROOT_ANCHOR_HELPER_V3",
        "FE2O3_NATIVE_ROOT_ANCHOR_DAEMON_V3",
        "FE2O3_NATIVE_COMPILER_EXEC_FIXTURE",
    ] {
        let file = File::open(path(name)).unwrap();
        let m = file.metadata().unwrap();
        assert!(
            m.is_file() && m.len() > 0 && m.len() <= MAX_IMAGE,
            "bounded static fixture {name}"
        );
        assert_eq!(m.mode() & 0o222, 0, "fixture must be read-only: {name}");
        assert_eq!(
            m.mode() & 0o7111,
            0o111,
            "fixture must execute for each service UID: {name}"
        );
    }
}

pub(super) fn mode(path: &Path, mode: u32) {
    disk::set_permissions(path, disk::Permissions::from_mode(mode)).unwrap();
}

pub(super) fn owner(path: &Path, uid: u32, gid: u32) {
    fs::chown(path, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))).unwrap();
}

pub(super) fn measurement(path: &Path) -> Measurement {
    let mut file = File::open(path).unwrap();
    let length = file.metadata().unwrap().len();
    assert!(length > 0 && length <= MAX_IMAGE);
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 8192];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut bytes).unwrap();
        if n == 0 {
            break;
        }
        read += n as u64;
        assert!(read <= length, "fixture changed while measuring");
        hash.update(&bytes[..n]);
    }
    assert_eq!(read, length);
    Measurement::new(hash.finalize().into(), length).unwrap()
}

pub(super) struct Fixture {
    pub dir: tempfile::TempDir,
    pub issuer: PathBuf,
    pub helper: PathBuf,
    pub daemon: PathBuf,
    pub compiler: PathBuf,
    service: PathBuf,
    pub service_root: File,
    pub anchor_root: File,
    lock: PathBuf,
    socket_owned: bool,
}

impl Fixture {
    pub fn new(corrupt: bool) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("root-issuer-")
            .tempdir_in(path("FE2O3_NATIVE_ROOT_SCRATCH"))
            .unwrap();
        mode(dir.path(), 0o755);
        let service = dir.path().join("issuer");
        let anchor = dir.path().join("anchor");
        for root in [&service, &anchor] {
            disk::create_dir(root).unwrap();
            mode(root, 0o700);
        }
        // Open control aliases while root owns the directories. CAP_CHOWN can
        // restore ownership at teardown without CAP_DAC_OVERRIDE or CAP_FOWNER.
        let service_root = File::open(&service).unwrap();
        let anchor_root = File::open(&anchor).unwrap();
        if corrupt {
            use std::io::Write;
            let state = service.join("compiler-execution-issuer-v3.state");
            File::create_new(&state)
                .unwrap()
                .write_all(b"invalid signed state")
                .unwrap();
            mode(&state, 0o600);
            owner(&state, ISSUER, ISSUER);
        }
        owner(&service, ISSUER, ISSUER);
        owner(&anchor, ANCHOR, ANCHOR);
        let lock = dir.path().join(Path::new(LOCK).file_name().unwrap());
        File::create_new(&lock).unwrap();
        mode(&lock, 0o400);
        Self {
            issuer: path("FE2O3_NATIVE_ROOT_ISSUER_V3"),
            helper: path("FE2O3_NATIVE_ROOT_ANCHOR_HELPER_V3"),
            daemon: path("FE2O3_NATIVE_ROOT_ANCHOR_DAEMON_V3"),
            compiler: path("FE2O3_NATIVE_COMPILER_EXEC_FIXTURE"),
            dir,
            service,
            service_root,
            anchor_root,
            lock,
            socket_owned: false,
        }
    }

    pub fn bind(&mut self) -> OwnedFd {
        let socket = net::socket_with(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        // Bind fails on any preexisting entry; never unlink someone else's socket.
        net::bind(&socket, &net::SocketAddrUnix::new(SOCKET).unwrap()).unwrap();
        self.socket_owned = true;
        owner(Path::new(SOCKET), 0, ISSUER);
        mode(Path::new(SOCKET), 0o660);
        socket
    }

    pub fn state(&self) -> PathBuf {
        self.service.join("compiler-execution-issuer-v3.state")
    }

    pub fn assert_state(&self, expected: &str) {
        let helper = test_name("native_root_issuer_state_probe");
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &helper,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_NATIVE_ROOT_STATE", self.state())
            .env("FE2O3_NATIVE_ROOT_STATE_EXPECTED", expected)
            .uid(ISSUER)
            .gid(ISSUER)
            .output()
            .unwrap();
        require_output(
            output,
            &format!("NATIVE_ROOT_ISSUER_STATE_OK expected={expected}"),
        );
    }

    pub fn assert_unlocked(&self) {
        let lock = File::open(&self.lock).unwrap();
        fs::flock(&lock, fs::FlockOperation::NonBlockingLockExclusive)
            .expect("all lifecycle aliases must retire after cleanup shutdown");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for root in [&self.service_root, &self.anchor_root] {
            fs::fchown(root, Some(Uid::from_raw(0)), Some(Gid::from_raw(0))).unwrap();
        }
        if self.socket_owned {
            disk::remove_file(SOCKET).unwrap();
        }
    }
}

pub(super) fn probe_state() {
    assert_eq!(
        std::env::var("FE2O3_RUN_NATIVE_ROOT_ISSUER").as_deref(),
        Ok("1")
    );
    assert_eq!(
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw()
        ),
        (ISSUER, ISSUER)
    );
    let state = path("FE2O3_NATIVE_ROOT_STATE");
    assert!(state.starts_with("/tmp"));
    let expected = std::env::var("FE2O3_NATIVE_ROOT_STATE_EXPECTED").unwrap();
    match expected.as_str() {
        "ready" => {
            let m = disk::symlink_metadata(state).expect("durable genesis must precede Ready120");
            assert!(m.is_file() && m.len() > 0);
            assert_eq!(
                (m.uid(), m.gid(), m.mode() & 0o7777),
                (ISSUER, ISSUER, 0o600)
            );
        }
        "corrupt" => assert_eq!(disk::read(state).unwrap(), b"invalid signed state"),
        "absent" => assert_eq!(
            disk::symlink_metadata(state).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        ),
        other => panic!("unknown state probe {other}"),
    }
    eprintln!("NATIVE_ROOT_ISSUER_STATE_OK expected={expected}");
}
