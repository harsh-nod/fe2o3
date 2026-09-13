use super::*;
use crate::{
    AssemblyConstraint, AssemblyOperand, AssemblySourceIdentity, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrWorkBudgetV1, Constant, Diagnostic, DiagnosticLocation, FunctionBody,
    InlineAssembly, InlineAssemblyTarget, IntrinsicOperation, MatrixElement, MatrixOperation,
    ModuleId, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    WaveOperation, WaveOperationKind, WaveWidth,
};

fn verify_with_resources<'module>(
    module: &'module Module,
    supported: Option<&BTreeSet<TargetCapability>>,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<VerifiedKernelIrModuleV1<'module>, MeteredKernelIrVerificationErrorV1>,
    usize,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
    let result = verify_exact_decoded_module_with_budget_v1(module, supported, &mut budget);
    let accepted_work = budget.work();
    let current_storage = budget.storage();
    let peak_storage = budget.peak_storage();
    (result, accepted_work, current_storage, peak_storage)
}

fn diagnostic(
    module: &str,
    function: Option<&str>,
    block: Option<BlockId>,
    operation: Option<usize>,
    code: DiagnosticCode,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        location: DiagnosticLocation {
            module: ModuleId::new(module),
            function: function.map(FunctionId::new),
            kernel: None,
            block,
            operation,
        },
        code,
        message: message.into(),
    }
}

fn assert_diagnostic_snapshot(
    module: &Module,
    supported: Option<&BTreeSet<TargetCapability>>,
    expected: Vec<Diagnostic>,
) {
    let (metered, _, storage, _) = verify_with_resources(module, supported, usize::MAX, usize::MAX);
    let actual = metered
        .map(|_| Vec::new())
        .unwrap_or_else(|error| match error {
            MeteredKernelIrVerificationErrorV1::Verification(error) => error.into_diagnostics(),
            MeteredKernelIrVerificationErrorV1::Resource(error) => {
                panic!("unexpected resource failure: {error}")
            }
        });
    assert_eq!(actual, expected);
    assert_eq!(storage, 0);
}

fn one_block_module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let parameter_values = (0..parameters.len())
        .map(|ordinal| ValueId(ordinal as u32))
        .collect();
    let function = Function::definition(
        "test",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    let mut module = Module::new("module");
    module.functions.push(function);
    module
}

#[test]
fn empty_valid_module_has_exact_constant_verification_boundary() {
    // With empty structural rosters, only the five-field module diagnostic
    // location passed through capability validation is constructed.
    const EXACT_WORK: usize = 5;
    let module = Module::new("module");

    let (result, work, storage, peak) = verify_with_resources(&module, None, EXACT_WORK, 0);
    assert_eq!(result.unwrap().module(), &module);
    assert_eq!((work, storage, peak), (EXACT_WORK, 0, 0));

    let (result, work, storage, peak) = verify_with_resources(&module, None, EXACT_WORK - 1, 0);
    assert!(matches!(
        result,
        Err(MeteredKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Work(error)
        )) if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
    ));
    assert_eq!((work, storage, peak), (0, 0, 0));
}

#[test]
fn invalid_empty_identity_has_exact_two_pass_work_and_storage() {
    const MESSAGE: &str = "module identity must not be empty";
    const COUNT_PASS: usize = 5 + 1 + MESSAGE.len() + 5;
    const MATERIALIZE_PASS: usize =
        5 + 1 + MESSAGE.len() + (5 + 2) + (MESSAGE.len() + MESSAGE.len() + 1) + 5;
    const FINISH: usize = 1;
    const EXACT_WORK: usize = COUNT_PASS + MATERIALIZE_PASS + FINISH;
    const ROW: usize = std::mem::size_of::<Diagnostic>().div_ceil(std::mem::size_of::<usize>());
    const EXACT_STORAGE: usize = ROW + MESSAGE.len();
    let module = Module::new("");
    let (result, work, storage, peak) =
        verify_with_resources(&module, None, EXACT_WORK, EXACT_STORAGE);
    let MeteredKernelIrVerificationErrorV1::Verification(errors) = result.unwrap_err() else {
        panic!("expected semantic verification diagnostics");
    };
    assert_eq!(
        errors.into_diagnostics(),
        [diagnostic(
            "",
            None,
            None,
            None,
            DiagnosticCode::InvalidIdentity,
            MESSAGE,
        )]
    );
    assert_eq!((work, storage, peak), (EXACT_WORK, 0, EXACT_STORAGE));

    let (result, work, storage, peak) =
        verify_with_resources(&module, None, EXACT_WORK - 1, EXACT_STORAGE);
    assert!(matches!(
        result,
        Err(MeteredKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Work(error)
        )) if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
    ));
    assert_eq!((work, storage, peak), (EXACT_WORK - 1, 0, EXACT_STORAGE));

    let (result, _, storage, peak) =
        verify_with_resources(&module, None, usize::MAX, EXACT_STORAGE - 1);
    assert!(matches!(
        result,
        Err(MeteredKernelIrVerificationErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == EXACT_STORAGE && error.limit() == EXACT_STORAGE - 1
    ));
    assert_eq!((storage, peak), (0, ROW));
}

#[test]
fn supplied_capability_roster_is_validated_before_module_requirements() {
    let module = Module::new("module");
    let supported = BTreeSet::from([TargetCapability::DynamicWorkgroupMemory]);
    let (result, _, storage, _) =
        verify_with_resources(&module, Some(&supported), usize::MAX, usize::MAX);
    let MeteredKernelIrVerificationErrorV1::Verification(errors) = result.unwrap_err() else {
        panic!("expected malformed supported-capability diagnostics");
    };
    assert_eq!(
        errors.into_diagnostics(),
        [diagnostic(
            "module",
            None,
            None,
            None,
            DiagnosticCode::InvalidCapability,
            "dynamic workgroup memory requires the base workgroup-memory capability",
        )]
    );
    assert_eq!(storage, 0);
}

#[test]
fn duplicate_long_identifier_role_diagnostic_has_stable_snapshot() {
    let id = FunctionId::new(format!("{}a", "long-common-prefix".repeat(240)));
    let signature = Signature::new(vec![], vec![]);
    let external = Function {
        id: id.clone(),
        signature: signature.clone(),
        role: FunctionRole::ExternalImport,
        body: None,
        required_capabilities: BTreeSet::new(),
    };
    let defined = Function {
        id,
        signature,
        role: FunctionRole::InternalHelper,
        body: Some(FunctionBody {
            parameters: Vec::<ValueId>::new(),
            blocks: vec![BasicBlock {
                id: BlockId(0),
                parameters: vec![],
                operations: vec![],
                terminator: Some(Terminator::Return { values: vec![] }),
            }],
        }),
        required_capabilities: BTreeSet::new(),
    };
    let module = Module {
        id: ModuleId::new("module"),
        functions: vec![external, defined],
        kernels: vec![],
        required_capabilities: BTreeSet::new(),
    };
    let (result, _, storage, _) = verify_with_resources(&module, None, usize::MAX, usize::MAX);
    let MeteredKernelIrVerificationErrorV1::Verification(errors) = result.unwrap_err() else {
        panic!("expected duplicate-function diagnostics");
    };
    let id = module.functions[0].id.as_str();
    assert_eq!(
        errors.into_diagnostics(),
        [
            diagnostic(
                "module",
                Some(id),
                None,
                None,
                DiagnosticCode::DuplicateFunction,
                format!("function {id} is defined more than once"),
            ),
            diagnostic(
                "module",
                Some(id),
                None,
                None,
                DiagnosticCode::ConflictingFunctionRole,
                format!("function {id} has conflicting roles ExternalImport and InternalHelper"),
            ),
        ]
    );
    assert_eq!(storage, 0);
}

#[test]
fn sparse_duplicate_definitions_and_blocks_have_stable_snapshot() {
    let function = Function {
        id: "sparse-duplicates".into(),
        signature: Signature::new(vec![Type::INDEX, Type::INDEX], vec![]),
        role: FunctionRole::InternalHelper,
        body: Some(FunctionBody {
            parameters: vec![ValueId(u32::MAX), ValueId(u32::MAX)],
            blocks: vec![
                BasicBlock {
                    id: BlockId(u32::MAX),
                    parameters: vec![],
                    operations: vec![],
                    terminator: Some(Terminator::Return { values: vec![] }),
                },
                BasicBlock {
                    id: BlockId(u32::MAX),
                    parameters: vec![ValueDef::new(ValueId(u32::MAX), Type::INDEX)],
                    operations: vec![],
                    terminator: Some(Terminator::Return { values: vec![] }),
                },
            ],
        }),
        required_capabilities: BTreeSet::new(),
    };
    let mut module = Module::new("module");
    module.functions.push(function);
    assert_diagnostic_snapshot(
        &module,
        None,
        vec![
            diagnostic(
                "module",
                Some("sparse-duplicates"),
                None,
                None,
                DiagnosticCode::DuplicateValue,
                "SSA value %4294967295 is defined more than once",
            ),
            diagnostic(
                "module",
                Some("sparse-duplicates"),
                Some(BlockId(u32::MAX)),
                None,
                DiagnosticCode::DuplicateBlock,
                "block bb4294967295 is defined more than once",
            ),
            diagnostic(
                "module",
                Some("sparse-duplicates"),
                Some(BlockId(u32::MAX)),
                None,
                DiagnosticCode::DuplicateValue,
                "SSA value %4294967295 is defined more than once",
            ),
        ],
    );
}

#[test]
fn cfg_dominance_branch_arguments_and_undefined_operands_have_stable_snapshot() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4), Type::F32),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(99),
            rhs: ValueId(99),
        },
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::F32),
        OperationKind::Constant(Constant::F32Bits(0)),
    ));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.parameters.push(ValueDef::new(ValueId(2), Type::F32));
    merge.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(3), Type::F32),
        OperationKind::Unary {
            op: crate::UnaryOp::Negate,
            operand: ValueId(1),
        },
    ));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "cfg",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![entry, then_block, else_block, merge],
    );
    let mut module = Module::new("module");
    module.functions.push(function);
    assert_diagnostic_snapshot(
        &module,
        None,
        vec![
            diagnostic(
                "module",
                Some("cfg"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::UndefinedValue,
                "SSA value %99 is not defined in this function",
            ),
            diagnostic(
                "module",
                Some("cfg"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::UndefinedValue,
                "SSA value %99 is not defined in this function",
            ),
            diagnostic(
                "module",
                Some("cfg"),
                Some(BlockId(1)),
                None,
                DiagnosticCode::BranchArgumentCount,
                "branch to bb3 supplies 0 arguments for 1 block parameters",
            ),
            diagnostic(
                "module",
                Some("cfg"),
                Some(BlockId(2)),
                None,
                DiagnosticCode::BranchArgumentCount,
                "branch to bb3 supplies 0 arguments for 1 block parameters",
            ),
            diagnostic(
                "module",
                Some("cfg"),
                Some(BlockId(3)),
                Some(0),
                DiagnosticCode::NonDominatingUse,
                "definition of %1 does not dominate this use",
            ),
        ],
    );
}

#[test]
fn registered_and_legacy_operation_families_have_stable_snapshots() {
    let intrinsic = Operation::effect_free(
        ValueDef::new(ValueId(0), Type::F32),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    );
    assert_diagnostic_snapshot(
        &one_block_module(vec![], vec![intrinsic]),
        None,
        vec![diagnostic(
            "module",
            Some("test"),
            Some(BlockId(0)),
            Some(0),
            DiagnosticCode::TypeMismatch,
            "result %0 has type Scalar(F32), expected Scalar(Index)",
        )],
    );

    let unknown_call = Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("missing"),
            arguments: vec![],
        },
    );
    assert_diagnostic_snapshot(
        &one_block_module(vec![], vec![unknown_call]),
        None,
        vec![diagnostic(
            "module",
            Some("test"),
            Some(BlockId(0)),
            Some(0),
            DiagnosticCode::UnknownCallee,
            "callee missing is not in the module",
        )],
    );

    let matrix = MatrixOperation::lds_load(ValueId(0), MatrixElement::F32);
    let matrix_results = matrix
        .result_types()
        .into_iter()
        .enumerate()
        .map(|(ordinal, ty)| ValueDef::new(ValueId(ordinal as u32 + 1), ty))
        .collect();
    assert_diagnostic_snapshot(
        &one_block_module(
            vec![Type::F32],
            vec![Operation::new(
                matrix_results,
                OperationKind::Matrix(matrix),
            )],
        ),
        None,
        vec![
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::InvalidOperandType,
                "matrix LDS base must be a workgroup pointer",
            ),
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS base must be the direct result of an authenticated workgroup-memory allocation",
            ),
        ],
    );

    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [0; 32]),
        mnemonic: "v_add_u32\ninvalid".to_owned(),
        operands: vec![AssemblyOperand::output(7, AssemblyConstraint::Vgpr32)],
        options: BTreeSet::new(),
        declared_effects: BTreeSet::new(),
    };
    assert_diagnostic_snapshot(
        &one_block_module(
            vec![],
            vec![Operation::new(
                vec![ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32))],
                OperationKind::InlineAssembly(assembly),
            )],
        ),
        None,
        vec![
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::ResultArity,
                "every inline assembly result must be referenced exactly once by an output or inout operand",
            ),
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::ResultArity,
                "inline assembly operand 0 references missing result 7",
            ),
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::InvalidInlineAssembly,
                "effect-free inline assembly requires an explicit NoMemory option",
            ),
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly mnemonic must be nonempty canonical lowercase ASCII",
            ),
            diagnostic(
                "module",
                Some("test"),
                Some(BlockId(0)),
                Some(0),
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly requires nonzero frontend-unit, function, contract, and statement identities",
            ),
        ],
    );

    let mut wave = WaveOperation::full(WaveOperationKind::LaneId, WaveWidth::Wave64);
    wave.active_lanes = 32;
    assert_diagnostic_snapshot(
        &one_block_module(
            vec![],
            vec![Operation::effect_free(
                ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
                OperationKind::Wave(wave),
            )],
        ),
        None,
        vec![diagnostic(
            "module",
            Some("test"),
            Some(BlockId(0)),
            Some(0),
            DiagnosticCode::InvalidWaveOperation,
            "the first wave-operation subset requires all 64 lanes active, found 32",
        )],
    );
}
