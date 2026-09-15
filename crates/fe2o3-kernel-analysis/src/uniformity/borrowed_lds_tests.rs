use super::*;
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/borrowed_lds/fixture.rs"
    ));
}

#[test]
fn borrowed_lds_uniformity_retains_variation_of_exact_owner() {
    let module = fixture::module(true, true);
    for input in [
        Variation::GridUniform,
        Variation::WorkgroupUniform,
        Variation::SubgroupUniform,
        Variation::Varying,
    ] {
        let allocated =
            execution_capability_variation(&fixture::contract_at(&module, 2).operation, |index| {
                assert_eq!(index, 0);
                input
            });
        assert_eq!(allocated, Variation::WorkgroupUniform.join(input));
        assert_eq!(
            execution_capability_variation(&fixture::contract_at(&module, 3).operation, |_| {
                allocated
            }),
            allocated
        );
    }
}
