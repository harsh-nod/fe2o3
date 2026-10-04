//! Descendant-aware Linux controller for workload-neutral functional-refinement proofs.

use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
use std::sync::atomic::{AtomicI32, Ordering};

use rustix::fs::OFlags;

use super::{
    ADDRESS_SPACE_LIMIT_V2, CORE_LIMIT_V2, CanonicalGeneratedVerusProofInputV3, DIST_DIRECTORY_FD,
};
use super::{
    DATA_LIMIT_V2, FILE_LIMIT_V2, GENERATED_PROOF_SOURCE_FD, ObjectIdentityV2, ObjectSnapshotV2,
    RUST_VERIFY_FD, RetainedFunctionalRefinementRuntimeErrorKindV1,
    RetainedFunctionalRefinementRuntimeErrorV1, RetainedFunctionalRefinementRuntimeOutputV1,
    RetainedRuntimeClosureV2, SYSTEM_LIB_DIRECTORY_FD, SealedGeneratedProofSourceV3,
    TOOLCHAIN_DIRECTORY_FD, TOOLCHAIN_LIB_DIRECTORY_FD, Z3_FD,
};

const RLIMIT_CPU: i32 = 0;
const RLIMIT_NPROC: i32 = 6;
const RLIMIT_NOFILE: i32 = 7;
const RLIMIT_FSIZE: i32 = 1;
const RLIMIT_DATA: i32 = 2;
const RLIMIT_CORE: i32 = 4;
const RLIMIT_AS: i32 = 9;
const CPU_LIMIT_MAX_SECONDS: u64 = 601;
// RLIMIT_NPROC is charged to the real UID across the host, including unrelated
// threads. Ptrace below separately bounds and authenticates proof descendants.
const PROCESS_LIMIT: u64 = 4096;
const DESCRIPTOR_LIMIT: u64 = 256;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const ACTIVE_TREE_POLL_INTERVAL: Duration = Duration::from_micros(100);
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);

const PTRACE_CONT: u32 = 7;
const PTRACE_SYSCALL: u32 = 24;
const PTRACE_SEIZE: u32 = 0x4206;
const PTRACE_INTERRUPT: u32 = 0x4207;
const PTRACE_GET_SYSCALL_INFO: u32 = 0x420e;
const PTRACE_GETREGS: u32 = 12;
const PTRACE_SETOPTIONS: u32 = 0x4200;
const PTRACE_GETEVENTMSG: u32 = 0x4201;
const PTRACE_O_TRACESYSGOOD: usize = 1;
const PTRACE_O_TRACEFORK: usize = 0x0000_0002;
const PTRACE_O_TRACEVFORK: usize = 0x0000_0004;
const PTRACE_O_TRACECLONE: usize = 0x0000_0008;
const PTRACE_O_TRACEEXEC: usize = 0x0000_0010;
const PTRACE_O_TRACEEXIT: usize = 0x0000_0040;
const PTRACE_O_TRACESECCOMP: usize = 0x0000_0080;
const PTRACE_O_EXITKILL: usize = 0x0010_0000;
const PTRACE_EVENT_FORK: u32 = 1;
const PTRACE_EVENT_VFORK: u32 = 2;
const PTRACE_EVENT_CLONE: u32 = 3;
const PTRACE_EVENT_EXEC: u32 = 4;
const PTRACE_EVENT_EXIT: u32 = 6;
const PTRACE_EVENT_SECCOMP: u32 = 7;
const PTRACE_EVENT_STOP: u32 = 128;
const WAIT_NOHANG: i32 = 1;
const WAIT_WALL: i32 = 0x4000_0000;
const SIGKILL: i32 = 9;
const SIGSTOP: i32 = 19;
const SIGTRAP: i32 = 5;

const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
const F_SETFD: i32 = 2;
const FD_CLOEXEC: i32 = 1;
const PR_SET_NO_NEW_PRIVS: i32 = 38;
const PR_SET_SECCOMP: i32 = 22;
const SECCOMP_MODE_FILTER: usize = 2;
const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
const X32_SYSCALL_BIT: u32 = 0x4000_0000;
const BPF_LOAD_WORD_ABSOLUTE: u16 = 0x20;
const BPF_ALU_AND: u16 = 0x54;
const BPF_JUMP_EQUAL: u16 = 0x15;
const BPF_JUMP_GREATER_EQUAL: u16 = 0x35;
const BPF_RETURN: u16 = 0x06;
const SECCOMP_RETURN_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RETURN_TRACE: u32 = 0x7ff0_0000;
const SECCOMP_RETURN_ALLOW: u32 = 0x7fff_0000;
const CLONE_SYSCALL: u32 = 56;
const CLONE_ESCAPE_FLAGS: u32 = 0x7e82_0080;
const MMAP_SYSCALL: u32 = 9;
const MPROTECT_SYSCALL: u32 = 10;
const MREMAP_SYSCALL: u32 = 25;
const REMAP_FILE_PAGES_SYSCALL: u32 = 216;
const PKEY_MPROTECT_SYSCALL: u32 = 329;
const OPEN_SYSCALL: u32 = 2;
const OPENAT_SYSCALL: u32 = 257;
const PROT_WRITE: u64 = 2;
const PROT_EXEC: u64 = 4;
const MAP_ANONYMOUS: u64 = 0x20;
const READ_IMPLIES_EXEC: u32 = 0x0040_0000;
// x86-64 O_ACCMODE, O_CREAT, O_TRUNC and __O_TMPFILE; O_DIRECTORY stays allowed.
const WRITABLE_OPEN_FLAGS: u64 = 3 | 0x40 | 0x200 | 0x0040_0000;
const CLONE3_SYSCALL: u32 = 435;
const CLONE3_ARGUMENT_BYTES: u64 = 88;
const RUST_THREAD_CLONE3_FLAGS: u64 = 0x003d_0f00;
const RUST_PROCESS_CLONE3_FLAGS: u64 = 0x0000_0001_0000_4100;
const SIGCHLD: u64 = 17;
const MAX_CLONE_STACK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TRACEES: usize = 32;
const ELF_HEADER_BYTES: usize = 64;
const ELF_PROGRAM_HEADER_BYTES: usize = 56;
const MAX_ELF_PROGRAM_HEADERS: usize = 256;
const ELF_LOAD_SEGMENT: u32 = 1;
const ELF_EXECUTABLE_FLAG: u32 = 1;
const SYSTEM_PAGE_BYTES: u64 = 4096;
const PRCTL_SYSCALL: u32 = 157;
const PR_SET_NAME: u64 = 15;
const SENSITIVE_SYSCALLS: [u32; 14] = [
    60,
    231, // exit/exit_group: drain kernel clear-tid/robust-list writes while parked
    CLONE_SYSCALL,
    57, // fork
    58, // vfork
    MMAP_SYSCALL,
    MPROTECT_SYSCALL,
    MREMAP_SYSCALL,
    REMAP_FILE_PAGES_SYSCALL,
    PKEY_MPROTECT_SYSCALL,
    CLONE3_SYSCALL,
    PRCTL_SYSCALL,
    OPEN_SYSCALL,
    OPENAT_SYSCALL,
];

// Process creation remains available only so rust_verify can create one observed Z3 child.
// Ptrace enforces cardinality. The filter kills every process-tree escape primitive.
const DENIED_SYSCALLS: [u32; 49] = [
    206, 207, 208, 209, 210, // asynchronous AIO must not write memory behind a parked task
    101, // ptrace
    85, 437, // creat and pointer-based openat2 cannot acquire writable procfs memory
    135, 323, // personality changes and userfaultfd page substitution
    105, 106, 113, 114, 117, 119, 116, 122, 123, // credentials and groups
    109, 112, // setpgid, setsid
    126, // capset; prctl is admitted only for exact Rust thread naming below
    155, 161, 165, // pivot_root, chroot, mount
    272, 308, // unshare, setns
    321, // bpf
    303, 304, // name_to_handle_at, open_by_handle_at
    30, 134, // shmat with SHM_EXEC, obsolete uselib executable mapping
    62, 129, 200, 234, 297, 424, // signal external processes or the controller
    310, 311, 312, 434, 438, 448, // cross-process memory, comparison, and pidfd access
    41, 53, // network and local socket creation
    425, 426, 427, // io_uring can perform operations outside classic seccomp mediation
];

const SENSITIVE_FILTER_START: usize = 15;
const DENIED_FILTER_START: usize = SENSITIVE_FILTER_START + SENSITIVE_SYSCALLS.len() * 2;
const FILTER_LEN: usize = DENIED_FILTER_START + DENIED_SYSCALLS.len() * 2 + 1;

#[cfg(test)]
static LAST_TEST_DESCENDANT: AtomicI32 = AtomicI32::new(0);
#[cfg(test)]
static FIRST_TEST_DESCENDANT: AtomicI32 = AtomicI32::new(0);
#[cfg(test)]
std::thread_local! {
    static PEAK_TEST_EXECUTED_SOLVER_GROUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FINISHED_TEST_SOLVER_CONTEXTS: std::cell::Cell<Option<(usize, usize, usize, usize)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(super) fn finished_solver_contexts_for_test() -> Option<(usize, usize, usize, usize)> {
    FINISHED_TEST_SOLVER_CONTEXTS.get()
}

#[cfg(test)]
fn reset_solver_context_observation() {
    PEAK_TEST_EXECUTED_SOLVER_GROUPS.set(0);
    FINISHED_TEST_SOLVER_CONTEXTS.set(None);
}
#[repr(C)]
#[derive(Clone, Copy)]
struct SockFilter {
    code: u16,
    jump_true: u8,
    jump_false: u8,
    value: u32,
}

#[repr(C)]
struct SockFilterProgram {
    length: u16,
    filters: *const SockFilter,
}

#[repr(C)]
struct ResourceLimit {
    current: u64,
    maximum: u64,
}

#[repr(C)]
#[derive(Default)]
struct UserRegistersX86_64 {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rax: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    orig_rax: u64,
    rip: u64,
    cs: u64,
    eflags: u64,
    rsp: u64,
    ss: u64,
    fs_base: u64,
    gs_base: u64,
    ds: u64,
    es: u64,
    fs: u64,
    gs: u64,
}

unsafe extern "C" {
    fn close_range(first: u32, last: u32, flags: u32) -> i32;
    fn dup2(old_descriptor: i32, new_descriptor: i32) -> i32;
    fn fcntl(descriptor: i32, command: i32, ...) -> i32;
    fn getrlimit(resource: i32, limit: *mut ResourceLimit) -> i32;
    fn kill(process: i32, signal: i32) -> i32;
    #[link_name = "ptrace"]
    fn linux_ptrace(request: u32, process: i32, address: *mut c_void, data: *mut c_void) -> i64;
    fn prctl(option: i32, ...) -> i32;
    fn setrlimit(resource: i32, limit: *const ResourceLimit) -> i32;
    fn waitpid(process: i32, status: *mut i32, options: i32) -> i32;
}

#[derive(Clone, Copy)]
struct DescriptorBinding {
    source: RawFd,
    destination: RawFd,
    close_on_exec: bool,
    identity: ObjectIdentityV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceeRole {
    Verifier,
    PendingExecutable,
    AuxiliaryVerifier,
    Solver,
}

#[derive(Clone, Copy, Debug)]
struct TraceeStop {
    status: i32,
    // A registered child may already have been reaped while its vfork parent
    // remains stopped. Never interpret that old birth as fresh PID ownership.
    birth_registered: bool,
}

impl TraceeStop {
    fn observed(status: i32) -> Option<Self> {
        stopped(status).then_some(Self {
            status,
            birth_registered: false,
        })
    }

    fn unregistered_birth(self) -> bool {
        !self.birth_registered
            && stop_signal(self.status) == SIGTRAP
            && matches!(
                (self.status as u32) >> 16,
                PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE
            )
    }

    fn is_exit(self) -> bool {
        stop_signal(self.status) == SIGTRAP && (self.status as u32) >> 16 == PTRACE_EVENT_EXIT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExitBoundary {
    Task(i32),
    Group(i32),
}

impl ExitBoundary {
    fn matches(self, status: i32) -> bool {
        let expected = match self {
            Self::Task(expected) | Self::Group(expected) => expected,
        };
        !stopped(status) && status == expected
    }
}

#[derive(Clone, Copy, Debug)]
struct Tracee {
    role: TraceeRole,
    thread_group: i32,
    leader: bool,
    exit_boundary: Option<ExitBoundary>,
    cleanup_exiting: bool,
    // Exact current kernel stop, retained independently of deferred dispatch.
    // Cleanup may need GETEVENTMSG even after queued_status was consumed.
    current_stop: Option<TraceeStop>,
    queued_status: Option<i32>,
    terminal_consumed: bool,
    // Published before a creation can run. A terminal wait releases this PID,
    // not an unknown child whose birth event may have been lost to a fatal signal.
    pending_creation: bool,
    cleanup_interrupt_sent: bool,
    cleanup_kill_sent: bool,
}

impl Tracee {
    fn pending(role: TraceeRole, group: i32, leader: bool) -> Self {
        Self {
            role,
            thread_group: group,
            leader,
            exit_boundary: None,
            cleanup_exiting: false,
            current_stop: None,
            queued_status: None,
            terminal_consumed: false,
            pending_creation: false,
            cleanup_interrupt_sent: false,
            cleanup_kill_sent: false,
        }
    }
}

#[path = "functional_refinement_process_tree_v1_custody.rs"]
mod custody;
use super::super::GeneratedProofProcessPolicyV2;
pub(crate) use custody::AttemptV1;
use custody::{Run, Tracees};

#[path = "functional_refinement_solver_contexts_v2.rs"]
mod solver_contexts;
use solver_contexts::SolverContextsV2;

#[path = "functional_refinement_process_tree_v1_spawn.rs"]
mod seized_spawn;
#[cfg(test)]
#[path = "functional_refinement_process_tree_v1_spawn_lease_tests.rs"]
mod spawn_lease_tests;
#[path = "functional_refinement_process_tree_v1_stable.rs"]
mod stable;

struct Capture {
    bytes: Vec<u8>,
    eof: bool,
}

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[cfg(test)]
#[path = "functional_refinement_process_tree_v1_output_tests.rs"]
mod output_tests;

#[derive(Clone, Debug)]
pub(super) struct AllowedRuntimeExecutableV1 {
    identity: ObjectIdentityV2,
    executable_file_ranges: Vec<(u64, u64)>,
}

pub(super) fn allowed_runtime_executable(
    file: &File,
    identity: ObjectIdentityV2,
    path: &Path,
) -> Result<Option<AllowedRuntimeExecutableV1>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let retained_file_bytes = rustix::fs::fstat(file)
        .map_err(|error| io_error("inspect runtime ELF length", error))?
        .st_size;
    let retained_file_bytes = u64::try_from(retained_file_bytes)
        .map_err(|_| process_failure("runtime ELF has a negative length"))?;
    let mut header = [0_u8; ELF_HEADER_BYTES];
    let count = rustix::io::pread(file, &mut header, 0).map_err(|error| {
        io_error(
            &format!("read runtime ELF header {}", path.display()),
            error,
        )
    })?;
    if count < 7 || header[..7] != [0x7f, b'E', b'L', b'F', 2, 1, 1] {
        return Ok(None);
    }
    if count != header.len() {
        return Err(process_failure(format!(
            "runtime ELF header {} is truncated",
            path.display()
        )));
    }
    let file_type = u16::from_le_bytes(header[16..18].try_into().expect("two-byte field"));
    let machine = u16::from_le_bytes(header[18..20].try_into().expect("two-byte field"));
    if !matches!(file_type, 2 | 3) || machine != 62 {
        return Ok(None);
    }
    let program_offset = u64::from_le_bytes(header[32..40].try_into().expect("eight-byte field"));
    let program_entry_bytes =
        u16::from_le_bytes(header[54..56].try_into().expect("two-byte field")) as usize;
    let program_count =
        u16::from_le_bytes(header[56..58].try_into().expect("two-byte field")) as usize;
    if program_entry_bytes != ELF_PROGRAM_HEADER_BYTES
        || !(1..=MAX_ELF_PROGRAM_HEADERS).contains(&program_count)
    {
        return Err(process_failure(format!(
            "runtime ELF program-header table {} is outside the pinned x86-64 ABI",
            path.display()
        )));
    }
    let mut ranges = Vec::new();
    for index in 0..program_count {
        let offset = program_offset
            .checked_add((index * ELF_PROGRAM_HEADER_BYTES) as u64)
            .ok_or_else(|| process_failure("runtime ELF program-header offset overflow"))?;
        let mut program = [0_u8; ELF_PROGRAM_HEADER_BYTES];
        let count = rustix::io::pread(file, &mut program, offset)
            .map_err(|error| io_error("read runtime ELF program header", error))?;
        if count != program.len() {
            return Err(process_failure(format!(
                "runtime ELF program-header table {} is truncated",
                path.display()
            )));
        }
        let segment_type = u32::from_le_bytes(program[0..4].try_into().expect("four-byte field"));
        let flags = u32::from_le_bytes(program[4..8].try_into().expect("four-byte field"));
        if segment_type != ELF_LOAD_SEGMENT || flags & ELF_EXECUTABLE_FLAG == 0 {
            continue;
        }
        let file_offset = u64::from_le_bytes(program[8..16].try_into().expect("eight-byte field"));
        let segment_file_bytes =
            u64::from_le_bytes(program[32..40].try_into().expect("eight-byte field"));
        if segment_file_bytes == 0 {
            continue;
        }
        let segment_file_end = file_offset
            .checked_add(segment_file_bytes)
            .ok_or_else(|| process_failure("runtime ELF executable range overflow"))?;
        if segment_file_end > retained_file_bytes {
            return Err(process_failure(format!(
                "runtime ELF executable segment {} exceeds the retained file",
                path.display()
            )));
        }
        let start = file_offset & !(SYSTEM_PAGE_BYTES - 1);
        let end = segment_file_end
            .checked_add(SYSTEM_PAGE_BYTES - 1)
            .map(|value| value & !(SYSTEM_PAGE_BYTES - 1))
            .ok_or_else(|| process_failure("runtime ELF executable range overflow"))?;
        ranges.push((start, end));
    }
    if ranges.is_empty() {
        return Err(process_failure(format!(
            "runtime ELF image {} has no executable load segment",
            path.display()
        )));
    }
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, previous_end)) = merged.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
        } else {
            merged.push((start, end));
        }
    }
    Ok(Some(AllowedRuntimeExecutableV1 {
        identity,
        executable_file_ranges: merged,
    }))
}

pub(super) fn execute(
    attempt: &mut AttemptV1,
    runtime: std::sync::Arc<RetainedRuntimeClosureV2>,
    source: &CanonicalGeneratedVerusProofInputV3,
    deadline: Instant,
    output_limit: usize,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    execute_with_policy(
        attempt,
        runtime,
        source,
        deadline,
        output_limit,
        GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
    )
}

pub(super) fn execute_with_policy(
    attempt: &mut AttemptV1,
    runtime: std::sync::Arc<RetainedRuntimeClosureV2>,
    source: &CanonicalGeneratedVerusProofInputV3,
    deadline: Instant,
    output_limit: usize,
    policy: GeneratedProofProcessPolicyV2,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    #[cfg(test)]
    reset_solver_context_observation();
    crate::authenticated_verus_execution_v2::validate_controller_security_v2().map_err(
        |error| {
            controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::Process,
                format!("controller security preflight failed: {error}"),
            )
        },
    )?;
    if output_limit == 0 {
        return Err(controller_error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::OutputTooLarge,
            "functional-refinement output bound is zero",
        ));
    }
    if Instant::now() >= deadline {
        return Err(controller_error(
            RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
            "functional-refinement deadline elapsed before spawn",
        ));
    }
    let sealed = SealedGeneratedProofSourceV3::create(source)?;
    sealed.revalidate(source)?;
    let rust_verify = runtime.required_file(Path::new("dist/rust_verify"))?;
    let z3 = runtime.required_file(Path::new("dist/z3"))?;
    let dist = runtime.required_directory(Path::new("dist"))?;
    let toolchain = runtime.required_directory(Path::new("toolchain"))?;
    let toolchain_lib = runtime.required_directory(Path::new("toolchain/lib"))?;
    let system_lib = runtime.required_directory(Path::new("system-lib"))?;
    let empty = runtime.required_directory(Path::new("empty"))?;

    let sources = [
        rust_verify,
        z3,
        dist,
        toolchain,
        toolchain_lib,
        system_lib,
        &sealed.file,
    ];
    let destinations = [
        RUST_VERIFY_FD,
        Z3_FD,
        DIST_DIRECTORY_FD,
        TOOLCHAIN_DIRECTORY_FD,
        TOOLCHAIN_LIB_DIRECTORY_FD,
        SYSTEM_LIB_DIRECTORY_FD,
        GENERATED_PROOF_SOURCE_FD,
    ];
    let mut duplicates = Vec::with_capacity(sources.len());
    let mut bindings = Vec::with_capacity(sources.len());
    let mut next = 200;
    for ((file, destination), close_on_exec) in sources
        .into_iter()
        .zip(destinations)
        .zip([true, false, false, false, false, false, false])
    {
        let descriptor = rustix::io::fcntl_dupfd_cloexec(file, next)
            .map_err(|error| io_error("duplicate functional-refinement descriptor", error))?;
        next = descriptor.as_raw_fd().checked_add(1).ok_or_else(|| {
            controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::Process,
                "functional-refinement descriptor space exhausted",
            )
        })?;
        bindings.push(DescriptorBinding {
            source: descriptor.as_raw_fd(),
            destination,
            close_on_exec,
            identity: ObjectSnapshotV2::capture(file, "functional-refinement retained object")?
                .object_identity(),
        });
        duplicates.push(descriptor);
    }
    let allowed_mappings = runtime.allowed_runtime_object_identities()?;
    let cpu_seconds = deadline
        .saturating_duration_since(Instant::now())
        .as_secs()
        .saturating_add(1)
        .clamp(1, CPU_LIMIT_MAX_SECONDS);
    let mut command = Command::new(format!("/proc/self/fd/{RUST_VERIFY_FD}"));
    command
        .arg(format!("/proc/self/fd/{GENERATED_PROOF_SOURCE_FD}"))
        .args([
            "--crate-type",
            "lib",
            "--triggers-mode",
            "silent",
            "--no-cheating",
            "--num-threads",
            "1",
            "--sysroot",
        ])
        .arg(format!("/proc/self/fd/{TOOLCHAIN_DIRECTORY_FD}"))
        .env_clear()
        .env("VERUS_ROOT", format!("/proc/self/fd/{DIST_DIRECTORY_FD}"))
        .env("VERUS_Z3_PATH", format!("/proc/self/fd/{Z3_FD}"))
        .env(
            "LD_LIBRARY_PATH",
            format!(
                "/proc/self/fd/{TOOLCHAIN_LIB_DIRECTORY_FD}:/proc/self/fd/{SYSTEM_LIB_DIRECTORY_FD}"
            ),
        )
        .current_dir(format!("/proc/self/fd/{}", empty.as_raw_fd()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    {
        let run = attempt.run()?;
        run.backing = Some(runtime);
        run.sealed = Some(sealed);
        run.descriptors = duplicates;
    }
    seized_spawn::spawn_in(attempt, command, bindings.clone(), cpu_seconds, deadline)?;
    let result = supervise_with_policy(
        attempt,
        &bindings,
        bindings[0].identity,
        bindings[1].identity,
        &allowed_mappings,
        true,
        true,
        deadline,
        output_limit,
        policy,
    );
    attempt
        .run()?
        .sealed
        .as_ref()
        .expect("published sealed source")
        .revalidate(source)?;
    attempt.complete()?;
    result
}

fn prepare_child(bindings: &[DescriptorBinding], cpu_seconds: u64) -> io::Result<()> {
    // SAFETY: close_range only marks descriptors close-on-exec in this process.
    if unsafe { close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for binding in bindings {
        // SAFETY: both descriptors are live and captured before fork.
        if unsafe { dup2(binding.source, binding.destination) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fcntl updates the close-on-exec flag of the duplicated descriptor.
        if unsafe {
            fcntl(
                binding.destination,
                F_SETFD,
                if binding.close_on_exec { FD_CLOEXEC } else { 0 },
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    for (resource, value) in [
        (RLIMIT_CPU, cpu_seconds),
        (RLIMIT_NPROC, PROCESS_LIMIT),
        (RLIMIT_NOFILE, DESCRIPTOR_LIMIT),
        (RLIMIT_AS, ADDRESS_SPACE_LIMIT_V2),
        (RLIMIT_DATA, DATA_LIMIT_V2),
        (RLIMIT_FSIZE, FILE_LIMIT_V2),
        (RLIMIT_CORE, CORE_LIMIT_V2),
    ] {
        let mut inherited = ResourceLimit {
            current: 0,
            maximum: 0,
        };
        // SAFETY: getrlimit writes one initialized fixed-layout value.
        if unsafe { getrlimit(resource, &mut inherited) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let value = value.min(inherited.current).min(inherited.maximum);
        let limit = ResourceLimit {
            current: value,
            maximum: value,
        };
        // SAFETY: setrlimit reads one initialized fixed-layout value.
        if unsafe { setrlimit(resource, &limit) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: PR_SET_NO_NEW_PRIVS has no pointer argument.
    if unsafe { prctl(PR_SET_NO_NEW_PRIVS, 1_usize, 0_usize, 0_usize, 0_usize) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let filters = seccomp_filter();
    let program = SockFilterProgram {
        length: filters.len() as u16,
        filters: filters.as_ptr(),
    };
    // SAFETY: the kernel copies the complete stack-resident BPF program before returning.
    if unsafe {
        prctl(
            PR_SET_SECCOMP,
            SECCOMP_MODE_FILTER,
            (&raw const program).addr(),
            0_usize,
            0_usize,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn seccomp_filter() -> [SockFilter; FILTER_LEN] {
    let mut filters = [statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS); FILTER_LEN];
    filters[0] = statement(BPF_LOAD_WORD_ABSOLUTE, 4);
    filters[1] = jump(BPF_JUMP_EQUAL, AUDIT_ARCH_X86_64, 1, 0);
    filters[2] = statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS);
    filters[3] = statement(BPF_LOAD_WORD_ABSOLUTE, 0);
    filters[4] = jump(BPF_JUMP_GREATER_EQUAL, X32_SYSCALL_BIT, 0, 1);
    filters[5] = statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS);
    filters[6] = jump(BPF_JUMP_EQUAL, CLONE_SYSCALL, 0, 7);
    filters[7] = statement(BPF_LOAD_WORD_ABSOLUTE, 16);
    filters[8] = statement(BPF_ALU_AND, CLONE_ESCAPE_FLAGS);
    filters[9] = jump(BPF_JUMP_EQUAL, 0, 1, 0);
    filters[10] = statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS);
    filters[11] = statement(BPF_LOAD_WORD_ABSOLUTE, 20);
    filters[12] = jump(BPF_JUMP_EQUAL, 0, 1, 0);
    filters[13] = statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS);
    filters[14] = statement(BPF_LOAD_WORD_ABSOLUTE, 0);
    for (index, syscall) in SENSITIVE_SYSCALLS.into_iter().enumerate() {
        filters[SENSITIVE_FILTER_START + index * 2] = jump(BPF_JUMP_EQUAL, syscall, 0, 1);
        filters[SENSITIVE_FILTER_START + index * 2 + 1] =
            statement(BPF_RETURN, SECCOMP_RETURN_TRACE);
    }
    for (index, syscall) in DENIED_SYSCALLS.iter().copied().enumerate() {
        filters[DENIED_FILTER_START + index * 2] = jump(BPF_JUMP_EQUAL, syscall, 0, 1);
        filters[DENIED_FILTER_START + index * 2 + 1] =
            statement(BPF_RETURN, SECCOMP_RETURN_KILL_PROCESS);
    }
    filters[FILTER_LEN - 1] = statement(BPF_RETURN, SECCOMP_RETURN_ALLOW);
    filters
}

const fn statement(code: u16, value: u32) -> SockFilter {
    SockFilter {
        code,
        jump_true: 0,
        jump_false: 0,
        value,
    }
}

const fn jump(code: u16, value: u32, jump_true: u8, jump_false: u8) -> SockFilter {
    SockFilter {
        code,
        jump_true,
        jump_false,
        value,
    }
}

#[allow(clippy::too_many_arguments)]
fn supervise(
    attempt: &mut AttemptV1,
    bindings: &[DescriptorBinding],
    verifier_identity: ObjectIdentityV2,
    solver_identity: ObjectIdentityV2,
    allowed_mappings: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
    require_auxiliary_verifier: bool,
    deadline: Instant,
    output_limit: usize,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    supervise_with_policy(
        attempt,
        bindings,
        verifier_identity,
        solver_identity,
        allowed_mappings,
        validate_mappings,
        require_auxiliary_verifier,
        deadline,
        output_limit,
        GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
    )
}

#[allow(clippy::too_many_arguments)]
fn supervise_with_policy(
    attempt: &mut AttemptV1,
    bindings: &[DescriptorBinding],
    verifier_identity: ObjectIdentityV2,
    solver_identity: ObjectIdentityV2,
    allowed_mappings: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
    require_auxiliary_verifier: bool,
    deadline: Instant,
    output_limit: usize,
    policy: GeneratedProofProcessPolicyV2,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    #[cfg(test)]
    reset_solver_context_observation();
    let run = attempt.run()?;
    run.check_thread()?;
    let result = supervise_run(
        run,
        bindings,
        verifier_identity,
        solver_identity,
        allowed_mappings,
        validate_mappings,
        require_auxiliary_verifier,
        deadline,
        output_limit,
        policy,
    );
    run.release_spawn_after_terminal();
    result
}

#[allow(clippy::too_many_arguments)]
fn supervise_run(
    run: &mut Run,
    bindings: &[DescriptorBinding],
    verifier_identity: ObjectIdentityV2,
    solver_identity: ObjectIdentityV2,
    allowed_mappings: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
    require_auxiliary_verifier: bool,
    deadline: Instant,
    output_limit: usize,
    policy: GeneratedProofProcessPolicyV2,
) -> Result<RetainedFunctionalRefinementRuntimeOutputV1, RetainedFunctionalRefinementRuntimeErrorV1>
{
    let Run {
        tracees,
        child,
        spawn_lease,
        stdout_capture,
        stderr_capture,
        ..
    } = run;
    let verifier = child.id() as i32;
    let Some(stdout) = child.stdout.as_mut() else {
        return Err(reject_and_reap(
            tracees,
            process_failure("traced verifier stdout pipe is missing"),
        ));
    };
    let Some(stderr) = child.stderr.as_mut() else {
        return Err(reject_and_reap(
            tracees,
            process_failure("traced verifier stderr pipe is missing"),
        ));
    };
    if let Err(error) = make_nonblocking(stdout) {
        return Err(reject_and_reap(tracees, error));
    }
    if let Err(error) = make_nonblocking(stderr) {
        return Err(reject_and_reap(tracees, error));
    }
    let execution = (|| {
        seized_spawn::wait_initial_exec(tracees, verifier, spawn_lease, deadline)?;
        set_trace_options(verifier)?;
        validate_executable(verifier, verifier_identity, "rust_verify")?;
        validate_initial_descriptor_closure(verifier, bindings)?;
        if validate_mappings {
            validate_executable_mappings(verifier, allowed_mappings)?;
        }
        resume_tracee(tracees, verifier, 0)?;

        let mut verifier_terminal = None;
        let mut auxiliary_terminal = None;
        let mut solver_terminal = None;
        let mut process_descendants_created = 0_usize;
        let mut auxiliary_started = false;
        let mut solver_started = false;
        let mut contexts = (!policy.is_legacy())
            .then(|| SolverContextsV2::with_policy(policy, require_auxiliary_verifier));
        let expected_process_descendants = if require_auxiliary_verifier { 2 } else { 1 };
        let mut idle_interval = ACTIVE_TREE_POLL_INTERVAL;
        while !tracees.is_empty() {
            drain(stdout, stdout_capture, output_limit, OutputStream::Stdout)?;
            drain(stderr, stderr_capture, output_limit, OutputStream::Stderr)?;
            if Instant::now() >= deadline {
                return Err(controller_error(
                    RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
                    "functional-refinement process tree exceeded its global deadline",
                ));
            }
            let mut progressed = false;
            let processes = tracees.pids();
            for process in processes {
                // A stable birth checkpoint may already have authenticated and
                // retired a queued terminal from this outer census snapshot.
                if contexts.is_some() && !tracees.contains_key(&process) {
                    continue;
                }
                let Some(status) = stable::next_status(tracees, process)? else {
                    continue;
                };
                progressed = true;
                if stopped(status) {
                    let event = (status as u32) >> 16;
                    let signal = stop_signal(status);
                    let inspection_deadline = if event == PTRACE_EVENT_EXEC {
                        Some(stable::park_for_inspection(tracees, deadline, &mut || {
                            drain(stdout, stdout_capture, output_limit, OutputStream::Stdout)?;
                            drain(stderr, stderr_capture, output_limit, OutputStream::Stderr)
                        })?)
                    } else {
                        None
                    };
                    match event {
                        PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                            // All creation is admitted and completed under the stable boundary.
                            return Err(process_failure("unmediated proof descendant creation"));
                        }
                        PTRACE_EVENT_EXEC => {
                            let tracee = tracees.get(&process).copied().ok_or_else(|| {
                                process_failure("exec event came from an unknown process")
                            })?;
                            if tracee.role != TraceeRole::PendingExecutable || !tracee.leader {
                                return Err(process_failure(
                                    "rust_verify or Z3 performed an unexpected re-exec",
                                ));
                            }
                            let observed = executable_identity(process)?;
                            let role = if let Some(contexts) = &mut contexts {
                                let role = contexts.expected_exec(process)?;
                                let (identity, name) = match role {
                                    TraceeRole::AuxiliaryVerifier => {
                                        (verifier_identity, "auxiliary rust_verify")
                                    }
                                    TraceeRole::Solver => (solver_identity, "Z3"),
                                    _ => {
                                        return Err(process_failure(
                                            "unexpected admitted context role",
                                        ));
                                    }
                                };
                                if observed != identity {
                                    return Err(process_failure(
                                        "traced context executable differs from its admitted role",
                                    ));
                                }
                                validate_exact_descriptor_closure(process, bindings, name)?;
                                role
                            } else if require_auxiliary_verifier
                                && !auxiliary_started
                                && observed == verifier_identity
                            {
                                auxiliary_started = true;
                                validate_exact_descriptor_closure(
                                    process,
                                    bindings,
                                    "auxiliary rust_verify",
                                )?;
                                TraceeRole::AuxiliaryVerifier
                            } else if !solver_started
                                && (!require_auxiliary_verifier || auxiliary_started)
                                && observed == solver_identity
                            {
                                solver_started = true;
                                validate_exact_descriptor_closure(process, bindings, "Z3")?;
                                TraceeRole::Solver
                            } else {
                                return Err(process_failure(
                                    "traced descendant executable identity differs or appears out of order",
                                ));
                            };
                            tracees
                                .get_mut(&process)
                                .expect("exec tracee remains retained")
                                .role = role;
                            if validate_mappings {
                                validate_executable_mappings(process, allowed_mappings)?;
                            }
                            if let Some(contexts) = &mut contexts {
                                contexts.executed(process, role)?;
                            }
                            stable::check_before_release(
                                inspection_deadline.expect("exec inspection deadline"),
                                &mut || {
                                    drain(
                                        stdout,
                                        stdout_capture,
                                        output_limit,
                                        OutputStream::Stdout,
                                    )?;
                                    drain(
                                        stderr,
                                        stderr_capture,
                                        output_limit,
                                        OutputStream::Stderr,
                                    )
                                },
                            )?;
                            resume_tracee(tracees, process, 0)?;
                        }
                        PTRACE_EVENT_SECCOMP => {
                            let mut progress = || {
                                drain(stdout, stdout_capture, output_limit, OutputStream::Stdout)?;
                                drain(stderr, stderr_capture, output_limit, OutputStream::Stderr)
                            };
                            stable::complete_request(
                                tracees,
                                process,
                                policy,
                                allowed_mappings,
                                validate_mappings,
                                &mut process_descendants_created,
                                expected_process_descendants,
                                &mut contexts,
                                deadline,
                                &mut progress,
                            )?;
                            if tracees[&process].current_stop.is_some() {
                                resume_tracee(tracees, process, 0)?;
                            }
                            stable::release_interrupts(tracees, deadline)?;
                        }
                        PTRACE_EVENT_EXIT => {
                            return Err(process_failure(
                                "proof task exited outside its stable exit boundary",
                            ));
                        }
                        PTRACE_EVENT_STOP if signal == SIGTRAP => {
                            set_trace_options(process)?;
                            resume_tracee(tracees, process, 0)?;
                        }
                        PTRACE_EVENT_STOP | 0 if signal == SIGSTOP => {
                            return Err(process_failure("unexpected proof process group stop"));
                        }
                        0 => resume_tracee(tracees, process, signal)?,
                        _ => {
                            return Err(process_failure(
                                "unknown ptrace event in proof process tree",
                            ));
                        }
                    }
                    if let Some(inspection_deadline) = inspection_deadline {
                        stable::release_interrupts(tracees, inspection_deadline)?;
                    }
                } else {
                    if let Some(contexts) = &mut contexts
                        && matches!(
                            tracees[&process].role,
                            TraceeRole::Solver
                                | TraceeRole::AuxiliaryVerifier
                                | TraceeRole::PendingExecutable
                        )
                    {
                        contexts.retire_terminal(tracees, process, status)?;
                        continue;
                    }
                    let tracee = tracees.remove_terminal(&process)?;
                    if !tracee
                        .exit_boundary
                        .is_some_and(|exit| exit.matches(status))
                    {
                        return Err(process_failure(
                            "tracee skipped its authenticated exit checkpoint",
                        ));
                    }
                    let terminal = terminal_status(status);
                    match (tracee.role, tracee.leader) {
                        (TraceeRole::Verifier, true) => verifier_terminal = Some(terminal),
                        (TraceeRole::AuxiliaryVerifier, true) => {
                            auxiliary_terminal = Some(terminal)
                        }
                        (TraceeRole::Solver, true) => solver_terminal = Some(terminal),
                        (TraceeRole::PendingExecutable, _) => {
                            return Err(process_failure("unexecuted proof descendant terminated"));
                        }
                        _ => {}
                    }
                }
            }
            if !progressed {
                thread::sleep(
                    idle_interval.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            idle_interval = next_tree_poll_interval(idle_interval, progressed);
        }
        let verifier_terminal = verifier_terminal
            .ok_or_else(|| process_failure("verifier terminal status is missing"))?;
        if let Some(contexts) = &mut contexts {
            contexts.finish().map_err(|error| {
                context_terminal_failure(
                    error,
                    verifier_terminal,
                    &stdout_capture.bytes,
                    &stderr_capture.bytes,
                )
            })?;
            return Ok(verifier_terminal);
        }
        let solver_terminal = solver_terminal.ok_or_else(|| {
            process_failure(format!(
                "Z3 descendant was not observed; verifier={verifier_terminal:?} auxiliary={auxiliary_terminal:?} stdout={:?} stderr={:?}",
                String::from_utf8_lossy(&stdout_capture.bytes),
                String::from_utf8_lossy(&stderr_capture.bytes),
            ))
        })?;
        if require_auxiliary_verifier && auxiliary_terminal != Some((Some(0), None)) {
            return Err(process_failure(
                "auxiliary rust_verify did not exit successfully",
            ));
        }
        if solver_terminal != (Some(0), None) {
            return Err(process_failure("Z3 descendant did not exit successfully"));
        }
        Ok(verifier_terminal)
    })();
    let terminal = match execution {
        Ok(terminal) => terminal,
        Err(execution_error) => return Err(reject_and_reap(tracees, execution_error)),
    };
    drain_to_eof(
        stdout,
        stderr,
        stdout_capture,
        stderr_capture,
        output_limit,
        deadline,
    )?;
    Ok(RetainedFunctionalRefinementRuntimeOutputV1 {
        policy,
        exit_code: terminal.0,
        signal: terminal.1,
        stdout: std::mem::take(&mut stdout_capture.bytes),
        stderr: std::mem::take(&mut stderr_capture.bytes),
    })
}

fn trace_options() -> usize {
    PTRACE_O_TRACESYSGOOD
        | PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_TRACEEXIT
        | PTRACE_O_TRACESECCOMP
        | PTRACE_O_EXITKILL
}

fn set_trace_options(process: i32) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    ptrace(PTRACE_SETOPTIONS, process, trace_options())
}

fn event_child(process: i32) -> Result<i32, RetainedFunctionalRefinementRuntimeErrorV1> {
    #[cfg(test)]
    if quarantine_tests::refuse_birth_query() {
        return Err(process_failure(
            "quarantine fixture persistent birth query refusal",
        ));
    }
    #[cfg(test)]
    if stable_tests::fail_birth_query_once() {
        return Err(process_failure(
            "fixture injected birth GETEVENTMSG failure",
        ));
    }
    let child = event_message(process, "read ptrace descendant identity")?;
    let child = i32::try_from(child)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or_else(|| {
            process_failure("ptrace descendant PID is not positive and representable")
        })?;
    #[cfg(test)]
    stable_tests::record_birth_identity(child);
    Ok(child)
}

fn event_message(
    process: i32,
    operation: &'static str,
) -> Result<usize, RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut message = 0_usize;
    // SAFETY: GETEVENTMSG writes one machine word at the supplied pointer.
    if unsafe {
        linux_ptrace(
            PTRACE_GETEVENTMSG,
            process,
            std::ptr::null_mut(),
            (&raw mut message).cast(),
        )
    } < 0
    {
        return Err(io_process_failure(operation));
    }
    Ok(message)
}

fn continue_tracee(
    process: i32,
    signal: i32,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    ptrace(PTRACE_CONT, process, signal as usize)
}

fn resume_tracee(
    tracees: &mut Tracees,
    process: i32,
    signal: i32,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let tracee = tracees
        .get_mut(&process)
        .ok_or_else(|| process_failure("resumed an unknown proof process"))?;
    continue_tracee(process, signal)?;
    tracee.current_stop = None;
    Ok(())
}

fn ptrace(
    request: u32,
    process: i32,
    data: usize,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    ptrace_result(request, process, data)
        .map_err(|error| process_failure(format!("operate on traced proof process: {error}")))
}

fn ptrace_result(request: u32, process: i32, data: usize) -> io::Result<()> {
    // SAFETY: ptrace interprets null address and the scalar data according to the request.
    if unsafe { linux_ptrace(request, process, std::ptr::null_mut(), data as *mut c_void) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn wait_for_specific(
    process: i32,
    deadline: Instant,
) -> Result<i32, RetainedFunctionalRefinementRuntimeErrorV1> {
    loop {
        if let Some(status) = wait_for_specific_nonblocking(process)? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(controller_error(
                RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
                "timed out waiting for traced proof process",
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_specific_nonblocking(
    process: i32,
) -> Result<Option<i32>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut status = 0;
    // SAFETY: waitpid writes one integer status for the exact traced PID.
    let result = unsafe { waitpid(process, &mut status, WAIT_NOHANG | WAIT_WALL) };
    match result {
        0 => Ok(None),
        value if value == process => Ok(Some(status)),
        _ => Err(io_process_failure("wait for traced proof process")),
    }
}

fn stopped(status: i32) -> bool {
    status & 0xff == 0x7f
}

fn stop_signal(status: i32) -> i32 {
    (status >> 8) & 0xff
}

fn terminal_status(status: i32) -> (Option<i32>, Option<i32>) {
    if status & 0x7f == 0 {
        (Some((status >> 8) & 0xff), None)
    } else {
        (None, Some(status & 0x7f))
    }
}

fn validate_executable(
    process: i32,
    expected: ObjectIdentityV2,
    label: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if executable_identity(process)? != expected {
        return Err(process_failure(format!(
            "traced {label} executable identity differs"
        )));
    }
    Ok(())
}

fn executable_identity(
    process: i32,
) -> Result<ObjectIdentityV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let file = File::open(format!("/proc/{process}/exe"))
        .map_err(|_| io_process_failure("open traced executable identity"))?;
    Ok(ObjectSnapshotV2::capture(&file, "traced executable")?.object_identity())
}

fn thread_group_id(process: i32) -> Result<i32, RetainedFunctionalRefinementRuntimeErrorV1> {
    let status = std::fs::read_to_string(format!("/proc/{process}/status"))
        .map_err(|_| io_process_failure("read traced thread-group identity"))?;
    if status.len() > 16 * 1024 {
        return Err(process_failure("traced process status is oversized"));
    }
    status
        .lines()
        .find_map(|line| line.strip_prefix("Tgid:")?.trim().parse::<i32>().ok())
        .filter(|thread_group| *thread_group > 0)
        .ok_or_else(|| process_failure("traced thread-group identity is missing"))
}

fn validate_initial_descriptor_closure(
    process: i32,
    bindings: &[DescriptorBinding],
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    validate_exact_descriptor_closure(process, bindings, "rust_verify")
}

fn validate_exact_descriptor_closure(
    process: i32,
    bindings: &[DescriptorBinding],
    label: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut expected = vec![0, 1, 2];
    expected.extend(
        bindings
            .iter()
            .filter(|binding| !binding.close_on_exec)
            .map(|binding| binding.destination),
    );
    expected.sort_unstable();
    let mut observed = descriptor_numbers(process)?;
    observed.sort_unstable();
    if observed != expected {
        return Err(process_failure(format!(
            "{label} inherited an unexpected descriptor set"
        )));
    }
    validate_bound_descriptor_identities(process, bindings)
}

fn validate_bound_descriptor_identities(
    process: i32,
    bindings: &[DescriptorBinding],
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    for binding in bindings.iter().filter(|binding| !binding.close_on_exec) {
        let file = File::open(format!("/proc/{process}/fd/{}", binding.destination))
            .map_err(|_| io_process_failure("open inherited retained descriptor"))?;
        if ObjectSnapshotV2::capture(&file, "inherited retained descriptor")?.object_identity()
            != binding.identity
        {
            return Err(process_failure(
                "inherited retained descriptor identity differs",
            ));
        }
    }
    Ok(())
}

fn descriptor_numbers(
    process: i32,
) -> Result<Vec<i32>, RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(format!("/proc/{process}/fd"))
        .map_err(|_| io_process_failure("scan traced descriptor table"))?
    {
        let entry = entry.map_err(|_| io_process_failure("read traced descriptor table"))?;
        let descriptor = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
            .ok_or_else(|| process_failure("traced descriptor name is noncanonical"))?;
        result.push(descriptor);
    }
    Ok(result)
}

#[cfg(test)]
fn validate_sensitive_registers(
    process: i32,
    registers: &UserRegistersX86_64,
    allowed: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    validate_sensitive_registers_with_policy(
        process,
        registers,
        allowed,
        validate_mappings,
        GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
        TraceeRole::Verifier,
    )
}

fn validate_sensitive_registers_with_policy(
    process: i32,
    registers: &UserRegistersX86_64,
    allowed: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
    policy: GeneratedProofProcessPolicyV2,
    role: TraceeRole,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    match u32::try_from(registers.orig_rax) {
        Ok(CLONE3_SYSCALL) => validate_clone3_request(process, registers, policy, role),
        Ok(OPEN_SYSCALL) => validate_read_only_open(registers.rsi),
        Ok(OPENAT_SYSCALL) => validate_read_only_open(registers.rdx),
        Ok(PRCTL_SYSCALL) if registers.rdi == PR_SET_NAME && registers.rsi != 0 => Ok(()),
        Ok(PRCTL_SYSCALL) => Err(process_failure(
            "prctl request is outside exact Rust thread naming",
        )),
        Ok(MMAP_SYSCALL) => {
            if registers.rdx & PROT_EXEC == 0 {
                return Ok(());
            }
            if registers.rdx & PROT_WRITE != 0 {
                return Err(process_failure("writable executable mmap is not admitted"));
            }
            if !validate_mappings {
                return Ok(());
            }
            if registers.r10 & MAP_ANONYMOUS != 0 || registers.r8 as i64 == -1 {
                return Err(process_failure(
                    "anonymous executable mmap is outside the retained runtime closure",
                ));
            }
            let descriptor = i32::try_from(registers.r8)
                .map_err(|_| process_failure("executable mmap uses a noncanonical descriptor"))?;
            let file = File::open(format!("/proc/{process}/fd/{descriptor}"))
                .map_err(|_| io_process_failure("open executable mmap descriptor"))?;
            let identity =
                ObjectSnapshotV2::capture(&file, "executable mmap descriptor")?.object_identity();
            if !executable_object_range_is_allowed(identity, registers.r9, registers.rsi, allowed)?
            {
                return Err(process_failure(
                    "executable mmap object or file range is outside the retained runtime closure",
                ));
            }
            Ok(())
        }
        Ok(MPROTECT_SYSCALL) | Ok(PKEY_MPROTECT_SYSCALL) => {
            if registers.rdx & PROT_EXEC == 0 {
                return Ok(());
            }
            // File identity cannot authenticate privately dirtied pages. The supported
            // loader must map text RX initially, never add or restore EXEC with mprotect.
            Err(process_failure("executable mprotect is not admitted"))
        }
        Ok(MREMAP_SYSCALL) if validate_mappings => {
            validate_nonexecutable_mapping_range(process, registers.rdi, registers.rsi)
        }
        Ok(REMAP_FILE_PAGES_SYSCALL) if validate_mappings => {
            if registers.rdx != 0 || registers.r8 != 0 {
                return Err(process_failure(
                    "remap_file_pages uses noncanonical protection or flags",
                ));
            }
            validate_nonexecutable_mapping_range(process, registers.rdi, registers.rsi)
        }
        Ok(MREMAP_SYSCALL) | Ok(REMAP_FILE_PAGES_SYSCALL) => Ok(()),
        _ => Err(process_failure(
            "unexpected syscall reached the sensitive-syscall admission checkpoint",
        )),
    }
}

fn validate_read_only_open(flags: u64) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    // Scalar flags avoid a pathname race and cover /proc/self/mem, thread-self,
    // numeric PIDs and procfd aliases alike. Existing inherited output pipes remain usable.
    if flags & WRITABLE_OPEN_FLAGS != 0 {
        return Err(process_failure("write-capable file open is not admitted"));
    }
    Ok(())
}

fn validate_clone3_request(
    process: i32,
    registers: &UserRegistersX86_64,
    policy: GeneratedProofProcessPolicyV2,
    role: TraceeRole,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if registers.rsi != CLONE3_ARGUMENT_BYTES || registers.rdi == 0 {
        return Err(process_failure(
            "clone3 argument size or pointer is not the pinned ABI",
        ));
    }
    let mut bytes = [0_u8; CLONE3_ARGUMENT_BYTES as usize];
    let memory = File::open(format!("/proc/{process}/mem"))
        .map_err(|_| io_process_failure("open traced clone3 arguments"))?;
    let count = rustix::io::pread(&memory, &mut bytes, registers.rdi)
        .map_err(|error| io_error("read traced clone3 arguments", error))?;
    if count != bytes.len() {
        return Err(process_failure("clone3 arguments were truncated"));
    }
    let mut arguments = [0_u64; 11];
    for (index, chunk) in bytes.chunks_exact(8).enumerate() {
        arguments[index] = u64::from_ne_bytes(chunk.try_into().expect("eight-byte chunk"));
    }
    validate_clone3_arguments(arguments, policy, role)
}

fn validate_clone3_arguments(
    arguments: [u64; 11],
    policy: GeneratedProofProcessPolicyV2,
    role: TraceeRole,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let [
        flags,
        pidfd,
        child_tid,
        parent_tid,
        exit_signal,
        stack,
        stack_size,
        tls,
        set_tid,
        set_tid_size,
        cgroup,
    ] = arguments;
    let common = set_tid == 0
        && set_tid_size == 0
        && cgroup == 0
        && stack != 0
        && stack.checked_add(stack_size).is_some();
    let rust_thread = flags == RUST_THREAD_CLONE3_FLAGS
        && exit_signal == 0
        && child_tid != 0
        && pidfd == child_tid
        && parent_tid != 0
        && parent_tid == child_tid
        && tls != 0;
    let rust_process = flags == RUST_PROCESS_CLONE3_FLAGS
        && exit_signal == SIGCHLD
        && pidfd == 0
        && child_tid == 0
        && parent_tid == 0
        && tls == 0;
    // Only roles established by the retained executable checks can use V3's
    // pinned interpreter stack. The pending child and solver never inherit it.
    let stack_limit =
        if rust_thread && matches!(role, TraceeRole::Verifier | TraceeRole::AuxiliaryVerifier) {
            policy.verifier_thread_stack_bytes()
        } else {
            MAX_CLONE_STACK_BYTES
        };
    if common && (1..=stack_limit).contains(&stack_size) && (rust_thread || rust_process) {
        Ok(())
    } else {
        Err(process_failure(format!(
            "clone3 request is outside the pinned Rust thread/process ABI: flags={flags:#x} pidfd={pidfd:#x} child_tid={child_tid:#x} parent_tid={parent_tid:#x} exit_signal={exit_signal} stack={stack:#x} stack_size={stack_size:#x} tls={tls:#x} set_tid={set_tid:#x} set_tid_size={set_tid_size} cgroup={cgroup}"
        )))
    }
}

fn read_registers(
    process: i32,
) -> Result<UserRegistersX86_64, RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut registers = UserRegistersX86_64::default();
    // SAFETY: GETREGS writes one architecture-specific register structure.
    if unsafe {
        linux_ptrace(
            PTRACE_GETREGS,
            process,
            std::ptr::null_mut(),
            (&raw mut registers).cast(),
        )
    } < 0
    {
        return Err(io_process_failure(
            "read executable-mapping syscall registers",
        ));
    }
    Ok(registers)
}

fn validate_nonexecutable_mapping_range(
    process: i32,
    start: u64,
    length: u64,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if length == 0 {
        return Err(process_failure(
            "zero-length mapping remap request is not admitted",
        ));
    }
    let end = start
        .checked_add(length)
        .ok_or_else(|| process_failure("mapping remap range overflow"))?;
    let maps = std::fs::read_to_string(format!("/proc/{process}/maps"))
        .map_err(|_| io_process_failure("read remapped process mappings"))?;
    if maps.len() > 1024 * 1024 {
        return Err(process_failure(
            "remapped process map inventory is oversized",
        ));
    }
    let mut cursor = start;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields
            .next()
            .ok_or_else(|| process_failure("malformed remapped process map"))?;
        let permissions = fields
            .next()
            .ok_or_else(|| process_failure("malformed remapped process map"))?;
        let (mapping_start, mapping_end) = parse_mapping_range(range)?;
        if mapping_end <= cursor {
            continue;
        }
        if mapping_start > cursor {
            break;
        }
        if permissions
            .as_bytes()
            .get(2)
            .is_some_and(|value| *value == b'x')
        {
            return Err(process_failure(
                "mapping remap covers an executable source range",
            ));
        }
        cursor = mapping_end.min(end);
        if cursor == end {
            return Ok(());
        }
    }
    Err(process_failure(
        "mapping remap source range is not fully mapped",
    ))
}

fn parse_mapping_range(
    range: &str,
) -> Result<(u64, u64), RetainedFunctionalRefinementRuntimeErrorV1> {
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| process_failure("malformed process mapping range"))?;
    let start = u64::from_str_radix(start, 16)
        .map_err(|_| process_failure("noncanonical process mapping start"))?;
    let end = u64::from_str_radix(end, 16)
        .map_err(|_| process_failure("noncanonical process mapping end"))?;
    if start >= end {
        return Err(process_failure("empty or inverted process mapping range"));
    }
    Ok((start, end))
}

fn parse_mapping_file_offset(
    offset: &str,
) -> Result<u64, RetainedFunctionalRefinementRuntimeErrorV1> {
    u64::from_str_radix(offset, 16)
        .map_err(|_| process_failure("noncanonical process mapping file offset"))
}

fn executable_object_range_is_allowed(
    identity: ObjectIdentityV2,
    offset: u64,
    length: u64,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<bool, RetainedFunctionalRefinementRuntimeErrorV1> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| process_failure("executable mapping file range overflow"))?;
    if length == 0 {
        return Ok(false);
    }
    Ok(allowed.iter().any(|executable| {
        executable.identity == identity
            && executable
                .executable_file_ranges
                .iter()
                .any(|(start, admitted_end)| *start <= offset && end <= *admitted_end)
    }))
}

fn mapping_file_range_is_allowed(
    device: &str,
    inode: &str,
    offset: u64,
    length: u64,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<bool, RetainedFunctionalRefinementRuntimeErrorV1> {
    let (major, minor) = device
        .split_once(':')
        .ok_or_else(|| process_failure("malformed process mapping device"))?;
    let major = u32::from_str_radix(major, 16)
        .map_err(|_| process_failure("noncanonical process mapping device major"))?;
    let minor = u32::from_str_radix(minor, 16)
        .map_err(|_| process_failure("noncanonical process mapping device minor"))?;
    let inode = inode
        .parse::<u64>()
        .map_err(|_| process_failure("noncanonical process mapping inode"))?;
    if inode == 0 {
        return Ok(false);
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| process_failure("executable mapping file range overflow"))?;
    if length == 0 {
        return Ok(false);
    }
    Ok(allowed.iter().any(|executable| {
        executable.identity.inode == inode
            && rustix::fs::major(executable.identity.device) == major
            && rustix::fs::minor(executable.identity.device) == minor
            && executable
                .executable_file_ranges
                .iter()
                .any(|(start, admitted_end)| *start <= offset && end <= *admitted_end)
    }))
}

fn validate_executable_mappings(
    process: i32,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let personality = std::fs::read_to_string(format!("/proc/{process}/personality"))
        .map_err(|_| io_process_failure("read traced process personality"))?;
    validate_personality(&personality)?;
    let maps = std::fs::read_to_string(format!("/proc/{process}/maps"))
        .map_err(|_| io_process_failure("read traced executable mappings"))?;
    if maps.len() > 1024 * 1024 {
        return Err(process_failure(
            "traced executable map inventory is oversized",
        ));
    }
    validate_executable_mapping_rows(&maps, allowed)
}

fn validate_personality(
    personality: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let value = u32::from_str_radix(personality.trim(), 16)
        .map_err(|_| process_failure("malformed traced process personality"))?;
    if value & READ_IMPLIES_EXEC != 0 {
        return Err(process_failure("READ_IMPLIES_EXEC is not admitted"));
    }
    Ok(())
}

fn validate_executable_mapping_rows(
    maps: &str,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut executable_count = 0_usize;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields
            .next()
            .ok_or_else(|| process_failure("malformed process map"))?;
        let (mapping_start, mapping_end) = parse_mapping_range(range)?;
        let permissions = fields
            .next()
            .ok_or_else(|| process_failure("malformed process map"))?;
        if permissions
            .as_bytes()
            .get(2)
            .is_none_or(|value| *value != b'x')
        {
            continue;
        }
        if permissions.as_bytes().get(1) == Some(&b'w') {
            return Err(process_failure(
                "writable executable mapping is not admitted",
            ));
        }
        executable_count += 1;
        if executable_count > 256 {
            return Err(process_failure("too many executable mappings"));
        }
        let file_offset = fields
            .next()
            .ok_or_else(|| process_failure("malformed process map"))?;
        let file_offset = parse_mapping_file_offset(file_offset)?;
        let device = fields
            .next()
            .ok_or_else(|| process_failure("malformed process map"))?;
        let inode = fields
            .next()
            .ok_or_else(|| process_failure("malformed process map"))?;
        let path = fields.next().unwrap_or("");
        if matches!(path, "[vdso]" | "[vsyscall]") {
            continue;
        }
        if path.is_empty() || path.starts_with('[') {
            return Err(process_failure(
                "anonymous executable mapping is not admitted",
            ));
        }
        if !mapping_file_range_is_allowed(
            device,
            inode,
            file_offset,
            mapping_end - mapping_start,
            allowed,
        )? {
            #[cfg(test)]
            {
                // Diagnostic only: the original closed identity/range predicate
                // above and its refusal below remain unchanged.
                let parsed_inode = inode.parse::<u64>().ok();
                let parsed_device = device.split_once(':').and_then(|(major, minor)| {
                    Some((
                        u32::from_str_radix(major, 16).ok()?,
                        u32::from_str_radix(minor, 16).ok()?,
                    ))
                });
                let same_inode = allowed
                    .iter()
                    .filter(|entry| Some(entry.identity.inode) == parsed_inode)
                    .count();
                let same_device = allowed
                    .iter()
                    .filter(|entry| {
                        Some((
                            rustix::fs::major(entry.identity.device),
                            rustix::fs::minor(entry.identity.device),
                        )) == parsed_device
                    })
                    .count();
                let inventory = format!("{allowed:?}");
                let inventory = inventory.as_bytes();
                eprintln!(
                    "runtime maps test diagnostic: line={:?}, same_inode={same_inode}, same_device={same_device}, retained_prefix={:?}",
                    String::from_utf8_lossy(&line.as_bytes()[..line.len().min(512)]),
                    String::from_utf8_lossy(&inventory[..inventory.len().min(2048)]),
                );
            }
            return Err(process_failure(
                "executable mapping object or file range is outside retained runtime closure",
            ));
        }
    }
    if executable_count == 0 {
        return Err(process_failure("traced process has no executable mappings"));
    }
    Ok(())
}

fn terminate_tree(tracees: &mut Tracees) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    #[cfg(test)]
    let deadline = quarantine_tests::cleanup_deadline(deadline);
    let mut failures = Vec::new();
    let mut discovery_failure = None;
    let mut wait_failure = None;
    let mut quiescent = false;
    let mut kill_failed = false;
    while tracees.unresolved() && Instant::now() < deadline {
        for process in tracees.pids() {
            let task = &tracees[&process];
            if task.terminal_consumed || task.current_stop.is_some() {
                continue;
            }
            let mut status = 0;
            // SAFETY: exact, unreaped PID retained by this originating tracer.
            let result = unsafe { waitpid(process, &mut status, WAIT_NOHANG | WAIT_WALL) };
            match result {
                0 => {}
                value if value == process => {
                    let task = tracees.get_mut(&process).expect("retained cleanup task");
                    task.current_stop = TraceeStop::observed(status);
                    task.cleanup_exiting |= task.exit_boundary.is_some()
                        || task.current_stop.is_some_and(TraceeStop::is_exit);
                    task.terminal_consumed = !stopped(status);
                    #[cfg(test)]
                    if stopped(status) {
                        stable_tests::record_cleanup_stop(process, status);
                    } else {
                        stable_tests::record_cleanup_terminal(process, status);
                        spawn_lease_tests::record_terminal(process, status);
                    }
                }
                _ if io::Error::last_os_error().raw_os_error() == Some(4) => {}
                _ => {
                    // ECHILD/procfs absence cannot authenticate a terminal wait.
                    wait_failure = Some(format!(
                        "wait PID {process}: {}",
                        io::Error::last_os_error()
                    ));
                }
            }
        }
        // Discover consumed and just-waited births BEFORE any fatal signal.
        discovery_failure = None;
        for process in tracees.pids() {
            if !tracees[&process]
                .current_stop
                .is_some_and(TraceeStop::unregistered_birth)
            {
                continue;
            }
            if tracees[&process].terminal_consumed {
                discovery_failure = Some("terminal parent retained an unresolved birth".to_owned());
                continue;
            }
            let discover = (|| {
                let child = event_child(process)?;
                if !tracees.contains_key(&child) || tracees[&child].terminal_consumed {
                    tracees.insert(
                        child,
                        Tracee::pending(TraceeRole::PendingExecutable, child, true),
                    )?;
                } else {
                    // A registered child is marked at its parent atomically; a
                    // duplicate here cannot establish which lifetime owns PID.
                    tracees.uncertain(child);
                    return Err(process_failure("duplicate cleanup birth identity"));
                }
                let parent = tracees.get_mut(&process).expect("birth parent");
                parent
                    .current_stop
                    .as_mut()
                    .expect("birth stop")
                    .birth_registered = true;
                parent.pending_creation = false;
                Ok::<_, RetainedFunctionalRefinementRuntimeErrorV1>(())
            })();
            if let Err(error) = discover {
                discovery_failure =
                    Some(format!("discover descendant from PID {process}: {error}"));
            }
        }
        if !quiescent && wait_failure.is_none() {
            for process in tracees.pids() {
                let task = tracees.get_mut(&process).expect("cleanup task");
                task.cleanup_exiting |= task.exit_boundary.is_some()
                    || task.current_stop.is_some_and(TraceeStop::is_exit);
                if task.terminal_consumed
                    || task.current_stop.is_some()
                    || task.cleanup_exiting
                    || task.cleanup_interrupt_sent
                {
                    continue;
                }
                // Creation events precede an INTERRUPT stop; retain new children
                // in this same fixed point. External kills require isolation.
                if unsafe {
                    linux_ptrace(
                        PTRACE_INTERRUPT,
                        process,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                } < 0
                {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(3) {
                        failures.push(format!("interrupt cleanup PID {process}: {error}"));
                    }
                }
                task.cleanup_interrupt_sent = true;
            }
            quiescent = discovery_failure.is_none()
                && !tracees.has_uncertain()
                && tracees.values().all(|task| {
                    !task.pending_creation
                        && (task.terminal_consumed
                            || task.current_stop.is_some()
                            || task.cleanup_exiting)
                });
        }
        if Instant::now() >= deadline {
            break;
        }
        if quiescent && discovery_failure.is_none() && !kill_failed {
            for process in tracees.pids() {
                let task = tracees.get_mut(&process).expect("cleanup task");
                if !task.terminal_consumed && !task.cleanup_kill_sent {
                    if let Err(error) = kill_tracee(process) {
                        kill_failed = true;
                        failures.push(error);
                        break;
                    }
                    task.cleanup_kill_sent = true;
                }
            }
            // PTRACE_CONT's signal argument is not delivery at a nonsignal
            // stop. A failed kill (including ESRCH) therefore closes ALL
            // resumes, even held EXIT stops. Keep exact waits for tasks already
            // running/exiting, otherwise retain custody at the original bound.
            if !kill_failed {
                for process in tracees.pids() {
                    let task = tracees.get_mut(&process).expect("cleanup task");
                    if !task.terminal_consumed && task.current_stop.is_some() {
                        match continue_killed_tracee(process) {
                            Ok(()) => task.current_stop = None,
                            Err(error) => {
                                failures.push(format!("continue killed PID {process}: {error}"))
                            }
                        }
                    }
                }
            }
        }
        if tracees.unresolved() {
            thread::sleep(POLL_INTERVAL);
        }
    }
    // Poison before any allocating error conversion and before caller Drop.
    // Never retry ptrace or wait from a later caller or another thread.
    if tracees.unresolved() {
        custody::poison();
    }
    if let Some(error) = discovery_failure {
        failures.push(error);
    }
    if let Some(error) = wait_failure {
        failures.push(error);
    }
    if tracees.unresolved() {
        failures.push("cleanup deadline left permanent unresolved task custody".to_owned());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(process_failure(failures.join("; ")))
    }
}
fn kill_tracee(process: i32) -> Result<(), String> {
    #[cfg(test)]
    stable_tests::record_cleanup_kill_request(process);
    let delivery = (|| -> io::Result<()> {
        #[cfg(test)]
        quarantine_tests::inject_kill_refusal()?;
        // Linux kill_pid_info resolves this retained TID and signals its group;
        // it is not a thread-directed terminal observation. ESRCH can race group
        // teardown and never establishes delivery or releases this task's PID.
        // SAFETY: the exact PID remains owned and unreaped until an actual wait.
        if unsafe { kill(process, SIGKILL) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    })();
    delivery.map_err(|error| format!("kill PID {process}: {error}"))
}

fn continue_killed_tracee(process: i32) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    #[cfg(test)]
    quarantine_tests::record_cleanup_continue();
    // SAFETY: cleanup calls this only after its closed kill pass succeeded.
    // The signal argument cannot substitute for actual SIGKILL delivery.
    if unsafe {
        linux_ptrace(
            PTRACE_CONT,
            process,
            std::ptr::null_mut(),
            (SIGKILL as usize) as *mut c_void,
        )
    } >= 0
        || io::Error::last_os_error().raw_os_error() == Some(3)
    {
        Ok(())
    } else {
        Err(io_process_failure("continue killed proof process"))
    }
}

fn reject_and_reap(
    tracees: &mut Tracees,
    execution_error: RetainedFunctionalRefinementRuntimeErrorV1,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    match terminate_tree(tracees) {
        Ok(()) => execution_error,
        Err(cleanup_error) => process_failure(format!(
            "failed to reap the rejected proof process tree after {execution_error}: {cleanup_error}"
        )),
    }
}

fn make_nonblocking(
    descriptor: &impl std::os::fd::AsFd,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let flags = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|error| io_error("read proof output flags", error))?;
    rustix::fs::fcntl_setfl(descriptor, flags | OFlags::NONBLOCK)
        .map_err(|error| io_error("set proof output nonblocking", error))
}

fn drain(
    pipe: &mut impl Read,
    capture: &mut Capture,
    limit: usize,
    stream: OutputStream,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let mut buffer = [0_u8; 4096];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                capture.eof = true;
                return Ok(());
            }
            Ok(count) => {
                if count > limit.saturating_sub(capture.bytes.len()) {
                    return Err(output_too_large(
                        stream,
                        &capture.bytes,
                        &buffer[..count],
                        limit,
                    ));
                }
                capture.bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(_) => return Err(io_process_failure("read traced proof output")),
        }
    }
}

fn output_too_large(
    stream: OutputStream,
    retained: &[u8],
    incoming: &[u8],
    limit: usize,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    let (prefix, truncated) = bounded_output_prefix(retained, incoming);
    let detail = format!(
        "functional-refinement process exceeded its output bound: stream={} limit={limit} retained={} observed_at_least={} prefix=\"{prefix}\"{}",
        stream.name(),
        retained.len(),
        retained.len().saturating_add(incoming.len()),
        if truncated { " (truncated)" } else { "" },
    );
    controller_error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::OutputTooLarge,
        detail,
    )
}

const MAX_ESCAPED_OUTPUT_PREFIX_BYTES: usize = 1024;

fn bounded_output_prefix(retained: &[u8], incoming: &[u8]) -> (String, bool) {
    bounded_output_prefix_with_limit(retained, incoming, MAX_ESCAPED_OUTPUT_PREFIX_BYTES)
}

fn bounded_output_prefix_with_limit(
    retained: &[u8],
    incoming: &[u8],
    limit: usize,
) -> (String, bool) {
    let mut prefix = String::with_capacity(limit);
    for byte in retained.iter().chain(incoming) {
        let escaped = std::ascii::escape_default(*byte);
        if escaped.len() > limit - prefix.len() {
            return (prefix, true);
        }
        prefix.extend(escaped.map(char::from));
    }
    (prefix, false)
}

const MAX_CENSUS_DIAGNOSTIC_PREFIX_BYTES: usize = 512;

fn context_terminal_failure(
    error: RetainedFunctionalRefinementRuntimeErrorV1,
    verifier: (Option<i32>, Option<i32>),
    stdout: &[u8],
    stderr: &[u8],
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    let prefix = |bytes: &[u8]| {
        bounded_output_prefix_with_limit(bytes, &[], MAX_CENSUS_DIAGNOSTIC_PREFIX_BYTES)
    };
    let (cause, cause_truncated) = prefix(error.detail().as_bytes());
    let (out, out_truncated) = prefix(stdout);
    let (err, err_truncated) = prefix(stderr);
    let marker = |truncated| if truncated { " (truncated)" } else { "" };
    RetainedFunctionalRefinementRuntimeErrorV1::new(
        error.kind(),
        format!(
            "{cause}{}; verifier={verifier:?}; stderr_bytes={} stderr=\"{err}\"{}; stdout_bytes={} stdout=\"{out}\"{}",
            marker(cause_truncated),
            stderr.len(),
            marker(err_truncated),
            stdout.len(),
            marker(out_truncated),
        ),
    )
}

fn drain_to_eof(
    stdout: &mut impl Read,
    stderr: &mut impl Read,
    stdout_capture: &mut Capture,
    stderr_capture: &mut Capture,
    limit: usize,
    deadline: Instant,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let grace = deadline.min(Instant::now() + Duration::from_millis(200));
    while (!stdout_capture.eof || !stderr_capture.eof) && Instant::now() < grace {
        drain(stdout, stdout_capture, limit, OutputStream::Stdout)?;
        drain(stderr, stderr_capture, limit, OutputStream::Stderr)?;
        if !stdout_capture.eof || !stderr_capture.eof {
            thread::sleep(POLL_INTERVAL);
        }
    }
    if !stdout_capture.eof || !stderr_capture.eof {
        return Err(process_failure(
            "proof output descriptors remained open after tree exit",
        ));
    }
    Ok(())
}

// Closely spaced ptrace stops should not each pay the fully idle polling delay.
fn next_tree_poll_interval(previous: Duration, progressed: bool) -> Duration {
    if progressed {
        ACTIVE_TREE_POLL_INTERVAL
    } else {
        previous.saturating_mul(2).min(POLL_INTERVAL)
    }
}

fn process_failure(detail: impl Into<String>) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    controller_error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::Process,
        detail,
    )
}

fn io_process_failure(context: &str) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    process_failure(format!("{context}: {}", io::Error::last_os_error()))
}

fn io_error(context: &str, error: rustix::io::Errno) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    controller_error(
        RetainedFunctionalRefinementRuntimeErrorKindV1::Io,
        format!("{context}: {error}"),
    )
}

fn controller_error(
    kind: RetainedFunctionalRefinementRuntimeErrorKindV1,
    detail: impl Into<String>,
) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    RetainedFunctionalRefinementRuntimeErrorV1::new(
        kind,
        format!(
            "functional-refinement process-tree controller: {}",
            detail.into()
        ),
    )
}

#[cfg(test)]
#[path = "functional_refinement_process_tree_v1_memory_tests.rs"]
mod memory_tests;

#[cfg(test)]
#[path = "functional_refinement_process_tree_v1_stable_tests.rs"]
mod stable_tests;

#[cfg(test)]
#[path = "functional_refinement_clone3_stack_v3_tests.rs"]
mod clone3_stack_v3_tests;

#[cfg(test)]
#[path = "functional_refinement_process_tree_v1_quarantine_tests.rs"]
mod quarantine_tests;

#[cfg(test)]
mod tests {
    use super::*;
    include!("functional_refinement_solver_process_v2_tests.rs");

    #[test]
    fn tree_poll_interval_backs_off_caps_and_resets_after_progress() {
        let mut interval = ACTIVE_TREE_POLL_INTERVAL;
        for microseconds in [200, 400, 800, 1600, 2000, 2000] {
            interval = next_tree_poll_interval(interval, false);
            assert_eq!(interval, Duration::from_micros(microseconds));
        }
        assert_eq!(next_tree_poll_interval(Duration::MAX, false), POLL_INTERVAL);
        assert_eq!(
            next_tree_poll_interval(interval, true),
            ACTIVE_TREE_POLL_INTERVAL,
        );
    }

    struct HostileRun {
        result: Result<
            RetainedFunctionalRefinementRuntimeOutputV1,
            RetainedFunctionalRefinementRuntimeErrorV1,
        >,
        first_descendant: i32,
        last_descendant: i32,
        peak_executed_solver_groups: usize,
    }

    pub(super) fn identity(path: &str) -> ObjectIdentityV2 {
        let file = File::open(path).unwrap();
        ObjectSnapshotV2::capture(&file, "hostile test executable")
            .unwrap()
            .object_identity()
    }

    pub(super) fn allowed_executable(path: &str) -> AllowedRuntimeExecutableV1 {
        let file = File::open(path).unwrap();
        let identity = ObjectSnapshotV2::capture(&file, "hostile test executable")
            .unwrap()
            .object_identity();
        allowed_runtime_executable(&file, identity, Path::new(path))
            .unwrap()
            .unwrap()
    }

    fn evaluate_filter(architecture: u32, syscall: u32, argument_zero: u64) -> u32 {
        let filter = seccomp_filter();
        let mut accumulator = 0_u32;
        let mut program_counter = 0_usize;
        loop {
            let instruction = filter[program_counter];
            match instruction.code {
                BPF_LOAD_WORD_ABSOLUTE => {
                    accumulator = match instruction.value {
                        0 => syscall,
                        4 => architecture,
                        16 => argument_zero as u32,
                        20 => (argument_zero >> 32) as u32,
                        offset => panic!("unexpected seccomp-data offset {offset}"),
                    };
                    program_counter += 1;
                }
                BPF_ALU_AND => {
                    accumulator &= instruction.value;
                    program_counter += 1;
                }
                BPF_JUMP_EQUAL => {
                    program_counter += 1 + usize::from(if accumulator == instruction.value {
                        instruction.jump_true
                    } else {
                        instruction.jump_false
                    });
                }
                BPF_JUMP_GREATER_EQUAL => {
                    program_counter += 1 + usize::from(if accumulator >= instruction.value {
                        instruction.jump_true
                    } else {
                        instruction.jump_false
                    });
                }
                BPF_RETURN => return instruction.value,
                code => panic!("unexpected BPF instruction {code:#x}"),
            }
        }
    }

    fn run_hostile(
        script: &str,
        expected_solver: &str,
        deadline_after: Duration,
        leaked: Option<&File>,
    ) -> HostileRun {
        run_hostile_with_runtime(script, expected_solver, deadline_after, leaked, &[], false)
    }

    fn run_hostile_with_runtime(
        script: &str,
        expected_solver: &str,
        deadline_after: Duration,
        leaked: Option<&File>,
        allowed_mappings: &[AllowedRuntimeExecutableV1],
        validate_mappings: bool,
    ) -> HostileRun {
        run_hostile_with_policy(
            script,
            expected_solver,
            deadline_after,
            leaked,
            allowed_mappings,
            validate_mappings,
            GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
        )
    }

    fn run_contexts(script: &str, expected_solver: &str, leaked: Option<&File>) -> HostileRun {
        run_hostile_with_policy(
            script,
            expected_solver,
            Duration::from_secs(3),
            leaked,
            &[],
            false,
            GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_hostile_with_policy(
        script: &str,
        expected_solver: &str,
        deadline_after: Duration,
        leaked: Option<&File>,
        allowed_mappings: &[AllowedRuntimeExecutableV1],
        validate_mappings: bool,
        policy: GeneratedProofProcessPolicyV2,
    ) -> HostileRun {
        let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        FIRST_TEST_DESCENDANT.store(0, Ordering::SeqCst);
        LAST_TEST_DESCENDANT.store(0, Ordering::SeqCst);
        PEAK_TEST_EXECUTED_SOLVER_GROUPS.set(0);
        let mut duplicates = Vec::new();
        let mut child_bindings = Vec::new();
        if let Some(leaked) = leaked {
            let duplicate = rustix::io::fcntl_dupfd_cloexec(leaked, 200).unwrap();
            child_bindings.push(DescriptorBinding {
                source: duplicate.as_raw_fd(),
                destination: 190,
                close_on_exec: false,
                identity: ObjectSnapshotV2::capture(leaked, "hostile leaked descriptor")
                    .unwrap()
                    .object_identity(),
            });
            duplicates.push(duplicate);
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let deadline = Instant::now() + deadline_after;
        let mut child = seized_spawn::spawn(command, child_bindings, 2, deadline).unwrap();
        let result = supervise_with_policy(
            &mut child,
            &[],
            identity("/bin/sh"),
            identity(expected_solver),
            allowed_mappings,
            validate_mappings,
            false,
            deadline,
            4096,
            policy,
        );
        drop(duplicates);
        HostileRun {
            result,
            first_descendant: FIRST_TEST_DESCENDANT.load(Ordering::SeqCst),
            last_descendant: LAST_TEST_DESCENDANT.load(Ordering::SeqCst),
            peak_executed_solver_groups: PEAK_TEST_EXECUTED_SOLVER_GROUPS.get(),
        }
    }

    pub(super) fn assert_process_disappears(process: i32) {
        assert!(process > 0);
        let process_path = format!("/proc/{process}");
        let reap_deadline = Instant::now() + CLEANUP_TIMEOUT;
        while Path::new(&process_path).exists() && Instant::now() < reap_deadline {
            thread::sleep(POLL_INTERVAL);
        }
        assert!(!Path::new(&process_path).exists());
    }

    pub(super) fn expect_error(
        result: Result<
            RetainedFunctionalRefinementRuntimeOutputV1,
            RetainedFunctionalRefinementRuntimeErrorV1,
        >,
    ) -> RetainedFunctionalRefinementRuntimeErrorV1 {
        match result {
            Ok(_) => panic!("hostile proof process unexpectedly succeeded"),
            Err(error) => error,
        }
    }

    #[test]
    fn filter_denies_every_escape_and_keeps_process_creation_traceable() {
        assert!(DENIED_SYSCALLS.contains(&109));
        assert!(DENIED_SYSCALLS.contains(&112));
        assert!(DENIED_SYSCALLS.contains(&272));
        assert!(DENIED_SYSCALLS.contains(&308));
        assert!(SENSITIVE_SYSCALLS.contains(&435));
        assert!(DENIED_SYSCALLS.contains(&62));
        assert!(DENIED_SYSCALLS.contains(&425));
        assert_ne!(CLONE_ESCAPE_FLAGS & 0x0080_0000, 0);
        for process_creation in [56, 57, 58] {
            assert!(!DENIED_SYSCALLS.contains(&process_creation));
        }
        let filter = seccomp_filter();
        assert_eq!(filter.len(), FILTER_LEN);
        assert_eq!(filter.last().unwrap().value, SECCOMP_RETURN_ALLOW);
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, CLONE_SYSCALL, 17),
            SECCOMP_RETURN_TRACE
        );
        assert_eq!(
            evaluate_filter(
                AUDIT_ARCH_X86_64,
                CLONE_SYSCALL,
                u64::from(CLONE_ESCAPE_FLAGS | 17)
            ),
            SECCOMP_RETURN_KILL_PROCESS
        );
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, CLONE_SYSCALL, 1_u64 << 32),
            SECCOMP_RETURN_KILL_PROCESS
        );
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, 435, 0),
            SECCOMP_RETURN_TRACE
        );
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, MMAP_SYSCALL, 0),
            SECCOMP_RETURN_TRACE
        );
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, MREMAP_SYSCALL, 0),
            SECCOMP_RETURN_TRACE
        );
        assert_eq!(
            evaluate_filter(AUDIT_ARCH_X86_64, 39, 0),
            SECCOMP_RETURN_ALLOW
        );
        assert_eq!(evaluate_filter(0, 39, 0), SECCOMP_RETURN_KILL_PROCESS);
    }

    #[test]
    fn trace_policy_covers_all_process_creation_and_exit_events() {
        let options = PTRACE_O_TRACEFORK
            | PTRACE_O_TRACEVFORK
            | PTRACE_O_TRACECLONE
            | PTRACE_O_TRACEEXEC
            | PTRACE_O_TRACEEXIT
            | PTRACE_O_TRACESECCOMP
            | PTRACE_O_EXITKILL;
        assert_ne!(options & PTRACE_O_TRACEFORK, 0);
        assert_ne!(options & PTRACE_O_TRACEVFORK, 0);
        assert_ne!(options & PTRACE_O_TRACECLONE, 0);
        assert_ne!(options & PTRACE_O_TRACESECCOMP, 0);
        assert_ne!(options & PTRACE_O_EXITKILL, 0);
    }

    #[test]
    fn limits_cover_cpu_processes_descriptors_and_memory() {
        assert!(CPU_LIMIT_MAX_SECONDS > 0);
        assert!((2..=4096).contains(&PROCESS_LIMIT));
        assert!(DESCRIPTOR_LIMIT > GENERATED_PROOF_SOURCE_FD as u64);
        assert!(ADDRESS_SPACE_LIMIT_V2 > 0);
        assert!(DATA_LIMIT_V2 > 0);
        assert_eq!(CORE_LIMIT_V2, 0);
    }

    #[test]
    fn controller_security_preflight_accepts_the_test_host() {
        crate::authenticated_verus_execution_v2::validate_controller_security_v2().unwrap();
    }

    #[test]
    #[ignore = "requires a complete pinned functional-refinement runtime test closure"]
    fn pinned_functional_refinement_runtime_executes_a_real_verus_proof() {
        let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::var_os("FE2O3_FUNCTIONAL_REFINEMENT_TEST_RUNTIME_ROOT")
            .expect("set the synthetic retained runtime root");
        let manifest = super::super::ManifestV2::parse_functional_refinement_runtime_v1().unwrap();
        let runtime = RetainedRuntimeClosureV2::open_for_test(Path::new(&root), &manifest).unwrap();
        let source = CanonicalGeneratedVerusProofInputV3::new(
            b"use vstd::prelude::*;\nverus! { pub proof fn retained_runtime_sample() {} }\n"
                .to_vec(),
        )
        .unwrap();
        let mut attempt = AttemptV1::begin().unwrap();
        let output = execute(
            &mut attempt,
            std::sync::Arc::new(runtime),
            &source,
            Instant::now() + Duration::from_secs(120),
            4096,
        )
        .unwrap();
        attempt.complete().unwrap();
        assert_eq!((output.exit_code, output.signal), (Some(0), None));
        assert!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .contains("1 verified, 0 errors")
        );
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn terminal_status_decoder_is_exact() {
        assert_eq!(terminal_status(7 << 8), (Some(7), None));
        assert_eq!(terminal_status(SIGKILL), (None, Some(SIGKILL)));
        assert!(stopped((SIGSTOP << 8) | 0x7f));
        assert_eq!(parse_mapping_range("1000-2000").unwrap(), (0x1000, 0x2000));
        assert!(parse_mapping_range("2000-1000").is_err());
        assert!(parse_mapping_range("not-a-range").is_err());
    }

    #[test]
    fn executable_mapping_admission_accepts_an_exact_host_closure() {
        let allowed = [
            "/bin/sh",
            "/bin/true",
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/lib64/ld-linux-x86-64.so.2",
        ]
        .map(allowed_executable);
        let run = run_hostile_with_runtime(
            "/bin/true; :",
            "/bin/true",
            Duration::from_secs(2),
            None,
            &allowed,
            true,
        );
        match run.result {
            Ok(output) => assert_eq!((output.exit_code, output.signal), (Some(0), None)),
            Err(error) => panic!("exact executable mapping closure was rejected: {error}"),
        }
    }

    #[test]
    fn executable_mapping_admission_rejects_non_executable_elf_ranges() {
        let executable = allowed_executable("/bin/true");
        let (start, end) = executable.executable_file_ranges[0];
        assert!(
            executable_object_range_is_allowed(
                executable.identity,
                start,
                end - start,
                std::slice::from_ref(&executable),
            )
            .unwrap()
        );
        assert!(
            !executable_object_range_is_allowed(
                executable.identity,
                end,
                SYSTEM_PAGE_BYTES,
                std::slice::from_ref(&executable),
            )
            .unwrap()
        );
    }

    #[test]
    fn transient_unretained_executable_mapping_is_rejected() {
        let allowed = [
            "/bin/sh",
            "/bin/true",
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/lib64/ld-linux-x86-64.so.2",
        ]
        .map(allowed_executable);
        let run = run_hostile_with_runtime(
            "LD_PRELOAD=/lib/x86_64-linux-gnu/libm.so.6 /bin/true; :",
            "/bin/true",
            Duration::from_secs(2),
            None,
            &allowed,
            true,
        );
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(
            error
                .to_string()
                .contains("outside the retained runtime closure"),
            "{error}"
        );
    }

    #[test]
    fn wrong_solver_exec_is_rejected() {
        let run = run_hostile("/bin/true; :", "/bin/false", Duration::from_secs(2), None);
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(
            error.to_string().contains("executable identity differs"),
            "{error}"
        );
    }

    #[test]
    fn additional_descendant_is_rejected() {
        let run = run_hostile(
            "/bin/sleep 1 & /bin/sleep 1 & wait",
            "/bin/sleep",
            Duration::from_secs(2),
            None,
        );
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(
            error
                .to_string()
                .contains("additional or nested descendant"),
            "{error}"
        );
        assert_process_disappears(run.first_descendant);
        assert_process_disappears(run.last_descendant);
    }

    #[test]
    fn sequential_second_descendant_is_rejected() {
        let run = run_hostile(
            "/bin/true; /bin/true; :",
            "/bin/true",
            Duration::from_secs(2),
            None,
        );
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(
            error
                .to_string()
                .contains("additional or nested descendant"),
            "{error}"
        );
    }

    #[test]
    fn unexpected_inherited_descriptor_is_rejected() {
        let leaked = File::open("/dev/null").unwrap();
        let run = run_hostile(
            "/bin/sleep 0.01; :",
            "/bin/sleep",
            Duration::from_secs(2),
            Some(&leaked),
        );
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(error.to_string().contains("unexpected descriptor set"));
    }

    #[test]
    fn verifier_cannot_leak_a_new_descriptor_to_z3() {
        let run = run_hostile(
            "exec 9</dev/null; /bin/true; :",
            "/bin/true",
            Duration::from_secs(2),
            None,
        );
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        assert!(
            error
                .to_string()
                .contains("Z3 inherited an unexpected descriptor set"),
            "{error}"
        );
    }

    #[test]
    fn deadline_kills_and_reaps_the_solver_descendant() {
        let run = run_hostile(
            "/bin/sleep 30; :",
            "/bin/sleep",
            Duration::from_millis(100),
            None,
        );
        let descendant = run.last_descendant;
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::TimedOut,
            "{error}"
        );
        assert_process_disappears(descendant);
    }

    #[test]
    fn session_and_process_group_escape_syscalls_fail_closed() {
        let setsid_run = run_hostile(
            "/usr/bin/setsid /bin/true; :",
            "/usr/bin/setsid",
            Duration::from_secs(2),
            None,
        );
        let setsid = expect_error(setsid_run.result);
        assert_eq!(
            setsid.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );

        let python = ["/usr/bin/python3", "/bin/python3"]
            .into_iter()
            .find(|path| Path::new(path).is_file())
            .unwrap();
        let setpgid_run = run_hostile(
            &format!("{python} -c 'import os; os.setpgid(0, 0)'; :"),
            python,
            Duration::from_secs(2),
            None,
        );
        let setpgid = expect_error(setpgid_run.result);
        assert_eq!(
            setpgid.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
    }

    #[test]
    fn clone_untraced_escape_flag_fails_closed() {
        let python = ["/usr/bin/python3", "/bin/python3"]
            .into_iter()
            .find(|path| Path::new(path).is_file())
            .unwrap();
        let script = format!(
            "{python} -c 'import ctypes, os; libc=ctypes.CDLL(None, use_errno=True); pid=libc.syscall(56, 0x00800011, 0, 0, 0, 0); os._exit(0) if pid == 0 else (os.waitpid(pid, 0) if pid > 0 else (_ for _ in ()).throw(OSError(ctypes.get_errno())))'; :"
        );
        let run = run_hostile(&script, python, Duration::from_secs(2), None);
        let error = expect_error(run.result);
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
    }
}
