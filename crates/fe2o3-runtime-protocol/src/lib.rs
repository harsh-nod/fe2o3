#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod application_handoff_v3;
mod static_application;
mod worker_v3_load_envelope;

pub use application_handoff_v3::{
    MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1, MAX_WORKER_V3_APPLICATION_INPUTS_V1,
    MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1, WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1, WORKER_V3_APPLICATION_HANDOFF_VERSION_V1,
    WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1, WorkerV3ApplicationHandoffAckV1,
    WorkerV3ApplicationHandoffChallengeV1, WorkerV3ApplicationHandoffCodecBudgetV1,
    WorkerV3ApplicationHandoffCommitmentV1, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffProtocolErrorV1, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceIdentityV1,
    WorkerV3ApplicationOccurrenceV1, WorkerV3LoadEnvelopeIdentityV1,
};
pub use static_application::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
};
pub use worker_v3_load_envelope::{
    MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
    RecoveredWorkerV3LoadEnvelopeV1, WORKER_V3_LOAD_ENVELOPE_MAGIC_V1,
    WORKER_V3_LOAD_ENVELOPE_VERSION_V1, WorkerV3LoadEnvelopeBindingFieldV1,
    WorkerV3LoadEnvelopeCodecBudgetV1, WorkerV3LoadEnvelopeErrorV1, WorkerV3LoadEnvelopeV1,
    WorkerV3LoadEnvelopeWireV1, recover_worker_v3_load_envelope_v1,
};
