use fe2o3_host::{
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedWorkerV3ArgumentsV1, GeneratedArgumentLayoutError,
    GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1,
    GeneratedWorkerV3ArgumentBindingV1,
};

struct SafeArguments;

impl<'allocation, K> CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K> for SafeArguments
where
    K: CompilerGeneratedKernelExpectationV1,
{
    fn generated_argument_layout_v1(
    ) -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        unimplemented!()
    }

    fn bind_arguments_v1(
        &self,
        _plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedWorkerV3ArgumentBindingV1<'allocation>, GeneratedArgumentPackError> {
        unimplemented!()
    }
}

fn main() {}
