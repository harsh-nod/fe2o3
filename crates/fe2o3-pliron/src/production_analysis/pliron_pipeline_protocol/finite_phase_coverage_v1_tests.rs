#[test]
fn finite_coverage_binds_dynamic_invocations_only_to_the_same_exact_layout() {
    let source = fixture(true, true).replace(
        "kernel_launch_extent: kernel.launch_extent 256",
        "kernel_launch_extent: kernel.launch_extent 0",
    );
    expect_clean(&source);
    let layout = source
        .lines()
        .find(|line| line.contains("gpu.execution_layout"))
        .unwrap();
    expect_rejected(&source.replace(layout, ""));
    expect_rejected(&source.replace(
        "gpu.execution_domain FullPhysicalWorkgroups",
        "gpu.execution_domain PotentiallyPartial",
    ));
    let conflicting_layout = layout.replace(
        "gpu_execution_global_x: gpu.execution_extent 256",
        "gpu_execution_global_x: gpu.execution_extent 384",
    );
    expect_rejected(&source.replace(layout, &format!("{layout}\n{conflicting_layout}")));
    let declaration = "other_index = kernel.invocation_index () [] [kernel_invocation_dimension: kernel.invocation_dimension 0, kernel_launch_extent: kernel.launch_extent 256]: <() -> (kernel.index )>;\n";
    expect_rejected(&source.replacen(
        &constant("zero", 0),
        &(declaration.to_owned() + &constant("zero", 0)),
        1,
    ));
    let out_of_layout = declaration.replace(
        "kernel.invocation_dimension 0",
        "kernel.invocation_dimension 3",
    );
    expect_rejected(&source.replacen(
        &constant("zero", 0),
        &(out_of_layout + &constant("zero", 0)),
        1,
    ));
}

#[test]
fn finite_coverage_rejects_unbound_invocation_and_partial_layouts() {
    let source = fixture(true, true);
    for (old, new) in [
        (
            "kernel_invocation_dimension: kernel.invocation_dimension 0",
            "kernel_invocation_dimension: kernel.invocation_dimension 1",
        ),
        (
            "kernel_launch_extent: kernel.launch_extent 256",
            "kernel_launch_extent: kernel.launch_extent 128",
        ),
        (
            "gpu_execution_domain: gpu.execution_domain FullPhysicalWorkgroups",
            "gpu_execution_domain: gpu.execution_domain PotentiallyPartial",
        ),
        (
            "gpu_execution_workgroup_x: gpu.execution_extent 128",
            "gpu_execution_workgroup_x: gpu.execution_extent 64",
        ),
    ] {
        expect_rejected(&source.replace(old, new));
    }
    // A partial last group cannot even carry the native full-physical marker.
    let partial = source.replace(
        "gpu_execution_global_x: gpu.execution_extent 256",
        "gpu_execution_global_x: gpu.execution_extent 255",
    );
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &partial).unwrap();
    assert!(verify_operation(operation, &context).is_err());
    expect_rejected(&partial.replace(
        "gpu.execution_domain FullPhysicalWorkgroups",
        "gpu.execution_domain PotentiallyPartial",
    ));
}

#[test]
fn finite_coverage_layout_metadata_does_not_supply_missing_lane_writes() {
    let source = fixture(false, false);
    for coordinate in ["zero", "one", "seven"] {
        expect_rejected(&source.replacen(
            &access("slot0", "lane", "Write"),
            &access("slot0", coordinate, "Write"),
            1,
        ));
    }
    expect_rejected(&source.replace(&constant("lanes", 128), &constant("lanes", 64)));
    expect_rejected(&source.replace(
        "kernel.ranked_view <32,true,[2,128]>",
        "kernel.ranked_view <16,true,[2,128]>",
    ));
}

#[test]
fn finite_coverage_rejects_a_writer_missing_on_one_predecessor() {
    let source = fixture(false, false);
    let writer = access("slot0", "lane", "Write");
    let conditional = format!(
        "kernel.index_eq_br_args (lane, zero, p0_i, p0_i) [^only_leader, ^after_write] []: <(kernel.index , kernel.index , kernel.index , kernel.index ) -> ()>\n\
         ^only_leader(writer_i: kernel.index ):\n\
         {writer}\
         kernel.br_args (writer_i) [^after_write] []: <(kernel.index ) -> ()>\n\
         ^after_write(join_i: kernel.index ):\n"
    );
    expect_rejected(&source.replacen(&writer, &conditional, 1));
}

#[test]
fn finite_coverage_rejects_divergent_preheader_participation() {
    let source = fixture(false, false);
    expect_rejected(&source.replacen(
        "kernel.br_args (zero) [^outer] []: <(kernel.index ) -> ()>",
        "kernel.index_eq_br_args (lane, zero, zero) [^outer, ^exit] []: <(kernel.index , kernel.index , kernel.index ) -> ()>",
        1,
    ));
}

#[test]
fn finite_coverage_rejects_aliased_views_and_wrong_owner_reads() {
    let source = fixture(false, false);
    let alias = "other = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Workgroup]: <() -> (kernel.ranked_view <32,true,[2,128]>)>;\n";
    let source = source.replacen(
        &constant("zero", 0),
        &(alias.to_owned() + &constant("zero", 0)),
        1,
    );
    expect_rejected(&source);
    expect_rejected(&source.replacen(
        &access("slot1", "lane", "Read"),
        &access("slot1", "lane", "Read").replace("(tile,", "(other,"),
        1,
    ));
}

#[test]
fn finite_coverage_rejects_write_after_commit_and_reads_outside_the_phase() {
    let source = fixture(false, false);
    let writer = access("slot0", "lane", "Write");
    let commit = event("epoch0", "slot0", "Commit");
    expect_rejected(&source.replacen(&(writer.clone() + &commit), &(commit + &writer), 1));
    let reader = access("slot1", "lane", "Read");
    let release = event("epoch1", "slot1", "Release");
    expect_rejected(&source.replacen(&(reader.clone() + &release), &(release + &reader), 1));
    expect_rejected(&source.replacen(&reader, &access("slot0", "lane", "Read"), 1));
    expect_rejected(&source.replacen(&reader, &access("slot1", "lanes", "Read"), 1));
    let unknown = "unknown = kernel.index_unknown () [] []: <() -> (kernel.index )>;\n";
    let source = source.replacen(
        &constant("zero", 0),
        &(unknown.to_owned() + &constant("zero", 0)),
        1,
    );
    expect_rejected(&source.replacen(&reader, &access("slot1", "unknown", "Read"), 1));
    expect_rejected(&source.replacen(&writer, &access("slot0", "unknown", "Write"), 1));
}
