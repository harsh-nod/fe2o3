//! A stopped census, one kernel operation, its exit, then release. No userspace
//! sharer runs between inspecting an FD/maps/clone argument and consuming it.
//! External writers still require the protected deployment's isolation boundary.

use super::*;
use std::collections::BTreeSet;

const STABLE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_COMPLETION_STOPS: usize = 128;
type Result<T> = std::result::Result<T, RetainedFunctionalRefinementRuntimeErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Birth {
    Thread,
    Fork,
    Vfork,
}

pub(super) fn next_status(tree: &mut BTreeMap<i32, Tracee>, pid: i32) -> Result<Option<i32>> {
    let task = tree
        .get_mut(&pid)
        .ok_or_else(|| process_failure("wait for unknown proof task"))?;
    if let Some(status) = task.queued_status.take() {
        return Ok(Some(status));
    }
    let status = wait_for_specific_nonblocking(pid)?;
    if let Some(status) = status {
        task.current_stop = TraceeStop::observed(status);
    }
    Ok(status)
}

fn checkpoint(deadline: Instant, progress: &mut impl FnMut() -> Result<()>) -> Result<()> {
    progress()?;
    if Instant::now() >= deadline {
        return Err(controller_error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
            "stable proof syscall boundary exceeded its deadline",
        ));
    }
    Ok(())
}

fn remember(tree: &mut BTreeMap<i32, Tracee>, pid: i32, status: i32) -> Result<()> {
    let task = tree
        .get_mut(&pid)
        .ok_or_else(|| process_failure("unknown parked task"))?;
    task.current_stop = TraceeStop::observed(status);
    if task.queued_status.replace(status).is_some() {
        return Err(process_failure("overwritten parked task event"));
    }
    Ok(())
}

fn parked(task: &Tracee) -> bool {
    task.current_stop.is_some() || task.queued_status.is_some_and(|s| !stopped(s))
}

fn park_all(
    tree: &mut BTreeMap<i32, Tracee>,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    for pid in tree.keys().copied().collect::<Vec<_>>() {
        if parked(&tree[&pid]) {
            continue;
        }
        // Do not interrupt an already queued stop unnecessarily. An event racing
        // INTERRUPT is retained, never replaced by an assumed synthetic stop.
        if let Some(status) = wait_for_specific_nonblocking(pid)? {
            remember(tree, pid, status)?;
        } else {
            ptrace(PTRACE_INTERRUPT, pid, 0)?;
        }
    }
    while tree.values().any(|task| !parked(task)) {
        checkpoint(deadline, progress)?;
        for pid in tree.keys().copied().collect::<Vec<_>>() {
            if !parked(&tree[&pid])
                && let Some(status) = wait_for_specific_nonblocking(pid)?
            {
                remember(tree, pid, status)?;
            }
        }
        if tree.values().any(|task| !parked(task)) {
            thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
        }
    }
    // Creation can only occur in complete_request, where its child is registered
    // before either side is resumed. Anything else is missing lifecycle custody.
    if tree.values().any(|t| {
        t.queued_status.is_some_and(|s| {
            matches!(
                (s as u32) >> 16,
                PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE | PTRACE_EVENT_EXIT
            )
        })
    }) {
        return Err(process_failure(
            "creation or exit escaped the stable syscall boundary",
        ));
    }
    census(tree)
}

pub(super) fn park_for_inspection(
    tree: &mut BTreeMap<i32, Tracee>,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<Instant> {
    let deadline = deadline.min(Instant::now() + STABLE_TIMEOUT);
    park_all(tree, deadline, progress)?;
    checkpoint(deadline, progress)?;
    Ok(deadline)
}

pub(super) fn check_before_release(
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    checkpoint(deadline, progress)
}

fn census(tree: &BTreeMap<i32, Tracee>) -> Result<()> {
    let mut groups = BTreeSet::new();
    for (&pid, task) in tree {
        if task.current_stop.is_none() || !groups.insert(task.thread_group) {
            continue;
        }
        let mut count = 0;
        for entry in std::fs::read_dir(format!("/proc/{pid}/task"))
            .map_err(|_| io_process_failure("enumerate parked proof tasks"))?
        {
            count += 1;
            if count > MAX_TRACEES {
                return Err(process_failure("parked proof census exceeds task bound"));
            }
            let entry = entry.map_err(|_| io_process_failure("read parked proof task"))?;
            let tid = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
                .ok_or_else(|| process_failure("noncanonical parked task identity"))?;
            if !tree
                .get(&tid)
                .is_some_and(|t| parked(t) && t.thread_group == task.thread_group)
            {
                return Err(process_failure("unaccounted sharer in parked proof census"));
            }
        }
    }
    Ok(())
}

pub(super) fn release_interrupts(
    tree: &mut BTreeMap<i32, Tracee>,
    deadline: Instant,
) -> Result<()> {
    for pid in tree.keys().copied().collect::<Vec<_>>() {
        if tree[&pid].queued_status.is_some_and(|s| {
            stopped(s) && (s as u32) >> 16 == PTRACE_EVENT_STOP && stop_signal(s) == SIGTRAP
        }) {
            checkpoint(deadline, &mut || Ok(()))?;
            tree.get_mut(&pid).expect("retained task").queued_status = None;
            resume_tracee(tree, pid, 0)?;
        }
    }
    Ok(())
}

pub(super) fn birth_request(pid: i32, r: &UserRegistersX86_64) -> Result<Option<Birth>> {
    let birth = match r.orig_rax as u32 {
        57 => Birth::Fork,
        58 => Birth::Vfork,
        CLONE_SYSCALL => match r.rdi {
            SIGCHLD | 0x0120_0011 => Birth::Fork, // glibc fork's child-tid bookkeeping
            0x4111 => Birth::Vfork,               // CLONE_VM | CLONE_VFORK | SIGCHLD
            RUST_THREAD_CLONE3_FLAGS if r.rsi != 0 && r.rdx != 0 && r.r10 != 0 && r.r8 != 0 => {
                Birth::Thread
            }
            _ => {
                return Err(process_failure(
                    "clone flags permit unsupported sharing or ABI",
                ));
            }
        },
        CLONE3_SYSCALL => {
            validate_clone3_request(pid, r)?;
            let memory = File::open(format!("/proc/{pid}/mem"))
                .map_err(|_| io_process_failure("open stable clone flags"))?;
            let mut bytes = [0; 8];
            if rustix::io::pread(&memory, &mut bytes, r.rdi)
                .map_err(|e| io_error("read stable clone flags", e))?
                != bytes.len()
            {
                return Err(process_failure("truncated stable clone flags"));
            }
            if u64::from_ne_bytes(bytes) == RUST_THREAD_CLONE3_FLAGS {
                Birth::Thread
            } else {
                Birth::Vfork
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(birth))
}

fn step(tree: &mut BTreeMap<i32, Tracee>, pid: i32) -> Result<()> {
    let task = tree
        .get_mut(&pid)
        .ok_or_else(|| process_failure("unknown syscall requester"))?;
    ptrace(PTRACE_SYSCALL, pid, 0)?;
    task.current_stop = None;
    Ok(())
}

fn wait_stopped(
    tree: &mut BTreeMap<i32, Tracee>,
    pid: i32,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<i32> {
    loop {
        checkpoint(deadline, progress)?;
        if let Some(status) = next_status(tree, pid)? {
            if !stopped(status) {
                // Retain the consumed terminal status for cleanup rather than waiting twice.
                remember(tree, pid, status)?;
                return Err(process_failure(
                    "requester terminated before syscall completion",
                ));
            }
            return Ok(status);
        }
        thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
    }
}

fn completed_result(pid: i32, syscall: u64, status: i32) -> Result<i64> {
    if (status as u32) >> 16 != 0 || stop_signal(status) != (SIGTRAP | 0x80) {
        return Err(process_failure(
            "sensitive syscall lacks an exact exit stop",
        ));
    }
    // Kernel-provided op=EXIT distinguishes a real completed operation from an
    // entry stop, signal, seccomp stop, or interrupted/restarted operation.
    let mut info = [0_u8; 88];
    // SAFETY: GET_SYSCALL_INFO writes at most the supplied buffer size.
    let bytes = unsafe {
        linux_ptrace(
            PTRACE_GET_SYSCALL_INFO,
            pid,
            info.len() as *mut c_void,
            info.as_mut_ptr().cast(),
        )
    };
    if bytes < 33
        || info[0] != 2
        || u32::from_ne_bytes(info[4..8].try_into().unwrap()) != AUDIT_ARCH_X86_64
    {
        return Err(process_failure(
            "kernel did not authenticate a syscall-exit boundary",
        ));
    }
    let result = i64::from_ne_bytes(info[24..32].try_into().unwrap());
    let registers = read_registers(pid)?;
    if registers.orig_rax != syscall
        || registers.rax as i64 != result
        || matches!(result, -4 | -512 | -513 | -514 | -516)
    {
        return Err(process_failure(
            "sensitive syscall completion changed or requires restart",
        ));
    }
    Ok(result)
}

fn register_child(
    tree: &mut BTreeMap<i32, Tracee>,
    parent: i32,
    birth: Birth,
    event: u32,
    created: &mut usize,
    expected: usize,
) -> Result<i32> {
    let pid = event_child(parent)?;
    let parent_task = tree[&parent];
    let exceeded = tree.len() >= MAX_TRACEES;
    if tree.contains_key(&pid) {
        // A previous terminal may still be queued for the outer result census.
        // On PID reuse this *new* child needs cleanup custody, not the old reaped
        // record (which cleanup intentionally excludes from signalling).
        if tree[&pid].queued_status.is_some_and(|s| !stopped(s)) {
            tree.insert(
                pid,
                Tracee {
                    role: TraceeRole::PendingExecutable,
                    thread_group: pid,
                    leader: true,
                    exit_boundary: None,
                    current_stop: None,
                    queued_status: None,
                },
            );
        }
        tree.get_mut(&parent)
            .expect("retained parent")
            .current_stop
            .as_mut()
            .expect("consumed birth stop")
            .birth_registered = true;
        return Err(process_failure("duplicate proof descendant identity"));
    }
    // Register before any fallible inspection, including admission failures.
    tree.insert(
        pid,
        Tracee {
            role: TraceeRole::PendingExecutable,
            thread_group: pid,
            leader: true,
            exit_boundary: None,
            current_stop: None,
            queued_status: None,
        },
    );
    tree.get_mut(&parent)
        .expect("retained parent")
        .current_stop
        .as_mut()
        .expect("consumed birth stop")
        .birth_registered = true;
    let group = thread_group_id(pid)?;
    let thread = group == parent_task.thread_group;
    if thread != (birth == Birth::Thread)
        || (event == PTRACE_EVENT_VFORK) != (birth == Birth::Vfork)
    {
        return Err(process_failure(
            "kernel child sharing differs from admitted clone",
        ));
    }
    let task = tree.get_mut(&pid).expect("registered descendant");
    task.thread_group = group;
    task.leader = !thread;
    if thread {
        task.role = parent_task.role;
    } else {
        #[cfg(test)]
        {
            let _ =
                FIRST_TEST_DESCENDANT.compare_exchange(0, pid, Ordering::SeqCst, Ordering::SeqCst);
            LAST_TEST_DESCENDANT.store(pid, Ordering::SeqCst);
        }
        *created = created
            .checked_add(1)
            .ok_or_else(|| process_failure("proof descendant counter overflow"))?;
        if parent_task.role != TraceeRole::Verifier || *created > expected {
            return Err(process_failure(
                "rust_verify created an additional or nested descendant, including sequential creation",
            ));
        }
    }
    if exceeded {
        return Err(process_failure(
            "proof process tree exceeded its exact tracee bound",
        ));
    }
    Ok(pid)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn complete_request(
    tree: &mut BTreeMap<i32, Tracee>,
    pid: i32,
    allowed: &[AllowedRuntimeExecutableV1],
    validate_maps: bool,
    created: &mut usize,
    expected: usize,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let initial = read_registers(pid)?;
    // Only scalar, immutable register decisions bypass the stop-the-tree boundary.
    if matches!(
        initial.orig_rax as u32,
        OPEN_SYSCALL | OPENAT_SYSCALL | PRCTL_SYSCALL
    ) {
        return validate_sensitive_registers(pid, &initial, allowed, validate_maps);
    }
    let deadline = deadline.min(Instant::now() + STABLE_TIMEOUT);
    park_all(tree, deadline, progress)?;
    let registers = read_registers(pid)?;
    if matches!(registers.orig_rax, 60 | 231) {
        if tree[&pid].role == TraceeRole::PendingExecutable {
            return Err(process_failure(
                "proof descendant exited before an admitted exec",
            ));
        }
        // Linux does not reap a group leader until its subthreads have exited.
        // Parking those siblings while awaiting leader-only exit would deadlock;
        // supporting that ABI needs a distinct retained zombie lifecycle.
        if registers.orig_rax == 60
            && tree[&pid].leader
            && tree.iter().any(|(&other, task)| {
                other != pid
                    && task.thread_group == tree[&pid].thread_group
                    && !task.queued_status.is_some_and(|s| !stopped(s))
            })
        {
            return Err(process_failure(
                "unsupported leader-only SYS_exit with live siblings",
            ));
        }
        if validate_maps {
            validate_executable_mappings(pid, allowed)?;
        }
        return complete_exit(
            tree,
            pid,
            registers.orig_rax == 231,
            ((registers.rdi & 0xff) << 8) as i32,
            deadline,
            progress,
        );
    }
    let birth = birth_request(pid, &registers)?;
    if birth.is_some() {
        if tree[&pid].role == TraceeRole::PendingExecutable {
            return Err(process_failure(
                "unexecuted proof child created a descendant",
            ));
        }
    } else {
        validate_sensitive_registers(pid, &registers, allowed, validate_maps)?;
    }
    #[cfg(test)]
    super::stable_tests::probe(pid, &registers, false)?;
    checkpoint(deadline, progress)?;
    step(tree, pid)?;
    #[cfg(test)]
    if birth.is_some() {
        super::stable_tests::abort_after_birth_resume(pid)?;
    }
    let mut child = None;
    for _ in 0..MAX_COMPLETION_STOPS {
        let status = wait_stopped(tree, pid, deadline, progress)?;
        match (status as u32) >> 16 {
            PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                let kind =
                    birth.ok_or_else(|| process_failure("non-creation syscall created a task"))?;
                if child.is_some() {
                    return Err(process_failure(
                        "one creation syscall reported multiple children",
                    ));
                }
                let new =
                    register_child(tree, pid, kind, (status as u32) >> 16, created, expected)?;
                child = Some(new);
                let first = wait_stopped(tree, new, deadline, progress)?;
                if (first as u32) >> 16 != PTRACE_EVENT_STOP || stop_signal(first) != SIGTRAP {
                    return Err(process_failure(
                        "new proof child missed its initial seized stop",
                    ));
                }
                if kind == Birth::Vfork {
                    // Parent remains at VFORK stop. Only its child runs until exec
                    // destroys their VM sharing; then hold that EXEC stop too.
                    run_vfork_to_exec(
                        tree,
                        new,
                        allowed,
                        validate_maps,
                        created,
                        expected,
                        deadline,
                        progress,
                    )?;
                } else {
                    remember(tree, new, first)?;
                }
                census(tree)?;
                step(tree, pid)?;
            }
            PTRACE_EVENT_STOP if stop_signal(status) == SIGTRAP => step(tree, pid)?,
            0 => {
                let result = completed_result(pid, registers.orig_rax, status)?;
                if birth.is_some()
                    && !match child {
                        Some(new) => result == i64::from(new),
                        None => (-4095..0).contains(&result),
                    }
                {
                    return Err(process_failure("clone result and observed child disagree"));
                }
                if validate_maps {
                    validate_executable_mappings(pid, allowed)?;
                }
                checkpoint(deadline, progress)?;
                #[cfg(test)]
                super::stable_tests::probe(pid, &registers, true)?;
                return Ok(());
            }
            _ => {
                return Err(process_failure(
                    "unexpected event during sensitive syscall completion",
                ));
            }
        }
    }
    Err(process_failure(
        "sensitive syscall exceeded its completion event bound",
    ))
}

#[allow(clippy::too_many_arguments)]
fn run_vfork_to_exec(
    tree: &mut BTreeMap<i32, Tracee>,
    child: i32,
    allowed: &[AllowedRuntimeExecutableV1],
    validate_maps: bool,
    created: &mut usize,
    expected: usize,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    resume_tracee(tree, child, 0)?;
    for _ in 0..MAX_COMPLETION_STOPS {
        let status = wait_stopped(tree, child, deadline, progress)?;
        match (status as u32) >> 16 {
            PTRACE_EVENT_EXEC => {
                remember(tree, child, status)?;
                return Ok(());
            }
            PTRACE_EVENT_SECCOMP => {
                complete_request(
                    tree,
                    child,
                    allowed,
                    validate_maps,
                    created,
                    expected,
                    deadline,
                    progress,
                )?;
                resume_tracee(tree, child, 0)?;
            }
            _ => {
                return Err(process_failure(
                    "vfork child failed to reach its bounded exec boundary",
                ));
            }
        }
    }
    Err(process_failure(
        "vfork child exceeded its pre-exec event bound",
    ))
}

fn complete_exit(
    tree: &mut BTreeMap<i32, Tracee>,
    pid: i32,
    group_exit: bool,
    exit_status: i32,
    deadline: Instant,
    progress: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let group = tree[&pid].thread_group;
    let members = tree
        .iter()
        .filter_map(|(&tid, task)| {
            (tid == pid || (group_exit && task.thread_group == group)).then_some(tid)
        })
        .collect::<Vec<_>>();
    checkpoint(deadline, progress)?;
    resume_tracee(tree, pid, 0)?;
    let mut group_dying = false;
    let mut stops = 0;
    loop {
        checkpoint(deadline, progress)?;
        let mut complete = true;
        for &tid in &members {
            if tree[&tid].queued_status.is_some_and(|s| !stopped(s)) {
                continue;
            }
            complete = false;
            if tid != pid && !group_dying {
                continue;
            }
            if let Some(status) = next_status(tree, tid)? {
                if !stopped(status) {
                    remember(tree, tid, status)?;
                    if status != exit_status
                        || !tree[&tid]
                            .exit_boundary
                            .is_some_and(|exit| exit.matches(status))
                    {
                        return Err(process_failure(format!(
                            "exiting task skipped its authenticated exit checkpoint: requester={pid} member={tid} group={group} group_exit={group_exit} group_dying={group_dying} terminal={:?} task={:?}",
                            terminal_status(status),
                            tree[&tid]
                        )));
                    }
                    #[cfg(test)]
                    super::stable_tests::record_exit_terminal(
                        tid,
                        status,
                        tree[&tid].exit_boundary.expect("checked exit boundary"),
                    );
                    continue;
                }
                stops += 1;
                if stops > MAX_COMPLETION_STOPS {
                    return Err(process_failure("exit boundary exceeded its event bound"));
                }
                match (status as u32) >> 16 {
                    PTRACE_EVENT_EXIT => {
                        if event_message(tid, "read proof exit status")? != exit_status as usize {
                            return Err(process_failure(
                                "proof EXIT status differs from its admitted exit syscall",
                            ));
                        }
                        tree.get_mut(&tid)
                            .expect("retained exiting task")
                            .exit_boundary = Some(ExitBoundary::Task(exit_status));
                        if tid == pid {
                            group_dying = true;
                            if group_exit {
                                // The requester's checked EXIT follows do_group_exit:
                                // SIGNAL_GROUP_EXIT is set and no sibling can return
                                // to userspace. SIGKILL can suppress a sibling EXIT
                                // stop; retain that distinct group boundary instead.
                                for &member in &members {
                                    let task =
                                        tree.get_mut(&member).expect("retained group member");
                                    if task.exit_boundary.is_none()
                                        && !task.queued_status.is_some_and(|s| !stopped(s))
                                    {
                                        task.exit_boundary = Some(ExitBoundary::Group(exit_status));
                                    }
                                }
                            }
                        }
                    }
                    // Once the group-exit requester reached EXIT, Linux has set
                    // SIGNAL_GROUP_EXIT and sent fatal signals to every sibling.
                    // Consume their previously parked stops; none can return to
                    // userspace. No mutable-state decision runs during this drain.
                    PTRACE_EVENT_STOP | PTRACE_EVENT_SECCOMP
                        if tid != pid && group_exit && group_dying => {}
                    _ => {
                        return Err(process_failure(
                            "unexpected event while draining proof task exit",
                        ));
                    }
                }
                resume_tracee(tree, tid, 0)?;
                #[cfg(test)]
                if group_exit && tree[&tid].leader && (status as u32) >> 16 == PTRACE_EVENT_EXIT {
                    super::stable_tests::abort_after_group_leader_exit_resume(tree, tid)?;
                }
            }
        }
        if complete {
            return Ok(());
        }
        thread::sleep(ACTIVE_TREE_POLL_INTERVAL);
    }
}
