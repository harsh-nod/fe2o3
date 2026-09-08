use fe2o3_host::__generated::{
    BlockSize, CompilerGeneratedKernelExpectationV2, GeneratedArgumentLayoutError,
};
use fe2o3_host::{
    AdmittedGeneratedHostContractV2, AqlDispatchGeometryV1, AqlGeometryError,
    AuthenticatedWorkerV3ExecutableV1, CheckedGfx942XnackMinusDevice, GeneratedHostContractErrorV2,
    GeneratedHostDispatchEvidenceV2, GeneratedHostLaunchGeometryV2, GeneratedHostPrepareErrorV2,
    GeneratedWorkerV3KfdInvocation, GeneratedWorkerV3KfdInvocationError,
    PreparedGeneratedHostInvocationV2, ProductionGeneratedHostFactsV2,
    ReviewedGeneratedHostAsyncBackendV2,
};
use fe2o3_kernel_descriptor::{DeviceDescriptorTableV1, KernelDescriptorV1};
use std::fmt;

const DEFAULT_TIMEOUT_MILLISECONDS: u32 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VecaddLaunchPlan {
    grid_x: u32,
    workgroup: [u32; 3],
}

impl VecaddLaunchPlan {
    fn for_lengths(a: usize, b: usize, c: usize) -> Result<Self, VecaddPrepareError> {
        let workgroup = exact_generated_workgroup()?;
        Self::for_lengths_and_workgroup(a, b, c, workgroup)
    }

    fn for_lengths_and_workgroup(
        a: usize,
        b: usize,
        c: usize,
        workgroup: [u32; 3],
    ) -> Result<Self, VecaddPrepareError> {
        let expected = exact_generated_workgroup()?;
        if workgroup != expected {
            return Err(VecaddPrepareError::LaunchGeometry);
        }
        if a != c || b != c {
            return Err(VecaddPrepareError::Shape { a, b, c });
        }
        let element_count = u32::try_from(c).map_err(|_| VecaddPrepareError::ElementCount)?;
        let grid_x = element_count
            .max(1)
            .checked_add(workgroup[0] - 1)
            .ok_or(VecaddPrepareError::ElementCount)?
            / workgroup[0]
            * workgroup[0];
        Ok(Self { grid_x, workgroup })
    }

    const fn host_geometry(self) -> GeneratedHostLaunchGeometryV2 {
        GeneratedHostLaunchGeometryV2::new([self.grid_x, 1, 1], self.workgroup, 0)
    }

    fn aql_geometry(self) -> Result<AqlDispatchGeometryV1, AqlGeometryError> {
        AqlDispatchGeometryV1::new([self.grid_x, 1, 1], self.workgroup)
    }
}

fn exact_generated_workgroup() -> Result<[u32; 3], VecaddPrepareError> {
    let contract = <crate::vecadd_gpu::Marker as CompilerGeneratedKernelExpectationV2>::
        generated_host_contract_v2()
        .map_err(VecaddPrepareError::GeneratedMetadata)?;
    let BlockSize::Exact(dimensions) = contract.launch().block_size() else {
        return Err(VecaddPrepareError::LaunchGeometry);
    };
    Ok([dimensions.x(), dimensions.y(), dimensions.z()])
}

/// Derives vecadd's complete dispatch geometry from generated launch metadata.
pub fn vecadd_launch_geometry(
    a: usize,
    b: usize,
    c: usize,
) -> Result<AqlDispatchGeometryV1, VecaddPrepareError> {
    VecaddLaunchPlan::for_lengths(a, b, c)?
        .aql_geometry()
        .map_err(|_| VecaddPrepareError::LaunchGeometry)
}

/// Protected admission and dispatch evidence supplied by the production host join.
pub struct ProtectedVecaddPrerequisite<'evidence> {
    admission: &'evidence AdmittedGeneratedHostContractV2<crate::vecadd_gpu::Marker>,
    evidence: GeneratedHostDispatchEvidenceV2,
}

impl<'evidence> ProtectedVecaddPrerequisite<'evidence> {
    pub const fn new(
        admission: &'evidence AdmittedGeneratedHostContractV2<crate::vecadd_gpu::Marker>,
        evidence: GeneratedHostDispatchEvidenceV2,
    ) -> Self {
        Self {
            admission,
            evidence,
        }
    }
}

#[derive(Debug)]
pub enum VecaddPrepareError {
    Shape { a: usize, b: usize, c: usize },
    ElementCount,
    GeneratedMetadata(GeneratedArgumentLayoutError),
    LaunchGeometry,
    Contract(GeneratedHostPrepareErrorV2),
}

impl fmt::Display for VecaddPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { a, b, c } => {
                write!(formatter, "vecadd lengths differ: a={a}, b={b}, c={c}")
            }
            Self::ElementCount => {
                formatter.write_str("vecadd element count exceeds u32 launch geometry")
            }
            Self::GeneratedMetadata(error) => error.fmt(formatter),
            Self::LaunchGeometry => formatter
                .write_str("vecadd launch geometry does not match compiler-generated metadata"),
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for VecaddPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shape { .. } | Self::ElementCount | Self::LaunchGeometry => None,
            Self::GeneratedMetadata(error) => Some(error),
            Self::Contract(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum VecaddKfdPrepareError {
    Request(VecaddPrepareError),
    Geometry,
    Contract(GeneratedWorkerV3KfdInvocationError),
}

impl fmt::Display for VecaddKfdPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(error) => error.fmt(formatter),
            Self::Geometry => formatter.write_str("invalid vecadd AQL launch geometry"),
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for VecaddKfdPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::Geometry => None,
            Self::Contract(error) => Some(error),
        }
    }
}

/// Joins compiler-owned V13 facts to independently inspected descriptor facts.
///
/// W6 must supply `production`; vecadd deliberately has no digest-based or test-only
/// constructor for this transition.
pub fn admit_protected_vecadd(
    table: &DeviceDescriptorTableV1,
    descriptor: &KernelDescriptorV1,
    production: ProductionGeneratedHostFactsV2,
) -> Result<AdmittedGeneratedHostContractV2<crate::vecadd_gpu::Marker>, GeneratedHostContractErrorV2>
{
    fe2o3_host::admit_generated_host_contract_v2(table, descriptor, production)
}

/// Prepares vecadd without publishing work and retains every borrow through completion.
pub fn prepare_protected_vecadd<'runtime, 'allocation, Backend, Context, Stream>(
    prerequisite: ProtectedVecaddPrerequisite<'_>,
    backend: &'runtime mut Backend,
    context: &'runtime mut Context,
    stream: &'runtime Stream,
    a: &'allocation [f32],
    b: &'allocation [f32],
    c: &'allocation mut [f32],
) -> Result<
    PreparedGeneratedHostInvocationV2<
        'runtime,
        crate::vecadd_gpu::Marker,
        Backend,
        Context,
        Stream,
        crate::vecadd_gpu::Arguments<'allocation>,
    >,
    VecaddPrepareError,
>
where
    Backend: ReviewedGeneratedHostAsyncBackendV2<
            crate::vecadd_gpu::Marker,
            Context,
            Stream,
            crate::vecadd_gpu::Arguments<'allocation>,
        >,
{
    let plan = VecaddLaunchPlan::for_lengths(a.len(), b.len(), c.len())?;
    let arguments = crate::vecadd_gpu::Arguments::new(
        fe2o3_host::__generated::GeneratedKfdReadSlice::new(a),
        fe2o3_host::__generated::GeneratedKfdReadSlice::new(b),
        fe2o3_host::__generated::GeneratedKfdWriteSlice::new(c),
    );
    prerequisite
        .admission
        .prepare(
            backend,
            context,
            stream,
            prerequisite.evidence,
            plan.host_geometry(),
            arguments,
        )
        .map_err(VecaddPrepareError::Contract)
}

/// Prepares the generated vecadd ABI through the protected direct-KFD boundary.
///
/// The returned invocation retains every argument borrow and is the only value that
/// can enter `execute`. Admission requires the exact sealed V5 result and its applicable
/// machine-refinement receipt to remain in Worker custody through dispatch.
#[allow(clippy::too_many_arguments)]
pub fn prepare_protected_vecadd_kfd<'allocation>(
    prerequisite: ProtectedVecaddPrerequisite<'_>,
    authenticated: AuthenticatedWorkerV3ExecutableV1<crate::vecadd_gpu::Marker>,
    device: CheckedGfx942XnackMinusDevice,
    a: &'allocation [f32],
    b: &'allocation [f32],
    c: &'allocation mut [f32],
) -> Result<
    GeneratedWorkerV3KfdInvocation<'allocation, crate::vecadd_gpu::Marker>,
    VecaddKfdPrepareError,
> {
    let plan = VecaddLaunchPlan::for_lengths(a.len(), b.len(), c.len())
        .map_err(VecaddKfdPrepareError::Request)?;
    let geometry = plan
        .aql_geometry()
        .map_err(|_| VecaddKfdPrepareError::Geometry)?;
    let arguments = crate::vecadd_gpu::Arguments::new(
        fe2o3_host::__generated::GeneratedKfdReadSlice::new(a),
        fe2o3_host::__generated::GeneratedKfdReadSlice::new(b),
        fe2o3_host::__generated::GeneratedKfdWriteSlice::new(c),
    );
    prerequisite
        .admission
        .prepare_direct_kfd_invocation_v2(
            authenticated,
            prerequisite.evidence,
            arguments,
            device,
            geometry,
            0,
            DEFAULT_TIMEOUT_MILLISECONDS,
        )
        .map_err(VecaddKfdPrepareError::Contract)
}

#[cfg(test)]
mod tests {
    use super::{VecaddLaunchPlan, VecaddPrepareError};

    #[allow(dead_code)]
    fn generated_arguments_typecheck<'allocation>(
        a: &'allocation [f32],
        b: &'allocation [f32],
        c: &'allocation mut [f32],
    ) {
        let arguments: crate::vecadd_gpu::Arguments<'allocation> =
            crate::vecadd_gpu::Arguments::new(
                fe2o3_host::__generated::GeneratedKfdReadSlice::new(a),
                fe2o3_host::__generated::GeneratedKfdReadSlice::new(b),
                fe2o3_host::__generated::GeneratedKfdWriteSlice::new(c),
            );
        drop(arguments);
    }

    #[test]
    fn bounds_and_launch_are_checked_before_host_or_kfd_preparation() {
        assert_eq!(
            VecaddLaunchPlan::for_lengths(65, 65, 65).unwrap(),
            VecaddLaunchPlan {
                grid_x: 128,
                workgroup: [64, 1, 1],
            }
        );
        assert!(matches!(
            VecaddLaunchPlan::for_lengths(64, 63, 64),
            Err(VecaddPrepareError::Shape {
                a: 64,
                b: 63,
                c: 64
            })
        ));
        assert!(matches!(
            VecaddLaunchPlan::for_lengths(usize::MAX, usize::MAX, usize::MAX),
            Err(VecaddPrepareError::ElementCount)
        ));
    }

    #[test]
    fn generated_launch_accepts_64_and_rejects_32() {
        let accepted = VecaddLaunchPlan::for_lengths_and_workgroup(65, 65, 65, [64, 1, 1])
            .expect("the generated exact workgroup must be accepted");
        assert_eq!(accepted.workgroup, [64, 1, 1]);
        assert!(matches!(
            VecaddLaunchPlan::for_lengths_and_workgroup(65, 65, 65, [32, 1, 1]),
            Err(VecaddPrepareError::LaunchGeometry)
        ));
    }
}
