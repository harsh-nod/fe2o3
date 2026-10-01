use super::*;

const SHARED: &str = r#"
let local = a ^ b;
let option: Option<&u32> = if a & 1 == 0 { Some(&local) } else { None };
let _result = match option {
    Some(value) => *value ^ b,
    None => a,
};
"#;

const MUTABLE: &str = r#"
let mut local = a ^ b;
let option: Option<&mut u32> = if a & 1 == 0 { Some(&mut local) } else { None };
let _result = match option {
    Some(value) => { *value = *value ^ b; *value },
    None => a,
};
"#;

#[test]
fn original_reference_enum_inputs_use_dynamic_safe_borrows_in_both_roots() {
    for (body, reference) in [(SHARED, "Some(&local)"), (MUTABLE, "Some(&mut local)")] {
        assert!(body.contains(reference));
        assert!(body.contains("if a & 1 == 0"));
        assert!(body.contains("match option"));
        assert!(body.contains("Some(value)"));
        assert!(body.contains("None => a"));
        for excluded in ["unsafe", "addr_of", "&raw", " as *", "transmute"] {
            assert!(!body.contains(excluded));
        }
        let program = original_program(body);
        assert_eq!(program.matches(reference).count(), 2);
        assert!(program.contains("pub fn z_original(a: u32, b: u32)"));
        assert!(program.contains("pub fn a_original(a: u32, b: u32)"));
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the reference enum parent"]
fn original_reference_enum_worker_child() {
    original_mir_worker_child();
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mir_reference_enums_reach_worker_without_forced_addresses() {
    let module = ORIGINAL_CHILD.rsplit_once("::").unwrap().0;
    let child = format!("{module}::reference_enum_tests::original_reference_enum_worker_child");
    run_actual_sources::<OriginalObservation>(
        &[("option_shared", SHARED), ("option_mutable", MUTABLE)],
        &[(0, 0)],
        &child,
        "ORIGINAL_MIR_REFERENCE_ENUM_V45",
        original_program,
        |_, _, case, report, _| {
            assert!(matches!(case, "option_shared" | "option_mutable"));
            assert!(report.work > 0 && report.peak > 0);
            assert_eq!(&report.census[..2], &[2, 2]);
            assert_ne!(report.statement, [0; 32]);
            assert_eq!(report.witness_protocol, [false; 6]);
        },
    );
}
