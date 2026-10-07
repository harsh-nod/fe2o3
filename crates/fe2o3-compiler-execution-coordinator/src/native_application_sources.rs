//! Closed pathname acquisition only; native admission still reads and seals every source.
use crate::native_inherited::{self as root, Result};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::{FileType, Mode, OFlags};
use std::fs::File;

pub(super) const WORK: usize = 8 + 512 * 1024;
pub(super) const SCRATCH: usize = 64 * 1024;
const PATHS: [&str; 14] = [
    "/run/fe2o3",
    "/var/lib/fe2o3/compiler-execution",
    "/var/lib/fe2o3/external-anchor",
    "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3",
    "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher",
    "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional",
    "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3",
    "/usr/libexec/fe2o3/fe2o3-external-anchor-service-v3",
    "/etc/fe2o3/compiler-execution/supervisor-deployment-v3",
    "/etc/fe2o3/compiler-execution/issuer-policy-v3",
    "/etc/fe2o3/compiler-execution/anchor-deployment-v3",
    "/etc/fe2o3/compiler-execution/anchor-provisioning-v3",
    "/etc/fe2o3/compiler-execution/issuer-signing-key-seed-v3",
    "/etc/fe2o3/compiler-execution/anchor-signing-key-seed-v3",
];
const FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW);

fn io(
    operation: &'static str,
    source: rustix::io::Errno,
) -> root::CompilerExecutionRootDeploymentErrorV2 {
    root::CompilerExecutionRootDeploymentErrorV2::Io { operation, source }
}

fn parent(file: &File) -> Result<()> {
    let s =
        rustix::fs::fstat(file).map_err(|e| io("inspect native application source parent", e))?;
    if FileType::from_raw_mode(s.st_mode) != FileType::Directory
        || s.st_uid != 0
        || s.st_gid != 0
        || s.st_mode & 0o022 != 0
        || s.st_mode & 0o100 == 0
        || s.st_nlink == 0
    {
        return Err(root::invalid(
            "native application source",
            "parent is not root-controlled",
        ));
    }
    if rustix::fs::flistxattr(file, &mut [0u8; 1])
        .map_err(|e| io("inspect native application parent attributes", e))?
        != 0
    {
        return Err(root::invalid(
            "native application source",
            "parent has extended attributes",
        ));
    }
    Ok(())
}

fn open_one(index: usize, limits: &[usize; 11]) -> Result<File> {
    let path = PATHS[index];
    let mut at = File::from(
        rustix::fs::open("/", FLAGS | OFlags::DIRECTORY, Mode::empty())
            .map_err(|e| io("open native application source root", e))?,
    );
    let mut parts = path.split('/').skip(1).peekable();
    let mut steps = 0;
    while let Some(name) = parts.next() {
        steps += 1;
        if steps > 8 || name.is_empty() || name == "." || name == ".." {
            return Err(root::invalid(
                "native application source",
                "closed path roster is invalid",
            ));
        }
        parent(&at)?;
        let directory = parts.peek().is_some() || index < 3;
        let flags = FLAGS
            | if directory {
                OFlags::DIRECTORY
            } else {
                OFlags::NONBLOCK
            };
        at = File::from(
            rustix::fs::openat(&at, name, flags, Mode::empty())
                .map_err(|e| io("open native application source edge", e))?,
        );
    }
    let s = rustix::fs::fstat(&at).map_err(|e| io("inspect native application source shape", e))?;
    root::check_source_shape(index, limits, &s)?;
    // Leaf ownership, exact mode, attributes and byte stability are checked by
    // the existing native role-specific admission before keys or children exist.
    Ok(at)
}

pub(super) fn open(limits: &[usize; 11], b: &mut Budget<'_>) -> Result<[File; 14]> {
    b.with_prepaid_scope(14 * root::FILE_STORAGE, 8, WORK, SCRATCH, |_| {
        crate::native::require_root()?;
        root::require_single_threaded()?;
        if rustix::thread::gettid().as_raw_pid() != rustix::process::getpid().as_raw_pid() {
            return Err(root::invalid(
                "native application source",
                "intake is not on the main thread",
            ));
        }
        // No inherited slot adoption, descriptor rewriting, path override or retry.
        Ok([
            open_one(0, limits)?,
            open_one(1, limits)?,
            open_one(2, limits)?,
            open_one(3, limits)?,
            open_one(4, limits)?,
            open_one(5, limits)?,
            open_one(6, limits)?,
            open_one(7, limits)?,
            open_one(8, limits)?,
            open_one(9, limits)?,
            open_one(10, limits)?,
            open_one(11, limits)?,
            open_one(12, limits)?,
            open_one(13, limits)?,
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_sources_are_exact_distinct_native_roles() {
        for (i, path) in PATHS.iter().enumerate() {
            assert!(path.starts_with('/'));
            assert!(path.len() < 128);
            assert!(path.split('/').skip(1).count() <= 8);
            assert!(!PATHS[..i].contains(path));
        }
        assert!(PATHS[5].ends_with("-issuer-conditional"));
        assert!(PATHS[8..].iter().all(|p| p.ends_with("-v3")));
    }

    #[test]
    fn native_application_ingress_is_not_the_compiler_socket() {
        use fe2o3_compiler_execution_protocol::{
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1 as COMPILER,
            NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3 as APPLICATION,
        };
        assert_ne!(APPLICATION, COMPILER);
        assert_eq!(APPLICATION, "/run/fe2o3/native-application-supervisor.sock");
    }
}
