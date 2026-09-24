//! Shared inert composition corpus. Not authenticated Rust source or execution evidence.
use super::ordered_composition_fixture_ir::*;

pub fn program(count: u8, alternate: bool) -> Gfx942U32ProgramV1 {
    use Gfx942ProgramDestinationV1::{Output, Scratch};
    use Gfx942ProgramRoleV1::{Input0, Input1, Input2};
    let mut words = [0; 16];
    for (i, word) in words.iter_mut().take(usize::from(count)).enumerate() {
        let destination = if i % 2 == 0 { Output } else { Scratch };
        let instruction = if i == 0 || i % 6 == 0 {
            Gfx942ProgramInstructionV1::Move {
                destination,
                source: if alternate { Input2 } else { Input0 },
            }
        } else {
            let opcode = match i % 6 {
                1 => Gfx942ProgramBinaryOpcodeV1::Add,
                2 => Gfx942ProgramBinaryOpcodeV1::Subtract,
                3 => Gfx942ProgramBinaryOpcodeV1::And,
                4 => Gfx942ProgramBinaryOpcodeV1::Or,
                _ => Gfx942ProgramBinaryOpcodeV1::Xor,
            };
            Gfx942ProgramInstructionV1::Binary {
                opcode,
                destination,
                left: Input0,
                right: if alternate { Input2 } else { Input1 },
            }
        };
        *word = instruction.descriptor();
    }
    Gfx942U32ProgramV1::from_descriptors(count, words).unwrap()
}
pub fn region(
    inputs: [ValueId; 3],
    result: ValueId,
    caller: u8,
    statement: u8,
    steps: u8,
) -> Operation {
    let registers = if caller % 2 == 0 {
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36])
    } else {
        Gfx942OrderedProgramRegistersV1::new(63, 0, [62, 17, 1])
    }
    .unwrap();
    Operation::effect_free(
        ValueDef::new(result, Type::Scalar(ScalarType::U32)),
        OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                AssemblySourceIdentity::new(
                    [1; 32],
                    [caller + 2; 32],
                    [3; 32],
                    [statement + 1; 32],
                ),
                registers,
                inputs,
                program(steps, caller % 2 == 1),
            )
            .unwrap(),
        ),
    )
}
/// One root, ordered direct regions followed by actual helper calls and one store.
pub fn module(
    direct: usize,
    helper_regions: &[usize],
    helper_calls: &[usize],
    steps: u8,
) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let capabilities =
        region([ValueId(0), ValueId(1), ValueId(2)], ValueId(4), 0, 0, 1).required_capabilities();
    let mut module = Module::new("inert_ordered_composition");
    module.required_capabilities = capabilities.clone();
    let mut root = BasicBlock::new(BlockId(40));
    let mut next = 100u32;
    let mut last = ValueId(0);
    for i in 0..direct {
        root.operations.push(region(
            [last, ValueId(1), ValueId(2)],
            ValueId(next),
            0,
            i as u8,
            steps,
        ));
        last = ValueId(next);
        next += 1;
    }
    for helper in helper_calls {
        root.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(next), scalar.clone()),
            OperationKind::Call {
                callee: FunctionId::new(format!("helper_{helper}")),
                arguments: vec![last, ValueId(1), ValueId(2)],
            },
        ));
        last = ValueId(next);
        next += 1;
    }
    root.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(3),
            value: last,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    root.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "root",
        Signature::new(
            vec![
                scalar.clone(),
                scalar.clone(),
                scalar.clone(),
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![root],
    );
    function.required_capabilities = capabilities.clone();
    module.functions.push(function);
    for (helper, count) in helper_regions.iter().enumerate() {
        let mut block = BasicBlock::new(BlockId(73));
        let mut last = ValueId(10);
        for i in 0..*count {
            let id = ValueId(100 + i as u32);
            block.operations.push(region(
                [last, ValueId(11), ValueId(12)],
                id,
                helper as u8 + 1,
                i as u8,
                steps,
            ));
            last = id;
        }
        block.terminator = Some(Terminator::Return { values: vec![last] });
        let mut function = Function::internal_helper(
            format!("helper_{helper}"),
            Signature::new(vec![scalar.clone(); 3], vec![scalar.clone()]),
            vec![ValueId(10), ValueId(11), ValueId(12)],
            vec![block],
        );
        if *count != 0 {
            function.required_capabilities = capabilities.clone();
        }
        module.functions.push(function);
    }
    let mut kernel = Kernel::new(
        "kernel",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = capabilities;
    module.kernels.push(kernel);
    module
}
