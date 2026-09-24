use super::*;

/// Bounded refusal from native inherited admission or its consuming service.
#[derive(Debug)]
pub enum CompilerExecutionIssuerEntrypointErrorV2 {
    Resource(Resource),
    Descriptor(crate::CompilerExecutionIssuerEntrypointErrorV1),
    Process(fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionErrorV1),
    ExpectedClient(fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV1),
    Inputs(crate::CompilerExecutionIssuerLaunchInputErrorV2),
    Policy(fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2),
    Key(fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2),
    Client(fe2o3_broker_authority_service::LiveClientPidfdErrorV2),
    ServiceAdmission(fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV2),
    Anchor(fe2o3_broker_authority_service::ProtectedExternalAnchorServiceErrorV2),
    Admission(fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionErrorV2),
    Service(fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerServiceErrorV2),
}
macro_rules! from_error {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for Error {
            fn from(error: $ty) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(crate::CompilerExecutionIssuerEntrypointErrorV1, Descriptor);
from_error!(crate::CompilerExecutionIssuerLaunchInputErrorV2, Inputs);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2,
    Policy
);
from_error!(
    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2,
    Key
);
from_error!(
    fe2o3_broker_authority_service::LiveClientPidfdErrorV2,
    Client
);
from_error!(
    fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV2,
    ServiceAdmission
);
from_error!(
    fe2o3_broker_authority_service::ProtectedExternalAnchorServiceErrorV2,
    Anchor
);
from_error!(
    fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionErrorV2,
    Admission
);
from_error!(
    fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerServiceErrorV2,
    Service
);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native issuer entrypoint refused: ")?;
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Descriptor(e) => e.fmt(f),
            Self::Process(e) => e.fmt(f),
            Self::ExpectedClient(e) => e.fmt(f),
            Self::Inputs(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Key(e) => e.fmt(f),
            Self::Client(e) => e.fmt(f),
            Self::ServiceAdmission(e) => e.fmt(f),
            Self::Anchor(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::Service(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
