//! Exact-owner entry and closed emission for one direct-root ordered program.
//! LLVM text is not source authentication, a final descriptor, or artifact authority.

use super::*;
use fe2o3_kernel_ir::{
    Gfx942OrderedProgramV1, Gfx942ProgramBinaryOpcodeV1, Gfx942ProgramInstructionV1,
    Gfx942ProgramRoleV1, VerifiedCanonicalKernelIrModuleV17, validate_gfx942_ordered_program_v1,
};

#[cfg(test)]
#[path = "ordered_program_v17_tests.rs"]
mod tests;

/// Lowers the exact retained V17 executable using the existing complete-module
/// engine, never a replacement graph or a V12 conversion. The first profile is
/// one direct-root region on an unconditional acyclic entry prefix, required
/// workgroup64x1x1, explicit wave64 and exact gfx942:xnack- target binding.
/// Module headers use the existing reviewed LLVM22 worker data layout, not
/// rustc's distinct frontend layout. Legacy lowering entry points are unchanged.
///
/// Surrounding ordinary operations remain in the same LLVM module. The returned
/// inert text grants no final machine encoding, resource lifetime, source
/// correspondence, semantic-anchor or protected finalizer authority. In particular,
/// the VGPR high-water comment is a necessary binding extent, not a descriptor check.
pub fn lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(
    owner: &VerifiedCanonicalKernelIrModuleV17,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_with_ordered_program_context_v17(
        owner.module(),
        LoweringTarget::Gfx942XnackMinusV1,
        None,
        None,
        true,
        Some(owner),
    )
}

pub(super) fn validate_owner_context(
    module: &Module,
    target: LoweringTarget,
    owner: &VerifiedCanonicalKernelIrModuleV17,
) -> Result<(), LoweringErrors> {
    if !std::ptr::eq(module, owner.module()) || target != LoweringTarget::Gfx942XnackMinusV1 {
        return Err(reject(
            LoweringLocation::module(module),
            "ordered-program lowering requires the actual V17 owner and exact gfx942:xnack- target",
        ));
    }
    let [kernel] = module.kernels.as_slice() else {
        return Err(reject(
            LoweringLocation::module(module),
            "initial ordered-program profile requires exactly one kernel root",
        ));
    };
    let mut count = 0_usize;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if !matches!(operation.kind, OperationKind::Gfx942OrderedProgram(_)) {
                    continue;
                }
                count += 1;
                if count > 1
                    || function.id != kernel.entry
                    || function.role != FunctionRole::KernelEntry
                {
                    return Err(reject(
                        LoweringLocation::device_operation(module, function, block.id, ordinal),
                        "initial ordered-program profile allows only one direct kernel-root region",
                    ));
                }
                for capabilities in [
                    &module.required_capabilities,
                    &kernel.required_capabilities,
                    &function.required_capabilities,
                ] {
                    if capabilities.iter().any(|capability| matches!(capability, TargetCapability::WaveWidth(width) if *width != WaveWidth::Wave64))
                        || operation.required_capabilities().iter().any(|required| !capabilities.contains(required))
                    {
                        return Err(reject(LoweringLocation::device_operation(module, function, block.id, ordinal), "ordered program requires all three profile capabilities on module, kernel and entry without a conflicting wave declaration"));
                    }
                }
            }
        }
    }
    if count != 1 {
        return Err(reject(
            LoweringLocation::module(module),
            "ordered-program owner entry requires exactly one region",
        ));
    }
    Ok(())
}

fn reject(location: LoweringLocation, message: &'static str) -> LoweringErrors {
    LoweringErrors::one(
        location,
        LoweringDiagnosticCode::UnsupportedInlineAssembly,
        message,
    )
}

impl FunctionLowerer<'_> {
    pub(super) fn validate_ordered_program_v17(
        &self,
        operation: &Operation,
        location: &LoweringLocation,
    ) -> Result<(), LoweringErrors> {
        if !self.ordered_program_v17
            || self.target != LoweringTarget::Gfx942XnackMinusV1
            || self.kernel.is_none()
            || self.wave_width != Some(WaveWidth::Wave64)
            || self.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        {
            return Err(reject(
                location.clone(),
                "ordered program requires V17 root context, gfx942:xnack-, wave64 and required workgroup64x1x1",
            ));
        }
        validate_gfx942_ordered_program_v1(operation, |value| {
            self.bindings
                .get(&value)
                .and_then(ValueBinding::value)
                .and_then(|(_, ty)| ty.as_scalar())
        })
        .map_err(|error| {
            LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedInlineAssembly,
                format!("invalid ordered program: {error}"),
            )
        })?;
        let target_block = location.block.ok_or_else(|| {
            reject(
                location.clone(),
                "ordered program has no exact block location",
            )
        })?;
        let body = self.body("ordered program requires a retained function body")?;
        let Some(first) = body.blocks.first() else {
            return Err(reject(
                location.clone(),
                "ordered program requires a function entry block",
            ));
        };
        let mut cursor = first.id;
        // The existing bounded CFG index supplies lookups. Requiring one incoming
        // edge on every nonentry prefix block excludes bypasses and backedges,
        // including a later loop back into the region. No recursive walk or map.
        for position in 0..body.blocks.len() {
            if self.control_flow.incoming_edges(cursor).map(<[usize]>::len)
                != Some(usize::from(position != 0))
            {
                return Err(reject(
                    location.clone(),
                    "ordered program entry prefix cannot have bypass or loop edges",
                ));
            }
            if cursor == target_block {
                return Ok(());
            }
            let Some(Terminator::Branch { target, .. }) = &self.block(cursor)?.terminator else {
                return Err(reject(
                    location.clone(),
                    "ordered program must precede conditional control flow on the unconditional entry prefix",
                ));
            };
            cursor = *target;
        }
        Err(reject(
            location.clone(),
            "ordered program is not on the bounded unconditional entry prefix",
        ))
    }

    pub(super) fn emit_ordered_program_v17(
        &self,
        output: &mut dyn fmt::Write,
        operation: &Operation,
        region: &Gfx942OrderedProgramV1,
    ) {
        let registers = region.registers();
        let scratch = registers.scratch();
        let destination = registers.output();
        let [input0, input1, input2] = registers.inputs();
        let [a, b, c] = *region.inputs();
        let (a, _) = self.value(a);
        let (b, _) = self.value(b);
        let (c, _) = self.value(c);
        let result = value_name(operation.results[0].id);
        writeln!(output, "  ; ordered-program-v17 vgpr-high-water={} (binding extent; final descriptor unverified)", registers.vgpr_high_water()).unwrap();
        write!(output, "  {result} = call i32 asm sideeffect \"").unwrap();
        // Iterate the immutable authored order, including dead and self moves.
        // No user text, descriptor bit reinterpretation or intermediate SSA is
        // accepted here; the checked typed instruction controls mnemonic/arity.
        for (index, instruction) in region.program().instructions().enumerate() {
            if index != 0 {
                write!(output, "\\0A\\09").unwrap();
            }
            match instruction {
                Gfx942ProgramInstructionV1::Move {
                    destination,
                    source,
                } => {
                    write!(output, "v_mov_b32_e32 ").unwrap();
                    emit_role(output, destination.role(), scratch);
                    write!(output, ", ").unwrap();
                    emit_role(output, source, scratch);
                }
                Gfx942ProgramInstructionV1::Binary {
                    opcode,
                    destination,
                    left,
                    right,
                } => {
                    let mnemonic = match opcode {
                        Gfx942ProgramBinaryOpcodeV1::Add => "v_add_u32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Subtract => "v_sub_u32_e32",
                        Gfx942ProgramBinaryOpcodeV1::And => "v_and_b32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Or => "v_or_b32_e32",
                        Gfx942ProgramBinaryOpcodeV1::Xor => "v_xor_b32_e32",
                    };
                    write!(output, "{mnemonic} ").unwrap();
                    emit_role(output, destination.role(), scratch);
                    write!(output, ", ").unwrap();
                    emit_role(output, left, scratch);
                    write!(output, ", ").unwrap();
                    emit_role(output, right, scratch);
                }
            }
        }
        writeln!(output, "\", \"=&{{v{destination}}},{{v{input0}}},{{v{input1}}},{{v{input2}}},~{{v{scratch}}}\"(i32 {a}, i32 {b}, i32 {c})").unwrap();
    }
}

fn emit_role(output: &mut dyn fmt::Write, role: Gfx942ProgramRoleV1, scratch: u8) {
    match role {
        Gfx942ProgramRoleV1::Input0 => write!(output, "$1"),
        Gfx942ProgramRoleV1::Input1 => write!(output, "$2"),
        Gfx942ProgramRoleV1::Input2 => write!(output, "$3"),
        Gfx942ProgramRoleV1::Scratch => write!(output, "v{scratch}"),
        Gfx942ProgramRoleV1::Output => write!(output, "$0"),
    }
    .unwrap();
}
