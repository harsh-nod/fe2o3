//! Physical tag operations, never initialized-value or active-view authority.

use super::storage_operations_v18::space_v18;
use super::*;
use fe2o3_kernel_ir::{
    MemoryAccess, StorageLayoutKindV1 as Kind, StorageVariantEncodingV1 as Encoding,
};

fn mask(bits: u32) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}

pub(super) fn emit_logical_discriminant_selection(
    context: &super::storage_native_v18::StorageEmissionContextV18<'_>,
    output: &mut dyn fmt::Write,
    encoding: Encoding,
    variants: &[fe2o3_kernel_ir::StorageVariantV1],
    bits: u32,
    prefix: &str,
) -> Result<usize, LoweringErrors> {
    // Prepay the complete roster, including uninhabited alternatives. The table
    // remains the original owner's borrowed table; no ordinal is a logical tag.
    context.charge(variants.len())?;
    let mut previous = None;
    for (index, variant) in variants.iter().enumerate() {
        if variant.uninhabited { continue; }
        match encoding {
            Encoding::Direct { .. } => {
                let bits_value = variant.direct_tag_bits.ok_or_else(|| context.reject("direct tag bits are absent"))?;
                writeln!(output, "  {prefix}.match{index} = icmp eq i{bits} {prefix}.tag, {bits_value}").unwrap();
            }
            Encoding::Niche { untagged_variant, first_niche_variant, last_niche_variant, .. } => {
                let index32 = u32::try_from(index).map_err(|_| context.reject("variant index overflow"))?;
                if index32 == untagged_variant {
                    writeln!(output, "  {prefix}.match{index} = icmp ugt i{bits} {prefix}.delta, {}", last_niche_variant - first_niche_variant).unwrap();
                } else {
                    let delta = index32.checked_sub(first_niche_variant)
                        .filter(|_| index32 <= last_niche_variant)
                        .ok_or_else(|| context.reject("variant is outside its niche encoding"))?;
                    writeln!(output, "  {prefix}.match{index} = icmp eq i{bits} {prefix}.delta, {delta}").unwrap();
                }
            }
        }
        if let Some(before) = previous {
            writeln!(output, "  {prefix}.valid{index} = or i1 {prefix}.valid{before}, {prefix}.match{index}").unwrap();
            writeln!(output, "  {prefix}.logical{index} = select i1 {prefix}.match{index}, i128 {}, i128 {prefix}.logical{before}", variant.discriminant).unwrap();
        } else {
            writeln!(output, "  {prefix}.valid{index} = or i1 false, {prefix}.match{index}").unwrap();
            writeln!(output, "  {prefix}.logical{index} = select i1 {prefix}.match{index}, i128 {}, i128 0", variant.discriminant).unwrap();
        }
        previous = Some(index);
    }
    previous.ok_or_else(|| context.reject("ReadDiscriminant has no inhabited variant"))
}

impl FunctionLowerer<'_> {
    pub(super) fn emit_storage_read_discriminant(
        &self,
        output: &mut dyn fmt::Write,
        block: BlockId,
        ordinal: usize,
        address: ValueId,
        access: MemoryAccess,
        result: &str,
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let (pointer, row) = self.storage_address(address)?;
        let Kind::Variants { encoding, variants } = &row.kind else {
            return Err(context.reject("ReadDiscriminant has no enum row"));
        };
        if access.volatile { return Err(context.reject("volatile discriminant reads are not qualified")); }
        let tag = encoding.tag();
        let tag_row = context.row(tag.layout)?;
        match tag_row.kind {
            Kind::Scalar(_) => {}
            Kind::Pointer(pointer) if matches!(encoding, Encoding::Niche { .. }) => {
                storage_v1::pointer_encoding(self.target, pointer)
                    .map_err(|_| context.reject("pointer niche has no exact LLVM target encoding"))?;
            }
            _ => return Err(context.reject("discriminant tag has no scalar or pointer encoding")),
        }
        let bits = u32::try_from(tag_row.size.checked_mul(8)
            .ok_or_else(|| context.reject("tag width overflow"))?)
            .map_err(|_| context.reject("tag width overflow"))?;
        if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
            return Err(context.reject("unsupported physical tag width"));
        }
        let base = self.value(address).0;
        let space = space_v18(pointer.address_space);
        writeln!(output, "  {prefix}.tag.address = getelementptr i8, ptr addrspace({space}) {base}, i64 {}", tag.offset).unwrap();
        writeln!(output, "  {prefix}.tag = load i{bits}, ptr addrspace({space}) {prefix}.tag.address, align {}", access.alignment).unwrap();
        if let Encoding::Niche { niche_start, .. } = *encoding {
            writeln!(output, "  {prefix}.delta = sub i{bits} {prefix}.tag, {}", niche_start & mask(bits)).unwrap();
        }
        let final_index = emit_logical_discriminant_selection(context, output, *encoding, variants, bits, prefix)?;
        let condition = self.storage_name(format_args!("{prefix}.valid{final_index}"))?;
        self.storage_check_branch(output, block, ordinal, condition.as_str());
        writeln!(output, "  {result} = add i128 {prefix}.logical{final_index}, 0").unwrap();
        Ok(())
    }

    pub(super) fn emit_storage_variant(
        &self,
        output: &mut dyn fmt::Write,
        block: BlockId,
        ordinal: usize,
        address: ValueId,
        variant: u32,
        access: MemoryAccess,
        result: &str,
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let (pointer, row) = self.storage_address(address)?;
        let Kind::Variants { encoding, variants } = &row.kind else {
            return Err(context.reject("active projection has no enum row"));
        };
        let selected = variants
            .get(variant as usize)
            .filter(|v| !v.uninhabited)
            .ok_or_else(|| context.reject("active projection names an invalid variant"))?;
        let tag = encoding.tag();
        let tag_row = context.row(tag.layout)?;
        match tag_row.kind {
            Kind::Scalar(_) => {}
            Kind::Pointer(pointer) if matches!(encoding, Encoding::Niche { .. }) => {
                storage_v1::pointer_encoding(self.target, pointer)
                    .map_err(|_| context.reject("pointer niche has no exact LLVM target encoding"))?;
            }
            _ => return Err(context.reject("variant tag has no scalar or pointer encoding")),
        }
        let bits = u32::try_from(
            tag_row
                .size
                .checked_mul(8)
                .ok_or_else(|| context.reject("tag width overflow"))?,
        )
        .map_err(|_| context.reject("tag width overflow"))?;
        if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
            return Err(context.reject("unsupported physical tag width"));
        }
        let base = self.value(address).0;
        let space = space_v18(pointer.address_space);
        let volatile = if access.volatile { "volatile " } else { "" };
        writeln!(
            output,
            "  {prefix}.tag.address = getelementptr i8, ptr addrspace({space}) {base}, i64 {}",
            tag.offset
        )
        .unwrap();
        writeln!(output, "  {prefix}.tag = load {volatile}i{bits}, ptr addrspace({space}) {prefix}.tag.address, align {}", access.alignment).unwrap();
        match *encoding {
            Encoding::Direct { .. } => writeln!(
                output,
                "  {prefix}.active = icmp eq i{bits} {prefix}.tag, {}",
                selected
                    .direct_tag_bits
                    .ok_or_else(|| context.reject("direct tag bits are absent"))?
            )
            .unwrap(),
            Encoding::Niche {
                untagged_variant,
                first_niche_variant,
                last_niche_variant,
                niche_start,
                ..
            } => {
                writeln!(
                    output,
                    "  {prefix}.delta = sub i{bits} {prefix}.tag, {}",
                    niche_start & mask(bits)
                )
                .unwrap();
                if variant == untagged_variant {
                    writeln!(
                        output,
                        "  {prefix}.active = icmp ugt i{bits} {prefix}.delta, {}",
                        last_niche_variant - first_niche_variant
                    )
                    .unwrap();
                } else {
                    let delta = variant
                        .checked_sub(first_niche_variant)
                        .filter(|_| variant <= last_niche_variant)
                        .ok_or_else(|| context.reject("variant is outside its niche encoding"))?;
                    writeln!(
                        output,
                        "  {prefix}.active = icmp eq i{bits} {prefix}.delta, {delta}"
                    )
                    .unwrap();
                }
            }
        }
        let condition = self.storage_name(format_args!("{prefix}.active"))?;
        self.storage_check_branch(output, block, ordinal, condition.as_str());
        writeln!(
            output,
            "  {result} = getelementptr i8, ptr addrspace({space}) {base}, i64 0"
        )
        .unwrap();
        Ok(())
    }

    pub(super) fn emit_storage_discriminant(
        &self,
        output: &mut dyn fmt::Write,
        address: ValueId,
        variant: u32,
        access: MemoryAccess,
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let (pointer, row) = self.storage_address(address)?;
        let Kind::Variants { encoding, variants } = &row.kind else {
            return Err(context.reject("SetDiscriminant has no enum row"));
        };
        let selected = variants
            .get(variant as usize)
            .filter(|v| !v.uninhabited)
            .ok_or_else(|| context.reject("SetDiscriminant names an invalid variant"))?;
        if access.volatile {
            return Err(
                context.reject("volatile discriminant updates are not in the source contract")
            );
        }
        if matches!(*encoding, Encoding::Niche { untagged_variant, .. } if untagged_variant == variant)
        {
            return Ok(());
        }
        let tag = encoding.tag();
        let tag_row = context.row(tag.layout)?;
        let bits = u32::try_from(
            tag_row
                .size
                .checked_mul(8)
                .ok_or_else(|| context.reject("tag width overflow"))?,
        )
        .map_err(|_| context.reject("tag width overflow"))?;
        if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
            return Err(context.reject("unsupported physical tag width"));
        }
        let value = match *encoding {
            Encoding::Direct { .. } => selected
                .direct_tag_bits
                .ok_or_else(|| context.reject("direct tag bits are absent"))?,
            Encoding::Niche {
                first_niche_variant,
                last_niche_variant,
                niche_start,
                ..
            } => {
                let delta = variant
                    .checked_sub(first_niche_variant)
                    .filter(|_| variant <= last_niche_variant)
                    .ok_or_else(|| context.reject("variant is outside its niche encoding"))?;
                niche_start.wrapping_add(u128::from(delta)) & mask(bits)
            }
        };
        let base = self.value(address).0;
        let space = space_v18(pointer.address_space);
        writeln!(
            output,
            "  {prefix}.tag.address = getelementptr i8, ptr addrspace({space}) {base}, i64 {}",
            tag.offset
        )
        .unwrap();
        writeln!(
            output,
            "  store i{bits} {value}, ptr addrspace({space}) {prefix}.tag.address, align {}",
            access.alignment
        )
        .unwrap();
        Ok(())
    }
}

#[cfg(test)]
#[path = "storage_variants_v18_tests.rs"]
mod tests;
