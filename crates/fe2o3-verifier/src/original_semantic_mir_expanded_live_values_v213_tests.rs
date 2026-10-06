use super::super::super::source_function::tile_fixture_tests::run_fixture_with_plan;
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1 as Layout;

const LIMIT: usize = 512 * 1024 * 1024;

fn run(layout: Layout, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_plan(layout, work, storage, |plan, slots, _, out| {
        let target = TileTargetV176::derive(slots, out)?;
        let start = out.text.len();
        emit_source_cut_values_v213(plan, slots, &target, FormalIndexWidth::Bits64, out)?;
        let source = plan.source(out)?;
        let archive = source.source_ssa(out.budget)?;
        let mut cuts = 0;
        for root in 0..source.root_count(out.budget)? {
            for instance in 0..plan.root(root, out)?.instances.len() {
                let row = plan.instance(root, instance, out)?;
                if !row.active {
                    continue;
                }
                let ssa = archive.plan_for_function(row.function).unwrap().plan();
                for block in 0..row.blocks.len() {
                    let name = format!(
                        "spec fn invocation_expanded_live_values_{root}_{instance}_{block}_v213("
                    );
                    let expected = usize::from(ssa.is_reachable(Block::new(block as u32)));
                    assert_eq!(out.text[start..].matches(&name).count(), expected);
                    if expected != 0 {
                        assert!(out.text[start..].contains(&format!(
                            "source.machine.pc == {} &&",
                            row.blocks.start + block
                        )));
                    }
                    cuts += expected;
                }
            }
        }
        let emitted = &out.text[start..];
        assert_eq!(
            emitted
                .matches("spec fn invocation_expanded_live_values_")
                .count(),
            cuts
        );
        assert!(cuts > 1);
        assert!(
            emitted.contains("invocation_execution_mapped_v205(source, target, execution_map,")
        );
        assert!(
            emitted.contains(
                "invocation_execution_payload_related_v209(source, target, execution_map,"
            )
        );
        assert!(emitted.contains("invocation_source_aggregate_leaf_v42(source,"));
        assert!(!emitted.contains("proof fn"));
        assert!(!emitted.contains("assume("));
        Ok(())
    })
}

#[test]
fn expanded_live_values_keep_every_original_reachable_cut_distinct() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run(layout, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn expanded_live_values_have_exact_and_one_short_resource_bounds() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run(layout, LIMIT, LIMIT);
        baseline.0.unwrap();
        let exact = run(layout, baseline.1, baseline.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let result = run(layout, work, storage).0;
            assert!(if is_work {
                matches!(result, Err(Error::Resource(Resource::Work(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(result, Err(Error::Resource(Resource::Storage(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

#[test]
fn expanded_live_values_refuse_unknown_width_before_emission() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(layout, LIMIT, LIMIT, |plan, slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let before = out.text.len();
            match emit_source_cut_values_v213(plan, slots, &target, FormalIndexWidth::Unknown, out)
            {
                Err(Error::Statement(
                    "original MIR paired byte relation differs from its exact source cuts",
                )) => (),
                Err(error) => return Err(error),
                Ok(()) => panic!("unknown-width live values were admitted"),
            }
            assert_eq!(out.text.len(), before);
            Ok(())
        })
        .0
        .unwrap();
    }
}
