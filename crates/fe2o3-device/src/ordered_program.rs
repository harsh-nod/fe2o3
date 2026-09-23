//! Compile-time syntax and packing for one closed gfx942 ordered u32 program.
//! Descriptors are source metadata, NOT AMD machine instruction encodings.

/// Packs and checks source descriptors without producing a runtime program
/// pointer, slice or helper argument. The backend independently checks the
/// actual typed const generics and source occurrence.
#[doc(hidden)]
pub const fn __checked_ordered_program_words_v1(words: &[u16]) -> [u64; 4] {
    assert!(
        !words.is_empty() && words.len() <= 16,
        "ordered program requires 1..16 steps"
    );
    let mut packed = [0_u64; 4];
    let mut defined = 0b00111_u8;
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let opcode = word & 7;
        let source0 = (word >> 4) & 7;
        let source1 = (word >> 7) & 7;
        assert!(
            word >> 10 == 0,
            "ordered program reserved descriptor bits must be zero"
        );
        assert!(opcode <= 5, "ordered program opcode is not admitted");
        assert!(source0 <= 4, "ordered program source role is not admitted");
        assert!(
            defined & (1 << source0) != 0,
            "ordered program reads an undefined source"
        );
        if opcode == 0 {
            assert!(
                source1 == 0,
                "ordered program move must not have a second source"
            );
        } else {
            assert!(
                source1 <= 4,
                "ordered program second source role is not admitted"
            );
            assert!(
                defined & (1 << source1) != 0,
                "ordered program reads an undefined second source"
            );
        }
        // Read against the pre-step state; only then define scratch or out.
        defined |= 1 << (3 + ((word >> 3) & 1));
        packed[index / 4] |= (word as u64) << ((index % 4) * 16);
        index += 1;
    }
    assert!(
        defined & 0b10000 != 0,
        "ordered program output is not defined"
    );
    packed
}

#[doc(hidden)]
#[path = "ordered_program_repeat_v1.rs"]
pub mod repeat_v1;

#[doc(hidden)]
#[path = "ordered_program_select_v1.rs"]
pub mod select_v1;

#[doc(hidden)]
#[macro_export]
macro_rules! __fe2o3_ordered_program_destination_v1 {
    (scratch) => {
        0_u16
    };
    (out) => {
        8_u16
    };
    ($($unsupported:tt)*) => {
        compile_error!("ordered program destinations are only scratch and out")
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __fe2o3_ordered_program_source_v1 {
    (input0) => {
        0_u16
    };
    (input1) => {
        1_u16
    };
    (input2) => {
        2_u16
    };
    (scratch) => {
        3_u16
    };
    (out) => {
        4_u16
    };
    ($($unsupported:tt)*) => {
        compile_error!("ordered program source role is not admitted")
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __fe2o3_ordered_program_binary_v1 {
    (add) => {
        1_u16
    };
    (sub) => {
        2_u16
    };
    (and) => {
        3_u16
    };
    (or) => {
        4_u16
    };
    (xor) => {
        5_u16
    };
    ($($unsupported:tt)*) => {
        compile_error!("ordered program binary opcode or arity is not admitted")
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __fe2o3_ordered_program_step_v1 {
    (mov($destination:ident, $source:ident)) => {
        $crate::__fe2o3_ordered_program_destination_v1!($destination)
            | ($crate::__fe2o3_ordered_program_source_v1!($source) << 4)
    };
    ($opcode:ident($destination:ident, $source0:ident, $source1:ident)) => {
        $crate::__fe2o3_ordered_program_binary_v1!($opcode)
            | $crate::__fe2o3_ordered_program_destination_v1!($destination)
            | ($crate::__fe2o3_ordered_program_source_v1!($source0) << 4)
            | ($crate::__fe2o3_ordered_program_source_v1!($source1) << 7)
    };
    ($($unsupported:tt)*) => {
        compile_error!("ordered program instruction or arity is not admitted")
    };
}

/// Authors 1..16 ordered u32 register-only steps for gfx942:xnack-/wave64.
///
/// The inputs are read-only. Scratch and out must be initialized before use;
/// out must be defined at exit. Allowed operations are mov, wrapping add/sub,
/// and, or and xor. Every authored step is retained, including overwritten
/// writes and self-moves. Five physical roles must be distinct literal v0..v63.
///
/// The compiler also checks one unconditional acyclic root occurrence, exact
/// target and required/maximum workgroup64x1x1. This operation is not a memory
/// fence. Logical CPU/debugger observation is whole-region before/after, not
/// physical-register contents or instruction microsteps. Host execution panics.
///
/// ```no_run
/// # use fe2o3_device::amdgpu_ordered_program;
/// # let (a, b, mask) = (19_u32, 23_u32, 42_u32);
/// let selected = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33);
///     in(34) = a; in(35) = b; in(36) = mask;
///     xor(scratch, input0, input1);
///     and(scratch, scratch, input2);
///     xor(out, input1, scratch);
/// };
/// ```
///
/// Read-before-definition is rejected during const evaluation:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// let _ = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33); in(34) = 1; in(35) = 2; in(36) = 3;
///     add(out, scratch, input0);
/// };
/// ```
///
/// Input writes and wrong arity are outside the source grammar:
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// let _ = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33); in(34) = 1; in(35) = 2; in(36) = 3;
///     mov(input0, input1);
/// };
/// ```
///
/// ```compile_fail
/// use fe2o3_device::amdgpu_ordered_program;
/// let _ = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33); in(34) = 1; in(35) = 2; in(36) = 3;
///     mov(out, input0, input1);
/// };
/// ```
///
/// A second, explicitly compile-time spelling supports one nonempty
/// initialization and one nonnested block repeated by a literal 1..=15 count.
/// The complete expansion must still contain at most 16 instructions. It
/// produces the same single marker and five typed constants as the flat form;
/// it is not a GPU loop, schedule recipe or additional executable description.
/// Scratch/output state flows through all copies. Runtime data expressions
/// still evaluate once, left to right, before the marker call.
///
/// ```no_run
/// use fe2o3_device::amdgpu_ordered_program;
/// let (a, b, c) = (19_u32, 23_u32, 42_u32);
/// let value = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
///     init { mov(out, input0); }
///     repeat(2) { add(out, out, input1); }
/// };
/// ```
///
/// A concrete bool constant can choose between two complete flat programs.
/// BOTH alternatives are checked independently, including the unselected arm.
/// Each must contain 1..=16 admitted instructions and initialize its own out.
/// Selection occurs before the unchanged single marker; there is no device
/// branch. Outer generic predicates, runtime data and nested control syntax
/// are not supported. Runtime data operands still evaluate once in order.
///
/// ```no_run
/// use fe2o3_device::amdgpu_ordered_program;
/// const SELECT: bool = 3_u32 < 4;
/// let (a, b, c) = (19_u32, 23_u32, 42_u32);
/// let value = amdgpu_ordered_program! {
///     gfx942_xnack_off_wave64;
///     scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
///     const_if(SELECT) {
///         mov(out, input0);
///         add(out, out, input1);
///     } else {
///         mov(out, input0);
///         add(out, out, input1);
///         add(out, out, input1);
///     }
/// };
/// ```
#[macro_export]
macro_rules! amdgpu_ordered_program {
    (
        gfx942_xnack_off_wave64;
        scratch($scratch:literal); out($output:literal);
        in($input0:literal) = $a:expr;
        in($input1:literal) = $b:expr;
        in($input2:literal) = $c:expr;
        $( $opcode:ident ( $( $role:ident ),* ) ; )*
    ) => {{
        const __WORDS: &[u16] = &[$($crate::__fe2o3_ordered_program_step_v1!($opcode($($role),*))),*];
        const __COUNT: u8 = {
            assert!(!__WORDS.is_empty() && __WORDS.len() <= 16, "ordered program requires 1..16 steps");
            __WORDS.len() as u8
        };
        const __PACKED: [u64; 4] = $crate::ordered_program::__checked_ordered_program_words_v1(__WORDS);
        $crate::diagnostics::__amdgpu_ordered_program_e32_v1::<
            __COUNT, { __PACKED[0] }, { __PACKED[1] }, { __PACKED[2] }, { __PACKED[3] },
        >($a, $b, $c, $scratch, $output, $input0, $input1, $input2)
    }};
    (
        gfx942_xnack_off_wave64;
        scratch($scratch:literal); out($output:literal);
        in($input0:literal) = $a:expr;
        in($input1:literal) = $b:expr;
        in($input2:literal) = $c:expr;
        init { $( $initial_opcode:ident ( $( $initial_role:ident ),* ) ; )* }
        repeat($repetitions:literal) {
            $( $repeated_opcode:ident ( $( $repeated_role:ident ),* ) ; )*
        }
    ) => {{
        const __INITIAL: &[u16] = &[
            $($crate::__fe2o3_ordered_program_step_v1!($initial_opcode($($initial_role),*))),*
        ];
        const __REPEATED: &[u16] = &[
            $($crate::__fe2o3_ordered_program_step_v1!($repeated_opcode($($repeated_role),*))),*
        ];
        const __REPETITIONS: usize = $repetitions;
        const __EXPANDED: (u8, [u64; 4]) =
            $crate::ordered_program::repeat_v1::__checked_ordered_repeat_v1(
                __INITIAL, __REPEATED, __REPETITIONS,
            );
        $crate::diagnostics::__amdgpu_ordered_program_e32_v1::<
            { __EXPANDED.0 },
            { __EXPANDED.1[0] },
            { __EXPANDED.1[1] },
            { __EXPANDED.1[2] },
            { __EXPANDED.1[3] },
        >($a, $b, $c, $scratch, $output, $input0, $input1, $input2)
    }};
    (
        gfx942_xnack_off_wave64;
        scratch($scratch:literal); out($output:literal);
        in($input0:literal) = $a:expr;
        in($input1:literal) = $b:expr;
        in($input2:literal) = $c:expr;
        const_if($condition:expr) {
            $( $true_opcode:ident ( $( $true_role:ident ),* ) ; )*
        } else {
            $( $false_opcode:ident ( $( $false_role:ident ),* ) ; )*
        }
    ) => {{
        $crate::diagnostics::__amdgpu_ordered_program_e32_v1::<
            {
                $crate::ordered_program::select_v1::__checked_ordered_select_v1(
                    $condition,
                    &[$($crate::__fe2o3_ordered_program_step_v1!($true_opcode($($true_role),*))),*],
                    &[$($crate::__fe2o3_ordered_program_step_v1!($false_opcode($($false_role),*))),*],
                ).0
            },
            {
                $crate::ordered_program::select_v1::__checked_ordered_select_v1(
                    $condition,
                    &[$($crate::__fe2o3_ordered_program_step_v1!($true_opcode($($true_role),*))),*],
                    &[$($crate::__fe2o3_ordered_program_step_v1!($false_opcode($($false_role),*))),*],
                ).1[0]
            },
            {
                $crate::ordered_program::select_v1::__checked_ordered_select_v1(
                    $condition,
                    &[$($crate::__fe2o3_ordered_program_step_v1!($true_opcode($($true_role),*))),*],
                    &[$($crate::__fe2o3_ordered_program_step_v1!($false_opcode($($false_role),*))),*],
                ).1[1]
            },
            {
                $crate::ordered_program::select_v1::__checked_ordered_select_v1(
                    $condition,
                    &[$($crate::__fe2o3_ordered_program_step_v1!($true_opcode($($true_role),*))),*],
                    &[$($crate::__fe2o3_ordered_program_step_v1!($false_opcode($($false_role),*))),*],
                ).1[2]
            },
            {
                $crate::ordered_program::select_v1::__checked_ordered_select_v1(
                    $condition,
                    &[$($crate::__fe2o3_ordered_program_step_v1!($true_opcode($($true_role),*))),*],
                    &[$($crate::__fe2o3_ordered_program_step_v1!($false_opcode($($false_role),*))),*],
                ).1[3]
            },
        >($a, $b, $c, $scratch, $output, $input0, $input1, $input2)
    }};
    ($($unsupported:tt)*) => {
        compile_error!("unsupported amdgpu_ordered_program! syntax; use the closed gfx942 u32 program with literal VGPR bindings")
    };
}
