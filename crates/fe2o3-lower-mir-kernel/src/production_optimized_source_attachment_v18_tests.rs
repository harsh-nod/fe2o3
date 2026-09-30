include!("production_scoped_inline_callee_v30_tests.rs");

#[test]
fn complete_source_site_census_survives_real_repeated_helper_optimization() {
    for factory in [
        scalar_payload_owner_v18 as fn() -> _,
        active_gap_owner_v18,
        active_shared_target_owner_v18,
        repeated_inline_owner_v18,
    ] {
        run_optimized_source_v18(factory, |view, budget| {
            let source = view.original_source(budget)?;
            let mut sites = 0usize;
            let mut operations = 0usize;
            for root in 0..source.root_count(budget)? {
                for row in &source.root_row(root)?.coordinates.spans.rows {
                    let (block, statement) = match row.source {
                        InstanceSpanSourceV1::Statement(site) => {
                            (site.semantic_block, Some(site.statement_ordinal))
                        }
                        InstanceSpanSourceV1::Terminator(site) => (site.semantic_block, None),
                        InstanceSpanSourceV1::Synthetic(_)
                        | InstanceSpanSourceV1::InvocationEntry(_) => continue,
                    };
                    let mut emitted = 0;
                    view.visit_source_operations(
                        root,
                        row.instance.index(),
                        block,
                        statement,
                        budget,
                        |span, budget| {
                            emitted += 1;
                            match span {
                                ProductionOptimizedSourceSpanV18::Operation(
                                    ProductionOptimizedSourceOperationV18::Retained {
                                        output, ..
                                    },
                                ) => {
                                    let inventory = view.output_inventory(budget)?;
                                    let function =
                                        &inventory.functions()[output.block.function.0 as usize];
                                    let block = &inventory.blocks()
                                        [function.blocks.start + output.block.block as usize];
                                    assert_eq!(
                                        inventory.operations()
                                            [block.operations.start + output.operation as usize]
                                            .coordinate,
                                        output
                                    );
                                    operations += 1;
                                }
                                ProductionOptimizedSourceSpanV18::Gap(
                                    ProductionOptimizedSourceGapV18::Reachable(gap),
                                ) => assert!(gap.first <= gap.last),
                                ProductionOptimizedSourceSpanV18::Operation(
                                    ProductionOptimizedSourceOperationV18::Rewritten { .. }
                                    | ProductionOptimizedSourceOperationV18::RemovedUnreachable {
                                        ..
                                    },
                                )
                                | ProductionOptimizedSourceSpanV18::Gap(
                                    ProductionOptimizedSourceGapV18::Unreachable { .. },
                                )
                                | ProductionOptimizedSourceSpanV18::OriginalRemovedCall
                                | ProductionOptimizedSourceSpanV18::OriginalNoOperations => {}
                            }
                            Ok(())
                        },
                    )?;
                    assert!(emitted > 0, "empty sites retain an explicit disposition");
                    sites += 1;
                }
            }
            assert!(sites > 0 && operations > 0);
            Ok(())
        });
    }
}

#[test]
fn every_input_definition_exposes_its_complete_checked_descendant_range() {
    run_optimized_source_v18(folding_source_owner_v18, |view, budget| {
        for definition in view.input_inventory(budget)?.definitions() {
            let rows = view.definition_descendants(definition.coordinate, budget)?;
            for row in rows {
                assert!(
                    view.output_inventory(budget)?
                        .definitions()
                        .iter()
                        .any(|candidate| candidate.coordinate == row.output)
                );
            }
        }
        Ok(())
    });
}
