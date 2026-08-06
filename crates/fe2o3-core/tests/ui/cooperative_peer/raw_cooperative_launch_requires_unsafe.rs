use fe2o3_core::{
    GpuFunction, KernelParams, LaunchConfig, Result, Stream,
    launch_cooperative_kernel_on_stream,
};

fn bypass(
    function: &GpuFunction,
    stream: &Stream,
    params: &mut KernelParams,
) -> Result<()> {
    launch_cooperative_kernel_on_stream(
        function,
        LaunchConfig::for_num_elems(1),
        stream,
        params,
    )
}

fn main() {}
