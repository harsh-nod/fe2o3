//! One complete fail-closed protected issuer service session.

use std::error::Error;
use std::fmt;
use std::os::fd::OwnedFd;
use std::time::Duration;

use crate::{
    ExitedProtectedIssuerV1, LaunchedProtectedIssuerV1, PreparedProtectedIssuerLaunchV1,
    ProtectedIssuerHandoffErrorV1, ProtectedIssuerLaunchErrorV1,
    ProtectedIssuerLaunchPreparationErrorV1, ProtectedIssuerSupervisorV1,
};

const MAX_BOUNDARY_TIMEOUT_V1: Duration = Duration::from_secs(120);
const MAX_SESSION_TIMEOUT_V1: Duration = Duration::from_secs(24 * 60 * 60);

/// Invalid trusted timeout policy for one protected issuer session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedIssuerSessionTimeoutErrorV1 {
    /// A handoff, launch, readiness, or publication timeout is zero or exceeds two minutes.
    InvalidBoundary(&'static str),
    /// The complete serving lifetime is zero or exceeds 24 hours.
    InvalidSession,
}

impl fmt::Display for ProtectedIssuerSessionTimeoutErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBoundary(boundary) => {
                write!(formatter, "invalid protected issuer {boundary} timeout")
            }
            Self::InvalidSession => {
                formatter.write_str("invalid protected issuer serving-session timeout")
            }
        }
    }
}

impl Error for ProtectedIssuerSessionTimeoutErrorV1 {}

/// Trusted absolute timeout policy for every boundary in one service session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerSessionTimeoutsV1 {
    handoff: Duration,
    launch: Duration,
    readiness: Duration,
    publication: Duration,
    session: Duration,
}

impl ProtectedIssuerSessionTimeoutsV1 {
    /// Constructs one policy after validating every independent lifecycle bound.
    pub fn new(
        handoff: Duration,
        launch: Duration,
        readiness: Duration,
        publication: Duration,
        session: Duration,
    ) -> Result<Self, ProtectedIssuerSessionTimeoutErrorV1> {
        for (name, timeout) in [
            ("handoff", handoff),
            ("launch", launch),
            ("readiness", readiness),
            ("publication", publication),
        ] {
            if timeout.is_zero() || timeout > MAX_BOUNDARY_TIMEOUT_V1 {
                return Err(ProtectedIssuerSessionTimeoutErrorV1::InvalidBoundary(name));
            }
        }
        if session.is_zero() || session > MAX_SESSION_TIMEOUT_V1 {
            return Err(ProtectedIssuerSessionTimeoutErrorV1::InvalidSession);
        }
        Ok(Self {
            handoff,
            launch,
            readiness,
            publication,
            session,
        })
    }

    /// Returns the bound for receiving and authenticating Cargo's handoff.
    pub const fn handoff(self) -> Duration {
        self.handoff
    }

    /// Returns the complete gated clone and authenticated exec bound.
    pub const fn launch(self) -> Duration {
        self.launch
    }

    /// Returns the exact issuer readiness bound.
    pub const fn readiness(self) -> Duration {
        self.readiness
    }

    /// Returns the Cargo readiness-publication bound.
    pub const fn publication(self) -> Duration {
        self.publication
    }

    /// Returns the maximum post-publication serving lifetime.
    pub const fn session(self) -> Duration {
        self.session
    }
}

/// Exact lifecycle stage that failed while running one protected issuer session.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerSessionErrorV1 {
    /// Cargo handoff authentication failed.
    Handoff(ProtectedIssuerHandoffErrorV1),
    /// Static-launch input materialization failed.
    Preparation(ProtectedIssuerLaunchPreparationErrorV1),
    /// Gated clone or authenticated static exec failed.
    Launch(ProtectedIssuerLaunchErrorV1),
    /// Exact issuer readiness admission failed.
    Readiness(ProtectedIssuerLaunchErrorV1),
    /// Publishing admitted readiness to Cargo failed.
    Publication(ProtectedIssuerLaunchErrorV1),
    /// Natural serving termination or exactly-once reaping failed.
    Exit(ProtectedIssuerLaunchErrorV1),
}

impl fmt::Display for ProtectedIssuerSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "issuer handoff failed: {error}"),
            Self::Preparation(error) => write!(formatter, "issuer preparation failed: {error}"),
            Self::Launch(error) => write!(formatter, "issuer launch failed: {error}"),
            Self::Readiness(error) => write!(formatter, "issuer readiness failed: {error}"),
            Self::Publication(error) => write!(formatter, "issuer publication failed: {error}"),
            Self::Exit(error) => write!(formatter, "issuer serving exit failed: {error}"),
        }
    }
}

impl Error for ProtectedIssuerSessionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::Launch(error)
            | Self::Readiness(error)
            | Self::Publication(error)
            | Self::Exit(error) => Some(error),
        }
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Runs one complete production session from authenticated handoff through exact reaping.
    ///
    /// The supplied control descriptor must be one accepted connection from the protected
    /// service listener. No intermediate descriptor or authority-bearing state is returned. Any
    /// failed stage drops its move-only input and therefore closes or cancels all later authority.
    pub fn run_session(
        &self,
        control: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerSessionErrorV1> {
        self.run_session_with_hooks::<true, _, _, _>(control, timeouts, |_| (), |(), _| {})
    }

    fn run_session_with_hooks<const PRODUCTION: bool, State, AfterPreparation, AfterLaunch>(
        &self,
        control: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
        after_preparation: AfterPreparation,
        after_launch: AfterLaunch,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerSessionErrorV1>
    where
        AfterPreparation: FnOnce(&PreparedProtectedIssuerLaunchV1) -> State,
        AfterLaunch: FnOnce(State, &LaunchedProtectedIssuerV1),
    {
        let accepted = self
            .accept_handoff_inner::<PRODUCTION>(control, timeouts.handoff())
            .map_err(ProtectedIssuerSessionErrorV1::Handoff)?;
        let prepared = self
            .prepare_launch(accepted)
            .map_err(ProtectedIssuerSessionErrorV1::Preparation)?;
        let state = after_preparation(&prepared);
        let launched = self
            .launch_inner::<PRODUCTION>(prepared, timeouts.launch())
            .map_err(ProtectedIssuerSessionErrorV1::Launch)?;
        after_launch(state, &launched);
        let ready = launched
            .await_readiness(timeouts.readiness())
            .map_err(ProtectedIssuerSessionErrorV1::Readiness)?;
        let serving = ready
            .publish_readiness(timeouts.publication())
            .map_err(ProtectedIssuerSessionErrorV1::Publication)?;
        serving
            .wait_for_exit(timeouts.session())
            .map_err(ProtectedIssuerSessionErrorV1::Exit)
    }

    #[cfg(test)]
    pub(crate) fn run_session_inner<const PRODUCTION: bool, State, AfterPreparation, AfterLaunch>(
        &self,
        control: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
        after_preparation: AfterPreparation,
        after_launch: AfterLaunch,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerSessionErrorV1>
    where
        AfterPreparation: FnOnce(&PreparedProtectedIssuerLaunchV1) -> State,
        AfterLaunch: FnOnce(State, &LaunchedProtectedIssuerV1),
    {
        self.run_session_with_hooks::<PRODUCTION, _, _, _>(
            control,
            timeouts,
            after_preparation,
            after_launch,
        )
    }
}
