//! Isolated public run_session fixtures. Source coverage is not execution evidence.
//! The primary must run these ignored cases serially in the opted-in root container
//! with the same separately measured static launcher and V2/V3 readiness images.
//! Select each native_session_{ready,missing_eof,trailing} coordinator with --exact
//! under native_consuming_test_process::session::{v2,v3}; never select private roles.
//! Opt-ins: FE2O3_RUN_NATIVE_SESSION_SUPERVISOR_V2_TEST=1 and the V3 counterpart.
use super::{Case, Family, LIFECYCLE_TIMEOUT, MeasuredImage, Mode};
use crate::authority_v2_test_process::{
    IO_TIMEOUT, frame, receive_packet, receive_sized_packet, send_packet,
};
use crate::launch_v2::test_support::{Witness, fd_inventory, pidfd_references};
use crate::{
    MAX_PROTECTED_ISSUER_PROCESSES_V1 as CAPACITY, ProtectedIssuerCleanupErrorV2 as CleanupError,
    ProtectedIssuerCleanupServiceV2 as Cleanup, ProtectedIssuerTerminationV1 as Termination,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceProcessProfileV2 as Profile, require_owned_sigchld_v2,
};
use std::{
    os::{fd::OwnedFd, unix::fs::PermissionsExt},
    path::PathBuf,
    time::{Duration, Instant},
};

const REQUEST_WORK: usize = 1_000_000_000_000;
const REQUEST_STORAGE: usize = 2 * 1024 * 1024 * 1024;
const REQUEST_PREFIX: usize = 37;
const EXTRA: usize = 19;
const SERVICE_WORK: usize = 10_000_000;
const SERVICE_PREFIX: usize = 53;
const CLEANUP_TURNS: usize = 4096;

mod v2 {
    use crate::authority_v2::tests::bound_consuming_fixture;
    use crate::{
        AcceptedCompilerExecutionHandoffV2 as Accepted, ProtectedIssuerBoundaryV2 as Boundary,
        ProtectedIssuerLaunchErrorV2 as LaunchError, ProtectedIssuerSessionErrorV2 as SessionError,
        ProtectedIssuerSessionLimitsV2 as SessionLimits, ProtectedIssuerWaitV2 as Wait,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
        CompilerExecutionServiceLaunchManifestV2 as Manifest,
    };
    const FAMILY: super::Family = super::Family::V2;
    include!("native_session_tests.rs");
}

mod v3 {
    use crate::authority_v3::tests::bound_consuming_fixture;
    use crate::{
        AcceptedCompilerExecutionHandoffV3 as Accepted, ProtectedIssuerBoundaryV3 as Boundary,
        ProtectedIssuerLaunchErrorV3 as LaunchError, ProtectedIssuerSessionErrorV3 as SessionError,
        ProtectedIssuerSessionLimitsV3 as SessionLimits, ProtectedIssuerWaitV3 as Wait,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
        CompilerExecutionServiceLaunchManifestV3 as Manifest,
    };
    const FAMILY: super::Family = super::Family::V3;
    include!("native_session_tests.rs");
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("fe2o3-native-session-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir(&self.0) {
            if std::thread::panicking() {
                eprintln!("native session root cleanup failed: {error}");
            } else {
                panic!("native session root is not empty or cannot be removed: {error}");
            }
        }
    }
}

fn require_profile(budget: &mut Budget<'_>) {
    let credentials = crate::IssuerServiceCredentialProfileV1::new(65_533, 65_533).unwrap();
    let retained = {
        let (profile, delta) = Profile::capture(credentials, budget).unwrap();
        budget.reserve_storage(delta.additional_storage()).unwrap();
        profile.revalidate_current(budget).unwrap();
        require_owned_sigchld_v2(budget).unwrap();
        delta.additional_storage()
    };
    budget.release_storage(retained).unwrap();
}

fn drain_cleanup(cleanup: &mut Cleanup) -> Account {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut previous = cleanup.report().unwrap().work;
    for _ in 0..CLEANUP_TURNS {
        assert!(Instant::now() < deadline, "native session cleanup deadline");
        let report = cleanup.pump(CAPACITY).unwrap();
        assert!(report.work > previous);
        assert_eq!(report.work_limit, SERVICE_WORK);
        assert_eq!(report.storage, Cleanup::STORAGE);
        assert_eq!(report.failed_work, None);
        previous = report.work;
        match cleanup.shutdown() {
            Ok(account) => {
                assert_eq!(account.storage(), 0);
                assert_eq!(account.peak_storage(), Cleanup::STORAGE);
                assert_eq!(account.work_limit(), SERVICE_WORK);
                assert!(account.work() >= previous);
                assert_eq!(account.failed_work(), None);
                assert_eq!(account.failed_storage(), None);
                return account;
            }
            Err(CleanupError::Busy) => std::thread::yield_now(),
            Err(error) => panic!("native session cleanup shutdown refused: {error:?}"),
        }
    }
    panic!("native session cleanup exhausted its funded turns");
}
