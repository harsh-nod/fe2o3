use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;

use crate::COMPILER_DESCRIPTOR_SECTION_NAME_V1;

pub const FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1: &str =
    ".fe2o3.flash-attention-authority-transcript.v1";
pub const MAX_FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_BYTES_V1: usize = 4_096;
pub const FLASH_ATTENTION_AUTHORITY_SECTION_NAME_V1: &str = ".fe2o3.flash-attention-auth.v1";
pub const FLASH_ATTENTION_AUTHORITY_BYTES_V1: usize = 32;
pub const FLASH_ATTENTION_OCML_EXP_BOUNDARY_SECTION_NAME_V1: &str =
    ".fe2o3.flash-attention-ocml-exp.v1";
pub const FLASH_ATTENTION_OCML_EXP_BOUNDARY_BYTES_V1: usize = 32;

const SECTION_PREFIX: &[u8] = b"module asm \".section ";
const ALIGNMENT: &[u8] = b"module asm \".balign 8\"\n";
const BYTE_LINE_PREFIX: &[u8] = b"module asm \".byte ";
const LINE_SUFFIX: &[u8] = b"\"\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedFlashAttentionCompilerSectionsV1 {
    descriptor: Vec<u8>,
    authority_transcript: Vec<u8>,
    authority: [u8; FLASH_ATTENTION_AUTHORITY_BYTES_V1],
    ocml_exp_boundary: [u8; FLASH_ATTENTION_OCML_EXP_BOUNDARY_BYTES_V1],
}

impl DecodedFlashAttentionCompilerSectionsV1 {
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn authority_transcript(&self) -> &[u8] {
        &self.authority_transcript
    }

    pub const fn authority(&self) -> &[u8; FLASH_ATTENTION_AUTHORITY_BYTES_V1] {
        &self.authority
    }

    pub const fn ocml_exp_boundary(&self) -> &[u8; FLASH_ATTENTION_OCML_EXP_BOUNDARY_BYTES_V1] {
        &self.ocml_exp_boundary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FlashAttentionCompilerSectionsErrorV1 {
    SectionClosure,
    SectionBoundary,
    DescriptorEncoding,
    AuthorityTranscriptEncoding,
    AuthorityEncoding,
    AuthoritySize,
    OcmlExpBoundaryEncoding,
    OcmlExpBoundarySize,
    TrailingBytes,
}

impl FlashAttentionCompilerSectionsErrorV1 {
    pub const fn profile_field(self) -> &'static str {
        match self {
            Self::SectionClosure => "bound compiler section closure",
            Self::SectionBoundary => "bound compiler section boundary",
            Self::DescriptorEncoding => "compiler descriptor section encoding",
            Self::AuthorityTranscriptEncoding => "frontend-authority transcript encoding",
            Self::AuthorityEncoding => "frontend-authority section encoding",
            Self::AuthoritySize => "frontend-authority commitment size",
            Self::OcmlExpBoundaryEncoding => "OCML exponential-boundary section encoding",
            Self::OcmlExpBoundarySize => "OCML exponential-boundary commitment size",
            Self::TrailingBytes => "bound compiler section trailing bytes",
        }
    }
}

impl fmt::Display for FlashAttentionCompilerSectionsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.profile_field())
    }
}

impl Error for FlashAttentionCompilerSectionsErrorV1 {}

/// Decodes the exact four-section suffix emitted only after FlashAttention V1 admission.
pub fn decode_flash_attention_compiler_sections_v1(
    module: &[u8],
) -> Result<DecodedFlashAttentionCompilerSectionsV1, FlashAttentionCompilerSectionsErrorV1> {
    let sections = [
        COMPILER_DESCRIPTOR_SECTION_NAME_V1,
        FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1,
        FLASH_ATTENTION_AUTHORITY_SECTION_NAME_V1,
        FLASH_ATTENTION_OCML_EXP_BOUNDARY_SECTION_NAME_V1,
    ];
    let headers = sections.map(module_assembly_section_header);
    let positions = headers
        .each_ref()
        .map(|header| unique_position(module, header.as_bytes()));
    let [Some(descriptor_position), Some(_), Some(_), Some(_)] = positions else {
        return Err(FlashAttentionCompilerSectionsErrorV1::SectionClosure);
    };
    if descriptor_position == 0 || module[descriptor_position - 1] != b'\n' {
        return Err(FlashAttentionCompilerSectionsErrorV1::SectionBoundary);
    }
    if contains_module_assembly_directive(module.get(..descriptor_position).unwrap_or_default()) {
        return Err(FlashAttentionCompilerSectionsErrorV1::SectionClosure);
    }

    let mut offset = descriptor_position;
    let descriptor = decode_section(module, &mut offset, &headers[0], MAX_DESCRIPTOR_TABLE_BYTES)
        .ok_or(FlashAttentionCompilerSectionsErrorV1::DescriptorEncoding)?;
    let authority_transcript = decode_section(
        module,
        &mut offset,
        &headers[1],
        MAX_FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_BYTES_V1,
    )
    .filter(|transcript| !transcript.is_empty())
    .ok_or(FlashAttentionCompilerSectionsErrorV1::AuthorityTranscriptEncoding)?;
    let authority = decode_section(
        module,
        &mut offset,
        &headers[2],
        FLASH_ATTENTION_AUTHORITY_BYTES_V1,
    )
    .ok_or(FlashAttentionCompilerSectionsErrorV1::AuthorityEncoding)?
    .try_into()
    .map_err(|_| FlashAttentionCompilerSectionsErrorV1::AuthoritySize)?;
    let ocml_exp_boundary = decode_section(
        module,
        &mut offset,
        &headers[3],
        FLASH_ATTENTION_OCML_EXP_BOUNDARY_BYTES_V1,
    )
    .ok_or(FlashAttentionCompilerSectionsErrorV1::OcmlExpBoundaryEncoding)?
    .try_into()
    .map_err(|_| FlashAttentionCompilerSectionsErrorV1::OcmlExpBoundarySize)?;
    if offset != module.len() {
        return Err(FlashAttentionCompilerSectionsErrorV1::TrailingBytes);
    }
    Ok(DecodedFlashAttentionCompilerSectionsV1 {
        descriptor,
        authority_transcript,
        authority,
        ocml_exp_boundary,
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

fn contains_module_assembly_directive(module: &[u8]) -> bool {
    let mut offset = 0_usize;
    let mut previous_was_module = false;
    while offset < module.len() {
        match module[offset] {
            byte if byte.is_ascii_whitespace() => offset += 1,
            b';' => {
                offset += 1;
                while offset < module.len() && module[offset] != b'\n' {
                    offset += 1;
                }
            }
            b'"' => {
                previous_was_module = false;
                offset += 1;
                while offset < module.len() {
                    match module[offset] {
                        b'\\' => offset = offset.saturating_add(2),
                        b'"' => {
                            offset += 1;
                            break;
                        }
                        _ => offset += 1,
                    }
                }
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = offset;
                while offset < module.len()
                    && (module[offset].is_ascii_alphanumeric() || module[offset] == b'_')
                {
                    offset += 1;
                }
                let token = &module[start..offset];
                if previous_was_module && token == b"asm" {
                    return true;
                }
                previous_was_module = token == b"module";
            }
            _ => {
                previous_was_module = false;
                offset += 1;
            }
        }
    }
    false
}

fn consume_exact_bytes(bytes: &[u8], offset: &mut usize, expected: &[u8]) -> Option<()> {
    let end = offset.checked_add(expected.len())?;
    if bytes.get(*offset..end)? != expected {
        return None;
    }
    *offset = end;
    Some(())
}

fn decode_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(name: &str, bytes: &[u8]) -> String {
        let mut output = format!("module asm \".section {name},\\22\\22,@progbits\"\n");
        output.push_str("module asm \".balign 8\"\n");
        for chunk in bytes.chunks(16) {
            output.push_str("module asm \".byte ");
            for (index, byte) in chunk.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                output.push_str(&format!("0x{byte:02x}"));
            }
            output.push_str("\"\n");
        }
        output
    }

    fn exact_module() -> Vec<u8> {
        let mut output = b"target triple = \"amdgcn-amd-amdhsa\"\n".to_vec();
        output.extend(section(COMPILER_DESCRIPTOR_SECTION_NAME_V1, &[1, 2, 3]).bytes());
        output.extend(
            section(
                FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1,
                &[4, 5, 6, 7],
            )
            .bytes(),
        );
        output.extend(section(FLASH_ATTENTION_AUTHORITY_SECTION_NAME_V1, &[8; 32]).bytes());
        output.extend(section(FLASH_ATTENTION_OCML_EXP_BOUNDARY_SECTION_NAME_V1, &[9; 32]).bytes());
        output
    }

    #[test]
    fn exact_flash_section_closure_decodes() {
        let decoded = decode_flash_attention_compiler_sections_v1(&exact_module()).unwrap();
        assert_eq!(decoded.descriptor(), [1, 2, 3]);
        assert_eq!(decoded.authority_transcript(), [4, 5, 6, 7]);
        assert_eq!(decoded.authority(), &[8; 32]);
        assert_eq!(decoded.ocml_exp_boundary(), &[9; 32]);
    }

    #[test]
    fn section_order_duplicates_and_trailing_bytes_fail_closed() {
        let exact = exact_module();
        let descriptor = module_assembly_section_header(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
        let transcript =
            module_assembly_section_header(FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_NAME_V1);
        let descriptor_position = unique_position(&exact, descriptor.as_bytes()).unwrap();
        let transcript_position = unique_position(&exact, transcript.as_bytes()).unwrap();

        let mut duplicate = exact.clone();
        duplicate.extend_from_slice(&exact[descriptor_position..transcript_position]);
        let mut reordered = exact.clone();
        reordered[descriptor_position..].rotate_left(transcript_position - descriptor_position);
        let mut trailing = exact.clone();
        trailing.push(b'\n');
        for hostile in [duplicate, reordered, trailing] {
            assert!(decode_flash_attention_compiler_sections_v1(&hostile).is_err());
        }
    }

    #[test]
    fn noncanonical_bytes_sizes_and_prior_module_assembly_fail_closed() {
        let exact = exact_module();
        let mut uppercase = exact.clone();
        let position = uppercase
            .windows(4)
            .position(|bytes| bytes == b"0x01")
            .unwrap();
        uppercase[position + 2] = b'0';
        uppercase[position + 3] = b'A';

        let mut short_authority = exact.clone();
        let authority_header =
            module_assembly_section_header(FLASH_ATTENTION_AUTHORITY_SECTION_NAME_V1);
        let authority = unique_position(&short_authority, authority_header.as_bytes()).unwrap();
        let byte_line = short_authority[authority..]
            .windows(BYTE_LINE_PREFIX.len())
            .position(|bytes| bytes == BYTE_LINE_PREFIX)
            .unwrap()
            + authority;
        short_authority.splice(byte_line..byte_line + BYTE_LINE_PREFIX.len() + 6, []);

        let mut prior_asm = b"module asm \"nop\"\n".to_vec();
        prior_asm.extend(exact);
        for hostile in [uppercase, short_authority, prior_asm] {
            assert!(decode_flash_attention_compiler_sections_v1(&hostile).is_err());
        }
    }
}
