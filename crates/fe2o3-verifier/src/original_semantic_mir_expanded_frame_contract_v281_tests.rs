use super::super::{
    expanded_generation::ExpandedGenerationV221,
    source_function::tile_fixture_tests::{
        run_fixture_with_plan, run_fixture_with_unreachable_root_v281,
    },
};
use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExecutionTileLayoutV1};

const LIMIT: usize = 512 * 1024 * 1024;

#[test]
fn expanded_frame_contracts_keep_complete_microstate_prefix_and_shared_caller_demands() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        run_fixture_with_unreachable_root_v281(layout, LIMIT, LIMIT, |plan, slots, _, out| {
            let model = ExpandedGenerationV221::derive(plan, slots, FormalIndexWidth::Bits64, EndiannessV2::Little, out)?;
            let cuts = TileMicroCutsV180::derive(model.target(out)?, plan, out)?;
            let frames = FramePlan::derive(plan, slots, out)?;
            model.emit_support_with_cuts_v280(&cuts, None, out)?;
            model.emit_frame_contracts_v281(&frames, &cuts, out)?;
            model.finish(out)?;
            assert!(out.text.contains("micro: MemoryMicroStateV30"));
            assert!(out.text.contains("micro.observations == target_prefix"));
            assert!(out.text.contains("invocation_tile_cursor_0_v180(source.machine.pc, micro)"));
            assert!(out.text.contains("invocation_source_byte_state_well_formed_v36(source)"));
            assert!(out.text.contains("invocation_execution_map_current_v205(source, micro.state, execution_map)"));
            assert!(!out.text.contains("proof fn invocation_expanded_frame_contract"));
            let rows = cuts.static_rows_v280(out)?;
            assert_eq!(rows.cuts.len(), frames.cuts.len());
            let mut suspended = 0;
            let mut zero_candidates = 0;
            for frame in &frames.frames {
                let name = format!("spec fn invocation_expanded_ancestors_{}_{}_v281(", frame.root, frame.instance);
                assert_eq!(out.text.matches(&name).count(), 1);
                if let Some(index) = frame.parent_call {
                    let call = &frames.calls[index];
                    if frame.active && !call.demands.is_empty() {
                        suspended += 1;
                        assert!(out.text.contains(&format!("invocation_expanded_carry_{}_{}_{}_v281(source, target, map, execution_map)", call.root, call.caller, call.block)));
                    }
                }
                for cut in &frames.cuts[frame.cuts.clone()] {
                    if rows.cuts[cut.pc].candidates.is_empty() {
                        zero_candidates += 1;
                        assert_eq!((cut.root, cut.instance), (0, 0));
                        assert_eq!(cut.pc + 1, plan.instance(0, 0, out)?.blocks.end);
                        assert!(!cut.reachable && cut.demands.is_empty());
                    }
                    assert_eq!(out.text.matches(&format!("spec fn invocation_expanded_current_{}_{}_{}_v281(", cut.root, cut.instance, cut.block)).count(), 1);
                    assert_eq!(out.text.matches(&format!(" {} => source.machine.frames.active.len() ==", cut.pc)).count(), 1);
                }
            }
            assert!(suspended > 0);
            assert_eq!(zero_candidates, 1);
            Ok(())
        }).0.unwrap();
    }
}

#[test]
fn expanded_frame_contract_refuses_foreign_equal_source_target_owner() {
    run_fixture_with_plan(
        ExecutionTileLayoutV1::Blocked,
        LIMIT,
        LIMIT,
        |plan, slots, _, out| {
            let first = ExpandedGenerationV221::derive(
                plan,
                slots,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            let second = ExpandedGenerationV221::derive(
                plan,
                slots,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            let cuts = TileMicroCutsV180::derive(first.target(out)?, plan, out)?;
            let frames = FramePlan::derive(plan, slots, out)?;
            assert!(matches!(
                second.emit_frame_contracts_v281(&frames, &cuts, out),
                Err(Error::Statement(
                    "expanded micro-cut differs from its original source or target owner"
                ))
            ));
            Ok(())
        },
    )
    .0
    .unwrap();
}
