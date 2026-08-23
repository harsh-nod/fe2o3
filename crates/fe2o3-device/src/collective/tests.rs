use super::{
    GFX942_COLLECTIVE_CONTRACT_VERSION_V1, GFX942_STATIC_LDS_U32X256_ALIGNMENT,
    GFX942_STATIC_LDS_U32X256_BYTES, GFX942_STATIC_LDS_U32X256_SLOTS,
    GFX942_WAVE_LDS_VERTICAL_SLICE_VERSION_V1, Gfx942CollectiveElement, Gfx942Collectives,
    MAX_GFX942_WORKGROUP_COLLECTIVE_SIZE, WorkgroupCollectiveScratch,
    WorkgroupCollectiveScratchError,
};
use crate::group::SubgroupTile;
use crate::thread::{GridSize, Invocation3D, WorkgroupId, WorkgroupSize, WorkitemId};
use crate::wave::{Wave64, WaveLane};
use crate::{DynamicLds, Workgroup, WorkgroupLdsScope};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::vec::Vec;

fn add<T: Gfx942CollectiveElement>(lhs: T, rhs: T) -> T {
    lhs.__fe2o3_add(rhs)
}

fn xor_reduce<T: Gfx942CollectiveElement>(input: &[T]) -> Vec<T> {
    assert_eq!(input.len(), 64);
    let mut values = input.to_vec();
    let mut offset = 32;
    while offset != 0 {
        let previous = values.clone();
        for lane in 0..64 {
            values[lane] = add(previous[lane], previous[lane ^ offset]);
        }
        offset >>= 1;
    }
    values
}

fn masked_xor_reduce_u32(input: &[u32], active: &[bool]) -> Vec<u32> {
    assert_eq!(input.len(), 64);
    assert_eq!(active.len(), 64);
    let masked = input
        .iter()
        .zip(active)
        .map(|(&value, &active)| if active { value } else { 0 })
        .collect::<Vec<_>>();
    xor_reduce(&masked)
}

fn inclusive_scan<T: Gfx942CollectiveElement>(input: &[T]) -> Vec<T> {
    assert!(input.len().is_power_of_two());
    let mut values = input.to_vec();
    let mut offset = 1;
    while offset < values.len() {
        let previous = values.clone();
        for lane in offset..values.len() {
            values[lane] = add(previous[lane - offset], previous[lane]);
        }
        offset <<= 1;
    }
    values
}

fn exclusive_scan<T: Gfx942CollectiveElement>(input: &[T]) -> Vec<T> {
    let inclusive = inclusive_scan(input);
    let mut result = Vec::with_capacity(input.len());
    result.push(T::ZERO);
    result.extend_from_slice(&inclusive[..inclusive.len() - 1]);
    result
}

fn workgroup_reduce<T: Gfx942CollectiveElement>(input: &[T]) -> Vec<T> {
    assert!(input.len().is_power_of_two());
    let mut scratch = input.to_vec();
    let mut offset = scratch.len() / 2;
    while offset != 0 {
        let previous = scratch.clone();
        for rank in 0..offset {
            scratch[rank] = add(previous[rank], previous[rank + offset]);
        }
        offset >>= 1;
    }
    std::vec![scratch[0]; input.len()]
}

fn invocation(size: u32, rank: u32) -> Invocation3D {
    Invocation3D::from_model_snapshot(
        WorkitemId::new(rank, 0, 0),
        WorkgroupId::new(0, 0, 0),
        WorkgroupSize::new(size, 1, 1).unwrap(),
        GridSize::new(1, 1, 1).unwrap(),
    )
    .unwrap()
}

#[test]
fn contract_and_type_matrix_are_exact() {
    fn admitted<T: Gfx942CollectiveElement>() {}

    assert_eq!(GFX942_COLLECTIVE_CONTRACT_VERSION_V1, 1);
    assert_eq!(GFX942_WAVE_LDS_VERTICAL_SLICE_VERSION_V1, 1);
    assert_eq!(MAX_GFX942_WORKGROUP_COLLECTIVE_SIZE, 256);
    assert_eq!(GFX942_STATIC_LDS_U32X256_SLOTS, 256);
    assert_eq!(GFX942_STATIC_LDS_U32X256_BYTES, 1_024);
    assert_eq!(GFX942_STATIC_LDS_U32X256_ALIGNMENT, 4);
    admitted::<u32>();
    admitted::<i32>();
    admitted::<f32>();
    assert_eq!(add(u32::MAX, 1), 0);
    assert_eq!(add(i32::MAX, 1), i32::MIN);
    assert_eq!(add(1.25_f32, 2.5), 3.75);
}

#[test]
fn wave64_masked_reduction_ignores_every_inactive_lane() {
    let input = (1..=64_u32).collect::<Vec<_>>();
    let active = (0..64).map(|lane| lane % 3 != 1).collect::<Vec<_>>();
    let expected = input
        .iter()
        .zip(&active)
        .filter_map(|(&value, &active)| active.then_some(value))
        .fold(0_u32, u32::wrapping_add);
    assert_eq!(
        masked_xor_reduce_u32(&input, &active),
        std::vec![expected; 64]
    );

    let none = std::vec![false; 64];
    assert_eq!(masked_xor_reduce_u32(&input, &none), std::vec![0; 64]);
}

#[test]
fn wave64_reduction_returns_the_same_sum_to_every_lane() {
    let unsigned = (0..64_u32).collect::<Vec<_>>();
    let expected = unsigned.iter().copied().sum::<u32>();
    assert_eq!(xor_reduce(&unsigned), std::vec![expected; 64]);

    let signed = (0..64_i32).map(|value| value - 32).collect::<Vec<_>>();
    let expected = signed.iter().copied().sum::<i32>();
    assert_eq!(xor_reduce(&signed), std::vec![expected; 64]);

    let floats = (0..64).map(|value| value as f32 * 0.5).collect::<Vec<_>>();
    let expected = floats.iter().copied().sum::<f32>();
    assert_eq!(xor_reduce(&floats), std::vec![expected; 64]);
}

#[test]
fn wave64_scans_match_independent_prefix_oracles() {
    let input = (1..=64_u32).collect::<Vec<_>>();
    let inclusive = inclusive_scan(&input);
    let exclusive = exclusive_scan(&input);
    let mut running = 0;
    for lane in 0..64 {
        assert_eq!(exclusive[lane], running);
        running += input[lane];
        assert_eq!(inclusive[lane], running);
    }

    let floats = std::vec![0.5_f32; 64];
    for (lane, value) in inclusive_scan(&floats).into_iter().enumerate() {
        assert_eq!(value, (lane + 1) as f32 * 0.5);
    }
}

#[test]
fn workgroup_oracles_cover_every_admitted_size_and_type() {
    for size in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
        let unsigned = (0..size as u32).collect::<Vec<_>>();
        let expected = unsigned.iter().copied().sum::<u32>();
        assert_eq!(workgroup_reduce(&unsigned), std::vec![expected; size]);

        let signed = std::vec![-2_i32; size];
        assert_eq!(workgroup_reduce(&signed), std::vec![-2 * size as i32; size]);

        let floats = std::vec![0.25_f32; size];
        assert_eq!(
            workgroup_reduce(&floats),
            std::vec![size as f32 * 0.25; size]
        );

        let inclusive = inclusive_scan(&unsigned);
        let exclusive = exclusive_scan(&unsigned);
        let mut running = 0;
        for rank in 0..size {
            assert_eq!(exclusive[rank], running);
            running += unsigned[rank];
            assert_eq!(inclusive[rank], running);
        }
    }
}

#[test]
fn scratch_binding_accepts_only_the_bounded_power_of_two_profile() {
    let invocation_snapshot = invocation(8, 3);
    let group = Workgroup::from_invocation_snapshot(&invocation_snapshot).unwrap();
    let mut slots = [0_u32; 8];
    let scratch = unsafe {
        WorkgroupCollectiveScratch::from_raw_parts(&group, slots.as_mut_ptr(), slots.len() as u32)
    }
    .unwrap();
    assert_eq!(scratch.slots(), 8);

    let mismatch =
        unsafe { WorkgroupCollectiveScratch::<u32>::from_raw_parts(&group, slots.as_mut_ptr(), 4) };
    assert_eq!(
        mismatch.unwrap_err(),
        WorkgroupCollectiveScratchError::SlotCountMismatch {
            required: 8,
            provided: 4,
        }
    );

    let null = unsafe {
        WorkgroupCollectiveScratch::<u32>::from_raw_parts(&group, core::ptr::null_mut(), 8)
    };
    assert_eq!(null.unwrap_err(), WorkgroupCollectiveScratchError::NullBase);

    let misaligned = unsafe {
        WorkgroupCollectiveScratch::<u32>::from_raw_parts(
            &group,
            core::ptr::without_provenance_mut(1),
            8,
        )
    };
    assert_eq!(
        misaligned.unwrap_err(),
        WorkgroupCollectiveScratchError::MisalignedBase {
            address: 1,
            alignment: 4,
        }
    );

    for size in [3, 6, 255, 512] {
        let invocation = invocation(size, 0);
        let group = Workgroup::from_invocation_snapshot(&invocation).unwrap();
        let rejected = unsafe {
            WorkgroupCollectiveScratch::<u32>::from_raw_parts(&group, slots.as_mut_ptr(), size)
        };
        assert_eq!(
            rejected.unwrap_err(),
            WorkgroupCollectiveScratchError::UnsupportedWorkgroupSize {
                size: u64::from(size),
            }
        );
    }
}

#[test]
fn typed_lds_capability_is_consumed_by_collective_scratch() {
    let invocation_snapshot = invocation(8, 3);
    let group = Workgroup::from_invocation_snapshot(&invocation_snapshot).unwrap();
    let mut slots = [core::mem::MaybeUninit::<i32>::uninit(); 8];
    let mut scope = WorkgroupLdsScope::for_host_test();
    let lds = unsafe {
        DynamicLds::<i32>::from_host_parts_for_test(
            &mut scope,
            slots.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&slots),
        )
    }
    .unwrap();
    let scratch = WorkgroupCollectiveScratch::from_dynamic_lds(&group, lds).unwrap();
    assert_eq!(scratch.slots(), 8);

    let mut short_slots = [core::mem::MaybeUninit::<i32>::uninit(); 4];
    let mut short_scope = WorkgroupLdsScope::for_host_test();
    let short_lds = unsafe {
        DynamicLds::<i32>::from_host_parts_for_test(
            &mut short_scope,
            short_slots.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&short_slots),
        )
    }
    .unwrap();
    assert_eq!(
        WorkgroupCollectiveScratch::from_dynamic_lds(&group, short_lds).unwrap_err(),
        WorkgroupCollectiveScratchError::SlotCountMismatch {
            required: 8,
            provided: 4,
        }
    );
}

#[test]
fn compiler_authority_and_collective_hooks_panic_closed_on_host() {
    assert!(catch_unwind(Gfx942Collectives::current).is_err());

    let context = Gfx942Collectives::for_host_test();
    assert!(catch_unwind(|| context.static_lds_u32x256()).is_err());
    assert!(catch_unwind(|| context.wave64_reduce_sum_active_u32(1, 7)).is_err());
    assert!(catch_unwind(|| context.subgroup_reduce_sum_f32::<16>(1.0)).is_err());
    assert!(catch_unwind(|| context.subgroup_reduce_max_f32::<16>(1.0)).is_err());
    let lane = WaveLane::<Wave64>::from_model_snapshot(7).unwrap();
    let tile = SubgroupTile::<64>::from_wave64_snapshot(&lane);
    assert!(catch_unwind(AssertUnwindSafe(|| tile.reduce_sum(&context, 7_u32))).is_err());
}
