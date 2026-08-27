use crate::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
    CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiDirectionV1, DeviceTargetV1, derive_device_ffi_contract_id_v1,
};

const PRODUCTION_GFX942_TARGET_V1: &str = "gfx942:xnack-";
const PRODUCTION_GFX942_OCML_EXP_OWNER_CRATE_V1: &str = "rustc_codegen_fe2o3";
const PRODUCTION_GFX942_OCML_EXP_OWNER_PATH_V1: &str =
    "rustc_codegen_fe2o3::production_gfx942::__ocml_exp_f32";
const PRODUCTION_GFX942_OCML_EXP_INSTANCE_V1: &str =
    "__fe2o3_compiler_owned_production_gfx942_ocml_exp_f32_v1";
const PRODUCTION_GFX942_OCML_EXP_OWNER_HASH_V1: [u8; 16] = [0x94; 16];

/// The sole device-library symbol admitted by the production gfx942 compiler route.
pub const PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1: &str = "__ocml_exp_f32";
/// Exact physical ABI of the production gfx942 OCML exponential boundary.
pub const PRODUCTION_GFX942_OCML_EXP_F32_ABI_V1: &str =
    "C(f32[size=4,align=4])->f32[size=4,align=4]";
/// OCML exp has no compiler-visible memory effects.
pub const PRODUCTION_GFX942_OCML_EXP_F32_EFFECTS_V1: &str = "none";

/// Exact admitted production gfx942 compiler-FFI envelope shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionGfx942CompilerFfiEnvelopeKindV1 {
    /// The module has no device FFI.
    NoDeviceFfi,
    /// The module has exactly the compiler-owned OCML exp import.
    OcmlExpF32 {
        /// Version-domain-separated digest of authoritative canonical Kernel IR bytes.
        canonical_kernel_ir_identity: [u8; 32],
    },
}

/// Constructs the sole compiler-owned device import admitted by production gfx942.
///
/// The semantic identity must be copied from an authenticated canonical Kernel IR owner.
/// This function selects no OCML provider and grants no link or artifact authority.
pub fn construct_production_gfx942_ocml_exp_envelope_v1(
    canonical_kernel_ir_identity: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, CompilerFfiEnvelopeError> {
    let target = DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET_V1)
        .expect("fixed production gfx942 target is canonical");
    let semantic_text = lower_hex(&canonical_kernel_ir_identity);
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol: PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1,
        calling_convention: "C",
        code_object_version: 6,
        target: PRODUCTION_GFX942_TARGET_V1,
        physical_abi: PRODUCTION_GFX942_OCML_EXP_F32_ABI_V1,
        effects: PRODUCTION_GFX942_OCML_EXP_F32_EFFECTS_V1,
        semantic_identity: &semantic_text,
    };
    let contract = CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Import,
        CompilerFfiLinkRoleV1::RequiresExternalDefinition,
        target,
        CodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            PRODUCTION_GFX942_OCML_EXP_OWNER_CRATE_V1,
            PRODUCTION_GFX942_OCML_EXP_OWNER_PATH_V1,
            PRODUCTION_GFX942_OCML_EXP_OWNER_HASH_V1,
            PRODUCTION_GFX942_OCML_EXP_INSTANCE_V1,
        )?,
        PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1,
        PRODUCTION_GFX942_OCML_EXP_F32_ABI_V1,
        PRODUCTION_GFX942_OCML_EXP_F32_EFFECTS_V1,
        canonical_kernel_ir_identity,
    )?;
    let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V6, 1)?;
    builder.push(contract)?;
    builder.finish()
}

/// Classifies an already canonical envelope under the exact production gfx942 policy.
pub fn inspect_production_gfx942_compiler_ffi_envelope_v1(
    envelope: &CompilerFfiEnvelopeV1,
) -> Option<ProductionGfx942CompilerFfiEnvelopeKindV1> {
    if envelope.target().to_string() != PRODUCTION_GFX942_TARGET_V1
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
            .then_some(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi);
    }
    if directional.import_count() != 1
        || directional.export_count() != 0
        || !directional
            .imports()
            .eq([PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1])
    {
        return None;
    }
    let identity = *directional.import_semantic_identities().next()?;
    let expected = construct_production_gfx942_ocml_exp_envelope_v1(identity).ok()?;
    (expected.canonical_bytes() == envelope.canonical_bytes()).then_some(
        ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
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
    use crate::{
        construct_production_gfx950_ocml_exp_envelope_v1,
        inspect_production_gfx950_compiler_ffi_envelope_v1,
    };

    #[test]
    fn exact_ocml_exp_envelope_round_trips_through_policy_inspection() {
        let identity = [0x37; 32];
        let envelope = construct_production_gfx942_ocml_exp_envelope_v1(identity).unwrap();
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity: identity,
            })
        );
        assert_eq!(
            envelope.directional_symbols().imports().collect::<Vec<_>>(),
            [PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1]
        );
    }

    #[test]
    fn no_device_ffi_is_a_distinct_exact_gfx942_shape() {
        let target = DeviceTargetV1::parse(PRODUCTION_GFX942_TARGET_V1).unwrap();
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap();
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi)
        );
    }

    #[test]
    fn target_specific_ocml_envelopes_are_not_interchangeable() {
        let identity = [0x39; 32];
        let gfx942 = construct_production_gfx942_ocml_exp_envelope_v1(identity).unwrap();
        let gfx950 = construct_production_gfx950_ocml_exp_envelope_v1(identity).unwrap();
        assert_eq!(
            inspect_production_gfx950_compiler_ffi_envelope_v1(&gfx942),
            None
        );
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&gfx950),
            None
        );
        assert_ne!(gfx942.canonical_bytes(), gfx950.canonical_bytes());
    }
}
