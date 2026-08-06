use crate::{KernelParams, LoadedLaunchError, LoadedPreparedLaunch};
use core::fmt;
use fe2o3_core::{CooperativeLaunchCapability, Error as CoreError, Stream};

const MAX_WORKGROUPS_WITHOUT_FUNCTION_OCCUPANCY: u64 = 1;

/// Conservative residency decision for one single-device cooperative launch.
///
/// The current raw HIP boundary does not expose per-function occupancy. The
/// safe admission layer therefore accepts exactly one already validated
/// workgroup and rejects larger grids rather than trusting caller-supplied
/// occupancy. HIP still performs its authoritative residency check at enqueue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CooperativeResidencyAdmission {
    requested_workgroups: u64,
    admitted_workgroups: u64,
}

impl CooperativeResidencyAdmission {
    pub const fn requested_workgroups(self) -> u64 {
        self.requested_workgroups
    }

    pub const fn admitted_workgroups(self) -> u64 {
        self.admitted_workgroups
    }
}

/// A checked single-device cooperative launch bound to one loaded function,
/// exact stream/context, geometry, resource request, and live HIP capability.
///
/// This value cannot be converted back into an ordinary launch. Its raw launch
/// operation remains unsafe because admission does not prove argument ABI,
/// pointer validity, aliasing, race freedom, completion, or kernel semantics.
pub struct CooperativeLaunchAdmission<'loaded, 'stream, K> {
    launch: LoadedPreparedLaunch<'loaded, K>,
    stream: &'stream Stream,
    _capability: CooperativeLaunchCapability,
    residency: CooperativeResidencyAdmission,
}

impl<K> fmt::Debug for CooperativeLaunchAdmission<'_, '_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CooperativeLaunchAdmission")
            .field("identity", self.launch.identity())
            .field("geometry", &self.launch.geometry())
            .field("resources", &self.launch.resources())
            .field("residency", &self.residency)
            .finish_non_exhaustive()
    }
}

impl<'loaded, 'stream, K> CooperativeLaunchAdmission<'loaded, 'stream, K> {
    pub(crate) fn new(
        launch: LoadedPreparedLaunch<'loaded, K>,
        stream: &'stream Stream,
        capability: CooperativeLaunchCapability,
        residency: CooperativeResidencyAdmission,
    ) -> Self {
        Self {
            launch,
            stream,
            _capability: capability,
            residency,
        }
    }

    pub const fn residency(&self) -> CooperativeResidencyAdmission {
        self.residency
    }

    /// Enqueues this exact admission through HIP's cooperative module launch.
    ///
    /// The exact stream is retained by the admission and cannot be substituted.
    /// HIP's excessive-residency error is reported separately from other
    /// runtime failures.
    ///
    /// # Safety
    ///
    /// `params` must exactly match the generated kernel ABI. Every reachable
    /// allocation must belong to the admitted context, remain live through
    /// completion, be in bounds, and satisfy aliasing and synchronization
    /// requirements. The caller must retain the loaded authority through
    /// completion and establish the executable's cooperative semantics. This
    /// method only enqueues work and does not synchronize.
    pub unsafe fn launch_raw(
        self,
        params: &mut KernelParams,
    ) -> Result<(), CooperativeLaunchError> {
        let Self {
            launch,
            stream,
            _capability: _,
            residency: _,
        } = self;
        match unsafe { launch.launch_cooperative_raw_impl(stream, params) } {
            Ok(()) => Ok(()),
            Err(LoadedLaunchError::Hip(CoreError::Hip(error)))
                if error.is_cooperative_launch_too_large() =>
            {
                Err(CooperativeLaunchError::ExcessiveResidencyAtEnqueue)
            }
            Err(error) => Err(CooperativeLaunchError::Launch(error)),
        }
    }
}

pub(crate) fn validate_admission(
    stream_matches_loaded_context: bool,
    capability_matches_stream_context: bool,
    requested_workgroups: u64,
) -> Result<CooperativeResidencyAdmission, CooperativeAdmissionError> {
    if !stream_matches_loaded_context {
        return Err(CooperativeAdmissionError::WrongStreamContext);
    }
    if !capability_matches_stream_context {
        return Err(CooperativeAdmissionError::WrongCapabilityContext);
    }
    if requested_workgroups == 0 {
        return Err(CooperativeAdmissionError::InvalidPreparedGrid);
    }
    if requested_workgroups > MAX_WORKGROUPS_WITHOUT_FUNCTION_OCCUPANCY {
        return Err(CooperativeAdmissionError::ConservativeResidencyExceeded {
            requested_workgroups,
            maximum_workgroups: MAX_WORKGROUPS_WITHOUT_FUNCTION_OCCUPANCY,
        });
    }
    Ok(CooperativeResidencyAdmission {
        requested_workgroups,
        admitted_workgroups: MAX_WORKGROUPS_WITHOUT_FUNCTION_OCCUPANCY,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CooperativeAdmissionError {
    WrongStreamContext,
    WrongCapabilityContext,
    InvalidPreparedGrid,
    ConservativeResidencyExceeded {
        requested_workgroups: u64,
        maximum_workgroups: u64,
    },
}

impl fmt::Display for CooperativeAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongStreamContext => {
                formatter.write_str("cooperative stream belongs to a different context")
            }
            Self::WrongCapabilityContext => formatter
                .write_str("cooperative capability was not observed from the exact stream context"),
            Self::InvalidPreparedGrid => {
                formatter.write_str("prepared cooperative grid contains no workgroups")
            }
            Self::ConservativeResidencyExceeded {
                requested_workgroups,
                maximum_workgroups,
            } => write!(
                formatter,
                "cooperative grid requests {requested_workgroups} workgroups, but only {maximum_workgroups} can be admitted without a per-function HIP occupancy observation"
            ),
        }
    }
}

impl std::error::Error for CooperativeAdmissionError {}

#[derive(Debug)]
#[non_exhaustive]
pub enum CooperativeLaunchError {
    ExcessiveResidencyAtEnqueue,
    Launch(LoadedLaunchError),
}

impl fmt::Display for CooperativeLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExcessiveResidencyAtEnqueue => {
                formatter.write_str("HIP rejected cooperative launch residency at enqueue")
            }
            Self::Launch(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CooperativeLaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Launch(error) => Some(error),
            Self::ExcessiveResidencyAtEnqueue => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_workgroup_is_the_only_conservative_admission() {
        let admission = validate_admission(true, true, 1).unwrap();
        assert_eq!(admission.requested_workgroups(), 1);
        assert_eq!(admission.admitted_workgroups(), 1);
    }

    #[test]
    fn context_and_capability_mismatches_fail_before_residency() {
        assert_eq!(
            validate_admission(false, true, 1),
            Err(CooperativeAdmissionError::WrongStreamContext)
        );
        assert_eq!(
            validate_admission(true, false, 1),
            Err(CooperativeAdmissionError::WrongCapabilityContext)
        );
    }

    #[test]
    fn zero_and_larger_grids_fail_closed() {
        assert_eq!(
            validate_admission(true, true, 0),
            Err(CooperativeAdmissionError::InvalidPreparedGrid)
        );
        assert_eq!(
            validate_admission(true, true, 2),
            Err(CooperativeAdmissionError::ConservativeResidencyExceeded {
                requested_workgroups: 2,
                maximum_workgroups: 1,
            })
        );
        assert!(matches!(
            validate_admission(true, true, u64::MAX),
            Err(CooperativeAdmissionError::ConservativeResidencyExceeded { .. })
        ));
    }

    #[test]
    fn errors_explain_missing_authority() {
        assert!(
            CooperativeAdmissionError::ConservativeResidencyExceeded {
                requested_workgroups: 4,
                maximum_workgroups: 1,
            }
            .to_string()
            .contains("per-function HIP occupancy observation")
        );
        assert!(
            CooperativeLaunchError::ExcessiveResidencyAtEnqueue
                .to_string()
                .contains("at enqueue")
        );
    }
}
