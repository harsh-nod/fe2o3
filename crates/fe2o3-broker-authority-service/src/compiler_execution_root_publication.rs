//! Actual V5 publication custody retained in the original compiler cleanup slot.
//! Connection replacement cannot release either the lease or the locked token.
use super::*;
use fe2o3_artifact_transaction::{
    ArtifactLockRetirementBarrierErrorV1 as BarrierError,
    ArtifactLockRetirementBarrierV1 as Barrier,
    CompilerModuleHandoffCurrentnessCustodyQuoteV5 as Quote,
    acquire_compiler_module_handoff_currentness_lease_with_quote_v5 as acquire_quoted,
    quote_compiler_module_handoff_currentness_custody_v5 as custody_quote,
    try_acquire_artifact_lock_retirement_barrier_v1 as retirement_barrier,
};
use fe2o3_protected_service_spawn::{
    LateRetainedCustodyV2 as Holder, ProtectedServiceCleanupServiceV2 as Cleanup,
    cleanup_bridge::{LateRetainedBuildV2 as Build, LateRetainedPayloadV2 as Payload},
    native_spawn::{ProtectedServiceSpawnErrorV2 as SpawnError, RootRetainedTaskTraceV2 as Trace},
};

#[path = "compiler_execution_root_publication_payload.rs"]
mod payload;
use payload::{Acquire, Owners, Validate};

const ENTRY: usize = 8;
const LOCAL_WORK: usize = ENTRY + 64 * 1088;
const FRAME: usize = 16 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3 + 8192;
type Result<T> = std::result::Result<T, RootPublicationCustodyErrorV3>;

/// Originating-thread handle to actual, still-locked V5 publication custody.
/// The original cleanup slot independently retains all acquired owners. Dropping
/// this handle or replacing an issuer connection does NOT release the locks.
/// Failure after slot installation is terminal for this observation attempt:
/// cancel the same compiler trace and retain its original cleanup controller.
/// Pending/quarantined cancellation is not permission to release its custody.
///
/// This observes a compiler/publication association, not protected proof execution
/// or durable retirement permission. It grants no publication, load or GPU launch
/// authority. No explicit retirement API exists until the durable join is wired.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootPublicationCustodyV3 as C;
/// fn copy(c: C) { let _ = c.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootPublicationCustodyV3 as C;
/// fn send<T: Send>() {} send::<C>();
/// ```
pub struct RootPublicationCustodyV3 {
    observation: NativeObservation,
    owners: Holder<Owners>,
    subject: Subject,
    identity: [u8; 32],
    retained: usize,
}

impl RootPublicationCustodyV3 {
    /// Observe through the actual trace, then fund and install custody before
    /// acquiring the publication lease or token. The same original Budget is
    /// used throughout. Returned storage is FULL and unreserved; the independent
    /// persistent cleanup charge is paid before any long-lived lock is acquired.
    /// The compiler must have completed its authenticated exec/alias closure.
    pub fn observe<T: Send + 'static>(
        trace: &mut Trace<'_, T>,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        b.with_prepaid_scope(trace.retained_storage(), ENTRY, LOCAL_WORK, FRAME, |b| {
            let (observation, observed_storage) = trace.with_task_observation(b, |root, b| {
                Ok::<_, RootPublicationCustodyErrorV3>(NativeObservation::observe_from(
                    Source::Root(root),
                    b,
                )?)
            })?;
            b.reserve_storage(observed_storage)?;
            let expected =
                Expected::derive(observation.descriptor()).map_err(NativeOccurrenceError::from)?;
            let output = observation.output_dir();
            let receipt = {
                // Recovery may create temporary OutputLocks on failing paths.
                // Exclude spawns through their complete rollback/destruction.
                let _barrier = retirement_barrier()?;
                recover(&output, &expected.producer, expected.attempt, b)
                    .map_err(NativeOccurrenceError::from)?
            };
            let quote = custody_quote(&output, &expected.producer, receipt)
                .map_err(NativeOccurrenceError::from)?;
            let payload_storage = quote
                .retained_storage()
                .checked_add(size_of::<Owners>())
                .ok_or(Resource::Arithmetic)?;
            // SAFETY: the artifact quote covers the exact receipt's complete
            // future lease/token backing. Owners only adds those two owners and
            // retires them together under an independent spawn barrier.
            let (owners, charge) =
                unsafe { trace.reserve_late_custody::<Owners>(cleanup, payload_storage, b) }?;
            b.reserve_storage(charge.additional_storage())?;
            trace
                .prepare_late_attachment(&owners, b)?
                .commit(Owners::new(quote));
            let mut observed = None;
            trace.with_task_observation(b, |root, b| {
                // SAFETY: the operation borrows this trace's own observation and
                // returns only inert subject bytes. Every acquired owner stays
                // in the already funded slot, including on late scope refusal.
                unsafe {
                    trace.build_late_custody(
                        &owners,
                        Acquire {
                            root,
                            observation: &observation,
                            expected: &expected,
                            receipt,
                            output: &mut observed,
                        },
                        b,
                    )
                }
            })?;
            let (subject, identity) = observed.ok_or(RootPublicationCustodyErrorV3::state(
                "publication observation produced no subject",
            ))?;
            b.reserve_storage(size_of::<(Subject, SubjectStorage)>())?;
            let retained = observed_storage
                .checked_add(owners.retained_storage())
                .and_then(|n| n.checked_add(size_of::<(Subject, SubjectStorage)>()))
                .and_then(|n| n.checked_add(size_of::<(Self, usize)>()))
                .ok_or(Resource::Arithmetic)?;
            Ok((
                Self {
                    observation,
                    owners,
                    subject,
                    identity,
                    retained,
                },
                retained,
            ))
        })
    }

    /// Revalidate the same original trace and locked publication. No new lock,
    /// payload reconstruction, or replacement compiler occurrence is admitted.
    pub fn revalidate<T: Send + 'static>(
        &self,
        trace: &Trace<'_, T>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained
            .checked_add(trace.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
            trace.with_task_observation(b, |root, b| {
                // SAFETY: Validate only checks the installed concrete owners;
                // it cannot remove, replace, export or release their custody.
                unsafe {
                    trace.build_late_custody(
                        &self.owners,
                        Validate {
                            root,
                            observation: &self.observation,
                        },
                        b,
                    )
                }
            })
        })
    }

    /// Inert coordinates only; consumers must separately authenticate authority.
    pub fn subject(&self) -> &Subject {
        &self.subject
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
}

/// Inert refusal, never an extracted publication owner or lock.
#[derive(Debug)]
pub struct RootPublicationCustodyErrorV3(Failure);
#[derive(Debug)]
enum Failure {
    Occurrence(NativeOccurrenceError),
    Spawn(SpawnError),
    Barrier(BarrierError),
    State(&'static str),
}
impl RootPublicationCustodyErrorV3 {
    fn state(message: &'static str) -> Self {
        Self(Failure::State(message))
    }
}
impl From<NativeOccurrenceError> for RootPublicationCustodyErrorV3 {
    fn from(e: NativeOccurrenceError) -> Self {
        Self(Failure::Occurrence(e))
    }
}
impl From<NativeObservationError> for RootPublicationCustodyErrorV3 {
    fn from(e: NativeObservationError) -> Self {
        NativeOccurrenceError::from(e).into()
    }
}
impl From<Resource> for RootPublicationCustodyErrorV3 {
    fn from(e: Resource) -> Self {
        NativeOccurrenceError::from(e).into()
    }
}
impl From<SpawnError> for RootPublicationCustodyErrorV3 {
    fn from(e: SpawnError) -> Self {
        Self(Failure::Spawn(e))
    }
}
impl From<BarrierError> for RootPublicationCustodyErrorV3 {
    fn from(e: BarrierError) -> Self {
        Self(Failure::Barrier(e))
    }
}
impl std::fmt::Display for RootPublicationCustodyErrorV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Failure::Occurrence(e) => e.fmt(f),
            Failure::Spawn(e) => e.fmt(f),
            Failure::Barrier(e) => e.fmt(f),
            Failure::State(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for RootPublicationCustodyErrorV3 {}
