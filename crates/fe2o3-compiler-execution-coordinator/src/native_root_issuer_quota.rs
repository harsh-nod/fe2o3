use super::*;
use fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY;

/// Additional funding on the ORIGINAL cleanup controller; its existing pool,
/// guard, compiler and anchor payload reservations remain prepaid independently.
/// Finite pump/shutdown turns are not a guarantee of eventual terminal cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IssuerCleanupQuota {
    work: usize,
    storage: usize,
}
impl IssuerCleanupQuota {
    pub(crate) const fn work(&self) -> usize {
        self.work
    }
    pub(crate) const fn additional_storage(&self) -> usize {
        self.storage
    }
}

fn repeated(count: usize, value: usize) -> Result<usize> {
    count.checked_mul(value).ok_or(Resource::Arithmetic.into())
}

// Both public I/O scratch contracts bound their complete returned owner. Using
// those ceilings avoids depending on private layouts; actual payload accounting
// at launch uses retained_storage(), never these upper bounds as declarations.
fn payload_ceiling<T: Send + 'static>(prepared: usize, dependency: usize) -> Result<usize> {
    payload_storage::<T>(
        prepared,
        dependency,
        ServiceKey::STORAGE,
        ManifestCap::IO_STORAGE,
    )
}

impl Prepared {
    /// Complete conservative original-request work and extra peak above this
    /// Prepared PLUS the full original CompilerTrace. Includes trace poll/input
    /// transfer, full overlapping dependency, fresh key/manifest, source images,
    /// frozen Stage, retained child growth, all finite transport/liveness attempts,
    /// two prepared process checks, the actual running issuer image, exact
    /// readiness, exec confirmation and the post-readiness root challenge.
    ///
    /// Query is inert and grants no launch authority. It includes NativeAttempt
    /// setup, not compiler resume or publication observation/issuance. Keep both
    /// consumed input reservations; reserve launch_root_attempt's growth above
    /// their SUM before retaining the returned attempt.
    /// Persistent cleanup uses issuer_cleanup_quota on its existing ledger.
    pub(crate) fn issuer_launch_quota<T: Send + 'static>(
        &self,
        trace: &CompilerTrace<'_, T>,
    ) -> Result<Quota> {
        let dependency = trace.issuer_dependency_storage()?;
        let validation = self.revalidation_quota()?;
        let guard = self.cleanup_guard_quota()?;
        let anchor = self.anchor.supervisor_transfer_quota()?;
        let anchor_validation = self.anchor.supervisor_transfer_validation_quota()?;
        issuer_launch_for::<T>(
            self.retained_storage(),
            dependency,
            trace.issuer_inputs_quota()?,
            Quota {
                work: validation.work(),
                scratch: validation.scratch(),
            },
            Quota {
                work: guard.work(),
                scratch: guard.scratch(),
            },
            Quota {
                work: anchor.work(),
                scratch: anchor.scratch(),
            },
            Quota {
                work: anchor_validation.work(),
                scratch: anchor_validation.scratch(),
            },
            self.programs[2].measurement().byte_len(),
            self.trust.policy().policy().executable().byte_len(),
            self.process_quota()?,
        )
    }

    /// Inert original-account startup ceiling before either consumed owner
    /// exists. `compiler_payload` must cover the entire original trace payload;
    /// actual launch still validates owners and uses actual retained charges.
    pub(crate) fn maximum_issuer_launch_quota<T: Send + 'static>(
        compiler_payload: usize,
    ) -> Result<Quota> {
        use fe2o3_protected_service_spawn::native_spawn::RootRetainedRuntimeTraceV1;
        let dependency =
            RootRetainedRuntimeTraceV1::<T>::dependency_quota_for_payload(compiler_payload)?;
        let validation = Self::maximum_revalidation_quota()?;
        let guard = Self::maximum_cleanup_guard_quota()?;
        let anchor = crate::native_v3::Anchor::maximum_supervisor_transfer_quota()?;
        let anchor_validation =
            crate::native_v3::Anchor::maximum_supervisor_transfer_validation_quota()?;
        issuer_launch_for::<T>(
            Self::maximum_retained_storage()?,
            dependency.retained_storage(),
            CompilerTrace::<T>::maximum_issuer_inputs_quota(compiler_payload)?,
            Quota {
                work: validation.work(),
                scratch: validation.scratch(),
            },
            Quota {
                work: guard.work(),
                scratch: guard.scratch(),
            },
            Quota {
                work: anchor.work(),
                scratch: anchor.scratch(),
            },
            Quota {
                work: anchor_validation.work(),
                scratch: anchor_validation.scratch(),
            },
            crate::native_v3::MAX_LAUNCHER,
            crate::native_v3::MAX_LAUNCHER,
            Self::process_quota_for(validation)?,
        )
    }

    pub(crate) fn maximum_issuer_continuity_quota<T: Send + 'static>() -> Result<Quota> {
        let root = RootConnection::validation_quota(crate::native_v3::MAX_LAUNCHER)?;
        continuity::<T>(
            Self::process_quota_for(Self::maximum_revalidation_quota()?)?,
            Quota {
                work: root.work(),
                scratch: root.scratch(),
            },
        )
    }

    pub(crate) fn maximum_issuer_cleanup_quota<T: Send + 'static>(
        compiler_payload: usize,
        cleanup_turns: usize,
    ) -> Result<IssuerCleanupQuota> {
        use fe2o3_protected_service_spawn::native_spawn::RootRetainedRuntimeTraceV1;
        let dependency =
            RootRetainedRuntimeTraceV1::<T>::dependency_quota_for_payload(compiler_payload)?;
        cleanup_quota::<T>(
            payload_ceiling::<T>(
                Self::maximum_retained_storage()?,
                dependency.retained_storage(),
            )?,
            cleanup_turns,
        )
    }

    pub(crate) fn issuer_continuity_quota<T: Send + 'static>(&self) -> Result<Quota> {
        let root =
            RootConnection::validation_quota(self.trust.policy().policy().executable().byte_len())?;
        continuity::<T>(
            self.process_quota()?,
            Quota {
                work: root.work(),
                scratch: root.scratch(),
            },
        )
    }

    /// Additional finite cleanup funding, with the original pool and all existing
    /// payload/guard charges preserved. Includes guard cloning, full new payload
    /// retention/retirement and exactly the supplied positive maximum pump/shutdown
    /// turns. Never create or renew an account from this query. Unresolved custody
    /// after the turn limit remains charged, even if all foreground owners drop.
    pub(crate) fn issuer_cleanup_quota<T: Send + 'static>(
        &self,
        trace: &CompilerTrace<'_, T>,
        cleanup_turns: usize,
    ) -> Result<IssuerCleanupQuota> {
        cleanup_quota::<T>(
            payload_ceiling::<T>(self.retained_storage(), trace.issuer_dependency_storage()?)?,
            cleanup_turns,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn issuer_launch_for<T: Send + 'static>(
    prepared: usize,
    dependency: usize,
    trace: Quota,
    validation: Quota,
    guard: Quota,
    anchor: Quota,
    anchor_validation: Quota,
    image_bytes: u64,
    policy_image_bytes: u64,
    process: Quota,
) -> Result<Quota> {
    let payload = payload_ceiling::<T>(prepared, dependency)?;
    let image = Image::quota_for_length(image_bytes, ImageOperation::Transfer)?;
    let source = staging::source_storage(image_bytes)?;
    let staging = Quota {
        work: sum(&[
            LOCAL_WORK,
            staging::PEER_PREPARE_WORK,
            repeated(2, validation.work())?,
            repeated(2, image.work())?,
            repeated(2, Inputs::WORK)?,
            repeated(2, PolicyCap::IO_WORK)?,
            repeated(2, ServiceKey::WORK)?,
            repeated(2, ManifestCap::IO_WORK)?,
            anchor_validation.work(),
            AnchorTransfer::INTO_DESCRIPTORS_WORK,
            Stage::STAGING_WORK,
        ])?,
        scratch: sum(&[
            FRAME,
            staging::PEER_PREPARE_SCRATCH,
            source,
            repeated(2, Stage::storage_for_sources(source)?)?,
            Stage::STAGING_SCRATCH,
            validation.scratch(),
            image.scratch(),
            Inputs::SCRATCH,
            PolicyCap::IO_STORAGE,
            ServiceKey::STORAGE,
            ManifestCap::IO_STORAGE,
            anchor_validation.scratch(),
            AnchorTransfer::INTO_DESCRIPTORS_SCRATCH,
        ])?,
    };
    let launch = launch_quota::<T>(
        payload,
        source,
        trace,
        guard,
        validation,
        anchor,
        staging,
        process,
        issuer_image_quota(policy_image_bytes)?,
    )?;
    let gate = RootConnection::handshake_quota(policy_image_bytes)?;
    root_startup_quota::<T>(
        launch,
        Quota {
            work: gate.work(),
            scratch: gate.scratch(),
        },
    )
}

fn issuer_image_quota(image_bytes: u64) -> Result<Quota> {
    let quota = fe2o3_broker_authority_service::retained_issuer_image_quota_v3(image_bytes)?;
    Ok(Quota {
        work: quota.work(),
        scratch: quota.scratch(),
    })
}

pub(super) fn root_startup_quota<T: Send + 'static>(launch: Quota, gate: Quota) -> Result<Quota> {
    Ok(Quota {
        work: sum(&[
            launch.work(),
            4 * RootChannel::WORK,
            CompilerTrace::<T>::OBSERVATION_WORK,
            Resources::<Payload<T>>::ACCESS_WORK,
            RootSession::CREATE_WORK,
            gate.work(),
        ])?,
        scratch: sum(&[
            launch.scratch(),
            RootChannel::STORAGE,
            RootChannel::SCRATCH,
            ReadyIssuer::<T>::ENVELOPE,
            CompilerTrace::<T>::OBSERVATION_SCRATCH,
            Resources::<Payload<T>>::ACCESS_SCRATCH,
            // Retained session overlaps the subsequent connection handshake.
            2 * RootSession::CREATE_SCRATCH,
            gate.scratch(),
            NativeAttempt::<T>::ENVELOPE,
        ])?,
    })
}

impl<T: Send + 'static> NativeAttempt<'_, T> {
    pub(crate) const fn original_policy_identity_quota() -> Quota {
        Quota {
            work: LOCAL_WORK + Resources::<Payload<T>>::ACCESS_WORK,
            scratch: FRAME + Resources::<Payload<T>>::ACCESS_SCRATCH,
        }
    }

    /// Complete request costs above the FULL attempt for consuming attachment.
    /// Persistent cleanup funding is separate and stays on the original service.
    pub(crate) fn publication_observation_quota(maximum_handoff_bytes: usize) -> Result<Quota> {
        let publication = Publication::observation_quota(maximum_handoff_bytes)?;
        publication_operation::<T>(Quota {
            work: publication.work(),
            scratch: publication.scratch(),
        })
    }

    pub(crate) fn publication_revalidation_quota(&self) -> Result<Quota> {
        let publication = self
            .publication
            .as_ref()
            .ok_or(Error::Invalid("root attempt has no publication"))?
            .revalidation_quota();
        publication_operation::<T>(Quota {
            work: publication.work(),
            scratch: publication.scratch(),
        })
    }

    pub(crate) fn maximum_publication_revalidation_quota(
        maximum_handoff_bytes: usize,
    ) -> Result<Quota> {
        let publication = Publication::maximum_revalidation_quota(maximum_handoff_bytes)?;
        publication_operation::<T>(Quota {
            work: publication.work(),
            scratch: publication.scratch(),
        })
    }

    /// Original-request work and extra peak for validate_original, above the FULL
    /// attempt reservation even when its issuer has already been removed.
    pub(crate) const fn original_validation_quota() -> Quota {
        Quota {
            work: LOCAL_WORK + CompilerTrace::<T>::OBSERVATION_WORK + RootSession::VALIDATE_WORK,
            scratch: FRAME
                + CompilerTrace::<T>::OBSERVATION_SCRATCH
                + RootSession::VALIDATE_SCRATCH,
        }
    }
}

fn publication_operation<T: Send + 'static>(publication: Quota) -> Result<Quota> {
    Ok(Quota {
        work: sum(&[
            LOCAL_WORK,
            2 * CompilerTrace::<T>::OBSERVATION_WORK,
            2 * RootSession::VALIDATE_WORK,
            CompilerTrace::<T>::PUBLICATION_FRAME_WORK,
            publication.work(),
        ])?,
        // Both root checks are sequential; the second overlaps the returned
        // publication, whose construction bound includes its full retained charge.
        scratch: sum(&[
            FRAME,
            CompilerTrace::<T>::OBSERVATION_SCRATCH,
            RootSession::VALIDATE_SCRATCH,
            CompilerTrace::<T>::PUBLICATION_FRAME_SCRATCH,
            publication.scratch(),
        ])?,
    })
}

pub(super) fn cleanup_quota<T: Send + 'static>(
    payload: usize,
    turns: usize,
) -> Result<IssuerCleanupQuota> {
    if turns == 0 {
        return Err(Error::Invalid(
            "issuer cleanup requires finite positive turns",
        ));
    }
    Ok(IssuerCleanupQuota {
        work: sum(&[
            Cleanup::GUARD_CLONE_WORK,
            Cleanup::retained_launch_work::<Payload<T>>(payload)?,
            repeated(turns, Cleanup::pump_work(CAPACITY)?)?,
            repeated(turns, Cleanup::shutdown_work())?,
        ])?,
        storage: Resources::<Payload<T>>::payload_storage(payload)?,
    })
}

// The shared MAX_WORK/MAX_LIVENESS_CHECKS cover profile, gate and exec phases
// (and conservatively their unused seqpacket ready phase). PIPE bounds add the
// actual separate 120-byte+EOF phase; never substitute MAX_READY_BYTES=88.
#[allow(clippy::too_many_arguments)]
pub(super) fn launch_quota<T: Send + 'static>(
    payload: usize,
    source: usize,
    trace: Quota,
    guard: Quota,
    validation: Quota,
    anchor: Quota,
    staging: Quota,
    process: Quota,
    running_image: Quota,
) -> Result<Quota> {
    let polling = sum(&[
        launch_io::MAX_WORK,
        launch_io::MAX_PIPE_WORK,
        repeated(
            sum(&[
                launch_io::MAX_LIVENESS_CHECKS,
                launch_io::MAX_PIPE_LIVENESS_CHECKS,
            ])?,
            PlainChild::OPERATION_WORK,
        )?,
    ])?;
    let child_growth = Child::<T>::storage_for(payload)?
        .checked_sub(payload)
        .ok_or(Resource::Accounting)?;
    Ok(Quota {
        work: sum(&[
            2 * LOCAL_WORK,
            trace.work(),
            guard.work(),
            validation.work(),
            anchor.work(),
            MANIFEST_WORK,
            ManifestCap::IO_WORK,
            ServiceKey::WORK,
            staging.work(),
            Stage::spawn_work_for(staging::DESTINATIONS.len(), 63)?,
            Cleanup::retained_launch_work::<Payload<T>>(payload)?,
            repeated(2, Resources::<Payload<T>>::ACCESS_WORK)?,
            repeated(2, process.work())?,
            running_image.work(),
            repeated(2, READY_WORK)?,
            polling,
            2 * PlainChild::OPERATION_WORK,
        ])?,
        scratch: sum(&[
            2 * FRAME,
            trace.scratch(),
            guard.scratch(),
            validation.scratch(),
            anchor.scratch(),
            AnchorTransfer::STORAGE,
            MANIFEST_SCRATCH,
            MANIFEST_OWNER,
            2 * ManifestCap::IO_STORAGE,
            2 * ServiceKey::STORAGE,
            Payload::<T>::ENVELOPE,
            Channels::STORAGE,
            staging.scratch(),
            Stage::storage_for_sources(source)?,
            Stage::spawn_retaining_scratch::<Payload<T>>(payload)?,
            child_growth,
            Resources::<Payload<T>>::ACCESS_SCRATCH,
            process.scratch(),
            running_image.scratch(),
            READY_BYTES,
            READY_SCRATCH,
            READY_OWNER,
            launch_io::ATTEMPT_SCRATCH,
            launch_io::PIPE_ATTEMPT_SCRATCH,
            PlainChild::OPERATION_SCRATCH,
            ManagedIssuer::<T>::ENVELOPE,
        ])?,
    })
}

pub(super) fn continuity<T: Send + 'static>(process: Quota, root: Quota) -> Result<Quota> {
    Ok(Quota {
        work: sum(&[
            LOCAL_WORK,
            Resources::<Payload<T>>::ACCESS_WORK,
            process.work(),
            CompilerTrace::<T>::OBSERVATION_WORK,
            root.work(),
            READY_WORK,
            PlainChild::OPERATION_WORK,
        ])?,
        scratch: sum(&[
            FRAME,
            Resources::<Payload<T>>::ACCESS_SCRATCH,
            process.scratch(),
            CompilerTrace::<T>::OBSERVATION_SCRATCH,
            root.scratch(),
            READY_SCRATCH,
            PlainChild::OPERATION_SCRATCH,
        ])?,
    })
}
