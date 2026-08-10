use fe2o3_host::{
    CompilerGeneratedKernelContractV1, InertLoadedWorkerV2KernelSelectionV1, LoadedHsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1, WorkerV2PrerequisiteAuthenticatorV1,
};

fn extract_kernel<P, S, A, Auth>(
    loaded: &mut LoadedHsaExecutableV1<P, A>,
    selection: InertLoadedWorkerV2KernelSelectionV1<'_, S>,
    authenticator: &mut Auth,
) where
    S: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
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
    let _kernel = resolved.kernel;
}

fn main() {}
