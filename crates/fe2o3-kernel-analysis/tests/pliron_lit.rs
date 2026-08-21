use std::{fs, path::Path};

use dialect_kernel::{DIALECT_NAME, register_dialect};
use fe2o3_kernel_analysis::{
    GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1, KernelCheckPassKindV1,
    require_general_pliron_kernel_checks_before_lowering_v1,
};
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
        fixtures.len() >= 19,
        "generic kernel-check lit suite unexpectedly shrank"
    );
    for fixture in fixtures {
        run_fixture(&fixture);
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
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &ir)
        .unwrap_or_else(|error| panic!("{} failed to parse: {error:?}", path.display()));
    verify_operation(operation, &context)
        .unwrap_or_else(|error| panic!("{} failed local verification: {error:?}", path.display()));
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    let function = FuncOp::from_operation(operation);
    let result = require_general_pliron_kernel_checks_before_lowering_v1(&context, &function);
    let output = match result {
        Ok(report) => {
            assert!(report.is_clean());
            "PASS".to_owned()
        }
        Err(error) => error.to_string(),
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

#[test]
fn lit_pipeline_uses_the_fixed_workload_neutral_pass_order() {
    assert_eq!(
        GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1,
        [
            KernelCheckPassKindV1::MemoryBounds,
            KernelCheckPassKindV1::RaceFreedom,
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::WorkgroupMemory,
            KernelCheckPassKindV1::SemanticRefinement,
        ]
    );
    assert!(
        GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1
            .iter()
            .all(|pass| !pass.name().contains("gemm"))
    );
}

fn result_is_rejected(output: &str) -> bool {
    output != "PASS"
}
