use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedScalarGemmV1Arguments,
    RecoveredWorkerV2SynchronousHsaHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
    WorkerV2PrerequisiteAuthenticatorV1,
};

fn unload_while_prepared<'allocation, Root, Selected, Adapter, Authenticator, Arguments>(
    mut authority: RecoveredWorkerV2SynchronousHsaHandoffV1<Root, Adapter>,
    authenticator: &mut Authenticator,
    arguments: Arguments,
) where
    Selected: CompilerGeneratedKernelExpectationV1,
    Adapter: ReviewedHsaImplicitKernargAdapterV1,
    Authenticator: WorkerV2PrerequisiteAuthenticatorV1<Selected>,
    Arguments: CompilerGeneratedScalarGemmV1Arguments<'allocation, Selected>,
{
    let prepared = match authority.prepare_generated_scalar_gemm_v1::<
        Selected,
        Authenticator,
        Arguments,
    >(authenticator, arguments) {
        Ok(prepared) => prepared,
        Err(_) => return,
    };
    let _ = authority.unload();
    drop(prepared);
}

fn main() {}
