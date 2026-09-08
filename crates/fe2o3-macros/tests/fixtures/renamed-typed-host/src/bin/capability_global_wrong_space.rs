use gpu_device::capability_memory::{ReadOnly, WorkgroupMemoryView};
use gpu_device::{InitialEpoch, KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn wrong_space(
    context: KernelContext<'_>,
    input: WorkgroupMemoryView<'_, '_, u32, ReadOnly, (), InitialEpoch>,
) {
    let _ = (context, input);
}

fn main() {}
