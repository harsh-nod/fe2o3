//! Process tests use the real launcher/filter/controller. Probes request and
//! observe hostile mutations, or inject a descendant-query refusal and observe
//! real cleanup operations. They cannot admit requests or synthesize wait results.

use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

type Result<T> = std::result::Result<T, RetainedFunctionalRefinementRuntimeErrorV1>;
type Probe = Box<dyn FnMut(i32, &UserRegistersX86_64, bool) -> Result<()>>;
thread_local! { static PROBE: RefCell<Option<Probe>> = RefCell::new(None); }

pub(super) fn probe(pid: i32, registers: &UserRegistersX86_64, completed: bool) -> Result<()> {
    PROBE.with(|probe| match probe.borrow_mut().as_mut() {
        Some(probe) => probe(pid, registers, completed),
        None => Ok(()),
    })
}

struct ResetProbe;
impl Drop for ResetProbe {
    fn drop(&mut self) {
        PROBE.with(|probe| *probe.borrow_mut() = None);
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CleanupObservation {
    QueryRefused,
    ResumedBirthVisible(i32),
    GroupLeaderExitResumed(i32, Vec<i32>),
    ChildIdentified(i32),
    StopObserved(i32, i32),
    KillRequested(i32),
    TerminalReaped(i32, i32),
    TerminalValidated(i32, i32, ExitBoundary),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BirthRefusal {
    ConsumedQuery,
    AfterResume,
}

#[derive(Debug, Default)]
struct CleanupProbe {
    refusal: Option<BirthRefusal>,
    abort_group_exit: bool,
    observations: Vec<CleanupObservation>,
}

thread_local! { static CLEANUP_PROBE: RefCell<Option<CleanupProbe>> = RefCell::new(None); }

pub(super) fn fail_birth_query_once() -> bool {
    CLEANUP_PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let Some(probe) = probe.as_mut() else {
            return false;
        };
        if probe.refusal != Some(BirthRefusal::ConsumedQuery) {
            return false;
        }
        probe.refusal = None;
        probe.observations.push(CleanupObservation::QueryRefused);
        true
    })
}

pub(super) fn abort_after_birth_resume(parent: i32) -> Result<()> {
    let abort = CLEANUP_PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let Some(probe) = probe.as_mut() else {
            return false;
        };
        if probe.refusal != Some(BirthRefusal::AfterResume) {
            return false;
        }
        probe.refusal = None;
        true
    });
    if !abort {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    let path = format!("/proc/{parent}/task/{parent}/children");
    loop {
        // Diagnostic rendezvous only: do not consume the birth stop or grant
        // custody from procfs. Cleanup still must query the actual birth event,
        // park the child, and observe terminal waits for both owned processes.
        let children = std::fs::read_to_string(&path)
            .map_err(|e| process_failure(format!("observe resumed fixture creation: {e}")))?;
        let children = children.split_whitespace().collect::<Vec<_>>();
        if let [child] = children.as_slice() {
            let child = child
                .parse::<i32>()
                .ok()
                .filter(|pid| *pid > 0)
                .ok_or_else(|| process_failure("fixture child PID is not canonical"))?;
            record_cleanup(CleanupObservation::ResumedBirthVisible(child));
            return Err(process_failure(
                "fixture abort after birth resume before wait",
            ));
        }
        if !children.is_empty() || Instant::now() >= deadline {
            return Err(process_failure(
                "fixture did not observe exactly one resumed birth",
            ));
        }
        thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
    }
}

pub(super) fn abort_after_group_leader_exit_resume(tree: &Tracees, leader: i32) -> Result<()> {
    let abort = CLEANUP_PROBE.with(|probe| {
        probe
            .borrow_mut()
            .as_mut()
            .is_some_and(|probe| std::mem::take(&mut probe.abort_group_exit))
    });
    if !abort {
        return Ok(());
    }
    let task = &tree[&leader];
    if !task.leader
        || !matches!(task.exit_boundary, Some(ExitBoundary::Task(_)))
        || task.current_stop.is_some()
    {
        return Err(process_failure(
            "fixture leader lacks a resumed authenticated EXIT",
        ));
    }
    let members = tree
        .iter()
        .filter_map(|(&pid, task)| {
            (task.thread_group == leader && !task.queued_status.is_some_and(|s| !stopped(s)))
                .then_some(pid)
        })
        .collect::<Vec<_>>();
    if !members
        .iter()
        .any(|&pid| pid != leader && tree[&pid].current_stop.is_some())
    {
        return Err(process_failure(
            "fixture exit-group siblings were not still parked",
        ));
    }
    record_cleanup(CleanupObservation::GroupLeaderExitResumed(leader, members));
    Err(process_failure(
        "fixture abort during exit_group after leader EXIT resume",
    ))
}

fn record_cleanup(observation: CleanupObservation) {
    CLEANUP_PROBE.with(|probe| {
        if let Some(probe) = probe.borrow_mut().as_mut() {
            probe.observations.push(observation);
        }
    });
}

pub(super) fn record_birth_identity(child: i32) {
    record_cleanup(CleanupObservation::ChildIdentified(child));
}

pub(super) fn record_cleanup_kill_request(pid: i32) {
    record_cleanup(CleanupObservation::KillRequested(pid));
}

pub(super) fn record_cleanup_terminal(pid: i32, status: i32) {
    record_cleanup(CleanupObservation::TerminalReaped(pid, status));
}

pub(super) fn record_cleanup_stop(pid: i32, status: i32) {
    record_cleanup(CleanupObservation::StopObserved(pid, status));
}

pub(super) fn record_exit_terminal(pid: i32, status: i32, boundary: ExitBoundary) {
    record_cleanup(CleanupObservation::TerminalValidated(pid, status, boundary));
}

struct ResetCleanupProbe;
impl Drop for ResetCleanupProbe {
    fn drop(&mut self) {
        CLEANUP_PROBE.with(|probe| *probe.borrow_mut() = None);
    }
}

#[repr(C)]
#[derive(Default)]
struct Control {
    mode: AtomicU64,
    phase: AtomicU64,
    done: AtomicU64,
    ready: AtomicU64,
    target: AtomicU64,
    offset: AtomicU64,
    validated: AtomicU64,
    completed: AtomicU64,
    clone3_unavailable: AtomicU64,
    mutator_ready: AtomicU64,
    attempted: AtomicU64,
}

unsafe extern "C" {
    fn signal(number: i32, handler: usize) -> usize;
    fn tgkill(group: i32, tid: i32, signal: i32) -> i32;
    fn mmap(
        address: *mut c_void,
        size: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(address: *mut c_void, size: usize) -> i32;
    fn mremap(
        address: *mut c_void,
        old_size: usize,
        new_size: usize,
        flags: i32,
        ...
    ) -> *mut c_void;
}

struct SharedControl(*mut Control);
impl SharedControl {
    fn map(fd: i32) -> Self {
        // SAFETY: the fixture retains a page-sized memfd and maps exactly one page.
        let address = unsafe { mmap(std::ptr::null_mut(), 4096, 3, 1, fd, 0) };
        assert_ne!(address as isize, -1);
        Self(address.cast())
    }
    fn get(&self) -> &Control {
        // SAFETY: retained mapping is page-sized; every shared field is atomic.
        unsafe { &*self.0 }
    }
}
impl Drop for SharedControl {
    fn drop(&mut self) {
        // SAFETY: this owner unmaps its own page exactly once.
        unsafe {
            munmap(self.0.cast(), 4096);
        }
    }
}

fn host_executables() -> Vec<AllowedRuntimeExecutableV1> {
    let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
    let mut paths = std::collections::BTreeSet::from(["/bin/true".to_owned()]);
    for row in maps.lines() {
        let fields = row.split_whitespace().collect::<Vec<_>>();
        if fields
            .get(1)
            .is_some_and(|s| s.as_bytes().get(2) == Some(&b'x'))
            && let Some(path) = fields.get(5).filter(|p| p.starts_with('/'))
        {
            paths.insert((*path).to_owned());
        }
    }
    paths.iter().map(|p| tests::allowed_executable(p)).collect()
}

fn hostile_race(mode: u64, expected_error: &str) {
    let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let shared =
        rustix::fs::memfd_create(c"proof-race-fixture", rustix::fs::MemfdFlags::CLOEXEC).unwrap();
    rustix::fs::ftruncate(&shared, 4096).unwrap();
    let mapping = SharedControl::map(shared.as_raw_fd());
    mapping.get().mode.store(mode, Ordering::SeqCst);
    mapping.get().offset.store(
        tests::allowed_executable("/bin/true").executable_file_ranges[0].0,
        Ordering::SeqCst,
    );
    let probe_mapping = SharedControl::map(shared.as_raw_fd());
    PROBE.with(|probe| {
        *probe.borrow_mut() = Some(Box::new(move |pid, r, completed| {
            let control = probe_mapping.get();
            if matches!(mode, 4..=7) {
                let leader = thread_group_id(pid)?;
                if mode == 7 && pid == leader {
                    return Ok(());
                }
                if !completed
                    && r.orig_rax == u64::from(MMAP_SYSCALL)
                    && control.ready.load(Ordering::SeqCst) == 1
                    && control
                        .phase
                        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                {
                    let recipient = if mode == 7 { pid } else { leader };
                    control.target.store(recipient as u64, Ordering::SeqCst);
                    // SAFETY: the diagnostic fixture installed this handler; target
                    // exactly the retained caller selected by this fixture mode.
                    if unsafe { tgkill(leader, recipient, 10) } != 0 {
                        return Err(io_process_failure("signal fixture leader exit"));
                    }
                }
                return Ok(());
            }
            let expected_syscall = match mode {
                1 => MMAP_SYSCALL,
                2 => CLONE3_SYSCALL,
                _ => MREMAP_SYSCALL,
            };
            let target = if mode == 1 { r.r8 } else { r.rdi };
            if control.ready.load(Ordering::SeqCst) != 1
                || r.orig_rax != u64::from(expected_syscall)
                || target != control.target.load(Ordering::SeqCst)
            {
                return Ok(());
            }
            if !completed {
                if control.mutator_ready.load(Ordering::SeqCst) != 1
                    || control.attempted.load(Ordering::SeqCst) != 0
                {
                    return Err(process_failure(
                        "hostile mutator was not ready at validation",
                    ));
                }
                control.validated.fetch_add(1, Ordering::SeqCst);
                control.phase.store(1, Ordering::SeqCst);
                // This deliberately widens the original decision/use race. A running
                // sibling would observe phase and mutate the FD/map/argument here.
                thread::sleep(Duration::from_millis(25));
            } else {
                let result = read_registers(pid)?.rax as i64;
                if mode == 2 && result == -38 {
                    control.clone3_unavailable.store(1, Ordering::SeqCst);
                    return Err(process_failure("fixture clone3 unavailable: ENOSYS"));
                }
                if result <= 0 {
                    return Err(process_failure(
                        "fixture requires an actual successful kernel operation",
                    ));
                }
                control.completed.fetch_add(1, Ordering::SeqCst);
            }
            if control.attempted.load(Ordering::SeqCst) != 0
                || control.done.load(Ordering::SeqCst) != 0
            {
                return Err(process_failure(
                    "hostile sibling ran inside the stable decision/completion boundary",
                ));
            }
            Ok(())
        }))
    });
    let _reset = ResetProbe;
    let source = rustix::io::fcntl_dupfd_cloexec(&shared, 200).unwrap();
    let shared_file = File::from(shared);
    let bindings = [DescriptorBinding {
        source: source.as_raw_fd(),
        destination: 190,
        close_on_exec: false,
        identity: ObjectSnapshotV2::capture(&shared_file, "race-control")
            .unwrap()
            .object_identity(),
    }];
    let executable = std::env::current_exe().unwrap();
    let selector = format!(
        "{}::seized_race_child",
        module_path!().split_once("::").unwrap().1
    );
    let mut command = Command::new(&executable);
    command
        .args([
            "--exact",
            &selector,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("FE2O3_PRIVATE_SEIZED_RACE", "1");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut child = seized_spawn::spawn(command, bindings.to_vec(), 10, deadline).unwrap();
    let pid = child.id() as i32;
    let _cleanup_reset = ResetCleanupProbe;
    if matches!(mode, 5..=7) {
        CLEANUP_PROBE.with(|probe| {
            *probe.borrow_mut() = Some(CleanupProbe {
                abort_group_exit: mode == 5,
                ..Default::default()
            });
        });
    }
    let error = tests::expect_error(supervise(
        &mut child,
        &bindings,
        tests::identity(executable.to_str().unwrap()),
        tests::identity("/bin/true"),
        &host_executables(),
        true,
        false,
        deadline,
        16384,
    ));
    if mapping.get().clone3_unavailable.load(Ordering::SeqCst) == 1 {
        assert!(
            error
                .to_string()
                .contains("fixture clone3 unavailable: ENOSYS"),
            "{error}"
        );
        assert_eq!(mapping.get().validated.load(Ordering::SeqCst), 1);
        assert_eq!(mapping.get().completed.load(Ordering::SeqCst), 0);
        tests::assert_process_disappears(pid);
        eprintln!(
            "SKIP concurrent_clone3_argument_write_waits_until_child_exec_and_completion: kernel returned ENOSYS; no genuine clone3 race coverage, no fallback"
        );
        return;
    }
    assert!(error.to_string().contains(expected_error), "{error}");
    if mode == 5 {
        assert!(!error.to_string().contains("failed to reap"), "{error}");
        assert_eq!(mapping.get().phase.load(Ordering::SeqCst), 1);
        let probe = CLEANUP_PROBE.with(|probe| probe.borrow_mut().take().unwrap());
        let observations = &probe.observations;
        let (leader, members) = observations
            .iter()
            .find_map(|o| match o {
                CleanupObservation::GroupLeaderExitResumed(leader, members) => {
                    Some((*leader, members))
                }
                _ => None,
            })
            .expect("fixture must abort after the real leader EXIT resume with siblings parked");
        assert_eq!(leader, pid);
        assert!(
            members.contains(&leader) && members.len() >= 2,
            "{observations:?}"
        );
        for member in members {
            let statuses = observations
                .iter()
                .filter_map(|o| match o {
                    CleanupObservation::TerminalReaped(pid, status) if pid == member => {
                        Some(*status)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                statuses.len(),
                1,
                "missing exact terminal wait for {member}: {observations:?}"
            );
            assert!(matches!(
                terminal_status(statuses[0]),
                (Some(0), None) | (None, Some(SIGKILL))
            ));
            tests::assert_process_disappears(*member);
        }
        return;
    }
    if matches!(mode, 6 | 7) {
        let probe = CLEANUP_PROBE.with(|probe| probe.borrow_mut().take().unwrap());
        let terminals = probe
            .observations
            .iter()
            .filter_map(|o| match o {
                CleanupObservation::TerminalValidated(pid, status, boundary) => {
                    Some((*pid, (*status, *boundary)))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        assert!(terminals.len() >= 2, "{probe:?}");
        assert_eq!(
            terminals.len(),
            probe
                .observations
                .iter()
                .filter(|o| matches!(o, CleanupObservation::TerminalValidated(..)))
                .count()
        );
        assert!(terminals.contains_key(&pid));
        let requester = mapping.get().target.load(Ordering::SeqCst) as i32;
        assert_eq!(requester == pid, mode == 6);
        assert_eq!(terminals[&requester], (0, ExitBoundary::Task(0)));
        assert!(!probe.observations.iter().any(|o| matches!(
            o,
            CleanupObservation::KillRequested(_) | CleanupObservation::TerminalReaped(_, _)
        )));
        for (tid, (status, boundary)) in terminals {
            assert_eq!(status, 0);
            assert!(boundary.matches(status));
            tests::assert_process_disappears(tid);
        }
        assert_eq!(mapping.get().phase.load(Ordering::SeqCst), 1);
        return;
    }
    if mode == 4 {
        assert_eq!(mapping.get().phase.load(Ordering::SeqCst), 1);
        tests::assert_process_disappears(pid);
        return;
    }
    assert_eq!(mapping.get().validated.load(Ordering::SeqCst), 1);
    assert_eq!(mapping.get().completed.load(Ordering::SeqCst), 1);
    assert_eq!(mapping.get().done.load(Ordering::SeqCst), 1);
    assert_eq!(mapping.get().attempted.load(Ordering::SeqCst), 1);
    tests::assert_process_disappears(pid);
}

#[test]
fn concurrent_fd_replacement_waits_until_mmap_completion() {
    hostile_race(1, "executable mmap object or file range is outside");
}

#[test]
fn concurrent_clone3_argument_write_waits_until_child_exec_and_completion() {
    hostile_race(2, "clone3 request is outside the pinned");
}

#[test]
fn concurrent_mapping_replacement_waits_until_remap_completion() {
    hostile_race(3, "mapping remap covers an executable source range");
}

#[test]
fn unsupported_leader_only_exit_with_live_siblings_is_refused_and_reaped() {
    hostile_race(4, "unsupported leader-only SYS_exit with live siblings");
}

#[test]
fn aborted_exit_group_drain_retains_exiting_leader_and_reaps_siblings() {
    hostile_race(
        5,
        "fixture abort during exit_group after leader EXIT resume",
    );
}

#[test]
fn ordinary_group_exit_with_parked_siblings_reaches_terminal_validation() {
    for _ in 0..8 {
        // This fixture has no solver. Reaching that final role check requires
        // actual terminal waits for the entire normally exiting thread group.
        hostile_race(6, "Z3 descendant was not observed");
        hostile_race(7, "Z3 descendant was not observed");
    }
}

#[test]
fn terminal_status_must_match_its_exact_task_or_group_exit_boundary() {
    for exit in [ExitBoundary::Task(0), ExitBoundary::Group(0)] {
        assert!(exit.matches(0));
        assert!(!exit.matches(1 << 8));
        assert!(!exit.matches(SIGKILL));
        assert!(!exit.matches((SIGTRAP << 8) | 0x7f));
    }
    assert!(ExitBoundary::Group(17 << 8).matches(17 << 8));
    assert!(!ExitBoundary::Group(17 << 8).matches(0));
}

fn wait_for(value: &AtomicU64, expected: u64, deadline: Instant) {
    while value.load(Ordering::SeqCst) != expected {
        assert!(
            Instant::now() < deadline,
            "diagnostic fixture rendezvous deadline elapsed"
        );
        thread::yield_now();
    }
}

extern "C" fn fixture_leader_exit(_: i32) {
    // SAFETY: diagnostic signal handler performs only SYS_exit and never returns.
    unsafe {
        std::arch::asm!("syscall", "ud2", in("rax") 60_u64, in("rdi") 0_u64, options(noreturn));
    }
}

extern "C" fn fixture_group_exit(_: i32) {
    // SAFETY: diagnostic signal handler performs only SYS_exit_group and never returns.
    unsafe {
        std::arch::asm!("syscall", "ud2", in("rax") 231_u64, in("rdi") 0_u64, options(noreturn));
    }
}

#[test]
#[ignore = "private controller fixture"]
fn seized_race_child() {
    if std::env::var_os("FE2O3_PRIVATE_SEIZED_RACE").is_none() {
        return;
    }
    let control = SharedControl::map(190);
    let mode = control.get().mode.load(Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(8);
    if matches!(mode, 4..=7) {
        // SAFETY: only this disposable fixture installs SIGUSR1's raw exit handler.
        let handler = if mode == 4 {
            fixture_leader_exit
        } else {
            fixture_group_exit
        };
        assert_ne!(
            unsafe { signal(10, handler as *const () as usize) },
            usize::MAX
        );
        control.get().ready.store(1, Ordering::SeqCst);
        // SAFETY: a fresh anonymous fixture page triggers the parked test probe.
        assert_ne!(
            unsafe { mmap(std::ptr::null_mut(), 4096, 3, 0x22, -1, 0) } as isize,
            -1
        );
        wait_for(&control.get().phase, 2, deadline);
        panic!("controller failed to refuse the leader-exit fixture");
    }
    let offset = control.get().offset.load(Ordering::SeqCst) as i64;
    let approved = File::open("/bin/true").unwrap();
    let unapproved = File::open("/dev/zero").unwrap();
    // SAFETY: fixture-only anonymous data mapping, retained until process cleanup.
    let region = unsafe { mmap(std::ptr::null_mut(), 65536, 3, 0x22, -1, 0) } as usize;
    assert_ne!(region, usize::MAX);
    let args = Arc::new(std::array::from_fn::<_, 11, _>(|_| AtomicU64::new(0)));
    args[0].store(RUST_PROCESS_CLONE3_FLAGS, Ordering::SeqCst);
    args[4].store(SIGCHLD, Ordering::SeqCst);
    args[5].store(region as u64, Ordering::SeqCst);
    args[6].store(65536, Ordering::SeqCst);
    let target = match mode {
        1 => 193,
        2 => args.as_ptr() as u64,
        _ => region as u64,
    };
    if mode == 1 {
        // SAFETY: fixture owns this otherwise unused descriptor number.
        assert_eq!(unsafe { dup2(approved.as_raw_fd(), 193) }, 193);
    }
    let args_other = Arc::clone(&args);
    let other_approved = approved.try_clone().unwrap();
    let mutator = thread::spawn(move || {
        let control = SharedControl::map(190);
        control.get().mutator_ready.store(1, Ordering::SeqCst);
        wait_for(&control.get().phase, 1, deadline);
        // This precedes even mmap's seccomp stop, so exclusion cannot be inferred
        // merely from the controller not yet handling that second trapped syscall.
        control.get().attempted.store(1, Ordering::SeqCst);
        match mode {
            1 => {
                // SAFETY: this hostile sibling intentionally replaces the shared FD.
                assert_eq!(unsafe { dup2(unapproved.as_raw_fd(), 193) }, 193);
            }
            2 => {
                args_other[0].store(0x0000_0200, Ordering::SeqCst);
            }
            _ => {
                // SAFETY: replace only the fixture-owned data mapping with admitted RX.
                assert_eq!(
                    unsafe {
                        mmap(
                            region as *mut c_void,
                            4096,
                            5,
                            0x12,
                            other_approved.as_raw_fd(),
                            offset,
                        )
                    } as usize,
                    region
                );
            }
        }
        control.get().done.store(1, Ordering::SeqCst);
        wait_for(&control.get().phase, 2, deadline);
    });
    wait_for(&control.get().mutator_ready, 1, deadline);
    control.get().target.store(target, Ordering::SeqCst);
    control.get().ready.store(1, Ordering::SeqCst);
    for attempt in 0..2 {
        match mode {
            1 => {
                // SAFETY: mmap reads the deliberately raced fixture descriptor.
                let result = unsafe { mmap(std::ptr::null_mut(), 4096, 5, 2, 193, offset) };
                assert_ne!(result as isize, -1);
            }
            2 => {
                assert!(
                    clone3_exec(args.as_ptr().cast()) > 0,
                    "clone3 positive unavailable"
                );
            }
            _ => {
                // SAFETY: this is the fixture's fully mapped, page-aligned region.
                assert_eq!(
                    unsafe { mremap(region as *mut c_void, 4096, 4096, 0) } as usize,
                    region
                );
            }
        }
        if attempt == 0 {
            wait_for(&control.get().done, 1, deadline);
            assert_eq!(control.get().attempted.load(Ordering::SeqCst), 1);
            control.get().ready.store(2, Ordering::SeqCst);
        }
    }
    control.get().phase.store(2, Ordering::SeqCst);
    mutator.join().unwrap();
    panic!("controller admitted the hostile second operation");
}

fn clone3_exec(arguments: *const u64) -> i64 {
    let argv = [c"/bin/true".as_ptr(), std::ptr::null()];
    let result: i64;
    // SAFETY: the vfork child has a distinct supplied stack and never executes Rust
    // or returns here; it uses only execve/_exit. Parent resumes on its original
    // stack after exec. All pointer backing remains retained by the blocked parent.
    unsafe {
        std::arch::asm!(
            "syscall", "test rax, rax", "jnz 2f",
            "mov rdi, r8", "mov rsi, r9", "xor edx, edx", "mov eax, 59", "syscall",
            "mov edi, 126", "mov eax, 60", "syscall", "ud2", "2:",
            inlateout("rax") u64::from(CLONE3_SYSCALL) => result,
            inlateout("rdi") arguments => _, inlateout("rsi") CLONE3_ARGUMENT_BYTES => _,
            inlateout("rdx") 0_u64 => _, in("r8") c"/bin/true".as_ptr(), in("r9") argv.as_ptr(),
            lateout("rcx") _, lateout("r11") _,
        );
    }
    result
}

#[test]
fn seized_invalid_exec_is_reaped_without_spawn_helper() {
    let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut child = seized_spawn::spawn(
        Command::new("/fe2o3-no-such-seized-fixture"),
        vec![],
        2,
        deadline,
    )
    .unwrap();
    let pid = child.id() as i32;
    assert!(
        supervise(
            &mut child,
            &[],
            tests::identity("/bin/true"),
            tests::identity("/bin/true"),
            &[],
            false,
            false,
            deadline,
            4096
        )
        .is_err()
    );
    tests::assert_process_disappears(pid);
}

#[test]
fn seized_expired_deadline_refuses_before_fork() {
    assert!(seized_spawn::spawn(Command::new("/bin/true"), vec![], 2, Instant::now()).is_err());
}

#[test]
fn queued_terminal_status_is_not_signalled_again() {
    let pid = std::process::id() as i32;
    let mut tree = Tracees::new().unwrap();
    let mut task = Tracee::pending(TraceeRole::Verifier, pid, true);
    task.exit_boundary = Some(ExitBoundary::Task(0));
    task.queued_status = Some(0);
    task.terminal_consumed = true;
    tree.insert(pid, task).unwrap();
    // The fixture's own PID makes a mistaken second SIGKILL immediately observable.
    terminate_tree(&mut tree).unwrap();
}

#[test]
fn consumed_unregistered_birth_is_discovered_before_kill_and_terminally_reaped() {
    assert_birth_refusal_reaped(BirthRefusal::ConsumedQuery);
}

#[test]
fn aborted_resumed_birth_is_quiesced_before_kill_and_terminally_reaped() {
    assert_birth_refusal_reaped(BirthRefusal::AfterResume);
}

fn assert_birth_refusal_reaped(refusal: BirthRefusal) {
    let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "/bin/true; :"]).env_clear();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut child = seized_spawn::spawn(command, vec![], 5, deadline).unwrap();
    let parent = child.id() as i32;
    let _reset = ResetCleanupProbe;
    CLEANUP_PROBE.with(|probe| {
        *probe.borrow_mut() = Some(CleanupProbe {
            refusal: Some(refusal),
            ..Default::default()
        });
    });
    let error = tests::expect_error(supervise(
        &mut child,
        &[],
        tests::identity("/bin/sh"),
        tests::identity("/bin/true"),
        &[],
        false,
        false,
        deadline,
        4096,
    ));
    let probe = CLEANUP_PROBE.with(|probe| probe.borrow_mut().take().unwrap());
    let message = error.to_string();
    let expected = match refusal {
        BirthRefusal::ConsumedQuery => "fixture injected birth GETEVENTMSG failure",
        BirthRefusal::AfterResume => "fixture abort after birth resume before wait",
    };
    assert!(message.contains(expected), "{message}");
    assert!(!message.contains("failed to reap"), "{message}");
    let observations = &probe.observations;
    let (identified_at, descendant) = observations
        .iter()
        .enumerate()
        .find_map(|(index, observation)| match observation {
            CleanupObservation::ChildIdentified(pid) => Some((index, *pid)),
            _ => None,
        })
        .expect("cleanup must recover the real child identity");
    assert!(descendant > 0 && descendant != parent);
    let expected_first = match refusal {
        BirthRefusal::ConsumedQuery => CleanupObservation::QueryRefused,
        BirthRefusal::AfterResume => CleanupObservation::ResumedBirthVisible(descendant),
    };
    assert_eq!(observations.first(), Some(&expected_first));
    assert_eq!(
        observations
            .iter()
            .filter(|o| matches!(o, CleanupObservation::QueryRefused))
            .count(),
        usize::from(refusal == BirthRefusal::ConsumedQuery),
    );
    let first_kill = observations
        .iter()
        .position(|o| matches!(o, CleanupObservation::KillRequested(_)))
        .expect("cleanup must terminate the refused tree");
    assert!(identified_at < first_kill, "{observations:?}");
    for pid in [parent, descendant] {
        assert!(
            observations.contains(&CleanupObservation::KillRequested(pid)),
            "{observations:?}"
        );
        // The consumed-query case already owns its parent's birth stop; the
        // after-resume case must acquire *both* stops before the first kill.
        if pid == descendant || refusal == BirthRefusal::AfterResume {
            assert!(observations[..first_kill].iter().any(|o| {
                matches!(o, CleanupObservation::StopObserved(observed, status) if *observed == pid && stopped(*status))
            }), "cleanup killed before parking PID {pid}: {observations:?}");
        }
        let statuses = observations
            .iter()
            .filter_map(|observation| match observation {
                CleanupObservation::TerminalReaped(observed, status) if *observed == pid => {
                    Some(*status)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            statuses.len(),
            1,
            "exact terminal wait required for PID {pid}: {observations:?}"
        );
        assert_eq!(terminal_status(statuses[0]), (None, Some(SIGKILL)));
        tests::assert_process_disappears(pid);
    }
}

#[test]
fn only_unregistered_birth_stops_authorize_cleanup_identity_queries() {
    for event in [PTRACE_EVENT_FORK, PTRACE_EVENT_VFORK, PTRACE_EVENT_CLONE] {
        let mut stop =
            TraceeStop::observed(((event as i32) << 16) | (SIGTRAP << 8) | 0x7f).unwrap();
        assert!(stop.unregistered_birth());
        stop.birth_registered = true;
        assert!(!stop.unregistered_birth());
    }
    for event in [
        0,
        PTRACE_EVENT_STOP,
        PTRACE_EVENT_EXEC,
        PTRACE_EVENT_EXIT,
        PTRACE_EVENT_SECCOMP,
    ] {
        let stop = TraceeStop::observed(((event as i32) << 16) | (SIGTRAP << 8) | 0x7f).unwrap();
        assert!(!stop.unregistered_birth());
    }
    assert!(TraceeStop::observed(0).is_none());
    assert!(TraceeStop::observed(SIGKILL).is_none());
}

#[test]
fn data_and_cross_process_sharing_are_not_clone_authority() {
    for flags in [0x100, 0x400, 0x500, 0x40_0000, 0x4111 | 0x400] {
        let r = UserRegistersX86_64 {
            orig_rax: u64::from(CLONE_SYSCALL),
            rdi: flags,
            ..Default::default()
        };
        assert!(stable::birth_request(std::process::id() as i32, &r).is_err());
    }
}

#[test]
fn output_capacity_rejects_an_oversized_first_read_without_appending() {
    for limit in [0, 1, 4095] {
        let mut capture = Capture {
            bytes: Vec::new(),
            eof: false,
        };
        let mut reader = io::Cursor::new(vec![b'x'; limit + 1]);
        let error = drain(&mut reader, &mut capture, limit, OutputStream::Stdout).unwrap_err();
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::OutputTooLarge
        );
        assert!(capture.bytes.is_empty());
        assert!(!capture.eof);
    }
    let mut capture = Capture {
        bytes: Vec::new(),
        eof: false,
    };
    drain(
        &mut io::Cursor::new(b"x"),
        &mut capture,
        1,
        OutputStream::Stdout,
    )
    .unwrap();
    assert_eq!(capture.bytes, b"x");
    assert!(capture.eof);
    assert!(
        drain(
            &mut io::Cursor::new(b"y"),
            &mut capture,
            1,
            OutputStream::Stdout
        )
        .is_err()
    );
    assert_eq!(capture.bytes, b"x");
}

#[test]
fn expired_inspection_deadline_refuses_admission_while_child_remains_stopped() {
    let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let outer = Instant::now() + Duration::from_secs(5);
    let mut child = seized_spawn::spawn(Command::new("/bin/true"), vec![], 5, outer).unwrap();
    let pid = child.id() as i32;
    let tree = &mut child.run().unwrap().tracees;
    let observation = (|| -> Result<()> {
        let status = wait_for_specific(pid, outer)?;
        tree.get_mut(&pid).unwrap().current_stop = TraceeStop::observed(status);
        if !stopped(status) {
            tree.get_mut(&pid).unwrap().queued_status = Some(status);
            tree.get_mut(&pid).unwrap().terminal_consumed = true;
            return Err(process_failure(
                "inspection fixture exited before its exec stop",
            ));
        }
        if (status as u32) >> 16 != PTRACE_EVENT_EXEC {
            return Err(process_failure("inspection fixture missed its exec stop"));
        }
        let deadline = stable::park_for_inspection(tree, outer, &mut || Ok(()))?;
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let error = stable::check_before_release(deadline, &mut || Ok(())).unwrap_err();
        if error.kind() != RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut {
            return Err(process_failure(
                "inspection admission did not enforce its retained deadline",
            ));
        }
        // GETREGS succeeding proves the child remains ptrace-stopped before cleanup.
        read_registers(pid)?;
        Ok(())
    })();
    let cleanup = terminate_tree(tree);
    observation.unwrap();
    cleanup.unwrap();
    tests::assert_process_disappears(pid);
}
