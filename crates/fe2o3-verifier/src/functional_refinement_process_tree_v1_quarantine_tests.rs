//! Permanent failures run in fresh reexec drivers. A separate diagnostic
//! subreaper owns their process group and requires actual terminal waits.
//! No fixture resets quarantine or grants runtime/proof admission.

use super::*;
use std::cell::Cell;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::sync::atomic::AtomicUsize;

const PREFIX: &str = "retained_functional_refinement_runtime_v1::linux::functional_refinement_process_tree_v1::quarantine_tests::";
const CASE: &str = "FE2O3_PRIVATE_QUARANTINE_CASE";
const PROTECTED_ROOT: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
static FORKS: AtomicUsize = AtomicUsize::new(0);
// Fixture-only custody, separate from the library's proof quarantine. Tests in
// this process serialize here; an unresolved diagnostic owner is never retried.
static DIAGNOSTIC_DOMAIN: std::sync::Mutex<Option<DiagnosticDomain>> = std::sync::Mutex::new(None);

struct DiagnosticDomain {
    child: std::process::Child,
    out: Capture,
    err: Capture,
    terminals: Vec<i32>,
}

fn publish_domain(slot: &mut Option<DiagnosticDomain>, command: &mut Command) {
    assert!(
        slot.is_none(),
        "prior diagnostic domain remains unresolved; no retry"
    );
    let terminals = Vec::with_capacity(MAX_TRACEES + 3);
    let child = spawn_fixture(command);
    // No fallible work between spawn and owner publication.
    *slot = Some(DiagnosticDomain {
        child,
        terminals,
        out: Capture {
            bytes: Vec::new(),
            eof: false,
        },
        err: Capture {
            bytes: Vec::new(),
            eof: false,
        },
    });
}

impl DiagnosticDomain {
    fn nonblocking(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        make_nonblocking(self.child.stdout.as_ref().expect("fixture stdout"))?;
        make_nonblocking(self.child.stderr.as_ref().expect("fixture stderr"))
    }
    fn drain(&mut self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        drain(
            self.child.stdout.as_mut().expect("fixture stdout"),
            &mut self.out,
            65536,
        )?;
        drain(
            self.child.stderr.as_mut().expect("fixture stderr"),
            &mut self.err,
            65536,
        )
    }
}
thread_local! {
    static MODE: Cell<u8> = const { Cell::new(0) };
    static QUERIES: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn cleanup_deadline(deadline: Instant) -> Instant {
    if matches!(MODE.get(), 1 | 4) {
        Instant::now()
    } else {
        deadline
    }
}

pub(super) fn after_fork() {
    FORKS.fetch_add(1, Ordering::SeqCst);
    assert_ne!(MODE.get(), 3, "fixture unwind after PID publication");
}

pub(super) fn before_attach() -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if MODE.get() == 4 {
        Err(process_failure("fixture attach refusal"))
    } else {
        Ok(())
    }
}

pub(super) fn refuse_birth_query() -> bool {
    let mode = MODE.get();
    if !matches!(mode, 2 | 6) {
        return false;
    }
    let queries = QUERIES.get() + 1;
    QUERIES.set(queries);
    assert!(
        !(mode == 6 && queries == 2),
        "fixture unwind inside unresolved-birth cleanup"
    );
    true
}

pub(super) fn after_birth_resume(pid: i32) {
    if MODE.get() == 5 {
        assert_eq!(diagnostic_children(pid).len(), 1);
        panic!("fixture unwind after creation resume before wait");
    }
}

fn diagnostic_children(pid: i32) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let contents = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap();
        let children = contents
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect::<Vec<_>>();
        if !children.is_empty() {
            return children;
        }
        assert!(Instant::now() < deadline, "real birth never became visible");
        thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
    }
}

fn private_command(fixture: &str, case: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            format!("{PREFIX}{fixture}"),
            "--exact".into(),
            "--ignored".into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ])
        .env_clear()
        .env(CASE, case)
        .env(
            "LD_LIBRARY_PATH",
            std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let parent = std::process::id() as i32;
    unsafe extern "C" {
        fn getppid() -> i32;
    }
    // Diagnostic crash backstop only, not terminal/cleanup evidence. The
    // production proof launcher has its independent pre-gate parent-death check.
    unsafe {
        command.pre_exec(move || {
            if prctl(1, SIGKILL as usize, 0_usize, 0_usize, 0_usize) != 0 || getppid() != parent {
                return Err(io::Error::from_raw_os_error(1));
            }
            Ok(())
        });
    }
    command
}

fn spawn_fixture(command: &mut Command) -> std::process::Child {
    crate::executor::spawn_artifact_coordinated_child(command).unwrap()
}

fn run_domain(case: &str) {
    let mut slot = DIAGNOSTIC_DOMAIN.lock().unwrap_or_else(|p| p.into_inner());
    publish_domain(
        &mut slot,
        &mut private_command("quarantine_domain_fixture", case),
    );
    let domain = slot.as_mut().expect("published domain");
    let mut read_error = domain.nonblocking().err();
    let deadline = Instant::now() + Duration::from_secs(if case == "protected" { 180 } else { 25 });
    let status = loop {
        if read_error.is_none() {
            read_error = domain.drain().err();
        }
        match domain.child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => panic!(
                "diagnostic domain owner retained after wait failure; cleanup unconfirmed: {error}"
            ),
        }
        assert!(
            Instant::now() < deadline,
            "diagnostic domain owner retained after outer deadline; cleanup unconfirmed"
        );
        thread::sleep(POLL_INTERVAL);
    };
    if read_error.is_none() {
        read_error = domain.drain().err();
    }
    // Only an actual terminal wait releases this process's direct child.
    let domain = slot.take().expect("terminal domain");
    assert!(
        status.success(),
        "domain failed: {} {}",
        String::from_utf8_lossy(&domain.out.bytes),
        String::from_utf8_lossy(&domain.err.bytes)
    );
    assert!(
        read_error.is_none(),
        "domain output read failed after confirmed termination: {read_error:?}"
    );
    assert!(String::from_utf8_lossy(&domain.out.bytes).contains("OUTER_TERMINALS_CONFIRMED"));
}
#[test]
fn cleanup_deadline_retains_owned_tasks_and_resources() {
    run_domain("deadline");
}
#[test]
fn pending_birth_query_failure_quarantines_without_losing_custody() {
    run_domain("birth");
}
#[test]
fn unwind_retains_originating_thread_custody() {
    for case in ["unwind-root", "unwind-resume", "unwind-birth"] {
        run_domain(case);
    }
}
#[test]
fn gated_spawn_failure_retains_unseized_child_custody() {
    run_domain("attach");
}
#[test]
fn tracer_thread_exit_does_not_clear_quarantine() {
    run_domain("thread-exit");
}
#[test]
fn quarantined_attempt_refuses_execution_on_every_thread() {
    run_domain("refusal");
}
#[test]
fn whole_attempt_gate_is_nonblocking_through_completion() {
    run_domain("gate");
}

#[test]
fn inherited_quarantine_refuses_at_allocation_free_process_preflight() {
    run_domain("inherited");
}

#[test]
#[ignore = "requires real root-owned protected runtime; diagnostic negative, not secure-helper qualification"]
fn protected_quarantine_retains_runtime_backing_after_lease_drop() {
    run_domain("protected");
}

#[test]
fn attempt_token_is_not_send() {
    // Ambiguous only if AttemptV1 implements Send; no privacy-based false positive.
    trait Ambiguous<A> {
        fn probe() {}
    }
    impl<T: ?Sized> Ambiguous<()> for T {}
    struct IfSend;
    impl<T: ?Sized + Send> Ambiguous<IfSend> for T {}
    let _ = <AttemptV1 as Ambiguous<_>>::probe;
}

#[test]
#[ignore = "private diagnostic domain owner"]
fn quarantine_domain_fixture() {
    assert_eq!(unsafe { prctl(36, 1_usize, 0_usize, 0_usize, 0_usize) }, 0);
    let case = std::env::var(CASE).unwrap();
    let mut command = private_command("quarantine_driver_fixture", &case);
    command.process_group(0);
    let mut slot = DIAGNOSTIC_DOMAIN.lock().unwrap_or_else(|p| p.into_inner());
    publish_domain(&mut slot, &mut command);
    drop(command);
    let domain = slot.as_mut().expect("published driver");
    let group = domain.child.id() as i32;
    let deadline = Instant::now() + Duration::from_secs(if case == "protected" { 165 } else { 15 });
    // Catch EVERY postspawn observation failure before fatal teardown. The
    // direct child stays unreaped so its private group ID cannot be recycled.
    let observation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        domain.nonblocking().unwrap();
        loop {
            domain.drain().unwrap();
            if let Some(line) = std::str::from_utf8(&domain.out.bytes)
                .unwrap()
                .lines()
                .find_map(|line| line.strip_prefix("QUARANTINE_READY "))
            {
                return line
                    .split_whitespace()
                    .map(|s| s.parse::<i32>().unwrap())
                    .collect::<Vec<_>>();
            }
            assert!(
                Instant::now() < deadline,
                "driver did not report before deadline"
            );
            thread::sleep(POLL_INTERVAL);
        }
    }));
    // No assertions, formatting or result interpretation before the kill/drain.
    let kill_result = unsafe { kill(-group, SIGKILL) };
    let cleanup_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut status = 0;
        let pid = unsafe { waitpid(-group, &mut status, WAIT_NOHANG | WAIT_WALL) };
        if pid > 0 {
            assert!(
                !stopped(status),
                "outer owner has a nonterminal event; custody retained"
            );
            assert!(
                domain.terminals.len() < MAX_TRACEES + 3,
                "diagnostic inventory bound exceeded; custody retained"
            );
            domain.terminals.push(pid);
        } else if pid < 0 {
            match io::Error::last_os_error().raw_os_error() {
                Some(10) => break,
                Some(4) => continue,
                error => panic!("outer domain wait failed; owner retained: {error:?}"),
            }
        } else {
            assert!(
                Instant::now() < cleanup_deadline,
                "outer domain has unreaped tasks; owner retained"
            );
            thread::sleep(POLL_INTERVAL);
        }
    }
    let domain = slot.take().expect("terminal diagnostic domain");
    assert_eq!(kill_result, 0);
    let observed = observation.unwrap_or_else(|_| {
        panic!(
            "driver failed before ready, domain reaped: {} {}",
            String::from_utf8_lossy(&domain.out.bytes),
            String::from_utf8_lossy(&domain.err.bytes)
        )
    });
    assert!(domain.terminals.contains(&group));
    for pid in observed {
        assert!(
            domain.terminals.contains(&pid),
            "missing real terminal for {pid}: {:?}",
            domain.terminals
        );
    }
    println!("OUTER_TERMINALS_CONFIRMED {:?}", domain.terminals);
}
fn refuse_on_all_threads() {
    let forks = FORKS.load(Ordering::SeqCst);
    for _ in 0..8 {
        let error = AttemptV1::begin().err().expect("permanent refusal");
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Quarantined
        );
    }
    thread::spawn(|| {
        let error = AttemptV1::begin().err().expect("no cross-thread retry");
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Quarantined
        );
    })
    .join()
    .unwrap();
    assert_eq!(FORKS.load(Ordering::SeqCst), forks);
}

fn exercise(case: &str) -> Vec<i32> {
    let mode = match case {
        "deadline" | "refusal" | "thread-exit" => 1,
        "birth" => 2,
        "unwind-root" => 3,
        "attach" => 4,
        "unwind-resume" => 5,
        "unwind-birth" => 6,
        _ => panic!("unknown private fixture case"),
    };
    MODE.set(mode);
    let source = CanonicalGeneratedVerusProofInputV3::new(b"fn main() {}\n".to_vec()).unwrap();
    let sealed = SealedGeneratedProofSourceV3::create(&source).unwrap();
    let sealed_fd = sealed.file.as_raw_fd();
    let mut attempt = AttemptV1::begin().unwrap();
    attempt.run().unwrap().sealed = Some(sealed);
    let start = Instant::now();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "/bin/sleep 30; :"]).env_clear();
        seized_spawn::spawn_in(&mut attempt, command, vec![], 10, deadline)?;
        if mode == 1 {
            let pid = attempt.id() as i32;
            let status = wait_for_specific(pid, deadline)?;
            let task = attempt.run()?.tracees.get_mut(&pid).unwrap();
            task.current_stop = TraceeStop::observed(status);
            task.terminal_consumed = !stopped(status);
            assert!(stopped(status));
            return Err(reject_and_reap(
                &mut attempt.run()?.tracees,
                process_failure("fixture expired cleanup"),
            ));
        }
        supervise(
            &mut attempt,
            &[],
            tests::identity("/bin/sh"),
            tests::identity("/bin/sleep"),
            &[],
            false,
            false,
            deadline,
            4096,
        )
    }));
    if matches!(mode, 3 | 5 | 6) {
        assert!(result.is_err());
    } else {
        assert!(result.unwrap().is_err());
    }
    assert!(start.elapsed() < Duration::from_secs(7));
    drop(source);
    let mut pids = custody::inspect_retained(|run| {
        assert!(run.tracees.unresolved());
        assert!(run.tracees.len() <= MAX_TRACEES + 1);
        assert_eq!(run.sealed.as_ref().unwrap().file.as_raw_fd(), sealed_fd);
        let seals = rustix::fs::fcntl_get_seals(&run.sealed.as_ref().unwrap().file).unwrap();
        assert!(seals.contains(rustix::fs::SealFlags::SEAL | rustix::fs::SealFlags::WRITE));
        assert!(rustix::fs::fstat(run.child.stdout.as_ref().unwrap()).is_ok());
        assert!(rustix::fs::fstat(run.child.stderr.as_ref().unwrap()).is_ok());
        if matches!(mode, 2 | 6) {
            assert!(run.tracees.values().any(|task| {
                task.current_stop
                    .is_some_and(TraceeStop::unregistered_birth)
            }));
            assert!(run.tracees.values().all(|task| !task.cleanup_kill_sent));
        }
        run.tracees.pids().collect::<Vec<_>>()
    });
    if matches!(mode, 2 | 5 | 6) {
        // Observation only. The library has NOT acquired this child from procfs.
        // The independent outer domain must later confirm its actual terminal.
        pids.extend(diagnostic_children(pids[0]));
    }
    refuse_on_all_threads();
    pids.sort_unstable();
    pids.dedup();
    pids
}

fn exercise_gate() {
    let mut attempt = seized_spawn::spawn(
        Command::new("/bin/true"),
        vec![],
        5,
        Instant::now() + Duration::from_secs(3),
    )
    .unwrap();
    let busy = || {
        assert_eq!(
            AttemptV1::begin().err().unwrap().kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Busy
        );
    };
    busy();
    thread::spawn(busy).join().unwrap();
    terminate_tree(&mut attempt.run().unwrap().tracees).unwrap();
    attempt.complete().unwrap();
    busy(); // The same permit is held through its caller's eventual publication.
    drop(attempt);
    AttemptV1::begin().unwrap().complete().unwrap();
}

fn check_inherited_preflight() {
    unsafe extern "C" {
        fn _exit(status: i32) -> !;
    }
    let fork_result: i64;
    // Fixture only: the child performs the exact allocation-free process check
    // used before begin's mutex, then _exit. It never runs Rust destructors.
    unsafe {
        std::arch::asm!("syscall", inlateout("rax") 57_i64 => fork_result,
            lateout("rcx") _, lateout("r11") _);
    }
    if fork_result == 0 {
        unsafe {
            _exit(if custody::inherited_process_refused() {
                0
            } else {
                1
            })
        }
    }
    assert!(fork_result > 0);
    let status =
        wait_for_specific(fork_result as i32, Instant::now() + Duration::from_secs(2)).unwrap();
    assert_eq!(terminal_status(status), (Some(0), None));
    refuse_on_all_threads();
}

fn exercise_protected() -> Vec<i32> {
    use super::super::super::open_retained_generated_verus_runtime_v1;
    let runtime = open_retained_generated_verus_runtime_v1(Path::new(PROTECTED_ROOT)).unwrap();
    let backing = std::sync::Arc::downgrade(&runtime.retained);
    let mut attempt = runtime.begin_attempt().unwrap();
    let source = CanonicalGeneratedVerusProofInputV3::new(b"fn main() {}\n".to_vec()).unwrap();
    MODE.set(4);
    let error = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(10),
            4096,
        )
        .err()
        .expect("injected attach and cleanup refusal");
    drop(error);
    drop(attempt);
    drop(source);
    drop(runtime);
    let pids = custody::inspect_retained(|run| {
        let admitted = backing
            .upgrade()
            .expect("real protected anchors outlive lease");
        assert!(std::sync::Arc::ptr_eq(
            &admitted,
            run.backing.as_ref().unwrap()
        ));
        assert!(run.sealed.is_some());
        assert!(run.tracees.unresolved());
        assert!(
            rustix::fs::fstat(
                admitted
                    .required_file(Path::new("dist/rust_verify"))
                    .unwrap()
            )
            .is_ok()
        );
        run.tracees.pids().collect::<Vec<_>>()
    });
    let fresh = crate::FunctionalRefinementVerusRuntimeLeaseV1::open(PROTECTED_ROOT).unwrap();
    assert!(
        fresh.begin_attempt().is_err(),
        "a genuinely admitted new public lease cannot reset custody"
    );
    refuse_on_all_threads();
    pids
}

#[test]
#[ignore = "private quarantine driver, never a standalone positive"]
fn quarantine_driver_fixture() {
    let case = std::env::var(CASE).unwrap();
    let pids = if case == "gate" {
        exercise_gate();
        Vec::new()
    } else if case == "protected" {
        exercise_protected()
    } else if case == "inherited" {
        let pids = exercise("deadline");
        check_inherited_preflight();
        pids
    } else if case == "thread-exit" {
        let pids = thread::spawn(|| exercise("thread-exit")).join().unwrap();
        refuse_on_all_threads();
        pids
    } else {
        exercise(&case)
    };
    println!(
        "\nQUARANTINE_READY {}",
        pids.iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    );
    std::io::stdout().flush().unwrap();
    // Keep the originating driver alive until independently owned domain kill;
    // no EXITKILL/tracer death is mistaken for library cleanup confirmation.
    thread::sleep(Duration::from_secs(20));
    panic!("outer diagnostic owner did not terminate the domain");
}
