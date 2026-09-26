use crate::authority_v3::tests::bound_consuming_fixture;
use crate::{
    ProtectedIssuerBoundaryV3 as Boundary, ProtectedIssuerLaunchErrorV3 as LaunchError,
    ProtectedIssuerWaitV3 as Wait,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES;

include!("native_consuming_tests.rs");
