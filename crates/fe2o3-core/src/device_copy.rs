/// A value that can be copied byte-for-byte between host and device memory.
///
/// `DeviceCopy` protects the Rust validity boundary of safe device-buffer
/// transfers. It does not prove that a kernel stays in bounds, uses a buffer
/// with the declared access mode, is free of data races, or receives ABI fields
/// in the correct order.
///
/// fe2o3 currently implements this trait only for fixed-width integer and
/// floating-point primitives, and arrays of `DeviceCopy` elements. Types such
/// as `bool`, `char`, `usize`, pointers, and references have no implementation.
/// User-defined structs remain unsupported until fe2o3 has a derive macro with
/// layout validation.
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
/// implementation:
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
/// Implementors must be padding-free plain data whose bytes are always fully
/// initialized, with a stable, identical host/device representation. Every
/// possible bit pattern of the type must be a valid Rust value. The type must
/// contain no references, pointers, resource ownership, or interior
/// mutability, including transitively through its fields. These guarantees must
/// continue to hold for all values and on every host/device target pair on
/// which the implementation is used.
pub unsafe trait DeviceCopy: Copy + Send + Sync + 'static {}

macro_rules! impl_device_copy_for_primitives {
    ($($type:ty),+ $(,)?) => {
        $(
            // SAFETY: These fixed-width primitives have no padding or invalid
            // bit patterns and use the same representation on supported AMD
            // host/device target pairs.
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
    }
}
