#![no_std]

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, KernelError, kernel};

macro_rules! root {
    ($name:ident) => {
        #[kernel(
            typed,
            launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1], static_shared_memory_bytes = 256)
        )]
        pub fn $name(
            mut context: KernelContext<'_>,
            mut output: Global<'_, f32, DisjointWrite<Index1D>>,
            value: f32,
        ) {
            let global = context.invocation().index_1d().into_disjoint();
            let observed = context.with_workgroup(|workgroup| {
                let mut memory = workgroup.allocate_memory::<f32, 64>();
                let index = workgroup.memory_index_1d().ok_or(KernelError::OutOfBounds)?;
                if !memory.store(&workgroup, index.into_disjoint(), value) {
                    return Err(KernelError::OutOfBounds);
                }
                let (workgroup, memory) = workgroup.publish_memory(memory);
                memory.load(&workgroup, 0).ok_or(KernelError::OutOfBounds)
            });
            let Ok(observed) = observed else { return; };
            let _stored = output.store(global, observed);
        }
    };
}

root!(scoped_index_primary);
root!(scoped_index_other);
