//! Original-owner coordinate checks; these are not executed Verus proofs.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticCheckedBinaryOpV1;

#[test]
fn checked_micro_sites_bind_actual_instances_and_absolute_pcs() {
    use super::super::super::paired::aggregate_tests::checked_transform;
    const LIMIT: usize = 256 * 1024 * 1024;
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            checked_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
                false,
                false,
            )
        },
        |plan, out| {
            super::super::tests::with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let mut live = 0;
                let mut nonzero_offset = 0;
                for root in 0..program.roots.len() {
                    let range = &program.roots[root].0;
                    for instance in 0..range.len() {
                        let Some(function) = &program.functions[range.start + instance] else {
                            continue;
                        };
                        let original = plan.instance(root, instance, out)?;
                        assert!(original.active);
                        assert_eq!(function.blocks, original.blocks);
                        for (block, row) in function.control.iter().enumerate() {
                            for statement in 0..row.statements {
                                let pc = function
                                    .checked_micro_pc_v293(root, instance, block, statement, out)?;
                                if matches!(row.end, End::Unreachable) {
                                    assert_eq!(pc, None);
                                } else {
                                    assert_eq!(pc, Some(original.blocks.start + block));
                                    live += 1;
                                    nonzero_offset += usize::from(original.blocks.start != 0);
                                }
                            }
                        }
                        // These calls change only query coordinates, never the admitted owner.
                        for (r, i, b, s) in [
                            (usize::MAX, instance, 0, 0),
                            (root, usize::MAX, 0, 0),
                            (root, instance, function.control.len(), 0),
                            (root, instance, 0, usize::MAX),
                        ] {
                            assert!(matches!(
                                function.checked_micro_pc_v293(r, i, b, s, out),
                                Err(Error::Statement(_))
                            ));
                        }
                    }
                }
                assert!(live > 0 && nonzero_offset > 0);
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}
