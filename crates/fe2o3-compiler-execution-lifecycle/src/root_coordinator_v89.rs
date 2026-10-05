use super::*;

/// Closed state-root names accepted by the root coordinator's lifecycle admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionLifecycleRootV89 {
    /// The protected compiler supervisor's existing service-owned state directory.
    Supervisor,
    /// The external-anchor service's existing service-owned state directory.
    ExternalAnchor,
}

impl CompilerExecutionLifecycleRootV89 {
    fn name(self) -> &'static str {
        match self {
            Self::Supervisor => "compiler-execution",
            Self::ExternalAnchor => "external-anchor",
        }
    }
}

impl CompilerExecutionServiceLifecycleLeaseV1 {
    /// Opens an independent lifecycle lease without searching inside a service-owned root.
    ///
    /// The canonical root-owned parent and its named child must agree with the inherited state
    /// descriptor before and after admission. This does not grant service, compiler, or launch
    /// authority; subsequent state-root admission and service identity checks remain mandatory.
    pub fn open_for_root_coordinator_v89(
        state_root: &impl AsFd,
        role: CompilerExecutionLifecycleRootV89,
    ) -> Result<Self, LifecycleLeaseErrorV1> {
        let parent = open_canonical_parent()?;
        let before = check_root(&parent, state_root, role.name())?;
        let lease = Self::open_from_parent(parent, ROOT_ID_V1, ROOT_ID_V1)?;
        let current = open_canonical_parent()?;
        if snapshot(&fstat(&current).map_err(|e| io_error("inspect current lifecycle parent", e))?)
            != snapshot(
                &fstat(&lease.parent)
                    .map_err(|e| io_error("inspect retained lifecycle parent", e))?,
            )
            || check_root(&current, state_root, role.name())? != before
            || check_root(&lease.parent, state_root, role.name())? != before
        {
            return Err(LifecycleLeaseErrorV1::PathChanged);
        }
        lease.revalidate()?;
        Ok(lease)
    }
}

fn open_canonical_parent() -> Result<File, LifecycleLeaseErrorV1> {
    let mut parent = File::from(
        rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| io_error("open canonical filesystem root", e))?,
    );
    validate_parent(&parent, ROOT_ID_V1, ROOT_ID_V1)?;
    for name in ["var", "lib", "fe2o3"] {
        parent = File::from(
            openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| io_error("open canonical lifecycle parent component", e))?,
        );
        validate_parent(&parent, ROOT_ID_V1, ROOT_ID_V1)?;
    }
    Ok(parent)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateSnapshot {
    object: ObjectSnapshotV1,
    mtime: (i64, u64),
    ctime: (i64, u64),
}

fn state_snapshot(fd: &impl AsFd) -> Result<StateSnapshot, LifecycleLeaseErrorV1> {
    let value = fstat(fd).map_err(|e| io_error("inspect inherited state-root identity", e))?;
    Ok(StateSnapshot {
        object: snapshot(&value),
        mtime: (value.st_mtime, value.st_mtime_nsec),
        ctime: (value.st_ctime, value.st_ctime_nsec),
    })
}

fn check_root(
    parent: &File,
    state_root: &impl AsFd,
    name: &str,
) -> Result<StateSnapshot, LifecycleLeaseErrorV1> {
    let flags = rustix::fs::fcntl_getfl(state_root)
        .map_err(|e| io_error("inspect inherited state-root access", e))?;
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || flags.contains(OFlags::PATH)
        || rustix::io::fcntl_getfd(state_root)
            .map_err(|e| io_error("inspect inherited state-root descriptor flags", e))?
            != rustix::io::FdFlags::CLOEXEC
    {
        return Err(LifecycleLeaseErrorV1::InvalidParent);
    }
    let before = state_snapshot(state_root)?;
    if FileType::from_raw_mode(before.object.mode) != FileType::Directory
        || before.object.mode & 0o7777 != 0o700
        || before.object.links == 0
    {
        return Err(LifecycleLeaseErrorV1::InvalidParent);
    }
    // O_PATH resolves only the directory entry; it does not require read/search of its contents.
    let named = openat(
        parent,
        name,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| io_error("join canonical service root to inherited descriptor", e))?;
    if state_snapshot(&named)? != before || state_snapshot(state_root)? != before {
        return Err(LifecycleLeaseErrorV1::PathChanged);
    }
    Ok(before)
}

#[cfg(test)]
#[path = "root_coordinator_v89_tests.rs"]
mod tests;
