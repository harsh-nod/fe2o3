//! Closed observation goldens, not a KIR text parser or production pass selector.
use super::*;
use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_canonical_kir_transition_v1};
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation as Diagnostic, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Coordinate,
};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Identity,
    LiveFlag,
    Neighbor,
    Trap,
    NoOp,
}
const CASES: [Case; 5] = [
    Case::Identity,
    Case::LiveFlag,
    Case::Neighbor,
    Case::Trap,
    Case::NoOp,
];

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::LiveFlag => "live-flag",
            Self::Neighbor => "nonidentity-neighbor",
            Self::Trap => "trap-origin",
            Self::NoOp => "no-op",
        }
    }
    fn golden(self) -> &'static str {
        match self {
            Self::Identity => include_str!("../tests/integer-continuation-golden/identity.golden"),
            Self::LiveFlag => include_str!("../tests/integer-continuation-golden/live-flag.golden"),
            Self::Neighbor => {
                include_str!("../tests/integer-continuation-golden/nonidentity-neighbor.golden")
            }
            Self::Trap => include_str!("../tests/integer-continuation-golden/trap-origin.golden"),
            Self::NoOp => include_str!("../tests/integer-continuation-golden/no-op.golden"),
        }
    }
}

fn input(case: Case) -> Module {
    let mut module = identity(
        Constant::U32(u32::from(case == Case::NoOp)),
        matches!(case, Case::LiveFlag | Case::NoOp),
    );
    let function = &mut module.functions[0];
    let body = function.body.as_mut().unwrap();
    match case {
        Case::Neighbor => {
            body.blocks[0].operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
                    Kind::Constant(Constant::U32(3)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
                    Kind::Binary {
                        op: BinaryOp::BitXor,
                        lhs: ValueId(5),
                        rhs: ValueId(6),
                    },
                ),
            ]);
            body.blocks[0].terminator = Some(Terminator::Return {
                values: vec![ValueId(7)],
            });
        }
        Case::Trap => {
            function.signature.parameters.push(Type::BOOL);
            body.parameters.push(ValueId(6));
            let returned = body.blocks[0].terminator.take();
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(6),
                then_target: BlockId(18),
                then_arguments: vec![],
                else_target: BlockId(19),
                else_arguments: vec![],
            });
            let mut trap = BasicBlock::new(BlockId(18));
            trap.operations.push(Diagnostic::Trap.operation(None));
            trap.terminator = Some(Terminator::Unreachable);
            let mut exit = BasicBlock::new(BlockId(19));
            exit.terminator = returned;
            body.blocks.extend([trap, exit]);
            function.required_capabilities = Diagnostic::Trap.required_capabilities();
            module.functions.push(Diagnostic::Trap.declaration());
        }
        _ => {}
    }
    module
}

fn types(types: impl IntoIterator<Item = Type>) -> String {
    types
        .into_iter()
        .map(|ty| match ty {
            Type::Scalar(ScalarType::U32) => "u32",
            Type::Scalar(ScalarType::Bool) => "bool",
            other => panic!("outside golden type grammar: {other:?}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn value(function: &Function, id: ValueId) -> String {
    let body = function.body.as_ref().unwrap();
    if let Some(index) = body.parameters.iter().position(|value| *value == id) {
        return format!("arg{index}");
    }
    for (block, contents) in body.blocks.iter().enumerate() {
        assert!(contents.parameters.is_empty());
        for (operation, contents) in contents.operations.iter().enumerate() {
            if let Some(result) = contents.results.iter().position(|value| value.id == id) {
                return format!("b{block}:o{operation}:r{result}");
            }
        }
    }
    panic!("golden use has no exact definition: {id:?}");
}

fn coord(module: &Module, coordinate: Coordinate) -> String {
    format!(
        "{}:b{}:o{}",
        module.functions[coordinate.block.function.0 as usize]
            .id
            .as_str(),
        coordinate.block.block,
        coordinate.operation
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

fn observe_module(text: &mut String, prefix: &str, module: &Module) {
    assert!(module.kernels.is_empty());
    let functions = module
        .functions
        .iter()
        .filter(|f| f.id.as_str() == "f")
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("exact fixture function")
    };
    let declarations = module
        .functions
        .iter()
        .filter(|f| f.id.as_str() != "f")
        .collect::<Vec<_>>();
    assert!(declarations.len() <= 1);
    if let Some(declaration) = declarations.first() {
        assert_eq!(**declaration, Diagnostic::Trap.declaration());
    }
    writeln!(
        text,
        "{prefix}: function f parameters=[{}] results=[{}] trap-declarations={}",
        types(function.signature.parameters.clone()),
        types(function.signature.results.clone()),
        declarations.len()
    )
    .unwrap();
    let body = function.body.as_ref().unwrap();
    let block_index = |id| body.blocks.iter().position(|block| block.id == id).unwrap();
    for (block, contents) in body.blocks.iter().enumerate() {
        assert!(contents.parameters.is_empty());
        for (operation, contents) in contents.operations.iter().enumerate() {
            let description = match &contents.kind {
                Kind::Constant(Constant::U32(value)) => format!("Constant U32({value})"),
                Kind::Constant(Constant::Bool(value)) => format!("Constant Bool({value})"),
                Kind::Binary { op, lhs, rhs }
                    if matches!(
                        op,
                        BinaryOp::Checked(CheckedBinaryOperator::Add)
                            | BinaryOp::BitOr
                            | BinaryOp::BitXor
                    ) =>
                {
                    format!(
                        "Binary {op:?} {},{}",
                        value(function, *lhs),
                        value(function, *rhs)
                    )
                }
                _ if contents == &Diagnostic::Trap.operation(None) => "Trap".into(),
                other => panic!("outside golden operation grammar: {other:?}"),
            };
            writeln!(
                text,
                "{prefix}: f:b{block}:o{operation} {description} results=[{}]",
                types(contents.results.iter().map(|result| result.ty.clone()))
            )
            .unwrap();
        }
        let description = match contents.terminator.as_ref().unwrap() {
            Terminator::Return { values } => format!(
                "Return [{}]",
                values
                    .iter()
                    .map(|id| value(function, *id))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                assert!(then_arguments.is_empty() && else_arguments.is_empty());
                format!(
                    "ConditionalBranch {} b{},b{}",
                    value(function, *condition),
                    block_index(*then_target),
                    block_index(*else_target)
                )
            }
            Terminator::Unreachable => "Unreachable".into(),
            other => panic!("outside golden terminator grammar: {other:?}"),
        };
        writeln!(text, "{prefix}: f:b{block}:term {description}").unwrap();
    }
}

struct Observation {
    text: String,
    bytes: Vec<(&'static str, Vec<u8>)>,
    occurrences: String,
}

fn observe(case: Case) -> Observation {
    let mut observation = None;
    with_owner(input(case), |encoded, budget| {
        let floor = budget.storage();
        let (decoded, receipt) = Owner::from_canonical_bytes_with_verification_budget_v12(
            encoded.canonical().canonical_bytes(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(decoded.module(), encoded.module());
        let checked = finish(&decoded, budget);
        let live = budget.storage();
        checked.replay(&decoded, budget).unwrap();
        assert_eq!(budget.storage(), live);
        assert_roster(checked.continuation());
        let before = checked.intermediate_policy5().owner();
        let after = checked.owner();
        let changed = before.canonical().canonical_bytes() != after.canonical().canonical_bytes();
        assert_eq!(changed, case != Case::NoOp);
        assert_eq!(
            binary_count(before),
            if case == Case::Neighbor { 3 } else { 2 }
        );
        assert_eq!(
            binary_count(after),
            match case {
                Case::Neighbor => 1,
                Case::NoOp => 2,
                _ => 0,
            }
        );
        let mut text = format!("CASE: {}\n", case.name());
        observe_module(&mut text, "CHECK-BEFORE", before.module());
        observe_module(&mut text, "CHECK-AFTER", after.module());
        let passes = checked.continuation().report().passes();
        writeln!(
            text,
            "CHECK-REMARK: passes={},{} changed={},{} independently-checked=true authority=false",
            passes[0].pass().name(),
            passes[1].pass().name(),
            passes[0].changed(),
            passes[1].changed()
        )
        .unwrap();
        assert!(!checked.grants_authority());
        let rows = checked.continuation().occurrences().candidate();
        for row in rows.operations {
            let origin = match row.origin {
                Origin::Retained(source) => format!("retained {}", coord(before.module(), source)),
                Origin::ConstantFrom(Definition::Result { operation, result }) => format!(
                    "constant-from {}:r{result}",
                    coord(before.module(), operation)
                ),
                other => panic!("outside golden origin grammar: {other:?}"),
            };
            writeln!(
                text,
                "CHECK-REMARK: {} {origin}",
                coord(after.module(), row.output)
            )
            .unwrap();
        }
        if case == Case::Trap {
            let (flat, trap) = rows
                .operations
                .iter()
                .enumerate()
                .find(|(_, row)| {
                    at(after.module(), row.output) == &Diagnostic::Trap.operation(None)
                })
                .unwrap();
            let Origin::Retained(source) = trap.origin else {
                panic!("ordered trap lost original occurrence")
            };
            assert_eq!(at(before.module(), source), at(after.module(), trap.output));
            let old_flat = before
                .module()
                .functions
                .iter()
                .filter_map(|function| function.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .position(|operation| operation == &Diagnostic::Trap.operation(None))
                .unwrap();
            assert_ne!(old_flat, flat);
            writeln!(
                text,
                "CHECK-REMARK: trap-flat={old_flat}->{flat} exact-retained-origin=true"
            )
            .unwrap();
        }
        // These bounded diagnostic copies are test output, not compiler receipts.
        observation = Some(Observation {
            text,
            bytes: vec![
                ("B", decoded.canonical().canonical_bytes().to_vec()),
                ("O", before.canonical().canonical_bytes().to_vec()),
                ("I", after.canonical().canonical_bytes().to_vec()),
                (
                    "POLICY5",
                    checked
                        .intermediate_policy5()
                        .execution()
                        .canonical_bytes()
                        .to_vec(),
                ),
                ("POLICY6", checked.execution().canonical_bytes().to_vec()),
                (
                    "CONTINUATION",
                    checked
                        .continuation()
                        .execution()
                        .canonical_bytes()
                        .to_vec(),
                ),
                ("MAP-DIGEST", checked.continuation().map().digest().to_vec()),
            ],
            occurrences: format!("{rows:?}"),
        });
        let retained = checked.retained_storage();
        drop(checked);
        budget.release_storage(retained).unwrap();
        drop(decoded);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
    observation.unwrap()
}

#[test]
fn fixed_policy6_integer_continuation_observation_goldens() {
    for case in CASES {
        assert_eq!(observe(case).text, case.golden(), "{}", case.name());
    }
}

#[test]
fn trap_origin_is_not_the_final_flattened_operation_ordinal() {
    with_owner(input(Case::Trap), |input, budget| {
        let checked = finish(input, budget);
        scoped(budget, |budget| {
            let (before, before_storage) =
                CanonicalKirInventoryV1::derive(checked.intermediate_policy5().owner(), budget)
                    .unwrap();
            budget.reserve_storage(before_storage.retained_storage())?;
            let (after, after_storage) =
                CanonicalKirInventoryV1::derive(checked.owner(), budget).unwrap();
            budget.reserve_storage(after_storage.retained_storage())?;
            let (flat, trap) = after
                .operations()
                .iter()
                .enumerate()
                .find(|(_, row)| row.operation == &Diagnostic::Trap.operation(None))
                .unwrap();
            let mut operations = checked
                .continuation()
                .occurrences()
                .candidate()
                .operations
                .to_vec();
            assert_eq!(operations[flat].output, trap.coordinate);
            assert_ne!(before.operations()[flat].operation, trap.operation);
            operations[flat].origin = Origin::Retained(before.operations()[flat].coordinate);
            let mut hostile = checked.continuation().occurrences().candidate();
            hostile.operations = &operations;
            assert!(check_canonical_kir_transition_v1(&before, &after, hostile, budget).is_err());
            Ok(())
        })
        .unwrap();
        checked.replay(input, budget).unwrap();
        let retained = checked.retained_storage();
        drop(checked);
        budget.release_storage(retained).unwrap();
    });
}

const CHILD: &str = "FE2O3_TEST_INTEGER_GOLDEN_CHILD";
const BEGIN: &str = "FE2O3_INTEGER_GOLDEN_BEGIN\n";
const END: &str = "FE2O3_INTEGER_GOLDEN_END";
const TRANSCRIPT_LIMIT: usize = 128 * 1024;

#[test]
#[ignore = "subprocess helper: exact canonical-byte, witness and observation determinism"]
fn canonical_integer_golden_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let mut transcript = String::new();
    for case in CASES {
        let observed = observe(case);
        assert_eq!(observed.text, case.golden());
        transcript.push_str(&observed.text);
        for (label, bytes) in observed.bytes {
            write!(transcript, "CANONICAL-{label}: ").unwrap();
            for byte in bytes {
                write!(transcript, "{byte:02x}").unwrap();
            }
            transcript.push('\n');
        }
        writeln!(transcript, "OCCURRENCES: {}", observed.occurrences).unwrap();
    }
    assert!(transcript.len() <= TRANSCRIPT_LIMIT);
    println!("{BEGIN}{transcript}{END}");
}

#[test]
fn canonical_bytes_witnesses_and_goldens_repeat_across_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::canonical_integer_golden_child");
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
        assert_eq!(stdout.matches(BEGIN).count(), 1);
        assert_eq!(stdout.matches(END).count(), 1);
        let transcript = stdout
            .split_once(BEGIN)
            .unwrap()
            .1
            .split_once(END)
            .unwrap()
            .0;
        assert!(transcript.len() <= TRANSCRIPT_LIMIT);
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}
