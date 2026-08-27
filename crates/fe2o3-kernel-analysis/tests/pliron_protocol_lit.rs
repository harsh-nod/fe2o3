use std::{fs, path::Path};

use dialect_kernel::{DIALECT_NAME, register_dialect};
use fe2o3_kernel_analysis::run_pliron_barrier_convergence_check_v1;
use pliron::{
    builtin::ops::FuncOp,
    context::Context,
    dialect::DialectName,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
};

#[test]
fn textual_simt_protocol_lit_suite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/protocol-lit");
    let mut fixtures = fs::read_dir(&root)
        .expect("protocol lit fixture directory")
        .map(|entry| entry.expect("protocol lit directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "pliron")
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    assert_eq!(fixtures.len(), 3);
    for fixture in fixtures {
        run_fixture(&fixture);
    }
}

fn run_fixture(path: &Path) {
    let source = fs::read_to_string(path).expect("UTF-8 fixture");
    assert_eq!(
        source
            .lines()
            .filter(|line| *line == "// RUN: fe2o3-pliron-lit --passes=simt-protocol %s")
            .count(),
        1,
    );
    let rejected = source.lines().any(|line| line == "// EXPECT: REJECT");
    assert_ne!(
        rejected,
        source.lines().any(|line| line == "// EXPECT: PASS")
    );
    let checks = source
        .lines()
        .filter_map(|line| line.strip_prefix("// CHECK: "))
        .collect::<Vec<_>>();
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
    let function = FuncOp::from_operation(operation);
    let report = run_pliron_barrier_convergence_check_v1(&context, &function);
    let output = if report.is_clean() {
        "PASS".to_owned()
    } else {
        report
            .findings()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(!report.is_clean(), rejected, "{}: {output}", path.display());
    for check in checks {
        assert!(
            output.contains(check),
            "{} missing CHECK `{check}` in `{output}`",
            path.display(),
        );
    }
}
