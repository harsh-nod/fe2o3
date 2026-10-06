use super::super::source_function::tile_fixture_tests::run_fixture_with_plan;
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1 as Layout;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const LIMIT: usize = 512 * 1024 * 1024;

#[test]
fn expanded_generation_headers_cover_context_and_runtime_queries() {
    let retained = size_of::<ExpandedGenerationV221<'_, '_, '_, '_>>();
    let results = 2 * size_of::<Result<ExpandedGenerationV221<'_, '_, '_, '_>>>()
        + 2 * size_of::<Result<()>>();
    let runtime =
        2 * size_of::<FormalIndexWidth>() + size_of::<EndiannessV2>() + size_of::<[u64; 3]>();
    let selected_launch = size_of::<Option<usize>>();
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
                run(layout, width, endianness, LIMIT, LIMIT).0.unwrap();
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
