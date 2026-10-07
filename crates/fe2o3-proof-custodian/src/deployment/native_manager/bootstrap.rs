//! One pre-service re-exec, separate from every application lifecycle account.
use super::*;
use std::{convert::Infallible, os::fd::AsRawFd};

impl<'root, 'work> Deployment<'root, 'work> {
    /// Classifies only the initial childless entry. `true` requires the actual
    /// running file to be the original installed manager inode with the approved
    /// bytes. `false` is not admission: the caller must then consume this owner
    /// through `admit_running`, and any failure is terminal rather than a retry.
    pub fn initial_bootstrap_required(&self, budget: &mut Budget<'work>) -> io::Result<bool> {
        let floor = budget.storage();
        budget.charge_work(64 * 1024).map_err(other)?;
        budget.reserve_storage(4096).map_err(other)?;
        self.revalidate(budget)?;
        require(
            self.pid == self.tid,
            "manager entry is not on the main thread",
        )?;
        require_childless_single_thread()?;
        let procfs = File::from(rustix::fs::open(
            "/proc",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        require(
            rustix::fs::fstatfs(&procfs)?.f_type as i64 == libc::PROC_SUPER_MAGIC as i64,
            "manager executable view is not procfs",
        )?;
        let running = File::from(rustix::fs::openat(
            &procfs,
            "self/exe",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        let sealed = match rustix::fs::fcntl_get_seals(&running) {
            Ok(seals) => seals.contains(
                rustix::fs::SealFlags::SEAL
                    | rustix::fs::SealFlags::WRITE
                    | rustix::fs::SealFlags::GROW
                    | rustix::fs::SealFlags::SHRINK,
            ),
            Err(rustix::io::Errno::INVAL) => false,
            Err(error) => return Err(error.into()),
        };
        if !sealed {
            require_same_installed_image(&running, self.image_tree.leaf())?;
            let measurement = self.config.measurement()?;
            budget
                .reserve_storage(Image::file_storage(measurement).map_err(other)?)
                .map_err(other)?;
            let (measured, charge) = Image::seal_source_for_owner(
                running,
                measurement,
                Owner::new(0, 0).map_err(other)?,
                "initial installed native manager entry",
                budget,
            )
            .map_err(other)?;
            budget
                .reserve_storage(charge.additional_storage())
                .map_err(other)?;
            measured.revalidate(budget).map_err(other)?;
            self.revalidate(budget)?;
            drop(measured);
        } else {
            drop(running);
        }
        drop(procfs);
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or_else(|| io::Error::other("manager entry accounting"))?,
            )
            .map_err(other)?;
        Ok(!sealed)
    }

    /// Executes only the independently approved, sealed manager image with no
    /// inherited descriptors or environment. There is exactly one exec attempt.
    /// Success replaces the process; failure consumes this startup owner.
    /// The new image must freshly admit `RunningNativeApplicationManagerV1` and
    /// acquire its own actual root inputs before listening or launching children.
    ///
    /// # Safety
    /// This is the dedicated single-threaded, childless bootstrap phase only.
    /// No application, native/GPU operation, listener, signing service, funded
    /// child cleanup, or process-custody obligation may have existed in this
    /// process. There must be no other owner depending on inherited descriptors.
    /// Never call it to restart or replenish an application/service work account.
    pub unsafe fn exec_before_service(self, budget: &mut Budget<'work>) -> io::Result<Infallible> {
        budget.charge_work(64 * 1024).map_err(other)?;
        budget.reserve_storage(4096).map_err(other)?;
        self.revalidate(budget)?;
        require(
            self.pid == self.tid,
            "manager bootstrap is not on the main thread",
        )?;
        require_childless_single_thread()?;
        let (image, storage) = self.image.try_clone_for_exec(budget).map_err(other)?;
        budget
            .reserve_storage(storage.additional_storage())
            .map_err(other)?;
        self.revalidate(budget)?;
        self.image
            .revalidate_exec_clone(&image, budget)
            .map_err(other)?;
        require_childless_single_thread()?;

        // Marking rather than closing leaves the exact exec descriptor available
        // for execveat; every descriptor, including that image, closes on success.
        // SAFETY: the caller's bootstrap contract excludes every live FD obligation.
        if unsafe {
            libc::syscall(
                libc::SYS_close_range,
                0u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        let argv = [
            c"fe2o3-native-application-manager".as_ptr(),
            std::ptr::null(),
        ];
        let envp = [std::ptr::null::<libc::c_char>()];
        // SAFETY: live sealed descriptor and static, terminated C strings/tables;
        // no libc/Rust state or application owner is allowed to survive this exec.
        unsafe {
            libc::syscall(
                libc::SYS_execveat,
                image.as_raw_fd(),
                c"".as_ptr(),
                argv.as_ptr(),
                envp.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
        }
        Err(io::Error::last_os_error())
    }
}

fn require_same_installed_image(running: &File, installed: &File) -> io::Result<()> {
    let key = |file: &File| -> io::Result<_> {
        let s = rustix::fs::fstat(file)?;
        Ok((
            s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid, s.st_nlink, s.st_size,
        ))
    };
    require(
        key(running)? == key(installed)?,
        "manager entry is not the original installed image",
    )
}

fn require_childless_single_thread() -> io::Result<()> {
    let tasks = File::from(rustix::fs::open(
        "/proc/self/task",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )?);
    require(
        rustix::fs::fstatfs(&tasks)?.f_type as i64 == libc::PROC_SUPER_MAGIC as i64,
        "manager bootstrap task view is not procfs",
    )?;
    let mut own = false;
    let expected = std::process::id().to_string();
    // RawDir exposes EINTR rather than hiding retries in the bounded inventory.
    let mut buffer = [std::mem::MaybeUninit::uninit(); 1024];
    let mut entries = rustix::fs::RawDir::new(&tasks, &mut buffer);
    for _ in 0..4 {
        let Some(entry) = entries.next() else {
            require(own, "manager bootstrap task is absent")?;
            let children = File::from(rustix::fs::open(
                "/proc/thread-self/children",
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )?);
            require(
                rustix::fs::fstatfs(&children)?.f_type as i64 == libc::PROC_SUPER_MAGIC as i64
                    && rustix::io::read(&children, &mut [0u8; 1])? == 0,
                "manager bootstrap already owns children",
            )?;
            return Ok(());
        };
        let entry = entry?;
        match entry.file_name().to_bytes() {
            b"." | b".." => {}
            name if name == expected.as_bytes() && !own => own = true,
            _ => return Err(io::Error::other("manager bootstrap is not single threaded")),
        }
    }
    Err(io::Error::other(
        "manager bootstrap task inventory exceeds bound",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_entry_bytes_do_not_replace_original_installed_inode() {
        use std::io::Write;
        let mut original = tempfile::tempfile().unwrap();
        let mut substitute = tempfile::tempfile().unwrap();
        original.write_all(b"same bounded image bytes").unwrap();
        substitute.write_all(b"same bounded image bytes").unwrap();
        require_same_installed_image(&original.try_clone().unwrap(), &original).unwrap();
        assert!(require_same_installed_image(&substitute, &original).is_err());
    }

    #[test]
    fn manager_bootstrap_refuses_multithreaded_test_process() {
        let (ready, wait) = std::sync::mpsc::channel();
        let (done, finish) = std::sync::mpsc::channel();
        let child = std::thread::spawn(move || {
            ready.send(()).unwrap();
            finish.recv().unwrap();
        });
        wait.recv().unwrap();
        let refused = require_childless_single_thread().is_err();
        done.send(()).unwrap();
        child.join().unwrap();
        assert!(refused);
    }
}
