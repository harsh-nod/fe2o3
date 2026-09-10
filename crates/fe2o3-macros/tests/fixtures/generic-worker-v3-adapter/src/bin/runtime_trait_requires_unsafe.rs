use generic_worker_v3_adapter_fixture::transform_gpu;

struct Forged;

impl gpu_host::CompilerGeneratedRuntimeArguments<transform_gpu::Marker> for Forged {
    fn account_runtime_arguments(
        &self,
        _: &mut gpu_host::GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<(), gpu_host::GeneratedRuntimeArgumentErrorV1> {
        unimplemented!()
    }
    fn generated_argument_layout()
    -> Result<gpu_host::CompilerGeneratedArgumentLayoutV1, gpu_host::GeneratedArgumentLayoutError>
    {
        unimplemented!()
    }

    fn bind_runtime_arguments(
        self,
        _: &gpu_host::GeneratedArgumentPackingPlanV1,
        _: &mut gpu_host::GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<
        gpu_host::GeneratedRuntimeArgumentBindingV1,
        gpu_host::GeneratedRuntimeArgumentErrorV1,
    > {
        unimplemented!()
    }
}

fn main() {}
