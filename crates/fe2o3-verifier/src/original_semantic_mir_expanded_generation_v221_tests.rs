use super::super::source_function::tile_fixture_tests::run_fixture_with_plan;
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1 as Layout;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const LIMIT: usize = 512 * 1024 * 1024;

fn support_only(
    layout: Layout,
    width: FormalIndexWidth,
    endianness: EndiannessV2,
    work: usize,
    storage: usize,
    inspect: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_plan(layout, work, storage, |plan, slots, _, out| {
        let model = ExpandedGenerationV221::derive(plan, slots, width, endianness, out)?;
        let start = out.text.len();
        model.emit_support(out)?;
        model.finish(out)?;
        let generated = &out.text[start..];
        for marker in [
            "spec fn invocation_source_byte_block_0_v36(",
            "spec fn byte_micro_step_0_v30(",
            "spec fn invocation_tile_micro_seek_0_v181(",
        ] {
            assert_eq!(generated.matches(marker).count(), 1, "{marker}");
        }
        assert!(generated.contains("proof fn invocation_context_issue_segment_"));
        assert_eq!(
            generated
                .matches("proof fn invocation_context_issue_initial_map_segment_")
                .count(),
            generated
                .matches("proof fn invocation_context_issue_segment_")
                .count()
        );
        assert_eq!(
            generated
                .matches("proof fn invocation_context_issue_current_map_segment_")
                .count(),
            generated
                .matches("proof fn invocation_context_issue_segment_")
                .count()
        );
        for segment in generated
            .split("proof fn invocation_context_issue_current_map_segment_")
            .skip(1)
        {
            let segment = segment.split("\nproof fn ").next().unwrap();
            assert_eq!(
                segment
                    .matches("invocation_context_issue_fresh_preserves_frame_v259(")
                    .count(),
                1
            );
            assert!(
                !segment.contains("invocation_context_issue_fresh_preserves_current_map_v238(")
            );
            assert_eq!(
                segment
                    .matches("invocation_context_issue_fresh_has_exact_updates_v211(")
                    .count(),
                1
            );
            assert_eq!(
                segment.matches("invocation_context_issue_segment_").count(),
                1
            );
            assert!(segment.contains(
                "invocation_context_issue_fresh_enabled_v211(source.source, target.state,"
            ));
            assert!(segment.contains(
                "invocation_execution_map_current_v205(source.source, target.state, execution_map)"
            ));
            assert!(segment.contains("assert(coupled.source.source.machine == (MemoryStateV30 {"));
            assert!(segment.contains("..source.source.machine }));"));
            assert!(segment.contains("let continued = invocation_source_byte_pc_v36("));
            assert_eq!(
                segment
                    .matches("invocation_execution_map_source_pc_v303(")
                    .count(),
                1
            );
            assert!(
                segment.contains("coupled.source.source, coupled.target, coupled.execution_map,")
            );
            assert!(segment.contains("assert(invocation_execution_map_current_v205(continued, coupled.target, coupled.execution_map));"));
            assert!(!segment.contains("assert forall|key: MemoryExecutionReferenceV178|"));
            assert!(!segment.contains("Map::empty()"));
        }
        assert!(!generated.contains("spec fn invocation_expanded_live_values_"));
        assert!(!generated.contains("spec fn expanded_source_component_"));
        assert!(!generated.contains("proof fn invocation_paired_"));
        assert!(!generated.contains("assume("));
        let bytes = match width {
            FormalIndexWidth::Bits32 => 4,
            FormalIndexWidth::Bits64 => 8,
            FormalIndexWidth::Unknown => panic!("support fixture needs a known index width"),
        };
        assert!(generated.contains(&format!(
            "spec fn invocation_runtime_index_bytes_v36() -> int {{ {bytes} }}"
        )));
        assert!(generated.contains(&format!(
            "spec fn invocation_runtime_little_endian_v36() -> bool {{ {} }}",
            endianness == EndiannessV2::Little
        )));
        inspect(generated);
        Ok(())
    })
}

#[test]
fn expanded_generation_support_checks_real_steps_and_runtime_without_live_value_claims() {
    for layout in [Layout::Blocked, Layout::Striped] {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for endianness in [EndiannessV2::Little, EndiannessV2::Big] {
                let result = support_only(layout, width, endianness, LIMIT, LIMIT, |_| {}).0;
                // This authentic fixture's target casts require a 64-bit INDEX.
                if width == FormalIndexWidth::Bits32 {
                    assert!(matches!(
                        result,
                        Err(Error::Statement("actual integral byte cast is not modeled"))
                    ));
                } else {
                    result.unwrap();
                }
            }
        }
    }
}

#[test]
fn expanded_generation_support_has_exact_and_one_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            support_only(
                layout,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                work,
                storage,
                |_| {},
            )
        };
        let baseline = run(LIMIT, LIMIT);
        baseline.0.unwrap();
        let exact = run(baseline.1, baseline.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        assert!(matches!(run(baseline.1 - 1, baseline.3).0,
            Err(Error::Resource(Resource::Work(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
        assert!(matches!(run(baseline.1, baseline.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
    }
}

#[test]
#[ignore = "complete actual ContextIssue step models; no whole paired refinement authority"]
fn diagnostic_complete_context_issue_segment_models_export_v224() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    for (layout, label) in [(Layout::Blocked, "blocked"), (Layout::Striped, "striped")] {
        support_only(layout, FormalIndexWidth::Bits64, EndiannessV2::Little, LIMIT, LIMIT, |generated| {
            assert!(generated.len() <= 16 * 1024 * 1024);
            let mut output = BufWriter::new(std::io::stdout().lock());
            write!(output, "{{\"kind\":\"fe2o3-context-issue-segment-model-v224\",\"scope\":\"actual ContextIssue endpoint replay only\",\"layout\":\"{label}\",\"bytes\":{},\"sha256\":\"", generated.len()).unwrap();
            for byte in Sha256::digest(generated.as_bytes()) { write!(output, "{byte:02x}").unwrap(); }
            write!(output, "\",\"model_hex\":\"").unwrap();
            for byte in generated.as_bytes() { write!(output, "{byte:02x}").unwrap(); }
            writeln!(output, "\"}}").unwrap();
            output.flush().unwrap();
        }).0.unwrap();
    }
}

#[test]
fn expanded_generation_headers_cover_context_and_runtime_queries() {
    let retained = size_of::<ExpandedGenerationV221<'_, '_, '_, '_>>();
    let results = 2 * size_of::<Result<ExpandedGenerationV221<'_, '_, '_, '_>>>()
        + 2 * size_of::<Result<()>>();
    let runtime =
        2 * size_of::<FormalIndexWidth>() + size_of::<EndiannessV2>() + size_of::<[u64; 3]>();
    let selected_launch = size_of::<
        Option<&[super::super::expanded_model_v280::ExpandedSupportRuntimeV280]>,
    >() + size_of::<Result<(u8, [u64; 3])>>()
        + size_of::<u8>();
    let callback_frames = 2 * size_of::<(
        &ExpandedGenerationV221<'_, '_, '_, '_>,
        &TileMicroCutsV180<'_, '_, '_, '_>,
        Option<&[super::super::expanded_model_v280::ExpandedSupportRuntimeV280]>,
    )>();
    let launch_iterator = size_of::<
        std::iter::Enumerate<
            std::slice::Iter<'_, fe2o3_lower_mir_kernel::ProductionSourceLaunchRootV1>,
        >,
    >();
    let coordinates = [
        ("root, count and launch index", 3),
        ("query temporaries", 9),
    ];
    let references = [
        ("context inputs and owner queries", 8),
        ("support and launch queries", 8),
    ];
    assert_eq!(
        ExpandedGenerationV221::headers(),
        retained
            + results
            + runtime
            + selected_launch
            + callback_frames
            + launch_iterator
            + coordinates.iter().map(|(_, count)| count).sum::<usize>() * size_of::<usize>()
            + references.iter().map(|(_, count)| count).sum::<usize>() * size_of::<&()>()
    );
}

fn run(
    layout: Layout,
    width: FormalIndexWidth,
    endianness: EndiannessV2,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_plan(layout, work, storage, |plan, slots, _, out| {
        let model = ExpandedGenerationV221::derive(plan, slots, width, endianness, out)?;
        assert!(std::ptr::eq(model.target(out)?.source_slots(out)?, slots));
        let original = slots.correspondence(out)?.inventory(out.budget)?;
        assert!(!std::ptr::eq(
            original.owner(),
            model.target(out)?.inventory(out)?.owner()
        ));
        let start = out.text.len();
        model.emit_support(out)?;
        model.emit_live_values(out)?;
        model.finish(out)?;
        let generated = &out.text[start..];
        for declaration in [
            "spec fn invocation_source_byte_block_0_v36(",
            "spec fn byte_micro_step_0_v30(",
            "spec fn invocation_tile_cursor_0_v180(",
            "spec fn invocation_tile_micro_seek_0_v181(",
            "spec fn invocation_expanded_live_values_0_0_0_v213(",
            "spec fn invocation_runtime_launch_0_v36(",
        ] {
            assert_eq!(generated.matches(declaration).count(), 1, "{declaration}");
        }
        assert!(generated.contains(&format!(
            "spec fn invocation_runtime_little_endian_v36() -> bool {{ {} }}",
            endianness == EndiannessV2::Little
        )));
        let index_bytes = match width {
            FormalIndexWidth::Bits32 => 4,
            FormalIndexWidth::Bits64 => 8,
            FormalIndexWidth::Unknown => panic!("positive fixture needs a concrete width"),
        };
        assert!(generated.contains(&format!(
            "spec fn invocation_runtime_index_bytes_v36() -> int {{ {index_bytes} }}"
        )));
        let (selected, absent) = match layout {
            Layout::Blocked => ("Blocked", "Striped"),
            Layout::Striped => ("Striped", "Blocked"),
        };
        assert_eq!(
            generated
                .matches(&format!(
                    "layout: InvocationSourceTileLayoutV161::{selected}"
                ))
                .count(),
            2
        );
        assert!(!generated.contains(&format!("layout: InvocationSourceTileLayoutV161::{absent}")));
        assert!(!generated.contains("proof fn invocation_paired_"));
        assert!(generated.contains("proof fn invocation_context_issue_segment_"));
        assert!(!generated.contains("assume("));
        Ok(())
    })
}

#[test]
fn expanded_generation_uses_actual_owners_layout_and_runtime() {
    for layout in [Layout::Blocked, Layout::Striped] {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for endianness in [EndiannessV2::Little, EndiannessV2::Big] {
                let result = run(layout, width, endianness, LIMIT, LIMIT).0;
                if width == FormalIndexWidth::Bits32 {
                    assert!(matches!(
                        result,
                        Err(Error::Statement("actual integral byte cast is not modeled"))
                    ));
                } else {
                    result.unwrap();
                }
            }
        }
    }
}

#[test]
fn expanded_generation_has_exact_and_one_short_resource_bounds() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run(
            layout,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            LIMIT,
            LIMIT,
        );
        baseline.0.unwrap();
        let exact = run(
            layout,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            baseline.1,
            baseline.3,
        );
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        assert!(
            matches!(run(layout, FormalIndexWidth::Bits64, EndiannessV2::Little, baseline.1 - 1, baseline.3).0,
            Err(Error::Resource(Resource::Work(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == baseline.1 && error.limit() == baseline.1 - 1)
        );
        assert!(
            matches!(run(layout, FormalIndexWidth::Bits64, EndiannessV2::Little, baseline.1, baseline.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == baseline.3 && error.limit() == baseline.3 - 1)
        );
    }
}

#[test]
fn expanded_generation_refuses_unknown_width_before_output() {
    run_fixture_with_plan(Layout::Blocked, LIMIT, LIMIT, |plan, slots, _, out| {
        let before = out.text.len();
        match ExpandedGenerationV221::derive(
            plan,
            slots,
            FormalIndexWidth::Unknown,
            EndiannessV2::Little,
            out,
        ) {
            Err(Error::Statement(
                "expanded generation differs from its retained source or runtime",
            )) => (),
            Err(error) => return Err(error),
            Ok(_) => panic!("unknown index width was admitted"),
        }
        assert_eq!(out.text.len(), before);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn expanded_generation_keeps_original_budget_and_storage_floor() {
    for refund in [false, true] {
        let mut reached = false;
        let result = run_fixture_with_plan(Layout::Blocked, LIMIT, LIMIT, |plan, slots, _, out| {
            let model = ExpandedGenerationV221::derive(
                plan,
                slots,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            if refund {
                out.budget.release_storage(1)?;
                reached = true;
                model.target(out).map(|_| ())
            } else {
                run_fixture_with_plan(Layout::Striped, LIMIT, LIMIT, |_, _, _, other_out| {
                    let before = other_out.text.len();
                    let error = model
                        .target(other_out)
                        .err()
                        .expect("foreign budget must refuse");
                    assert_eq!(other_out.text.len(), before);
                    reached = true;
                    Err(error)
                })
                .0
            }
        })
        .0;
        assert!(reached);
        assert!(matches!(
            result,
            Err(Error::Resource(Resource::Accounting))
                | Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}
