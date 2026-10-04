use fe2o3_host::{
    BlockSizeV1, DimensionsV1, KernelBrand, KernelId, LaunchConstraintsV1, ObservedContext,
};

struct Kernel;

fn forge(context: ObservedContext) {
    let dimensions = DimensionsV1::new(1, 1, 1).unwrap();
    let contract =
        LaunchConstraintsV1::new(1, BlockSizeV1::Any, dimensions, 1, 0, 0).unwrap();
    let _forged = KernelBrand::<Kernel>::from_internal_binding(
        KernelId::from_bytes([0; 32]),
        contract,
        context,
    );
}

fn main() {}
