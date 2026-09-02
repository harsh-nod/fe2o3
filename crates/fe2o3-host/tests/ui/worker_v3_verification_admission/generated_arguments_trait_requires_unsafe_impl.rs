use fe2o3_host::{
    CompilerGeneratedArgumentLayoutV1, CompilerGeneratedKfdArguments,
    CompilerGeneratedKernelExpectationV1, GeneratedArgumentLayoutError,
    GeneratedArgumentPackingPlanV1, GeneratedKfdArgumentBinding, GeneratedKfdArgumentError,
};

struct SafeArguments;

impl<'allocation, K> CompilerGeneratedKfdArguments<'allocation, K> for SafeArguments
where
    K: CompilerGeneratedKernelExpectationV1,
{
    fn generated_argument_layout(
    ) -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        unimplemented!()
    }

    fn bind_kfd_arguments(
        self,
        _plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedKfdArgumentBinding<'allocation>, GeneratedKfdArgumentError> {
        unimplemented!()
    }
}

fn main() {}
