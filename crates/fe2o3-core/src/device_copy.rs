/// A host value whose complete object representation can be copied as bytes.
///
/// `DeviceCopy` protects the host Rust validity boundary of safe device-buffer
/// transfers. It requires every byte in a host value to be initialized, with
/// no padding, and every possible bit pattern to be a valid host Rust value.
/// The trait also requires `Copy + Send + Sync + 'static`.
///
/// fe2o3 currently implements this trait only for fixed-width integer and
/// floating-point primitives, arrays of `DeviceCopy` elements, and structs
/// accepted by `#[derive(DeviceCopy)]`. Types such as `bool`, `char`, `usize`,
/// pointers, and references have no implementation.
///
/// The derive accepts non-generic `#[repr(C)]` and `#[repr(transparent)]`
/// structs. It requires every field to implement `DeviceCopy` and proves, in
/// the compiling target's layout, that checked addition of all field sizes is
/// exactly the struct size. This excludes internal and trailing padding. The
/// generated unsafe implementation also makes the compiler enforce the
/// trait's `Copy + Send + Sync + 'static` supertraits.
///
/// These are structural host-side byte-copy guarantees only. Integer bit
/// patterns may still encode host addresses, resource handles, or other
/// application-defined state; the derive cannot identify their semantic
/// meaning. `DeviceCopy` does not assert that a device compiler gives the type
/// the same layout or calling convention, or that its values have valid device
/// provenance or capabilities. It must not, by itself, authorize safe typed
/// launch or device interpretation. Those APIs require separate manifest type
/// and ABI identity, provenance/address-space, and capability evidence. Raw
/// launches must place those obligations on their unsafe caller. `DeviceCopy`
/// also does not prove bounds, access modes, aliasing, synchronization, or race
/// freedom.
///
/// This example is exercised by the HIP-free derive compile fixture. It is
/// ignored by rustdoc because linking any executable against `fe2o3-core`
/// requires the HIP runtime library.
///
/// ```ignore
/// # pub use fe2o3_core::DeviceCopy;
/// # fn main() {
///
/// #[derive(Clone, Copy, fe2o3_core::DeviceCopy)]
/// #[repr(C)]
/// struct Pair {
///     left: u32,
///     right: f32,
/// }
///
/// fn requires_device_copy<T: fe2o3_core::DeviceCopy>() {}
/// requires_device_copy::<Pair>();
/// # }
/// ```
///
/// `bool` is rejected:
///
/// ```compile_fail
/// use fe2o3_core::DeviceCopy;
///
/// fn requires_device_copy<T: DeviceCopy>() {}
/// requires_device_copy::<bool>();
/// ```
///
/// Pointer-sized integers are rejected:
///
/// ```compile_fail
/// use fe2o3_core::DeviceCopy;
///
/// fn requires_device_copy<T: DeviceCopy>() {}
/// requires_device_copy::<usize>();
/// ```
///
/// References are rejected:
///
/// ```compile_fail
/// use fe2o3_core::DeviceCopy;
///
/// fn requires_device_copy<T: DeviceCopy>() {}
/// requires_device_copy::<&'static u32>();
/// ```
///
/// A user-defined `Copy` type is not accepted without an audited unsafe
/// implementation or the validated derive:
///
/// ```compile_fail
/// use fe2o3_core::DeviceCopy;
///
/// #[derive(Clone, Copy)]
/// struct Pair {
///     left: u32,
///     right: u32,
/// }
///
/// fn requires_device_copy<T: DeviceCopy>() {}
/// requires_device_copy::<Pair>();
/// ```
///
/// # Safety
///
/// On the compiling host target, implementors must have no padding, every byte
/// of every value must be initialized, and every possible bit pattern must be a
/// valid Rust value. The supertraits enforce `Copy + Send + Sync + 'static`.
/// This contract makes no claim about host/device ABI equality, value
/// provenance, resource-handle semantics, or suitability for device use.
pub unsafe trait DeviceCopy: Copy + Send + Sync + 'static {}

macro_rules! impl_device_copy_for_primitives {
    ($($type:ty),+ $(,)?) => {
        $(
            // SAFETY: On the host, these fixed-width primitives have no padding,
            // every byte is initialized, and every bit pattern is valid.
            unsafe impl DeviceCopy for $type {}
        )+
    };
}

impl_device_copy_for_primitives!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64,);

// SAFETY: Arrays are contiguous repetitions of `T` with no additional
// padding. The `DeviceCopy` requirements for every element therefore extend
// to the complete array, including zero-length arrays.
unsafe impl<T: DeviceCopy, const N: usize> DeviceCopy for [T; N] {}

#[cfg(test)]
mod tests {
    use super::DeviceCopy;

    #[derive(Clone, Copy, crate::DeviceCopy)]
    #[repr(C)]
    struct DerivedInsideCore {
        left: u32,
        right: u32,
    }

    fn assert_device_copy<T: DeviceCopy>() {}

    #[test]
    fn audited_primitives_and_arrays_are_device_copy() {
        assert_device_copy::<u8>();
        assert_device_copy::<i16>();
        assert_device_copy::<u32>();
        assert_device_copy::<i64>();
        assert_device_copy::<u128>();
        assert_device_copy::<f32>();
        assert_device_copy::<f64>();
        assert_device_copy::<[u32; 4]>();
        assert_device_copy::<[[f64; 2]; 3]>();
        assert_device_copy::<[u8; 0]>();
        assert_device_copy::<DerivedInsideCore>();
    }
}
