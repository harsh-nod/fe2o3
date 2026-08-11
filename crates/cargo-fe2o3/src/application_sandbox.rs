//! Fail-closed Linux launch profile for descriptor-bearing applications.

use std::io;

const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
const BPF_LOAD_WORD_ABSOLUTE: u16 = 0x20;
const BPF_JUMP_EQUAL: u16 = 0x15;
const BPF_RETURN: u16 = 0x06;
const SECCOMP_DATA_NUMBER_OFFSET: u32 = 0;
const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const SECCOMP_SET_MODE_FILTER: libc::c_uint = 1;

pub(crate) fn no_fork_application_filter() -> Vec<libc::sock_filter> {
    let allowed = allowed_application_syscalls();
    let mut filter = Vec::with_capacity(5 + allowed.len() * 2);
    filter.push(statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_DATA_ARCH_OFFSET));
    filter.push(jump(BPF_JUMP_EQUAL, AUDIT_ARCH_X86_64, 1, 0));
    filter.push(statement(BPF_RETURN, SECCOMP_RET_KILL_PROCESS));
    filter.push(statement(
        BPF_LOAD_WORD_ABSOLUTE,
        SECCOMP_DATA_NUMBER_OFFSET,
    ));
    for syscall in allowed {
        filter.push(jump(BPF_JUMP_EQUAL, *syscall as u32, 0, 1));
        filter.push(statement(BPF_RETURN, SECCOMP_RET_ALLOW));
    }
    filter.push(statement(
        BPF_RETURN,
        SECCOMP_RET_ERRNO | libc::EPERM as u32,
    ));
    filter
}

/// Installs a permanent, single-threaded profile. It has to run after descriptor setup and before
/// the first application `execve`; both `no_new_privs` and seccomp survive that exec.
pub(crate) fn install_no_fork_application_profile(filter: &[libc::sock_filter]) -> io::Result<()> {
    let length =
        u16::try_from(filter.len()).map_err(|_| io::Error::from_raw_os_error(libc::E2BIG))?;
    // SAFETY: the documented scalar `PR_SET_NO_NEW_PRIVS` arguments affect only this pre-exec
    // child. Failure is propagated to `Command::spawn`, so launch remains fail closed.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let program = libc::sock_fprog {
        len: length,
        filter: filter.as_ptr().cast_mut(),
    };
    // SAFETY: `program` and its immutable filter storage remain live for the syscall. Seccomp
    // copies the filter into the kernel before returning.
    if unsafe { libc::syscall(libc::SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0_u32, &program) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

const fn statement(code: u16, value: u32) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k: value,
    }
}

const fn jump(code: u16, value: u32, on_equal: u8, on_other: u8) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: on_equal,
        jf: on_other,
        k: value,
    }
}

fn allowed_application_syscalls() -> &'static [libc::c_long] {
    &[
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_pread64,
        libc::SYS_pwrite64,
        libc::SYS_close,
        libc::SYS_lseek,
        libc::SYS_fstat,
        libc::SYS_newfstatat,
        libc::SYS_statx,
        libc::SYS_access,
        libc::SYS_faccessat,
        libc::SYS_faccessat2,
        libc::SYS_openat,
        libc::SYS_openat2,
        libc::SYS_getdents64,
        libc::SYS_getcwd,
        libc::SYS_chdir,
        libc::SYS_fchdir,
        libc::SYS_readlink,
        libc::SYS_readlinkat,
        libc::SYS_unlink,
        libc::SYS_unlinkat,
        libc::SYS_rename,
        libc::SYS_renameat,
        libc::SYS_renameat2,
        libc::SYS_mkdir,
        libc::SYS_mkdirat,
        libc::SYS_rmdir,
        libc::SYS_link,
        libc::SYS_linkat,
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        libc::SYS_umask,
        libc::SYS_fcntl,
        libc::SYS_flock,
        libc::SYS_fsync,
        libc::SYS_fdatasync,
        libc::SYS_dup,
        libc::SYS_dup2,
        libc::SYS_dup3,
        libc::SYS_ioctl,
        libc::SYS_poll,
        libc::SYS_ppoll,
        libc::SYS_select,
        libc::SYS_pselect6,
        libc::SYS_epoll_create1,
        libc::SYS_epoll_ctl,
        libc::SYS_epoll_wait,
        libc::SYS_epoll_pwait,
        libc::SYS_mmap,
        libc::SYS_mprotect,
        libc::SYS_munmap,
        libc::SYS_mremap,
        libc::SYS_madvise,
        libc::SYS_brk,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_futex,
        libc::SYS_sched_yield,
        libc::SYS_sched_getaffinity,
        libc::SYS_nanosleep,
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_gettimeofday,
        libc::SYS_getrandom,
        libc::SYS_getpid,
        libc::SYS_getppid,
        libc::SYS_gettid,
        libc::SYS_getuid,
        libc::SYS_geteuid,
        libc::SYS_getgid,
        libc::SYS_getegid,
        libc::SYS_getrusage,
        libc::SYS_wait4,
        libc::SYS_uname,
        libc::SYS_sysinfo,
        libc::SYS_arch_prctl,
        libc::SYS_set_tid_address,
        libc::SYS_set_robust_list,
        libc::SYS_rseq,
        libc::SYS_prlimit64,
        libc::SYS_execve,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_session_namespace_and_io_uring_creation_are_not_allowlisted() {
        let allowed = allowed_application_syscalls();
        for forbidden in [
            libc::SYS_fork,
            libc::SYS_vfork,
            libc::SYS_clone,
            libc::SYS_clone3,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_setsid,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
        ] {
            assert!(!allowed.contains(&forbidden), "syscall {forbidden} escaped");
        }
    }

    #[test]
    fn filter_kills_foreign_architectures_and_denies_unknown_syscalls() {
        let filter = no_fork_application_filter();
        assert_eq!(filter[0].k, SECCOMP_DATA_ARCH_OFFSET);
        assert_eq!(filter[1].k, AUDIT_ARCH_X86_64);
        assert_eq!(filter[2].k, SECCOMP_RET_KILL_PROCESS);
        assert_eq!(
            filter.last().unwrap().k,
            SECCOMP_RET_ERRNO | libc::EPERM as u32
        );
    }
}
