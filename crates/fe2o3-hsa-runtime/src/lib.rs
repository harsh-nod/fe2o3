#![doc = include_str!("../README.md")]

mod api;
mod backend;
mod dispatch;
mod environment;
mod lifecycle;
mod sys;
#[cfg(test)]
mod test_process_execution {
    use std::io;
    use std::process::{Command, ExitStatus};

    pub(super) fn status(command: &mut Command) -> io::Result<ExitStatus> {
        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
            .and_then(|mut child| child.wait())
    }
}
pub use backend::{ReviewedHsaRuntimeBackendErrorV1, ReviewedHsaRuntimeBackendV1};
#[cfg(feature = "hardware-test-hooks")]
pub use dispatch::{
    ReviewedHsaHardwareTestBufferV1, ReviewedHsaProfiledDispatchObservationV1,
    ReviewedHsaProfiledDispatchSessionV1,
};
pub use environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
pub use lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelSetV1, ReviewedHsaKernelV1};

/// Whether the explicitly selected native HSA backend is compiled into this build.
///
/// Enabling `native-hsa` validates the required headers and libraries during the
/// build, so a successful native build always reports `true`. The default stub
/// build reports `false` without inspecting the build host.
pub const HSA_RUNTIME_AVAILABLE: bool = cfg!(feature = "native-hsa");

#[cfg(test)]
mod backend_selection_tests {
    use super::HSA_RUNTIME_AVAILABLE;

    #[cfg(not(feature = "native-hsa"))]
    #[test]
    fn default_build_is_the_stub_backend() {
        assert!(!std::hint::black_box(HSA_RUNTIME_AVAILABLE));
    }

    #[cfg(feature = "native-hsa")]
    #[test]
    fn explicitly_enabled_native_build_is_available() {
        assert!(std::hint::black_box(HSA_RUNTIME_AVAILABLE));
    }
}
