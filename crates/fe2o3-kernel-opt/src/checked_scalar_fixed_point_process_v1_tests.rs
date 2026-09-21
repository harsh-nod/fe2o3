use super::{fixture::*, *};
use std::fmt::Write;
const CHILD: &str = "FE2O3_SCALAR_FIXED_POINT_TEST_CHILD_V1";
const BEGIN: &str = "SCALAR_FIXED_POINT_BEGIN\n";
const END: &str = "SCALAR_FIXED_POINT_END";

fn bytes(text: &mut String, value: &[u8]) {
    for byte in value {
        write!(text, "{byte:02x}").unwrap();
    }
    text.push('\n');
}
fn transcript(input: &Owner, owner: &CheckedScalarFixedPointOwnerV1) -> String {
    let mut text = String::new();
    bytes(&mut text, owner.execution().policy_identity());
    bytes(&mut text, owner.execution().canonical_bytes());
    bytes(&mut text, input.canonical().canonical_bytes());
    for round in owner.rounds() {
        writeln!(text, "round={}", round.ordinal()).unwrap();
        bytes(&mut text, round.integer().native_input_audit_bytes());
        bytes(
            &mut text,
            round.integer().owner().canonical().canonical_bytes(),
        );
        bytes(&mut text, round.integer().execution().canonical_bytes());
        writeln!(
            text,
            "integer-rows={:?}",
            round.integer().occurrences().candidate()
        )
        .unwrap();
        writeln!(text, "integer-report={:?}", round.integer().report()).unwrap();
        bytes(&mut text, round.integer().map().digest());
        writeln!(
            text,
            "integer-relations={:?} synthesized={:?}",
            round.integer().map().relations(),
            round.integer().map().synthesized_operations()
        )
        .unwrap();
        for row in round.integer().map().relations() {
            writeln!(text, "{:?}", round.integer().map().targets(row)).unwrap();
        }
        bytes(&mut text, round.scalar().native_input_audit_bytes());
        bytes(
            &mut text,
            round.scalar().owner().canonical().canonical_bytes(),
        );
        bytes(&mut text, round.scalar().execution().canonical_bytes());
        writeln!(
            text,
            "scalar-rows={:?}",
            round.scalar().occurrences().candidate()
        )
        .unwrap();
        writeln!(text, "scalar-report={:?}", round.scalar().report()).unwrap();
        bytes(&mut text, round.scalar().map().digest());
        writeln!(
            text,
            "scalar-relations={:?} synthesized={:?}",
            round.scalar().map().relations(),
            round.scalar().map().synthesized_operations()
        )
        .unwrap();
        for row in round.scalar().map().relations() {
            writeln!(text, "{:?}", round.scalar().map().targets(row)).unwrap();
        }
    }
    text
}

#[test]
#[ignore = "fresh-process helper; parent requires exact child success and complete transcript"]
fn scalar_fixed_point_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
    let mut complete = String::new();
    for module in [
        Module::new("fixed"),
        reverse_chain(1),
        loop_kernel(false, true),
    ] {
        with_input(module, |input, budget| {
            let first = finish(input, budget);
            first.replay_against(input, budget).unwrap();
            let a = transcript(input, &first);
            let second = finish(input, budget);
            second.replay_against(input, budget).unwrap();
            assert_eq!(a, transcript(input, &second));
            complete.push_str(&a);
            release(second, budget);
            release(first, budget);
        });
    }
    assert!(complete.len() < 2 * 1024 * 1024);
    println!("{BEGIN}{complete}{END}");
}

#[test]
fn complete_actual_graphs_rounds_records_rows_and_maps_repeat_in_fresh_processes() {
    let executable = std::env::current_exe().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let child = format!("{module}::scalar_fixed_point_child");
    let mut previous = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&executable)
            .args(["--exact", &child, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let output = std::str::from_utf8(&result.stdout).unwrap();
        assert_eq!(output.matches(BEGIN).count(), 1);
        assert_eq!(output.matches(END).count(), 1);
        let value = output
            .split_once(BEGIN)
            .unwrap()
            .1
            .split_once(END)
            .unwrap()
            .0;
        assert!(value.len() < 2 * 1024 * 1024);
        assert!(value.contains("round=2"));
        if let Some(previous) = &previous {
            assert_eq!(value, previous);
        } else {
            previous = Some(value.to_owned());
        }
    }
}
