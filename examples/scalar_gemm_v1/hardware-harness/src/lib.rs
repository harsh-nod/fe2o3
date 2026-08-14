use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::__generated::{
    GeneratedScalarGemmV1ReadDeviceSlice, GeneratedScalarGemmV1ReadWriteDeviceSlice,
};
use fe2o3_host::{
    ObservedContext, RecoveredWorkerV2SynchronousHsaDispatchError,
    RecoveredWorkerV2SynchronousHsaHandoffV1, WorkerV2PrerequisiteAuthenticatorV1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_scalar_gemm_v1::harness::{
    HARDWARE_CASES, HardwareCase, SCALAR_GEMM_WORKGROUP_X, scalar_gemm_oracle,
};
use fe2o3_scalar_gemm_v1::kernel::scalar_gemm_v1_gpu;
use std::error::Error;
use std::fmt::{self, Debug};
use std::io;
use std::sync::Arc;

type BoxError = Box<dyn Error + Send + Sync>;
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_5a5a);
const LEFT_CANARY: f32 = f32::from_bits(0x7fc0_1234);
const RIGHT_CANARY: f32 = f32::from_bits(0x7fc0_5678);
const CANARY_ELEMENTS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardwareCaseEvidence {
    pub name: &'static str,
    pub dimensions: [u32; 3],
    pub groups: Option<u32>,
    pub dispatched: bool,
    pub output_elements: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardwareSuiteEvidence {
    pub cases: Vec<HardwareCaseEvidence>,
    pub executable_released: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingAuthenticatedScalarGemmCapability;

impl fmt::Display for MissingAuthenticatedScalarGemmCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "an inspected, current, authenticated Scalar GEMM V1 Worker V2 HSA capability is required",
        )
    }
}

impl Error for MissingAuthenticatedScalarGemmCapability {}

pub fn fail_closed_without_authenticated_capability()
-> Result<HardwareSuiteEvidence, MissingAuthenticatedScalarGemmCapability> {
    Err(MissingAuthenticatedScalarGemmCapability)
}

/// Runnable Scalar GEMM hardware evidence with exact, linear recovered HSA authority.
///
/// Construction has no bytes-or-path overload. The recovered value must already
/// have passed Worker V2 admission, prerequisite authentication, reviewed HSA
/// authorization, exact-byte loading, exact-symbol resolution, and retained
/// publication/application currentness admission.
///
/// A lower-level loaded executable cannot cross this boundary:
///
/// ```compile_fail
/// use fe2o3_core::GpuContext;
/// use fe2o3_host::{
///     LoadedHsaExecutableV1, ObservedContext, WorkerV2PrerequisiteAuthenticatorV1,
/// };
/// use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
/// use fe2o3_scalar_gemm_v1::kernel::scalar_gemm_v1_gpu;
/// use fe2o3_scalar_gemm_v1_hardware_harness::AuthenticatedScalarGemmHarness;
/// use std::fmt::Debug;
/// use std::sync::Arc;
///
/// fn cannot_downgrade<Authenticator>(
///     loaded: LoadedHsaExecutableV1<
///         scalar_gemm_v1_gpu::Marker,
///         ReviewedHsaRuntimeAdapterV1,
///     >,
///     context: &Arc<GpuContext>,
///     observed: &ObservedContext,
///     authenticator: &mut Authenticator,
/// ) where
///     Authenticator: WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
///     Authenticator::Error: Debug,
/// {
///     let _ = AuthenticatedScalarGemmHarness::new(
///         loaded,
///         context,
///         observed,
///         authenticator,
///     );
/// }
/// ```
///
/// Transferring recovered authority into the harness is linear:
///
/// ```compile_fail
/// use fe2o3_core::GpuContext;
/// use fe2o3_host::{
///     ObservedContext, RecoveredWorkerV2SynchronousHsaHandoffV1,
///     WorkerV2PrerequisiteAuthenticatorV1,
/// };
/// use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
/// use fe2o3_scalar_gemm_v1::kernel::scalar_gemm_v1_gpu;
/// use fe2o3_scalar_gemm_v1_hardware_harness::AuthenticatedScalarGemmHarness;
/// use std::fmt::Debug;
/// use std::sync::Arc;
///
/// fn cannot_reuse<Authenticator>(
///     authority: RecoveredWorkerV2SynchronousHsaHandoffV1<
///         scalar_gemm_v1_gpu::Marker,
///         ReviewedHsaRuntimeAdapterV1,
///     >,
///     context: &Arc<GpuContext>,
///     observed: &ObservedContext,
///     authenticator: &mut Authenticator,
/// ) where
///     Authenticator: WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
///     Authenticator::Error: Debug,
/// {
///     let _harness = AuthenticatedScalarGemmHarness::new(
///         authority,
///         context,
///         observed,
///         authenticator,
///     );
///     let _ = authority.load_observation();
/// }
/// ```
pub struct AuthenticatedScalarGemmHarness<'context, 'authenticator, Authenticator>
where
    Authenticator: WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
{
    authority: RecoveredWorkerV2SynchronousHsaHandoffV1<
        scalar_gemm_v1_gpu::Marker,
        ReviewedHsaRuntimeAdapterV1,
    >,
    context: &'context Arc<GpuContext>,
    observed: &'context ObservedContext,
    authenticator: &'authenticator mut Authenticator,
}

impl<'context, 'authenticator, Authenticator>
    AuthenticatedScalarGemmHarness<'context, 'authenticator, Authenticator>
where
    Authenticator: WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
    Authenticator::Error: Debug,
{
    pub fn new(
        authority: RecoveredWorkerV2SynchronousHsaHandoffV1<
            scalar_gemm_v1_gpu::Marker,
            ReviewedHsaRuntimeAdapterV1,
        >,
        context: &'context Arc<GpuContext>,
        observed: &'context ObservedContext,
        authenticator: &'authenticator mut Authenticator,
    ) -> Result<Self, BoxError> {
        if !observed.is_for_context(context) || observed.device().target() != "gfx942:xnack-" {
            return Err(failure(
                "Scalar GEMM V1 hardware evidence requires the exact observed gfx942:xnack- context",
            ));
        }
        Ok(Self {
            authority,
            context,
            observed,
            authenticator,
        })
    }

    pub fn run(mut self) -> Result<HardwareSuiteEvidence, BoxError> {
        let mut cases = Vec::with_capacity(HARDWARE_CASES.len());
        for case in HARDWARE_CASES {
            cases.push(self.run_case(*case)?);
        }
        let unloaded = self
            .authority
            .unload()
            .map_err(|error| failure(format!("recovered HSA unload failed: {error:?}")))?;
        let executable_released = unloaded.unload_observation().released();
        if !executable_released {
            return Err(failure("reviewed HSA unload did not report release"));
        }
        Ok(HardwareSuiteEvidence {
            cases,
            executable_released,
        })
    }

    fn run_case(&mut self, case: HardwareCase) -> Result<HardwareCaseEvidence, BoxError> {
        let shape = case.shape()?;
        let dimensions = shape.dimensions();
        let expected_groups = shape.expected_groups()?;
        let (a_host, b_host) = case.inputs(shape);
        let expected = scalar_gemm_oracle(shape, &a_host, &b_host);
        let stream = self.context.default_stream();

        let a = DeviceBuffer::from_host(&stream, &a_host)?;
        let b = DeviceBuffer::from_host(&stream, &b_host)?;
        let left_canary_host = vec![LEFT_CANARY; CANARY_ELEMENTS];
        let right_canary_host = vec![RIGHT_CANARY; CANARY_ELEMENTS];
        let mut guarded_host = Vec::with_capacity(CANARY_ELEMENTS * 2 + shape.c_len);
        guarded_host.extend_from_slice(&left_canary_host);
        guarded_host.extend(std::iter::repeat_n(OUTPUT_POISON, shape.c_len));
        guarded_host.extend_from_slice(&right_canary_host);
        let mut guarded = DeviceBuffer::from_host(&stream, &guarded_host)?;

        let expected_geometry = expected_groups.map(|groups| {
            fe2o3_host::HsaLaunchGeometryV1::new([groups, 1, 1], [SCALAR_GEMM_WORKGROUP_X, 1, 1], 0)
        });
        let dispatched = {
            let (_left_canary, c, _right_canary) =
                guarded.split_range_mut(CANARY_ELEMENTS..CANARY_ELEMENTS + shape.c_len)?;
            let arguments = scalar_gemm_v1_gpu::Arguments::new(
                GeneratedScalarGemmV1ReadDeviceSlice::new(self.observed, &a)?,
                GeneratedScalarGemmV1ReadDeviceSlice::new(self.observed, &b)?,
                GeneratedScalarGemmV1ReadWriteDeviceSlice::from_view_mut(self.observed, c)?,
                case.m,
                case.n,
                case.k,
            );
            let prepared = self
                .authority
                .prepare_generated_scalar_gemm_v1::<scalar_gemm_v1_gpu::Marker, Authenticator, _>(
                    self.authenticator,
                    arguments,
                )
                .map_err(|error| {
                    failure(format!(
                        "{} recovered Scalar GEMM preparation failed: {error:?}",
                        case.name
                    ))
                })?;
            if prepared.geometry() != expected_geometry {
                return Err(failure(format!(
                    "{} generated geometry did not use rounded WG256 launch",
                    case.name
                )));
            }
            let completion = require_current_recovered_dispatch(case.name, prepared.dispatch())?;
            if completion.was_dispatched() != expected_groups.is_some() {
                return Err(failure(format!(
                    "{} zero-output dispatch state changed",
                    case.name
                )));
            }
            if let Some(completed) = completion.completed_dispatch()
                && (!completed.dispatch().completed()
                    || completed.geometry() != expected_geometry.expect("dispatch has geometry"))
            {
                return Err(failure(format!(
                    "{} did not return exact synchronous completion",
                    case.name
                )));
            }
            completion.was_dispatched()
        };

        let a_after = a.to_host_vec(&stream)?;
        let b_after = b.to_host_vec(&stream)?;
        let guarded_after = guarded.to_host_vec(&stream)?;
        let (left_canary_after, remainder) = guarded_after.split_at(CANARY_ELEMENTS);
        let (c_after, right_canary_after) = remainder.split_at(shape.c_len);
        require_bits_equal(case.name, "A input", &a_after, &a_host)?;
        require_bits_equal(case.name, "B input", &b_after, &b_host)?;
        require_bits_equal(case.name, "C output", c_after, &expected)?;
        require_bits_equal(
            case.name,
            "left output canary",
            left_canary_after,
            &left_canary_host,
        )?;
        require_bits_equal(
            case.name,
            "right output canary",
            right_canary_after,
            &right_canary_host,
        )?;
        if case.k == 0 && c_after.iter().any(|value| value.to_bits() != 0) {
            return Err(failure(format!(
                "{} did not write positive zero for k=0",
                case.name
            )));
        }

        Ok(HardwareCaseEvidence {
            name: case.name,
            dimensions,
            groups: expected_groups,
            dispatched,
            output_elements: shape.c_len,
        })
    }
}

fn require_current_recovered_dispatch<Completion, AdapterError: Debug>(
    case: &str,
    result: Result<Completion, RecoveredWorkerV2SynchronousHsaDispatchError<AdapterError>>,
) -> Result<Completion, BoxError> {
    result.map_err(|error| match error {
        RecoveredWorkerV2SynchronousHsaDispatchError::CurrentPublication(source) => failure(
            format!("{case} stale Worker V2 publication prevented HSA dispatch: {source}"),
        ),
        #[cfg(target_os = "linux")]
        RecoveredWorkerV2SynchronousHsaDispatchError::ApplicationDescriptors(source) => failure(
            format!("{case} stale application descriptors prevented HSA dispatch: {source}"),
        ),
        RecoveredWorkerV2SynchronousHsaDispatchError::Dispatch(source) => {
            failure(format!("{case} HSA dispatch failed: {source:?}"))
        }
        _ => failure(format!(
            "{case} recovered authority rejected HSA dispatch: {error:?}"
        )),
    })
}

fn require_bits_equal(
    case: &str,
    role: &str,
    actual: &[f32],
    expected: &[f32],
) -> Result<(), BoxError> {
    if actual.len() != expected.len() {
        return Err(failure(format!(
            "{case} {role} length changed: {} != {}",
            actual.len(),
            expected.len()
        )));
    }
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        if actual.to_bits() != expected.to_bits() {
            return Err(failure(format!(
                "{case} {role}[{index}] changed: {:#010x} != {:#010x}",
                actual.to_bits(),
                expected.to_bits()
            )));
        }
    }
    Ok(())
}

fn failure(message: impl Into<String>) -> BoxError {
    Box::new(io::Error::other(message.into()))
}

#[cfg(test)]
mod tests {
    use super::{
        HARDWARE_CASES, MissingAuthenticatedScalarGemmCapability,
        fail_closed_without_authenticated_capability, require_current_recovered_dispatch,
    };
    use fe2o3_artifact_transaction::DurableLinkPublicationError;
    use fe2o3_host::RecoveredWorkerV2SynchronousHsaDispatchError;

    const SOURCE: &str = include_str!("lib.rs");

    #[test]
    fn missing_authenticated_artifact_fails_closed() {
        assert_eq!(
            fail_closed_without_authenticated_capability(),
            Err(MissingAuthenticatedScalarGemmCapability)
        );
    }

    #[test]
    fn controller_covers_required_correctness_observations() {
        assert!(HARDWARE_CASES.iter().any(|case| case.name == "zero-k"));
        assert!(
            HARDWARE_CASES
                .iter()
                .any(|case| case.name == "fma-distinguishing")
        );
        for required in [
            "expected_groups",
            "completion.was_dispatched()",
            "positive zero for k=0",
            "A input",
            "B input",
            "C output",
            "left output canary",
            "right output canary",
        ] {
            assert!(SOURCE.contains(required), "missing `{required}`");
        }
    }

    #[test]
    fn authority_boundary_has_no_path_or_raw_pointer_bypass() {
        let production = SOURCE
            .split("#[cfg(test)]")
            .next()
            .expect("hardware harness has production source");
        let implementation = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("///"))
            .collect::<String>();
        for forbidden in [
            "FE2O3_SCALAR_GEMM_HSACO",
            "std::env",
            "read_to_end",
            "from_raw_parts",
            "as_device_ptr",
            "LoadedHsaExecutableV1",
            ".prepare(&mut self.authority",
        ] {
            assert!(!implementation.contains(forbidden), "found `{forbidden}`");
        }
        for required in [
            "RecoveredWorkerV2SynchronousHsaHandoffV1",
            "prepare_generated_scalar_gemm_v1",
            "WorkerV2PrerequisiteAuthenticatorV1",
            "ReviewedHsaRuntimeAdapterV1",
            "GeneratedScalarGemmV1ReadDeviceSlice",
            "GeneratedScalarGemmV1ReadWriteDeviceSlice",
        ] {
            assert!(implementation.contains(required), "missing `{required}`");
        }
    }

    #[test]
    fn stale_publication_binding_cannot_be_a_dispatch_success() {
        let result = require_current_recovered_dispatch::<(), ()>(
            "stale-publication",
            Err(
                RecoveredWorkerV2SynchronousHsaDispatchError::CurrentPublication(
                    DurableLinkPublicationError::CurrentPublication {
                        reason: "test generation changed".into(),
                    },
                ),
            ),
        );
        let message = result.unwrap_err().to_string();
        assert!(message.contains("stale Worker V2 publication prevented HSA dispatch"));
        assert!(message.contains("test generation changed"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stale_application_binding_cannot_be_a_dispatch_success() {
        let result = require_current_recovered_dispatch::<(), ()>(
            "stale-application",
            Err(
                RecoveredWorkerV2SynchronousHsaDispatchError::ApplicationDescriptors(
                    fe2o3_host::WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged,
                ),
            ),
        );
        let message = result.unwrap_err().to_string();
        assert!(message.contains("stale application descriptors prevented HSA dispatch"));
        assert!(message.contains("inherited Worker V2 envelope changed"));
    }
}
