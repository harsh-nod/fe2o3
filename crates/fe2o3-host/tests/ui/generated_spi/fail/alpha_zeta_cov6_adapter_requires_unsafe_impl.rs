use fe2o3_host::__generated::{
    AlphaZetaCov6DispatchIdentityV1, CompilerGeneratedArgumentLayoutV1,
    CompilerGeneratedAlphaZetaCov6ArgumentsV1, CompilerGeneratedKernelExpectationV1,
    GeneratedArgumentLayoutError,
    GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1,
    GeneratedAlphaZetaCov6ArgumentBindingV1,
};

struct Arguments;

impl<K: CompilerGeneratedKernelExpectationV1>
    CompilerGeneratedAlphaZetaCov6ArgumentsV1<'static, K> for Arguments
{
    fn dispatch_identity_v1() -> AlphaZetaCov6DispatchIdentityV1 {
        unimplemented!()
    }

    fn generated_argument_layout_v1(
    ) -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        unimplemented!()
    }

    fn bind_arguments_v1(
        &self,
        _plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedAlphaZetaCov6ArgumentBindingV1<'static>, GeneratedArgumentPackError> {
        unimplemented!()
    }
}

fn main() {}
