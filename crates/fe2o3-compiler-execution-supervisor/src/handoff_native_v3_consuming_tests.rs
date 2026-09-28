use crate::authority_v2_test_process::*;
use crate::authority_v3::tests::measured_policy;
use crate::handoff_v2_test_process::{enable_pidfd, send_excess_rights};
use crate::native_consuming_test_process::{
    Case as ConsumingCase, Family, LIFECYCLE_TIMEOUT, MeasuredImage,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionServiceLaunchManifestV3 as Manifest, CompilerExecutionServiceReadyV3 as Ready,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    os::fd::AsFd,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const UID: u32 = 65_532;
const FAMILY: Family = Family::V3;

#[test]
#[ignore = "private V3 submitter role, selected only by the consuming coordinator"]
fn submitter_process_helper() {
    require_child_credentials(FAMILY.submitter_role(), UID);
    let control = inherited_control();
    let (request, []) = receive_packet::<0>(&control, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&request[..4], FAMILY.case_tag());
    let case = ConsumingCase::from_id(u32::from_le_bytes(request[4..].try_into().unwrap()));
    run_submitter(control, Some(case));
}

include!("handoff_native_submitter_tests.rs");
