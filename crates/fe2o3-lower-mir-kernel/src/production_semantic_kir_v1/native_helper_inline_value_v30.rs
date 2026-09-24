// Independent actual-KIR interpretation of the six source-authored u32 ISA values.
// The enclosing native context joins every occurrence to retained source first.
// This leaf neither rewrites the operation nor treats NoMemory as permission to move it.
fn native_helper_inline_value_v30(
    operation: &Operation,
    rows: &[Binding<'_>],
    output: &mut Template,
    meter: &mut dyn Meter,
) -> Result<Value, Error> {
    use fe2o3_kernel_ir::{
        AssemblyOption, Gfx942InlineAssemblyInstructionV1 as Instruction,
        validate_gfx942_inline_assembly_v1,
    };
    let OperationKind::InlineAssembly(assembly) = &operation.kind else {
        return Err("native helper expected an inline instruction");
    };
    meter.work(
        assembly
            .mnemonic
            .len()
            .checked_add(192)
            .ok_or("native helper inline validation work overflow")?,
    )?;
    if assembly.options.len() != 1
        || !assembly.options.contains(&AssemblyOption::NoMemory)
        || !assembly.declared_effects.is_empty()
        || assembly.operands.len() > 3
    {
        return Err("native helper inline options or effects changed");
    }
    // The shared validator accepts Fn, so prepay its at-most-two indexed lookups.
    lookup_work(rows.len(), meter)?;
    lookup_work(rows.len(), meter)?;
    let validated = validate_gfx942_inline_assembly_v1(operation, |value| {
        rows.binary_search_by_key(&value, |row| row.id)
            .ok()
            .and_then(|index| rows[index].ty.as_scalar())
    })
    .map_err(|_| "native helper inline operation violates the gfx942 contract")?;
    if validated.scalar_type() != ScalarType::U32 || !validated.instruction().has_source_macro() {
        return Err("native helper inline instruction is outside the six u32 source forms");
    }
    let left = read(rows, validated.inputs()[0], meter)?;
    let operation = match validated.instruction() {
        Instruction::VMovB32 => return Ok(left),
        Instruction::VAddU32 => ProductionSemanticBinaryOpV2::Add,
        Instruction::VSubU32 => ProductionSemanticBinaryOpV2::Subtract,
        Instruction::VAndB32 => ProductionSemanticBinaryOpV2::BitAnd,
        Instruction::VOrB32 => ProductionSemanticBinaryOpV2::BitOr,
        Instruction::VXorB32 => ProductionSemanticBinaryOpV2::BitXor,
        Instruction::SMovB32 => {
            return Err("native helper scalar-register form has no source contract");
        }
    };
    let right = read(rows, validated.inputs()[1], meter)?;
    output.push(
        Scalar::Integer {
            signed: false,
            bits: 32,
        },
        Kind::Binary(
            operation,
            ProductionOverflowContractV2::Wrapping,
            left,
            right,
        ),
        meter,
    )
}
