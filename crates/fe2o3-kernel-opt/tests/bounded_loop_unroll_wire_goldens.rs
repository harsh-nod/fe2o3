//! Canonical-wire regression graphs, not Pliron textual lit or source admission.
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollCopyV1 as CopyRole, CanonicalKirLoopUnrollErrorV1 as PairError,
    CanonicalKirLoopUnrollLimitsV1 as Limits, CanonicalKirLoopUnrollOriginsV1 as Origins,
    CanonicalKirLoopUnrollSelectionV1 as Selection,
    check_canonical_kir_loop_unroll_pair_v1 as check,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CheckedBinaryOperator, KernelIrDecodeError, OperationKind as Kind, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::{
    OwnedLoopUnrollErrorV1 as OwnedError, unroll_canonical_kir_loops_v1 as unroll,
};
use std::fmt::Write;

#[path = "common/canonical_wire_observation_v1.rs"]
mod common;
use common::{STORAGE, WORK, decode, dump, hex, operation, site};

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    wire: &'static str,
    golden: &'static str,
    iterations: Option<u8>,
    private: bool,
}
macro_rules! case {
    ($name:literal, $iterations:expr, $private:expr) => {
        Case {
            name: $name,
            wire: include_str!(concat!("bounded-loop-unroll-golden/", $name, ".hex")),
            golden: include_str!(concat!("bounded-loop-unroll-golden/", $name, ".golden")),
            iterations: $iterations,
            private: $private,
        }
    };
}
const CASES: [Case; 5] = [
    case!("checked-u32-zero", Some(0), false),
    case!("checked-u32-one", Some(1), false),
    case!("checked-u32-three-private", Some(3), true),
    case!("symbolic-noop", None, false),
    case!("over-ceiling-noop", None, false),
];

fn block(at: Block) -> String {
    format!("f{}:b{}", at.function.0, at.block)
}
fn definition(at: Definition) -> String {
    match at {
        Definition::FunctionArgument { function, argument } => {
            format!("f{}:a{argument}", function.0)
        }
        Definition::BlockArgument {
            block: at,
            argument,
        } => format!("{}:a{argument}", block(at)),
        Definition::Result { operation, result } => format!("{}:r{result}", site(operation)),
    }
}
fn edge(at: Edge) -> String {
    format!("{}:e{}", block(at.source), at.successor)
}
fn argument(at: Argument) -> String {
    format!("{}:a{}", edge(at.edge), at.argument)
}
fn copy(role: CopyRole) -> String {
    match role {
        CopyRole::Retained => "retained".into(),
        CopyRole::Header(n) => format!("header({n})"),
        CopyRole::Body(n) => format!("body({n})"),
        CopyRole::OmittedBody => "omitted-body".into(),
    }
}
fn same_origins(left: Origins<'_>, right: Origins<'_>) {
    assert_eq!(left.selection, right.selection);
    assert_eq!(left.blocks, right.blocks);
    assert_eq!(left.definitions, right.definitions);
    assert_eq!(left.operations, right.operations);
    assert_eq!(left.terminators, right.terminators);
    assert_eq!(left.edges, right.edges);
    assert_eq!(left.arguments, right.arguments);
}
fn remarks(text: &mut String, limits: Limits, rows: Origins<'_>) {
    let l = limits.loops;
    writeln!(text, "CHECK-REMARK: limits functions={} blocks={} edges={} definitions={} operations={} loops={} rows={} max-iterations={} operand-uses={} edge-arguments={} origin-rows={}",
        l.functions, l.blocks, l.edges, l.definitions, l.operations, l.loops, l.rows,
        limits.max_iterations, limits.max_output_operand_uses, limits.max_output_edge_arguments,
        limits.max_origin_rows).unwrap();
    match rows.selection {
        Some(s) => writeln!(text, "CHECK-REMARK: selection fact={} iterations={} independent-pair=true owning-replay=true authority=false", s.fact, s.iterations).unwrap(),
        None => writeln!(text, "CHECK-REMARK: selection none independent-pair=true owning-replay=true authority=false").unwrap(),
    }
    macro_rules! table {
        ($label:literal, $rows:expr, $format:expr) => {
            for row in $rows {
                let output = row.output.map($format).unwrap_or_else(|| "none".into());
                writeln!(
                    text,
                    "CHECK-REMARK: {} {} -> {} {}",
                    $label,
                    $format(row.input),
                    output,
                    copy(row.copy)
                )
                .unwrap();
            }
        };
    }
    table!("block", rows.blocks, block);
    table!("definition", rows.definitions, definition);
    table!("operation", rows.operations, site);
    table!("terminator", rows.terminators, block);
    table!("edge", rows.edges, edge);
    table!("argument", rows.arguments, argument);
}

#[derive(Debug, Eq, PartialEq)]
struct Observation {
    text: String,
    bytes: [Vec<u8>; 2],
}
fn observe(case: Case) -> Observation {
    let wire = hex(case.wire);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let ledger = budget.work_ledger_identity_v1();
    let sibling = vec![0x6bu8; 43];
    let sibling_storage = std::mem::size_of_val(&sibling) + sibling.capacity();
    budget.reserve_storage(sibling_storage).unwrap();
    let (input, input_storage) = decode(&wire, &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let floor = budget.storage();
    let (owner, storage) = unroll(&input, Limits::default(), &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let final_floor = budget.storage();
    let rows = owner.origins();
    assert_eq!(
        rows.selection,
        case.iterations.map(|iterations| Selection {
            fact: 0,
            iterations
        })
    );
    assert_eq!(owner.limits(), Limits::default());
    assert!(!owner.grants_authority());
    if let Some(n) = case.iterations {
        assert_eq!(
            owner.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .len(),
            3 + 2 * usize::from(n)
        );
        assert_eq!(
            rows.blocks
                .iter()
                .filter(|r| matches!(r.copy, CopyRole::Header(_)))
                .count(),
            usize::from(n) + 1
        );
        assert_eq!(
            rows.blocks
                .iter()
                .filter(|r| matches!(r.copy, CopyRole::Body(_)))
                .count(),
            usize::from(n)
        );
        assert_eq!(
            rows.blocks
                .iter()
                .filter(|r| r.copy == CopyRole::OmittedBody)
                .count(),
            usize::from(n == 0)
        );
        if case.private {
            for is_load in [false, true] {
                let count = rows
                    .operations
                    .iter()
                    .filter(|r| {
                        r.output.is_some()
                            && matches!(r.copy, CopyRole::Body(_))
                            && if is_load {
                                matches!(operation(input.module(), r.input).kind, Kind::Load { .. })
                            } else {
                                matches!(
                                    operation(input.module(), r.input).kind,
                                    Kind::Store { .. }
                                )
                            }
                    })
                    .count();
                assert_eq!(count, usize::from(n));
            }
        }
    } else {
        assert!(rows.blocks.iter().all(|r| r.copy == CopyRole::Retained));
    }
    let pair_storage = {
        let (pair, ps) = check(&input, owner.output(), rows, owner.limits(), &mut budget).unwrap();
        assert_eq!(budget.storage(), final_floor);
        budget.reserve_storage(ps.retained_storage()).unwrap();
        let (replay, rs) = owner.replay(&input, owner.limits(), &mut budget).unwrap();
        assert_eq!(budget.storage(), final_floor + ps.retained_storage());
        budget.reserve_storage(rs.retained_storage()).unwrap();
        for actual in [&pair, &replay] {
            assert!(std::ptr::eq(actual.input(), &input));
            assert!(std::ptr::eq(actual.output(), owner.output()));
            assert_eq!(actual.limits(), owner.limits());
            same_origins(actual.origins(), rows);
            assert!(!actual.grants_authority());
        }
        ps.retained_storage() + rs.retained_storage()
    };
    budget.release_storage(pair_storage).unwrap();
    assert_eq!(budget.storage(), final_floor);
    // Golden strings and byte copies are harness-owned, not verifier scratch.
    let mut text = format!("CASE: {}\n", case.name);
    dump(&mut text, "CHECK-BEFORE", input.module());
    dump(&mut text, "CHECK-AFTER", owner.output().module());
    remarks(&mut text, owner.limits(), rows);
    let bytes = [&input, owner.output()].map(|o| o.canonical().canonical_bytes().to_vec());
    assert_eq!(bytes[0], wire);
    assert_eq!(bytes[0] != bytes[1], case.iterations.is_some());
    let (repeated, repeated_storage) = unroll(owner.output(), owner.limits(), &mut budget).unwrap();
    budget
        .reserve_storage(repeated_storage.retained_storage())
        .unwrap();
    assert_eq!(repeated.origins().selection, None);
    assert!(
        repeated
            .origins()
            .blocks
            .iter()
            .all(|r| r.copy == CopyRole::Retained)
    );
    assert_eq!(repeated.output().canonical().canonical_bytes(), bytes[1]);
    let repeat_pair_storage = {
        let (pair, ps) = repeated
            .replay(owner.output(), repeated.limits(), &mut budget)
            .unwrap();
        budget.reserve_storage(ps.retained_storage()).unwrap();
        same_origins(pair.origins(), repeated.origins());
        assert!(std::ptr::eq(pair.input(), owner.output()));
        assert!(std::ptr::eq(pair.output(), repeated.output()));
        ps.retained_storage()
    };
    budget.release_storage(repeat_pair_storage).unwrap();
    drop(repeated);
    budget
        .release_storage(repeated_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), final_floor);
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), sibling_storage);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x6b; 43]);
    drop(sibling);
    budget.release_storage(sibling_storage).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
    Observation { text, bytes }
}

#[test]
fn literal_v12_bounded_unroll_goldens() {
    for case in CASES {
        let first = observe(case);
        assert_eq!(first.text, case.golden, "{}", case.name);
        assert_eq!(first, observe(case), "{}", case.name);
    }
}

const CHILD: &str = "FE2O3_TEST_BOUNDED_UNROLL_WIRE_CHILD";
const BEGIN: &str = "FE2O3_BOUNDED_UNROLL_WIRE_BEGIN\n";
const END: &str = "FE2O3_BOUNDED_UNROLL_WIRE_END";
#[test]
#[ignore = "subprocess helper: actual canonical-wire unroll determinism"]
fn canonical_bounded_unroll_wire_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
    let mut transcript = String::new();
    for case in CASES {
        let observed = observe(case);
        assert_eq!(observed.text, case.golden);
        transcript.push_str(&observed.text);
        for (label, bytes) in ["INPUT", "OUTPUT"].into_iter().zip(observed.bytes) {
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
fn canonical_bounded_unroll_wire_repeats_in_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args([
                "--exact",
                "canonical_bounded_unroll_wire_child",
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
        for label in ["CANONICAL-INPUT: ", "CANONICAL-OUTPUT: "] {
            assert_eq!(transcript.matches(label).count(), CASES.len());
        }
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}

#[test]
fn malformed_unroll_literal_wire_is_refused_before_transformation() {
    let valid = hex(CASES[0].wire);
    for mode in 0..5 {
        let mut bytes = valid.clone();
        match mode {
            0 => bytes[0] ^= 1,
            1 => bytes[8] = 11,
            2 => bytes[10] = 1,
            3 => {
                bytes.pop();
            }
            4 => bytes.push(0),
            _ => unreachable!(),
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let sibling = vec![0x6bu8; 43];
        let floor = std::mem::size_of_val(&sibling) + sibling.capacity();
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let Err(AdmissionError::Decode(error)) =
            Owner::from_canonical_bytes_with_verification_budget_v12(&bytes, &mut budget)
        else {
            panic!("exact wire decode refusal");
        };
        assert_eq!(
            error,
            match mode {
                0 => KernelIrDecodeError::InvalidMagic,
                1 => KernelIrDecodeError::UnknownVersion(11),
                2 => KernelIrDecodeError::UnsupportedFlags(1),
                3 => KernelIrDecodeError::Truncated,
                4 => KernelIrDecodeError::TrailingBytes,
                _ => unreachable!(),
            }
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x6b; 43]);
        drop(sibling);
        budget.release_storage(floor).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn actual_unroll_wire_pair_rejects_hostile_rows_and_endpoints() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let sibling = vec![0x6bu8; 43];
    let sibling_storage = std::mem::size_of_val(&sibling) + sibling.capacity();
    budget.reserve_storage(sibling_storage).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (input, input_storage) = decode(&hex(CASES[2].wire), &mut budget);
    budget.reserve_storage(input_storage).unwrap();
    let (owner, storage) = unroll(&input, Limits::default(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        owner.origins().selection,
        Some(Selection {
            fact: 0,
            iterations: 3
        })
    );
    let floor = budget.storage();
    // Candidate row copies and graph payloads are owned by this test harness.
    macro_rules! hostile {
        ($field:ident) => {
            for remove in [false, true] {
                let mut changed = owner.origins().$field.to_vec();
                assert!(!changed.is_empty());
                if remove {
                    changed.pop();
                } else {
                    changed[0].copy = CopyRole::OmittedBody;
                }
                let rows = Origins {
                    $field: &changed,
                    ..owner.origins()
                };
                assert!(matches!(
                    check(&input, owner.output(), rows, owner.limits(), &mut budget),
                    Err(PairError::Mismatch("complete ordered origin"))
                ));
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x6b; 43]);
            }
        };
    }
    hostile!(blocks);
    hostile!(definitions);
    hostile!(operations);
    hostile!(terminators);
    hostile!(edges);
    hostile!(arguments);
    let (foreign, foreign_storage) = decode(&hex(CASES[1].wire), &mut budget);
    budget.reserve_storage(foreign_storage).unwrap();
    assert!(matches!(
        owner.replay(&foreign, owner.limits(), &mut budget),
        Err(OwnedError::ForeignInput)
    ));
    assert_eq!(budget.storage(), floor + foreign_storage);
    drop(foreign);
    budget.release_storage(foreign_storage).unwrap();
    let changed = Limits {
        max_iterations: 2,
        ..owner.limits()
    };
    assert!(matches!(
        owner.replay(&input, changed, &mut budget),
        Err(OwnedError::LimitsMismatch)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        check(
            &input,
            owner.output(),
            owner.origins(),
            changed,
            &mut budget
        ),
        Err(PairError::Mismatch("first qualifying literal fact"))
    ));
    assert_eq!(budget.storage(), floor);
    let mut candidate = owner.output().module().clone();
    let at = owner
        .origins()
        .operations
        .iter()
        .find(|row| {
            matches!(row.copy, CopyRole::Body(0))
                && matches!(
                    operation(input.module(), row.input).kind,
                    Kind::Binary {
                        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                        ..
                    }
                )
        })
        .unwrap()
        .output
        .unwrap();
    let Kind::Binary { rhs, .. } = &mut candidate.functions[at.block.function.0 as usize]
        .body
        .as_mut()
        .unwrap()
        .blocks[at.block.block as usize]
        .operations[at.operation as usize]
        .kind
    else {
        panic!("actual cloned checked Add");
    };
    *rhs = ValueId(12);
    let (hostile, hs) =
        Owner::from_module_ref_with_verification_budget_v12(&candidate, &mut budget).unwrap();
    budget.reserve_storage(hs.retained_storage()).unwrap();
    assert!(matches!(
        check(
            &input,
            &hostile,
            owner.origins(),
            owner.limits(),
            &mut budget
        ),
        Err(PairError::Mismatch(
            "cloned operation payload/operand order"
        ))
    ));
    assert_eq!(budget.storage(), floor + hs.retained_storage());
    drop(hostile);
    budget.release_storage(hs.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x6b; 43]);
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    drop(sibling);
    budget.release_storage(sibling_storage).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
}
