//! Tag arithmetic shared by current runtime reads and prepared input domains.

use super::*;
use fe2o3_kernel_ir::{StorageVariantEncodingV1, StorageVariantV1};

pub(super) fn storage_raw_variant_v29(
    bits: u128,
    width: usize,
    encoding: StorageVariantEncodingV1,
    variants: &[StorageVariantV1],
    accounting: &StorageAccountingV1,
) -> Result<Option<usize>, SimulationExecutionErrorKindV1> {
    accounting.charge(variants.len())?;
    Ok(match encoding {
        StorageVariantEncodingV1::Direct { .. } => variants
            .iter()
            .position(|variant| !variant.uninhabited && variant.direct_tag_bits == Some(bits)),
        StorageVariantEncodingV1::Niche {
            untagged_variant,
            first_niche_variant,
            last_niche_variant,
            niche_start,
            ..
        } => {
            let relative = bits.wrapping_sub(niche_start) & mask((width * 8) as u16);
            Some(
                if relative <= u128::from(last_niche_variant - first_niche_variant) {
                    first_niche_variant + relative as u32
                } else {
                    untagged_variant
                } as usize,
            )
        }
    })
}

/// Classification of the entire non-null bit domain, never a dereference.
/// Callers must independently establish a complete pointer representation.
pub(super) fn storage_nonnull_pointer_variant_v29(
    recipe: fe2o3_amd_target::AmdPointerEncodingV1,
    encoding: StorageVariantEncodingV1,
    count: usize,
    accounting: &StorageAccountingV1,
) -> Result<usize, SimulationExecutionErrorKindV1> {
    accounting.charge(count)?;
    let StorageVariantEncodingV1::Niche {
        untagged_variant,
        first_niche_variant,
        last_niche_variant,
        niche_start,
        ..
    } = encoding
    else {
        return Err(storage_violation_v1(
            "symbolic pointer tag requires a niche encoding",
        ));
    };
    if first_niche_variant > last_niche_variant
        || last_niche_variant as usize >= count
        || untagged_variant as usize >= count
    {
        return Err(storage_violation_v1(
            "pointer niche differs from its original variant table",
        ));
    }
    for index in 0..count {
        if index < first_niche_variant as usize || index > last_niche_variant as usize {
            continue;
        }
        let bits = niche_start.wrapping_add((index - first_niche_variant as usize) as u128)
            & mask(recipe.bits());
        if recipe.nonnull_domain_contains(bits) {
            return Err(storage_violation_v1(
                "symbolic pointer domain crosses possible logical variants",
            ));
        }
    }
    Ok(untagged_variant as usize)
}
