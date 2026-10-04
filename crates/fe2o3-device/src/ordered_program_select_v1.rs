//! Eager compile-time selection between two complete flat ordered programs.
//! The result uses the existing count plus four packed source constants.
//! There is no runtime branch, new marker schema or executable artifact owner.

/// Validate BOTH complete arms, then select their inert count/packed metadata.
///
/// Each arm starts with only input0/input1/input2 initialized. The existing
/// flat checker independently enforces 1..=16 instructions, admitted opcodes,
/// reserved bits, source/destination roles, read-before-definition and output
/// definition. A valid selected arm never exempts the other arm from checking.
/// Scratch/output state cannot leak from one alternative into the other.
///
/// This is a source-expansion helper, not a device-data conditional. The
/// authoring macro requires a concrete Rust constant expression of type bool;
/// runtime values and references to an outer generic parameter are not admitted.
/// Both arms use only the existing flat syntax; no nested selection, repetition
/// or initialization block is introduced. The backend still checks the actual
/// single marker Instance, five typed constants, literal physical roles, target,
/// launch contract and direct-root occurrence. No source/proof/launch authority
/// or physical-register lifetime follows from these source descriptors.
///
/// Both old packer calls run before selection. Each rejects length outside
/// 1..=16 before its descriptor loop; at most 32 descriptor checks and two
/// 32-byte packed arrays are required per helper call. The macro projects five
/// const-generic arguments using five direct helper calls, hence at most 160
/// descriptor checks across those syntactic calls; rustc may reuse evaluations.
/// This does not bound predicate evaluation, macro tokens, rustc const-evaluation
/// machinery, stack layout, allocations or process RSS.
///
/// ```
/// use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1;
/// const FIRST: (u8, [u64; 4]) =
///     __checked_ordered_select_v1(3_u32 < 4, &[8, 201], &[8, 201, 201]);
/// const SECOND: (u8, [u64; 4]) =
///     __checked_ordered_select_v1(4_u32 < 3, &[8, 201], &[8, 201, 201]);
/// assert_eq!(FIRST, (2, [8 | (201 << 16), 0, 0, 0]));
/// assert_eq!(SECOND, (3, [8 | (201 << 16) | (201 << 32), 0, 0, 0]));
/// ```
///
/// An inactive arm must still be nonempty and must initialize its own output:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_select_v1(true, &[8], &[]);
/// ```
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_select_v1(true, &[8], &[201]);
/// ```
///
/// Validation is symmetric, even when the invalid first arm is not selected:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_select_v1(false, &[0], &[8]);
/// ```
///
/// An inactive arm cannot exceed the existing flat limit:
///
/// ```compile_fail
/// use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1;
/// const BAD: (u8, [u64; 4]) = __checked_ordered_select_v1(true, &[8], &[8; 17]);
/// ```
///
/// The predicate is a bool constant, not a runtime operand:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// fn dynamic(a: u32, b: u32) -> u32 {
///     amdgpu_ordered_program! {
///         gfx942_xnack_off_wave64;
///         scratch(32); out(33); in(34) = a; in(35) = b; in(36) = 0;
///         const_if(a != 0) { mov(out, input0); }
///         else { mov(out, input1); }
///     }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// fn non_bool(a: u32, b: u32) -> u32 {
///     amdgpu_ordered_program! {
///         gfx942_xnack_off_wave64;
///         scratch(32); out(33); in(34) = a; in(35) = b; in(36) = 0;
///         const_if(1_u32) { mov(out, input0); }
///         else { mov(out, input1); }
///     }
/// }
/// ```
///
/// This bounded spelling does not accept an outer const-generic predicate:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// fn generic<const SELECT: bool>(a: u32, b: u32) -> u32 {
///     amdgpu_ordered_program! {
///         gfx942_xnack_off_wave64;
///         scratch(32); out(33); in(34) = a; in(35) = b; in(36) = 0;
///         const_if(SELECT) { mov(out, input0); }
///         else { mov(out, input1); }
///     }
/// }
/// ```
#[doc(hidden)]
pub const fn __checked_ordered_select_v1(
    condition: bool,
    if_true: &[u16],
    if_false: &[u16],
) -> (u8, [u64; 4]) {
    // Do not put either validation behind condition or a lazy boolean operator.
    // Panics in an inactive arm are part of the source contract.
    let true_packed = super::__checked_ordered_program_words_v1(if_true);
    let false_packed = super::__checked_ordered_program_words_v1(if_false);
    // Both lengths have already been checked as 1..=16 before these casts.
    if condition {
        (if_true.len() as u8, true_packed)
    } else {
        (if_false.len() as u8, false_packed)
    }
}
