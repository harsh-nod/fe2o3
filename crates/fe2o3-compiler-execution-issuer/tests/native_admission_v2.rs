//! Public native issuer admission, not issuer activation or deployment qualification.
//!
//! Build this test target with the pinned musl toolchain and static relocation/linking.
//! Run only one exact ignored matrix (public admission or native service) at a time
//! in a disposable root container with explicit FE2O3_RUN_NATIVE_ISSUER_ADMISSION=1.
//! The runner must provide read-only root/executable, private /tmp and namespaces,
//! no network/GPU, default seccomp, no-new-privileges, drop-all capabilities plus
//! CHOWN/KILL/SETUID/SETGID, and CPU/memory/PID and 600-second outer limits.
//! Helpers are ignored subprocess roles, not independently runnable test cases.
//! No missing prerequisite is reported as a skipped success. Fixture management
//! is outside logical verification accounting; issuer operations use one original
//! budget. The client role and deliberate foreign-ledger probes own separate ledgers.
#![forbid(unsafe_code)]

use fe2o3_broker_authority_service::{
    ProtectedCompilerExecutionIssuerAdmissionErrorV2 as Error,
    ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Key;
use fe2o3_compiler_execution_client::CompilerExecutionClientV2 as WireClient;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestV2 as Manifest, CompilerExecutionServiceReadyV2 as Ready,
};

const PUBLIC_CASE_OK: &str = "FE2O3_NATIVE_ISSUER_PUBLIC_CASE_OK";
const PUBLIC_MATRIX_OK: &str = "FE2O3_NATIVE_ISSUER_PUBLIC_MATRIX_OK";
const SERVICE_CASE_OK: &str = "FE2O3_NATIVE_SERVICE_READINESS_CASE_OK";
const SERVICE_MATRIX_OK: &str = "FE2O3_NATIVE_SERVICE_READINESS_MATRIX_OK";

#[path = "native_service_v2/mod.rs"]
mod native_service;

include!("native_admission/mod.rs");
