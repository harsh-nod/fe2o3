//! Preserve the authenticated route while sharing issuer lifecycle mechanics.

use fe2o3_broker_authority_service::{
    CompilerExecutionObserverErrorV1, ObservedApplicationRegistrationV1,
    ObservedCustodianApplicationRegistrationV1, RegisteredApplicationObserverV1,
    RegisteredCustodianApplicationObserverV1,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV1;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationCustodianSupervisorReadyV1, WorkerV3ApplicationSupervisorReadyErrorV1,
    WorkerV3ApplicationSupervisorReadyV1,
};
use std::time::Instant;

pub(crate) enum RegisteredApplicationRouteV1 {
    Legacy(RegisteredApplicationObserverV1),
    Custodian(RegisteredCustodianApplicationObserverV1),
}

impl RegisteredApplicationRouteV1 {
    #[cfg(test)]
    pub(crate) fn binding(
        &self,
    ) -> &fe2o3_runtime_protocol::WorkerV3ApplicationRegistrationBindingV1 {
        match self {
            Self::Legacy(value) => value.binding(),
            Self::Custodian(value) => value.binding(),
        }
    }

    pub(crate) fn await_observation(
        self,
        deadline: Instant,
    ) -> Result<ObservedApplicationRouteV1, CompilerExecutionObserverErrorV1> {
        match self {
            Self::Legacy(value) => value
                .await_observation(deadline)
                .map(ObservedApplicationRouteV1::Legacy),
            Self::Custodian(value) => value
                .await_observation(deadline)
                .map(ObservedApplicationRouteV1::Custodian),
        }
    }
}

pub(crate) enum ObservedApplicationRouteV1 {
    Legacy(ObservedApplicationRegistrationV1),
    Custodian(ObservedCustodianApplicationRegistrationV1),
}

// Keep bounded canonical records inline; publication after observation must not allocate.
#[allow(clippy::large_enum_variant)]
pub(crate) enum ApplicationReadinessRecordV1 {
    Legacy(WorkerV3ApplicationSupervisorReadyV1),
    Custodian(WorkerV3ApplicationCustodianSupervisorReadyV1),
}

impl ApplicationReadinessRecordV1 {
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(value) => value.canonical_bytes(),
            Self::Custodian(value) => value.canonical_bytes(),
        }
    }
}

impl ObservedApplicationRouteV1 {
    pub(crate) fn readiness_bytes(
        &self,
        compiler: CompilerExecutionServiceReadyV1,
    ) -> Result<ApplicationReadinessRecordV1, WorkerV3ApplicationSupervisorReadyErrorV1> {
        match self {
            Self::Legacy(value) => Ok(ApplicationReadinessRecordV1::Legacy(
                WorkerV3ApplicationSupervisorReadyV1::new(value.binding(), compiler)?,
            )),
            Self::Custodian(value) => Ok(ApplicationReadinessRecordV1::Custodian(
                WorkerV3ApplicationCustodianSupervisorReadyV1::new(
                    WorkerV3ApplicationSupervisorReadyV1::new(value.binding(), compiler)?,
                ),
            )),
        }
    }

    pub(crate) fn revalidate(&self) -> Result<(), CompilerExecutionObserverErrorV1> {
        match self {
            Self::Legacy(value) => value.revalidate(),
            Self::Custodian(value) => value.revalidate(),
        }
    }

    pub(crate) fn confirm_publication(
        self,
        deadline: Instant,
    ) -> Result<(), CompilerExecutionObserverErrorV1> {
        match self {
            Self::Legacy(value) => value.confirm_publication(deadline),
            Self::Custodian(value) => value.confirm_publication(deadline),
        }
    }
}
