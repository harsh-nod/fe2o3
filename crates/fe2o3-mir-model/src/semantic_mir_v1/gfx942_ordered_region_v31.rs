//! Closed inert V31 ordered-region descriptions. This is V30's reviewed grammar
//! plus one profile, never V29 execution capabilities. Source references establish
//! structural consistency only; the source owner must authenticate occurrences.
//! Physical bindings are literal operands of a call, not shared callee identity.

use super::*;

/// One fixed gfx942 xnack-off wave64 sequence: XOR e32 then wrapping ADD e32.
/// Target/launch/root placement and physical execution remain contextual checks.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticGfx942OrderedRegionProfileV31 {
    XorAddU32E32,
}

impl SemanticGfx942OrderedRegionProfileV31 {
    pub(super) const fn wire_tag(self) -> u8 {
        match self {
            Self::XorAddU32E32 => 0,
        }
    }

    pub(super) const fn from_wire_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::XorAddU32E32),
            _ => None,
        }
    }
}

/// Checked local bindings, not a register file or lifetime/occupancy proof.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGfx942OrderedRegionRegistersV31 {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
}

impl SemanticGfx942OrderedRegionRegistersV31 {
    pub fn new(scratch: u8, output: u8, inputs: [u8; 3]) -> Result<Self, SemanticMirErrorV1> {
        let indices = [scratch, output, inputs[0], inputs[1], inputs[2]];
        for (position, index) in indices.into_iter().enumerate() {
            if index > 63 || indices[..position].contains(&index) {
                return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
            }
        }
        Ok(Self {
            scratch,
            output,
            inputs,
        })
    }

    pub const fn scratch(self) -> u8 {
        self.scratch
    }

    pub const fn output(self) -> u8 {
        self.output
    }

    pub const fn inputs(self) -> [u8; 3] {
        self.inputs
    }

    /// Register-index high water only; not complete kernel resource usage.
    pub const fn vgpr_high_water(self) -> u8 {
        let mut maximum = self.scratch;
        if self.output > maximum {
            maximum = self.output;
        }
        let mut position = 0;
        while position < self.inputs.len() {
            if self.inputs[position] > maximum {
                maximum = self.inputs[position];
            }
            position += 1;
        }
        maximum + 1
    }
}

/// Four nonzero inert references for one call occurrence, never callee authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticOrderedRegionSourceV31 {
    frontend_unit: [u8; 32],
    function: SemanticFunctionIdentityV1,
    contract: [u8; 32],
    statement: [u8; 32],
}

impl SemanticOrderedRegionSourceV31 {
    pub fn new(
        frontend_unit: [u8; 32],
        function: SemanticFunctionIdentityV1,
        contract: [u8; 32],
        statement: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        if [frontend_unit, *function.as_bytes(), contract, statement].contains(&[0; 32]) {
            return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
        }
        Ok(Self {
            frontend_unit,
            function,
            contract,
            statement,
        })
    }

    pub const fn frontend_unit(self) -> [u8; 32] {
        self.frontend_unit
    }

    pub const fn function(self) -> SemanticFunctionIdentityV1 {
        self.function
    }

    pub const fn contract(self) -> [u8; 32] {
        self.contract
    }

    pub const fn statement(self) -> [u8; 32] {
        self.statement
    }
}

/// A checked borrow of one actual call in an admitted inert graph. Not a source
/// occurrence authenticator or a semantic-to-KIR lowering authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticGfx942OrderedRegionCallV31<'a> {
    profile: SemanticGfx942OrderedRegionProfileV31,
    source: SemanticOrderedRegionSourceV31,
    registers: SemanticGfx942OrderedRegionRegistersV31,
    inputs: &'a [SemanticOperandV1; 3],
}

impl<'a> SemanticGfx942OrderedRegionCallV31<'a> {
    pub const fn profile(self) -> SemanticGfx942OrderedRegionProfileV31 {
        self.profile
    }

    pub const fn source(self) -> SemanticOrderedRegionSourceV31 {
        self.source
    }

    pub const fn registers(self) -> SemanticGfx942OrderedRegionRegistersV31 {
        self.registers
    }

    pub const fn inputs(self) -> &'a [SemanticOperandV1; 3] {
        self.inputs
    }
}

impl AdmittedInertSemanticMirV1 {
    /// Selects from this immutable graph, not a caller-supplied replacement call.
    /// This validates inert shape/caller binding only. Target/root custody and
    /// occurrence replay must still be checked by the production source owner.
    pub fn checked_gfx942_ordered_region_call_v31(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    ) -> Result<SemanticGfx942OrderedRegionCallV31<'_>, SemanticMirErrorV1> {
        if self.wire_version != SemanticMirWireVersionV1::V31 {
            return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
        }
        let function = self
            .request
            .functions
            .get(function.0 as usize)
            .ok_or(SemanticMirErrorV1::InvalidOrderedRegionV31)?;
        let block = function
            .blocks
            .get(block.0 as usize)
            .ok_or(SemanticMirErrorV1::InvalidOrderedRegionV31)?;
        let SemanticTerminatorKindV1::Call(call) = &block.terminator.kind else {
            return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
        };
        checked_call(&self.request, function, call)
    }
}

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.c_variadic()
        && !abi.can_unwind()
        && inputs.len() == 8
        && inputs[..3].iter().all(|input| *input == output)
        && is_unsigned_integer_with_bits(request, output, 32)
        && inputs[3..].iter().all(|input| *input == inputs[3])
        && is_unsigned_integer_with_bits(request, inputs[3], 8)
        && abi.arguments().len() == 8
        && abi
            .arguments()
            .iter()
            .all(|argument| matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_)))
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
}

fn checked_call<'a>(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
) -> Result<SemanticGfx942OrderedRegionCallV31<'a>, SemanticMirErrorV1> {
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(profile),
        binding,
        ..
    }) = request.callables.get(call.callee.0 as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
    };
    let source = call
        .ordered_region_source_v31
        .ok_or(SemanticMirErrorV1::InvalidOrderedRegionV31)?;
    if call.inline_assembly_source_v30.is_some()
        || call.ordered_program_source_v32.is_some()
        || source.function != function.identity
        || !signature_matches(request, &binding.abi)
        || !call.variadic_argument_abis.is_empty()
        || call.arguments.len() != 8
        || !matches!(
            call.unwind,
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
    {
        return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
    }
    let abi_inputs = binding.abi.source_input_types();
    if call
        .arguments
        .iter()
        .zip(abi_inputs)
        .any(|(argument, expected)| argument.ty() != *expected)
    {
        return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
    }
    let inputs = call.arguments[..3]
        .try_into()
        .map_err(|_| SemanticMirErrorV1::InvalidOrderedRegionV31)?;
    let mut physical = [0_u8; 5];
    for (position, operand) in call.arguments[3..].iter().enumerate() {
        let SemanticOperandV1::Constant(SemanticConstantV1 {
            value: SemanticConstantValueV1::Scalar(value),
            ..
        }) = operand
        else {
            return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
        };
        if value.size_bytes() != 1 {
            return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
        }
        physical[position] =
            u8::try_from(value.bits()).map_err(|_| SemanticMirErrorV1::InvalidOrderedRegionV31)?;
    }
    let registers = SemanticGfx942OrderedRegionRegistersV31::new(
        physical[0],
        physical[1],
        [physical[2], physical[3], physical[4]],
    )?;
    Ok(SemanticGfx942OrderedRegionCallV31 {
        profile: *profile,
        source,
        registers,
        inputs,
    })
}

pub(super) fn validate_call_source(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    let region = matches!(
        request.callables.get(call.callee.0 as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(_),
            ..
        })
    );
    if region {
        checked_call(request, function, call).map(|_| ())
    } else if call.ordered_region_source_v31.is_some() {
        Err(SemanticMirErrorV1::InvalidOrderedRegionV31)
    } else {
        Ok(())
    }
}

pub(super) fn uses_v31(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(_),
                ..
            }
        )
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
                if call.ordered_region_source_v31.is_some())
        })
    })
}

#[cfg(test)]
#[path = "gfx942_ordered_region_v31_tests.rs"]
mod tests;
