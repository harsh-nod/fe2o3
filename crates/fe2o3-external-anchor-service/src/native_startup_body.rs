pub(super) const INPUT_STORAGE: usize = Policy::FILE_STORAGE
    + Supervisor::FILE_STORAGE
    + Deployment::FILE_STORAGE
    + Key::FILE_STORAGE
    + Anchor::ROOT_STORAGE
    + crate::NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2
    + Lease::FILE_STORAGE;

pub(super) fn run<H: StartupHooks>(b: &mut Budget<'_>, hooks: &mut H) -> Result<(Report, Storage)> {
    let mut sources = Sources {
        live: [true; INPUT_FDS.len()],
    };
    b.with_prepaid_scope(INPUT_STORAGE, 8, NATIVE_EXTERNAL_ANCHOR_STARTUP_WORK_V2,
        NATIVE_EXTERNAL_ANCHOR_STARTUP_FRAME_STORAGE_V2, |b| {
            hooks.invocation()?;
            // Profile/namespace snapshots contain no descriptors. Admit them before
            // cleanup and before touching the secret slot, then bind actual context.
            let profile = hooks.profile(b)?;
            close_unrelated()?;
            sources.validate()?;
            let (policy, c) = Policy::from_inherited_at(INPUT_FDS[POLICY], b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(POLICY)?;
            let (supervisor, c) = Supervisor::from_inherited_at(INPUT_FDS[SUPERVISOR], policy.policy(), b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(SUPERVISOR)?;
            let (deployment, c) = Deployment::from_inherited_at(INPUT_FDS[DEPLOYMENT], supervisor.deployment(), policy.policy(), b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(DEPLOYMENT)?;
            let d = deployment.deployment();
            if d.service().uid() != rustix::process::geteuid().as_raw()
                || d.service().gid() != rustix::process::getegid().as_raw() {
                return Err(Error::Credentials);
            }
            let m = d.executable();
            let measurement = Measurement::new(m.sha256(), m.byte_len(),
                fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1)
                .map_err(ProtectedStaticExecutableErrorV2::from)?;
            let owner = Owner::new(d.service().uid(), d.service().gid())
                .map_err(ProtectedStaticExecutableErrorV2::from)?;
            let executable = hooks.executable(measurement, owner, b)?;
            profile.revalidate(b)?;
            policy.revalidate(b)?;
            supervisor.revalidate(b)?;
            deployment.revalidate(b)?;
            let (key, c) = Key::from_inherited_at(INPUT_FDS[KEY], d, b)?;
            b.reserve_storage(c.additional_storage())?;
            sources.close(KEY)?;
            let root = sources.take(ROOT, 256, "native durable root")?;
            let peer = sources.take(PEER, 257, "native connected peer")?;
            let lifecycle = hooks.lifecycle(File::from(sources.take(LIFECYCLE, 258, "native lifecycle")?), &root, b)?;
            key.revalidate(d, b)?;
            profile.revalidate(b)?;
            if let Some(e) = &executable { e.revalidate(b)?; }
            let (mut anchor, c) = Anchor::open(root, key, d, b)?;
            b.reserve_storage(c.additional_storage())?;
            lifecycle.revalidate(b)?;
            deployment.revalidate(b)?;
            profile.revalidate(b)?;
            hooks.ready(b)?;
            let (report, c) = serve(&mut anchor, d, peer, b)?;
            b.reserve_storage(c.additional_storage())?;
            lifecycle.revalidate(b)?;
            deployment.revalidate(b)?;
            profile.revalidate(b)?;
            Ok((report, c))
        })
}
