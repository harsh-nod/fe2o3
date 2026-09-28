//! Native descriptor-only peer serving over the existing wire I/O and scheduler.
use crate::{
    DurableExternalAnchorV2, DurableExternalAnchorV3,
    native::{NativeExternalAnchorErrorV2 as Error, NativeExternalAnchorStorageV2 as Storage},
    peer_io::{self, IoStep},
    peer_loop::{self, PeerSession},
    service::{
        EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1, ExternalAnchorServiceReportV1 as Report,
        NoopServiceHooksV1, ServiceHooksV1,
    },
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV2, CompilerExecutionExternalAnchorDeploymentV3,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1 as CHALLENGE_BYTES,
    ANCHOR_OBSERVATION_WIRE_LEN_V1 as OBSERVATION_BYTES,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{mem::size_of, os::fd::OwnedFd, time::Duration};

type Result<T> = std::result::Result<T, Error>;
type Challenge = [u8; CHALLENGE_BYTES];
type Observation = [u8; OBSERVATION_BYTES];

/// Prepaid consumed peer charge. Retire it after serving returns, on either outcome.
pub const NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2: usize = size_of::<(OwnedFd, Storage)>();
/// Full unreserved successful report charge, independent of consumed peer cleanup.
pub const NATIVE_EXTERNAL_ANCHOR_PEER_REPORT_STORAGE_V2: usize = size_of::<(Report, Storage)>();
/// Outer entry, credential checks and fixed frame setup. I/O attempts and key/anchor
/// operations additionally charge the original ledger as they occur.
pub const NATIVE_EXTERNAL_ANCHOR_PEER_WORK_V2: usize =
    8 + 16 * 1024 + 32 * (CHALLENGE_BYTES + OBSERVATION_BYTES);
/// Outer live packet/control frame, not an allocator, generated-stack or RSS bound.
/// Peak additionally includes nested anchor scratch or a retained response charge.
pub const NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2: usize =
    4 * CHALLENGE_BYTES + 4 * OBSERVATION_BYTES + 4 * size_of::<libc::sockaddr_un>() + 8192;

// Closed implementations preserve nominal deployment types without exposing a
// caller-supplied signer, persistence engine, transport, or legacy upgrade.
trait NativeAnchor {
    type Deployment;
    fn retained_storage(&self) -> usize;
    fn deployment_storage(d: &Self::Deployment) -> usize;
    fn validate(&self, d: &Self::Deployment, b: &mut Budget<'_>) -> Result<()>;
    fn exchange(
        &mut self,
        bytes: &Challenge,
        d: &Self::Deployment,
        b: &mut Budget<'_>,
    ) -> Result<(Observation, Storage)>;
}
macro_rules! anchor {
    ($A:ident, $D:ident) => {
        impl NativeAnchor for $A {
            type Deployment = $D;
            fn retained_storage(&self) -> usize {
                self.retained_storage()
            }
            fn deployment_storage(d: &Self::Deployment) -> usize {
                d.retained_storage()
            }
            fn validate(&self, d: &Self::Deployment, b: &mut Budget<'_>) -> Result<()> {
                self.validate_service(d, b)
            }
            fn exchange(
                &mut self,
                bytes: &Challenge,
                d: &Self::Deployment,
                b: &mut Budget<'_>,
            ) -> Result<(Observation, Storage)> {
                self.exchange(bytes, d, b)
            }
        }
    };
}
anchor!(
    DurableExternalAnchorV2,
    CompilerExecutionExternalAnchorDeploymentV2
);
anchor!(
    DurableExternalAnchorV3,
    CompilerExecutionExternalAnchorDeploymentV3
);

trait PeerTransport {
    fn validate(&mut self, peer: &OwnedFd, b: &mut Budget<'_>) -> Result<()>;
    fn receive(&mut self, peer: &OwnedFd, b: &mut Budget<'_>) -> Result<Option<Challenge>>;
    fn send(
        &mut self,
        peer: &OwnedFd,
        bytes: &Observation,
        timeout: Duration,
        b: &mut Budget<'_>,
    ) -> Result<()>;
}
struct SystemPeer;
impl PeerTransport for SystemPeer {
    fn validate(&mut self, peer: &OwnedFd, b: &mut Budget<'_>) -> Result<()> {
        peer_io::validate_peer(peer, &mut |step: IoStep| {
            b.charge_work(step.work())?;
            Ok(())
        })
    }
    fn receive(&mut self, peer: &OwnedFd, b: &mut Budget<'_>) -> Result<Option<Challenge>> {
        peer_io::receive_challenge(peer, &mut |step: IoStep| {
            b.charge_work(step.work())?;
            Ok(())
        })
    }
    fn send(
        &mut self,
        peer: &OwnedFd,
        bytes: &Observation,
        timeout: Duration,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        peer_io::send_observation(peer, bytes, timeout, &mut |step: IoStep| {
            b.charge_work(step.work())?;
            Ok(())
        })
    }
}

struct ChargedObservation {
    bytes: Observation,
    charge: Storage,
}
struct NativeSession<'a, 'work, A: NativeAnchor, T> {
    anchor: &'a mut A,
    deployment: &'a A::Deployment,
    budget: &'a mut Budget<'work>,
    transport: &'a mut T,
}
impl<A: NativeAnchor, T: PeerTransport> PeerSession for NativeSession<'_, '_, A, T> {
    type Error = Error;
    type Observation = ChargedObservation;
    fn receive(&mut self, peer: &OwnedFd) -> Result<Option<Challenge>> {
        self.transport.receive(peer, self.budget)
    }
    fn exchange(&mut self, challenge: &Challenge) -> Result<Self::Observation> {
        let (bytes, charge) = self
            .anchor
            .exchange(challenge, self.deployment, self.budget)?;
        self.budget.reserve_storage(charge.additional_storage())?;
        Ok(ChargedObservation { bytes, charge })
    }
    fn send(
        &mut self,
        peer: &OwnedFd,
        observation: &Self::Observation,
        timeout: Duration,
    ) -> Result<()> {
        self.transport
            .send(peer, &observation.bytes, timeout, self.budget)
    }
    fn retire(&mut self, observation: Self::Observation) -> Result<()> {
        let bytes = observation.charge.additional_storage();
        drop(observation);
        self.budget.release_storage(bytes)?;
        Ok(())
    }
}

fn serve<A: NativeAnchor, T: PeerTransport>(
    anchor: &mut A,
    deployment: &A::Deployment,
    peer: OwnedFd,
    b: &mut Budget<'_>,
    transport: &mut T,
    hooks: &mut impl ServiceHooksV1,
    timeout: Duration,
) -> Result<(Report, Storage)> {
    let floor = anchor
        .retained_storage()
        .checked_add(A::deployment_storage(deployment))
        .and_then(|n| n.checked_add(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2))
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(
        floor,
        8,
        NATIVE_EXTERNAL_ANCHOR_PEER_WORK_V2,
        NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2,
        |b| {
            anchor.validate(deployment, b)?;
            transport.validate(&peer, b)?;
            let mut session = NativeSession {
                anchor,
                deployment,
                budget: b,
                transport,
            };
            let report = peer_loop::run(&mut session, &peer, timeout, hooks)?;
            Ok((
                report,
                Storage(NATIVE_EXTERNAL_ANCHOR_PEER_REPORT_STORAGE_V2),
            ))
        },
    )
}

macro_rules! serve_native {
    ($function:ident, $A:ident, $D:ident, $other:ident) => {
        /// Serves one consumed connected peer using native key/deployment custody.
        /// Requires prepaid peer, full anchor and actual deployment charges on the
        /// original ledger. Retire PEER_STORAGE after return/error; on success reserve
        /// the returned FULL report charge separately. Anchor/deployment stay borrowed.
        /// Every poll/receive/send attempt is charged before I/O, including retries.
        /// Logical quotas do not bound idle waits, syscall latency or service lifetime.
        /// Errors/unwinds close the peer and restore entry storage, preserving work,
        /// peak and first-denial history. A failure after persistence may leave a commit
        /// without a response; reopen/recovery uses the durable state, never rollback.
        /// Caller must independently admit protected peer identity, process profile and
        /// lifecycle. Endpoint shape and native custody alone grant no compiler authority.
        /// The report is the same inert exchange counter used by V1, not a V1-owner upgrade.
        ///
        /// ```
        #[doc = concat!("use fe2o3_external_anchor_service::{", stringify!($function), ", ", stringify!($A), "};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::", stringify!($D), ";")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc = concat!("fn serve(a: &mut ", stringify!($A), ", d: &", stringify!($D), ", peer: std::os::fd::OwnedFd, b: &mut Budget<'_>) {")]
        #[doc = concat!("    let _ = ", stringify!($function), "(a, d, peer, b);")]
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::{", stringify!($function), ", ", stringify!($A), "};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::", stringify!($other), " as Other;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc = concat!("fn mix(a: &mut ", stringify!($A), ", d: &Other, peer: std::os::fd::OwnedFd, b: &mut Budget<'_>) {")]
        #[doc = concat!("    let _ = ", stringify!($function), "(a, d, peer, b);")]
        /// }
        /// ```
        pub fn $function(anchor: &mut $A, deployment: &$D, peer: OwnedFd, b: &mut Budget<'_>)
            -> Result<(Report, Storage)> {
            serve(anchor, deployment, peer, b, &mut SystemPeer, &mut NoopServiceHooksV1,
                EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1)
        }
    };
}
serve_native!(
    serve_connected_peer_v2,
    DurableExternalAnchorV2,
    CompilerExecutionExternalAnchorDeploymentV2,
    CompilerExecutionExternalAnchorDeploymentV3
);
serve_native!(
    serve_connected_peer_v3,
    DurableExternalAnchorV3,
    CompilerExecutionExternalAnchorDeploymentV3,
    CompilerExecutionExternalAnchorDeploymentV2
);

#[cfg(test)]
mod tests;
