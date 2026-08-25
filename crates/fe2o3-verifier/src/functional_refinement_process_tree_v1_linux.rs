//! Descendant-aware Linux controller for workload-neutral functional-refinement proofs.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
use std::sync::atomic::{AtomicI32, Ordering};

use rustix::fs::OFlags;

use super::{
    ADDRESS_SPACE_LIMIT_V2, CORE_LIMIT_V2, CanonicalGeneratedVerusProofInputV3, DIST_DIRECTORY_FD,
};
use super::{
    DATA_LIMIT_V2, FILE_LIMIT_V2, GENERATED_PROOF_SOURCE_FD, GeneralGemmRuntimeClosureErrorKindV2,
    GeneralGemmRuntimeClosureErrorV2, GeneralGemmRuntimeProcessOutputV2, ObjectIdentityV2,
    ObjectSnapshotV2, RUST_VERIFY_FD, RetainedRuntimeClosureV2, SYSTEM_LIB_DIRECTORY_FD,
    SealedGeneratedProofSourceV3, TOOLCHAIN_DIRECTORY_FD, TOOLCHAIN_LIB_DIRECTORY_FD, Z3_FD,
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
// threads. Ptrace below supplies the strict per-proof one-descendant bound.
const PROCESS_LIMIT: u64 = 4096;
const DESCRIPTOR_LIMIT: u64 = 256;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);

const PTRACE_TRACEME: u32 = 0;
const PTRACE_CONT: u32 = 7;
const PTRACE_GETREGS: u32 = 12;
const PTRACE_SETOPTIONS: u32 = 0x4200;
const PTRACE_GETEVENTMSG: u32 = 0x4201;
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
const PROT_EXEC: u64 = 4;
const MAP_ANONYMOUS: u64 = 0x20;
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
const SENSITIVE_SYSCALLS: [u32; 7] = [
    MMAP_SYSCALL,
    MPROTECT_SYSCALL,
    MREMAP_SYSCALL,
    REMAP_FILE_PAGES_SYSCALL,
    PKEY_MPROTECT_SYSCALL,
    CLONE3_SYSCALL,
    PRCTL_SYSCALL,
];

// Process creation remains available only so rust_verify can create one observed Z3 child.
// Ptrace enforces cardinality. The filter kills every process-tree escape primitive.
const DENIED_SYSCALLS: [u32; 40] = [
    101, // ptrace
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

#[derive(Clone, Copy)]
struct Tracee {
    role: TraceeRole,
    thread_group: i32,
    leader: bool,
    saw_exit_event: bool,
    stop_consumed: bool,
}

struct Capture {
    bytes: Vec<u8>,
    eof: bool,
}

#[derive(Clone, Debug)]
pub(super) struct AllowedRuntimeExecutableV1 {
    identity: ObjectIdentityV2,
    executable_file_ranges: Vec<(u64, u64)>,
}

pub(super) fn allowed_runtime_executable(
    file: &File,
    identity: ObjectIdentityV2,
    path: &Path,
) -> Result<Option<AllowedRuntimeExecutableV1>, GeneralGemmRuntimeClosureErrorV2> {
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
    runtime: &RetainedRuntimeClosureV2,
    source: &CanonicalGeneratedVerusProofInputV3,
    deadline: Instant,
    output_limit: usize,
) -> Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2> {
    crate::authenticated_verus_execution_v2::validate_controller_security_v2().map_err(
        |error| {
            controller_error(
                GeneralGemmRuntimeClosureErrorKindV2::Process,
                format!("controller security preflight failed: {error}"),
            )
        },
    )?;
    if output_limit == 0 {
        return Err(controller_error(
            GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge,
            "functional-refinement output bound is zero",
        ));
    }
    if Instant::now() >= deadline {
        return Err(controller_error(
            GeneralGemmRuntimeClosureErrorKindV2::TimedOut,
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
                GeneralGemmRuntimeClosureErrorKindV2::Process,
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
        .min(CPU_LIMIT_MAX_SECONDS)
        .max(1);
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

    let child_bindings = bindings.clone();
    // SAFETY: the callback performs only raw async-signal-safe syscalls over captured scalars.
    unsafe {
        command.pre_exec(move || prepare_child(&child_bindings, cpu_seconds));
    }
    let mut child =
        crate::executor::spawn_artifact_coordinated_child(&mut command).map_err(|error| {
            controller_error(
                GeneralGemmRuntimeClosureErrorKindV2::Process,
                format!("spawn traced functional-refinement verifier: {error}"),
            )
        })?;
    drop(duplicates);
    let result = supervise(
        &mut child,
        &bindings,
        bindings[0].identity,
        bindings[1].identity,
        &allowed_mappings,
        true,
        true,
        deadline,
        output_limit,
    );
    sealed.revalidate(source)?;
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
    // SAFETY: the process asks its parent to trace it before installing the filter.
    if unsafe {
        linux_ptrace(
            PTRACE_TRACEME,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
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

fn supervise(
    child: &mut Child,
    bindings: &[DescriptorBinding],
    verifier_identity: ObjectIdentityV2,
    solver_identity: ObjectIdentityV2,
    allowed_mappings: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
    require_auxiliary_verifier: bool,
    deadline: Instant,
    output_limit: usize,
) -> Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2> {
    let verifier =
        i32::try_from(child.id()).map_err(|_| process_failure("verifier PID overflow"))?;
    let mut tracees = BTreeMap::from([(
        verifier,
        Tracee {
            role: TraceeRole::Verifier,
            thread_group: verifier,
            leader: true,
            saw_exit_event: false,
            stop_consumed: false,
        },
    )]);
    let Some(mut stdout) = child.stdout.take() else {
        return Err(reject_and_reap(
            &tracees,
            process_failure("traced verifier stdout pipe is missing"),
        ));
    };
    let Some(mut stderr) = child.stderr.take() else {
        return Err(reject_and_reap(
            &tracees,
            process_failure("traced verifier stderr pipe is missing"),
        ));
    };
    if let Err(error) = make_nonblocking(&stdout) {
        return Err(reject_and_reap(&tracees, error));
    }
    if let Err(error) = make_nonblocking(&stderr) {
        return Err(reject_and_reap(&tracees, error));
    }
    let mut stdout_capture = Capture {
        bytes: Vec::new(),
        eof: false,
    };
    let mut stderr_capture = Capture {
        bytes: Vec::new(),
        eof: false,
    };
    let execution = (|| {
        let status = wait_for_specific(verifier, deadline)?;
        if !stopped(status) || stop_signal(status) != SIGTRAP {
            return Err(process_failure(
                "verifier did not stop at its initial exec boundary",
            ));
        }
        tracees
            .get_mut(&verifier)
            .expect("verifier trace record exists")
            .stop_consumed = true;
        set_trace_options(verifier)?;
        validate_executable(verifier, verifier_identity, "rust_verify")?;
        validate_initial_descriptor_closure(verifier, bindings)?;
        if validate_mappings {
            validate_executable_mappings(verifier, allowed_mappings)?;
        }
        resume_tracee(&mut tracees, verifier, 0)?;

        let mut verifier_terminal = None;
        let mut auxiliary_terminal = None;
        let mut solver_terminal = None;
        let mut process_descendants_created = 0_usize;
        let mut auxiliary_started = false;
        let mut solver_started = false;
        let expected_process_descendants = if require_auxiliary_verifier { 2 } else { 1 };
        while !tracees.is_empty() {
            drain(&mut stdout, &mut stdout_capture, output_limit)?;
            drain(&mut stderr, &mut stderr_capture, output_limit)?;
            if Instant::now() >= deadline {
                return Err(controller_error(
                    GeneralGemmRuntimeClosureErrorKindV2::TimedOut,
                    "functional-refinement process tree exceeded its global deadline",
                ));
            }
            let mut progressed = false;
            let processes = tracees.keys().copied().collect::<Vec<_>>();
            for process in processes {
                let Some(status) = wait_for_specific_nonblocking(process)? else {
                    continue;
                };
                progressed = true;
                if stopped(status) {
                    tracees
                        .get_mut(&process)
                        .ok_or_else(|| process_failure("stop came from an unknown process"))?
                        .stop_consumed = true;
                    let event = (status as u32) >> 16;
                    let signal = stop_signal(status);
                    match event {
                        PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                            let parent = tracees.get(&process).copied().ok_or_else(|| {
                                process_failure("ptrace event came from an unknown process")
                            })?;
                            let child_process = event_child(process)?;
                            let exceeded_tracee_bound = tracees.len() >= MAX_TRACEES;
                            if tracees
                                .insert(
                                    child_process,
                                    Tracee {
                                        role: TraceeRole::PendingExecutable,
                                        thread_group: child_process,
                                        leader: true,
                                        saw_exit_event: false,
                                        stop_consumed: false,
                                    },
                                )
                                .is_some()
                            {
                                return Err(process_failure(
                                    "ptrace reported a duplicate proof tracee",
                                ));
                            }
                            let child_thread_group = thread_group_id(child_process)?;
                            let same_thread_group = child_thread_group == parent.thread_group;
                            let (role, leader) = if same_thread_group {
                                if parent.role == TraceeRole::PendingExecutable {
                                    return Err(process_failure(
                                        "unexecuted proof child created a thread",
                                    ));
                                }
                                (parent.role, false)
                            } else {
                                #[cfg(test)]
                                {
                                    let _ = FIRST_TEST_DESCENDANT.compare_exchange(
                                        0,
                                        child_process,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    );
                                    LAST_TEST_DESCENDANT.store(child_process, Ordering::SeqCst);
                                }
                                process_descendants_created =
                                    process_descendants_created.checked_add(1).ok_or_else(
                                        || process_failure("proof descendant counter overflow"),
                                    )?;
                                if parent.role != TraceeRole::Verifier
                                    || process_descendants_created > expected_process_descendants
                                {
                                    return Err(process_failure(
                                        "rust_verify created an additional or nested descendant, including sequential creation",
                                    ));
                                }
                                (TraceeRole::PendingExecutable, true)
                            };
                            let child = tracees
                                .get_mut(&child_process)
                                .expect("new proof tracee remains registered for cleanup");
                            child.role = role;
                            child.thread_group = child_thread_group;
                            child.leader = leader;
                            if exceeded_tracee_bound {
                                return Err(process_failure(
                                    "proof process tree exceeded its exact tracee bound",
                                ));
                            }
                            resume_tracee(&mut tracees, process, 0)?;
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
                            let role = if require_auxiliary_verifier
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
                            resume_tracee(&mut tracees, process, 0)?;
                        }
                        PTRACE_EVENT_SECCOMP => {
                            validate_sensitive_syscall_request(
                                process,
                                allowed_mappings,
                                validate_mappings,
                            )?;
                            resume_tracee(&mut tracees, process, 0)?;
                        }
                        PTRACE_EVENT_EXIT => {
                            let tracee = tracees.get_mut(&process).ok_or_else(|| {
                                process_failure("exit event came from an unknown process")
                            })?;
                            if tracee.role == TraceeRole::PendingExecutable {
                                return Err(process_failure(
                                    "proof descendant exited before an admitted exec",
                                ));
                            }
                            if validate_mappings {
                                validate_executable_mappings(process, allowed_mappings)?;
                            }
                            tracee.saw_exit_event = true;
                            resume_tracee(&mut tracees, process, 0)?;
                        }
                        0 if signal == SIGSTOP => {
                            set_trace_options(process)?;
                            resume_tracee(&mut tracees, process, 0)?;
                        }
                        0 => resume_tracee(&mut tracees, process, signal)?,
                        _ => {
                            return Err(process_failure(
                                "unknown ptrace event in proof process tree",
                            ));
                        }
                    }
                } else {
                    let tracee = tracees.remove(&process).ok_or_else(|| {
                        process_failure("terminal event came from an unknown process")
                    })?;
                    if !tracee.saw_exit_event {
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
                thread::sleep(POLL_INTERVAL);
            }
        }
        let verifier_terminal = verifier_terminal
            .ok_or_else(|| process_failure("verifier terminal status is missing"))?;
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
        Err(execution_error) => return Err(reject_and_reap(&tracees, execution_error)),
    };
    drain_to_eof(
        &mut stdout,
        &mut stderr,
        &mut stdout_capture,
        &mut stderr_capture,
        output_limit,
        deadline,
    )?;
    Ok(GeneralGemmRuntimeProcessOutputV2 {
        exit_code: terminal.0,
        signal: terminal.1,
        stdout: stdout_capture.bytes,
        stderr: stderr_capture.bytes,
    })
}

fn set_trace_options(process: i32) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let options = PTRACE_O_TRACEFORK
        | PTRACE_O_TRACEVFORK
        | PTRACE_O_TRACECLONE
        | PTRACE_O_TRACEEXEC
        | PTRACE_O_TRACEEXIT
        | PTRACE_O_TRACESECCOMP
        | PTRACE_O_EXITKILL;
    ptrace(PTRACE_SETOPTIONS, process, options)
}

fn event_child(process: i32) -> Result<i32, GeneralGemmRuntimeClosureErrorV2> {
    let mut child = 0_usize;
    // SAFETY: GETEVENTMSG writes one machine word at the supplied pointer.
    if unsafe {
        linux_ptrace(
            PTRACE_GETEVENTMSG,
            process,
            std::ptr::null_mut(),
            (&raw mut child).cast(),
        )
    } < 0
    {
        return Err(io_process_failure("read ptrace descendant identity"));
    }
    i32::try_from(child).map_err(|_| process_failure("ptrace descendant PID overflow"))
}

fn continue_tracee(process: i32, signal: i32) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    ptrace(PTRACE_CONT, process, signal as usize)
}

fn resume_tracee(
    tracees: &mut BTreeMap<i32, Tracee>,
    process: i32,
    signal: i32,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    continue_tracee(process, signal)?;
    tracees
        .get_mut(&process)
        .ok_or_else(|| process_failure("resumed an unknown proof process"))?
        .stop_consumed = false;
    Ok(())
}

fn ptrace(request: u32, process: i32, data: usize) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    // SAFETY: ptrace interprets null address and the scalar data according to the request.
    if unsafe { linux_ptrace(request, process, std::ptr::null_mut(), data as *mut c_void) } < 0 {
        Err(io_process_failure("operate on traced proof process"))
    } else {
        Ok(())
    }
}

fn wait_for_specific(
    process: i32,
    deadline: Instant,
) -> Result<i32, GeneralGemmRuntimeClosureErrorV2> {
    loop {
        if let Some(status) = wait_for_specific_nonblocking(process)? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(controller_error(
                GeneralGemmRuntimeClosureErrorKindV2::TimedOut,
                "timed out waiting for traced proof process",
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_specific_nonblocking(
    process: i32,
) -> Result<Option<i32>, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    if executable_identity(process)? != expected {
        return Err(process_failure(format!(
            "traced {label} executable identity differs"
        )));
    }
    Ok(())
}

fn executable_identity(process: i32) -> Result<ObjectIdentityV2, GeneralGemmRuntimeClosureErrorV2> {
    let file = File::open(format!("/proc/{process}/exe"))
        .map_err(|_| io_process_failure("open traced executable identity"))?;
    Ok(ObjectSnapshotV2::capture(&file, "traced executable")?.object_identity())
}

fn thread_group_id(process: i32) -> Result<i32, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    validate_exact_descriptor_closure(process, bindings, "rust_verify")
}

fn validate_exact_descriptor_closure(
    process: i32,
    bindings: &[DescriptorBinding],
    label: &str,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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

fn descriptor_numbers(process: i32) -> Result<Vec<i32>, GeneralGemmRuntimeClosureErrorV2> {
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

fn validate_sensitive_syscall_request(
    process: i32,
    allowed: &[AllowedRuntimeExecutableV1],
    validate_mappings: bool,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let registers = read_registers(process)?;
    match u32::try_from(registers.orig_rax) {
        Ok(CLONE3_SYSCALL) => validate_clone3_request(process, &registers),
        Ok(PRCTL_SYSCALL) if registers.rdi == PR_SET_NAME && registers.rsi != 0 => Ok(()),
        Ok(PRCTL_SYSCALL) => Err(process_failure(
            "prctl request is outside exact Rust thread naming",
        )),
        Ok(MMAP_SYSCALL) if validate_mappings => {
            if registers.rdx & PROT_EXEC == 0 {
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
        Ok(MPROTECT_SYSCALL) | Ok(PKEY_MPROTECT_SYSCALL) if validate_mappings => {
            if registers.rdx & PROT_EXEC == 0 {
                return Ok(());
            }
            validate_existing_mapping_range(process, registers.rdi, registers.rsi, allowed)
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
        Ok(MMAP_SYSCALL) | Ok(MPROTECT_SYSCALL) | Ok(PKEY_MPROTECT_SYSCALL) => Ok(()),
        Ok(MREMAP_SYSCALL) | Ok(REMAP_FILE_PAGES_SYSCALL) => Ok(()),
        _ => Err(process_failure(
            "unexpected syscall reached the sensitive-syscall admission checkpoint",
        )),
    }
}

fn validate_clone3_request(
    process: i32,
    registers: &UserRegistersX86_64,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
        && (1..=MAX_CLONE_STACK_BYTES).contains(&stack_size)
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
    if common && (rust_thread || rust_process) {
        Ok(())
    } else {
        Err(process_failure(format!(
            "clone3 request is outside the pinned Rust thread/process ABI: flags={flags:#x} pidfd={pidfd:#x} child_tid={child_tid:#x} parent_tid={parent_tid:#x} exit_signal={exit_signal} stack={stack:#x} stack_size={stack_size:#x} tls={tls:#x} set_tid={set_tid:#x} set_tid_size={set_tid_size} cgroup={cgroup}"
        )))
    }
}

fn read_registers(process: i32) -> Result<UserRegistersX86_64, GeneralGemmRuntimeClosureErrorV2> {
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

fn validate_existing_mapping_range(
    process: i32,
    start: u64,
    length: u64,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    if length == 0 {
        return Err(process_failure(
            "zero-length executable mprotect request is not admitted",
        ));
    }
    let end = start
        .checked_add(length)
        .ok_or_else(|| process_failure("executable mprotect range overflow"))?;
    let maps = std::fs::read_to_string(format!("/proc/{process}/maps"))
        .map_err(|_| io_process_failure("read executable mprotect mappings"))?;
    if maps.len() > 1024 * 1024 {
        return Err(process_failure(
            "executable mprotect map inventory is oversized",
        ));
    }
    let mut cursor = start;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields
            .next()
            .ok_or_else(|| process_failure("malformed executable mprotect map"))?;
        let _permissions = fields
            .next()
            .ok_or_else(|| process_failure("malformed executable mprotect map"))?;
        let (mapping_start, mapping_end) = parse_mapping_range(range)?;
        if mapping_end <= cursor {
            continue;
        }
        if mapping_start > cursor {
            break;
        }
        let mapping_file_offset = fields
            .next()
            .ok_or_else(|| process_failure("malformed executable mprotect map"))?;
        let mapping_file_offset = parse_mapping_file_offset(mapping_file_offset)?;
        let device = fields
            .next()
            .ok_or_else(|| process_failure("malformed executable mprotect map"))?;
        let inode = fields
            .next()
            .ok_or_else(|| process_failure("malformed executable mprotect map"))?;
        let path = fields.next().unwrap_or("");
        if path.is_empty() || path.starts_with('[') {
            return Err(process_failure(
                "executable mprotect covers an anonymous mapping",
            ));
        }
        let covered_end = mapping_end.min(end);
        let covered_offset = mapping_file_offset
            .checked_add(cursor - mapping_start)
            .ok_or_else(|| process_failure("executable mprotect file offset overflow"))?;
        if !mapping_file_range_is_allowed(
            device,
            inode,
            covered_offset,
            covered_end - cursor,
            allowed,
        )? {
            return Err(process_failure(
                "executable mprotect object or file range is outside the retained runtime closure",
            ));
        }
        cursor = covered_end;
        if cursor == end {
            return Ok(());
        }
    }
    Err(process_failure(
        "executable mprotect range is not fully backed by admitted mappings",
    ))
}

fn validate_nonexecutable_mapping_range(
    process: i32,
    start: u64,
    length: u64,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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

fn parse_mapping_range(range: &str) -> Result<(u64, u64), GeneralGemmRuntimeClosureErrorV2> {
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

fn parse_mapping_file_offset(offset: &str) -> Result<u64, GeneralGemmRuntimeClosureErrorV2> {
    u64::from_str_radix(offset, 16)
        .map_err(|_| process_failure("noncanonical process mapping file offset"))
}

fn executable_object_range_is_allowed(
    identity: ObjectIdentityV2,
    offset: u64,
    length: u64,
    allowed: &[AllowedRuntimeExecutableV1],
) -> Result<bool, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<bool, GeneralGemmRuntimeClosureErrorV2> {
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
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let maps = std::fs::read_to_string(format!("/proc/{process}/maps"))
        .map_err(|_| io_process_failure("read traced executable mappings"))?;
    if maps.len() > 1024 * 1024 {
        return Err(process_failure(
            "traced executable map inventory is oversized",
        ));
    }
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
        if !permissions
            .as_bytes()
            .get(2)
            .is_some_and(|value| *value == b'x')
        {
            continue;
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

fn terminate_tree(tracees: &BTreeMap<i32, Tracee>) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let mut failures = Vec::new();
    let mut remaining = tracees
        .iter()
        .map(|(process, tracee)| (*process, tracee.stop_consumed))
        .collect::<BTreeMap<_, _>>();
    for process in remaining.keys().copied() {
        if let Err(error) = kill_tracee(process) {
            failures.push(error);
        }
    }
    for (process, stop_consumed) in &remaining {
        if *stop_consumed && let Err(error) = continue_killed_tracee(*process) {
            failures.push(format!("continue killed PID {process}: {error}"));
        }
    }
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    while !remaining.is_empty() && Instant::now() < deadline {
        let mut reaped = Vec::new();
        let mut discovered = Vec::new();
        for process in remaining.keys().copied().collect::<Vec<_>>() {
            let mut status = 0;
            // SAFETY: waitpid writes one status for the exact ptrace-owned PID.
            let result = unsafe { waitpid(process, &mut status, WAIT_NOHANG | WAIT_WALL) };
            match result {
                0 => {}
                value if value == process && stopped(status) => {
                    let event = (status as u32) >> 16;
                    if matches!(
                        event,
                        PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE
                    ) {
                        match event_child(process) {
                            Ok(child) if !remaining.contains_key(&child) => {
                                if let Err(error) = kill_tracee(child) {
                                    failures.push(error);
                                }
                                discovered.push((child, false));
                            }
                            Ok(_) => {}
                            Err(error) => failures.push(format!(
                                "discover pending descendant from PID {process}: {error}"
                            )),
                        }
                    }
                    if let Err(error) = continue_killed_tracee(process) {
                        failures.push(format!("continue exit-stop PID {process}: {error}"));
                    }
                }
                value if value == process => reaped.push(process),
                _ if io::Error::last_os_error().raw_os_error() == Some(4) => {}
                _ if io::Error::last_os_error().raw_os_error() == Some(10) => {
                    match process_is_live(process) {
                        Ok(false) => reaped.push(process),
                        Ok(true) => {}
                        Err(error) => {
                            failures.push(format!("inspect lost PID {process}: {error}"));
                            reaped.push(process);
                        }
                    }
                }
                _ => {
                    failures.push(format!(
                        "wait PID {process}: {}",
                        io::Error::last_os_error()
                    ));
                    reaped.push(process);
                }
            }
        }
        for process in reaped {
            remaining.remove(&process);
        }
        for (process, stop_consumed) in discovered {
            remaining.entry(process).or_insert(stop_consumed);
        }
        if !remaining.is_empty() {
            thread::sleep(POLL_INTERVAL);
        }
    }
    if !remaining.is_empty() {
        failures.push(format!(
            "timed out reaping rejected proof process IDs {remaining:?}"
        ));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(process_failure(failures.join("; ")))
    }
}

fn kill_tracee(process: i32) -> Result<(), String> {
    // SAFETY: the PID is ptrace-owned and unreaped until the cleanup wait.
    if unsafe { kill(process, SIGKILL) } == 0
        || io::Error::last_os_error().raw_os_error() == Some(3)
    {
        Ok(())
    } else {
        Err(format!(
            "kill PID {process}: {}",
            io::Error::last_os_error()
        ))
    }
}

fn process_is_live(process: i32) -> io::Result<bool> {
    let status = match std::fs::read_to_string(format!("/proc/{process}/stat")) {
        Ok(status) => status,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let state = status
        .rsplit_once(") ")
        .and_then(|(_, suffix)| suffix.as_bytes().first())
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed process stat"))?;
    Ok(!matches!(state, b'Z' | b'X'))
}

fn continue_killed_tracee(process: i32) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    // SAFETY: the tracee is stopped or already gone and only SIGKILL is injected.
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
    tracees: &BTreeMap<i32, Tracee>,
    execution_error: GeneralGemmRuntimeClosureErrorV2,
) -> GeneralGemmRuntimeClosureErrorV2 {
    match terminate_tree(tracees) {
        Ok(()) => execution_error,
        Err(cleanup_error) => process_failure(format!(
            "failed to reap the rejected proof process tree after {execution_error}: {cleanup_error}"
        )),
    }
}

fn make_nonblocking(
    descriptor: &impl std::os::fd::AsFd,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let flags = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|error| io_error("read proof output flags", error))?;
    rustix::fs::fcntl_setfl(descriptor, flags | OFlags::NONBLOCK)
        .map_err(|error| io_error("set proof output nonblocking", error))
}

fn drain(
    pipe: &mut impl Read,
    capture: &mut Capture,
    limit: usize,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let mut buffer = [0_u8; 4096];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                capture.eof = true;
                return Ok(());
            }
            Ok(count) => {
                if capture.bytes.len() > limit.saturating_sub(count) {
                    return Err(controller_error(
                        GeneralGemmRuntimeClosureErrorKindV2::OutputTooLarge,
                        "functional-refinement process exceeded its output bound",
                    ));
                }
                capture.bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(_) => return Err(io_process_failure("read traced proof output")),
        }
    }
}

fn drain_to_eof(
    stdout: &mut ChildStdout,
    stderr: &mut ChildStderr,
    stdout_capture: &mut Capture,
    stderr_capture: &mut Capture,
    limit: usize,
    deadline: Instant,
) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    let grace = deadline.min(Instant::now() + Duration::from_millis(200));
    while (!stdout_capture.eof || !stderr_capture.eof) && Instant::now() < grace {
        drain(stdout, stdout_capture, limit)?;
        drain(stderr, stderr_capture, limit)?;
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

fn process_failure(detail: impl Into<String>) -> GeneralGemmRuntimeClosureErrorV2 {
    controller_error(GeneralGemmRuntimeClosureErrorKindV2::Process, detail)
}

fn io_process_failure(context: &str) -> GeneralGemmRuntimeClosureErrorV2 {
    process_failure(format!("{context}: {}", io::Error::last_os_error()))
}

fn io_error(context: &str, error: rustix::io::Errno) -> GeneralGemmRuntimeClosureErrorV2 {
    controller_error(
        GeneralGemmRuntimeClosureErrorKindV2::Io,
        format!("{context}: {error}"),
    )
}

fn controller_error(
    kind: GeneralGemmRuntimeClosureErrorKindV2,
    detail: impl Into<String>,
) -> GeneralGemmRuntimeClosureErrorV2 {
    GeneralGemmRuntimeClosureErrorV2::new(
        kind,
        format!(
            "functional-refinement process-tree controller: {}",
            detail.into()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HostileRun {
        result: Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2>,
        first_descendant: i32,
        last_descendant: i32,
    }

    fn identity(path: &str) -> ObjectIdentityV2 {
        let file = File::open(path).unwrap();
        ObjectSnapshotV2::capture(&file, "hostile test executable")
            .unwrap()
            .object_identity()
    }

    fn allowed_executable(path: &str) -> AllowedRuntimeExecutableV1 {
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
        let _guard = super::super::RUNTIME_CLOSURE_PROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        FIRST_TEST_DESCENDANT.store(0, Ordering::SeqCst);
        LAST_TEST_DESCENDANT.store(0, Ordering::SeqCst);
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
        let prepared = child_bindings.clone();
        // SAFETY: production uses the same syscall-only pre-exec callback.
        unsafe {
            command.pre_exec(move || prepare_child(&prepared, 2));
        }
        let mut child = crate::executor::spawn_artifact_coordinated_child(&mut command).unwrap();
        let deadline = Instant::now() + deadline_after;
        let result = supervise(
            &mut child,
            &[],
            identity("/bin/sh"),
            identity(expected_solver),
            allowed_mappings,
            validate_mappings,
            false,
            deadline,
            4096,
        );
        drop(duplicates);
        HostileRun {
            result,
            first_descendant: FIRST_TEST_DESCENDANT.load(Ordering::SeqCst),
            last_descendant: LAST_TEST_DESCENDANT.load(Ordering::SeqCst),
        }
    }

    fn assert_process_disappears(process: i32) {
        assert!(process > 0);
        let process_path = format!("/proc/{process}");
        let reap_deadline = Instant::now() + CLEANUP_TIMEOUT;
        while Path::new(&process_path).exists() && Instant::now() < reap_deadline {
            thread::sleep(POLL_INTERVAL);
        }
        assert!(!Path::new(&process_path).exists());
    }

    fn expect_error(
        result: Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2>,
    ) -> GeneralGemmRuntimeClosureErrorV2 {
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
            SECCOMP_RETURN_ALLOW
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
        let output = execute(
            &runtime,
            &source,
            Instant::now() + Duration::from_secs(120),
            4096,
        )
        .unwrap();
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
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
            GeneralGemmRuntimeClosureErrorKindV2::TimedOut,
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
        assert_eq!(setsid.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);

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
            GeneralGemmRuntimeClosureErrorKindV2::Process
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
        assert_eq!(error.kind(), GeneralGemmRuntimeClosureErrorKindV2::Process);
    }
}
