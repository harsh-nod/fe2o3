use fe2o3_host::{
    CompilerGeneratedKernelContractV1, InertLoadedWorkerV2KernelSelectionV1, LoadedHsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1, WorkerV2PrerequisiteAuthenticatorV1,
};

fn unload_while_resolved<P, S, A, Auth>(
    mut loaded: LoadedHsaExecutableV1<P, A>,
    selection: InertLoadedWorkerV2KernelSelectionV1<'_, S>,
    authenticator: &mut Auth,
) where
    S: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
    Auth: WorkerV2PrerequisiteAuthenticatorV1<S>,
{
    let Ok(authenticated) = selection.authenticate(authenticator) else {
        return;
    };
    let Ok(resolved) = authenticated.resolve(&mut loaded) else {
        return;
    };
    let _unloaded = loaded.unload();
    drop(resolved);
}

fn main() {}
