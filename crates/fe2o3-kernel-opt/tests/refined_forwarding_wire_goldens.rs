//! Literal canonical V12 bytes through both actual owning transformations.
//! These are canonical-wire goldens, not Pliron textual lit or source admission.
use fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1 as RefineRow;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::{
    prepare_owned_cross_block_forwarding_v1 as forward,
    prepare_owned_induction_refinement_v1 as refine,
};
use std::fmt::Write;

#[path = "refined-forwarding-golden/observation_tests.rs"]
mod harness;
#[path = "refined-forwarding-golden/refusal_tests.rs"]
mod refusal_tests;
#[path = "refined-forwarding-golden/resource_tests.rs"]
mod resource_tests;
use harness::{CASES, Case, STORAGE, WORK, decode, dump, hex, site};

#[derive(Debug, Eq, PartialEq)]
struct Observation {
    text: String,
    bytes: [Vec<u8>; 3],
}

fn observe(case: Case) -> Observation {
    let wire = hex(case.wire);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let ledger = budget.work_ledger_identity_v1();
    let sibling = vec![0x5au8; 43];
    let sibling_storage = std::mem::size_of_val(&sibling) + sibling.capacity();
    budget.reserve_storage(sibling_storage).unwrap();
    let (input, input_storage) = decode(&wire, &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let input_floor = budget.storage();
    let refined = refine(&input, Default::default(), &mut budget).unwrap();
    assert_eq!(budget.storage(), input_floor);
    budget.reserve_storage(refined.retained_storage()).unwrap();
    let middle_floor = budget.storage();
    let final_owner = forward(refined.output(), Default::default(), &mut budget).unwrap();
    assert_eq!(budget.storage(), middle_floor);
    budget
        .reserve_storage(final_owner.retained_storage())
        .unwrap();
    let final_floor = budget.storage();
    assert_eq!(refined.input_identity(), input.canonical().identity());
    assert_eq!(
        final_owner.input_identity(),
        refined.output().canonical().identity()
    );
    assert!(!refined.grants_authority() && !final_owner.grants_authority());
    let splits = refined
        .origins()
        .iter()
        .filter(|r| matches!(r, RefineRow::CheckedAddSplit { .. }))
        .count();
    let forwards = final_owner
        .origins()
        .iter()
        .filter(|r| r.store.is_some())
        .count();
    assert_eq!(
        (splits, forwards),
        (case.splits, case.forwards),
        "{}",
        case.name
    );
    let pair_storage = {
        let (left, ls) = refined
            .replay_against(&input, refined.limits(), &mut budget)
            .unwrap();
        assert_eq!(budget.storage(), final_floor);
        budget.reserve_storage(ls.retained_storage()).unwrap();
        let (right, rs) = final_owner
            .replay_against(refined.output(), &mut budget)
            .unwrap();
        assert_eq!(budget.storage(), final_floor + ls.retained_storage());
        budget.reserve_storage(rs.retained_storage()).unwrap();
        assert!(std::ptr::eq(left.input(), &input));
        assert!(std::ptr::eq(left.output(), right.input()));
        assert!(std::ptr::eq(right.output(), final_owner.output()));
        assert_eq!(left.origins(), refined.origins());
        assert_eq!(right.origins(), final_owner.origins());
        assert!(!left.grants_authority() && !right.grants_authority());
        ls.retained_storage() + rs.retained_storage()
    };
    budget.release_storage(pair_storage).unwrap();
    assert_eq!(budget.storage(), final_floor);

    // Observation strings/byte copies belong to the harness, not verifier scratch.
    let mut text = format!("CASE: {}\n", case.name);
    dump(&mut text, "CHECK-BEFORE", input.module());
    dump(&mut text, "CHECK-R", refined.output().module());
    dump(&mut text, "CHECK-AFTER", final_owner.output().module());
    writeln!(text, "CHECK-REMARK: splits={splits} forwards={forwards} same-middle-owner=true independent-pairs=true authority=false").unwrap();
    for row in refined.origins() {
        match row {
            RefineRow::Unchanged { input, output } => writeln!(
                text,
                "CHECK-REMARK: R {} unchanged {}",
                site(*input),
                site(*output)
            )
            .unwrap(),
            RefineRow::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                induction_row_ordinal,
            } => writeln!(
                text,
                "CHECK-REMARK: R {} split {},{} fact={induction_row_ordinal}",
                site(*input),
                site(*sum_output),
                site(*false_output)
            )
            .unwrap(),
        }
    }
    for row in final_owner.origins() {
        assert_eq!(row.input, row.output);
        if let Some(store) = row.store {
            let operation = harness::operation(refined.output().module(), store);
            let fe2o3_kernel_ir::OperationKind::Store { value, .. } = operation.kind else {
                panic!("actual retained initializing Store")
            };
            writeln!(
                text,
                "CHECK-REMARK: F {} forwarded store={} value=v{}",
                site(row.input),
                site(store),
                value.0
            )
            .unwrap();
        } else {
            writeln!(text, "CHECK-REMARK: F {} retained", site(row.input)).unwrap();
        }
    }
    let bytes = [&input, refined.output(), final_owner.output()]
        .map(|owner| owner.canonical().canonical_bytes().to_vec());
    assert_eq!(bytes[0], wire);
    assert_eq!(bytes[0] != bytes[1], splits != 0);
    assert_eq!(bytes[1] != bytes[2], forwards != 0);

    let repeated_r = refine(final_owner.output(), Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(repeated_r.retained_storage())
        .unwrap();
    assert!(
        repeated_r
            .origins()
            .iter()
            .all(|r| matches!(r, RefineRow::Unchanged { .. }))
    );
    let repeated_f = forward(repeated_r.output(), Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(repeated_f.retained_storage())
        .unwrap();
    assert!(repeated_f.origins().iter().all(|r| r.store.is_none()));
    assert_eq!(repeated_r.output().canonical().canonical_bytes(), bytes[2]);
    assert_eq!(repeated_f.output().canonical().canonical_bytes(), bytes[2]);
    let size = repeated_f.retained_storage();
    drop(repeated_f);
    budget.release_storage(size).unwrap();
    let size = repeated_r.retained_storage();
    drop(repeated_r);
    budget.release_storage(size).unwrap();
    assert_eq!(budget.storage(), final_floor);
    let size = final_owner.retained_storage();
    drop(final_owner);
    budget.release_storage(size).unwrap();
    let size = refined.retained_storage();
    drop(refined);
    budget.release_storage(size).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), sibling_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, vec![0x5a; 43]);
    drop(sibling);
    budget.release_storage(sibling_storage).unwrap();
    assert_eq!(budget.storage(), 0);
    Observation { text, bytes }
}

#[test]
fn literal_v12_refinement_then_forwarding_goldens() {
    for case in CASES {
        let first = observe(case);
        assert_eq!(first.text, case.golden, "{}", case.name);
        assert_eq!(first, observe(case), "{}", case.name);
    }
}

const CHILD: &str = "FE2O3_TEST_REFINED_FORWARDING_WIRE_CHILD";
const BEGIN: &str = "FE2O3_REFINED_FORWARDING_WIRE_BEGIN\n";
const END: &str = "FE2O3_REFINED_FORWARDING_WIRE_END";

#[test]
#[ignore = "subprocess helper: actual canonical-wire chain determinism"]
fn canonical_refined_forwarding_wire_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
    let mut transcript = String::new();
    for case in CASES {
        let observed = observe(case);
        assert_eq!(observed.text, case.golden);
        transcript.push_str(&observed.text);
        for (label, bytes) in ["L", "R", "F"].into_iter().zip(observed.bytes) {
            write!(transcript, "CANONICAL-{label}: ").unwrap();
            for byte in bytes {
                write!(transcript, "{byte:02x}").unwrap();
            }
            transcript.push('\n');
        }
    }
    assert!(transcript.len() <= 256 * 1024);
    println!("{BEGIN}{transcript}{END}");
}

#[test]
fn canonical_refined_forwarding_wire_repeats_in_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args([
                "--exact",
                "canonical_refined_forwarding_wire_child",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = std::str::from_utf8(&result.stdout).unwrap();
        assert_eq!(
            stdout
                .matches("test result: ok. 1 passed; 0 failed; 0 ignored;")
                .count(),
            1
        );
        assert_eq!(stdout.matches(BEGIN).count(), 1);
        assert_eq!(stdout.matches(END).count(), 1);
        let transcript = stdout
            .split_once(BEGIN)
            .unwrap()
            .1
            .split_once(END)
            .unwrap()
            .0;
        assert!(transcript.len() <= 256 * 1024);
        for label in ["CANONICAL-L: ", "CANONICAL-R: ", "CANONICAL-F: "] {
            assert_eq!(transcript.matches(label).count(), CASES.len());
        }
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}
