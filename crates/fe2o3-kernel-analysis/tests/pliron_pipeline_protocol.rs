use dialect_kernel::{DIALECT_NAME, register_dialect};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironPipelineProtocolFindingV1, run_pliron_barrier_convergence_check_v1,
    run_pliron_pipeline_protocol_check_v1,
};
use pliron::{
    builtin::ops::FuncOp,
    context::Context,
    dialect::DialectName,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
};

fn parse_fixture(source: &str) -> (Context, FuncOp) {
    let ir = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid dialect name"),
    )
    .expect("kernel dialect");
    dialect_gpu::register_dialect(&mut context).expect("gpu dialect");
    dialect_proof::register_dialect(&mut context).expect("proof dialect");
    let operation =
        parse_from_str(Operation::top_level_parser(), &mut context, &ir).expect("fixture parses");
    verify_operation(operation, &context).expect("fixture verifies locally");
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    (context, FuncOp::from_operation(operation))
}

fn dynamic_pipeline_with_wait_barriers(extra_barrier: bool) -> String {
    let source = include_str!("lit/pipeline_dynamic_double_buffer.pliron");
    let layout = "    gpu.execution_layout () [] [gpu_execution_grid_identity: gpu.grid_identity 7, gpu_execution_global_x: gpu.execution_extent 0, gpu_execution_global_y: gpu.execution_extent 1, gpu_execution_global_z: gpu.execution_extent 1, gpu_execution_workgroup_x: gpu.execution_extent 64, gpu_execution_workgroup_y: gpu.execution_extent 1, gpu_execution_workgroup_z: gpu.execution_extent 1, gpu_execution_subgroup_size: gpu.subgroup_size 64, gpu_execution_domain: gpu.execution_domain FullPhysicalWorkgroups]: <() -> ()>;";
    let barrier = "    gpu.barrier Workgroup Workgroup Workgroup AcquireRelease;";
    let mut output = Vec::new();
    for line in source.lines() {
        output.push(line.to_owned());
        if line.contains("^entry_block1v1(bound_v5: kernel.index ):") {
            output.push(layout.to_owned());
        }
        if line.contains("kernel_pipeline_event_kind: kernel.pipeline_event_kind Wait") {
            let event = output.pop().expect("wait line was just appended");
            output.push(barrier.to_owned());
            output.push(event);
        }
        if extra_barrier && line.contains("^exit_block4v1():") {
            output.push(barrier.to_owned());
        }
    }
    output.join("\n")
}

#[test]
fn proves_wait_barriers_in_uniform_dynamic_epoch_loops() {
    let source = dynamic_pipeline_with_wait_barriers(false);
    let (context, function) = parse_fixture(&source);
    let report = run_pliron_barrier_convergence_check_v1(&context, &function);
    assert!(report.is_clean(), "{:?}", report.findings());

    let source = dynamic_pipeline_with_wait_barriers(true);
    let (context, function) = parse_fixture(&source);
    let report = run_pliron_barrier_convergence_check_v1(&context, &function);
    assert!(!report.is_clean());
}

#[test]
fn proves_symbolic_double_and_triple_buffer_windows_without_unrolling() {
    for (source, buffers, distance) in [
        (
            include_str!("lit/pipeline_dynamic_double_buffer.pliron"),
            2,
            1,
        ),
        (
            include_str!("lit/pipeline_dynamic_triple_buffer.pliron"),
            3,
            2,
        ),
        (include_str!("lit/pipeline_dynamic_multiblock.pliron"), 2, 1),
        (include_str!("lit/pipeline_dynamic_regions.pliron"), 2, 1),
    ] {
        let (context, function) = parse_fixture(source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Clean);
        let [certificate] = report.certificates() else {
            panic!("expected one pipeline certificate");
        };
        assert_eq!(certificate.buffers(), buffers);
        assert_eq!(certificate.prefetch_distance(), distance);
        let summary = certificate.dynamic_loop().expect("dynamic loop summary");
        assert_eq!(summary.prologue(), 0);
        assert_eq!(summary.step(), 1);
        assert_eq!(summary.prefetched_epochs(), distance);
        assert_eq!(summary.live_epoch_window(), distance + 1);
        assert_eq!(summary.drained_epochs(), distance);
        assert!(summary.live_epoch_window() <= buffers);
        assert!(!summary.body().is_empty());
        assert!(!summary.prologue_blocks().is_empty());
        assert!(!summary.drain_blocks().is_empty());
    }
}

#[test]
fn dynamic_epoch_loop_allows_unrelated_carried_block_arguments() {
    let source = include_str!("lit/pipeline_dynamic_double_buffer.pliron")
        .replace(
            "kernel.br_args (zero_v2) [^header_block2v1] []: <(kernel.index ) -> ()>",
            "kernel.br_args (zero_v2, one_v3) [^header_block2v1] []: <(kernel.index , kernel.index ) -> ()>",
        )
        .replace(
            "^header_block2v1(i_v6: kernel.index ):",
            "^header_block2v1(i_v6: kernel.index , carried_v12: kernel.index ):",
        )
        .replace(
            "kernel.index_lt_br_args (i_v6, bound_v5, i_v6) [^body_block3v1, ^exit_block4v1] []: <(kernel.index , kernel.index , kernel.index ) -> ()>",
            "kernel.index_lt_br_args (i_v6, bound_v5, i_v6, carried_v12) [^body_block3v1, ^exit_block4v1] []: <(kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>",
        )
        .replace(
            "^body_block3v1(body_i_v7: kernel.index ):",
            "^body_block3v1(body_i_v7: kernel.index , body_carried_v13: kernel.index ):",
        )
        .replace(
            "kernel.br_args (future_v8) [^header_block2v1] []: <(kernel.index ) -> ()>",
            "kernel.br_args (future_v8, body_carried_v13) [^header_block2v1] []: <(kernel.index , kernel.index ) -> ()>",
        );
    let (context, function) = parse_fixture(&source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(report.is_clean(), "{:?}", report.findings());
}

#[test]
fn proves_symbolic_pipeline_access_windows_and_slots() {
    let (context, function) =
        parse_fixture(include_str!("lit/pipeline_dynamic_double_buffer.pliron"));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    let [certificate] = report.certificates() else {
        panic!("expected one pipeline certificate");
    };
    assert_eq!(certificate.staged_writes(), 2);
    assert_eq!(certificate.consuming_reads(), 1);
    assert!(certificate.access_refinement_proven());
}

#[test]
fn rejects_mismatched_symbolic_stage_and_consume_coordinates() {
    let source = include_str!("lit/pipeline_dynamic_double_buffer.pliron").replace(
        "kernel.access (v0, current_slot_v10, zero_v2)",
        "kernel.access (v0, current_slot_v10, one_v3)",
    );
    let (context, function) = parse_fixture(&source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
}

#[test]
fn accepts_interleaved_independent_pipeline_lifecycles() {
    let (context, function) =
        parse_fixture(include_str!("lit/pipeline_independent_storage.pliron"));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert_eq!(report.certificates().len(), 2);
    assert!(report.certificates().windows(2).all(|pair| {
        (pair[0].pipeline_block(), pair[0].pipeline_operation())
            < (pair[1].pipeline_block(), pair[1].pipeline_operation())
    }));
}

#[test]
fn rejects_order_reuse_slot_epoch_and_drain_failures() {
    for (case, source) in [
        include_str!("lit/pipeline_wrong_slot.pliron"),
        include_str!("lit/pipeline_wait_before_commit.pliron"),
        include_str!("lit/pipeline_overwrite_before_release.pliron"),
        include_str!("lit/pipeline_missing_release.pliron"),
        include_str!("lit/pipeline_dynamic_missing_drain.pliron"),
        include_str!("lit/pipeline_dynamic_wrong_future_epoch.pliron"),
        include_str!("lit/pipeline_dynamic_nonuniform_bound.pliron"),
        include_str!("lit/pipeline_dynamic_bypass_drain.pliron"),
        include_str!("lit/pipeline_duplicate_commit.pliron"),
        include_str!("lit/pipeline_consume_before_wait.pliron"),
        include_str!("lit/pipeline_release_before_consume.pliron"),
        include_str!("lit/pipeline_conditional_nonentry.pliron"),
        include_str!("lit/pipeline_write_after_commit.pliron"),
        include_str!("lit/pipeline_read_before_consume.pliron"),
        include_str!("lit/pipeline_dynamic_wrong_access_slot.pliron"),
    ]
    .into_iter()
    .enumerate()
    {
        let (context, function) = parse_fixture(source);
        let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Rejected,
            "case {case}"
        );
        assert!(matches!(
            report.findings().first(),
            Some(PlironPipelineProtocolFindingV1::InvalidSchedule { .. })
        ));
    }
}

#[test]
fn rejects_distinct_views_in_the_same_noalias_storage_class() {
    let (context, function) = parse_fixture(include_str!("lit/pipeline_aliased_storage.pliron"));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings().first(),
        Some(PlironPipelineProtocolFindingV1::AliasedStorage { .. })
    ));
    assert_eq!(
        report
            .findings()
            .iter()
            .filter(|finding| matches!(
                finding,
                PlironPipelineProtocolFindingV1::AliasedStorage { .. }
            ))
            .count(),
        2,
    );
}

#[test]
fn rejects_multiple_pipeline_storages_without_disjoint_provenance() {
    let (context, function) =
        parse_fixture(include_str!("lit/pipeline_unknown_storage_alias.pliron"));
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings().first(),
        Some(PlironPipelineProtocolFindingV1::AliasedStorage { .. })
    ));
}
