//! Bounded literal rendering of each actual canonical step and CFG terminator.
use super::*;
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding, Gfx942PhysicalEntryRegisterV20 as Register,
    Gfx942PhysicalLdsExchangeInstructionV1 as Instruction,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, OperationKind, Terminator,
    gfx942_physical_lds_exchange_declaration_v22,
};
use std::fmt::Write as _;
type EmitError = Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22;
fn invalid(message: &'static str) -> EmitError {
    EmitError::Profile(message)
}
pub(super) struct Text {
    pub(super) value: String,
    cap: usize,
    lines: u16,
}
struct Count(usize);
impl fmt::Write for Count {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
impl Text {
    pub(super) fn new(cap: usize) -> Result<Self> {
        let mut value = String::new();
        value
            .try_reserve_exact(cap)
            .map_err(|_| Resource::Allocation)?;
        if value.capacity() != cap {
            return Err(Resource::Allocation.into());
        }
        Ok(Self {
            value,
            cap,
            lines: 0,
        })
    }
    pub(super) fn format(&mut self, args: fmt::Arguments<'_>) -> Result<()> {
        let mut count = Count(0);
        fmt::write(&mut count, args).map_err(|_| EmitError::TextLimit)?;
        if self
            .value
            .len()
            .checked_add(count.0)
            .is_none_or(|n| n > self.cap)
        {
            return Err(EmitError::TextLimit);
        }
        self.value
            .write_fmt(args)
            .map_err(|_| EmitError::TextLimit)?;
        Ok(())
    }
    fn line(&mut self, args: fmt::Arguments<'_>) -> Result<u16> {
        let ordinal = self.lines;
        self.format(format_args!("{args}\n"))?;
        self.lines = self.lines.checked_add(1).ok_or(EmitError::TextLimit)?;
        Ok(ordinal)
    }
}
fn instruction(text: &mut Text, value: Instruction) -> Result<u16> {
    let Instruction {
        opcode,
        destination: d,
        source0: a,
        source1: b,
        immediate: i,
    } = value;
    match opcode {
        Opcode::LoadKernargPair => {
            text.line(format_args!("s_load_dwordx2 s[{d}:{}], s[0:1], {i}", d + 1))
        }
        Opcode::WaitLgkm0 => text.line(format_args!("s_waitcnt lgkmcnt(0)")),
        Opcode::ScalarLshl32 => text.line(format_args!("s_lshl_b32 s{d}, s{a}, {i}")),
        Opcode::VectorAddU32 => text.line(format_args!("v_add_u32_e32 v{d}, s{a}, v{b}")),
        Opcode::VectorMove32 if a == 255 => text.line(format_args!("v_mov_b32_e32 v{d}, 0")),
        Opcode::VectorMove32 => text.line(format_args!("v_mov_b32_e32 v{d}, s{a}")),
        Opcode::VectorLshlrev64 => text.line(format_args!(
            "v_lshlrev_b64 v[{d}:{}], {i}, v[{a}:{}]",
            d + 1,
            a + 1
        )),
        Opcode::VectorAddCarry => text.line(format_args!("v_add_co_u32_e32 v{d}, vcc, s{a}, v{b}")),
        Opcode::VectorAddCarryIn => {
            text.line(format_args!("v_addc_co_u32_e32 v{d}, vcc, v{a}, v{b}, vcc"))
        }
        Opcode::VectorCompareGtU64 => text.line(format_args!(
            "v_cmp_gt_u64_e32 vcc, s[{a}:{}], v[{b}:{}]",
            a + 1,
            b + 1
        )),
        Opcode::SaveAndMaskExec => {
            text.line(format_args!("s_and_saveexec_b64 s[{d}:{}], vcc", d + 1))
        }
        Opcode::GlobalLoadDword => text.line(format_args!(
            "global_load_dword v{d}, v[{a}:{}], off",
            a + 1
        )),
        Opcode::GlobalStoreDword => text.line(format_args!(
            "global_store_dword v[{a}:{}], v{b}, off",
            a + 1
        )),
        Opcode::WaitVm0 => text.line(format_args!("s_waitcnt vmcnt(0)")),
        Opcode::RestoreExec => text.line(format_args!("s_mov_b64 exec, s[{a}:{}]", a + 1)),
        Opcode::VectorLshlrev32 => text.line(format_args!("v_lshlrev_b32_e32 v{d}, {i}, v{a}")),
        Opcode::VectorXor32 => text.line(format_args!("v_xor_b32_e32 v{d}, {i}, v{a}")),
        Opcode::LdsWriteB32 => text.line(format_args!("ds_write_b32 v{a}, v{b}")),
        Opcode::LdsReadB32 => text.line(format_args!("ds_read_b32 v{d}, v{a}")),
        Opcode::WorkgroupPublishBarrier => text.line(format_args!("s_barrier")),
        Opcode::Endpgm0 => Err(invalid("physical terminator opcode stored as operation")),
    }
}
fn result_ids(operation: &fe2o3_kernel_ir::Operation) -> Result<[Option<ValueId>; 5]> {
    if operation.results.len() > 5 {
        return Err(invalid("physical result correspondence bound"));
    }
    let mut values = [None; 5];
    for (slot, value) in values.iter_mut().zip(&operation.results) {
        *slot = Some(value.id)
    }
    Ok(values)
}
pub(super) const ABSENCE_ATTRIBUTES: [&str; 14] = [
    "amdgpu-no-dispatch-ptr",
    "amdgpu-no-queue-ptr",
    "amdgpu-no-dispatch-id",
    "amdgpu-no-hostcall-ptr",
    "amdgpu-no-multigrid-sync-arg",
    "amdgpu-no-heap-ptr",
    "amdgpu-no-default-queue",
    "amdgpu-no-completion-action",
    "amdgpu-no-workgroup-id-y",
    "amdgpu-no-workgroup-id-z",
    "amdgpu-no-cluster-id-y",
    "amdgpu-no-cluster-id-z",
    "amdgpu-no-workitem-id-y",
    "amdgpu-no-workitem-id-z",
];
fn llvm(
    symbol: &str,
    assembly: &str,
    clobbers: &[bool; 131],
    frame: &Gfx942PhysicalLdsExchangeFrameV1,
) -> Result<String> {
    // Only called with this immutable verified owner's exact frame. The
    // internal backend reservation is not accepted as public source text.
    frame
        .validate_shape()
        .map_err(|_| invalid("physical LDS frame"))?;
    let mut llvm = Text::new(GFX942_PHYSICAL_LDS_EXCHANGE_LLVM_BYTES_V22)?;
    llvm.format(format_args!(r#"; Exact canonical V22 physical-lds-exchange body; inert until normal worker qualification.
target datalayout = "{layout}"
target triple = "amdgcn-amd-amdhsa"
define amdgpu_kernel void @{symbol}(ptr addrspace(1) %input_data, i64 %input_length, ptr addrspace(1) %output_data, i64 %output_length) #0 !reqd_work_group_size !0 {{
entry:
  call void asm sideeffect ""#,
        layout=fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1))?;
    for line in assembly.lines() {
        llvm.format(format_args!("{line}\\0A\\09"))?;
    }
    llvm.format(format_args!("\", \""))?;
    let mut separator = "";
    // SGPR/VGPR units then VCC/SCC, the independently qualified fixture order.
    for (unit, written) in clobbers[..128].iter().enumerate() {
        if !written {
            continue;
        }
        let (bank, index) = if unit < 64 {
            ("s", unit)
        } else {
            ("v", unit - 64)
        };
        llvm.format(format_args!("{separator}~{{{bank}{index}}}"))?;
        separator = ",";
    }
    for (unit, name) in [(129, "vcc"), (128, "scc")] {
        if clobbers[unit] {
            llvm.format(format_args!("{separator}~{{{name}}}"))?;
            separator = ",";
        }
    }
    llvm.format(format_args!(
        "{separator}~{{memory}}\"()\n  unreachable\n}}\n"
    ))?;
    llvm.format(format_args!(r#"attributes #0 = {{ nounwind "target-cpu"="gfx942" "target-features"="-xnack,-wavefrontsize32,+wavefrontsize64" "amdgpu-flat-work-group-size"="128,128" "amdgpu-max-num-workgroups"="1,1,1""#))?;
    llvm.format(format_args!(
        r#" "amdgpu-lds-size"="{0},{0}""#,
        frame.byte_length
    ))?;
    for attribute in ABSENCE_ATTRIBUTES {
        llvm.format(format_args!(" \"{attribute}\""))?;
    }
    llvm.format(format_args!(
        r#" }}
!0 = !{{i32 128, i32 1, i32 1}}
!llvm.module.flags = !{{!1}}
!1 = !{{i32 1, !"amdhsa_code_object_version", i32 600}}
"#
    ))?;
    Ok(llvm.value)
}
pub(super) fn render(
    owner: &VerifiedCanonicalKernelIrModuleV22,
) -> Result<Gfx942PhysicalLdsExchangeCanonicalEmissionV22> {
    let declaration = gfx942_physical_lds_exchange_declaration_v22(owner)
        .ok_or(invalid("owner has no exact physical-lds-exchange profile"))?;
    let [function] = owner.module().functions.as_slice() else {
        return Err(invalid("one physical entry"));
    };
    let body = function
        .body
        .as_ref()
        .ok_or(invalid("physical lds-exchange body absent"))?;
    // Membership is from the same immutable owner already whole-verified;
    // no source bytes, replacement graph, caller plan or re-admission is used.
    let mut assembly = Text::new(GFX942_PHYSICAL_LDS_EXCHANGE_ASSEMBLY_BYTES_V22)?;
    let mut operations = [None; 40];
    let mut blocks = [None; 1];
    let mut clobbers = [false; 131];
    let mut count = 0usize;
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        if block_ordinal != 0 {
            return Err(invalid("LDS exchange exact one block"));
        }
        let contract = &declaration.block;
        let label_line = assembly.line(format_args!(
            ".Lfe2o3_lds_exchange_${{:uid}}_b{}:",
            block.id.0
        ))?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let (site, descriptor, native, line) = match &operation.kind {
                OperationKind::Gfx942PhysicalLdsExchangeDeclaration(value) => {
                    (value.begin_site, None, None, None)
                }
                OperationKind::Gfx942PhysicalLdsExchangeStep(step) => {
                    let line = instruction(&mut assembly, step.instruction)?;
                    for register in step.instruction.result_registers().into_iter().flatten() {
                        if register != Register::Exec {
                            let unit = register
                                .state_index()
                                .ok_or(invalid("physical clobber register bound"))?;
                            clobbers[unit] = true;
                        }
                    }
                    (
                        step.site,
                        Some(step.instruction.descriptor()),
                        Some(step.native_ordinal),
                        Some(line),
                    )
                }
                _ => return Err(invalid("unexpected operation in sealed physical owner")),
            };
            let slot = operations
                .get_mut(count)
                .ok_or(invalid("physical operation correspondence bound"))?;
            *slot = Some(Gfx942PhysicalLdsExchangeOperationCorrespondenceV22 {
                block: block.id,
                operation_ordinal: u8::try_from(ordinal)
                    .map_err(|_| invalid("operation ordinal"))?,
                source_site: site,
                results: result_ids(operation)?,
                instruction_descriptor: descriptor,
                native_ordinal: native,
                assembly_line: line,
            });
            count += 1;
        }
        let terminal = match (&block.terminator, contract.encoding) {
            (Some(Terminator::Return { .. }), Encoding::Endpgm0) => {
                Some(assembly.line(format_args!("s_endpgm"))?)
            }
            _ => return Err(invalid("physical actual CFG/encoding relation")),
        };
        blocks[block_ordinal] = Some(Gfx942PhysicalLdsExchangeBlockCorrespondenceV22 {
            block: block.id,
            authored_label: contract.label,
            label_site: contract.label_site,
            terminator_site: contract.terminator_site,
            encoding: contract.encoding,
            assembly_label_line: label_line,
            terminator_assembly_line: terminal,
            native_ordinal: contract.native_ordinal,
        });
    }
    let llvm = llvm(
        function.id.as_str(),
        &assembly.value,
        &clobbers,
        &declaration.lds_frame,
    )?;
    Ok(Gfx942PhysicalLdsExchangeCanonicalEmissionV22 {
        identity: *owner.identity(),
        llvm,
        assembly: assembly.value,
        operations,
        blocks,
        clobbered_register_units: clobbers,
        lds_frame: declaration.lds_frame,
    })
}
#[cfg(test)]
mod bounded_text_tests {
    use super::*;
    #[test]
    fn rejected_appends_do_not_extend_exact_capacity() {
        let mut text = Text::new(4).unwrap();
        text.format(format_args!("abcd")).unwrap();
        assert_eq!(text.format(format_args!("e")), Err(EmitError::TextLimit));
        assert_eq!(text.value, "abcd");
        assert_eq!(text.value.capacity(), 4);
        let mut text = Text::new(2).unwrap();
        assert_eq!(text.line(format_args!("a")), Ok(0));
        assert_eq!(text.line(format_args!("")), Err(EmitError::TextLimit));
        assert_eq!(text.lines, 1);
    }
}
