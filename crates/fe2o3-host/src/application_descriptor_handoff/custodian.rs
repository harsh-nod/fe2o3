//! One evolving controller owner, retained inside the original application descriptors.
use super::*;
use fe2o3_compiler_execution_client::{
    RegisteredApplicationCustodianV1, RetainedApplicationProofV1,
};

/// The strict custodian handoff and the original publication lock held across its ACK.
/// This owner cannot export either admission or controller, and is not an executable.
///
/// ```
/// fn send_sync<T: Send + Sync>() {}
/// send_sync::<fe2o3_host::RegisteredWorkerV3CustodianApplicationV1>();
/// ```
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_host::RegisteredWorkerV3CustodianApplicationV1>();
/// ```
/// ```compile_fail
/// fn extract(owner: fe2o3_host::RegisteredWorkerV3CustodianApplicationV1) {
///     let _ = owner.into_parts();
/// }
/// ```
#[derive(Debug)]
pub struct RegisteredWorkerV3CustodianApplicationV1 {
    pub(super) admission: RecoveredWorkerV3PinnedDescriptorV1,
    pub(super) current: DurableCurrentLinkPublicationTokenV1,
}

impl RegisteredWorkerV3CustodianApplicationV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        RecoveredWorkerV3PinnedDescriptorV1,
        DurableCurrentLinkPublicationTokenV1,
    ) {
        (self.admission, self.current)
    }
}

pub(super) enum ApplicationCustodianStateV1 {
    Registered(Box<RegisteredApplicationCustodianV1>),
    Proved(Box<RetainedApplicationProofV1>),
    Terminal,
}

pub(super) fn terminal() -> WorkerV3ApplicationDescriptorHandoffErrorV1 {
    WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel(
        ApplicationProofChannelErrorV1::Invalid(
            "application custodian is absent, busy or terminal",
        ),
    )
}

impl ApplicationCustodianStateV1 {
    pub(super) fn register(
        endpoint: RetainedApplicationProofEndpointV1,
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        deadline: Instant,
    ) -> Result<Self, WorkerV3ApplicationDescriptorHandoffErrorV1> {
        endpoint
            .register_custodian_pre_ack(inputs, deadline)
            .map(|owner| Self::Registered(Box::new(owner)))
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel)
    }

    pub(super) fn revalidate(&self) -> Result<(), ApplicationProofChannelErrorV1> {
        match self {
            Self::Registered(owner) => owner.revalidate(),
            Self::Proved(owner) => owner.revalidate(),
            Self::Terminal => Err(ApplicationProofChannelErrorV1::Invalid(
                "terminal proof custody",
            )),
        }
    }
}

impl RetainedWorkerV3ApplicationDescriptorsV1 {
    pub(crate) fn request_custodian_proof(
        &self,
        kernel: KernelId,
        payload: &[u8],
        deadline: Instant,
    ) -> Result<Vec<u8>, WorkerV3ApplicationDescriptorHandoffErrorV1> {
        self.revalidate()?;
        let subject = {
            let ApplicationRegistrationCustodyV1::Custodian(state) = &self.proof else {
                return Err(terminal());
            };
            let mut state = state.try_lock().map_err(|_| terminal())?;
            let ApplicationCustodianStateV1::Registered(owner) =
                std::mem::replace(&mut *state, ApplicationCustodianStateV1::Terminal)
            else {
                return Err(terminal());
            };
            // No publication/currentness callbacks while locked: those re-enter this owner.
            let proof = owner
                .request_proof(
                    *kernel.as_bytes(),
                    &self.exact_envelope_bytes,
                    payload,
                    deadline,
                )
                .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel)?;
            let subject = proof.subject_bytes().to_vec();
            *state = ApplicationCustodianStateV1::Proved(Box::new(proof));
            subject
        };
        self.revalidate()?;
        Ok(subject)
    }

    pub(crate) fn probe_custodian_proof(
        &self,
        deadline: Instant,
    ) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
        self.revalidate()?;
        {
            let ApplicationRegistrationCustodyV1::Custodian(state) = &self.proof else {
                return Err(terminal());
            };
            let mut state = state.try_lock().map_err(|_| terminal())?;
            let ApplicationCustodianStateV1::Proved(owner) = &mut *state else {
                return Err(terminal());
            };
            owner
                .probe(deadline)
                .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel)?;
        }
        self.revalidate()
    }
}
