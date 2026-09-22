//! Bounded compile-time expansion into the existing ordered-program descriptors.
//! There is no runtime loop, alternate executable graph or new marker schema.

/// Expand one nonempty initialization sequence followed by 1..=15 copies of one
/// nonempty block. At most 16 flat descriptors may result. The existing packer
/// performs all opcode/role/read-before-definition/exit checks on the complete
/// expanded sequence, preserving state across copies rather than resetting it.
///
/// This hidden source helper returns only inert count/packed source metadata.
/// The ordinary backend independently rechecks the normalized five constants,
/// register literals, actual marker Instance and one-root source occurrence.
/// It grants no source, proof, scheduling, artifact or launch authority.
///
/// Every input bound is checked before copying. Local storage is one 16-word
/// array (32 payload bytes) plus the existing packer's 32-byte packed array;
/// at most 16 copies and 16 existing validation iterations occur. This does not
/// bound rustc macro parsing, const evaluation machinery or process memory.
///
/// Valid constant expansion, including the maximum 16-step result:
///
/// ```
/// use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1;
/// const ONE: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], 1);
/// const TWO: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], 2);
/// const FIFTEEN: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], 15);
/// assert_eq!((ONE.0, TWO.0, FIFTEEN.0), (2, 3, 16));
/// ```
///
/// Zero copies are unsupported; there is no dead-block exception:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], 0);
/// ```
///
/// Sixteen copies with initialization would exceed the flat descriptor profile:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], 16);
/// ```
///
/// A huge repeat count is refused before multiplication, copying or packing:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[8], &[201], usize::MAX);
/// ```
///
/// Defining scratch does not initialize output; the existing checker catches
/// the first repeated read of an undefined output:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_repeat_v1(&[0], &[201], 1);
/// ```
///
/// The authoring syntax requires a literal count, not a device runtime value:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// fn dynamic(count: usize, a: u32, b: u32) -> u32 {
///     amdgpu_ordered_program! {
///         gfx942_xnack_off_wave64;
///         scratch(32); out(33); in(34) = a; in(35) = b; in(36) = 0;
///         init { mov(out, input0); }
///         repeat(count) { add(out, out, input1); }
///     }
/// }
/// ```
///
/// There is exactly one nonnested repetition block:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// fn nested(a: u32, b: u32) -> u32 {
///     amdgpu_ordered_program! {
///         gfx942_xnack_off_wave64;
///         scratch(32); out(33); in(34) = a; in(35) = b; in(36) = 0;
///         init { mov(out, input0); }
///         repeat(2) { repeat(2) { add(out, out, input1); } }
///     }
/// }
/// ```
#[doc(hidden)]
pub const fn __checked_ordered_repeat_v1(
    initial: &[u16],
    repeated: &[u16],
    repetitions: usize,
) -> (u8, [u64; 4]) {
    assert!(
        !initial.is_empty() && initial.len() <= 16,
        "ordered repeat initialization requires 1..16 steps"
    );
    assert!(
        !repeated.is_empty() && repeated.len() <= 16,
        "ordered repeat block requires 1..16 steps"
    );
    assert!(
        repetitions > 0 && repetitions <= 15,
        "ordered repeat count must be 1..15"
    );
    // The fixed bounds above already exclude usize overflow on supported Rust
    // targets. Keep checked arithmetic explicit if the local grammar evolves.
    let repeated_count = match repeated.len().checked_mul(repetitions) {
        Some(count) => count,
        // fe2o3-hygiene: allow-panic issue-280: checked const-source expansion must fail closed on overflow.
        None => panic!("ordered repeat step multiplication overflow"),
    };
    let count = match initial.len().checked_add(repeated_count) {
        Some(count) => count,
        // fe2o3-hygiene: allow-panic issue-280: checked const-source expansion must fail closed on overflow.
        None => panic!("ordered repeat step addition overflow"),
    };
    assert!(count <= 16, "expanded ordered program exceeds 16 steps");
    let mut expanded = [0_u16; 16];
    let mut index = 0;
    while index < initial.len() {
        expanded[index] = initial[index];
        index += 1;
    }
    while index < count {
        expanded[index] = repeated[(index - initial.len()) % repeated.len()];
        index += 1;
    }
    let (selected, _) = expanded.split_at(count);
    (
        count as u8,
        super::__checked_ordered_program_words_v1(selected),
    )
}
