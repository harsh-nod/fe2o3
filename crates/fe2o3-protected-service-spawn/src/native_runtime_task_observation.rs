//! Borrowed actual-task inspection, selected only by original trace custody.
use super::*;
use crate::native_spawn::ProtectedServiceSpawnStorageV2 as Storage;
use rustix::{fs, io as rio, process};
use std::{fs::File, mem::size_of};
const AUXV_BYTES: usize = 4096;

/// A held event-acquired task and exact stopped-census generation. No constructor,
/// clone, mutable trace, independent PID authority or descriptor trait is exposed.
/// Exported bytes/files are inert and require caller validation and full funding.
pub struct RuntimeTaskObservationV1<'owner, 'work> {
    owner: &'owner RootRuntimeTraceV1<'work>,
    index: usize,
    pid: Pid,
    stop: Stop,
    generation: u64,
    selected: bool,
}

impl RootRuntimeTraceV1<'_> {
    /// Callback cannot outlive the immutable trace borrow. The selected owned
    /// task, exact stop generation and complete stopped census are checked both
    /// before and after; no caller PID or record selects the target.
    pub fn with_selected_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &RuntimeTaskObservationV1<'_, '_>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<Error>,
    {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            RuntimeTaskObservationV1::VIEW_WORK,
            RuntimeTaskObservationV1::FRAME,
            |b| {
                let (index, task) = self.selected()?;
                let view = RuntimeTaskObservationV1 {
                    owner: self,
                    index,
                    pid: task.pid,
                    stop: task.stop,
                    generation: self.generation,
                    selected: true,
                };
                view.check(b)?;
                let result = operation(&view, b)?;
                view.check(b)?;
                Ok(result)
            },
        )
    }
}

impl RootRuntimeTraceV1<'_> {
    /// Traverse only the original controller's reconciled stopped census. The
    /// callback may inspect each actual task before any application resume;
    /// no caller-supplied PID, index or purported task record is accepted.
    pub fn for_each_task_observation<'budget, E>(
        &self,
        b: &mut Budget<'budget>,
        mut operation: impl FnMut(
            &RuntimeTaskObservationV1<'_, '_>,
            &mut Budget<'budget>,
        ) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E>
    where
        E: From<Resource> + From<Error>,
    {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            RuntimeTaskObservationV1::CENSUS_WORK,
            RuntimeTaskObservationV1::FRAME,
            |b| {
                self.check(b, true)?;
                for (index, task) in self.tasks.iter().enumerate() {
                    let Some(task) = task else {
                        continue;
                    };
                    let view = RuntimeTaskObservationV1 {
                        owner: self,
                        index,
                        pid: task.pid,
                        stop: task.stop,
                        generation: self.generation,
                        selected: false,
                    };
                    view.check(b)?;
                    operation(&view, b)?;
                    view.check(b)?;
                }
                Ok(())
            },
        )
    }
}

impl RuntimeTaskObservationV1<'_, '_> {
    pub const VIEW_WORK: usize = ENTRY + 8 * 1088;
    /// Complete fixed traversal charge, excluding each caller policy callback.
    pub const CENSUS_WORK: usize = ENTRY + CAPACITY * Self::VIEW_WORK;
    pub const DESCRIPTOR_WORK: usize = ENTRY + 12 * 1088;
    pub const FRAME: usize = 4096 + 4 * size_of::<File>() + 2 * size_of::<fs::StatFs>();
    pub const MAPS_WORK: usize =
        ENTRY + crate::trace_runtime::MAX_MAP_BYTES * 64 + (2048 + 16) * 1088;
    pub const MAPS_SCRATCH: usize =
        Self::FRAME + crate::trace_runtime::MAX_MAP_BYTES + size_of::<(Vec<u8>, Storage)>();
    pub const PERSONALITY_WORK: usize = ENTRY + 12 * 1088;
    pub const PERSONALITY_SCRATCH: usize = Self::FRAME + 128;
    pub const AUXV_BYTES: usize = AUXV_BYTES;
    pub const AUXV_WORK: usize = ENTRY + Self::AUXV_BYTES * 64 + 16 * 1088;
    pub const MEMORY_BYTES: usize = 64 * 1024;
    pub const MEMORY_WORK: usize = ENTRY + Self::MEMORY_BYTES * 64 + 16 * 1088;

    /// Context only. The controller's unreaped custody, not this number, binds it.
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// Inert exact-stop classification. Subsequent reads still revalidate the
    /// original stopped generation; this does not authenticate executable bytes.
    pub fn is_exec_boundary(&self) -> bool {
        self.stop == Stop::Exec
    }

    fn check(&self, b: &Budget<'_>) -> Result<()> {
        self.owner.check(b, true)?;
        if self.owner.generation != self.generation
            || self.owner.census_generation != Some(self.generation)
            || (self.selected && self.owner.selected != Some(self.index))
            || !self.owner.tasks[self.index].is_some_and(|t| {
                t.pid == self.pid
                    && t.stop == self.stop
                    && matches!(
                        t.stop,
                        Stop::Exec | Stop::Seccomp | Stop::Syscall | Stop::Interrupt
                    )
            })
        {
            return Err(Error::State(
                "runtime task observation lost its stopped generation",
            ));
        }
        let mut registers = MaybeUninit::<libc::user_regs_struct>::uninit();
        request(
            Request::Registers,
            self.pid,
            0,
            registers.as_mut_ptr() as usize,
        )?;
        Ok(())
    }

    /// Actual file-table equality is required before borrowing the root pidfd.
    /// Otherwise the transient pidfd is resolved only while this event-acquired
    /// task remains unreaped and stopped; unsupported private-table threads refuse.
    /// The returned file charge is FULL and UNRESERVED on the original account.
    pub fn duplicate_descriptor(
        &self,
        descriptor: i32,
        b: &mut Budget<'_>,
    ) -> Result<(File, Storage)> {
        b.with_prepaid_scope(
            self.owner.retained,
            ENTRY,
            Self::DESCRIPTOR_WORK,
            Self::FRAME,
            |b| {
                self.check(b)?;
                if descriptor < 0 {
                    return Err(Error::State("negative runtime descriptor"));
                }
                let root_retained = self.owner.tasks[0].is_some_and(|task| task.stop.parked());
                let same = self.index == 0 || (root_retained && self.same_root_file_table()?);
                let file = if same {
                    File::from(
                        process::pidfd_getfd(
                            self.owner.root().child.pidfd()?,
                            descriptor,
                            process::PidfdGetfdFlags::empty(),
                        )
                        .map_err(|e| io("duplicate shared runtime descriptor", e))?,
                    )
                } else {
                    let pidfd = process::pidfd_open(self.pid, process::PidfdFlags::empty())
                        .map_err(|e| io("open pidfd for retained stopped task", e))?;
                    File::from(
                        process::pidfd_getfd(&pidfd, descriptor, process::PidfdGetfdFlags::empty())
                            .map_err(|e| io("duplicate retained runtime descriptor", e))?,
                    )
                };
                self.check(b)?;
                Ok((file, Storage(size_of::<(File, Storage)>())))
            },
        )
    }

    fn same_root_file_table(&self) -> Result<bool> {
        // SAFETY: both identities come solely from this same original trace's
        // unreaped stopped roster. KCMP_FILES reads actual kernel sharing; no
        // pointer, descriptor ownership or trace authority is imported.
        let result = unsafe {
            libc::syscall(
                libc::SYS_kcmp,
                self.owner.root().pid().as_raw_nonzero().get(),
                self.pid.as_raw_nonzero().get(),
                2_u32,
                0_usize,
                0_usize,
            )
        };
        if result == -1 {
            let errno = std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO);
            return Err(io(
                "compare retained runtime file tables",
                rio::Errno::from_raw_os_error(errno),
            ));
        }
        Ok(result == 0)
    }

    /// Exact bounded personality bytes from this stopped task's procfs entry.
    /// Apply the shared validate_personality policy separately; these bytes are
    /// not executable-image admission or a runtime guard.
    pub fn read_personality(&self, b: &mut Budget<'_>) -> Result<([u8; 64], usize)> {
        b.with_prepaid_scope(
            self.owner.retained,
            ENTRY,
            Self::PERSONALITY_WORK,
            Self::PERSONALITY_SCRATCH,
            |b| {
                self.check(b)?;
                let file = self.open_proc("personality")?;
                let mut bytes = [0; 64];
                let count = rio::read(&file, &mut bytes)
                    .map_err(|e| io("read retained task personality", e))?;
                let mut extra = [0; 1];
                if count == 0
                    || rio::read(&file, &mut extra)
                        .map_err(|e| io("finish retained task personality", e))?
                        != 0
                {
                    return Err(Error::State("runtime personality byte bound exceeded"));
                }
                self.check(b)?;
                Ok((bytes, count))
            },
        )
    }

    /// Actual kernel-saved auxiliary vector of this same held task. No pathname,
    /// supplied descriptor or caller byte string can substitute for this read.
    pub fn read_auxv(&self, b: &mut Budget<'_>) -> Result<([u8; AUXV_BYTES], usize)> {
        b.with_prepaid_scope(
            self.owner.retained,
            ENTRY,
            Self::AUXV_WORK,
            Self::FRAME + Self::AUXV_BYTES + 16,
            |b| {
                self.check(b)?;
                let file = self.open_proc("auxv")?;
                let mut bytes = [0; AUXV_BYTES];
                let count =
                    rio::read(&file, &mut bytes).map_err(|e| io("read retained task auxv", e))?;
                let mut extra = [0; 1];
                if count == 0
                    || rio::read(&file, &mut extra)
                        .map_err(|e| io("finish retained task auxv", e))?
                        != 0
                {
                    return Err(Error::State("runtime auxiliary-vector byte bound exceeded"));
                }
                self.check(b)?;
                Ok((bytes, count))
            },
        )
    }

    /// Copy bounded bytes from the actual held task, without exporting a proc
    /// descriptor or accepting a PID. The address is data, not image authority.
    /// The caller retains/funds the output buffer; short reads fail closed.
    pub fn read_memory(&self, address: u64, out: &mut [u8], b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.owner.retained,
            ENTRY,
            Self::MEMORY_WORK,
            Self::FRAME,
            |b| {
                self.check(b)?;
                if out.is_empty()
                    || out.len() > Self::MEMORY_BYTES
                    || address.checked_add(out.len() as u64).is_none()
                {
                    return Err(Error::State(
                        "runtime memory observation exceeds bounded range",
                    ));
                }
                let file = self.open_proc("mem")?;
                if rio::pread(&file, &mut *out, address)
                    .map_err(|e| io("read retained task memory", e))?
                    != out.len()
                {
                    return Err(Error::State("runtime memory observation was short"));
                }
                self.check(b)?;
                Ok(())
            },
        )
    }

    fn open_proc(&self, member: &str) -> Result<std::os::fd::OwnedFd> {
        if !matches!(member, "maps" | "personality" | "auxv" | "mem") {
            return Err(Error::State("unsupported runtime proc member"));
        }
        let path = format!("/proc/{}/{member}", self.pid.as_raw_nonzero().get());
        let file = fs::open(
            path.as_str(),
            fs::OFlags::RDONLY | fs::OFlags::CLOEXEC | fs::OFlags::NOFOLLOW,
            fs::Mode::empty(),
        )
        .map_err(|e| io("open retained task proc observation", e))?;
        if fs::fstatfs(&file)
            .map_err(|e| io("inspect retained proc filesystem", e))?
            .f_type as u64
            != 0x9fa0
        {
            return Err(Error::State("runtime task observation is not procfs"));
        }
        Ok(file)
    }

    /// Read the selected task's actual procfs maps with a strict byte/read bound.
    /// No caller path, proc file or process identity is accepted. Full returned
    /// vector capacity must be reserved by the caller; it is not mapping admission.
    pub fn read_maps(&self, b: &mut Budget<'_>) -> Result<(Vec<u8>, Storage)> {
        b.with_prepaid_scope(
            self.owner.retained,
            ENTRY,
            Self::MAPS_WORK,
            Self::MAPS_SCRATCH,
            |b| {
                self.check(b)?;
                let file = self.open_proc("maps")?;
                let limit = crate::trace_runtime::MAX_MAP_BYTES;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(limit)
                    .map_err(|_| Error::State("mapping census allocation refused"))?;
                if bytes.capacity() > limit {
                    return Err(Error::State("mapping census capacity exceeded"));
                }
                let mut chunk = [0_u8; 1024];
                for _ in 0..2048 {
                    let count = rio::read(&file, &mut chunk)
                        .map_err(|e| io("read retained mapping census", e))?;
                    if count == 0 {
                        self.check(b)?;
                        let storage = bytes
                            .capacity()
                            .checked_add(size_of::<(Vec<u8>, Storage)>())
                            .ok_or(Resource::Arithmetic)?;
                        return Ok((bytes, Storage(storage)));
                    }
                    if bytes
                        .len()
                        .checked_add(count)
                        .is_none_or(|length| length > limit)
                    {
                        return Err(Error::State("mapping census byte bound exceeded"));
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                Err(Error::State("mapping census read bound exceeded"))
            },
        )
    }
}
