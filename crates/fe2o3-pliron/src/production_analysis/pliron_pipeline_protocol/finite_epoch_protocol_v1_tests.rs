use super::*;
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::{dialect::DialectName, op::Op, operation::verify_operation, parsable::parse_from_str};

fn parse(source: &str) -> (Context, FuncOp) {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, source).unwrap();
    verify_operation(operation, &context).unwrap();
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    (context, FuncOp::from_operation(operation))
}

fn event(epoch: &str, slot: &str, kind: &str) -> String {
    format!(
        "kernel.pipeline_event (pipeline, {epoch}, {slot}) [] [kernel_pipeline_event_kind: kernel.pipeline_event_kind {kind}]: <(kernel.pipeline <2,1>, kernel.index , kernel.index ) -> ()>;\n"
    )
}

fn access(slot: &str, coordinate: &str, kind: &str) -> String {
    format!(
        "kernel.access (tile, {slot}, {coordinate}) [] [kernel_access_kind: kernel.access_kind {kind}]: <(kernel.ranked_view <32,true,[2,128]>, kernel.index , kernel.index ) -> ()>;\n"
    )
}

fn binary(result: &str, left: &str, right: &str, kind: &str) -> String {
    format!(
        "{result} = kernel.index_binary ({left}, {right}) [] [kernel_index_binary_kind: kernel.index_binary_kind {kind}]: <(kernel.index , kernel.index ) -> (kernel.index )>;\n"
    )
}

fn constant(name: &str, value: u32) -> String {
    format!(
        "{name} = kernel.index_constant () [] [kernel_index_value: kernel.index_value {value}]: <() -> (kernel.index )>;\n"
    )
}

fn start_phase(source: &mut String, epoch: &str, slot: &str) {
    source.push_str(&event(epoch, slot, "Stage"));
    source.push_str(&access(slot, "lane", "Write"));
    for kind in ["Commit", "Wait", "Consume"] {
        source.push_str(&event(epoch, slot, kind));
    }
}

/// This is verifier test IR, never a substitute for Rust source extraction.
/// P1 closes two epochs per outer iteration; P2 adds a finite arithmetic loop;
/// P3 retains every LDS read in an all-lane broadcast/reduction pattern.
fn fixture(scan: bool, reduction: bool) -> String {
    let mut source = String::from(
        "builtin.func @finite_epoch: builtin.function <() -> ()> {\n\
         ^entry():\n\
         gpu.execution_layout () [] [gpu_execution_grid_identity: gpu.grid_identity 7, gpu_execution_global_x: gpu.execution_extent 256, gpu_execution_global_y: gpu.execution_extent 1, gpu_execution_global_z: gpu.execution_extent 1, gpu_execution_workgroup_x: gpu.execution_extent 128, gpu_execution_workgroup_y: gpu.execution_extent 1, gpu_execution_workgroup_z: gpu.execution_extent 1, gpu_execution_subgroup_size: gpu.subgroup_size 64, gpu_execution_domain: gpu.execution_domain FullPhysicalWorkgroups]: <() -> ()>;\n\
         tile = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Workgroup]: <() -> (kernel.ranked_view <32,true,[2,128]>)>;\n\
         pipeline = kernel.pipeline_create (tile) [] []: <(kernel.ranked_view <32,true,[2,128]>) -> (kernel.pipeline <2,1>)>;\n\
         global = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 256]: <() -> (kernel.index )>;\n",
    );
    for (name, value) in [
        ("zero", 0),
        ("one", 1),
        ("two", 2),
        ("seven", 7),
        ("sixteen", 16),
        ("lanes", 128),
    ] {
        source.push_str(&constant(name, value));
    }
    source.push_str(&binary("lane", "global", "lanes", "Remainder"));
    source.push_str("kernel.br_args (zero) [^outer] []: <(kernel.index ) -> ()>\n\
        ^outer(outer_i: kernel.index ):\n\
        kernel.index_lt_br_args (outer_i, sixteen, outer_i) [^body, ^exit] []: <(kernel.index , kernel.index , kernel.index ) -> ()>\n\
        ^body(body_i: kernel.index ):\n");
    if scan {
        source.push_str("kernel.br_args (body_i, zero) [^scan_header] []: <(kernel.index , kernel.index ) -> ()>\n\
            ^scan_header(scan_outer: kernel.index , scan_i: kernel.index ):\n\
            kernel.index_lt_br_args (scan_i, seven, scan_outer, scan_i, scan_outer) [^scan_body, ^phase0] []: <(kernel.index , kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>\n\
            ^scan_body(scan_body_outer: kernel.index , scan_body_i: kernel.index ):\n");
        source.push_str(&binary("next_scan", "scan_body_i", "one", "Add"));
        source.push_str("kernel.br_args (scan_body_outer, next_scan) [^scan_header] []: <(kernel.index , kernel.index ) -> ()>\n");
    } else {
        source.push_str("kernel.br_args (body_i) [^phase0] []: <(kernel.index ) -> ()>\n");
    }
    source.push_str("^phase0(p0_i: kernel.index ):\n");
    source.push_str(&binary("epoch0", "two", "p0_i", "Multiply"));
    source.push_str(&binary("slot0", "epoch0", "two", "Remainder"));
    start_phase(&mut source, "epoch0", "slot0");
    source.push_str(&access(
        "slot0",
        if reduction { "zero" } else { "lane" },
        "Read",
    ));
    source.push_str(&event("epoch0", "slot0", "Release"));
    source.push_str(
        "kernel.br_args (p0_i) [^phase1] []: <(kernel.index ) -> ()>\n\
        ^phase1(p1_i: kernel.index ):\n",
    );
    source.push_str(&binary("twice1", "two", "p1_i", "Multiply"));
    source.push_str(&binary("epoch1", "twice1", "one", "Add"));
    source.push_str(&binary("slot1", "epoch1", "two", "Remainder"));
    start_phase(&mut source, "epoch1", "slot1");
    if reduction {
        source.push_str("kernel.br_args (p1_i, zero) [^read_header] []: <(kernel.index , kernel.index ) -> ()>\n\
            ^read_header(read_outer: kernel.index , column: kernel.index ):\n\
            kernel.index_lt_br_args (column, lanes, read_outer, column, read_outer) [^read_body, ^finish_read] []: <(kernel.index , kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>\n\
            ^read_body(read_body_outer: kernel.index , read_column: kernel.index ):\n");
        source.push_str(&binary("read_twice", "two", "read_body_outer", "Multiply"));
        source.push_str(&binary("read_epoch", "read_twice", "one", "Add"));
        source.push_str(&binary("read_slot", "read_epoch", "two", "Remainder"));
        source.push_str(&access("read_slot", "read_column", "Read"));
        source.push_str(&binary("next_column", "read_column", "one", "Add"));
        source.push_str("kernel.br_args (read_body_outer, next_column) [^read_header] []: <(kernel.index , kernel.index ) -> ()>\n\
            ^finish_read(finish_i: kernel.index ):\n");
        source.push_str(&binary("finish_twice", "two", "finish_i", "Multiply"));
        source.push_str(&binary("finish_epoch", "finish_twice", "one", "Add"));
        source.push_str(&binary("finish_slot", "finish_epoch", "two", "Remainder"));
        source.push_str(&event("finish_epoch", "finish_slot", "Release"));
        source.push_str("kernel.br_args (finish_i) [^latch] []: <(kernel.index ) -> ()>\n");
    } else {
        source.push_str(&access("slot1", "lane", "Read"));
        source.push_str(&event("epoch1", "slot1", "Release"));
        source.push_str("kernel.br_args (p1_i) [^latch] []: <(kernel.index ) -> ()>\n");
    }
    source.push_str("^latch(latch_i: kernel.index ):\n");
    source.push_str(&binary("next_outer", "latch_i", "one", "Add"));
    source.push_str(
        "kernel.br_args (next_outer) [^outer] []: <(kernel.index ) -> ()>\n\
        ^exit():\n kernel.return () [] []: <() -> ()>\n}\n",
    );
    source
}

fn expect_clean(source: &str) {
    let (context, function) = parse(source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.certificates().len(), 1);
    assert!(report.certificates()[0].access_refinement_proven());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

fn expect_rejected(source: &str) {
    let (context, function) = parse(source);
    let report = run_pliron_pipeline_protocol_check_v1(&context, &function);
    assert!(!report.is_clean(), "{report:?}");
    assert!(report.certificates().is_empty());
}

#[test]
fn finite_epoch_p1_two_complete_phases() {
    expect_clean(&fixture(false, false));
}

#[test]
fn finite_epoch_p2_finite_event_free_inner_loop() {
    expect_clean(&fixture(true, false));
}

#[test]
fn finite_epoch_p3_whole_workgroup_broadcast_and_reduction_reads() {
    expect_clean(&fixture(true, true));
}

#[test]
fn finite_epoch_rejects_missing_duplicate_and_reordered_lifecycle() {
    let source = fixture(false, false);
    for kind in ["Stage", "Commit", "Wait", "Consume", "Release"] {
        let line = event("epoch1", "slot1", kind);
        expect_rejected(&source.replacen(&line, "", 1));
        expect_rejected(&source.replacen(&line, &format!("{line}{line}"), 1));
    }
    expect_rejected(&source.replacen(
        &(event("epoch1", "slot1", "Wait") + &event("epoch1", "slot1", "Consume")),
        &(event("epoch1", "slot1", "Consume") + &event("epoch1", "slot1", "Wait")),
        1,
    ));
}

#[test]
fn finite_epoch_rejects_lane_bound_and_nonprogressing_inner_loop() {
    let source = fixture(true, false);
    expect_rejected(&source.replace("(scan_i, seven,", "(scan_i, lane,"));
    expect_rejected(&source.replace(
        &binary("next_scan", "scan_body_i", "one", "Add"),
        &binary("next_scan", "scan_body_i", "zero", "Add"),
    ));
}

#[test]
fn finite_epoch_rejects_wrong_slot_epoch_and_unstaged_coordinate() {
    let source = fixture(false, false);
    expect_rejected(&source.replacen(
        &event("epoch1", "slot1", "Wait"),
        &event("epoch1", "slot0", "Wait"),
        1,
    ));
    expect_rejected(&source.replacen(
        &event("epoch1", "slot1", "Consume"),
        &event("epoch0", "slot1", "Consume"),
        1,
    ));
    expect_rejected(&source.replacen(
        &access("slot1", "lane", "Read"),
        &access("slot1", "lanes", "Read"),
        1,
    ));
    expect_rejected(&source.replacen(&access("slot1", "lane", "Write"), "", 1));
}

#[test]
fn finite_epoch_rejects_unknown_branch_skipping_a_phase() {
    let source = fixture(false, false);
    expect_rejected(&source.replace(
        "kernel.br_args (p0_i) [^phase1] []: <(kernel.index ) -> ()>",
        "kernel.analysis_split (p0_i, p0_i) [^phase1, ^latch] [kernel_analysis_split_control_count: kernel.analysis_split_control_count 0]: <(kernel.index , kernel.index ) -> ()>",
    ));
}

#[test]
fn finite_epoch_rejects_reduction_read_before_consume_and_out_of_range() {
    let source = fixture(true, true);
    expect_rejected(&source.replacen(&event("epoch1", "slot1", "Consume"), "", 1));
    expect_rejected(&source.replacen(
        &access("read_slot", "read_column", "Read"),
        &access("read_slot", "lanes", "Read"),
        1,
    ));
}

#[test]
fn finite_epoch_identical_trace_across_unknown_branch_is_accepted() {
    let source = fixture(false, false);
    expect_clean(&source.replace(
        "kernel.br_args (body_i) [^phase0] []: <(kernel.index ) -> ()>",
        "kernel.analysis_split (body_i, body_i) [^left, ^right] [kernel_analysis_split_control_count: kernel.analysis_split_control_count 0]: <(kernel.index , kernel.index ) -> ()>\n\
         ^left(left_i: kernel.index ):\n\
         kernel.br_args (left_i) [^phase0] []: <(kernel.index ) -> ()>\n\
         ^right(right_i: kernel.index ):\n\
         kernel.br_args (right_i) [^phase0] []: <(kernel.index ) -> ()>",
    ));
}

#[test]
fn finite_epoch_rejects_creation_reentry() {
    let source = fixture(false, false);
    let creation = "pipeline = kernel.pipeline_create (tile) [] []: <(kernel.ranked_view <32,true,[2,128]>) -> (kernel.pipeline <2,1>)>;\n";
    let source = source.replacen(creation, "", 1).replacen(
        "^body(body_i: kernel.index ):\n",
        &format!("^body(body_i: kernel.index ):\n{creation}"),
        1,
    );
    expect_rejected(&source);
}

#[test]
fn finite_epoch_inner_loop_cannot_hide_pipeline_events() {
    let source = fixture(true, false);
    let nested_event = binary("scan_epoch", "two", "scan_body_outer", "Multiply")
        + &binary("scan_slot", "scan_epoch", "two", "Remainder")
        + &event("scan_epoch", "scan_slot", "Stage");
    expect_rejected(&source.replace(
        "^scan_body(scan_body_outer: kernel.index , scan_body_i: kernel.index ):\n",
        &format!("^scan_body(scan_body_outer: kernel.index , scan_body_i: kernel.index ):\n{nested_event}"),
    ));
}

#[test]
fn finite_epoch_rejects_distinct_payloads_on_duplicate_target_edges() {
    let source = fixture(false, false);
    // A target-only lookup would select body_i twice, missing the other edge's
    // induction reset. Every physical edge occurrence must be authenticated.
    for branch in [
        "kernel.analysis_split (body_i, zero) [^phase0, ^phase0] [kernel_analysis_split_control_count: kernel.analysis_split_control_count 0]: <(kernel.index , kernel.index ) -> ()>",
        "kernel.index_lt_br_args (lane, one, body_i, zero) [^phase0, ^phase0] []: <(kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>",
        "kernel.index_eq_br_args (lane, zero, body_i, zero) [^phase0, ^phase0] []: <(kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>",
    ] {
        expect_rejected(&source.replace(
            "kernel.br_args (body_i) [^phase0] []: <(kernel.index ) -> ()>",
            branch,
        ));
    }
}

#[test]
fn finite_epoch_rejects_partial_workgroup_entry_trap_or_bypass() {
    let source = fixture(false, false);
    for exit in [
        "kernel.trap () [] []: <() -> ()>",
        "kernel.return () [] []: <() -> ()>",
    ] {
        let branch = format!(
            "kernel.index_eq_br (lane, zero) [^participates, ^bypasses] []: <(kernel.index , kernel.index ) -> ()>\n\
             ^participates():\n\
             kernel.br_args (zero) [^outer] []: <(kernel.index ) -> ()>\n\
             ^bypasses():\n{exit}"
        );
        expect_rejected(&source.replacen(
            "kernel.br_args (zero) [^outer] []: <(kernel.index ) -> ()>",
            &branch,
            1,
        ));
    }
}

include!("finite_phase_coverage_v1_tests.rs");
