use fe2o3_core::GpuContext;
use fe2o3_host::GeneratedWave64CollectivesV1HostAdapterV1;

fn rebind(
    value: GeneratedWave64CollectivesV1HostAdapterV1<'_, '_, '_, '_>,
    context: &GpuContext,
) {
    let _ = value.with_context(context);
}

fn main() {}
