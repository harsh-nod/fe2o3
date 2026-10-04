//! Target-neutral fixed-width SIMD values.
//!
//! [`GpuSimd`] deliberately has the same representation as its backing array.
//! That makes source and ABI behavior independent of host SIMD support and
//! gives the AMD backend a legal aggregate fallback when it cannot select a
//! native vector operation. It does not promise a target vector register ABI.

use core::fmt;
use core::ops::{Add, Div, Index, IndexMut, Mul, Neg, Sub};

mod sealed {
    pub trait Element {}
    pub trait LaneCount {}
}

/// A fixed-width scalar admitted as a [`GpuSimd`] lane.
///
/// This trait is sealed. Fixed-width integers, `f32`, `f64`, and the reviewed
/// fe2o3 floating-point storage values are supported. Pointer-sized integers
/// are excluded because their layout is target-dependent.
pub trait GpuSimdElement: sealed::Element + Copy {}

macro_rules! simd_elements {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl sealed::Element for $ty {}
            impl GpuSimdElement for $ty {}
        )+
    };
}

simd_elements!(
    u8,
    i8,
    u16,
    i16,
    u32,
    i32,
    u64,
    i64,
    f32,
    f64,
    crate::F16,
    crate::Bf16,
    crate::Fp8E4M3Fnuz,
    crate::Fp8E5M2Fnuz,
);

/// Type-level identity for a `GpuSimd` lane count.
#[doc(hidden)]
pub struct GpuSimdLaneCount<const N: usize>;

/// A lane count admitted by the bounded v1 SIMD contract.
///
/// This trait is sealed. The v1 profile supports 2, 4, 8, and 16 lanes.
pub trait ValidGpuSimdLaneCount: sealed::LaneCount {}

macro_rules! simd_lane_counts {
    ($($count:literal),+ $(,)?) => {
        $(
            impl sealed::LaneCount for GpuSimdLaneCount<$count> {}
            impl ValidGpuSimdLaneCount for GpuSimdLaneCount<$count> {}
        )+
    };
}

simd_lane_counts!(2, 4, 8, 16);

/// A target-neutral vector of `N` homogeneous scalar lanes.
///
/// The v1 profile accepts lane counts 2, 4, 8, and 16. Its representation is
/// exactly `[T; N]`, including the array's size and alignment. Operations have
/// portable lane-wise semantics and make no native-instruction claim.
#[repr(transparent)]
pub struct GpuSimd<T: GpuSimdElement, const N: usize>([T; N])
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount;

impl<T: GpuSimdElement, const N: usize> GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    /// Number of lanes in this value.
    pub const LANES: usize = N;

    /// Constructs a vector from lanes in increasing lane-index order.
    pub const fn from_array(lanes: [T; N]) -> Self {
        Self(lanes)
    }

    /// Returns the lanes in increasing lane-index order.
    pub const fn to_array(self) -> [T; N] {
        self.0
    }

    /// Constructs a vector whose lanes all contain `value`.
    pub const fn splat(value: T) -> Self {
        Self([value; N])
    }

    /// Returns a shared view of the backing lane array.
    pub const fn as_array(&self) -> &[T; N] {
        &self.0
    }

    /// Returns an exclusive view of the backing lane array.
    pub fn as_mut_array(&mut self) -> &mut [T; N] {
        &mut self.0
    }

    /// Returns the lane at `index`, or `None` when it is out of bounds.
    pub fn lane(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }

    /// Returns exclusive access to the lane at `index`, or `None` when it is
    /// out of bounds.
    pub fn lane_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index)
    }

    /// Returns host-rustc layout facts for ABI evidence.
    ///
    /// These facts are data only and do not authorize a kernel or target.
    #[doc(hidden)]
    pub const fn __fe2o3_rust_layout_v1() -> (usize, usize) {
        (core::mem::size_of::<Self>(), core::mem::align_of::<Self>())
    }
}

impl<T: GpuSimdElement, const N: usize> Clone for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GpuSimdElement, const N: usize> Copy for GpuSimd<T, N> where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount
{
}

impl<T: GpuSimdElement + Default, const N: usize> Default for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn default() -> Self {
        Self::splat(T::default())
    }
}

impl<T: GpuSimdElement + PartialEq, const N: usize> PartialEq for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: GpuSimdElement + Eq, const N: usize> Eq for GpuSimd<T, N> where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount
{
}

impl<T: GpuSimdElement + fmt::Debug, const N: usize> fmt::Debug for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("GpuSimd").field(&self.0).finish()
    }
}

impl<T: GpuSimdElement, const N: usize> From<[T; N]> for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn from(lanes: [T; N]) -> Self {
        Self::from_array(lanes)
    }
}

impl<T: GpuSimdElement, const N: usize> From<GpuSimd<T, N>> for [T; N]
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn from(value: GpuSimd<T, N>) -> Self {
        value.to_array()
    }
}

impl<T: GpuSimdElement, const N: usize> Index<usize> for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T: GpuSimdElement, const N: usize> IndexMut<usize> for GpuSimd<T, N>
where
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

macro_rules! impl_lane_binary_operation {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl<T, const N: usize> $trait for GpuSimd<T, N>
        where
            T: GpuSimdElement + $trait<Output = T>,
            GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
        {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                let mut lanes = self.0;
                let mut index = 0;
                while index < N {
                    lanes[index] = lanes[index] $operator rhs.0[index];
                    index += 1;
                }
                Self(lanes)
            }
        }
    };
}

impl_lane_binary_operation!(Add, add, +);
impl_lane_binary_operation!(Sub, sub, -);
impl_lane_binary_operation!(Mul, mul, *);
impl_lane_binary_operation!(Div, div, /);

impl<T, const N: usize> Neg for GpuSimd<T, N>
where
    T: GpuSimdElement + Neg<Output = T>,
    GpuSimdLaneCount<N>: ValidGpuSimdLaneCount,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        let mut lanes = self.0;
        let mut index = 0;
        while index < N {
            lanes[index] = -lanes[index];
            index += 1;
        }
        Self(lanes)
    }
}
