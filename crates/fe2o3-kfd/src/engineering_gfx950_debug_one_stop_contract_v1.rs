//! Fixed source/ordinary-worker artifact profile, not arbitrary HSACO admission.
//! Constants are from the independently qualified unchanged O0/O3 fixture.
//! This content relation is not authenticated Rust-source or protected authority.
use super::E;
use fe2o3_amdhsa_loader::{AdmittedProfile, ValidatedKernelEnvelope};
use fe2o3_hsaco::{
    ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind, HiddenValueKind as H, KernelKind,
};
use sha2::{Digest, Sha256};

pub(super) const SYMBOL: &str = "fe2o3_gfx950_one_stop_fixture";
pub(super) const OBJECT_BYTES: usize = 5312;
pub(super) const OBJECT_SHA256: [u8; 32] = [
    0xcd, 0x3d, 0xaa, 0xb7, 0x6d, 0xb3, 0x47, 0x10, 0x41, 0x66, 0xc8, 0x98, 0xa5, 0x8d, 0xcc, 0x29,
    0xc1, 0x97, 0xee, 0x1e, 0xac, 0x8b, 0x25, 0x8b, 0x10, 0xa2, 0xc7, 0x93, 0xe8, 0x77, 0x9c, 0x87,
];
pub(super) const SOURCE_SHA256: [u8; 32] = [
    0xe5, 0x67, 0xe9, 0xb4, 0x14, 0xf0, 0x08, 0x22, 0xfb, 0xf6, 0x43, 0x69, 0x84, 0xa4, 0x52, 0xd9,
    0x94, 0x06, 0x30, 0xec, 0xce, 0xc4, 0x0a, 0x0e, 0x86, 0xb2, 0xc5, 0xc1, 0x91, 0x59, 0x80, 0x17,
];
pub(super) const ENTRY: [u8; 84] = [
    0, 2, 6, 192, 0, 0, 0, 0, 0, 0, 140, 191, 255, 0, 140, 190, 223, 155, 87, 19, 255, 0, 141, 190,
    224, 172, 104, 36, 0, 3, 4, 126, 128, 2, 6, 126, 4, 0, 143, 210, 130, 4, 2, 0, 9, 2, 12, 126,
    8, 8, 8, 50, 6, 11, 10, 56, 12, 0, 16, 42, 13, 0, 18, 42, 0, 128, 112, 220, 4, 8, 127, 0, 0, 0,
    140, 191, 3, 0, 146, 191, 0, 0, 129, 191,
];
pub(super) const DESCRIPTOR: [u8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 8, 1, 0, 0, 0, 0, 0, 0, 64, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 129, 0, 175, 0, 132, 0, 0, 0, 8, 0, 0, 0, 0,
    0, 0, 0,
];
pub(super) const OUTPUT: usize = 6;
pub(super) const OUTPUT_LOGICAL: usize = 272;
pub(super) const PAGE: usize = 4096;
pub(super) const KERNARG_BYTES: usize = 264;
const HIDDEN: [(u64, u64, H); 13] = [
    (8, 4, H::BlockCountX),
    (12, 4, H::BlockCountY),
    (16, 4, H::BlockCountZ),
    (20, 2, H::GroupSizeX),
    (22, 2, H::GroupSizeY),
    (24, 2, H::GroupSizeZ),
    (26, 2, H::RemainderX),
    (28, 2, H::RemainderY),
    (30, 2, H::RemainderZ),
    (48, 8, H::GlobalOffsetX),
    (56, 8, H::GlobalOffsetY),
    (64, 8, H::GlobalOffsetZ),
    (72, 2, H::GridDimensions),
];
pub(super) fn validate_object(bytes: &[u8]) -> Result<ValidatedKernelEnvelope<'_>, E> {
    if bytes.len() != OBJECT_BYTES || <[u8; 32]>::from(Sha256::digest(bytes)) != OBJECT_SHA256 {
        return Err(E::Contract("fixed one-stop source artifact identity"));
    }
    let inspection = fe2o3_hsaco::inspect(bytes).map_err(|e| E::Native(format!("{e:?}")))?;
    if inspection.code_object_version() != CodeObjectVersion::V6 || inspection.kernels().len() != 1
    {
        return Err(E::Contract("fixed one-stop sole COV6 kernel"));
    }
    let closure = fe2o3_amdhsa_loader::validate(bytes, AdmittedProfile::Gfx950XnackOffCov6)
        .map_err(|e| E::Native(format!("{e:?}")))?
        .bind_kernel(SYMBOL)
        .map_err(|e| E::Native(format!("{e:?}")))?;
    validate_selected(&closure)?;
    Ok(closure)
}
fn validate_selected(c: &ValidatedKernelEnvelope<'_>) -> Result<(), E> {
    let k = c.selected_kernel();
    let b = c.selected_binding();
    let d = b.descriptor();
    let plan = c.envelope().plan();
    use fe2o3_amdhsa_loader::SegmentPermissions as P;
    let segments = [
        (0, 2048, 0, 2048, 0, 4096, P::ReadOnly),
        (2048, 1152, 6144, 1152, 4096, 4096, P::ReadExecute),
        (3200, 128, 11392, 896, 8192, 4096, P::ReadWrite),
    ];
    if plan.input_len() != OBJECT_BYTES as u64
        || plan.image_start() != 0
        || plan.image_end() != 12288
        || plan.segments().len() != 3
        || c.envelope().materialization().image_len() != 12288
    {
        return Err(E::Contract("fixed one-stop complete load-image envelope"));
    }
    for (actual, expected) in plan.segments().iter().zip(segments) {
        if (
            actual.file_offset(),
            actual.file_size(),
            actual.virtual_address(),
            actual.memory_size(),
            actual.mapping_address(),
            actual.mapping_size(),
            actual.permissions(),
        ) != expected
        {
            return Err(E::Contract(
                "fixed one-stop segment/permitted-layout contract",
            ));
        }
    }
    if k.name() != SYMBOL
        || k.symbol() != "fe2o3_gfx950_one_stop_fixture.kd"
        || k.kernarg_segment_size() != 264
        || k.kernarg_segment_alignment() != 8
        || k.private_segment_fixed_size() != 0
        || k.group_segment_fixed_size() != 0
        || k.wavefront_size() != 64
        || k.sgpr_count() != 20
        || k.vgpr_count() != 10
        || k.agpr_count() != Some(0)
        || k.sgpr_spill_count() != Some(0)
        || k.vgpr_spill_count() != Some(0)
        || k.max_flat_workgroup_size() != 64
        || k.required_workgroup_size() != Some([64, 1, 1])
        || k.max_workgroups() != [Some(1); 3]
        || k.cluster_dims().is_some()
        || k.kind() != KernelKind::Normal
        || k.uses_dynamic_stack_declaration() != Some(false)
        || !k.arguments_were_emitted()
        || k.explicit_arguments().len() != 1
        || k.hidden_arguments().len() != HIDDEN.len()
        || k.implicit_argument_offset() != Some(8)
        || k.implicit_argument_size() != 256
        || d.group_segment_fixed_size() != 0
        || d.private_segment_fixed_size() != 0
        || d.kernarg_size() != 264
        || d.kernel_code_entry_byte_offset() != 4160
        || d.compute_pgm_rsrc1() != 11468929
        || d.compute_pgm_rsrc2() != 132
        || d.compute_pgm_rsrc3() != 2
        || d.kernel_code_properties() != 8
        || d.kernarg_preload() != 0
        || c.descriptor_bytes() != DESCRIPTOR
        || c.entry_bytes() != ENTRY
        || b.descriptor_file_offset() != 1984
        || b.descriptor_address() != 1984
        || b.entry_file_offset() != 2048
        || b.entry_address() != 6144
        || b.entry_size() != 84
        || c.relocation_evidence().admitted_relocation_count() != 0
        || c.relocation_evidence().applied_relocation_count() != 0
    {
        return Err(E::Contract(
            "fixed one-stop selected metadata/entry/descriptor",
        ));
    }
    let a = &k.explicit_arguments()[0];
    if a.name() != Some("out")
        || a.offset() != 0
        || a.size() != 8
        || a.value_kind() != ExplicitValueKind::GlobalBuffer
        || a.address_space() != Some(ArgumentAddressSpace::Global)
    {
        return Err(E::Contract("fixed one-stop output ABI"));
    }
    for (actual, (offset, size, kind)) in k.hidden_arguments().iter().zip(HIDDEN) {
        if (actual.offset(), actual.size(), actual.value_kind()) != (offset, size, kind) {
            return Err(E::Contract("fixed one-stop hidden ABI"));
        }
    }
    Ok(())
}
pub(super) fn initialize_output(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() != PAGE {
        return Err("fixed one-stop output backing".into());
    }
    bytes.fill(0x5a);
    bytes[..8].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
    bytes[8..264].fill(0xa5);
    bytes[264..272].copy_from_slice(&0xfedc_ba98_7654_3210_u64.to_le_bytes());
    Ok(())
}
pub(super) fn check_output(bytes: &[u8], completed: bool) -> Result<(), E> {
    if bytes.len() != PAGE
        || bytes[..8] != 0x0123_4567_89ab_cdef_u64.to_le_bytes()
        || bytes[264..272] != 0xfedc_ba98_7654_3210_u64.to_le_bytes()
        || bytes[272..].iter().any(|v| *v != 0x5a)
    {
        return Err(E::Contract("fixed one-stop output extent/canaries"));
    }
    for lane in 0..64_u32 {
        let offset = 8 + lane as usize * 4;
        let expected = if completed {
            (0x1357_9bdf_u32 ^ lane).to_le_bytes()
        } else {
            [0xa5; 4]
        };
        if bytes[offset..offset + 4] != expected {
            return Err(E::Contract("fixed one-stop output payload"));
        }
    }
    Ok(())
}
pub(super) fn check_kernarg(bytes: &[u8], output_payload: u64) -> Result<(), E> {
    if bytes.len() != KERNARG_BYTES || output_payload == 0 || !output_payload.is_multiple_of(8) {
        return Err(E::Contract("fixed one-stop kernarg shape"));
    }
    // Exact COV6 initializer result, independently constrained for the sole geometry.
    let mut expected = [0_u8; KERNARG_BYTES];
    expected[..8].copy_from_slice(&output_payload.to_le_bytes());
    for offset in [8, 12, 16] {
        expected[offset..offset + 4].copy_from_slice(&1_u32.to_le_bytes());
    }
    for (offset, value) in [(20, 64_u16), (22, 1), (24, 1), (72, 1)] {
        expected[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    if bytes != expected {
        return Err(E::Contract("fixed one-stop kernarg values"));
    }
    Ok(())
}
pub(super) fn signal_addresses(base: u64, backing: u64) -> Result<(u64, u64), E> {
    if base == 0
        || !base.is_multiple_of(64)
        || backing != PAGE as u64
        || base.checked_add(backing).is_none()
    {
        return Err(E::Contract("fixed AMD signal extent"));
    }
    Ok((
        base,
        base.checked_add(8)
            .ok_or(E::Contract("fixed AMD signal value extent"))?,
    ))
}
#[cfg(test)]
#[path = "engineering_gfx950_debug_one_stop_contract_v1_tests.rs"]
mod tests;
