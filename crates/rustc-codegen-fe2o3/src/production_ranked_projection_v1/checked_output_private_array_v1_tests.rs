use super::*;
use fe2o3_mir_model::SsaBlockIdV1;
use fe2o3_pliron::{
    ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Role,
};

fn array_source(index: u32, unreachable: bool) -> SemanticFunctionDeclV1 {
    let mut statements = assertion_ranked_write_statements(1, 2);
    statements[0] = typed_assignment(
        2,
        A_U32,
        SemanticRvalueKindV1::Use(typed_constant(A_U32, u128::from(index), 4)),
    );
    let blocks = if unreachable {
        vec![
            block(
                201,
                vec![],
                assertion_terminator(typed_constant(A_BOOL, 0, 1), true, 1),
            ),
            block(202, statements, SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                201,
                statements,
                assertion_terminator(typed_constant(A_BOOL, 1, 1), true, 1),
            ),
            block(202, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    // The existing ABI/layout factory and materializer perform fresh source,
    // SSA and executable admission. No retained row is installed by this test.
    assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_ARRAY, SemanticLocalRoleV1::Temporary),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![],
        blocks,
        false,
    )
}

fn operation_count(owner: &Owner) -> usize {
    owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}

#[test]
fn real_target_binding_and_changed_output_supply_the_exact_private_index() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for index in [0, 7] {
            let source = assertion_materialized(array_source(index, false));
            let source_bytes = source.executable().canonical().canonical_bytes().to_vec();
            with_actual(&source, profile, |bound, checked, budget| {
                assert_ne!(
                    bound.canonical().identity(),
                    checked.owner().canonical().identity()
                );
                assert!(operation_count(checked.owner()) < operation_count(bound));
                with_checked_output_assertions_budget_v1(
                    &source,
                    bound,
                    checked,
                    profile,
                    budget,
                    |session| {
                        let mut facts = session.for_source(ROOT, ROOT);
                        assert!(facts.is_materialized_block(0)?);
                        let site = Site::Statement {
                            block: SsaBlockIdV1::new(0),
                            statement: 1,
                        };
                        assert!(facts.private_array_access(site, Role::Destination)?);
                        assert_eq!(
                            facts.private_array_constant_index(site, Role::Destination)?,
                            Some(u64::from(index))
                        );
                        Ok(())
                    },
                )
                .unwrap();
            });
            assert_eq!(
                source.executable().canonical().canonical_bytes(),
                source_bytes
            );
        }
    }
}

#[test]
fn checked_unreachable_write_cannot_become_a_live_index_hint() {
    let source = assertion_materialized(array_source(0, true));
    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
        assert_ne!(
            bound.canonical().identity(),
            checked.owner().canonical().identity()
        );
        assert!(
            !checked
                .owner()
                .module()
                .functions
                .iter()
                .filter_map(|function| function.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    fe2o3_kernel_ir::OperationKind::Store { .. }
                ))
        );
        with_checked_output_assertions_budget_v1(
            &source,
            bound,
            checked,
            Profile::Gfx942,
            budget,
            |session| {
                let mut facts = session.for_source(ROOT, ROOT);
                assert!(!facts.is_materialized_block(1)?);
                let site = Site::Statement {
                    block: SsaBlockIdV1::new(1),
                    statement: 1,
                };
                assert!(matches!(
                    facts.private_array_access(site, Role::Destination),
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "checked output private-array write is not executable"
                    ))
                ));
                assert!(matches!(
                    facts.private_array_constant_index(site, Role::Destination),
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "checked output private-array write is not executable"
                    ))
                ));
                Ok(())
            },
        )
        .unwrap();
    });
}
