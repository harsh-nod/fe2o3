//! Closed observations of the actual owning V18 Policy10 service.
//! Subjects use canonical V18 codecs, not a test IR parser. Golden text is
//! descriptive and cannot grant source, proof, artifact, or launch authority.
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirOperationOriginV1 as Origin, CastKind, Constant,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    StorageLayoutIdV1 as LayoutId, StorageLayoutKindV1 as LayoutKind, StorageLayoutLimitsV1,
    StorageLayoutV1, StorageOperationV1 as Storage, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_pliron::{
    PlironOptimizationPassV1 as Pass, optimize_neutral_kernel_ir_mixed_pure_cse_v18,
};
use std::fmt::Write;

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 8,
    edges: 16,
    containment_depth: 8,
    object_bytes: 64,
};
const PASSES: [Pass; 4] = [
    Pass::IntegerNeutralWorklistCanonicalization,
    Pass::LocalPureCommonSubexpressionElimination,
    Pass::DominancePureCommonSubexpressionElimination,
    Pass::DeadCodeElimination,
];
const U32: Type = Type::Scalar(ScalarType::U32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    MixedEffects,
    Dominance,
    Siblings,
    Loop,
    TypedCasts,
    TrappingArithmetic,
    NeutralThenCse,
}
const CASES: [Case; 7] = [
    Case::MixedEffects,
    Case::Dominance,
    Case::Siblings,
    Case::Loop,
    Case::TypedCasts,
    Case::TrappingArithmetic,
    Case::NeutralThenCse,
];
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::MixedEffects => "mixed-effects",
            Self::Dominance => "dominance",
            Self::Siblings => "siblings",
            Self::Loop => "loop",
            Self::TypedCasts => "typed-casts",
            Self::TrappingArithmetic => "trapping-arithmetic",
            Self::NeutralThenCse => "neutral-then-cse",
        }
    }
    fn golden(self) -> &'static str {
        match self {
            Self::MixedEffects => include_str!("mixed-pure-cse-v18-golden/mixed-effects.golden"),
            Self::Dominance => include_str!("mixed-pure-cse-v18-golden/dominance.golden"),
            Self::Siblings => include_str!("mixed-pure-cse-v18-golden/siblings.golden"),
            Self::Loop => include_str!("mixed-pure-cse-v18-golden/loop.golden"),
            Self::TypedCasts => include_str!("mixed-pure-cse-v18-golden/typed-casts.golden"),
            Self::TrappingArithmetic => {
                include_str!("mixed-pure-cse-v18-golden/trapping-arithmetic.golden")
            }
            Self::NeutralThenCse => {
                include_str!("mixed-pure-cse-v18-golden/neutral-then-cse.golden")
            }
        }
    }
    fn changed(self) -> bool {
        !matches!(self, Self::Siblings | Self::TrappingArithmetic)
    }
}

fn value(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    value(
        id,
        U32,
        Kind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn ret(values: &[u32]) -> Terminator {
    Terminator::Return {
        values: values.iter().copied().map(ValueId).collect(),
    }
}
fn branch(target: u32, arguments: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().copied().map(ValueId).collect(),
    }
}
fn block(id: u32, operations: Vec<Operation>, term: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(term);
    block
}
fn helper(parameters: Vec<Type>, results: Vec<Type>, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("policy10-golden");
    let arguments = (0..parameters.len()).map(|i| ValueId(i as u32)).collect();
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(parameters, results),
        arguments,
        blocks,
    ));
    module
}
fn conditional(yes: u32, yes_args: &[u32], no: u32, no_args: &[u32]) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(yes),
        then_arguments: yes_args.iter().copied().map(ValueId).collect(),
        else_target: BlockId(no),
        else_arguments: no_args.iter().copied().map(ValueId).collect(),
    }
}

// Both graphs are independently constructed from this closed recipe. `after`
// is never computed from optimizer output, candidate rows, or output decoding.
fn fixture(case: Case, after: bool) -> Module {
    match case {
        Case::MixedEffects => {
            let access = MemoryAccess::new(AddressSpace::Private, 4);
            let global = MemoryAccess::new(AddressSpace::Global, 4);
            let mut operations = vec![
                value(
                    20,
                    Type::Execution(Role::Context),
                    Kind::Execution(Execution::ContextIssue),
                ),
                value(
                    21,
                    Type::Execution(Role::Workgroup),
                    Kind::Execution(Execution::WorkgroupDerive {
                        context: ValueId(20),
                    }),
                ),
                value(
                    10,
                    Type::pointer(
                        Type::StorageObject(LayoutId(0)),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    ),
                    Kind::Alloca {
                        element: Type::StorageObject(LayoutId(0)),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                binary(30, BinaryOp::BitXor, 0, 1),
            ];
            if !after {
                operations.push(binary(31, BinaryOp::BitXor, 0, 1));
            }
            let store = |v| {
                Operation::new(
                    vec![],
                    Kind::Storage(Storage::WriteValue {
                        address: ValueId(10),
                        value: ValueId(v),
                        access,
                    }),
                )
            };
            operations.push(store(30));
            for id in [40, 41] {
                operations.push(value(
                    id,
                    U32,
                    Kind::Storage(Storage::ReadValue {
                        address: ValueId(10),
                        access,
                    }),
                ));
            }
            for id in [42, 43] {
                operations.push(value(
                    id,
                    U32,
                    Kind::Load {
                        pointer: ValueId(2),
                        access: global,
                    },
                ));
            }
            for id in [40, 41, 42, 43] {
                operations.push(store(id));
            }
            operations.push(Operation::new(
                vec![],
                Kind::Store {
                    pointer: ValueId(2),
                    value: ValueId(if after { 30 } else { 31 }),
                    access: global,
                },
            ));
            operations.push(Operation::new(
                vec![],
                Kind::Execution(Execution::ScopeEnd {
                    workgroup: ValueId(21),
                    discarded: vec![],
                }),
            ));
            let mut module = Module::new("policy10-golden");
            module.storage_layouts.push(StorageLayoutV1 {
                size: 4,
                alignment: 4,
                kind: LayoutKind::Scalar(ScalarType::U32),
            });
            module.functions.push(Function::kernel_entry(
                "f",
                Signature::new(
                    vec![
                        U32,
                        U32,
                        Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite),
                    ],
                    vec![],
                ),
                vec![ValueId(0), ValueId(1), ValueId(2)],
                vec![block(90, operations, ret(&[]))],
            ));
            module.kernels.push(Kernel::new(
                "f",
                "f",
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(64),
                },
            ));
            module
        }
        Case::Dominance => helper(
            vec![Type::BOOL, U32, U32],
            vec![U32],
            vec![
                block(
                    90,
                    vec![binary(10, BinaryOp::BitXor, 1, 2)],
                    branch(200, &[]),
                ),
                block(
                    200,
                    if after {
                        vec![]
                    } else {
                        vec![binary(11, BinaryOp::BitXor, 1, 2)]
                    },
                    ret(&[if after { 10 } else { 11 }]),
                ),
            ],
        ),
        Case::Siblings => helper(
            vec![Type::BOOL, U32, U32],
            vec![U32],
            vec![
                block(90, vec![], conditional(200, &[], 300, &[])),
                block(200, vec![binary(10, BinaryOp::BitXor, 1, 2)], ret(&[10])),
                block(300, vec![binary(11, BinaryOp::BitXor, 1, 2)], ret(&[11])),
            ],
        ),
        Case::Loop => {
            let mut ops = Vec::new();
            if !after {
                ops.push(binary(11, BinaryOp::BitXor, 1, 2));
            }
            ops.push(binary(
                21,
                BinaryOp::BitXor,
                20,
                if after { 10 } else { 11 },
            ));
            if !after {
                ops.push(binary(22, BinaryOp::BitXor, 20, 11));
            }
            let mut header = block(
                200,
                ops,
                conditional(200, &[21], 300, &[if after { 21 } else { 22 }]),
            );
            header.parameters.push(ValueDef::new(ValueId(20), U32));
            let mut exit = block(300, vec![], ret(&[30]));
            exit.parameters.push(ValueDef::new(ValueId(30), U32));
            helper(
                vec![Type::BOOL, U32, U32],
                vec![U32],
                vec![
                    block(
                        90,
                        vec![binary(10, BinaryOp::BitXor, 1, 2)],
                        branch(200, &[10]),
                    ),
                    header,
                    exit,
                ],
            )
        }
        Case::TypedCasts => {
            let cast = |id, ty: Type| {
                value(
                    id,
                    ty.clone(),
                    Kind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(0),
                        to: ty,
                    },
                )
            };
            let mut operations = vec![cast(10, Type::Scalar(ScalarType::I32))];
            if !after {
                operations.push(cast(11, Type::Scalar(ScalarType::I32)));
            }
            operations.push(cast(12, Type::Scalar(ScalarType::F32)));
            helper(
                vec![U32],
                vec![Type::Scalar(ScalarType::I32), Type::Scalar(ScalarType::F32)],
                vec![block(
                    90,
                    operations,
                    ret(&[if after { 10 } else { 11 }, 12]),
                )],
            )
        }
        Case::TrappingArithmetic => helper(
            vec![U32, U32],
            vec![U32, U32, U32, U32],
            vec![block(
                90,
                vec![
                    binary(10, BinaryOp::Add, 0, 1),
                    binary(11, BinaryOp::Add, 0, 1),
                    binary(12, BinaryOp::Divide, 0, 1),
                    binary(13, BinaryOp::Divide, 0, 1),
                ],
                ret(&[10, 11, 12, 13]),
            )],
        ),
        Case::NeutralThenCse => {
            let mut operations = Vec::new();
            if !after {
                operations.push(value(2, U32, Kind::Constant(Constant::U32(0))));
                operations.push(binary(11, BinaryOp::BitXor, 0, 2));
            }
            operations.push(binary(12, BinaryOp::BitXor, 0, 1));
            if !after {
                operations.push(binary(13, BinaryOp::BitXor, 11, 1));
            }
            helper(
                vec![U32, U32],
                vec![U32],
                vec![block(90, operations, ret(&[if after { 12 } else { 13 }]))],
            )
        }
    }
}

fn ids(values: &[ValueId]) -> String {
    values
        .iter()
        .map(|v| format!("v{}", v.0))
        .collect::<Vec<_>>()
        .join(",")
}
fn ty(t: &Type) -> String {
    match t {
        Type::Scalar(
            s @ (ScalarType::U32 | ScalarType::I32 | ScalarType::F32 | ScalarType::Bool),
        ) => format!("{s:?}"),
        Type::Execution(r @ (Role::Context | Role::Workgroup)) => format!("Execution<{r:?}>"),
        Type::StorageObject(row) => format!("Object<{}>", row.0),
        Type::Pointer(pointer) => format!(
            "Ptr<{},{:?},{:?}>",
            ty(&pointer.pointee),
            pointer.address_space,
            pointer.access
        ),
        other => panic!("outside closed golden type observations: {other:?}"),
    }
}
fn definitions(values: &[ValueDef]) -> String {
    values
        .iter()
        .map(|v| format!("v{}:{}", v.id.0, ty(&v.ty)))
        .collect::<Vec<_>>()
        .join(",")
}
fn memory(access: MemoryAccess) -> String {
    format!(
        "{:?} align={} volatile={}",
        access.address_space, access.alignment, access.volatile
    )
}
fn describe(operation: &Operation) -> String {
    let kind = match &operation.kind {
        Kind::Binary { op, lhs, rhs } => format!("{op:?} v{},v{}", lhs.0, rhs.0),
        Kind::Constant(Constant::U32(n)) => format!("U32 {n}"),
        Kind::Cast {
            kind: CastKind::Bitcast,
            value,
            to,
        } => format!("Bitcast v{} to {}", value.0, ty(to)),
        Kind::Alloca {
            element,
            count: None,
            address_space,
            alignment,
        } => format!("Alloca {} {address_space:?} align={alignment}", ty(element)),
        Kind::Load { pointer, access } => format!("Load v{} {}", pointer.0, memory(*access)),
        Kind::Store {
            pointer,
            value,
            access,
        } => format!("Store v{} <- v{} {}", pointer.0, value.0, memory(*access)),
        Kind::Storage(Storage::ReadValue { address, access }) => {
            format!("StorageRead v{} {}", address.0, memory(*access))
        }
        Kind::Storage(Storage::WriteValue {
            address,
            value,
            access,
        }) => format!(
            "StorageWrite v{} <- v{} {}",
            address.0,
            value.0,
            memory(*access)
        ),
        Kind::Execution(Execution::ContextIssue) => "ContextIssue".into(),
        Kind::Execution(Execution::WorkgroupDerive { context }) => {
            format!("WorkgroupDerive v{}", context.0)
        }
        Kind::Execution(Execution::ScopeEnd {
            workgroup,
            discarded,
        }) if discarded.is_empty() => format!("ScopeEnd v{}", workgroup.0),
        other => panic!("outside closed golden operation observations: {other:?}"),
    };
    format!("[{}] {kind}", definitions(&operation.results))
}
fn observe_module(text: &mut String, prefix: &str, module: &Module) {
    writeln!(
        text,
        "{prefix}: module {} kernels={} layouts={}",
        module.id.as_str(),
        module.kernels.len(),
        module.storage_layouts.len()
    )
    .unwrap();
    for (fi, function) in module.functions.iter().enumerate() {
        let body = function.body.as_ref().unwrap();
        let parameters = body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .map(|(v, t)| format!("v{}:{}", v.0, ty(t)))
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
            writeln!(
                text,
                "{prefix}: f{fi}:b{bi} block-id={} params=[{}]",
                block.id.0,
                definitions(&block.parameters)
            )
            .unwrap();
            for (oi, operation) in block.operations.iter().enumerate() {
                writeln!(text, "{prefix}: f{fi}:b{bi}:o{oi} {}", describe(operation)).unwrap();
            }
            let term = match block.terminator.as_ref().unwrap() {
                Terminator::Return { values } => format!("Return [{}]", ids(values)),
                Terminator::Branch { target, arguments } => {
                    format!("Branch id={} [{}]", target.0, ids(arguments))
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
                    ids(then_arguments),
                    else_target.0,
                    ids(else_arguments)
                ),
                other => panic!("outside closed golden terminator observations: {other:?}"),
            };
            writeln!(text, "{prefix}: f{fi}:b{bi}:term {term}").unwrap();
        }
    }
}
fn pass_name(pass: Pass) -> &'static str {
    match pass {
        Pass::IntegerNeutralWorklistCanonicalization => "integer-neutral-worklist",
        Pass::LocalPureCommonSubexpressionElimination => "local-pure-cse",
        Pass::DominancePureCommonSubexpressionElimination => "dominance-pure-cse",
        Pass::DeadCodeElimination => "dce",
        other => panic!("unexpected actual Policy10 pass: {other:?}"),
    }
}
struct Observation {
    text: String,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
    rows: String,
}
fn observe(case: Case) -> Observation {
    let mut work = Work::new(10_000_000_000_000);
    let mut budget = Budget::new(&mut work, 2_000_000_000);
    budget.reserve_storage(97).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (input, input_credit) = Owner::from_module_ref_with_verification_budget_v18(
        &fixture(case, false),
        LIMITS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(input_credit.retained_storage())
        .unwrap();
    let (expected, expected_credit) = Owner::from_module_ref_with_verification_budget_v18(
        &fixture(case, true),
        LIMITS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(expected_credit.retained_storage())
        .unwrap();
    let (decoded, decoded_credit) = Owner::from_canonical_bytes_with_verification_budget_v18(
        input.canonical_bytes(),
        LIMITS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(decoded_credit.retained_storage())
        .unwrap();
    assert_eq!(input.module(), decoded.module());
    assert_eq!(input.identity(), decoded.identity());
    let floor = budget.storage();
    let observed =
        optimize_neutral_kernel_ir_mixed_pure_cse_v18(&decoded, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    assert!(std::ptr::eq(observed.input(), &decoded));
    assert_eq!(
        observed.owner().module(),
        expected.module(),
        "{}",
        case.name()
    );
    assert_eq!(
        observed.owner().canonical_bytes(),
        expected.canonical_bytes()
    );
    assert_eq!(
        observed
            .report()
            .passes()
            .iter()
            .map(|r| r.pass())
            .collect::<Vec<_>>(),
        PASSES
    );
    assert_eq!(
        observed.report().passes().iter().any(|r| r.changed()),
        case.changed()
    );
    assert_eq!(
        (
            observed.execution().policy_version(),
            observed.execution().graph_schema()
        ),
        (10, 18)
    );
    assert_eq!(observed.execution().canonical_bytes().len(), 576);
    assert_eq!(
        &observed.execution().canonical_bytes()[..8],
        &[10, 0, 1, 0, 4, 0, 18, 0]
    );
    assert!(!observed.execution().grants_authority());
    assert!(observed.map().matches_execution(observed.report()));
    observed
        .map()
        .check_against(&decoded, observed.owner(), &mut budget)
        .unwrap();
    // Consuming adoption independently checks the complete actual occurrence
    // transition. Both the observed and checked owners retain distinct credit.
    let checked = observed.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    assert_eq!(checked.input_audit_bytes(), decoded.canonical_bytes());
    assert!(!checked.grants_authority());
    let candidate = checked.occurrences().candidate();
    assert!(
        candidate
            .operations
            .iter()
            .all(|r| matches!(r.origin, Origin::Retained(_)))
    );
    assert!(candidate.edges.iter().all(|r| r.input == r.output));
    assert!(candidate.segments.iter().all(|r| r.connector.is_none()));
    assert_eq!(
        candidate.blocks.len(),
        decoded.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .len()
    );
    // These transcript copies are harness allocations, not verifier scratch.
    // All genuine owners remain reserved while their observations are copied.
    let mut text = format!("CASE: {}\n", case.name());
    observe_module(&mut text, "CHECK-BEFORE", decoded.module());
    observe_module(&mut text, "CHECK-AFTER", checked.owner().module());
    let roster = checked
        .report()
        .passes()
        .iter()
        .map(|r| pass_name(r.pass()))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(text, "CHECK-REMARK: policy=10 schema=18 passes={roster}").unwrap();
    writeln!(
        text,
        "CHECK-REMARK: changed={} independently-checked=true authority=false",
        case.changed()
    )
    .unwrap();
    let observation = Observation {
        text,
        input_bytes: decoded.canonical_bytes().to_vec(),
        output_bytes: checked.owner().canonical_bytes().to_vec(),
        rows: format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            candidate.operations,
            candidate.definitions,
            candidate.definition_outputs,
            candidate.uses,
            candidate.edges,
            candidate.edge_arguments
        ),
    };
    let retained = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(retained).unwrap();
    drop(decoded);
    budget
        .release_storage(decoded_credit.retained_storage())
        .unwrap();
    drop(expected);
    budget
        .release_storage(expected_credit.retained_storage())
        .unwrap();
    drop(input);
    budget
        .release_storage(input_credit.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 97);
    assert!(budget.work_ledger_identity_v1() == ledger);
    observation
}

#[test]
fn actual_owning_policy10_mixed_v18_observation_goldens() {
    for case in CASES {
        assert_eq!(observe(case).text, case.golden(), "{}", case.name());
    }
}

const CHILD: &str = "FE2O3_TEST_POLICY10_GOLDEN_CHILD";
const BEGIN: &str = "FE2O3_POLICY10_GOLDEN_BEGIN\n";
const END: &str = "FE2O3_POLICY10_GOLDEN_END";
#[test]
#[ignore = "subprocess helper: actual V18 canonical bytes and checked rows"]
fn canonical_policy10_golden_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let mut transcript = String::new();
    for case in CASES {
        let observation = observe(case);
        assert_eq!(observation.text, case.golden(), "{}", case.name());
        transcript.push_str(&observation.text);
        writeln!(transcript, "ROWS: {}", observation.rows).unwrap();
        for (label, bytes) in [
            ("INPUT", observation.input_bytes),
            ("OUTPUT", observation.output_bytes),
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
fn policy10_canonical_bytes_and_checked_rows_repeat_across_fresh_processes() {
    let current = std::env::current_exe().unwrap();
    let mut prior = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&current)
            .args([
                "--exact",
                "canonical_policy10_golden_child",
                "--ignored",
                "--test-threads=1",
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
        assert!(transcript.len() <= 256 * 1024);
        if let Some(prior) = &prior {
            assert_eq!(transcript, prior);
        } else {
            prior = Some(transcript.to_owned());
        }
    }
}
