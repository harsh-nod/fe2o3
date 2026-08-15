use fe2o3_core::GpuContext;
use fe2o3_host::{GeneratedWorkgroupScopedAtomicV1HostAdapterV1, JoinedWorkgroupScopedAtomicV1};

fn rebind_host(
    value: GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'_, '_, '_>,
    context: &GpuContext,
) {
    let _ = value.with_context(context);
    let _ = value.with_device(1);
    let _ = value.with_runtime([7; 16]);
}

fn rebind_joined(value: JoinedWorkgroupScopedAtomicV1<'_, '_, '_>) {
    let _ = value.with_context([1; 16]);
    let _ = value.with_device(1);
    let _ = value.with_runtime([2; 16]);
}

fn main() {}
