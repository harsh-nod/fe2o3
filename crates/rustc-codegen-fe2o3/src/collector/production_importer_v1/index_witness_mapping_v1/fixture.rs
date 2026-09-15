#![no_std]

use fe2o3_device::{
    CurrentTarget, DisjointIndex, DisjointSlice, Index1D, InitialEpoch, KernelCapabilityBrand,
    KernelError, KernelResult, NextEpoch, RegisteredLaunch, Shifted, ThreadIndex,
    WorkgroupCapability, WorkgroupIndex1D, WorkgroupMemoryBrand,
};

pub struct KernelMarker;
pub struct OtherKernelMarker;
pub struct UnknownSpace;
pub struct FakeIndex(pub usize);
pub struct FakeSlice(pub *mut f32, pub usize);

type Brand = KernelCapabilityBrand<'static, KernelMarker, CurrentTarget, RegisteredLaunch>;
type OtherBrand =
    KernelCapabilityBrand<'static, OtherKernelMarker, CurrentTarget, RegisteredLaunch>;
type MemoryBrand = WorkgroupMemoryBrand<'static, Brand, InitialEpoch>;
type OtherMemoryBrand = WorkgroupMemoryBrand<'static, OtherBrand, InitialEpoch>;
type NextMemoryBrand = WorkgroupMemoryBrand<'static, Brand, NextEpoch<InitialEpoch>>;

pub fn rowsoft_conversion(
    workgroup: WorkgroupCapability<'static, Brand, InitialEpoch>,
    local_max: f32,
) -> KernelResult {
    let mut maxima = workgroup.allocate_memory::<f32, 64>();
    let maximum_index = workgroup
        .memory_index_1d()
        .ok_or(KernelError::OutOfBounds)?;
    if !maxima.store(&workgroup, maximum_index.into_disjoint(), local_max) {
        return Err(KernelError::OutOfBounds);
    }
    let (_workgroup, _maxima) = workgroup.publish_memory(maxima);
    Ok(())
}

pub fn global_conversion(index: ThreadIndex<Index1D>) -> DisjointIndex<Index1D> {
    index.into_disjoint()
}

pub fn shifted_conversion(
    index: ThreadIndex<Shifted<Index1D, 3>>,
) -> DisjointIndex<Shifted<Index1D, 3>> {
    index.into_disjoint()
}

pub fn alternate_outputs(
    _wrong_space: DisjointIndex<Index1D, MemoryBrand>,
    _wrong_brand: DisjointIndex<WorkgroupIndex1D, OtherMemoryBrand>,
    _wrong_epoch: DisjointIndex<WorkgroupIndex1D, NextMemoryBrand>,
    _unbranded: DisjointIndex<WorkgroupIndex1D>,
    _wrong_kind: ThreadIndex<WorkgroupIndex1D, MemoryBrand>,
    _fake: FakeIndex,
) {
}

pub fn slice_identity(slice: &mut DisjointSlice<f32>, index: ThreadIndex) -> bool {
    slice.get_mut(index).is_some()
}

pub fn slice_shifted(
    slice: &mut DisjointSlice<f32, Shifted<Index1D, 3>>,
    index: ThreadIndex<Shifted<Index1D, 3>>,
) -> bool {
    slice.get_mut(index).is_some()
}

pub fn slice_unsupported(
    slice: &mut DisjointSlice<f32, UnknownSpace>,
    index: ThreadIndex<UnknownSpace>,
) -> bool {
    slice.get_mut(index).is_some()
}

pub fn slice_fake(_slice: &mut FakeSlice, _index: FakeIndex) {}
