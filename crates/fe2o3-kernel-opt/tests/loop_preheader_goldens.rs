//! Closed observations of the actual owning preheader component. Canonical V12
//! admission owns the IR; these goldens are not an input parser or source,
//! backend, numbered-policy, proof, runtime or default-pipeline admission.
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, Constant, Function, IntegerSwitchCase,
    Module, Operation, OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::prepare_owned_loop_preheaders_v1;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    DuplicateConditional,
    DuplicateIntegerSwitch,
    Nested,
    ExistingPreheader,
}
const CASES: [Case; 4] = [
    Case::DuplicateConditional,
    Case::DuplicateIntegerSwitch,
    Case::Nested,
    Case::ExistingPreheader,
];
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::DuplicateConditional => "duplicate-conditional",
            Self::DuplicateIntegerSwitch => "duplicate-integer-switch",
            Self::Nested => "nested-loops",
            Self::ExistingPreheader => "existing-preheader",
        }
    }
    fn golden(self) -> &'static str {
        match self {
            Self::DuplicateConditional => {
                include_str!("loop-preheader-golden/duplicate-conditional.golden")
            }
            Self::DuplicateIntegerSwitch => {
                include_str!("loop-preheader-golden/duplicate-integer-switch.golden")
            }
            Self::Nested => include_str!("loop-preheader-golden/nested-loops.golden"),
            Self::ExistingPreheader => {
                include_str!("loop-preheader-golden/existing-preheader.golden")
            }
        }
    }
    fn rows(self) -> &'static [(u32, u32)] {
        match self {
            Self::DuplicateConditional | Self::DuplicateIntegerSwitch => &[(1, 4)],
            Self::Nested => &[(1, 6), (2, 7)],
            Self::ExistingPreheader => &[],
        }
    }
}

fn branch(target: u32, arguments: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().copied().map(ValueId).collect(),
    }
}
fn conditional(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, term: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(term);
    block
}
fn helper(parameters: Vec<Type>, results: Vec<Type>, blocks: Vec<BasicBlock>) -> Module {
    let ids = (0..u32::try_from(parameters.len()).unwrap())
        .map(ValueId)
        .collect();
    let mut module = Module::new("loop-preheader-golden-subject");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, results),
        ids,
        blocks,
    ));
    module
}

// Expected subjects are prescribed from fixture constants, never output rows,
// loop discovery, optimizer callbacks or a replacement textual IR parser.
fn fixture(case: Case) -> (Module, Module) {
    if case == Case::Nested {
        let input = helper(
            vec![Type::BOOL],
            vec![],
            vec![
                block(10, conditional(0, 20, 20)),
                block(20, conditional(0, 30, 30)),
                block(30, conditional(0, 40, 50)),
                block(40, branch(30, &[])),
                block(50, conditional(0, 20, 60)),
                block(60, Terminator::Return { values: vec![] }),
            ],
        );
        let mut expected = input.clone();
        let blocks = &mut expected.functions[0].body.as_mut().unwrap().blocks;
        blocks[0].terminator = Some(conditional(0, 61, 61));
        blocks[1].terminator = Some(conditional(0, 62, 62));
        blocks.extend([block(61, branch(20, &[])), block(62, branch(30, &[]))]);
        return (input, expected);
    }
    let incoming = |target| match case {
        Case::DuplicateConditional => Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(target),
            then_arguments: vec![ValueId(1), ValueId(2)],
            else_target: BlockId(target),
            else_arguments: vec![ValueId(2), ValueId(1)],
        },
        Case::DuplicateIntegerSwitch => Terminator::IntegerSwitch {
            selector: ValueId(1),
            cases: vec![
                IntegerSwitchCase {
                    value: Constant::U32(0),
                    target: BlockId(target),
                    arguments: vec![ValueId(1), ValueId(2)],
                },
                IntegerSwitchCase {
                    value: Constant::U32(1),
                    target: BlockId(target),
                    arguments: vec![ValueId(2), ValueId(1)],
                },
            ],
            default_target: BlockId(target),
            default_arguments: vec![ValueId(1), ValueId(2)],
        },
        Case::ExistingPreheader => branch(target, &[1, 2]),
        Case::Nested => unreachable!(),
    };
    let u32_type = Type::Scalar(ScalarType::U32);
    let mut entry = block(10, incoming(50));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(100), u32_type.clone()),
        Kind::Constant(Constant::U32(1)),
    ));
    let mut header = block(50, conditional(202, 80, 90));
    header.parameters = vec![
        ValueDef::new(ValueId(200), u32_type.clone()),
        ValueDef::new(ValueId(201), u32_type.clone()),
    ];
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(202), Type::BOOL),
        Kind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(200),
            rhs: ValueId(3),
        },
    ));
    let mut latch = block(80, branch(50, &[203, 201]));
    latch.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(203), u32_type.clone()),
        Kind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(200),
            rhs: ValueId(100),
        },
    ));
    let exit = block(
        90,
        Terminator::Return {
            values: vec![ValueId(200), ValueId(201)],
        },
    );
    let input = helper(
        vec![
            Type::BOOL,
            u32_type.clone(),
            u32_type.clone(),
            u32_type.clone(),
        ],
        vec![u32_type.clone(), u32_type.clone()],
        vec![entry, header, latch, exit],
    );
    let mut expected = input.clone();
    if case != Case::ExistingPreheader {
        let blocks = &mut expected.functions[0].body.as_mut().unwrap().blocks;
        blocks[0].terminator = Some(incoming(91));
        let mut forwarding = block(91, branch(50, &[204, 205]));
        forwarding.parameters = vec![
            ValueDef::new(ValueId(204), u32_type.clone()),
            ValueDef::new(ValueId(205), u32_type),
        ];
        blocks.push(forwarding);
    }
    (input, expected)
}

fn values(values: &[ValueId]) -> String {
    values
        .iter()
        .map(|id| format!("v{}", id.0))
        .collect::<Vec<_>>()
        .join(",")
}
fn ty(ty: &Type) -> &'static str {
    match ty {
        Type::Scalar(ScalarType::U32) => "U32",
        Type::Scalar(ScalarType::Bool) => "Bool",
        other => panic!("outside closed golden type grammar: {other:?}"),
    }
}
fn definitions(definitions: &[ValueDef]) -> String {
    definitions
        .iter()
        .map(|value| format!("v{}:{}", value.id.0, ty(&value.ty)))
        .collect::<Vec<_>>()
        .join(",")
}
fn observe_module(text: &mut String, prefix: &str, module: &Module) {
    assert!(module.kernels.is_empty());
    assert_eq!(module.functions.len(), 1);
    writeln!(text, "{prefix}: module {} kernels=0", module.id.as_str()).unwrap();
    let function = &module.functions[0];
    let body = function.body.as_ref().unwrap();
    let parameters = body
        .parameters
        .iter()
        .zip(&function.signature.parameters)
        .map(|(id, t)| format!("v{}:{}", id.0, ty(t)))
        .collect::<Vec<_>>()
        .join(",");
    let results = function
        .signature
        .results
        .iter()
        .map(ty)
        .collect::<Vec<_>>()
        .join(",");
    writeln!(
        text,
        "{prefix}: f0 {} params=[{parameters}] results=[{results}]",
        function.id.as_str()
    )
    .unwrap();
    for (bi, block) in body.blocks.iter().enumerate() {
        writeln!(
            text,
            "{prefix}: f0:b{bi} block-id={} params=[{}]",
            block.id.0,
            definitions(&block.parameters)
        )
        .unwrap();
        for (oi, operation) in block.operations.iter().enumerate() {
            let kind = match &operation.kind {
                Kind::Constant(Constant::U32(value)) => format!("Constant U32({value})"),
                Kind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs,
                    rhs,
                } => format!("LessThan v{},v{}", lhs.0, rhs.0),
                Kind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                } => format!("Add v{},v{}", lhs.0, rhs.0),
                other => panic!("outside closed golden operation grammar: {other:?}"),
            };
            writeln!(
                text,
                "{prefix}: f0:b{bi}:o{oi} [{}] {kind}",
                definitions(&operation.results)
            )
            .unwrap();
        }
        let term = match block.terminator.as_ref().unwrap() {
            Terminator::Return { values: returned } => format!("Return [{}]", values(returned)),
            Terminator::Branch { target, arguments } => {
                format!("Branch block-id={} [{}]", target.0, values(arguments))
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => format!(
                "Conditional v{} then-id={} [{}] else-id={} [{}]",
                condition.0,
                then_target.0,
                values(then_arguments),
                else_target.0,
                values(else_arguments)
            ),
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                let cases = cases
                    .iter()
                    .map(|case| {
                        let Constant::U32(value) = &case.value else {
                            panic!("outside closed golden switch grammar")
                        };
                        format!(
                            "U32({value})->{} [{}]",
                            case.target.0,
                            values(&case.arguments)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                format!(
                    "IntegerSwitch v{} cases=[{cases}] default-id={} [{}]",
                    selector.0,
                    default_target.0,
                    values(default_arguments)
                )
            }
            other => panic!("outside closed golden terminator grammar: {other:?}"),
        };
        writeln!(text, "{prefix}: f0:b{bi}:term {term}").unwrap();
    }
}

struct Observation {
    text: String,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
}
fn observe(case: Case) -> Observation {
    let (input, expected) = fixture(case);
    let mut work = Work::new(500_000_000);
    let mut budget = Budget::new(&mut work, 128 << 20);
    let ledger = budget.work_ledger_identity_v1();
    let sibling = 43;
    budget.reserve_storage(sibling).unwrap();
    let (encoded, encoded_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&input, &mut budget).unwrap();
    budget
        .reserve_storage(encoded_storage.retained_storage())
        .unwrap();
    let (expected, expected_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&expected, &mut budget).unwrap();
    budget
        .reserve_storage(expected_storage.retained_storage())
        .unwrap();
    let (decoded, decoded_storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
        encoded.canonical().canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(decoded_storage.retained_storage())
        .unwrap();
    assert_eq!(decoded.module(), encoded.module());
    assert_eq!(
        decoded.canonical().identity(),
        encoded.canonical().identity()
    );
    let input_floor = budget.storage();
    let actual = prepare_owned_loop_preheaders_v1(&decoded, &mut budget).unwrap();
    assert_eq!(budget.storage(), input_floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.reserve_storage(actual.retained_storage()).unwrap();
    let output_floor = budget.storage();
    assert_eq!(actual.input_identity(), decoded.canonical().identity());
    assert_eq!(actual.output().module(), expected.module());
    assert_eq!(
        actual.output().canonical().canonical_bytes(),
        expected.canonical().canonical_bytes()
    );
    assert_eq!(actual.preheaders().len(), case.rows().len());
    for (row, &(header, preheader)) in actual.preheaders().iter().zip(case.rows()) {
        assert_eq!(row.header.function.0, 0);
        assert_eq!(row.header.block, header);
        assert_eq!(row.preheader.function.0, 0);
        assert_eq!(row.preheader.block, preheader);
    }
    assert!(!actual.grants_authority());
    let changed =
        actual.output().canonical().canonical_bytes() != decoded.canonical().canonical_bytes();
    assert_eq!(changed, !case.rows().is_empty());
    let pair_retained = {
        let (pair, pair_storage) = actual.replay_against(&decoded, &mut budget).unwrap();
        assert_eq!(budget.storage(), output_floor);
        budget
            .reserve_storage(pair_storage.retained_storage())
            .unwrap();
        assert!(std::ptr::eq(pair.input(), &decoded));
        assert!(std::ptr::eq(pair.output(), actual.output()));
        assert_eq!(pair.preheaders(), actual.preheaders());
        assert!(!pair.grants_authority());
        pair_storage.retained_storage()
    };
    budget.release_storage(pair_retained).unwrap();

    let repeated = prepare_owned_loop_preheaders_v1(actual.output(), &mut budget).unwrap();
    assert_eq!(budget.storage(), output_floor);
    budget.reserve_storage(repeated.retained_storage()).unwrap();
    assert!(repeated.preheaders().is_empty());
    assert_eq!(repeated.output().module(), actual.output().module());
    assert_eq!(
        repeated.output().canonical().canonical_bytes(),
        actual.output().canonical().canonical_bytes()
    );
    assert!(!repeated.grants_authority());
    let repeated_storage = repeated.retained_storage();
    drop(repeated);
    budget.release_storage(repeated_storage).unwrap();
    assert_eq!(budget.storage(), output_floor);

    // Observation strings are test harness allocations, not verifier scratch.
    // Complete actual owners and their row capacities remain reserved here.
    let mut text = format!("CASE: {}\n", case.name());
    observe_module(&mut text, "CHECK-BEFORE", decoded.module());
    observe_module(&mut text, "CHECK-AFTER", actual.output().module());
    writeln!(
        text,
        "CHECK-REMARK: preheaders={} changed={changed} independently-checked=true idempotent=true authority=false",
        actual.preheaders().len()
    )
    .unwrap();
    for row in actual.preheaders() {
        writeln!(
            text,
            "CHECK-REMARK: f0:b{} preheader f0:b{}",
            row.header.block, row.preheader.block
        )
        .unwrap();
    }
    let observed = Observation {
        text,
        input_bytes: decoded.canonical().canonical_bytes().to_vec(),
        output_bytes: actual.output().canonical().canonical_bytes().to_vec(),
    };
    let actual_storage = actual.retained_storage();
    drop(actual);
    budget.release_storage(actual_storage).unwrap();
    drop(decoded);
    budget
        .release_storage(decoded_storage.retained_storage())
        .unwrap();
    drop(expected);
    budget
        .release_storage(expected_storage.retained_storage())
        .unwrap();
    drop(encoded);
    budget
        .release_storage(encoded_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), sibling);
    assert!(budget.work_ledger_identity_v1() == ledger);
    observed
}

#[test]
fn actual_owning_loop_preheader_observation_goldens() {
    for case in CASES {
        assert_eq!(observe(case).text, case.golden(), "{}", case.name());
    }
}

const CHILD: &str = "FE2O3_TEST_PREHEADER_GOLDEN_CHILD";
const BEGIN: &str = "FE2O3_PREHEADER_GOLDEN_BEGIN\n";
const END: &str = "FE2O3_PREHEADER_GOLDEN_END";

#[test]
#[ignore = "subprocess helper: actual canonical-byte and observation determinism"]
fn canonical_loop_preheader_golden_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
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
    assert!(transcript.len() <= 256 * 1024);
    println!("{BEGIN}{transcript}{END}");
}

#[test]
fn canonical_loop_preheader_bytes_and_goldens_repeat_across_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args([
                "--exact",
                "canonical_loop_preheader_golden_child",
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
        assert_eq!(transcript.matches("CANONICAL-INPUT: ").count(), CASES.len());
        assert_eq!(
            transcript.matches("CANONICAL-OUTPUT: ").count(),
            CASES.len()
        );
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}
