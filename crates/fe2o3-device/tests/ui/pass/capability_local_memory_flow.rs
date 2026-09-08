use fe2o3_device::prelude::*;

enum Kernel {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn local_memory_flow<'kernel>(mut context: KernelContext<'kernel, Kernel>) {
    let mut private = context.private_memory::<u32, 4>();
    assert!(private.store(0, 7));
    let _: Option<u32> = private.load(0);

    context.with_workgroup(|workgroup| {
        let mut memory = workgroup.allocate_memory::<u32, 64>();
        let index = workgroup.memory_index_1d().unwrap().into_disjoint();
        let _: bool = memory.store(&workgroup, index, 11);
        let (workgroup, published) = workgroup.publish_memory(memory);
        let _: Option<u32> = published.load(&workgroup, 0);
    });
}

fn exact_types<'kernel, 'memory, 'workgroup, Epoch: SynchronizationEpoch>(
    private: &PrivateMemoryView<'memory, u32, ExclusiveReadWrite, Brand<'kernel>>,
    workgroup: &WorkgroupMemoryView<
        'memory,
        'workgroup,
        u32,
        ReadOnly,
        Brand<'kernel>,
        Epoch,
    >,
) {
    let _: usize = private.len();
    let _: usize = workgroup.len();
}

fn main() {
    let _ = local_memory_flow;
    let _ = exact_types::<InitialEpoch>;
}
