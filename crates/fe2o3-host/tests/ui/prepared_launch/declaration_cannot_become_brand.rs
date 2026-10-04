use fe2o3_host::{
    BlockSizeV1, DimensionsV1, KernelBrand, KernelId, LaunchConstraintsV1,
    UntrustedKernelDeclaration,
};

struct Kernel;

fn main() {
    let dimensions = DimensionsV1::new(1, 1, 1).unwrap();
    let contract =
        LaunchConstraintsV1::new(1, BlockSizeV1::Any, dimensions, 1, 0, 0).unwrap();
    let declaration =
        UntrustedKernelDeclaration::<Kernel>::new(KernelId::from_bytes([0; 32]), contract);
    let _forged: KernelBrand<Kernel> = declaration.into();
}
