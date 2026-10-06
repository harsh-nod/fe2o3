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
    // None after publication means spawn is in flight or unwound, not no child.
    child: Option<std::process::Child>,
    // No Drop cleanup: an unresolved child/domain must retain its scratch.
    scratch: Option<std::path::PathBuf>,
    out: Capture,
    err: Capture,
    terminals: Vec<i32>,
}

struct UnspawnedScratch(Option<std::path::PathBuf>);

impl UnspawnedScratch {
    fn remove(&mut self) -> io::Result<()> {
        if let Some(path) = &self.0 {
            std::fs::remove_dir_all(path)?;
        }
        self.0 = None;
        Ok(())
    }
}

impl Drop for UnspawnedScratch {
    fn drop(&mut self) {
        // Armed only before spawn or after an explicit no-live-child error.
        let _ = self.remove();
    }
}

fn publish_domain(
    slot: &mut Option<DiagnosticDomain>,
    command: &mut Command,
    mut scratch: UnspawnedScratch,
) -> io::Result<()> {
    assert!(
        slot.is_none(),
        "prior diagnostic domain remains unresolved; no retry"
    );
    let terminals = Vec::with_capacity(MAX_TRACEES + 3);
    let domain = slot.insert(DiagnosticDomain {
        child: None,
        scratch: None,
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
    // Transfer before entering spawn, so even an unknown postfork unwind retains
    // scratch and the occupied slot refuses retry. No panic is treated as no fork.
    domain.scratch = scratch.0.take();
    match crate::executor::spawn_artifact_coordinated_child(command) {
        Ok(child) => {
            // No fallible work between spawn and exact child publication.
            domain.child = Some(child);
            Ok(())
        }
        Err(error) => {
            // Command::spawn returns Err before fork or after disposing its failed
            // exec child. The private pre_exec callback cannot create descendants.
            scratch.0 = slot.take().expect("published spawn refusal").scratch;
            scratch.remove()?;
            Err(error)
        }
    }
}

impl DiagnosticDomain {
    fn nonblocking(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let child = self.child.as_ref().expect("published child");
        make_nonblocking(child.stdout.as_ref().expect("fixture stdout"))?;
        make_nonblocking(child.stderr.as_ref().expect("fixture stderr"))
    }
    fn drain(&mut self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        let child = self.child.as_mut().expect("published child");
        drain(
            child.stdout.as_mut().expect("fixture stdout"),
            &mut self.out,
            65536,
            OutputStream::Stdout,
        )?;
        drain(
            child.stderr.as_mut().expect("fixture stderr"),
            &mut self.err,
            65536,
            OutputStream::Stderr,
        )
    }
}
thread_local! {
    static MODE: Cell<u8> = const { Cell::new(0) };
    static QUERIES: Cell<usize> = const { Cell::new(0) };
    static KILL_REFUSALS: Cell<usize> = const { Cell::new(0) };
    static CLEANUP_CONTINUES: Cell<usize> = const { Cell::new(0) };
    static UNREGISTERED_CHILD: Cell<Option<i32>> = const { Cell::new(None) };
    static CREATOR_TERMINAL: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
}

pub(super) fn inject_kill_refusal() -> io::Result<()> {
    let errno = match MODE.get() {
        7 => 1, // EPERM
        8 => 3, // ESRCH is not delivery or terminal evidence.
        _ => return Ok(()),
    };
    KILL_REFUSALS.set(KILL_REFUSALS.get() + 1);
    Err(io::Error::from_raw_os_error(errno))
}

pub(super) fn record_cleanup_continue() {
    if matches!(MODE.get(), 7 | 8) {
        CLEANUP_CONTINUES.set(CLEANUP_CONTINUES.get() + 1);
    }
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
    if !matches!(mode, 2 | 6 | 9 | 10) {
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

pub(super) fn after_birth_resume(
    tree: &mut Tracees,
    pid: i32,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if MODE.get() == 5 {
        assert_eq!(diagnostic_children(pid).len(), 1);
        panic!("fixture unwind after creation resume before wait");
    }
    if !matches!(MODE.get(), 9 | 10) {
        return Ok(());
    }
    assert!(
        tree[&pid].pending_creation,
        "custody precedes the syscall step"
    );
    assert!(tree[&pid].current_stop.is_none());
    let children = diagnostic_children(pid);
    assert_eq!(children.len(), 1);
    // Diagnostic evidence only, never inserted into library custody. The outer
    // subreaper must authenticate this child's terminal after domain teardown.
    UNREGISTERED_CHILD.set(Some(children[0]));
    if MODE.get() == 10 {
        return Err(process_failure(
            "fixture refused birth after creation resume",
        ));
    }
    // Simulate an external fatal signal before consuming the birth event. This
    // deliberately violates the production isolation prerequisite; it must not
    // turn the creator's real terminal into proof that no unknown child exists.
    assert_eq!(unsafe { kill(pid, SIGKILL) }, 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(
            Instant::now() < deadline,
            "creator fatal drain exceeded bound"
        );
        if let Some(status) = stable::next_status(tree, pid)? {
            assert!(tree[&pid].pending_creation);
            if !stopped(status) {
                assert_eq!(terminal_status(status), (None, Some(SIGKILL)));
                assert!(tree[&pid].terminal_consumed);
                CREATOR_TERMINAL.set(Some((pid, status)));
                return Err(process_failure(
                    "fixture fatal creator retained an unresolved birth",
                ));
            }
            if tree[&pid]
                .current_stop
                .is_some_and(TraceeStop::unregistered_birth)
            {
                assert!(
                    event_child(pid).is_err(),
                    "negative hook withholds birth identity"
                );
            }
            // Only this negative fixture advances stops after its successful
            // real SIGKILL. No synthetic terminal, forged EXIT or positive proof.
            continue_killed_tracee(pid)?;
            tree.get_mut(&pid).unwrap().current_stop = None;
        } else {
            thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
        }
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

pub(super) fn run_domain(case: &str) {
    let mut slot = DIAGNOSTIC_DOMAIN.lock().unwrap_or_else(|p| p.into_inner());
    publish_domain(
        &mut slot,
        &mut private_command("quarantine_domain_fixture", case),
        UnspawnedScratch(None),
    )
    .unwrap();
    let domain = slot.as_mut().expect("published domain");
    let mut read_error = domain.nonblocking().err();
    let deadline = Instant::now() + Duration::from_secs(if case == "protected" { 180 } else { 25 });
    let status = loop {
        if read_error.is_none() {
            read_error = domain.drain().err();
        }
        match domain.child.as_mut().expect("published child").try_wait() {
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
    if case == "fatal-creation" {
        assert!(String::from_utf8_lossy(&domain.out.bytes).contains("CREATOR_TERMINAL_CONFIRMED"));
    }
}

#[test]
fn diagnostic_scratch_is_removed_on_no_child_spawn_refusal() {
    let scratch = UnspawnedScratch(super::spawn_lease_tests::prepare_directory(
        "lease-no-child-refusal",
    ));
    let path = scratch.0.as_ref().unwrap().clone();
    let mut command = private_command("quarantine_driver_fixture", "lease-no-child-refusal");
    // Pinned std rejects NUL arguments before I/O setup or fork. This is a real
    // spawn error with no child, not a fabricated exec or terminal observation.
    command.arg("\0");
    let mut slot = DIAGNOSTIC_DOMAIN.lock().unwrap_or_else(|p| p.into_inner());
    assert!(path.is_dir());
    let error = publish_domain(&mut slot, &mut command, scratch).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(
        slot.is_none(),
        "confirmed no-child refusal needs no custody"
    );
    assert!(
        !path.try_exists().unwrap(),
        "scratch leaked before driver spawn"
    );
}

#[test]
fn diagnostic_scratch_is_removed_on_pre_spawn_unwind() {
    let scratch = UnspawnedScratch(super::spawn_lease_tests::prepare_directory(
        "lease-pre-spawn-unwind",
    ));
    let path = scratch.0.as_ref().unwrap().clone();
    let result = std::panic::catch_unwind(move || {
        let _scratch = scratch;
        panic!("fixture setup failed before entering spawn");
    });
    assert!(result.is_err());
    assert!(!path.try_exists().unwrap(), "scratch leaked during setup");
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
fn cleanup_kill_refusal_never_resumes_a_seized_nonsignal_stop() {
    for case in ["kill-eperm", "kill-esrch"] {
        run_domain(case);
    }
}

#[test]
fn pending_creation_survives_actual_creator_terminal_and_refused_birth() {
    for case in ["fatal-creation", "refused-resume"] {
        run_domain(case);
    }
}

#[test]
fn pending_creation_state_cannot_be_removed_or_overwritten_after_terminal() {
    // State invariant only: these integers are not OS wait evidence and never
    // enter an Attempt, cleanup, ptrace or proof-admission path.
    let mut tree = Tracees::new().unwrap();
    let mut task = Tracee::pending(TraceeRole::Verifier, 101, true);
    task.pending_creation = true;
    task.terminal_consumed = true;
    tree.insert(101, task).unwrap();
    assert!(tree.unresolved());
    assert!(tree.remove_terminal(&101).is_err());
    assert!(tree[&101].pending_creation);
    assert!(tree[&101].terminal_consumed);
    assert!(
        stable::next_status(&mut tree, 101)
            .unwrap_err()
            .to_string()
            .contains("cannot wait twice for a terminal proof task")
    );
    assert!(
        tree.insert(101, Tracee::pending(TraceeRole::Solver, 101, true))
            .is_err()
    );
    assert!(tree.has_uncertain());
    assert!(tree[&101].pending_creation);
    assert!(tree[&101].terminal_consumed);
    assert!(tree[&101].current_stop.is_none());
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
    let scratch = UnspawnedScratch(super::spawn_lease_tests::prepare_directory(&case));
    if let Some(path) = &scratch.0 {
        command.env(super::spawn_lease_tests::DIRECTORY_ENV, path);
    }
    command.process_group(0);
    let mut slot = DIAGNOSTIC_DOMAIN.lock().unwrap_or_else(|p| p.into_inner());
    publish_domain(&mut slot, &mut command, scratch).unwrap();
    drop(command);
    let domain = slot.as_mut().expect("published driver");
    let group = domain.child.as_ref().expect("published child").id() as i32;
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
    if let Some(path) = &domain.scratch {
        // The exact owned domain is terminal, including any blocked lock releaser.
        std::fs::remove_dir_all(path).unwrap();
    }
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
    if case == "fatal-creation" {
        let output = String::from_utf8_lossy(&domain.out.bytes);
        let terminal = output
            .lines()
            .find(|line| line.starts_with("CREATOR_TERMINAL_CONFIRMED "))
            .expect("driver must confirm the creator's real terminal separately");
        println!("{terminal}");
    }
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

pub(super) fn exercise(case: &str) -> Vec<i32> {
    let mode = match case {
        "deadline" | "refusal" | "thread-exit" => 1,
        "birth" => 2,
        "unwind-root" => 3,
        "attach" => 4,
        "unwind-resume" => 5,
        "unwind-birth" => 6,
        "fatal-creation" => 9,
        "refused-resume" => 10,
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
        let error = result
            .unwrap()
            .err()
            .expect("negative fixture must refuse execution");
        if mode == 9 {
            assert!(
                error
                    .to_string()
                    .contains("fixture fatal creator retained an unresolved birth")
            );
        } else if mode == 10 {
            assert!(
                error
                    .to_string()
                    .contains("fixture refused birth after creation resume")
            );
        }
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
        if matches!(mode, 2 | 6 | 10) {
            assert!(run.tracees.values().any(|task| {
                task.pending_creation
                    && task
                        .current_stop
                        .is_some_and(TraceeStop::unregistered_birth)
            }));
            assert!(run.tracees.values().all(|task| !task.cleanup_kill_sent));
        }
        if mode == 9 {
            let (pid, status) = CREATOR_TERMINAL.get().expect("actual terminal observation");
            let task = &run.tracees[&pid];
            assert!(task.terminal_consumed);
            assert!(task.current_stop.is_none());
            assert!(task.pending_creation);
            assert!(!task.cleanup_kill_sent);
            assert_eq!(terminal_status(status), (None, Some(SIGKILL)));
            println!("\nCREATOR_TERMINAL_CONFIRMED {pid} {status}");
        }
        run.tracees
            .iter()
            .filter_map(|(&pid, task)| (!task.terminal_consumed).then_some(pid))
            .collect::<Vec<_>>()
    });
    if matches!(mode, 2 | 5 | 6) {
        // Observation only. The library has NOT acquired this child from procfs.
        // The independent outer domain must later confirm its actual terminal.
        pids.extend(diagnostic_children(pids[0]));
    }
    if matches!(mode, 9 | 10) {
        assert!(custody::inspect_retained(|run| !run
            .tracees
            .contains_key(&UNREGISTERED_CHILD.get().unwrap())));
        pids.push(
            UNREGISTERED_CHILD
                .get()
                .expect("diagnostic child, not custody"),
        );
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

fn exercise_kill_refusal(case: &str) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf USERSPACE_RESUMED"]).env_clear();
    let mut attempt = seized_spawn::spawn(command, vec![], 5, deadline).unwrap();
    let pid = attempt.id() as i32;
    let tree = &mut attempt.run().unwrap().tracees;
    let exec = wait_for_specific(pid, deadline).unwrap();
    let task = tree.get_mut(&pid).unwrap();
    task.current_stop = TraceeStop::observed(exec);
    task.terminal_consumed = !stopped(exec);
    assert!(stopped(exec) && (exec as u32) >> 16 == PTRACE_EVENT_EXEC);

    // Queue a real INTERRUPT behind the initial EXEC stop, then consume its
    // actual kernel EVENT_STOP. No synthesized status or admission bypass.
    ptrace(PTRACE_INTERRUPT, pid, 0).unwrap();
    resume_tracee(tree, pid, 0).unwrap();
    let status = wait_for_specific(pid, deadline).unwrap();
    let task = tree.get_mut(&pid).unwrap();
    task.current_stop = TraceeStop::observed(status);
    task.terminal_consumed = !stopped(status);
    assert!(stopped(status));
    assert_eq!((status as u32) >> 16, PTRACE_EVENT_STOP);
    assert_eq!(stop_signal(status), SIGTRAP);
    let registers = read_registers(pid).unwrap();
    make_nonblocking(attempt.run().unwrap().child.stdout.as_ref().unwrap()).unwrap();

    // Only kill delivery is refused. CONT is observed, never intercepted, and
    // cleanup uses its real 500ms bound rather than an injected deadline.
    MODE.set(if case == "kill-eperm" { 7 } else { 8 });
    let start = Instant::now();
    let error = terminate_tree(&mut attempt.run().unwrap().tracees).unwrap_err();
    assert!(start.elapsed() >= CLEANUP_TIMEOUT);
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(error.to_string().contains("kill PID"));
    assert!(attempt.complete().is_err());
    drop(error);
    drop(attempt);
    assert_eq!(KILL_REFUSALS.get(), 1);
    assert_eq!(CLEANUP_CONTINUES.get(), 0);
    custody::inspect_retained(|run| {
        run.check_thread().unwrap();
        assert!(run.tracees.unresolved());
        let task = &run.tracees[&pid];
        assert_eq!(task.current_stop.unwrap().status, status);
        assert!(!task.terminal_consumed);
        assert!(!task.cleanup_kill_sent);
        assert!(task.exit_boundary.is_none());
        // GETREGS proves the real task is still ptrace-stopped; unchanged
        // registers and zero CONT observations exclude a release/re-stop cycle.
        assert_eq!(read_registers(pid).unwrap().rip, registers.rip);
        assert_eq!(read_registers(pid).unwrap().rsp, registers.rsp);
        let mut pipe = run.child.stdout.as_ref().unwrap();
        let mut output = Capture {
            bytes: Vec::new(),
            eof: false,
        };
        drain(&mut pipe, &mut output, 4096, OutputStream::Stdout).unwrap();
        assert!(output.bytes.is_empty());
        assert!(!output.eof);
    });
    refuse_on_all_threads();
    // The diagnostic domain owner, not this quarantined tracer, kills the
    // still-stopped child and confirms its actual terminal wait after READY.
    vec![pid]
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
    let pids = if case.starts_with("lease-") {
        super::spawn_lease_tests::exercise(&case)
    } else if case == "gate" {
        exercise_gate();
        Vec::new()
    } else if matches!(case.as_str(), "kill-eperm" | "kill-esrch") {
        exercise_kill_refusal(&case)
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
