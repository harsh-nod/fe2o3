//! One literal-specialized scalar helper draft, derived from retained typed KIR.
//! This is not the physical ordered-program grammar or compiler admission.
use super::*;
use fe2o3_kernel_ir::Constant;

pub const MAX_CONST_U32_HELPER_SOURCE_BYTES_V1: usize = 4096;

/// One actual u32 constant definition; original_value is observed, never supplied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringConstU32ParameterV1 {
    pub name: &'static str,
    pub value: AuthoringValueV1,
    pub definition: AuthoringOperationCoordinateV1,
    pub original_value: u32,
}

/// Additive diagnostic output. The original region boundary is preserved;
/// runtime_parameters separately describes the generated helper signature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringConstU32CandidateV1 {
    pub schema: &'static str,
    pub authority: AuthoringAuthorityV1,
    pub region: AuthoringRegionV1,
    pub helper_name: String,
    pub source: String,
    pub runtime_parameters: [AuthoringValueV1; 1],
    pub const_parameter: AuthoringConstU32ParameterV1,
    /// Uses generated vN names, not authenticated names in the original Rust.
    pub original_call_template: String,
    pub status: &'static str,
    pub frontend_readmission: &'static str,
    pub source_application: &'static str,
    pub semantic_equivalence: &'static str,
    pub exact_machine_contract: &'static str,
    pub physical_register_bindings: &'static str,
}

impl AuthoringSnapshotV1 {
    /// Draft exactly one u32 AND/OR/XOR with one direct, earlier same-block
    /// Constant::U32 live-in. The other live-in stays a runtime u32 parameter.
    /// No constant propagation, alias following, caller-supplied bits or source
    /// insertion is performed. A specialization change requires fresh Rust input.
    pub fn materialize_const_u32_helper_v1(
        &self,
        selector: &AuthoringRegionSelectorV1,
        helper_name: &str,
    ) -> Result<AuthoringConstU32CandidateV1> {
        if !valid_helper_name(helper_name) {
            return Err(AuthoringErrorV1::InvalidHelperName);
        }
        self.check_selector(selector)?;
        let [coordinate] = selector.operations.as_slice() else {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        };
        let instruction = self
            .draft_instruction(*coordinate, self.operation(*coordinate)?)
            .ok_or(AuthoringErrorV1::UnsupportedMaterialization)?;
        if !matches!(instruction.mnemonic, "v_and_b32" | "v_or_b32" | "v_xor_b32") {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        }
        let [left, right] = instruction.inputs.as_slice() else {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        };
        if left == right {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        }
        let region = self.select_region(selector)?;
        if region.live_in.len() != 2
            || region.live_out.len() != 1
            || region.live_out[0].value != instruction.result.0
            || region.live_in[0].value != left.0
            || region.live_in[1].value != right.0
        {
            return Err(AuthoringErrorV1::InvalidBoundary);
        }
        let left_constant = self.direct_const_u32_v1(*coordinate, *left)?;
        let right_constant = self.direct_const_u32_v1(*coordinate, *right)?;
        let (constant, runtime) = match (left_constant, right_constant) {
            (Some(constant), None) => (constant, self.value_view(coordinate.function, *right)?),
            (None, Some(constant)) => (constant, self.value_view(coordinate.function, *left)?),
            _ => return Err(AuthoringErrorV1::UnsupportedMaterialization),
        };
        let source = render(helper_name, &instruction, &constant)?;
        let mut call = BoundedText::new(256);
        write!(
            call,
            "{helper_name}::<{}u32>(v{})",
            constant.original_value, runtime.value
        )
        .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        bounded_report(AuthoringConstU32CandidateV1 {
            schema: "fe2o3-const-u32-helper-draft-v1",
            authority: AUTHORITY,
            region,
            helper_name: helper_name.to_owned(),
            source,
            runtime_parameters: [runtime],
            const_parameter: constant,
            original_call_template: call.text,
            status: "diagnostic_const_u32_source_draft_only",
            frontend_readmission: "not_performed_requires_fresh_source_compilation",
            source_application: "unavailable_requires_explicit_new_source_and_normal_frontend",
            semantic_equivalence: "unproved",
            exact_machine_contract: "unproved",
            physical_register_bindings: "unavailable_compiler_owned_scalar_values",
        })
    }

    fn direct_const_u32_v1(
        &self,
        selected: AuthoringOperationCoordinateV1,
        value: ValueId,
    ) -> Result<Option<AuthoringConstU32ParameterV1>> {
        let definition = self
            .definitions
            .get(selected.function as usize)
            .and_then(|values| values.get(&value))
            .ok_or(AuthoringErrorV1::InvalidBoundary)?;
        let Definition::Result(block, operation, result_index) = *definition else {
            // Parameters/edge arguments may remain runtime inputs, never constants.
            return Ok(None);
        };
        let coordinate = AuthoringOperationCoordinateV1 {
            function: selected.function,
            block: ordinal(block)?,
            operation: ordinal(operation)?,
        };
        let operation = self.operation(coordinate)?;
        let OperationKind::Constant(constant) = &operation.kind else {
            // Do not follow copies, casts, instruction results or computed aliases.
            return Ok(None);
        };
        let Constant::U32(bits) = constant else {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        };
        if coordinate.block != selected.block || coordinate.operation >= selected.operation {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        }
        let [result] = operation.results.as_slice() else {
            return Err(AuthoringErrorV1::InvalidBoundary);
        };
        if result_index != 0
            || result.id != value
            || result.ty != Type::Scalar(ScalarType::U32)
            || self.value_type(selected.function, value) != Some(&Type::Scalar(ScalarType::U32))
        {
            return Err(AuthoringErrorV1::InvalidBoundary);
        }
        Ok(Some(AuthoringConstU32ParameterV1 {
            name: "C0",
            value: self.value_view(selected.function, value)?,
            definition: coordinate,
            original_value: *bits,
        }))
    }
}

fn render(
    helper: &str,
    instruction: &DraftInstructionV1,
    constant: &AuthoringConstU32ParameterV1,
) -> Result<String> {
    let mut text = BoundedText::new(MAX_CONST_U32_HELPER_SOURCE_BYTES_V1);
    let runtime = instruction
        .inputs
        .iter()
        .find(|value| value.0 != constant.value.value)
        .ok_or(AuthoringErrorV1::InvalidBoundary)?;
    writeln!(
        text,
        "// Diagnostic scalar helper draft; fresh source readmission required."
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    writeln!(
        text,
        "// gfx942:xnack-; physical allocation remains compiler-owned."
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    writeln!(text, "#[inline(never)]").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    writeln!(
        text,
        "pub fn {helper}<const C0: u32>(v{}: u32) -> (u32,) {{",
        runtime.0
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    write!(
        text,
        "    let v{}: u32 = fe2o3_device::amdgpu_asm!({}(",
        instruction.result.0, instruction.mnemonic
    )
    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    for (index, input) in instruction.inputs.iter().enumerate() {
        if index != 0 {
            write!(text, ", ").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
        if input.0 == constant.value.value {
            write!(text, "C0").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        } else {
            write!(text, "v{}", input.0).map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
    }
    writeln!(text, "));\n    (v{},)\n}}", instruction.result.0)
        .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    Ok(text.text)
}
