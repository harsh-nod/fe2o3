use crate::authority_v2::tests::bound_consuming_fixture;
use crate::{
    ProtectedIssuerBoundaryV2 as Boundary, ProtectedIssuerLaunchErrorV2 as LaunchError,
    ProtectedIssuerWaitV2 as Wait,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES;

include!("native_consuming_tests.rs");
