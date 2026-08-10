//! Bounded gfx942 diagnostics and typed inline operations.
//!
//! These entry points are semantic markers for the fe2o3 backend. On a host or
//! through an unrecognized compiler path they panic instead of pretending to
//! provide device behavior.

/// Maximum UTF-8 byte length admitted by the bounded format contract.
pub const MAX_DIAGNOSTIC_FORMAT_BYTES_V1: usize = 96;

/// Maximum number of typed values accepted by the bounded format contract.
pub const MAX_DIAGNOSTIC_ARGUMENTS_V1: usize = 2;

const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Validates the bounded V1 format grammar and returns its stable identity.
///
/// V1 accepts printable ASCII plus newline and tab. `{}` consumes one `u32`;
/// `{{` and `}}` encode literal braces. Other formatting syntax fails closed.
#[doc(hidden)]
pub const fn __checked_format_id_v1(format: &str, expected_arguments: usize) -> Option<u32> {
    let bytes = format.as_bytes();
    if bytes.len() > MAX_DIAGNOSTIC_FORMAT_BYTES_V1
        || expected_arguments > MAX_DIAGNOSTIC_ARGUMENTS_V1
    {
        return None;
    }

    let mut index = 0usize;
    let mut arguments = 0usize;
    let mut hash = FNV_OFFSET ^ expected_arguments as u32;
    while index < bytes.len() {
        let byte = bytes[index];
        if !matches!(byte, b'\n' | b'\t' | 0x20..=0x7e) {
            return None;
        }
        if byte == b'{' {
            if index + 1 >= bytes.len() {
                return None;
            }
            match bytes[index + 1] {
                b'{' => index += 2,
                b'}' => {
                    arguments += 1;
                    if arguments > expected_arguments {
                        return None;
                    }
                    index += 2;
                }
                _ => return None,
            }
        } else if byte == b'}' {
            if index + 1 >= bytes.len() || bytes[index + 1] != b'}' {
                return None;
            }
            index += 2;
        } else {
            index += 1;
        }
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    if arguments != expected_arguments {
        return None;
    }
    Some(if hash == 0 { 1 } else { hash })
}

/// Hashes source metadata into a nonzero bounded diagnostic site identity.
#[doc(hidden)]
pub const fn __site_id_v1(site: &str) -> u32 {
    let bytes = site.as_bytes();
    let mut index = 0usize;
    let mut hash = FNV_OFFSET;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        index += 1;
    }
    if hash == 0 { 1 } else { hash }
}

macro_rules! device_operation {
    ($name:ident, $marker:literal, ($($argument:ident: $ty:ty),*) -> $result:ty) => {
        #[doc(hidden)]
        #[inline(never)]
        #[rustc_diagnostic_item = $marker]
        pub fn $name($($argument: $ty),*) -> $result {
            let _ = ($($argument,)*);
            unreachable!(concat!(stringify!($name), " must be lowered by the fe2o3 backend"))
        }
    };
}

device_operation!(__amdgpu_v_mov_b32_v1, "fe2o3_device_amdgpu_v_mov_b32_v1", (value: u32) -> u32);
device_operation!(__amdgpu_v_add_u32_v1, "fe2o3_device_amdgpu_v_add_u32_v1", (lhs: u32, rhs: u32) -> u32);
device_operation!(__amdgpu_v_sub_u32_v1, "fe2o3_device_amdgpu_v_sub_u32_v1", (lhs: u32, rhs: u32) -> u32);
device_operation!(__amdgpu_v_and_b32_v1, "fe2o3_device_amdgpu_v_and_b32_v1", (lhs: u32, rhs: u32) -> u32);
device_operation!(__amdgpu_v_or_b32_v1, "fe2o3_device_amdgpu_v_or_b32_v1", (lhs: u32, rhs: u32) -> u32);
device_operation!(__amdgpu_v_xor_b32_v1, "fe2o3_device_amdgpu_v_xor_b32_v1", (lhs: u32, rhs: u32) -> u32);
device_operation!(__gpu_printf_0_v1, "fe2o3_device_gpu_printf_0_v1", (format_id: u32) -> ());
device_operation!(__gpu_printf_1_v1, "fe2o3_device_gpu_printf_1_v1", (format_id: u32, value0: u32) -> ());
device_operation!(__gpu_printf_2_v1, "fe2o3_device_gpu_printf_2_v1", (format_id: u32, value0: u32, value1: u32) -> ());
device_operation!(__gpu_assert_fail_v1, "fe2o3_device_gpu_assert_fail_v1", (site_id: u32, line: u32) -> ());
device_operation!(clock32, "fe2o3_device_clock32_v1", () -> u32);
device_operation!(trap, "fe2o3_device_trap_v1", () -> ());
device_operation!(debugtrap, "fe2o3_device_debugtrap_v1", () -> ());
device_operation!(__profiling_marker_v1, "fe2o3_device_profiling_marker_v1", (marker: u32) -> ());

#[cfg(test)]
mod tests {
    use super::__checked_format_id_v1;

    #[test]
    fn bounded_format_grammar_accepts_only_exact_u32_slots() {
        assert!(__checked_format_id_v1("ready", 0).is_some());
        assert!(__checked_format_id_v1("x={} y={} {{ok}}\n", 2).is_some());
        assert_eq!(
            __checked_format_id_v1("x={}", 1),
            __checked_format_id_v1("x={}", 1)
        );
        assert_ne!(
            __checked_format_id_v1("x={}", 1),
            __checked_format_id_v1("y={}", 1)
        );
    }

    #[test]
    fn bounded_format_grammar_rejects_ambiguous_or_oversized_inputs() {
        assert_eq!(__checked_format_id_v1("{}", 0), None);
        assert_eq!(__checked_format_id_v1("plain", 1), None);
        assert_eq!(__checked_format_id_v1("{0}", 1), None);
        assert_eq!(__checked_format_id_v1("{:x}", 1), None);
        assert_eq!(__checked_format_id_v1("unclosed {", 0), None);
        assert_eq!(__checked_format_id_v1("unclosed }", 0), None);
        assert_eq!(__checked_format_id_v1("bad\0", 0), None);
        assert_eq!(
            __checked_format_id_v1(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                0
            ),
            None
        );
        assert_eq!(__checked_format_id_v1("{} {} {}", 3), None);
    }
}
