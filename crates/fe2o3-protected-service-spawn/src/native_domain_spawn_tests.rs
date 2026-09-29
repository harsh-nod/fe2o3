use super::*;
use crate::{PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    os::fd::{AsFd, OwnedFd},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const WORK: usize = 100_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;

struct Input(Arc<AtomicUsize>);
impl Drop for Input {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct Descendant(OwnedFd);
impl Drop for Descendant {
    fn drop(&mut self) {
        let _ = rustix::process::pidfd_send_signal(&self.0, rustix::process::Signal::KILL);
        for _ in 0..1000 {
            if terminal(&self.0) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // The isolated root driver additionally drains its exact outer cgroup.
        eprintln!("diagnostic descendant did not terminate in its cleanup allowance");
    }
}

struct Drain(Cleanup, bool);
impl Drain {
    fn finish(&mut self) -> bool {
        if self.1 {
            return true;
        }
        for _ in 0..1024 {
            if self.0.shutdown().is_ok() {
                self.1 = true;
                return true;
            }
            if self
                .0
                .pump(crate::MAX_PROTECTED_SERVICE_PROCESSES_V2)
                .is_err()
            {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        false
    }
}
impl Drop for Drain {
    fn drop(&mut self) {
        let drained = self.finish();
        assert!(
            drained || std::thread::panicking(),
            "fresh domain did not drain"
        );
    }
}

fn pipe() -> (OwnedFd, OwnedFd) {
    rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap()
}

fn read_ready(fd: &OwnedFd, bytes: &mut [u8], deadline: Instant) {
    rustix::fs::fcntl_setfl(fd, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut offset = 0;
    while offset < bytes.len() {
        assert!(
            Instant::now() < deadline,
            "fresh-domain fixture readiness timeout"
        );
        match rustix::io::read(fd, &mut bytes[offset..]) {
            Ok(0) => panic!("fresh-domain fixture closed readiness early"),
            Ok(n) => offset += n,
            Err(Errno::AGAIN | Errno::INTR) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("fresh-domain fixture read: {e}"),
        }
    }
}

fn terminal(fd: &OwnedFd) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec};
    let mut watched = [PollFd::new(fd, PollFlags::IN)];
    rustix::event::poll(
        &mut watched,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .unwrap();
    let flags = watched[0].revents();
    assert!(!flags.intersects(PollFlags::ERR | PollFlags::NVAL));
    flags.contains(PollFlags::IN)
}

#[test]
#[ignore = "requires isolated root cgroup-v2 lane and static descendant fixture"]
fn root_exit_does_not_retire_live_descendant_domain() {
    assert!(syscall::has_exact_root_identity());
    let helper =
        File::open(std::env::var_os("FE2O3_FRESH_DOMAIN_FIXTURE").expect("fixture path")).unwrap();
    let uid: u32 = std::env::var("FE2O3_FRESH_DOMAIN_UID")
        .expect("fixture uid")
        .parse()
        .unwrap();
    let gid: u32 = std::env::var("FE2O3_FRESH_DOMAIN_GID")
        .expect("fixture gid")
        .parse()
        .unwrap();
    let credentials = Credentials::new(uid, gid).unwrap();
    let source_storage = helper.metadata().unwrap().len() as usize + 4096;
    let (ready_read, ready_write) = pipe();
    let (profile_read, profile_write) = pipe();
    let (gate_read, gate_write) = pipe();
    let (_status_read, status_write) = rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::SEQPACKET,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let bindings = [Binding::new(ready_write.as_fd(), 3).unwrap()];
    let mut pool = Drain(
        Cleanup::admit(Account::new(Work::new(WORK), STORAGE)).unwrap(),
        false,
    );
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(source_storage).unwrap();
    // This explicitly selected root diagnostic admits no executable or proof
    // authority. The fixture is a privately built static helper with one pipe.
    let (stage, charge) = unsafe {
        StagedProtectedServiceExecV2::stage(
            &helper,
            &bindings,
            profile_write.as_fd(),
            gate_read.as_fd(),
            status_write.as_fd(),
            source_storage,
            &mut budget,
        )
    }
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    const INPUT_STORAGE: usize = 256;
    budget.reserve_storage(INPUT_STORAGE).unwrap();
    // Root-controlled private cgroup lane, immutable helper, no exported cgroup
    // handles, and this test is the sole direct-child consuming-wait owner.
    let (mut child, child_charge) = unsafe {
        stage.spawn_retaining_in_fresh_domain(
            credentials,
            Input(drops.clone()),
            INPUT_STORAGE,
            &mut pool.0,
            &mut budget,
        )
    }
    .unwrap();
    budget
        .reserve_storage(child_charge.additional_storage())
        .unwrap();
    drop(stage);
    drop(profile_write);
    drop(gate_read);
    drop(status_write);
    drop(ready_write);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut profile = [0];
    read_ready(&profile_read, &mut profile, deadline);
    assert_eq!(profile, [PROTECTED_SERVICE_PROFILE_READY_V1]);
    assert_eq!(
        rustix::io::write(&gate_write, &[PROTECTED_SERVICE_GATE_RELEASE_V1]).unwrap(),
        1
    );
    let mut descendant = [0; 4];
    read_ready(&ready_read, &mut descendant, deadline);
    let descendant = i32::from_ne_bytes(descendant);
    assert!(descendant > 0);
    let descendant_pidfd = Descendant(
        rustix::process::pidfd_open(
            rustix::process::Pid::from_raw(descendant).unwrap(),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap(),
    );
    assert!(!terminal(&descendant_pidfd.0));
    while child.is_live(&mut budget).unwrap() {
        assert!(Instant::now() < deadline, "fixture root did not exit");
        std::thread::sleep(Duration::from_millis(1));
    }
    // Root terminal observation is not a consuming wait and cannot release inputs.
    let descendant_group = std::fs::read_to_string(format!("/proc/{descendant}/cgroup")).unwrap();
    let root_group =
        std::fs::read_to_string(format!("/proc/{}/cgroup", child.pid().as_raw_pid())).unwrap();
    assert_eq!(descendant_group, root_group);
    assert_ne!(
        root_group,
        std::fs::read_to_string("/proc/self/cgroup").unwrap()
    );
    let unified = root_group
        .lines()
        .filter_map(|line| line.strip_prefix("0::"))
        .collect::<Vec<_>>();
    assert_eq!(unified.len(), 1);
    let domain = std::path::Path::new("/sys/fs/cgroup").join(unified[0].strip_prefix('/').unwrap());
    assert!(
        domain
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("fe2o3-native-")
    );
    assert!(std::fs::metadata(&domain).unwrap().is_dir());
    assert!(!terminal(&descendant_pidfd.0));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let _ = child.cancel();
    drop(child);
    assert!(pool.finish(), "aggregate cancellation failed");
    assert!(
        terminal(&descendant_pidfd.0),
        "retirement left the descendant alive"
    );
    assert_eq!(
        std::fs::metadata(domain).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    println!(
        "fresh-domain actual root exit, descendant cancellation and retained-input retirement passed"
    );
}
