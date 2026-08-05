use fe2o3_host::{
    CheckedDimensions, KernelId, ObservedContext, PreparedGeometry, PreparedLaunch,
    PreparedResources,
};

struct Kernel;

fn forge(context: ObservedContext) {
    let _forged = PreparedLaunch::<Kernel> {
        kernel: KernelId::from_bytes([0; 32]),
        context,
        geometry: PreparedGeometry {
            rank: 1,
            grid: CheckedDimensions {
                dimensions: [1, 1, 1],
                product: 1,
            },
            block: CheckedDimensions {
                dimensions: [1, 1, 1],
                product: 1,
            },
            total_threads: 1,
        },
        resources: PreparedResources {
            static_shared_memory_bytes: 0,
            dynamic_shared_memory_bytes: 0,
            total_shared_memory_bytes: 0,
        },
        marker: std::marker::PhantomData,
    };
}

fn main() {}
