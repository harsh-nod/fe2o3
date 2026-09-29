//! Installation of an already owned wrapper capture, never a late global capture.
//!
//! The caller must obtain CapturedStdioV1 from the actual wrapper entry before
//! wrapper setup opens descriptors. At Rust main this observes runtime-sanitized
//! stdslots, not the original pre-runtime inherited state. Single-threaded entry
//! alone cannot justify capture_current: its unsafe contract also excludes shared
//! status-flag mutation through external OFD aliases. The entry adapter must
//! establish that exclusion; no immutable-alias isolation is claimed here. No
//! raw-FD, pathname, environment, or root-process stdio substitutes are accepted.

use fe2o3_process_identity::CapturedStdioV1;
use rustix::{fs::OFlags, io::FdFlags};
use std::{
    io,
    os::{
        fd::{AsRawFd, OwnedFd, RawFd},
        unix::process::CommandExt,
    },
    process::{Command, Stdio},
};

// The selected binding-wrapper hooks install capabilities only below this floor.
// Their destinations must never overwrite our retained sources in the child.
const SOURCE_FLOOR: RawFd =
    fe2o3_compiler_closure_capability::COMPILER_EXECUTION_POLICY_CHILD_FD_V1 + 1;
const _: () = assert!(SOURCE_FLOOR > crate::ARTIFACT_CHILD_FD);
const _: () = assert!(SOURCE_FLOOR > crate::BACKEND_CHILD_FD);
const _: () = assert!(SOURCE_FLOOR > crate::RUSTC_INVOCATION_CHILD_FD);
const _: () = assert!(SOURCE_FLOOR > crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(SOURCE_FLOOR > crate::RUSTC_CHILD_FD);
const _: () =
    assert!(SOURCE_FLOOR > fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1);
const _: () =
    assert!(SOURCE_FLOOR > fe2o3_compiler_execution_client::COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);

struct StagedDescriptor {
    source: OwnedFd,
    status_flags: OFlags,
}

/// Checks the selected wrapper's entry/setup precondition without filling holes.
/// Call before its first FD open and again before staging. Otherwise pinning may
/// save /proc/self/fd/0 for rustc, then lose that executable when the final stdio
/// hook replaces fd0. These three scalar probes neither capture stdio identity
/// nor prove concurrent-slot exclusion; the entry adapter must exclude mutation
/// throughout setup/spawn, and the wrapper itself does not close parent slots.
pub(crate) fn require_open_parent_stdio() -> io::Result<()> {
    for fd in 0..=2 {
        // SAFETY: scalar inspection does not borrow an absent or reused slot.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Adds the final stdio hook to the selected wrapper's prepared Command.
///
/// Command owns up to three independent CLOEXEC duplicates through spawn/drop;
/// the original capture can move to parent custody or be dropped. Original
/// absence and original CLOEXEC both close the child slot before exec; only the
/// original capture retains their distinct pre-exec states. Status flags and
/// offsets remain shared with the captured OFDs; neither is reset. Revalidation
/// detects drift, not hostile aliases or a historical identity join. No authority
/// is granted.
///
/// Call after all other wrapper hooks and immediately before spawn. The wrapper
/// must keep its current parent slots open across spawn: Rust Command allocates
/// its private error channel before running hooks. An absent parent slot could
/// otherwise become that channel and then be overwritten here. This check is
/// only a launch precondition; it never supplies the child's stdio identity.
/// The selected wrapper does not close/reassign parent slots or install handlers
/// doing so between this check and spawn. Additional hooks must not overwrite
/// retained sources or stdio. Arbitrary Command customization is not supported.
///
/// Preparation performs at most 12 descriptor syscalls (three parent-slot probes,
/// six capture checks, three duplicates); the hook performs at most nine (six
/// staged-source checks, three installs/closes). Partial failure closes up to
/// three duplicates. Fixed storage; no reads, writes, opens, status mutation or
/// retries. The caller prepays these descriptors, cleanup, each hook execution
/// and Command's hook allocation/lifetime on the enclosing attempt's account.
pub(crate) fn configure_captured_stdio(
    command: &mut Command,
    capture: &CapturedStdioV1,
) -> io::Result<()> {
    require_open_parent_stdio()?;
    capture.revalidate()?;
    let mut staged: [Option<StagedDescriptor>; 3] = [None, None, None];
    for (slot, captured) in
        staged
            .iter_mut()
            .zip([capture.stdin(), capture.stdout(), capture.stderr()])
    {
        if let Some(captured) =
            captured.filter(|slot| !slot.descriptor_flags().contains(FdFlags::CLOEXEC))
        {
            *slot = Some(StagedDescriptor {
                source: rustix::io::fcntl_dupfd_cloexec(captured.source(), SOURCE_FLOOR)?,
                status_flags: captured.status_flags(),
            });
        }
    }
    // All fallible parent preparation precedes Command mutation.
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // SAFETY: Command owns every source; all are above stdio and the selected
    // wrapper's capability destinations. The hook only performs bounded scalar
    // descriptor syscalls, with no allocation, locking or retry.
    unsafe {
        command.pre_exec(move || {
            for slot in staged.iter().flatten() {
                if rustix::io::fcntl_getfd(&slot.source)? != FdFlags::CLOEXEC
                    || rustix::fs::fcntl_getfl(&slot.source)? != slot.status_flags
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
            }
            for (destination, slot) in (0..=2).zip(&staged) {
                match slot {
                    Some(slot) => {
                        if libc::dup3(slot.source.as_raw_fd(), destination, 0) < 0 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    None => {
                        if libc::close(destination) < 0 {
                            let error = io::Error::last_os_error();
                            if error.raw_os_error() != Some(libc::EBADF) {
                                return Err(error);
                            }
                        }
                    }
                }
            }
            Ok(())
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "inert_rustc_stdio_capture_tests.rs"]
mod tests;
