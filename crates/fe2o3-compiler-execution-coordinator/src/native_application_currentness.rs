//! Actual application issuer launch; no compiler trace or compiler dependency.
use super::*;
use fe2o3_broker_authority_service::{
    ApplicationCurrentnessCustodyV3 as Custody, PendingRootNativeApplicationV3 as Registration,
    RootApplicationCurrentnessConnectionV3 as Connection,
};

struct ApplicationPayload {
    prepared: Prepared,
    key: ServiceKey,
    manifest: ManifestCap,
    retained: usize,
}
impl ApplicationPayload {
    const ENVELOPE: usize = size_of::<Self>()
        - size_of::<Prepared>()
        - size_of::<ServiceKey>()
        - size_of::<ManifestCap>();
}

/// The original measured issuer child for one authenticated native application.
/// This owns actual prepared root authority and its funded cleanup slot, not a
/// compiler trace or a decoded readiness claim. Keep its creator thread and
/// original cleanup controller alive through all deferred cleanup. Its complete
/// reservation remains prepaid until this owner and any borrowed custody drop.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::ApplicationCurrentnessIssuerV3;
/// fn fake(ready: &[u8]) { let _ = ApplicationCurrentnessIssuerV3::decode(ready); }
/// ```
pub struct ApplicationCurrentnessIssuerV3<'work> {
    child: RetainedChild<ApplicationPayload>,
    ready: Ready,
    connection: Option<Connection<'work>>,
    retained: usize,
    account: RequestAccount,
    _creator: PhantomData<(&'work Budget<'work>, Rc<()>)>,
}
impl<'work> ApplicationCurrentnessIssuerV3<'work> {
    const ENVELOPE: usize = size_of::<(Self, Storage)>()
        - size_of::<RetainedChild<ApplicationPayload>>()
        - size_of::<Ready>()
        - size_of::<Connection<'static>>();

    /// Full conservative retained charge, including the consumed preparation.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// The original child PID; this inert observation grants no launch authority.
    pub fn issuer_pid(&self) -> rustix::process::Pid {
        self.child.pid()
    }

    /// Forward actual native service readiness through the original supervisor
    /// only after this owner completed the authenticated currentness root gate.
    pub fn publish_to_supervisor(
        &self,
        registration: &mut Registration<'_, '_, 'work>,
        b: &mut Budget<'work>,
    ) -> Result<()> {
        self.revalidate(registration, b)?;
        self.connection
            .as_ref()
            .ok_or(Error::Invalid(
                "application currentness custody already transferred",
            ))?
            .publish_to_supervisor(registration, &self.child, &self.ready, b)?;
        self.revalidate(registration, b)
    }

    /// Recheck the original native registration, measured child and live gate.
    pub fn revalidate(
        &self,
        registration: &Registration<'_, '_, 'work>,
        b: &mut Budget<'work>,
    ) -> Result<()> {
        self.check_account(b)?;
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            self.child.with_resources(b, |p, b| -> Result<()> {
                p.prepared.validate_process(self.child.pid(), b)?;
                match_ready(&self.ready, self.child.pid(), &p.manifest, &p.prepared, b)
            })?;
            self.connection
                .as_ref()
                .ok_or(Error::Invalid(
                    "application currentness custody already transferred",
                ))?
                .validate(registration, &self.child, b)?;
            Ok(())
        })
    }

    /// Move the one actual root connection into proof startup while borrowing
    /// this original child. This is a one-shot transition; errors after taking
    /// the connection leave no retry path. The issuer's complete reservation
    /// stays prepaid, and the receipt is additional growth above that charge.
    pub fn take_currentness_custody<'root, 'registry, 'custody>(
        &'root mut self,
        registration: &Registration<'registry, 'custody, 'work>,
        b: &mut Budget<'work>,
    ) -> Result<(Custody<'root, 'work>, Storage)>
    where
        'custody: 'root,
    {
        self.revalidate(registration, b)?;
        let connection = self.connection.take().ok_or(Error::Invalid(
            "application currentness custody already transferred",
        ))?;
        let (custody, growth) =
            connection.into_application_custody(registration, &self.child, b)?;
        Ok((custody, Storage(growth)))
    }

    fn check_account(&self, b: &Budget<'_>) -> Result<()> {
        if self.account.ledger != b.work_ledger_identity_v1()
            || self.account.address != b as *const Budget<'_> as usize
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

/// Additional persistent funding on the original cleanup controller. This does
/// not create or renew an account, or guarantee terminal cleanup within a turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationCurrentnessCleanupQuotaV3 {
    work: usize,
    storage: usize,
}
impl ApplicationCurrentnessCleanupQuotaV3 {
    /// Conservative work added before launch on the original cleanup ledger.
    pub const fn work(&self) -> usize {
        self.work
    }
    /// Additional retained payload storage on the original cleanup ledger.
    pub const fn additional_storage(&self) -> usize {
        self.storage
    }
}

impl Prepared {
    /// Additional persistent cleanup funding for this actual preparation and a
    /// finite positive number of pump/shutdown turns. Existing pool/guard/anchor
    /// reservations remain independently prepaid on that same controller.
    pub fn application_currentness_cleanup_quota(
        &self,
        turns: usize,
    ) -> Result<ApplicationCurrentnessCleanupQuotaV3> {
        use fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2;
        if turns == 0 {
            return Err(Error::Invalid(
                "currentness cleanup requires positive turns",
            ));
        }
        let payload = sum(&[
            self.retained_storage(),
            ServiceKey::STORAGE,
            ManifestCap::IO_STORAGE,
            ApplicationPayload::ENVELOPE,
        ])?;
        let repeated = |n: usize| n.checked_mul(turns).ok_or(Resource::Arithmetic);
        Ok(ApplicationCurrentnessCleanupQuotaV3 {
            work: sum(&[
                Cleanup::GUARD_CLONE_WORK,
                Cleanup::retained_launch_work::<ApplicationPayload>(payload)?,
                repeated(Cleanup::pump_work(MAX_PROTECTED_SERVICE_PROCESSES_V2)?)?,
                repeated(Cleanup::shutdown_work())?,
            ])?,
            storage: Resources::<ApplicationPayload>::payload_storage(payload)?,
        })
    }

    /// Consume actual prepared root authority to launch the pinned issuer for
    /// this authenticated native application. The original root gate selects
    /// only currentness/Cancel after exact readiness and EOF, never compiler
    /// Prepare/Issue/Recover/Publish. Both input reservations stay prepaid; the
    /// returned receipt is growth above the consumed complete preparation.
    ///
    /// # Safety
    /// Run on the original root manager's cloning thread, retain that thread and
    /// the already funded cleanup controller through all termination/quarantine,
    /// and preserve sole consuming wait, namespace and descriptor custody. The
    /// cleanup controller must carry this actual preparation's lifecycle guard.
    /// The registration and all phases use the same cumulative original Budget.
    #[allow(unsafe_code)]
    pub unsafe fn launch_application_currentness<'work>(
        self,
        registration: &Registration<'_, '_, 'work>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(ApplicationCurrentnessIssuerV3<'work>, Storage)> {
        let deadline = launch_io::bounded_deadline(timeout)?;
        if deadline > registration.startup_deadline() {
            return Err(Error::Invalid(
                "currentness launch exceeds registration deadline",
            ));
        }
        self.validate_cleanup_guard(cleanup, b)?;
        let account = RequestAccount::capture(b);
        // The callback borrows the original observed endpoint and app pidfd;
        // no compiler trace or caller-selected descriptor can enter this path.
        Ok(
            registration.with_currentness_launch_inputs(b, |client, peer, pidfd, b| {
                // SAFETY: the public contract preserves original creator/cleanup
                // custody; registration supplies only its live admitted originals.
                unsafe {
                    launch(
                        self,
                        registration,
                        client,
                        peer,
                        pidfd,
                        account,
                        deadline,
                        cleanup,
                        b,
                    )
                }
                .map_err(std::io::Error::other)
            })?,
        )
    }
}

#[allow(unsafe_code, clippy::too_many_arguments)]
unsafe fn launch<'work>(
    prepared: Prepared,
    registration: &Registration<'_, '_, 'work>,
    client: Client,
    peer: BorrowedFd<'_>,
    pidfd: BorrowedFd<'_>,
    account: RequestAccount,
    deadline: Instant,
    cleanup: &mut Cleanup,
    b: &mut Budget<'work>,
) -> Result<(ApplicationCurrentnessIssuerV3<'work>, Storage)> {
    let original = prepared.retained_storage();
    b.with_prepaid_scope(
        original,
        8,
        LOCAL_WORK,
        FRAME + launch_io::ATTEMPT_SCRATCH + launch_io::PIPE_ATTEMPT_SCRATCH,
        |b| {
            prepared.revalidate(b)?;
            let credentials = prepared.credentials;
            let (anchor, charge) = prepared.anchor.try_clone_for_supervisor(
                prepared.trust.deployment(),
                prepared.trust.policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let (manifest, charge) = Manifest::new(
                client,
                anchor.service(),
                prepared.trust.policy().policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            // Currentness is for the original native carriage, not a new policy or
            // anchor generation. Exact comparison precedes key transfer and clone.
            if manifest.canonical_bytes()
                != registration
                    .binding()
                    .compiler_handoff()
                    .launch_manifest()
                    .canonical_bytes()
            {
                return Err(Error::Invalid(
                    "currentness preparation changed original launch manifest",
                ));
            }
            let (manifest, charge) = ManifestCap::create(manifest, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (key, charge) = prepared.trust.key_template().reissue_for_deployed_service(
                prepared.trust.deployment().deployment(),
                prepared.trust.policy().policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = sum(&[
                original,
                key.retained_storage(),
                manifest.retained_storage(),
                ApplicationPayload::ENVELOPE,
            ])?;
            b.reserve_storage(ApplicationPayload::ENVELOPE)?;
            let payload = ApplicationPayload {
                prepared,
                key,
                manifest,
                retained,
            };
            b.reserve_storage(Channels::STORAGE)?;
            let channels = Channels::new()?;
            let (mut root_channel, charge) = RootChannel::create(b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (stage, stage_charge) = staging::stage(
                &StageInputs {
                    prepared: &payload.prepared,
                    key: &payload.key,
                    manifest: &payload.manifest,
                    retained: payload.retained,
                },
                anchor,
                peer,
                pidfd,
                &channels,
                &root_channel,
                b,
            )?;
            b.reserve_storage(stage_charge)?;
            check_deadline(deadline, "application issuer staging")?;
            // SAFETY: the unchanged closed Stage validated every final image/file
            // alias. The complete original preparation enters cleanup before clone.
            let (mut child, charge) =
                unsafe { stage.spawn_retaining(credentials, payload, retained, cleanup, b) }?;
            b.reserve_storage(charge.additional_storage())?;
            drop(stage);
            b.release_storage(stage_charge)?;
            root_channel.close_parent_issuer_endpoint(b)?;
            let Readers {
                ready,
                profile,
                gate,
                exec,
            } = channels.close_child_ends();
            launch_io::await_profile_ready(
                profile.as_fd(),
                exec.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            child.with_resources(b, |p, b| p.prepared.validate_process(child.pid(), b))?;
            launch_io::release_child(
                gate.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            drop(gate);
            launch_io::await_exec_eof(
                exec.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            b.reserve_storage(READY_BYTES)?;
            let bytes = launch_io::receive_ready_pipe::<READY_BYTES, _>(
                ready.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            let (ready_record, charge) = Ready::decode(&bytes, b)?;
            b.reserve_storage(charge.additional_storage())?;
            child.with_resources(b, |p, b| -> Result<()> {
                p.prepared.validate_process(child.pid(), b)?;
                match_ready(&ready_record, child.pid(), &p.manifest, &p.prepared, b)?;
                fe2o3_broker_authority_service::validate_retained_issuer_image_v3(
                    &child,
                    p.prepared.trust.policy().policy(),
                    b,
                )?;
                Ok(())
            })?;
            if !child.is_live(b)? {
                return Err(launch_io::Failure::ChildExited("application issuer readiness").into());
            }
            check_deadline(deadline, "application issuer readiness")?;
            // SAFETY: actual profile/exec EOF, private exact Ready+EOF, measured
            // retained image, final liveness, and every parent writer alias closure.
            unsafe { child.confirm_exec(b) }?;
            let (connection, growth) = Connection::connect_after_readiness(
                registration,
                &child,
                &ready_record,
                root_channel,
                deadline.saturating_duration_since(Instant::now()),
                b,
            )?;
            b.reserve_storage(growth)?;
            check_deadline(deadline, "application issuer root join")?;
            let retained = sum(&[
                child.retained_storage(),
                ready_record.retained_storage(),
                connection.retained_storage(),
                ApplicationCurrentnessIssuerV3::ENVELOPE,
            ])?;
            b.reserve_storage(ApplicationCurrentnessIssuerV3::ENVELOPE)?;
            let growth = retained.checked_sub(original).ok_or(Resource::Accounting)?;
            Ok((
                ApplicationCurrentnessIssuerV3 {
                    child,
                    ready: ready_record,
                    connection: Some(connection),
                    retained,
                    account,
                    _creator: PhantomData,
                },
                Storage(growth),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_payload_has_only_actual_prepared_inputs() {
        assert_eq!(
            ApplicationPayload::ENVELOPE
                + size_of::<Prepared>()
                + size_of::<ServiceKey>()
                + size_of::<ManifestCap>(),
            size_of::<ApplicationPayload>()
        );
        assert!(ApplicationCurrentnessIssuerV3::ENVELOPE >= size_of::<RequestAccount>());
        assert_eq!(staging::DESTINATIONS, [3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }
}
