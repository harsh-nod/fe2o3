use crate::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
    CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiDirectionV1, DeviceTargetV1, derive_device_ffi_contract_id_v1,
};

const PRODUCTION_GFX950_TARGET_V1: &str = "gfx950:xnack-";
const PRODUCTION_GFX950_OCML_EXP_OWNER_CRATE_V1: &str = "rustc_codegen_fe2o3";
const PRODUCTION_GFX950_OCML_EXP_OWNER_PATH_V1: &str =
    "rustc_codegen_fe2o3::production_gfx950::__ocml_exp_f32";
const PRODUCTION_GFX950_OCML_EXP_INSTANCE_V1: &str =
    "__fe2o3_compiler_owned_production_gfx950_ocml_exp_f32_v1";
const PRODUCTION_GFX950_OCML_EXP_OWNER_HASH_V1: [u8; 16] = [0x95; 16];

/// The sole device-library symbol admitted by the production gfx950 compiler route.
pub const PRODUCTION_GFX950_OCML_EXP_F32_SYMBOL_V1: &str = "__ocml_exp_f32";
/// Exact physical ABI of the production gfx950 OCML exponential boundary.
pub const PRODUCTION_GFX950_OCML_EXP_F32_ABI_V1: &str =
    "C(f32[size=4,align=4])->f32[size=4,align=4]";
/// OCML exp has no compiler-visible memory effects.
pub const PRODUCTION_GFX950_OCML_EXP_F32_EFFECTS_V1: &str = "none";

/// Exact admitted production gfx950 compiler-FFI envelope shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionGfx950CompilerFfiEnvelopeKindV1 {
    /// The module has no device FFI. This is the required GEMM shape.
    NoDeviceFfi,
    /// The module has exactly the compiler-owned OCML exp import.
    OcmlExpF32 {
        /// Version-domain-separated digest of authoritative canonical Kernel IR bytes.
        canonical_kernel_ir_identity: [u8; 32],
    },
}

/// Constructs the sole compiler-owned device import admitted by production gfx950.
///
/// The semantic identity must be copied from an authenticated canonical Kernel IR owner.
/// This function selects no OCML provider and grants no link or artifact authority.
pub fn construct_production_gfx950_ocml_exp_envelope_v1(
    canonical_kernel_ir_identity: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
    let target = DeviceTargetV1::parse(PRODUCTION_GFX950_TARGET_V1)
        .expect("fixed production gfx950 target is canonical");
    let semantic_text = lower_hex(&canonical_kernel_ir_identity);
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol: PRODUCTION_GFX950_OCML_EXP_F32_SYMBOL_V1,
        calling_convention: "C",
        code_object_version: 6,
        target: PRODUCTION_GFX950_TARGET_V1,
        physical_abi: PRODUCTION_GFX950_OCML_EXP_F32_ABI_V1,
        effects: PRODUCTION_GFX950_OCML_EXP_F32_EFFECTS_V1,
        semantic_identity: &semantic_text,
    };
    let contract = CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Import,
        CompilerFfiLinkRoleV1::RequiresExternalDefinition,
        target,
        CodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            PRODUCTION_GFX950_OCML_EXP_OWNER_CRATE_V1,
            PRODUCTION_GFX950_OCML_EXP_OWNER_PATH_V1,
            PRODUCTION_GFX950_OCML_EXP_OWNER_HASH_V1,
            PRODUCTION_GFX950_OCML_EXP_INSTANCE_V1,
        )?,
        PRODUCTION_GFX950_OCML_EXP_F32_SYMBOL_V1,
        PRODUCTION_GFX950_OCML_EXP_F32_ABI_V1,
        PRODUCTION_GFX950_OCML_EXP_F32_EFFECTS_V1,
        canonical_kernel_ir_identity,
    )?;
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V6, 1)?;
    builder.push(contract)?;
    builder.finish()
}

/// Classifies an already canonical envelope under the exact production gfx950 policy.
///
/// For the OCML case, every source-owner, target, COV, ABI, effect, role, and symbol byte is
/// rederived and compared. The returned semantic identity remains inert; only the production
/// compiler can associate it with authenticated canonical Kernel IR custody.
pub fn inspect_production_gfx950_compiler_ffi_envelope_v1(
    envelope: &CompilerFfiEnvelopeV1,
) -> Option<ProductionGfx950CompilerFfiEnvelopeKindV1> {
    if envelope.target().to_string() != PRODUCTION_GFX950_TARGET_V1
        || envelope.code_object_version() != CodeObjectVersion::V6
    {
        return None;
    }
    let directional = envelope.directional_symbols();
    if directional.total_count() == 0 {
        let expected = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            envelope.target(),
            envelope.code_object_version(),
        )
        .ok()?;
        return (expected.canonical_bytes() == envelope.canonical_bytes())
            .then_some(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi);
    }
    if directional.import_count() != 1
        || directional.export_count() != 0
        || !directional
            .imports()
            .eq([PRODUCTION_GFX950_OCML_EXP_F32_SYMBOL_V1])
    {
        return None;
    }
    let identity = *directional.import_semantic_identities().next()?;
    let expected = construct_production_gfx950_ocml_exp_envelope_v1(identity).ok()?;
    (expected.canonical_bytes() == envelope.canonical_bytes()).then_some(
        ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 {
            canonical_kernel_ir_identity: identity,
        },
    )
}

fn lower_hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_ocml_exp_envelope_round_trips_through_policy_inspection() {
        let identity = [0x38; 32];
        let envelope = construct_production_gfx950_ocml_exp_envelope_v1(identity).unwrap();
        assert_eq!(
            inspect_production_gfx950_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity: identity,
            })
        );
        assert_eq!(
            envelope.directional_symbols().imports().collect::<Vec<_>>(),
            [PRODUCTION_GFX950_OCML_EXP_F32_SYMBOL_V1]
        );
    }

    #[test]
    fn no_device_ffi_is_a_distinct_exact_gfx950_shape() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX950_TARGET_V1).unwrap();
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap();
        assert_eq!(
            inspect_production_gfx950_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi)
        );
    }
}
