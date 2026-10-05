//! Kernel-enforced output write confinement, not source or device admission.
//!
//! The original stage supplies its retained output directory, never a pathname.
//! Landlock does not revoke inherited/imported descriptors, prevent external
//! writers, or exclude read-only device-open effects. Those remain independent
//! controller obligations. No unsupported ABI or denied installation falls back.

use std::mem::size_of;

pub(crate) const OUTPUT_DESTINATION: i32 = 197;
const REQUIRED_ABI: libc::c_long = 3;
const CREATE_RULESET_VERSION: u32 = 1;
const RULE_PATH_BENEATH: u32 = 1;
const WRITE_FILE: u64 = 1 << 1;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
const MAKE_CHAR: u64 = 1 << 6;
const MAKE_DIR: u64 = 1 << 7;
const MAKE_REG: u64 = 1 << 8;
const MAKE_SOCK: u64 = 1 << 9;
const MAKE_FIFO: u64 = 1 << 10;
const MAKE_BLOCK: u64 = 1 << 11;
const MAKE_SYM: u64 = 1 << 12;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;

// Reads/exec remain subject to the separate runtime policy. Handle all ABI3
// content/creation/removal rights; grant only ordinary output-file operations.
const HANDLED: u64 = WRITE_FILE
    | REMOVE_DIR
    | REMOVE_FILE
    | MAKE_CHAR
    | MAKE_DIR
    | MAKE_REG
    | MAKE_SOCK
    | MAKE_FIFO
    | MAKE_BLOCK
    | MAKE_SYM
    | REFER
    | TRUNCATE;
const ALLOWED: u64 = WRITE_FILE | REMOVE_DIR | REMOVE_FILE | MAKE_DIR | MAKE_REG | REFER | TRUNCATE;

#[repr(C)]
struct Ruleset {
    handled_access_fs: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

pub(crate) const SCRATCH: usize = size_of::<Ruleset>() + size_of::<PathBeneath>() + 256;
// GET_NO_NEW_PRIVS, ABI query, create, add, restrict, close; includes failure close.
pub(crate) const WORK: usize = 6 * (1024 + 64) + 256;

/// Installs only in the owned, single-threaded pre-READY child with NNP set.
/// The source is the already validated original stage binding, above all final
/// destinations and live through this synchronous call. No allocation or callback.
pub(super) unsafe fn install(output: i32) -> bool {
    let ruleset = Ruleset {
        handled_access_fs: HANDLED,
    };
    let path = PathBeneath {
        allowed_access: ALLOWED,
        parent_fd: output,
    };
    // SAFETY: these fixed kernel ABI records are initialized, live and copied
    // synchronously; the owned child alone changes its own Landlock domain.
    // The returned ruleset FD is closed once on every path after acquisition.
    unsafe {
        if output < 0
            || libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1
            || libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<Ruleset>(),
                0_usize,
                CREATE_RULESET_VERSION,
            ) < REQUIRED_ABI
        {
            return false;
        }
        let fd = libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &raw const ruleset,
            size_of::<Ruleset>(),
            0_u32,
        );
        if fd < 0 {
            return false;
        }
        let installed = libc::syscall(
            libc::SYS_landlock_add_rule,
            fd,
            RULE_PATH_BENEATH,
            &raw const path,
            0_u32,
        ) == 0
            && libc::syscall(libc::SYS_landlock_restrict_self, fd, 0_u32) == 0;
        let closed = libc::close(fd as i32) == 0;
        installed && closed
    }
}

#[cfg(test)]
#[path = "native_compiler_filesystem_tests.rs"]
mod tests;
