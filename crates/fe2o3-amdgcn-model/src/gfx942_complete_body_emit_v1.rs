//! Fixed ordinary AMDGPU shell and checked typed assembly, no text extension.
use super::*;
use crate::{Gfx942CompleteBodyBoundaryV1, Gfx942CompleteBodyResourcesV1};
use fe2o3_kernel_ir::{
    Gfx942CompleteBodyTerminatorV1 as Terminator, Gfx942ProgramBinaryOpcodeV1 as Opcode,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role,
};
use std::fmt::Write as _;

type Error = Gfx942CompleteBodyEmissionErrorV1;
type Result<T> = std::result::Result<T, Error>;

/// Request the fixed text allowance and refuse allocator capacity rounding;
/// each append checks logical bytes before extending the string. No temporary
/// replacement/format string is allocated.
struct Text {
    value: String,
    kind: Gfx942CompleteBodyTextKindV1,
    cap: usize,
    lines: u16,
}
impl Text {
    fn new(kind: Gfx942CompleteBodyTextKindV1, cap: usize) -> Result<Self> {
        let mut value = String::new();
        value
            .try_reserve_exact(cap)
            .map_err(|_| Error::Allocation)?;
        if value.capacity() != cap {
            return Err(Error::Allocation);
        }
        Ok(Self {
            value,
            kind,
            cap,
            lines: 0,
        })
    }
    fn format(&mut self, value: fmt::Arguments<'_>) -> Result<()> {
        self.write_fmt(value)
            .map_err(|_| Error::TextLimit(self.kind))
    }
    fn line(&mut self, value: fmt::Arguments<'_>) -> Result<u16> {
        let index = self.lines;
        self.format(value)?;
        self.format(format_args!("\n"))?;
        self.lines = self.lines.checked_add(1).ok_or(Error::PlanInvariant)?;
        Ok(index)
    }
}
impl fmt::Write for Text {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .value
            .len()
            .checked_add(value.len())
            .is_none_or(|size| size > self.cap)
        {
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
}

fn register(registers: Gfx942OrderedProgramRegistersV1, role: Role) -> u8 {
    match role {
        Role::Input0 => registers.inputs()[0],
        Role::Input1 => registers.inputs()[1],
        Role::Input2 => registers.inputs()[2],
        Role::Scratch => registers.scratch(),
        Role::Output => registers.output(),
    }
}
fn arithmetic(
    body: &mut Text,
    registers: Gfx942OrderedProgramRegistersV1,
    instruction: Instruction,
) -> Result<u16> {
    match instruction {
        Instruction::Move {
            destination,
            source,
        } => body.line(format_args!(
            "v_mov_b32_e32 v{}, v{}",
            register(registers, destination.role()),
            register(registers, source)
        )),
        Instruction::Binary {
            opcode,
            destination,
            left,
            right,
        } => {
            let mnemonic = match opcode {
                Opcode::Add => "v_add_u32_e32",
                Opcode::Subtract => "v_sub_u32_e32",
                Opcode::And => "v_and_b32_e32",
                Opcode::Or => "v_or_b32_e32",
                Opcode::Xor => "v_xor_b32_e32",
            };
            body.line(format_args!(
                "{mnemonic} v{}, v{}, v{}",
                register(registers, destination.role()),
                register(registers, left),
                register(registers, right)
            ))
        }
    }
}
fn target(plan: &Gfx942CompleteBodyPlanV1, label: Gfx942CompleteBodyLabelV1) -> Result<u8> {
    (0..plan.block_count())
        .find(|ordinal| {
            plan.block(*ordinal)
                .is_some_and(|block| block.label == label)
        })
        .map(|ordinal| ordinal as u8)
        .ok_or(Error::PlanInvariant)
}
fn compiler_tail(body: &mut Text, output: u8) -> Result<Gfx942CompleteBodyEmittedTailV1> {
    let label_line = body.line(format_args!(".Lfe2o3_cb_${{:uid}}_tail:"))?;
    let first_instruction_line = body.lines;
    // Exactly the established ordinary-shell tail, including pointer-high v4:
    // VCC carry-in already consumes the gfx942 scalar constant bus.
    body.line(format_args!("v_lshlrev_b64 v[2:3], 2, v[0:1]"))?;
    body.line(format_args!("v_add_co_u32_e32 v2, vcc, s16, v2"))?;
    body.line(format_args!("v_addc_co_u32_e32 v3, vcc, v4, v3, vcc"))?;
    body.line(format_args!("v_cmp_gt_u64_e32 vcc, s[18:19], v[0:1]"))?;
    body.line(format_args!("s_and_saveexec_b64 s[20:21], vcc"))?;
    body.line(format_args!("global_store_dword v[2:3], v{output}, off"))?;
    body.line(format_args!("s_waitcnt vmcnt(0)"))?;
    body.line(format_args!("s_mov_b64 exec, s[20:21]"))?;
    body.line(format_args!("s_endpgm"))?;
    Ok(Gfx942CompleteBodyEmittedTailV1 {
        label_line,
        first_instruction_line,
        instruction_count: 9,
    })
}

pub(super) fn render(
    plan: &Gfx942CompleteBodyPlanV1,
    symbol: Gfx942CompleteBodySymbolV1<'_>,
) -> Result<Gfx942CompleteBodyEmissionV1> {
    let registers = plan.registers();
    if plan.boundary() != Gfx942CompleteBodyBoundaryV1::PROFILE
        || plan.resources() != Gfx942CompleteBodyResourcesV1::required(registers)
        || !(1..=8).contains(&plan.block_count())
        || !(1..=16).contains(&plan.instruction_count())
    {
        return Err(Error::PlanInvariant);
    }
    let mut body = Text::new(
        Gfx942CompleteBodyTextKindV1::Assembly,
        GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1,
    )?;
    let mut blocks = [None; 8];
    let mut instructions = [None; 16];
    let mut step = 0_usize;
    for (ordinal, slot) in blocks.iter_mut().enumerate().take(plan.block_count()) {
        let block = plan.block(ordinal).ok_or(Error::PlanInvariant)?;
        let label_line = body.line(format_args!(".Lfe2o3_cb_${{:uid}}_b{ordinal}:"))?;
        let first_instruction_ordinal = step as u8;
        for (local, instruction) in block.instructions.iter().copied().enumerate() {
            let assembly_line = arithmetic(&mut body, registers, instruction)?;
            *instructions.get_mut(step).ok_or(Error::PlanInvariant)? =
                Some(Gfx942CompleteBodyEmittedInstructionV1 {
                    ordinal: step as u8,
                    block_ordinal: ordinal as u8,
                    instruction_in_block: local as u8,
                    descriptor: instruction.descriptor(),
                    assembly_line,
                });
            step += 1;
        }
        let terminator_line = body.lines;
        let (terminator, terminator_line_count) = match block.terminator {
            Terminator::Jump(label) => {
                let destination = target(plan, label)?;
                body.line(format_args!("s_branch .Lfe2o3_cb_${{:uid}}_b{destination}"))?;
                (
                    Gfx942CompleteBodyLoweredTerminatorV1::Jump {
                        target_ordinal: destination,
                    },
                    1,
                )
            }
            Terminator::BranchSelectorZero { zero, nonzero } => {
                let zero_ordinal = target(plan, zero)?;
                let nonzero_ordinal = target(plan, nonzero)?;
                body.line(format_args!("s_cmp_eq_u32 s22, 0"))?;
                body.line(format_args!(
                    "s_cbranch_scc1 .Lfe2o3_cb_${{:uid}}_b{zero_ordinal}"
                ))?;
                body.line(format_args!(
                    "s_branch .Lfe2o3_cb_${{:uid}}_b{nonzero_ordinal}"
                ))?;
                (
                    Gfx942CompleteBodyLoweredTerminatorV1::BranchSelectorZero {
                        zero_ordinal,
                        nonzero_ordinal,
                    },
                    3,
                )
            }
            Terminator::GuardedStoreOutputAndEnd => {
                body.line(format_args!("s_branch .Lfe2o3_cb_${{:uid}}_tail"))?;
                (Gfx942CompleteBodyLoweredTerminatorV1::CompilerTail, 1)
            }
        };
        *slot = Some(Gfx942CompleteBodyEmittedBlockV1 {
            ordinal: ordinal as u8,
            label: block.label,
            label_line,
            first_instruction_ordinal,
            instruction_count: block.instructions.len() as u8,
            terminator_line,
            terminator_line_count,
            terminator,
        });
    }
    if step != plan.instruction_count() {
        return Err(Error::PlanInvariant);
    }
    let tail = compiler_tail(&mut body, registers.output())?;
    let mut llvm = Text::new(
        Gfx942CompleteBodyTextKindV1::Llvm,
        GFX942_COMPLETE_BODY_LLVM_BYTES_V1,
    )?;
    llvm.format(format_args!(
        r#"; diagnostic-complete-body-llvm-mechanism-v1
; checked inert intent only; no source/native/proof/publication authority
target triple = "amdgcn-amd-amdhsa"
target datalayout = "{layout}"
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.amdgcn.workitem.id.x() #1
define amdgpu_kernel void @{symbol}(ptr addrspace(1) %data, i64 %length, i32 %a, i32 %b, i32 %c, i32 %selector) #0 !reqd_work_group_size !0 {{
entry:
  %ptr = ptrtoint ptr addrspace(1) %data to i64
  %ptr_lo = trunc i64 %ptr to i32
  %ptr_shift = lshr i64 %ptr, 32
  %ptr_hi = trunc i64 %ptr_shift to i32
  %len_lo = trunc i64 %length to i32
  %len_shift = lshr i64 %length, 32
  %len_hi = trunc i64 %len_shift to i32
  %local32 = call i32 @llvm.amdgcn.workitem.id.x()
  %group32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %local = zext i32 %local32 to i64
  %group = zext i32 %group32 to i64
  %base = mul i64 %group, 64
  %index = add i64 %base, %local
  %idx_lo = trunc i64 %index to i32
  %idx_shift = lshr i64 %index, 32
  %idx_hi = trunc i64 %idx_shift to i32
  call void asm sideeffect ""#,
        layout = fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
        symbol = symbol.as_str(),
    ))?;
    // Assembly consists solely of fixed literals, typed numeric registers and
    // generated local-label ordinals. Escape line separators incrementally.
    for line in body.value.lines() {
        llvm.format(format_args!("{line}\\0A\\09"))?;
    }
    // EXEC is saved/restored in the opaque unit, not a net clobber. All authored
    // registers are distinct v8..v63, so fixed ABI inputs/clobbers cannot alias.
    llvm.format(format_args!(
        r#"", "{{s16}},{{v4}},{{s18}},{{s19}},{{v{a}}},{{v{b}}},{{v{c}}},{{v0}},{{v1}},{{s22}},~{{v2}},~{{v3}},~{{v{scratch}}},~{{v{output}}},~{{s20}},~{{s21}},~{{vcc}},~{{scc}},~{{memory}}"(i32 %ptr_lo, i32 %ptr_hi, i32 %len_lo, i32 %len_hi, i32 %a, i32 %b, i32 %c, i32 %idx_lo, i32 %idx_hi, i32 %selector)
  unreachable
}}
attributes #0 = {{ nounwind "amdgpu-flat-work-group-size"="64,64" "target-cpu"="gfx942" "target-features"="-xnack,-wavefrontsize32,+wavefrontsize64" }}
attributes #1 = {{ nounwind readnone speculatable willreturn }}
!0 = !{{i32 64, i32 1, i32 1}}
"#,
        a = registers.inputs()[0], b = registers.inputs()[1], c = registers.inputs()[2],
        scratch = registers.scratch(), output = registers.output(),
    ))?;
    Ok(Gfx942CompleteBodyEmissionV1 {
        llvm: llvm.value,
        assembly: body.value,
        registers,
        blocks,
        instructions,
        tail,
    })
}

#[cfg(test)]
mod text_tests {
    use super::*;
    #[test]
    fn exact_cap_and_rejected_append_do_not_extend_text() {
        let mut text = Text::new(Gfx942CompleteBodyTextKindV1::Assembly, 4).unwrap();
        text.format(format_args!("abcd")).unwrap();
        assert_eq!(text.value, "abcd");
        assert_eq!(
            text.format(format_args!("e")),
            Err(Error::TextLimit(Gfx942CompleteBodyTextKindV1::Assembly))
        );
        assert_eq!(text.value, "abcd");
    }
    #[test]
    fn utf8_accounting_is_bytes_not_character_count() {
        let mut text = Text::new(Gfx942CompleteBodyTextKindV1::Llvm, 3).unwrap();
        text.format(format_args!("é")).unwrap();
        assert_eq!(
            text.format(format_args!("é")),
            Err(Error::TextLimit(Gfx942CompleteBodyTextKindV1::Llvm))
        );
        assert_eq!(text.value, "é");
        text.format(format_args!("x")).unwrap();
        assert_eq!(text.value.len(), 3);
    }
    #[test]
    fn line_separator_consumes_the_same_fixed_allowance() {
        let mut text = Text::new(Gfx942CompleteBodyTextKindV1::Assembly, 2).unwrap();
        assert_eq!(text.line(format_args!("a")), Ok(0));
        assert_eq!(text.value, "a\n");
        assert_eq!(
            text.line(format_args!("")),
            Err(Error::TextLimit(Gfx942CompleteBodyTextKindV1::Assembly))
        );
        assert_eq!(text.lines, 1);
        assert_eq!(text.value, "a\n");
    }
}
