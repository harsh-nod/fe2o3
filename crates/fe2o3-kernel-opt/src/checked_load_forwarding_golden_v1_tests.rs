//! Observation goldens, not a KIR text parser or a production/lit driver.
use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_load_forwarding_v1;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate,
};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    TwoLoads,
    GlobalStoreBarrier,
    Uninitialized,
    Escaped,
}
const CASES: [Case; 4] = [
    Case::TwoLoads,
    Case::GlobalStoreBarrier,
    Case::Uninitialized,
    Case::Escaped,
];

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::TwoLoads => "two-loads",
            Self::GlobalStoreBarrier => "global-store-barrier",
            Self::Uninitialized => "uninitialized",
            Self::Escaped => "escaped",
        }
    }
    fn golden(self) -> &'static str {
        match self {
            Self::TwoLoads => include_str!("../tests/load-forwarding-golden/two-loads.golden"),
            Self::GlobalStoreBarrier => {
                include_str!("../tests/load-forwarding-golden/global-store-barrier.golden")
            }
            Self::Uninitialized => {
                include_str!("../tests/load-forwarding-golden/uninitialized.golden")
            }
            Self::Escaped => include_str!("../tests/load-forwarding-golden/escaped.golden"),
        }
    }
}

fn input(case: Case) -> Module {
    // Reuse the checked rule's genuine typed fixture. No source parser or
    // synthetic proof record supplies its operations or verified owner.
    let mut module = fixture();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.pop();
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    match case {
        Case::TwoLoads => {}
        Case::GlobalStoreBarrier => {
            let barrier = block.operations[2].clone();
            block.operations.insert(4, barrier);
        }
        Case::Uninitialized => {
            block.operations.remove(1);
        }
        Case::Escaped => {
            let pointer = block.operations[0].results[0].ty.clone();
            block.operations.insert(
                3,
                Operation::new(
                    vec![],
                    Kind::Call {
                        callee: "g".into(),
                        arguments: vec![ValueId(2)],
                    },
                ),
            );
            let mut callee = BasicBlock::new(BlockId(14));
            callee.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::internal_helper(
                "g",
                Signature::new(vec![pointer], vec![]),
                vec![ValueId(99)],
                vec![callee],
            ));
        }
    }
    // Stored function ordinal 1 and block ordinal 0 are not function name f
    // or sparse BlockId(73). The golden and hostile test distinguish them.
    let mut prefix = BasicBlock::new(BlockId(91));
    prefix.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.insert(
        0,
        Function::internal_helper("a", Signature::new(vec![], vec![]), vec![], vec![prefix]),
    );
    module
}

fn coord(coordinate: Coordinate) -> String {
    format!(
        "f{}:b{}:o{}",
        coordinate.block.function.0, coordinate.block.block, coordinate.operation
    )
}

fn at(module: &Module, coordinate: Coordinate) -> &Operation {
    &module.functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize]
}

fn values(values: &[ValueId]) -> String {
    values
        .iter()
        .map(|value| format!("v{}", value.0))
        .collect::<Vec<_>>()
        .join(",")
}

// A deliberately closed test observation, not a general IR printer. Fail when
// the fixture grows outside this grammar; never silently omit an opcode.
fn describe(operation: &Operation) -> String {
    let result = values(
        &operation
            .results
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>(),
    );
    let kind = match &operation.kind {
        Kind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } => {
            assert_eq!(element, &Type::Scalar(ScalarType::U32));
            assert!(count.is_none());
            assert_eq!(
                operation.results[0].ty,
                Type::pointer(element.clone(), *address_space, AccessMode::ReadWrite)
            );
            format!("Alloca u32 {address_space:?} count=none align={alignment}")
        }
        Kind::Store {
            pointer,
            value,
            access,
        } => format!(
            "Store v{} <- v{} {:?} align={} volatile={}",
            pointer.0, value.0, access.address_space, access.alignment, access.volatile
        ),
        Kind::Load { pointer, access } => {
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
            format!(
                "Load v{} {:?} align={} volatile={}",
                pointer.0, access.address_space, access.alignment, access.volatile
            )
        }
        Kind::Binary {
            op: BinaryOp::BitOr,
            lhs,
            rhs,
        } => {
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
            format!("Binary BitOr v{},v{}", lhs.0, rhs.0)
        }
        Kind::Call { callee, arguments } => {
            format!("Call {}({})", callee.as_str(), values(arguments))
        }
        other => panic!("outside golden fixture grammar: {other:?}"),
    };
    format!("[{result}] {kind}")
}

fn observe_module(text: &mut String, prefix: &str, module: &Module) {
    writeln!(
        text,
        "{prefix}: module {} kernels={}",
        module.id.as_str(),
        module.kernels.len()
    )
    .unwrap();
    for (function_index, function) in module.functions.iter().enumerate() {
        writeln!(
            text,
            "{prefix}: f{function_index} {} {:?}",
            function.id.as_str(),
            function.role
        )
        .unwrap();
        let body = function.body.as_ref().unwrap();
        for (block_index, block) in body.blocks.iter().enumerate() {
            writeln!(
                text,
                "{prefix}: f{function_index}:b{block_index} block-id={}",
                block.id.0
            )
            .unwrap();
            for (operation_index, operation) in block.operations.iter().enumerate() {
                writeln!(
                    text,
                    "{prefix}: f{function_index}:b{block_index}:o{operation_index} {}",
                    describe(operation)
                )
                .unwrap();
            }
            let Some(Terminator::Return { values: returned }) = &block.terminator else {
                panic!("outside golden terminator grammar")
            };
            writeln!(
                text,
                "{prefix}: f{function_index}:b{block_index}:term Return [{}]",
                values(returned)
            )
            .unwrap();
        }
    }
}

struct Observation {
    text: String,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
}

fn observe(case: Case) -> Observation {
    let mut observation = None;
    with_owner(input(case), |encoded, budget| {
        let floor = budget.storage();
        let (decoded, storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
            encoded.canonical().canonical_bytes(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(decoded.module(), encoded.module());
        assert_eq!(
            decoded.canonical().identity(),
            encoded.canonical().identity()
        );
        let transformed = optimize_checked_load_forwarding_v1(&decoded, budget).unwrap();
        budget
            .reserve_storage(transformed.retained_storage())
            .unwrap();
        let checked_storage = {
            let (checked, checked_storage) = transformed.replay(budget).unwrap();
            budget
                .reserve_storage(checked_storage.retained_storage())
                .unwrap();
            assert!(std::ptr::eq(checked.input(), &decoded));
            assert!(std::ptr::eq(checked.output(), transformed.output()));
            assert_eq!(checked.rows(), transformed.rows());
            assert!(!transformed.grants_authority());
            assert!(!checked.grants_authority());
            let mut text = format!("CASE: {}\n", case.name());
            observe_module(&mut text, "CHECK-BEFORE", checked.input().module());
            observe_module(&mut text, "CHECK-AFTER", checked.output().module());
            let changed = checked.input().canonical().canonical_bytes()
                != checked.output().canonical().canonical_bytes();
            writeln!(
                text,
                "CHECK-REMARK: rows={} changed={changed} independently-checked=true authority={}",
                checked.rows().len(),
                checked.grants_authority()
            )
            .unwrap();
            for row in checked.rows() {
                let first = at(checked.input().module(), row.first);
                let replacement = at(checked.output().module(), row.load);
                assert_eq!(
                    replacement.results,
                    at(checked.input().module(), row.load).results
                );
                let value = first.results[0].id;
                assert_eq!(
                    replacement.kind,
                    Kind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: value,
                        rhs: value
                    }
                );
                assert_eq!(at(checked.output().module(), row.first), first);
                writeln!(
                    text,
                    "CHECK-REMARK: replace {} Load with Binary BitOr v{},v{}; retain {} Load",
                    coord(row.load),
                    value.0,
                    value.0,
                    coord(row.first)
                )
                .unwrap();
            }
            if case == Case::TwoLoads {
                assert_eq!(checked.rows().len(), 1);
                assert_eq!(
                    checked.rows()[0].first,
                    Coordinate {
                        block: Block {
                            function: FunctionCoordinate(1),
                            block: 0
                        },
                        operation: 3
                    }
                );
                assert_eq!(
                    checked.rows()[0].load,
                    Coordinate {
                        block: Block {
                            function: FunctionCoordinate(1),
                            block: 0
                        },
                        operation: 4
                    }
                );
            } else {
                assert!(checked.rows().is_empty());
                assert!(!changed);
                assert_eq!(
                    checked.input().canonical().identity(),
                    checked.output().canonical().identity()
                );
            }
            // Diagnostic copies belong to the test harness, not a compiler receipt.
            observation = Some(Observation {
                text,
                input_bytes: checked.input().canonical().canonical_bytes().to_vec(),
                output_bytes: checked.output().canonical().canonical_bytes().to_vec(),
            });
            checked_storage
        };
        budget
            .release_storage(checked_storage.retained_storage())
            .unwrap();
        let retained = transformed.retained_storage();
        drop(transformed);
        budget.release_storage(retained).unwrap();
        drop(decoded);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
    observation.unwrap()
}

#[test]
fn canonical_load_forwarding_observation_goldens() {
    for case in CASES {
        assert_eq!(observe(case).text, case.golden(), "{}", case.name());
    }
}

#[test]
fn recorded_coordinates_do_not_accept_raw_block_id_or_another_function() {
    with_owner(input(Case::TwoLoads), |input, budget| {
        let transformed = optimize_checked_load_forwarding_v1(input, budget).unwrap();
        budget
            .reserve_storage(transformed.retained_storage())
            .unwrap();
        let floor = budget.storage();
        for mutation in 0..3 {
            let mut rows = transformed.rows().to_vec();
            match mutation {
                0 => rows[0].load.block.block = 73,
                1 => rows[0].first.block.function = FunctionCoordinate(0),
                2 => rows[0].load.operation = 3,
                _ => unreachable!(),
            }
            assert!(
                check_canonical_kir_load_forwarding_v1(input, transformed.output(), &rows, budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
        let retained = transformed.retained_storage();
        drop(transformed);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn safe_goldens_are_also_consumed_by_the_fixed_policy5_schedule() {
    for case in [Case::TwoLoads, Case::GlobalStoreBarrier] {
        with_owner(input(case), |input, budget| {
            let checked =
                crate::optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
            budget.reserve_storage(checked.retained_storage()).unwrap();
            assert_eq!(checked.execution().policy_version(), 5);
            assert!(checked.intermediate_policy4().forwarding_rows().is_empty());
            assert_eq!(
                checked.load_forwarding_rows().len(),
                usize::from(case == Case::TwoLoads)
            );
            checked.replay(input, budget).unwrap();
            assert!(!checked.grants_authority());
            let retained = checked.retained_storage();
            drop(checked);
            budget.release_storage(retained).unwrap();
        });
    }
}

const CHILD: &str = "FE2O3_TEST_LOAD_GOLDEN_CHILD";
const BEGIN: &str = "FE2O3_LOAD_GOLDEN_BEGIN\n";
const END: &str = "FE2O3_LOAD_GOLDEN_END";

#[test]
#[ignore = "subprocess helper: exact canonical-byte and observation determinism"]
fn canonical_load_golden_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let mut transcript = String::new();
    for case in CASES {
        let observed = observe(case);
        assert_eq!(observed.text, case.golden());
        transcript.push_str(&observed.text);
        for (label, bytes) in [
            ("INPUT", observed.input_bytes),
            ("OUTPUT", observed.output_bytes),
        ] {
            write!(transcript, "CANONICAL-{label}: ").unwrap();
            for byte in bytes {
                write!(transcript, "{byte:02x}").unwrap();
            }
            transcript.push('\n');
        }
    }
    assert!(transcript.len() <= 64 * 1024);
    println!("{BEGIN}{transcript}{END}");
}

#[test]
fn canonical_bytes_and_observation_goldens_repeat_across_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::canonical_load_golden_child");
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args(["--exact", &name, "--ignored", "--nocapture"])
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
        // Extract only our bounded test transcript, excluding libtest timing.
        let transcript = stdout
            .split_once(BEGIN)
            .unwrap()
            .1
            .split_once(END)
            .unwrap()
            .0;
        assert!(transcript.len() <= 64 * 1024);
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}
