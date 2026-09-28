pub(super) const INPUT_STORAGE: usize = 4 * FILE_STORAGE
    + IMAGE_MAX
    + Policy::FILE_STORAGE
    + Supervisor::FILE_STORAGE
    + Deployment::FILE_STORAGE
    + Key::FILE_STORAGE
    + Provisioning::FILE_STORAGE;

pub(super) trait Hooks: StartupHooks {
    fn reissue(&mut self, f: File, d: &DeploymentRecord, b: &mut Budget<'_>) -> Result<Key> {
        let (key, c) = Key::reissue_root_template_for_current_service(f, d, b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(key)
    }
}

pub(super) fn run<H: Hooks>(b: &mut Budget<'_>, h: &mut H) -> Result<Infallible> {
    let mut sources = Sources([true; 9]);
    b.with_prepaid_scope(
        INPUT_STORAGE,
        8,
        NATIVE_EXTERNAL_ANCHOR_HELPER_WORK_V2,
        NATIVE_EXTERNAL_ANCHOR_HELPER_FRAME_STORAGE_V2,
        |b| {
            h.invocation()?;
            let profile = h.profile(b)?;
            close_unrelated()?;
            sources.validate()?;
            let (policy, c) = Policy::from_inherited_at(INPUT_FDS[POLICY], b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(POLICY)?;
            let (supervisor, c) =
                Supervisor::from_inherited_at(INPUT_FDS[SUPERVISOR], policy.policy(), b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(SUPERVISOR)?;
            let (deployment, c) = Deployment::from_inherited_at(
                INPUT_FDS[DEPLOYMENT],
                supervisor.deployment(),
                policy.policy(),
                b,
            )?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(DEPLOYMENT)?;
            let d = deployment.deployment();
            if d.service().uid() != rustix::process::geteuid().as_raw()
                || d.service().gid() != rustix::process::getegid().as_raw()
            {
                return Err(Error::Invalid("helper credentials do not match deployment"));
            }
            let (provisioning, c) = Provisioning::from_inherited_at(INPUT_FDS[PROVISIONING], d, b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(PROVISIONING)?;
            let owner = Owner::new(d.service().uid(), d.service().gid())
                .map_err(ProtectedStaticExecutableErrorV2::from)?;
            let helper = h.running(measurement(provisioning.provisioning().helper())?, owner, b)?;
            profile.revalidate(b)?;
            let bootstrap: OwnedFd = sources.take(BOOTSTRAP)?.into();
            h.bootstrap(&bootstrap)?;
            let root = sources.take(ROOT)?;
            let lifecycle = h.lifecycle(sources.take(LIFECYCLE)?, &root, b)?;
            let daemon_measurement = measurement(d.executable())?;
            let intake_charge = Executable::file_storage(daemon_measurement)?;
            // Intake duplicates before closing the raw slot; both full images coexist.
            let daemon_file =
                b.with_prepaid_scope(intake_charge, 0, 0, intake_charge, |_| sources.take(DAEMON))?;
            let (daemon, c) = Executable::admit_sealed(
                daemon_file,
                daemon_measurement,
                owner,
                "native anchor daemon",
                b,
            )?;
            b.reserve_storage(c.additional_storage())?;
            // Secret template is touched only after profile/image/root-parent admission.
            let key = h.reissue(sources.take(KEY)?, d, b)?;
            let mut next = helper_io::STAGED_DESCRIPTOR_FLOOR_V1;
            let state_root = duplicate(&root, &mut next, b)?;
            let (key_image, c) = key.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let (state_key, c) = Key::from_file(key_image, d, b)?;
            b.reserve_storage(c.additional_storage())?;
            h.before_state(b)?;
            let ((anchor, disposition), c) =
                Anchor::open_or_initialize(state_root.into(), state_key, d, b)?;
            b.reserve_storage(c.additional_storage())?;
            h.after_state(disposition, b)?;
            b.reserve_storage(2 * FILE_STORAGE)?;
            let (supervisor_peer, daemon_peer) = rustix::net::socketpair(
                rustix::net::AddressFamily::UNIX,
                rustix::net::SocketType::SEQPACKET,
                rustix::net::SocketFlags::CLOEXEC | rustix::net::SocketFlags::NONBLOCK,
                None,
            )
            .map_err(|e| helper_io::io_error("create native anchor peer pair", e.into()))?;

            let (image, c) = daemon.try_clone_for_exec(b)?;
            let image_charge = c.additional_storage();
            b.reserve_storage(image_charge)?;
            let staged_daemon = stage_file(image, image_charge, &mut next, b)?;
            let (file, c) = lifecycle.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let staged_lifecycle = stage_file(file, Lease::FILE_STORAGE, &mut next, b)?;
            let (file, c) = policy.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let staged_policy = stage_file(file, Policy::FILE_STORAGE, &mut next, b)?;
            let (file, c) = supervisor.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let staged_supervisor = stage_file(file, Supervisor::FILE_STORAGE, &mut next, b)?;
            let (file, c) = deployment.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let staged_deployment = stage_file(file, Deployment::FILE_STORAGE, &mut next, b)?;
            let (file, c) = key.try_clone_for_transfer(b)?;
            b.reserve_storage(c.additional_storage())?;
            let staged_key = stage_file(file, Key::FILE_STORAGE, &mut next, b)?;
            let staged_peer = duplicate(&daemon_peer, &mut next, b)?;
            let staged_root = duplicate(&root, &mut next, b)?;
            let staged_bootstrap: OwnedFd = duplicate(&bootstrap, &mut next, b)?.into();

            let revalidate = |b: &mut Budget<'_>| -> Result<()> {
                policy.validate_transfer(&staged_policy, b)?;
                supervisor.validate_transfer(&staged_supervisor, b)?;
                deployment.validate_transfer(&staged_deployment, b)?;
                provisioning.revalidate(b)?;
                key.validate_transfer(&staged_key, d, b)?;
                lifecycle.validate_transfer(&staged_lifecycle, b)?;
                daemon.revalidate_exec_clone(&staged_daemon, b)?;
                if let Some(e) = &helper {
                    e.revalidate(b)?;
                }
                profile.revalidate(b)?;
                Ok(())
            };
            revalidate(b)?;
            let ready = Ready::new(match disposition {
                Disposition::Existing => ReadyDisposition::Existing,
                Disposition::Initialized => ReadyDisposition::Initialized,
            });
            h.ready(&staged_bootstrap, &supervisor_peer, &ready, b)?;
            drop((supervisor_peer, daemon_peer, bootstrap));
            b.release_storage(2 * FILE_STORAGE)?;
            revalidate(b)?;
            h.before_exec(b)?;
            // Retain the state lock and all native owners until exec closes their CLOEXEC fds.
            let _retain_state_lock = anchor;
            let table = [
                (staged_peer.as_raw_fd(), 3, 2),
                (staged_root.as_raw_fd(), 4, 3),
                (staged_lifecycle.as_raw_fd(), 5, 4),
                (staged_policy.as_raw_fd(), 202, 5),
                (staged_supervisor.as_raw_fd(), 220, 6),
                (staged_deployment.as_raw_fd(), 221, 7),
                (staged_key.as_raw_fd(), 222, 8),
            ];
            // SAFETY: all exact revalidated sources are owned above all destinations;
            // fixed terminal work was prepaid at entry and failure terminates without retry.
            unsafe {
                helper_io::exec_inherited_daemon(
                    staged_daemon.as_raw_fd(),
                    staged_bootstrap.as_raw_fd(),
                    &table,
                    NAME,
                    9,
                )
            }
        },
    )
}
