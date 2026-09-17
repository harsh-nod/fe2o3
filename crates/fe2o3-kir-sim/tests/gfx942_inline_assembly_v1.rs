//! Canonical KIR fixtures: these deliberately do not claim source marker admission.

use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AssemblyConstraint, AssemblyEffect, AssemblyOperand,
    AssemblyOperandKind, AssemblyOption, AssemblySourceIdentity, BasicBlock, BlockId, Function,
    InlineAssembly, InlineAssemblyTarget, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV11,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugSinkControlV1,
    SimulationDebugSinkV1, SimulationDebugValueV1, SimulationLimitsV1, SimulationPreflightErrorV1,
    SimulationRequestV1, SimulationTargetV1, UnsupportedFeatureV1,
};

fn module(mnemonic: &str, scalar: ScalarType) -> Module {
    let scalar_type = Type::Scalar(scalar);
    let constraint = if mnemonic == "s_mov_b32" {
        AssemblyConstraint::Sgpr32
    } else {
        AssemblyConstraint::Vgpr32
    };
    let mut operands = vec![
        AssemblyOperand::output(0, constraint),
        AssemblyOperand::input(ValueId(0), constraint),
    ];
    if !matches!(mnemonic, "v_mov_b32" | "s_mov_b32") {
        operands.push(AssemblyOperand::input(ValueId(1), constraint));
    }
    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        mnemonic: mnemonic.to_owned(),
        operands,
        options: BTreeSet::from([AssemblyOption::NoMemory]),
        declared_effects: BTreeSet::new(),
    };
    let capabilities = assembly.required_capabilities();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar_type.clone()),
            OperationKind::InlineAssembly(assembly),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "integer_isa_impl",
        Signature::new(
            vec![
                scalar_type.clone(),
                scalar_type.clone(),
                Type::pointer(scalar_type, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    function.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "integer_isa",
        "integer_isa_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("integer-isa-canonical-fixture");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn assembly(module: &mut Module) -> &mut InlineAssembly {
    let OperationKind::InlineAssembly(assembly) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assembly
}

fn admitted(module: Module, version: u16) -> AdmittedSimulationModuleV1 {
    match version {
        7 => AdmittedSimulationModuleV1::admit(
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap(),
        11 => AdmittedSimulationModuleV1::admit_v11(
            VerifiedCanonicalKernelIrV11::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        )
        .unwrap(),
        _ => unreachable!(),
    }
}

fn request(scalar: ScalarType, lhs: u32, rhs: u32) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    SimulationRequestV1::new(
        "integer_isa",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(scalar, u128::from(lhs), target).unwrap(),
            ),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(scalar, u128::from(rhs), target).unwrap(),
            ),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    scalar,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; 12],
                    vec![false; 12],
                    target,
                )
                .unwrap(),
            ),
        ],
    )
}

#[test]
fn six_vector_instructions_execute_with_wrap_semantics_and_preserve_canaries() {
    let cases: [(&str, u32, u32, u32); 11] = [
        ("v_mov_b32", 0x89ab_cdef, 0, 0x89ab_cdef),
        ("v_add_u32", u32::MAX, 1, 0),
        ("v_add_u32", 0x7fff_ffff, 1, 0x8000_0000),
        ("v_add_u32", 0x8000_0000, 0x8000_0000, 0),
        ("v_add_u32", 19, 23, 42),
        ("v_sub_u32", 0, 1, u32::MAX),
        ("v_sub_u32", 0x8000_0000, 1, 0x7fff_ffff),
        ("v_sub_u32", 19, 23, 0xffff_fffc),
        ("v_and_b32", 0xaaaa_ffff, 0x5555_1234, 0x0000_1234),
        ("v_or_b32", 0xaaaa_0000, 0x5555_1234, 0xffff_1234),
        ("v_xor_b32", 0xaaaa_ffff, 0x5555_1234, 0xffff_edcb),
    ];
    for version in [7, 11] {
        for scalar in [ScalarType::U32, ScalarType::I32] {
            for (mnemonic, lhs, rhs, expected) in cases {
                let admitted = admitted(module(mnemonic, scalar), version);
                let request = request(scalar, lhs, rhs);
                let original = request.clone();
                let execution = admitted
                    .simulate(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                    )
                    .unwrap();
                assert_eq!(request, original, "caller-owned input is immutable");
                let output = execution.buffer(2).unwrap();
                assert_eq!(
                    &output.bytes()[..4],
                    &expected.to_le_bytes(),
                    "{mnemonic} {scalar:?}"
                );
                assert_eq!(&output.bytes()[4..], &[0x5a; 8]);
                assert_eq!(&output.initialized()[..4], &[true; 4]);
                assert_eq!(&output.initialized()[4..], &[false; 8]);
                assert!(!execution.grants_execution_authority());
            }
        }
    }
}

#[test]
fn valid_canonical_but_unsupported_instruction_contracts_fail_before_execution() {
    for mutation in 0..8 {
        let mut module = module("v_add_u32", ScalarType::U32);
        match mutation {
            0 => assembly(&mut module).mnemonic = "s_waitcnt".to_owned(),
            1 => assembly(&mut module).operands[1].constraint = AssemblyConstraint::Sgpr32,
            2 => {
                assembly(&mut module).operands[1] = AssemblyOperand {
                    kind: AssemblyOperandKind::ImmediateI32(1),
                    constraint: AssemblyConstraint::ImmediateI32,
                }
            }
            3 => {
                assembly(&mut module).operands.pop();
            }
            4 => assembly(&mut module).operands.swap(0, 1),
            5 => {
                assembly(&mut module)
                    .declared_effects
                    .insert(AssemblyEffect::ControlFlow);
            }
            6 => {
                let assembly = assembly(&mut module);
                assembly.options = BTreeSet::from([AssemblyOption::ReadOnly]);
                assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
            }
            _ => module.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32),
        }
        let admitted = admitted(module, 11);
        let mut request = request(ScalarType::U32, 7, 9);
        if mutation == 7 {
            request.arguments[0] = SimulationArgumentV1::Scalar(ScalarBitsV1::i32(7));
        }
        let error = admitted
            .preflight(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected unsupported assembly for mutation {mutation}: {error:?}")
        };
        assert!(
            report.findings().iter().any(|finding| finding.feature
                == UnsupportedFeatureV1::InlineAssembly
                && finding.operation == Some(0)),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_register_moves_remain_unavailable_without_a_uniformity_model() {
    let admitted = admitted(module("s_mov_b32", ScalarType::U32), 11);
    let error = admitted
        .preflight(
            &request(ScalarType::U32, 7, 9),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        report.findings()[0].feature,
        UnsupportedFeatureV1::InlineAssembly
    );
}

#[test]
fn incomplete_source_identity_is_rejected_by_canonical_admission() {
    let mut module = module("v_add_u32", ScalarType::U32);
    assembly(&mut module).source.statement = [0; 32];
    assert!(VerifiedCanonicalKernelIrV11::from_module(module).is_err());
}

#[derive(Default)]
struct DebugRecords(Vec<SimulationDebugRecordV1>);

impl SimulationDebugSinkV1 for DebugRecords {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

fn debug_assembly_result(records: &DebugRecords) -> Option<SimulationDebugValueV1> {
    records.0.iter().find_map(|record| {
        if record.site.function_ordinal != 0
            || record.site.block != BlockId(0)
            || record.site.operation != 0
        {
            return None;
        }
        let SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } = &record.kind
        else {
            return None;
        };
        frames.iter().find_map(|frame| {
            let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
                return None;
            };
            values
                .iter()
                .find(|binding| binding.value == ValueId(3))
                .map(|binding| binding.observed.clone())
        })
    })
}

#[test]
fn vector_instructions_are_debuggable_on_a_bounded_stack() {
    std::thread::Builder::new()
        .name("integer-isa-small-stack".to_owned())
        .stack_size(256 * 1024)
        .spawn(|| {
            let cases = [
                ("v_mov_b32", 0x89ab_cdef, 0, 0x89ab_cdef),
                ("v_add_u32", u32::MAX, 1, 0),
                ("v_sub_u32", 0, 1, u32::MAX),
                ("v_and_b32", 0xaaaa_ffff, 0x5555_1234, 0x0000_1234),
                ("v_or_b32", 0xaaaa_0000, 0x5555_1234, 0xffff_1234),
                ("v_xor_b32", 0xaaaa_ffff, 0x5555_1234, 0xffff_edcb),
            ];
            for (mnemonic, lhs, rhs, expected) in cases {
                // Inspect the ISA result directly through its SSA checkpoint. Store
                // canaries are covered above; the existing OOB small-stack test
                // independently covers its memory-error path, not successful stores.
                let mut isa_module = module(mnemonic, ScalarType::U32);
                isa_module.functions[0].body.as_mut().unwrap().blocks[0]
                    .operations
                    .truncate(1);
                let admitted = admitted(isa_module, 11);
                let request = request(ScalarType::U32, lhs, rhs);
                let mut records = DebugRecords::default();
                admitted
                    .simulate_debugged_with_sink(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        SimulationDebugCaptureLimitsV1::new(8, 16, 8, 64).unwrap(),
                        &mut records,
                    )
                    .unwrap();
                assert_eq!(
                    debug_assembly_result(&records),
                    Some(SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(expected))),
                    "{mnemonic}"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn original_operation_coordinates_and_wrapped_results_reach_debug_checkpoints() {
    let admitted = admitted(module("v_add_u32", ScalarType::U32), 11);
    let request = request(ScalarType::U32, u32::MAX, 1);
    let mut first = DebugRecords::default();
    let mut second = DebugRecords::default();
    for records in [&mut first, &mut second] {
        admitted
            .simulate_debugged_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationDebugCaptureLimitsV1::new(8, 16, 8, 64).unwrap(),
                records,
            )
            .unwrap();
    }
    assert_eq!(first.0, second.0, "debug observations are deterministic");
    assert_eq!(
        debug_assembly_result(&first),
        Some(SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(0)))
    );
}
