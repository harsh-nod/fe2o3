use fe2o3_host::{
    CompilerGeneratedKernelContractV1, HsaLaunchGeometryV1,
    InertLoadedWorkerV2KernelSelectionV1, LoadedHsaExecutableV1,
    ReviewedHsaImplicitKernargAdapterV1, WorkerV2PrerequisiteAuthenticatorV1,
};

fn launch<P, S, A, Auth>(
    loaded: &mut LoadedHsaExecutableV1<P, A>,
    selection: InertLoadedWorkerV2KernelSelectionV1<'_, S>,
    authenticator: &mut Auth,
    kernarg: &mut [u8],
) where
    S: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
    Auth: WorkerV2PrerequisiteAuthenticatorV1<S>,
{
    let expected_identity = selection.artifact_identity().clone();
    drop(selection);
    let Ok(authenticated) = loaded
        .authenticate_typed_kernel_once::<S, _>(&expected_identity, authenticator)
    else {
        return;
    };
    let Ok(resolved) = authenticated.resolve(loaded) else {
        return;
    };
    let _completed = resolved.dispatch_generated_and_wait(
        HsaLaunchGeometryV1::new([1, 1, 1], [256, 1, 1], 0),
        kernarg,
        48,
        48,
        256,
    );
}

fn main() {}
