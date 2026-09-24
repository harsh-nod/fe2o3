//! Inert test-only graph construction from the literal qualified instruction list.
//! No source authentication, numeric GPU addresses, native execution or launch authority.
use crate::*;
type Reg = Gfx942PhysicalEntryRegisterV20;
type Opcode = Gfx942PhysicalEntryOpcodeV20;
type Encoding = Gfx942PhysicalEntryBranchEncodingVNext;
type Contract = Gfx942PhysicalEntryBlockContractVNext;
type Site = Gfx942PhysicalEntrySourceSiteVNext;
struct Builder {
    occurrence: u8,
    native: u8,
    next: u32,
}
impl Builder {
    fn site(&mut self) -> Site {
        let n = self.occurrence;
        self.occurrence += 1;
        Site {
            occurrence: n,
            raw_block: 100 + u32::from(n),
            semantic_block_index: 72 - u32::from(n),
            semantic_block_identity: [n + 1; 32],
            semantic_callable_index: 7,
        }
    }
    fn step(
        &mut self,
        regs: &mut [Option<ValueId>; 131],
        opcode: Opcode,
        d: u8,
        a: u8,
        b: u8,
        immediate: u32,
    ) -> Operation {
        let instruction = Gfx942PhysicalEntryInstructionVNext {
            opcode,
            destination: d,
            source0: a,
            source1: b,
            immediate,
        };
        instruction.validate_shape().unwrap();
        let operands = instruction.operand_registers().map(|reg| {
            reg.map(|reg| regs[reg.state_index().unwrap()].expect("fixture operand definition"))
        });
        let mut results = Vec::new();
        for reg in instruction.result_registers().into_iter().flatten() {
            let id = ValueId(self.next);
            self.next += 1;
            results.push(ValueDef::new(id, Type::Scalar(reg.scalar_type())));
            regs[reg.state_index().unwrap()] = Some(id);
        }
        let site = self.site();
        let native_ordinal = self.native;
        self.native += 1;
        Operation::new(
            results,
            OperationKind::Gfx942PhysicalEntryStep(Gfx942PhysicalEntryStepVNext {
                site,
                native_ordinal,
                instruction,
                operands,
            }),
        )
    }
    fn terminal(&mut self, label: u8, label_site: Site, encoding: Encoding) -> Contract {
        let terminator_site = self.site();
        let native_ordinal = if encoding == Encoding::Fallthrough {
            None
        } else {
            let n = self.native;
            self.native += 1;
            Some(n)
        };
        Contract {
            label,
            label_site,
            encoding,
            terminator_site,
            native_ordinal,
        }
    }
}
fn setup(builder: &mut Builder, regs: &mut [Option<ValueId>; 131]) -> Vec<Operation> {
    [
        (Opcode::LoadKernargPair, 8, 0, 0, 0),
        (Opcode::LoadKernargPair, 10, 0, 0, 8),
        (Opcode::LoadKernargDword, 12, 0, 0, 16),
        (Opcode::LoadKernargDword, 13, 0, 0, 20),
        (Opcode::LoadKernargDword, 14, 0, 0, 24),
        (Opcode::LoadKernargDword, 15, 0, 0, 28),
        (Opcode::WaitLgkm0, 0, 0, 0, 0),
        (Opcode::ScalarLshl32, 16, 2, 0, 6),
        (Opcode::VectorAddU32, 2, 16, 0, 0),
        (Opcode::VectorMove32, 3, 255, 0, 0),
        (Opcode::VectorMove32, 4, 9, 0, 0),
    ]
    .into_iter()
    .map(|(op, d, a, b, i)| builder.step(regs, op, d, a, b, i))
    .collect()
}
fn tail(builder: &mut Builder, regs: &mut [Option<ValueId>; 131]) -> Vec<Operation> {
    [
        (Opcode::VectorLshlrev64, 6, 2, 0, 2),
        (Opcode::VectorAddCarry, 6, 8, 6, 0),
        (Opcode::VectorAddCarryIn, 7, 4, 7, 0),
        (Opcode::VectorCompareGtU64, 0, 10, 2, 0),
        (Opcode::SaveAndMaskExec, 18, 0, 0, 0),
        (Opcode::GlobalStoreDword, 0, 6, 8, 0),
        (Opcode::WaitVm0, 0, 0, 0, 0),
        (Opcode::RestoreExec, 0, 18, 0, 0),
    ]
    .into_iter()
    .map(|(op, d, a, b, i)| builder.step(regs, op, d, a, b, i))
    .collect()
}
pub fn module(select: bool) -> Module {
    let mut builder = Builder {
        occurrence: 0,
        native: 0,
        next: 10,
    };
    let begin_site = builder.site();
    let mut regs = [None; 131];
    let results = GFX942_PHYSICAL_ENTRY_REGISTERS_V20
        .into_iter()
        .enumerate()
        .map(|(index, reg)| {
            let id = ValueId(5 + index as u32);
            regs[reg.state_index().unwrap()] = Some(id);
            ValueDef::new(id, Type::Scalar(reg.scalar_type()))
        })
        .collect();
    let mut contracts = [Contract::ZERO; 4];
    let mut entry = BasicBlock::new(BlockId(0));
    let label = builder.site();
    entry.operations = setup(&mut builder, &mut regs);
    let mut blocks = if !select {
        entry
            .operations
            .push(builder.step(&mut regs, Opcode::VectorMove32, 8, 12, 0, 0));
        entry.operations.extend(tail(&mut builder, &mut regs));
        entry.terminator = Some(Terminator::Return { values: vec![] });
        contracts[0] = builder.terminal(250, label, Encoding::Endpgm0);
        vec![entry]
    } else {
        entry
            .operations
            .push(builder.step(&mut regs, Opcode::ScalarCompareEqZero, 0, 15, 0, 0));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: regs[Reg::Scc.state_index().unwrap()].unwrap(),
            then_target: BlockId(2),
            then_arguments: vec![],
            else_target: BlockId(1),
            else_arguments: vec![],
        });
        contracts[0] = builder.terminal(250, label, Encoding::Scc1);
        let mut nonzero_regs = regs;
        let mut zero_regs = regs;
        let mut nonzero = BasicBlock::new(BlockId(1));
        let label = builder.site();
        nonzero
            .operations
            .push(builder.step(&mut nonzero_regs, Opcode::VectorMove32, 8, 13, 0, 0));
        nonzero.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![nonzero_regs[Reg::Vgpr(8).state_index().unwrap()].unwrap()],
        });
        contracts[1] = builder.terminal(4, label, Encoding::Jump);
        let mut zero = BasicBlock::new(BlockId(2));
        let label = builder.site();
        zero.operations
            .push(builder.step(&mut zero_regs, Opcode::VectorMove32, 8, 12, 0, 0));
        zero.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![zero_regs[Reg::Vgpr(8).state_index().unwrap()].unwrap()],
        });
        contracts[2] = builder.terminal(0, label, Encoding::Fallthrough);
        let mut join = BasicBlock::new(BlockId(3));
        let label = builder.site();
        let phi = ValueId(builder.next);
        builder.next += 1;
        join.parameters
            .push(ValueDef::new(phi, Type::Scalar(ScalarType::U32)));
        regs[Reg::Vgpr(8).state_index().unwrap()] = Some(phi);
        join.operations = tail(&mut builder, &mut regs);
        join.terminator = Some(Terminator::Return { values: vec![] });
        contracts[3] = builder.terminal(7, label, Encoding::Endpgm0);
        vec![entry, nonzero, zero, join]
    };
    let declaration = Gfx942PhysicalEntryDeclarationVNext {
        origin: Gfx942PhysicalEntryOriginVNext {
            root_axes: [[1; 32]; 5],
            mir_body: [2; 32],
            source_signature: [3; 32],
            rustc_fn_abi: [4; 32],
            frontend_bytes_sha256: [5; 32],
        },
        begin_site,
        parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        block_count: blocks.len() as u8,
        native_instruction_count: builder.native,
        workgroup: [64, 1, 1],
        maximum_workgroups: [2, 1, 1],
        blocks: contracts,
    };
    declaration.validate_shape().unwrap();
    blocks[0].operations.insert(
        0,
        Operation::new(
            results,
            OperationKind::Gfx942PhysicalEntryDeclaration(declaration),
        ),
    );
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut function = Function::kernel_entry(
        "physical_fixture",
        Signature::new(
            vec![
                Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                u32_ty.clone(),
                u32_ty.clone(),
                u32_ty.clone(),
                u32_ty,
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        blocks,
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "physical_fixture",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("inert_physical_fixture");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}
