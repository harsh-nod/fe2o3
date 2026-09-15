#[test]
fn derived_read_guard_unknown_index_is_not_reconstructed_from_cpu_coordinate() {
    let (kernel, bindings, write, reserved) = compact_row_join_fixture(true);
    let mut blocks = kernel.blocks().to_vec();
    let ProductionRankedTerminatorV1::IndexLessThan {
        rhs,
        true_block,
        false_block,
        ..
    } = blocks[4].terminator()
    else {
        panic!()
    };
    blocks[4] = ProductionRankedBlockV1::new(
        blocks[4].operations().to_vec(),
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs: ProductionRankedValueV1::Argument(2),
            rhs: *rhs,
            true_block: *true_block,
            false_block: *false_block,
        },
    );
    let kernel = ProductionRankedKernelV1::new("opaque_read_guard", 3, blocks).unwrap();
    assert!(matches!(
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            block: 6,
            operation: 1,
            detail: "GPU guard operand has no representable ranked definition",
        })
    ));
}
