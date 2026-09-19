//! Closed CHECK observations of the actual owning service, not a text IR parser.
//! Canonical V12 encode/decode owns both subjects. These component tests do not
//! activate a numbered policy, ordinary-source route, proof or runtime admission.
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind,
    CanonicalKirOperationCoordinateV1 as Coordinate, CanonicalKirOperationOriginV1 as Origin,
    Function, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::optimize_checked_commutative_bitwise_cse_v1;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    SameBlock,
    CrossBlock,
    ReverseChain,
    Siblings,
    OrderedAdd,
    TrapAndStore,
}
const CASES: [Case; 6] = [
    Case::SameBlock,
    Case::CrossBlock,
    Case::ReverseChain,
    Case::Siblings,
    Case::OrderedAdd,
    Case::TrapAndStore,
];
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::SameBlock => "same-block",
            Self::CrossBlock => "cross-block",
            Self::ReverseChain => "reverse-chain",
            Self::Siblings => "siblings",
            Self::OrderedAdd => "ordered-add",
            Self::TrapAndStore => "trap-and-store",
        }
    }
    fn golden(self) -> &'static str {
        match self {
            Self::SameBlock => include_str!("commutative-cse-golden/same-block.golden"),
            Self::CrossBlock => include_str!("commutative-cse-golden/cross-block.golden"),
            Self::ReverseChain => include_str!("commutative-cse-golden/reverse-chain.golden"),
            Self::Siblings => include_str!("commutative-cse-golden/siblings.golden"),
            Self::OrderedAdd => include_str!("commutative-cse-golden/ordered-add.golden"),
            Self::TrapAndStore => include_str!("commutative-cse-golden/trap-and-store.golden"),
        }
    }
    fn pairs(self) -> usize {
        match self {
            Self::SameBlock | Self::CrossBlock | Self::TrapAndStore => 1,
            Self::ReverseChain => 3,
            Self::Siblings | Self::OrderedAdd => 0,
        }
    }
}
fn expression(id: u32, ty: ScalarType, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ty)),
        Kind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn ret(block: &mut BasicBlock, value: u32) {
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(value)],
    });
}
fn branch(block: &mut BasicBlock, target: u32) {
    block.terminator = Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    });
}
fn module(ty: ScalarType, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("commutative-golden-subject");
    let ty = Type::Scalar(ty);
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        blocks,
    ));
    module
}

// Expected modules come from the closed fixture recipe, never candidate rows,
// an optimizer callback, output decoding or a replacement IR parser.
fn fixture(case: Case) -> (Module, Module) {
    if case == Case::ReverseChain {
        let mut entry = BasicBlock::new(BlockId(0));
        for (id, lhs) in [(10, 0), (11, 10), (12, 11)] {
            entry
                .operations
                .push(expression(id, ScalarType::U32, BinaryOp::BitXor, lhs, 1));
        }
        branch(&mut entry, 1);
        let mut input = vec![entry.clone()];
        let mut output = vec![entry];
        // Actual flow is 1 -> 2 -> 3, while physical order is 3, 2, 1.
        for (block_id, id, lhs) in [(3, 1002, 1001), (2, 1001, 1000), (1, 1000, 0)] {
            let mut block = BasicBlock::new(BlockId(block_id));
            block
                .operations
                .push(expression(id, ScalarType::U32, BinaryOp::BitXor, 1, lhs));
            if block_id == 3 {
                ret(&mut block, id);
            } else {
                branch(&mut block, block_id + 1);
            }
            input.push(block.clone());
            block.operations.clear();
            if block_id == 3 {
                ret(&mut block, 12);
            }
            output.push(block);
        }
        return (
            module(ScalarType::U32, input),
            module(ScalarType::U32, output),
        );
    }
    if case == Case::Siblings {
        let mut entry = BasicBlock::new(BlockId(10));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(20),
            then_arguments: vec![],
            else_target: BlockId(30),
            else_arguments: vec![],
        });
        let mut left = BasicBlock::new(BlockId(20));
        left.operations
            .push(expression(3, ScalarType::U32, BinaryOp::BitOr, 0, 1));
        ret(&mut left, 3);
        let mut right = BasicBlock::new(BlockId(30));
        right
            .operations
            .push(expression(4, ScalarType::U32, BinaryOp::BitOr, 1, 0));
        ret(&mut right, 4);
        let mut input = module(ScalarType::U32, vec![entry, left, right]);
        input.functions[0].signature.parameters.push(Type::BOOL);
        input.functions[0]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .push(ValueId(2));
        return (input.clone(), input);
    }
    let (ty, op) = match case {
        Case::CrossBlock => (ScalarType::I16, BinaryOp::BitOr),
        Case::OrderedAdd => (ScalarType::U32, BinaryOp::Add),
        Case::SameBlock | Case::TrapAndStore => (ScalarType::U32, BinaryOp::BitAnd),
        _ => unreachable!(),
    };
    let mut first = BasicBlock::new(BlockId(10));
    let lhs = if case == Case::TrapAndStore {
        first
            .operations
            .push(expression(5, ty, BinaryOp::Divide, 0, 1));
        5
    } else {
        0
    };
    first.operations.push(expression(2, ty, op, lhs, 1));
    let duplicate = expression(3, ty, op, 1, lhs);
    if case == Case::CrossBlock {
        branch(&mut first, 20);
        let mut second = BasicBlock::new(BlockId(20));
        second.operations.push(duplicate);
        ret(&mut second, 3);
        let input = module(ty, vec![first.clone(), second.clone()]);
        second.operations.clear();
        ret(&mut second, 2);
        return (input, module(ty, vec![first, second]));
    }
    first.operations.push(duplicate);
    ret(&mut first, 3);
    let mut input = module(ty, vec![first.clone()]);
    if case == Case::OrderedAdd {
        return (input.clone(), input);
    }
    first.operations.pop();
    ret(&mut first, 2);
    let mut output = module(ty, vec![first]);
    if case == Case::TrapAndStore {
        for (subject, value) in [(&mut input, 3), (&mut output, 2)] {
            subject.functions[0]
                .signature
                .parameters
                .push(Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ));
            let body = subject.functions[0].body.as_mut().unwrap();
            body.parameters.push(ValueId(4));
            body.blocks[0].operations.push(Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(4),
                    value: ValueId(value),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
        }
    }
    (input, output)
}

fn values(values: &[ValueId]) -> String {
    values
        .iter()
        .map(|value| format!("v{}", value.0))
        .collect::<Vec<_>>()
        .join(",")
}
fn ty(ty: &Type) -> String {
    match ty {
        Type::Scalar(ScalarType::U32) => "U32".into(),
        Type::Scalar(ScalarType::I16) => "I16".into(),
        Type::Scalar(ScalarType::Bool) => "Bool".into(),
        Type::Pointer(_) => {
            assert_eq!(
                ty,
                &Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            );
            "Ptr<U32,Global,ReadWrite>".into()
        }
        other => panic!("outside closed golden type grammar: {other:?}"),
    }
}
fn describe(operation: &Operation) -> String {
    let results = operation
        .results
        .iter()
        .map(|value| format!("v{}:{}", value.id.0, ty(&value.ty)))
        .collect::<Vec<_>>()
        .join(",");
    let kind = match &operation.kind {
        Kind::Binary { op, lhs, rhs } => {
            assert!(matches!(
                op,
                BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Add
                    | BinaryOp::Divide
            ));
            format!("{op:?} v{},v{}", lhs.0, rhs.0)
        }
        Kind::Store {
            pointer,
            value,
            access,
        } => format!(
            "Store v{} <- v{} {:?} align={} volatile={}",
            pointer.0, value.0, access.address_space, access.alignment, access.volatile
        ),
        other => panic!("outside closed golden operation grammar: {other:?}"),
    };
    format!("[{results}] {kind}")
}
fn observe_module(text: &mut String, prefix: &str, module: &Module) {
    assert!(module.kernels.is_empty());
    writeln!(text, "{prefix}: module {} kernels=0", module.id.as_str()).unwrap();
    for (fi, function) in module.functions.iter().enumerate() {
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
            "{prefix}: f{fi} {} params=[{parameters}] results=[{results}]",
            function.id.as_str()
        )
        .unwrap();
        for (bi, block) in body.blocks.iter().enumerate() {
            assert!(block.parameters.is_empty());
            writeln!(text, "{prefix}: f{fi}:b{bi} block-id={}", block.id.0).unwrap();
            for (oi, operation) in block.operations.iter().enumerate() {
                writeln!(text, "{prefix}: f{fi}:b{bi}:o{oi} {}", describe(operation)).unwrap();
            }
            let term = match block.terminator.as_ref().unwrap() {
                Terminator::Return { values: returned } => {
                    format!("Return [{}]", values(returned))
                }
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
                other => panic!("outside closed golden terminator grammar: {other:?}"),
            };
            writeln!(text, "{prefix}: f{fi}:b{bi}:term {term}").unwrap();
        }
    }
}
fn coord(c: Coordinate) -> String {
    format!(
        "f{}:b{}:o{}",
        c.block.function.0, c.block.block, c.operation
    )
}
fn definition(c: Definition) -> String {
    match c {
        Definition::Result { operation, result } => format!("{}:r{result}", coord(operation)),
        other => panic!("substitution outside closed golden result grammar: {other:?}"),
    }
}
struct Observation {
    text: String,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
}
fn observe(case: Case) -> Observation {
    let (input, expected) = fixture(case);
    let mut work = Work::new(1_000_000_000_000);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    let ledger = budget.work_ledger_identity_v1();
    let floor = 97;
    budget.reserve_storage(floor).unwrap();
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
    let actual = optimize_checked_commutative_bitwise_cse_v1(&decoded, &mut budget).unwrap();
    assert_eq!(budget.storage(), input_floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.reserve_storage(actual.retained_storage()).unwrap();
    let output_floor = budget.storage();
    assert!(std::ptr::eq(actual.input(), &decoded));
    assert_eq!(actual.owner().module(), expected.module());
    assert_eq!(
        actual.owner().canonical().canonical_bytes(),
        expected.canonical().canonical_bytes()
    );
    assert_eq!(actual.proved_pairs(), case.pairs());
    assert_eq!(actual.replay(&mut budget).unwrap(), case.pairs());
    assert_eq!(budget.storage(), output_floor);
    assert_eq!(actual.execution().changed(), case.pairs() != 0);
    assert!(!actual.grants_authority());
    // Human-readable observations are test harness allocations, not claimed
    // verifier scratch. The complete actual owners/rows stay reserved throughout.
    let mut text = format!("CASE: {}\n", case.name());
    observe_module(&mut text, "CHECK-BEFORE", actual.input().module());
    observe_module(&mut text, "CHECK-AFTER", actual.owner().module());
    writeln!(
        text,
        "CHECK-REMARK: proved={} changed={} independently-checked=true authority=false",
        actual.proved_pairs(),
        actual.execution().changed()
    )
    .unwrap();
    let rows = actual.occurrences().candidate();
    for row in rows.operations {
        let Origin::Retained(input) = row.origin else {
            panic!("commutative CSE synthesized an operation")
        };
        writeln!(
            text,
            "CHECK-REMARK: {} retained {}",
            coord(row.output),
            coord(input)
        )
        .unwrap();
    }
    let mut substituted = 0;
    for row in rows.definitions {
        assert_eq!(row.outputs.len, 1);
        let output = &rows.definition_outputs[row.outputs.start as usize];
        if output.kind == DescendantKind::Substituted {
            substituted += 1;
            writeln!(
                text,
                "CHECK-REMARK: {} substituted {}",
                definition(row.input),
                definition(output.output)
            )
            .unwrap();
        }
    }
    assert_eq!(substituted, case.pairs());
    let observed = Observation {
        text,
        input_bytes: actual.input().canonical().canonical_bytes().to_vec(),
        output_bytes: actual.owner().canonical().canonical_bytes().to_vec(),
    };
    let retained = actual.retained_storage();
    drop(actual);
    budget.release_storage(retained).unwrap();
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
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    observed
}

#[test]
fn actual_owning_commutative_cse_observation_goldens() {
    for case in CASES {
        assert_eq!(observe(case).text, case.golden(), "{}", case.name());
    }
}

const CHILD: &str = "FE2O3_TEST_COMMUTATIVE_GOLDEN_CHILD";
const BEGIN: &str = "FE2O3_COMMUTATIVE_GOLDEN_BEGIN\n";
const END: &str = "FE2O3_COMMUTATIVE_GOLDEN_END";

#[test]
#[ignore = "subprocess helper: actual canonical-byte and observation determinism"]
fn canonical_commutative_golden_child() {
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
fn canonical_bytes_and_checked_goldens_repeat_across_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args([
                "--exact",
                "canonical_commutative_golden_child",
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
        assert_eq!(stdout.matches(BEGIN).count(), 1);
        assert_eq!(stdout.matches(END).count(), 1);
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
