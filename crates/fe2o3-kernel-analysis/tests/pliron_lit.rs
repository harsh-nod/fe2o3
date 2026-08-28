use std::{fmt::Write, fs, path::Path};

use dialect_kernel::{AtomicScopeAttr, DIALECT_NAME, MemorySpaceAttr, register_dialect};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
    PlironAtomicTargetCapabilityV1, PlironAtomicTargetContextV1, PlironHostAllocationV1,
    PlironLaunchContractV1, PlironLaunchTargetLimitsV1,
    require_production_pliron_checks_before_lowering_v2,
    require_production_pliron_checks_with_atomic_and_target_before_lowering_v2,
    require_production_pliron_checks_with_atomic_target_before_lowering_v2,
    require_production_pliron_checks_with_target_before_lowering_v2,
};
use fe2o3_pliron_owner_core::ensure_context_identity;
use pliron::{
    builtin::ops::FuncOp,
    context::Context,
    dialect::DialectName,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
};

const MAX_FIXTURE_BYTES: u64 = 64 * 1024;

#[test]
fn textual_pliron_lit_suite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lit");
    let mut fixtures = fs::read_dir(&root)
        .expect("lit fixture directory")
        .map(|entry| entry.expect("lit directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "pliron")
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(
        fixtures.len() >= 31,
        "generic kernel-check lit suite unexpectedly shrank"
    );
    for fixture in fixtures {
        run_fixture(&fixture);
    }
}

#[test]
fn total_coverage_textual_pliron_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lit");
    for fixture in [
        "ownership_collective_complete.pliron",
        "ownership_collective_missing.pliron",
        "ownership_total_extra_write.pliron",
        "ownership_total_guarded_tail.pliron",
        "ownership_total_multidimensional.pliron",
        "ownership_total_overwrite.pliron",
    ] {
        run_fixture(&root.join(fixture));
    }
}

#[test]
fn collective_semantics_textual_pliron_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lit");
    for fixture in [
        "collective_fold_policy_checked.pliron",
        "collective_fold_missing_value_proof.pliron",
        "collective_fold_coverage_mismatch.pliron",
        "collective_recurrence_policy_checked.pliron",
        "ownership_total_multi_output_disjoint.pliron",
        "ownership_total_multi_output_overlap.pliron",
        "collective_permutation_policy_checked.pliron",
    ] {
        run_fixture(&root.join(fixture));
    }
}

#[test]
fn parallel_reference_prerequisite_mutation_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lit");
    for fixture in [
        "ownership_hole.pliron",
        "ownership_nonrectangular_subgroup.pliron",
        "ownership_total_overwrite.pliron",
        "collective_fold_policy_mismatch.pliron",
        "collective_recurrence_witness_type_mismatch.pliron",
        "collective_permutation_non_integer_map.pliron",
        "tensor_layout_wrong_accumulator_permutation.pliron",
        "tensor_layout_wrong_fragment_width.pliron",
        "tensor_layout_wrong_role.pliron",
        "tensor_layout_missing_tail.pliron",
        "tensor_layout_partial_subgroup.pliron",
        "tensor_layout_subgroup_scoped.pliron",
        "tensor_layout_mixed_swizzle.pliron",
        "tensor_layout_divergent_trace.pliron",
    ] {
        run_fixture(&root.join(fixture));
    }
}

fn run_fixture(path: &Path) {
    let metadata = fs::metadata(path).expect("fixture metadata");
    assert!(metadata.is_file());
    assert!(metadata.len() <= MAX_FIXTURE_BYTES);
    let source = fs::read_to_string(path).expect("UTF-8 fixture");
    assert_eq!(
        source
            .lines()
            .filter(|line| *line == "// RUN: fe2o3-pliron-lit --passes=general %s")
            .count(),
        1,
        "{} must name the production pass pipeline exactly once",
        path.display(),
    );
    let expectation_count = source
        .lines()
        .filter(|line| matches!(*line, "// EXPECT: PASS" | "// EXPECT: REJECT"))
        .count();
    assert_eq!(
        expectation_count,
        1,
        "{} has ambiguous EXPECT",
        path.display()
    );
    let checks = source
        .lines()
        .filter_map(|line| line.strip_prefix("// CHECK: "))
        .collect::<Vec<_>>();
    let rejected = source.lines().any(|line| line == "// EXPECT: REJECT");
    assert!(!checks.is_empty(), "{} has no CHECK lines", path.display());
    let ir = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut context = Context::new();
    ensure_context_identity(&mut context).expect("lit context identity");
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &ir)
        .unwrap_or_else(|error| panic!("{} failed to parse: {error:?}", path.display()));
    verify_operation(operation, &context)
        .unwrap_or_else(|error| panic!("{} failed local verification: {error:?}", path.display()));
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    let function = FuncOp::from_operation(operation);
    let capabilities = source
        .lines()
        .filter_map(|line| line.strip_prefix("// ATOMIC-CAPABILITY: "))
        .map(parse_atomic_capability)
        .collect::<Vec<_>>();
    let atomic_target = (!capabilities.is_empty()).then(|| {
        PlironAtomicTargetContextV1::new(capabilities).expect("valid bounded atomic target context")
    });
    let target_contract = parse_target_contract(&source);
    let result = match (atomic_target.as_ref(), target_contract.as_ref()) {
        (Some(atomic), Some(target)) => {
            require_production_pliron_checks_with_atomic_and_target_before_lowering_v2(
                &context, &function, atomic, target,
            )
        }
        (Some(target), None) => {
            require_production_pliron_checks_with_atomic_target_before_lowering_v2(
                &context, &function, target,
            )
        }
        (None, Some(target)) => require_production_pliron_checks_with_target_before_lowering_v2(
            &context, &function, target,
        ),
        (None, None) => require_production_pliron_checks_before_lowering_v2(&context, &function),
    };
    let output = match result {
        Ok(report) => {
            assert!(report.is_clean());
            let mut output = "PASS".to_owned();
            if source.lines().any(|line| line == "// CHECK-WITNESSES") {
                for stage in report.report_validation().stages() {
                    write!(
                        output,
                        "\nWITNESS {:?} {:?} obligations={}",
                        stage.checkpoint().pass(),
                        stage.witness().coverage().status(),
                        stage.witness().coverage().obligation_count(),
                    )
                    .expect("write witness output");
                }
            }
            output
        }
        Err(error) => {
            let repairs = error.repair_hints();
            assert!(
                !repairs.is_empty(),
                "{} production error has no repair hint",
                path.display()
            );
            assert!(repairs.iter().all(|repair| !repair.message().is_empty()));
            let rendered = error.to_string();
            assert!(
                rendered.contains("help[FE2O3-FIX-"),
                "{} production error does not render its structured repair: {rendered}",
                path.display()
            );
            rendered
        }
    };
    assert_eq!(
        result_is_rejected(&output),
        rejected,
        "{}: {output}",
        path.display()
    );
    for check in checks {
        assert!(
            output.contains(check),
            "{} missing CHECK `{check}` in `{output}`",
            path.display()
        );
    }
}

fn parse_target_contract(source: &str) -> Option<PlironLaunchContractV1> {
    let target = source
        .lines()
        .find_map(|line| line.strip_prefix("// TARGET: "))?;
    let fields = target.split_ascii_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "malformed TARGET directive");
    let triple = |field: &str, name: &str| {
        let values = field
            .strip_prefix(name)
            .unwrap()
            .split(',')
            .map(|value| value.parse::<u64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 3);
        [values[0], values[1], values[2]]
    };
    let scalar =
        |field: &str, name: &str| field.strip_prefix(name).unwrap().parse::<u64>().unwrap();
    let subgroups = fields[3]
        .strip_prefix("subgroups=")
        .unwrap()
        .split(',')
        .map(|value| value.parse::<u64>().unwrap())
        .collect();
    let limits = PlironLaunchTargetLimitsV1::new(
        triple(fields[0], "grid="),
        triple(fields[1], "workgroup="),
        scalar(fields[2], "invocations="),
        subgroups,
        scalar(fields[4], "lds="),
        scalar(fields[5], "alignment="),
        scalar(fields[6], "allocations=") as usize,
    )
    .unwrap();
    let allocations = source
        .lines()
        .filter_map(|line| line.strip_prefix("// HOST-ALLOCATION: "))
        .map(|line| {
            let values = line
                .split_ascii_whitespace()
                .map(|value| value.parse::<u64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 3);
            PlironHostAllocationV1::new(values[0], values[1], values[2]).unwrap()
        })
        .collect();
    Some(PlironLaunchContractV1::new(limits, allocations).unwrap())
}

#[test]
fn lit_pipeline_uses_the_fixed_workload_neutral_pass_order() {
    assert_eq!(
        PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        [
            KernelCheckPassKindV1::TensorLayout,
            KernelCheckPassKindV1::MemoryBounds,
            KernelCheckPassKindV1::AtomicLegality,
            KernelCheckPassKindV1::RaceFreedom,
            KernelCheckPassKindV1::HierarchicalOwnership,
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::PipelineProtocol,
            KernelCheckPassKindV1::WorkgroupMemory,
            KernelCheckPassKindV1::SemanticRefinement,
        ]
    );
    assert!(
        PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
            .iter()
            .all(|pass| !pass.name().contains("gemm"))
    );
}

fn parse_atomic_capability(source: &str) -> PlironAtomicTargetCapabilityV1 {
    let fields = source.split_ascii_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 3, "malformed ATOMIC-CAPABILITY directive");
    let width = fields[0].parse::<u32>().expect("atomic capability width");
    let memory_space = match fields[1] {
        "Global" => MemorySpaceAttr::Global,
        "Workgroup" => MemorySpaceAttr::Workgroup,
        other => panic!("unsupported atomic capability memory space {other}"),
    };
    let max_scope = match fields[2] {
        "Workgroup" => AtomicScopeAttr::Workgroup,
        "Agent" => AtomicScopeAttr::Agent,
        "Device" => AtomicScopeAttr::Device,
        "System" => AtomicScopeAttr::System,
        other => panic!("unsupported atomic capability scope {other}"),
    };
    PlironAtomicTargetCapabilityV1::new(width, memory_space, max_scope)
        .expect("valid atomic capability")
}

fn result_is_rejected(output: &str) -> bool {
    !output.starts_with("PASS")
}
