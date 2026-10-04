// Lock, observation and exact-subject join shared by nominal native families.
use crate::compiler_execution_supervision::NativeObservationSource as Source;

#[derive(Debug)]
pub(crate) enum NativeOccurrenceError {
    Resource(Resource),
    Observation(NativeObservationError),
    Handoff(HandoffError),
    Subject(SubjectError),
    Expected(super::ProtectedCompilerExecutionOccurrenceErrorV1),
    Mismatch,
}
impl std::fmt::Display for NativeOccurrenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "{e}"),
            Self::Observation(e) => write!(f, "{e}"),
            Self::Handoff(e) => write!(f, "{e}"),
            Self::Subject(e) => write!(f, "{e}"),
            Self::Expected(e) => write!(f, "{e}"),
            Self::Mismatch => f.write_str("native compiler occurrence mismatch"),
        }
    }
}
type Result<T> = std::result::Result<T, NativeOccurrenceError>;
macro_rules! from_error {
    ($t:ty, $v:ident) => {
        impl From<$t> for NativeOccurrenceError {
            fn from(e: $t) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(NativeObservationError, Observation);
from_error!(HandoffError, Handoff);
from_error!(SubjectError, Subject);
from_error!(super::ProtectedCompilerExecutionOccurrenceErrorV1, Expected);

pub(crate) struct NativeOccurrence {
    observation: NativeObservation,
    publication: Lease,
    token: Token,
    subject: Subject,
    identity: [u8; 32],
    retained: usize,
}
impl NativeOccurrence {
    const FRAME: usize = 16 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3 + 8192;
    const WORK: usize = 4096 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;

    pub(crate) fn observe(service: &Service, b: &mut Budget<'_>) -> Result<(Self, usize)> {
        Self::observe_from(Source::Service(service), b)
    }

    fn observe_from(source: Source<'_, '_, '_>, b: &mut Budget<'_>) -> Result<(Self, usize)> {
        b.with_prepaid_scope(source.retained_storage(), 8, Self::WORK, Self::FRAME, |b| {
            let (observation, observation_storage) = NativeObservation::observe_from(source, b)?;
            b.reserve_storage(observation_storage)?;
            let expected = Expected::derive(observation.descriptor())?;
            let output = observation.output_dir();
            let receipt = recover(&output, &expected.producer, expected.attempt, b)?;
            let (publication, storage) = acquire(&output, &expected.producer, receipt, b)?;
            b.reserve_storage(storage.retained_storage())?;
            let (token, storage) = publication.acquire_current_token(b)?;
            b.reserve_storage(storage.retained_storage())?;
            let (subject, identity) = join_subject(&observation, &expected, receipt, &token, b)?;
            let retained = observation_storage
                .checked_add(publication.storage().retained_storage())
                .and_then(|n| n.checked_add(token.storage().retained_storage()))
                .and_then(|n| n.checked_add(size_of::<(Subject, SubjectStorage)>()))
                .and_then(|n| n.checked_add(size_of::<Self>()))
                .ok_or(Resource::Arithmetic)?;
            let occurrence = Self {
                observation,
                publication,
                token,
                subject,
                identity,
                retained,
            };
            // All components are covered by the staged frame and their charges.
            occurrence.revalidate_from(source, b)?;
            Ok((occurrence, retained))
        })
    }

    pub(crate) fn revalidate(&self, service: &Service, b: &mut Budget<'_>) -> Result<()> {
        self.revalidate_from(Source::Service(service), b)
    }

    fn revalidate_from(&self, source: Source<'_, '_, '_>, b: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(source.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, 16, 128, |b| {
            // validate_current_token is a fixed Arc-identity comparison, not I/O.
            self.publication.validate_current_token(&self.token)?;
            self.observation.revalidate_from(source, b)?;
            self.token.revalidate_locked_currentness(b)?;
            Ok(())
        })
    }
    pub(crate) fn subject(&self) -> &Subject {
        &self.subject
    }
    pub(crate) fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
}

// Shared exact join, under the caller's prepaid occurrence construction frame.
// Observation/lock retention and post-join currentness checks remain with its owner.
fn join_subject(
    observation: &NativeObservation,
    expected: &Expected,
    receipt: Receipt,
    token: &Token,
    b: &mut Budget<'_>,
) -> Result<(Subject, [u8; 32])> {
    require_joined_invocation(observation, token)?;
    let (subject, storage) = Subject::from_publication(receipt, token.handoff(), b)?;
    b.reserve_storage(storage.retained_storage())?;
    finish_join_subject(observation, expected, subject)
}

fn require_joined_invocation(observation: &NativeObservation, token: &Token) -> Result<()> {
    if published_invocation(token) != observation.descriptor() {
        return Err(NativeOccurrenceError::Mismatch);
    }
    Ok(())
}

fn finish_join_subject(
    observation: &NativeObservation,
    expected: &Expected,
    subject: Subject,
) -> Result<(Subject, [u8; 32])> {
    if subject.attempt() != expected.attempt
        || subject.rustc_invocation_sha256() != &expected.invocation_digest
        || subject.compiler_closure() != *observation.descriptor().compiler_closure()
    {
        return Err(NativeOccurrenceError::Mismatch);
    }
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_DOMAIN);
    digest.update(observation.identity());
    digest.update(subject.canonical_bytes());
    Ok((subject, digest.finalize().into()))
}
