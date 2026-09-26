use super::*;
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Kernel,
    ProductionRankedOperationV1 as Op, ProductionRankedTerminatorV1 as Terminator,
    ProductionRankedValueIdV1 as Id,
};

#[test]
fn selector_uses_typed_effect_presence_not_a_function_name_or_diagnostic_text() {
    for name in ["vecadd", "conditional", "unrelated"] {
        let kernel = Kernel::new(
            name,
            0,
            vec![Block::new(
                vec![Op::SemanticConstant {
                    result: Id::new(0),
                    value: 1,
                }],
                Terminator::Return,
            )],
        )
        .unwrap();
        budgeted(|b| {
            assert!(matches!(
                select_native_conditional_ownership_site_v2(&kernel, b),
                Err(E(Cause::Invalid("missing bound output")))
            ));
            assert_eq!(b.work(), 2);
            assert_eq!(b.storage(), FLOOR);
        });
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(matches!(
            select_native_conditional_ownership_site_v2(&kernel, &mut budget),
            Err(E(Cause::Resource(Resource::Work(_))))
        ));
        assert!(budget.failed_work().is_some());
    }
    // A positive selected RequireEffectRefinement needs an actual imported
    // receipt identity. Its protected test is not replaced by signing a fixture.
}
