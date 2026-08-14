use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;

use crate::COMPILER_DESCRIPTOR_SECTION_NAME_V1;

/// LLVM assembly section carrying the retained row-softmax authority transcript.
pub const ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1: &str =
    ".fe2o3.row-softmax-authority-transcript.v1";
/// Maximum retained row-softmax authority-transcript bytes.
pub const MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1: usize = 4_096;
/// LLVM assembly section carrying the row-softmax frontend-authority commitment.
pub const ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1: &str = ".fe2o3.row-softmax-auth.v1";
/// Bytes in the row-softmax frontend-authority commitment.
pub const ROW_SOFTMAX_AUTHORITY_BYTES_V1: usize = 32;
/// LLVM assembly section carrying the row-softmax exponential-boundary commitment.
pub const ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1: &str = ".fe2o3.row-exp.v1";
/// Bytes in the row-softmax exponential-boundary commitment.
pub const ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1: usize = 32;

const COMPILER_SECTION_PREFIX: &[u8] = b"module asm \".section .fe2o3.";
const SECTION_PREFIX: &[u8] = b"module asm \".section ";
const ALIGNMENT: &[u8] = b"module asm \".balign 8\"\n";
const BYTE_LINE_PREFIX: &[u8] = b"module asm \".byte ";
const LINE_SUFFIX: &[u8] = b"\"\n";

/// Canonically decoded compiler-owned row-softmax LLVM assembly suffix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRowSoftmaxCompilerSectionsV1 {
    descriptor: Vec<u8>,
    authority_transcript: Vec<u8>,
    authority: [u8; ROW_SOFTMAX_AUTHORITY_BYTES_V1],
    exponential_boundary: [u8; ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1],
}

impl DecodedRowSoftmaxCompilerSectionsV1 {
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn authority_transcript(&self) -> &[u8] {
        &self.authority_transcript
    }

    pub const fn authority(&self) -> &[u8; ROW_SOFTMAX_AUTHORITY_BYTES_V1] {
        &self.authority
    }

    pub const fn exponential_boundary(&self) -> &[u8; ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1] {
        &self.exponential_boundary
    }
}

/// Failure to decode the exact compiler-owned row-softmax section closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RowSoftmaxCompilerSectionsErrorV1 {
    SectionClosure,
    SectionBoundary,
    DescriptorEncoding,
    AuthorityTranscriptEncoding,
    AuthorityEncoding,
    AuthoritySize,
    ExponentialBoundaryEncoding,
    ExponentialBoundarySize,
    TrailingBytes,
}

impl RowSoftmaxCompilerSectionsErrorV1 {
    /// Stable field label for profile-level error reporting.
    pub const fn profile_field(self) -> &'static str {
        match self {
            Self::SectionClosure => "bound compiler section closure",
            Self::SectionBoundary => "bound compiler section boundary",
            Self::DescriptorEncoding => "compiler descriptor section encoding",
            Self::AuthorityTranscriptEncoding => "frontend-authority transcript encoding",
            Self::AuthorityEncoding => "frontend-authority section encoding",
            Self::AuthoritySize => "frontend-authority commitment size",
            Self::ExponentialBoundaryEncoding => "exponential-boundary section encoding",
            Self::ExponentialBoundarySize => "exponential-boundary commitment size",
            Self::TrailingBytes => "bound compiler section trailing bytes",
        }
    }
}

impl fmt::Display for RowSoftmaxCompilerSectionsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.profile_field())
    }
}

impl Error for RowSoftmaxCompilerSectionsErrorV1 {}

/// Decodes the exact four-section compiler-owned row-softmax LLVM assembly suffix.
///
/// The descriptor and transcript allocations are bounded before growth. The two
/// commitments have exact fixed sizes. No `.fe2o3.*` module-assembly section may
/// precede the suffix, and no bytes may follow it.
pub fn decode_row_softmax_compiler_sections_v1(
    module: &[u8],
) -> Result<DecodedRowSoftmaxCompilerSectionsV1, RowSoftmaxCompilerSectionsErrorV1> {
    let sections = [
        COMPILER_DESCRIPTOR_SECTION_NAME_V1,
        ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1,
        ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1,
        ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1,
    ];
    let headers = sections.map(module_assembly_section_header);
    let positions = headers
        .each_ref()
        .map(|header| unique_position(module, header.as_bytes()));
    let [Some(descriptor_position), Some(_), Some(_), Some(_)] = positions else {
        return Err(RowSoftmaxCompilerSectionsErrorV1::SectionClosure);
    };
    if descriptor_position == 0 || module[descriptor_position - 1] != b'\n' {
        return Err(RowSoftmaxCompilerSectionsErrorV1::SectionBoundary);
    }
    if contains(
        module.get(..descriptor_position).unwrap_or_default(),
        COMPILER_SECTION_PREFIX,
    ) {
        return Err(RowSoftmaxCompilerSectionsErrorV1::SectionClosure);
    }

    let mut offset = descriptor_position;
    let descriptor = decode_section(module, &mut offset, &headers[0], MAX_DESCRIPTOR_TABLE_BYTES)
        .ok_or(RowSoftmaxCompilerSectionsErrorV1::DescriptorEncoding)?;
    let authority_transcript = decode_section(
        module,
        &mut offset,
        &headers[1],
        MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1,
    )
    .filter(|transcript| !transcript.is_empty())
    .ok_or(RowSoftmaxCompilerSectionsErrorV1::AuthorityTranscriptEncoding)?;
    let authority = decode_section(
        module,
        &mut offset,
        &headers[2],
        ROW_SOFTMAX_AUTHORITY_BYTES_V1,
    )
    .ok_or(RowSoftmaxCompilerSectionsErrorV1::AuthorityEncoding)?
    .try_into()
    .map_err(|_| RowSoftmaxCompilerSectionsErrorV1::AuthoritySize)?;
    let exponential_boundary = decode_section(
        module,
        &mut offset,
        &headers[3],
        ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1,
    )
    .ok_or(RowSoftmaxCompilerSectionsErrorV1::ExponentialBoundaryEncoding)?
    .try_into()
    .map_err(|_| RowSoftmaxCompilerSectionsErrorV1::ExponentialBoundarySize)?;
    if offset != module.len() {
        return Err(RowSoftmaxCompilerSectionsErrorV1::TrailingBytes);
    }
    Ok(DecodedRowSoftmaxCompilerSectionsV1 {
        descriptor,
        authority_transcript,
        authority,
        exponential_boundary,
    })
}

fn unique_position(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    let mut positions = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index));
    let position = positions.next()?;
    positions.next().is_none().then_some(position)
}

fn module_assembly_section_header(section: &str) -> String {
    format!("module asm \".section {section},\\22\\22,@progbits\"\n")
}

fn decode_section(
    module: &[u8],
    offset: &mut usize,
    header: &str,
    max_bytes: usize,
) -> Option<Vec<u8>> {
    consume_exact_bytes(module, offset, header.as_bytes())?;
    consume_exact_bytes(module, offset, ALIGNMENT)?;

    let mut result = Vec::new();
    let mut previous_chunk = None;
    while *offset < module.len() && !module[*offset..].starts_with(SECTION_PREFIX) {
        if previous_chunk.is_some_and(|count| count != 16) {
            return None;
        }
        let remaining = &module[*offset..];
        let end = remaining
            .windows(LINE_SUFFIX.len())
            .position(|window| window == LINE_SUFFIX)?;
        let line_end = end.checked_add(LINE_SUFFIX.len())?;
        let values = remaining[..line_end]
            .strip_prefix(BYTE_LINE_PREFIX)?
            .strip_suffix(LINE_SUFFIX)?;
        let mut count = 0_usize;
        for value in values.split(|byte| *byte == b',') {
            if result.len() == max_bytes {
                return None;
            }
            let value = if count == 0 {
                value
            } else {
                value.strip_prefix(b" ")?
            };
            let [b'0', b'x', high, low] = value else {
                return None;
            };
            result.push((decode_hex_nibble(*high)? << 4) | decode_hex_nibble(*low)?);
            count = count.checked_add(1)?;
        }
        if count == 0 || count > 16 {
            return None;
        }
        previous_chunk = Some(count);
        *offset = offset.checked_add(line_end)?;
    }
    previous_chunk.filter(|count| (1..=16).contains(count))?;
    Some(result)
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn consume_exact_bytes(input: &[u8], offset: &mut usize, expected: &[u8]) -> Option<()> {
    let remaining = input.get(*offset..)?;
    if !remaining.starts_with(expected) {
        return None;
    }
    *offset = offset.checked_add(expected.len())?;
    Some(())
}

const fn decode_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTOR: &[u8] = &[0x11; 33];
    const TRANSCRIPT: &[u8] = b"bounded-authority-transcript";
    const AUTHORITY: [u8; ROW_SOFTMAX_AUTHORITY_BYTES_V1] = [0x22; ROW_SOFTMAX_AUTHORITY_BYTES_V1];
    const EXPONENTIAL: [u8; ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1] =
        [0x33; ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_BYTES_V1];

    #[test]
    fn exact_four_section_suffix_decodes() {
        let module = exact_module();
        let decoded = decode_row_softmax_compiler_sections_v1(&module).unwrap();

        assert_eq!(decoded.descriptor(), DESCRIPTOR);
        assert_eq!(decoded.authority_transcript(), TRANSCRIPT);
        assert_eq!(decoded.authority(), &AUTHORITY);
        assert_eq!(decoded.exponential_boundary(), &EXPONENTIAL);
    }

    #[test]
    fn closure_rejects_leading_duplicate_reordered_and_trailing_sections() {
        let exact = exact_sections();
        let leading = module_with_sections(&[
            (".fe2o3.unreviewed.v1", &[0x44]),
            exact[0],
            exact[1],
            exact[2],
            exact[3],
        ]);
        let duplicate = module_with_sections(&[exact[0], exact[1], exact[1], exact[2], exact[3]]);
        let reordered = module_with_sections(&[exact[1], exact[0], exact[2], exact[3]]);
        let trailing = module_with_sections(&[
            exact[0],
            exact[1],
            exact[2],
            exact[3],
            (".fe2o3.unreviewed.v1", &[0x44]),
        ]);

        for module in [leading, duplicate, reordered, trailing] {
            assert!(decode_row_softmax_compiler_sections_v1(&module).is_err());
        }
    }

    #[test]
    fn closure_rejects_truncation_and_noncanonical_chunks() {
        let mut truncated = exact_module();
        truncated.pop();

        let exact = exact_sections();
        let mut noncanonical = module_prefix();
        append_section_with_width(&mut noncanonical, exact[0].0, exact[0].1, 8);
        for (name, bytes) in &exact[1..] {
            append_section(&mut noncanonical, name, bytes);
        }

        assert!(decode_row_softmax_compiler_sections_v1(&truncated).is_err());
        assert!(decode_row_softmax_compiler_sections_v1(&noncanonical).is_err());
    }

    fn exact_sections() -> [(&'static str, &'static [u8]); 4] {
        [
            (COMPILER_DESCRIPTOR_SECTION_NAME_V1, DESCRIPTOR),
            (ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1, TRANSCRIPT),
            (ROW_SOFTMAX_AUTHORITY_SECTION_NAME_V1, &AUTHORITY),
            (
                ROW_SOFTMAX_EXPONENTIAL_BOUNDARY_SECTION_NAME_V1,
                &EXPONENTIAL,
            ),
        ]
    }

    fn exact_module() -> Vec<u8> {
        module_with_sections(&exact_sections())
    }

    fn module_prefix() -> Vec<u8> {
        b"; ModuleID = 'row-softmax-section-test'\n".to_vec()
    }

    fn module_with_sections(sections: &[(&str, &[u8])]) -> Vec<u8> {
        let mut module = module_prefix();
        for (name, bytes) in sections {
            append_section(&mut module, name, bytes);
        }
        module
    }

    fn append_section(module: &mut Vec<u8>, section: &str, bytes: &[u8]) {
        append_section_with_width(module, section, bytes, 16);
    }

    fn append_section_with_width(module: &mut Vec<u8>, section: &str, bytes: &[u8], width: usize) {
        module.extend_from_slice(module_assembly_section_header(section).as_bytes());
        module.extend_from_slice(ALIGNMENT);
        for chunk in bytes.chunks(width) {
            module.extend_from_slice(BYTE_LINE_PREFIX);
            for (index, byte) in chunk.iter().enumerate() {
                if index != 0 {
                    module.extend_from_slice(b", ");
                }
                module.extend_from_slice(format!("0x{byte:02x}").as_bytes());
            }
            module.extend_from_slice(LINE_SUFFIX);
        }
    }
}
