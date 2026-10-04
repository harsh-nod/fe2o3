//! Diagnostic compiler-owned ABI/index setup and one authored compute/store tail.
//! This deliberately is NOT a naked or fully author-controlled entry contract.
use super::profile::{Profile, Result};
use fe2o3_kernel_ir::{
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role,
};
use std::fmt::Write;
const LIMIT: usize = 16 * 1024;
fn register(profile: &Profile<'_>, role: Role) -> u8 {
    let plan = profile.program.registers();
    match role {
        Role::Input0 => plan.inputs()[0],
        Role::Input1 => plan.inputs()[1],
        Role::Input2 => plan.inputs()[2],
        Role::Scratch => plan.scratch(),
        Role::Output => plan.output(),
    }
}
pub fn emit(profile: &Profile<'_>) -> Result<String> {
    let mut body = String::with_capacity(4096);
    for instruction in profile.program.program().instructions() {
        match instruction {
            Instruction::Move {
                destination,
                source,
            } => {
                writeln!(
                    body,
                    "v_mov_b32_e32 v{}, v{}",
                    register(profile, destination.role()),
                    register(profile, source)
                )
                .unwrap();
            }
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
                writeln!(
                    body,
                    "{mnemonic} v{}, v{}, v{}",
                    register(profile, destination.role()),
                    register(profile, left),
                    register(profile, right)
                )
                .unwrap();
            }
        }
    }
    // All setup registers are fixed by this diagnostic lowering, not user ABI
    // assertions. The compiler must materialize the nine typed input constraints.
    // Address arithmetic is unsigned modulo 64 bits like non-inbounds GEP.
    // Host address representability remains an explicit external premise.
    body.push_str("v_lshlrev_b64 v[2:3], 2, v[0:1]\n");
    body.push_str("v_add_co_u32_e32 v2, vcc, s16, v2\n");
    // Carry-in VCC already consumes the scalar constant bus on gfx942.
    // Pointer-high is therefore a compiler-materialized VGPR input (v4).
    body.push_str("v_addc_co_u32_e32 v3, vcc, v4, v3, vcc\n");
    body.push_str("v_cmp_gt_u64_e32 vcc, s[18:19], v[0:1]\n");
    body.push_str("s_and_saveexec_b64 s[20:21], vcc\n");
    writeln!(
        body,
        "global_store_dword v[2:3], v{}, off",
        profile.program.registers().output()
    )
    .unwrap();
    body.push_str("s_waitcnt vmcnt(0)\n");
    body.push_str("s_mov_b64 exec, s[20:21]\n");
    body.push_str("s_endpgm");
    let escaped = body.replace('\n', "\\0A\\09");
    let plan = profile.program.registers();
    // EXEC has no net clobber: its original value is saved in declared scratch
    // and restored unconditionally inside this single opaque unit. Declaring
    // the reserved EXEC register clobbered would ask LLVM to preserve it.
    let constraints = format!(
        "{{s16}},{{v4}},{{s18}},{{s19}},{{v{}}},{{v{}}},{{v{}}},{{v0}},{{v1}},~{{v2}},~{{v3}},~{{v{}}},~{{v{}}},~{{s20}},~{{s21}},~{{vcc}},~{{scc}},~{{memory}}",
        plan.inputs()[0],
        plan.inputs()[1],
        plan.inputs()[2],
        plan.scratch(),
        plan.output()
    );
    let text = format!(
        r#"; diagnostic-source-body-gfx942-v1
; compiler-owned ABI/index setup; no native functional execution or publication authority
target triple = "amdgcn-amd-amdhsa"
target datalayout = "{layout}"
declare i32 @llvm.amdgcn.workgroup.id.x() #1
declare i32 @llvm.amdgcn.workitem.id.x() #1
define amdgpu_kernel void @{symbol}(ptr addrspace(1) %data, i64 %length, i32 %a, i32 %b, i32 %c) #0 !reqd_work_group_size !0 {{
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
  call void asm sideeffect "{escaped}", "{constraints}"(i32 %ptr_lo, i32 %ptr_hi, i32 %len_lo, i32 %len_hi, i32 %a, i32 %b, i32 %c, i32 %idx_lo, i32 %idx_hi)
  unreachable
}}
attributes #0 = {{ nounwind "amdgpu-flat-work-group-size"="64,64" "target-cpu"="gfx942" "target-features"="-xnack,-wavefrontsize32,+wavefrontsize64" }}
attributes #1 = {{ nounwind readnone speculatable willreturn }}
!0 = !{{i32 64, i32 1, i32 1}}
"#,
        layout = fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
        symbol = profile.symbol
    );
    if text.len() > LIMIT {
        return Err("LLVM text bound");
    }
    Ok(text)
}
