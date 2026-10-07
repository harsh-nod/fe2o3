//! Private issuer proxy for an occurrence retained by its authenticated root.
//! Neither signed journal recovery nor decoded Subject bytes can construct it.
use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionRootControlKindV3 as RootKind;
use std::mem::size_of;

const OBSERVED_BYTES: usize = 32 + SUBJECT_BYTES;
const CONTEXT_WORK: usize = 8 + 8 * OBSERVED_BYTES;
const CONTEXT_FRAME: usize = 4 * size_of::<OccurrenceContext<'static>>() + 8192;
const _: () = {
    assert!(
        OBSERVED_BYTES
            <= fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3
    );
    assert!(
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3
            <= fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3
    );
};

#[derive(Default)]
pub(super) struct OccurrenceContext<'work> {
    root: Option<RootContext<'work>>,
    currentness: Option<root_control::CurrentnessRoot<'work>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_root_context_has_no_local_observation_fallback() {
        let context = OccurrenceContext::default();
        assert_eq!(context.retained_storage(), 0);
        assert!(context.root().is_err());
        let session = Session::with_context(context);
        assert!(session.occurrence().is_err());
        assert_eq!(session.retained_storage(), 0);
    }

    #[test]
    fn missing_root_context_refuses_with_exact_entry_work_and_no_storage() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        for available in [0, 7, 8, 16] {
            let mut work = Work::new(available);
            let mut b = Budget::new(&mut work, 0);
            let session = Session::default();
            assert!(session.require_context(&mut b).is_err());
            assert_eq!(b.storage(), 0);
            if available >= 8 {
                assert_eq!(b.work(), 8);
                assert!(session.require_context(&mut b).is_err());
                if available == 16 {
                    assert_eq!(b.work(), 16);
                }
            }
            assert!(session.occurrence().is_err());
        }
    }
}

struct RootContext<'work> {
    endpoint: root_control::RootEndpoint<'work>,
    manifest: Manifest,
    retained: usize,
}

/// The only production constructor consumes a reply from the actual retained
/// root endpoint after its original-Admission/manifest/request checks. It does
/// not itself hold the artifact lock: that owner remains in the root session.
pub(super) struct NativeOccurrence {
    subject: Subject,
    identity: [u8; 32],
}

impl NativeOccurrence {
    const STORAGE: usize = size_of::<Self>()
        + size_of::<(
            Subject,
            fe2o3_artifact_transaction::InertCompilerExecutionSubjectStorageV3,
        )>();
    pub(super) fn subject(&self) -> &Subject {
        &self.subject
    }
    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub(super) const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }
}

impl<'work> OccurrenceContext<'work> {
    pub(super) fn require_available(&self) -> Result<()> {
        if self.currentness.is_some() {
            Ok(())
        } else {
            self.root().map(|_| ())
        }
    }

    pub(super) fn require_kind(&self, kind: Kind) -> Result<()> {
        if self.currentness.is_some() {
            require_currentness_kind(kind)
        } else {
            Ok(())
        }
    }

    pub(super) fn from_currentness(root: root_control::CurrentnessRoot<'work>) -> Self {
        Self {
            root: None,
            currentness: Some(root),
        }
    }
    pub(super) fn require_carriage(&self, carriage: &Carriage) -> Result<()> {
        if let Some(root) = &self.currentness {
            root.require_carriage(carriage.identity().as_bytes())?;
        }
        Ok(())
    }

    pub(super) fn validate_service(
        &self,
        admission: &Admission<'_>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        if let Some(root) = &self.currentness {
            root.validate(admission, b)?;
        }
        Ok(())
    }

    /// The consumed endpoint stays prepaid. Return only growth above that charge;
    /// the enclosing native service keeps all reservations on its original account.
    pub(super) fn from_root(
        endpoint: root_control::RootEndpoint<'work>,
        manifest: &Manifest,
        b: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let input = root_control::RootEndpoint::STORAGE + manifest.retained_storage();
        b.with_prepaid_scope(input, 8, CONTEXT_WORK, CONTEXT_FRAME, |b| {
            let (manifest, charge) = Manifest::decode(manifest.canonical_bytes(), b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = root_control::RootEndpoint::STORAGE
                .checked_add(manifest.retained_storage())
                .and_then(|n| n.checked_add(size_of::<Self>()))
                .ok_or(Resource::Arithmetic)?;
            let growth = retained - root_control::RootEndpoint::STORAGE;
            Ok((
                Self {
                    root: Some(RootContext {
                        endpoint,
                        manifest,
                        retained,
                    }),
                    currentness: None,
                },
                growth,
            ))
        })
    }

    pub(super) fn retained_storage(&self) -> usize {
        self.root.as_ref().map_or(0, |root| root.retained)
            + self
                .currentness
                .as_ref()
                .map_or(0, |root| root.retained_storage())
    }

    fn root(&self) -> Result<&RootContext<'work>> {
        self.root.as_ref().ok_or_else(|| {
            Error::rejected("V3 occurrence requires the original authenticated root channel")
        })
    }

    pub(super) fn observe(
        &self,
        a: &Admission<'_>,
        b: &mut Budget<'_>,
    ) -> Result<(NativeOccurrence, usize)> {
        let floor = self
            .retained_storage()
            .checked_add(a.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, CONTEXT_WORK, CONTEXT_FRAME, |b| {
            let root = self.root()?;
            let (reply, charge) =
                root.endpoint
                    .exchange(a, &root.manifest, RootKind::Observe, &[], b)?;
            b.reserve_storage(charge.additional_storage())?;
            let bytes = reply.payload();
            if bytes.len() != OBSERVED_BYTES || bytes[..32] == [0; 32] {
                return Err(Error::rejected("root observation payload"));
            }
            let (subject, charge) = Subject::decode(&bytes[32..], b)?;
            b.reserve_storage(charge.retained_storage())?;
            let mut identity = [0; 32];
            identity.copy_from_slice(&bytes[..32]);
            Ok((
                NativeOccurrence { subject, identity },
                NativeOccurrence::STORAGE,
            ))
        })
    }

    pub(super) fn validate(
        &self,
        a: &Admission<'_>,
        occurrence: &NativeOccurrence,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(a.retained_storage())
            .and_then(|n| n.checked_add(occurrence.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            floor,
            8,
            CONTEXT_WORK,
            CONTEXT_FRAME + OBSERVED_BYTES,
            |b| {
                let root = self.root()?;
                let mut payload = [0; OBSERVED_BYTES];
                payload[..32].copy_from_slice(occurrence.identity());
                payload[32..].copy_from_slice(occurrence.subject().canonical_bytes());
                let (reply, charge) =
                    root.endpoint
                        .exchange(a, &root.manifest, RootKind::Validate, &payload, b)?;
                b.reserve_storage(charge.additional_storage())?;
                if reply.payload() != payload {
                    return Err(Error::rejected(
                        "root validation changed retained occurrence",
                    ));
                }
                Ok(())
            },
        )
    }

    /// Only Session::retire calls this, AFTER Ledger::retirement_carriage has
    /// joined the actual completed Worker, published anchor, Ready issuer and
    /// exact last ACK. Canonical carriage decoding alone cannot reach this call.
    pub(super) fn retire(
        &self,
        a: &Admission<'_>,
        occurrence: Option<&NativeOccurrence>,
        carriage: &Carriage,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(a.retained_storage())
            .and_then(|n| n.checked_add(carriage.retained_storage()))
            .and_then(|n| n.checked_add(occurrence.map_or(0, NativeOccurrence::retained_storage)))
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, CONTEXT_WORK, CONTEXT_FRAME, |b| {
            let root = self.root()?;
            if let Some(occurrence) = occurrence {
                if carriage.publication().compiler_occurrence_identity() != *occurrence.identity()
                    || carriage.request().subject().canonical_bytes()
                        != occurrence.subject().canonical_bytes()
                {
                    return Err(Error::rejected(
                        "root retirement changed original occurrence",
                    ));
                }
            }
            let (reply, charge) = root.endpoint.exchange(
                a,
                &root.manifest,
                RootKind::Retire,
                carriage.canonical_bytes(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            if reply.payload() != carriage.identity().as_bytes() {
                return Err(Error::rejected(
                    "root retirement reply changed durable carriage",
                ));
            }
            Ok(())
        })
    }
}

fn require_currentness_kind(kind: Kind) -> Result<()> {
    match kind {
        Kind::VerifyCurrent | Kind::Cancel => Ok(()),
        Kind::Prepare | Kind::Issue | Kind::Publish | Kind::Recover | Kind::Inspect => Err(
            Error::rejected("application issuer only permits currentness or cancellation"),
        ),
    }
}

#[cfg(test)]
mod currentness_role_tests {
    use super::*;
    #[test]
    fn currentness_role_excludes_every_compiler_issuance_and_recovery_operation() {
        for kind in [
            Kind::Prepare,
            Kind::Issue,
            Kind::Publish,
            Kind::Recover,
            Kind::Inspect,
        ] {
            assert!(require_currentness_kind(kind).is_err());
        }
        assert!(require_currentness_kind(Kind::VerifyCurrent).is_ok());
        assert!(require_currentness_kind(Kind::Cancel).is_ok());
    }
    #[test]
    fn existing_default_context_does_not_implicitly_select_currentness() {
        let context = OccurrenceContext::default();
        assert!(context.currentness.is_none());
        assert!(context.require_available().is_err());
        assert!(context.root().is_err());
    }
}
