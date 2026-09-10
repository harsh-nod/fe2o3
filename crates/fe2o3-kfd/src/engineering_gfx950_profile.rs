//! Exact engineering queue geometry, not gfx942 or protected runtime authority.

use crate::topology::{GfxTarget, HostTopologySnapshot};

pub(super) const PAGE_BYTES: usize = 4096;
pub(super) const RING_BYTES: usize =
    crate::engineering_wire::MAX_UNRETIRED_RING_PACKETS_V1 as usize * 64;
pub(super) const XCC_COUNT: usize = 8;
pub(super) const CONTROL_STACK_BYTES: u32 = 0x3000;
pub(super) const CONTEXT_BYTES_PER_XCC: usize = 0x15a_3000;
pub(super) const DEBUG_BYTES_TOTAL: u32 = 0x5_0000;
pub(super) const CWSR_BYTES: usize = 0xad6_8000;
pub(super) const MAX_BUFFER_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub(super) const MAX_ALLOCATIONS: usize = 2048;
pub(super) const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024 * 1024;

#[derive(Clone, Copy)]
struct QueueFacts {
    target: GfxTarget,
    capacity: [u32; 9],
    module: [Option<i32>; 3],
}

fn validate_facts(facts: QueueFacts) -> Result<(), &'static str> {
    if facts.target != GfxTarget::Gfx950 || facts.capacity != [1024, 4, 8, 32, 1, 160, 8, 24, 64] {
        return Err("gfx950 queue topology profile");
    }
    if facts.module != [Some(0), Some(0), Some(1)] {
        return Err("gfx950 queue module parameters");
    }
    Ok(())
}

pub(super) fn validate_profile(
    snapshot: &HostTopologySnapshot,
    unique_id: u64,
) -> Result<(), &'static str> {
    let gpu = snapshot
        .topology()
        .gpu_nodes()
        .iter()
        .find(|gpu| gpu.unique_id() == unique_id)
        .ok_or("selected GPU absent")?;
    let properties = gpu.capacity();
    let module = snapshot.amdgpu_module();
    validate_facts(QueueFacts {
        target: gpu.target(),
        capacity: [
            properties.simd_count(),
            properties.simd_per_cu(),
            properties.xcc_count(),
            properties.array_count(),
            properties.simd_arrays_per_engine(),
            properties.lds_size_in_kb(),
            properties.max_waves_per_simd(),
            properties.compute_queue_count(),
            properties.wavefront_size(),
        ],
        module: [module.mes(), module.sched_policy(), module.cwsr_enable()],
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Gfx950DoorbellPlanV1 {
    pub(super) encoded_slice_offset: u64,
    pub(super) queue_byte_offset: u64,
}

pub(super) fn admit_doorbell(
    queue_id: u32,
    raw: u64,
    gpu_id: u32,
) -> Result<Gfx950DoorbellPlanV1, &'static str> {
    use fe2o3_kfd_uapi::{
        KFD_MAX_QUEUE_SLOTS_PER_PROCESS, KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_TYPE_DOORBELL,
        KFD_MMAP_TYPE_SHIFT,
    };
    const GPU_ID_HASH_MASK: u64 = 0xffff;
    let offset = raw & ((1_u64 << KFD_MMAP_GPU_ID_HASH_SHIFT) - 1);
    if queue_id >= KFD_MAX_QUEUE_SLOTS_PER_PROCESS
        || raw >> KFD_MMAP_TYPE_SHIFT != KFD_MMAP_TYPE_DOORBELL
        || (raw >> KFD_MMAP_GPU_ID_HASH_SHIFT) & GPU_ID_HASH_MASK
            != u64::from(gpu_id) & GPU_ID_HASH_MASK
        || offset >= 8192
        || !offset.is_multiple_of(8)
    {
        return Err("gfx950 queue output/doorbell identity");
    }
    Ok(Gfx950DoorbellPlanV1 {
        encoded_slice_offset: raw & !8191,
        queue_byte_offset: offset,
    })
}

pub(super) fn checked_range(
    total: u64,
    offset: u64,
    bytes: u64,
) -> Result<std::ops::Range<usize>, &'static str> {
    let end = offset
        .checked_add(bytes)
        .filter(|end| *end <= total)
        .ok_or("buffer extent")?;
    let start = usize::try_from(offset).map_err(|_| "buffer offset width")?;
    let end = usize::try_from(end).map_err(|_| "buffer end width")?;
    Ok(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_queue_profile_fact_is_exact_and_opposite_target_is_rejected() {
        let facts = QueueFacts {
            target: GfxTarget::Gfx950,
            capacity: [1024, 4, 8, 32, 1, 160, 8, 24, 64],
            module: [Some(0), Some(0), Some(1)],
        };
        validate_facts(facts).unwrap();
        let mut changed = facts;
        changed.target = GfxTarget::Gfx942;
        assert!(validate_facts(changed).is_err());
        for index in 0..facts.capacity.len() {
            let mut changed = facts;
            changed.capacity[index] += 1;
            assert!(validate_facts(changed).is_err(), "capacity {index}");
        }
        for index in 0..facts.module.len() {
            for value in [None, Some(2)] {
                let mut changed = facts;
                changed.module[index] = value;
                assert!(validate_facts(changed).is_err(), "module {index}");
            }
        }
    }

    #[test]
    fn gfx950_cwsr_matches_active_driver_formula_not_gfx942_geometry() {
        let cu_per_xcc = 1024_usize / 4 / XCC_COUNT;
        let waves = (cu_per_xcc * 40).min(32 * 512);
        let control = (40 + waves * 8 + 8).next_multiple_of(PAGE_BYTES);
        let workgroup =
            (cu_per_xcc * (0x80000 + 0x4000 + 160 * 1024 + 0x1000)).next_multiple_of(PAGE_BYTES);
        let debug_per_xcc = (waves * 32).next_multiple_of(64);
        assert_eq!(control, CONTROL_STACK_BYTES as usize);
        assert_eq!(control + workgroup, CONTEXT_BYTES_PER_XCC);
        assert_eq!(debug_per_xcc * XCC_COUNT, DEBUG_BYTES_TOTAL as usize);
        assert_eq!(
            ((control + workgroup + debug_per_xcc) * XCC_COUNT).next_multiple_of(PAGE_BYTES),
            CWSR_BYTES
        );
        assert_ne!(
            CWSR_BYTES as u64,
            crate::GFX942_CONTEXT_SAVE_MAPPING_BYTES_V1
        );
    }

    #[test]
    fn exact_empty_extent_is_allowed_without_allowing_overflow_or_past_end() {
        assert_eq!(checked_range(2, 2, 0), Ok(2..2));
        assert!(checked_range(2, 3, 0).is_err());
        assert!(checked_range(2, 1, 2).is_err());
        assert!(checked_range(u64::MAX, u64::MAX, 1).is_err());
    }

    #[test]
    fn doorbell_output_is_exact_to_device_and_whole_slice() {
        use fe2o3_kfd_uapi::{
            KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_TYPE_DOORBELL, KFD_MMAP_TYPE_SHIFT,
        };
        let base =
            (KFD_MMAP_TYPE_DOORBELL << KFD_MMAP_TYPE_SHIFT) | (7 << KFD_MMAP_GPU_ID_HASH_SHIFT);
        assert_eq!(
            admit_doorbell(0, base + 8, 7).unwrap().encoded_slice_offset,
            base
        );
        assert!(admit_doorbell(u32::MAX, base, 7).is_err());
        assert!(admit_doorbell(0, base, 8).is_err());
        assert!(admit_doorbell(0, base + 4, 7).is_err());
        assert!(admit_doorbell(0, base + 8192, 7).is_err());
        assert!(admit_doorbell(0, 0, 7).is_err());
    }
}
