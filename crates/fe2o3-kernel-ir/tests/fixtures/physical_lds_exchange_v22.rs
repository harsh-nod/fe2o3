//! Inert actual-SSA fixture only: no Rust source authentication or native evidence.
use super::physical_lds_exchange_fixture_ir::*;
type Opcode = Gfx942PhysicalLdsExchangeOpcodeV1;
type Site = Gfx942PhysicalEntrySourceSiteVNext;
pub(crate) fn site(n: u8) -> Site {
    Site {
        occurrence: n,
        raw_block: 100 + u32::from(n),
        semantic_block_index: 41 - u32::from(n),
        semantic_block_identity: [n + 1; 32],
        semantic_callable_index: 7,
    }
}
pub(crate) const ROWS: [(Opcode, u8, u8, u8, u32); 31] = [
    (Opcode::LoadKernargPair, 8, 0, 0, 0),
    (Opcode::LoadKernargPair, 10, 0, 0, 8),
    (Opcode::LoadKernargPair, 12, 0, 0, 16),
    (Opcode::LoadKernargPair, 14, 0, 0, 24),
    (Opcode::WaitLgkm0, 0, 0, 0, 0),
    (Opcode::ScalarLshl32, 16, 2, 0, 7),
    (Opcode::VectorAddU32, 2, 16, 0, 0),
    (Opcode::VectorMove32, 3, 255, 0, 0),
    (Opcode::VectorMove32, 4, 9, 0, 0),
    (Opcode::VectorLshlrev64, 6, 2, 0, 2),
    (Opcode::VectorAddCarry, 6, 8, 6, 0),
    (Opcode::VectorAddCarryIn, 7, 4, 7, 0),
    (Opcode::GlobalLoadDword, 8, 6, 0, 0),
    (Opcode::WaitVm0, 0, 0, 0, 0),
    (Opcode::VectorLshlrev32, 16, 0, 0, 2),
    (Opcode::LdsWriteB32, 0, 16, 8, 0),
    (Opcode::WaitLgkm0, 0, 0, 0, 0),
    (Opcode::WorkgroupPublishBarrier, 0, 0, 0, 0),
    (Opcode::VectorXor32, 17, 0, 0, 64),
    (Opcode::VectorLshlrev32, 17, 17, 0, 2),
    (Opcode::LdsReadB32, 18, 17, 0, 0),
    (Opcode::WaitLgkm0, 0, 0, 0, 0),
    (Opcode::VectorMove32, 5, 13, 0, 0),
    (Opcode::VectorLshlrev64, 10, 2, 0, 2),
    (Opcode::VectorAddCarry, 10, 12, 10, 0),
    (Opcode::VectorAddCarryIn, 11, 5, 11, 0),
    (Opcode::VectorCompareGtU64, 0, 14, 2, 0),
    (Opcode::SaveAndMaskExec, 18, 0, 0, 0),
    (Opcode::GlobalStoreDword, 0, 10, 18, 0),
    (Opcode::WaitVm0, 0, 0, 0, 0),
    (Opcode::RestoreExec, 0, 18, 0, 0),
];
pub(crate) fn module() -> Module {
    module_with_registers(false)
}
pub(crate) fn module_with_registers(edited: bool) -> Module {
    let mut regs = [None; 131];
    let mut next = 2u32;
    let results = GFX942_PHYSICAL_ENTRY_REGISTERS_V20
        .into_iter()
        .map(|reg| {
            let id = ValueId(next);
            next += 1;
            regs[reg.state_index().unwrap()] = Some(id);
            ValueDef::new(id, Type::Scalar(reg.scalar_type()))
        })
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    let mut rows = ROWS.to_vec();
    if edited {
        rows[12].1 = 22;
        rows[14].1 = 24;
        rows[15].2 = 24;
        rows[15].3 = 22;
        rows[18].1 = 25;
        rows[19].1 = 25;
        rows[19].2 = 25;
        rows[20].1 = 26;
        rows[20].2 = 25;
        rows[28].3 = 26;
    }
    for (ordinal, (opcode, d, a, b, immediate)) in rows.into_iter().enumerate() {
        let instruction = Gfx942PhysicalLdsExchangeInstructionV1 {
            opcode,
            destination: d,
            source0: a,
            source1: b,
            immediate,
        };
        instruction.validate_shape().unwrap();
        let operands = instruction.operand_registers().map(|reg| {
            reg.map(|reg| regs[reg.state_index().unwrap()].expect("fixture current unit"))
        });
        let mut results = Vec::new();
        for reg in instruction.result_registers().into_iter().flatten() {
            let id = ValueId(next);
            next += 1;
            results.push(ValueDef::new(id, Type::Scalar(reg.scalar_type())));
            regs[reg.state_index().unwrap()] = Some(id);
        }
        block.operations.push(Operation::new(
            results,
            OperationKind::Gfx942PhysicalLdsExchangeStep(Gfx942PhysicalLdsExchangeStepV1 {
                site: site(ordinal as u8 + 2),
                native_ordinal: ordinal as u8,
                instruction,
                operands,
            }),
        ));
    }
    let native = block.operations.len() as u8 + 1;
    let declaration = Gfx942PhysicalLdsExchangeDeclarationV1 {
        origin: Gfx942PhysicalEntryOriginVNext {
            root_axes: [[1; 32]; 5],
            mir_body: [2; 32],
            source_signature: [3; 32],
            rustc_fn_abi: [4; 32],
            frontend_bytes_sha256: [5; 32],
        },
        begin_site: site(0),
        parameters: [ValueId(0), ValueId(1)],
        native_instruction_count: native,
        workgroup: [128, 1, 1],
        maximum_workgroups: [1, 1, 1],
        lds_frame: Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1,
        },
        block: Gfx942PhysicalEntryBlockContractVNext {
            label: 0,
            label_site: site(1),
            encoding: Gfx942PhysicalEntryBranchEncodingVNext::Endpgm0,
            terminator_site: site(native + 1),
            native_ordinal: Some(native - 1),
        },
    };
    block.operations.insert(
        0,
        Operation::new(
            results,
            OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration),
        ),
    );
    block.terminator = Some(Terminator::Return { values: vec![] });
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut function = Function::kernel_entry(
        "physical_lds_exchange_fixture",
        Signature::new(
            vec![
                Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly),
                Type::slice(u32_ty, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "physical_lds_exchange_fixture",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(128, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("inert_physical_lds_exchange_fixture");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}
