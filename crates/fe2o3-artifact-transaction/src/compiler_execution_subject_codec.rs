//! Fixed execution-subject wire mechanics, selected by a private version schema.
use super::{CompilerExecutionSubjectErrorV1 as Failure, InertCompilerExecutionContentBindingV1};
use crate::{BuildAttempt, BuildInvocation, BuildSession};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::mem::size_of;

pub(super) const SHA256_BYTES: usize = 32;
pub(super) const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 4;
pub(super) const BUILD_ATTEMPT_BYTES: usize = 8 + 16 + SHA256_BYTES;
pub(super) const SLOT_BYTES: usize = 1 + 7;
pub(super) const COMPILER_CLOSURE_BYTES: usize = (6 * SHA256_BYTES) + 2 + SHA256_BYTES;
pub(super) const CONTENT_BINDING_BYTES: usize = SHA256_BYTES + 8;
pub(super) const CONTENT_BINDING_COUNT: usize = 7;
pub(super) const SUBJECT_PREIMAGE_BYTES: usize = HEADER_BYTES
    + BUILD_ATTEMPT_BYTES
    + SLOT_BYTES
    + SHA256_BYTES
    + SHA256_BYTES
    + COMPILER_CLOSURE_BYTES
    + (CONTENT_BINDING_COUNT * CONTENT_BINDING_BYTES);

pub(super) const BYTES: usize = SUBJECT_PREIMAGE_BYTES + SHA256_BYTES;

// Reader/error slots and result envelopes; full fields/wires are paid separately.
pub(super) const SCALAR_STORAGE: usize = size_of::<Reader<'static>>()
    + 4 * size_of::<Result<[u8; 32], Failure>>()
    + (size_of::<Result<Encoded, Failure>>() - size_of::<Encoded>())
    + (size_of::<Result<(Fields, Encoded), Failure>>() - size_of::<(Fields, Encoded)>());

pub(super) struct Schema {
    pub magic: [u8; 8],
    pub version: u16,
    pub identity_domain: &'static [u8],
    pub transaction_label: &'static str,
    pub outer_label: &'static str,
}

#[derive(Eq, PartialEq)]
pub(super) struct Encoded {
    pub sha256: [u8; 32],
    pub canonical_bytes: [u8; BYTES],
}

#[derive(Eq, PartialEq)]
pub(super) struct Fields {
    pub(super) attempt: BuildAttempt,
    pub(super) slot: u8,
    pub(super) transaction_identity: [u8; 32],
    pub(super) rustc_invocation_sha256: [u8; SHA256_BYTES],
    pub(super) compiler_closure: CompilerClosureV2,
    pub(super) rustc_identity_inventory: InertCompilerExecutionContentBindingV1,
    pub(super) rustc_preflight_plan: InertCompilerExecutionContentBindingV1,
    pub(super) semantic_capsule: InertCompilerExecutionContentBindingV1,
    pub(super) final_compiler_module_commitment: InertCompilerExecutionContentBindingV1,
    pub(super) compiler_module_handoff: InertCompilerExecutionContentBindingV1,
    pub(super) compiler_module_pair_binding: InertCompilerExecutionContentBindingV1,
    pub(super) outer_handoff: InertCompilerExecutionContentBindingV1,
}

impl Fields {
    const fn content_bindings(
        &self,
    ) -> [InertCompilerExecutionContentBindingV1; CONTENT_BINDING_COUNT] {
        [
            self.rustc_identity_inventory,
            self.rustc_preflight_plan,
            self.semantic_capsule,
            self.final_compiler_module_commitment,
            self.compiler_module_handoff,
            self.compiler_module_pair_binding,
            self.outer_handoff,
        ]
    }
}

impl Schema {
    pub(super) fn encode(&self, fields: &Fields) -> Result<Encoded, Failure> {
        if fields.rustc_invocation_sha256 == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: "rustc invocation",
            });
        }
        if fields.transaction_identity == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: self.transaction_label,
            });
        }

        if fields.slot != 0 {
            return Err(Failure::InvalidSlot(fields.slot));
        }
        let mut canonical_bytes = [0_u8; BYTES];
        let mut offset = 0;
        put_slice(&mut canonical_bytes, &mut offset, &self.magic);
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &self.version.to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &(BYTES as u64).to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &fields.attempt.generation().to_le_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            fields.attempt.session().as_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            fields.attempt.invocation().as_bytes(),
        );
        canonical_bytes[offset] = fields.slot;
        offset += SLOT_BYTES;
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &fields.transaction_identity,
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &fields.rustc_invocation_sha256,
        );
        encode_compiler_closure(&mut canonical_bytes, &mut offset, fields.compiler_closure);
        for binding in fields.content_bindings() {
            put_slice(&mut canonical_bytes, &mut offset, binding.sha256());
            put_slice(
                &mut canonical_bytes,
                &mut offset,
                &binding.byte_len().to_le_bytes(),
            );
        }
        debug_assert_eq!(offset, SUBJECT_PREIMAGE_BYTES);
        let sha256 = self.identity(&canonical_bytes[..SUBJECT_PREIMAGE_BYTES]);
        if sha256 == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: "compiler execution subject",
            });
        }
        put_slice(&mut canonical_bytes, &mut offset, &sha256);
        debug_assert_eq!(offset, BYTES);

        Ok(Encoded {
            sha256,
            canonical_bytes,
        })
    }

    pub(super) fn decode(&self, bytes: &[u8]) -> Result<(Fields, Encoded), Failure> {
        if bytes.len() != BYTES {
            return Err(Failure::InvalidLength {
                actual: bytes.len(),
                expected: BYTES,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != self.magic {
            return Err(Failure::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != self.version {
            return Err(Failure::UnsupportedVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(Failure::UnsupportedFlags(flags));
        }
        let declared_len = reader.u64()?;
        if declared_len != BYTES as u64 {
            return Err(Failure::InvalidDeclaredLength(declared_len));
        }
        if reader.u32()? != 0 {
            return Err(Failure::NonzeroReserved);
        }

        let generation = reader.u64()?;
        let session = BuildSession::from_bytes(reader.fixed::<16>()?);
        let invocation = BuildInvocation::from_bytes(reader.fixed::<32>()?);
        let attempt =
            BuildAttempt::new(generation, session, invocation).map_err(Failure::Attempt)?;
        let slot_value = reader.u8()?;
        if slot_value != 0 {
            return Err(Failure::InvalidSlot(slot_value));
        }
        let slot = slot_value;
        if reader.fixed::<7>()? != [0; 7] {
            return Err(Failure::NonzeroReserved);
        }
        let transaction_identity_bytes = reader.fixed::<32>()?;
        if transaction_identity_bytes == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: self.transaction_label,
            });
        }
        let transaction_identity = transaction_identity_bytes;
        let rustc_invocation_sha256 = reader.fixed::<32>()?;
        if rustc_invocation_sha256 == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: "rustc invocation",
            });
        }
        let compiler_closure = decode_compiler_closure(&mut reader)?;

        let mut binding = |field| {
            InertCompilerExecutionContentBindingV1::new(reader.fixed::<32>()?, reader.u64()?, field)
        };
        let fields = Fields {
            attempt,
            slot,
            transaction_identity,
            rustc_invocation_sha256,
            compiler_closure,
            rustc_identity_inventory: binding("rustc identity inventory")?,
            rustc_preflight_plan: binding("rustc preflight plan")?,
            semantic_capsule: binding("semantic capsule")?,
            final_compiler_module_commitment: binding("final compiler module commitment")?,
            compiler_module_handoff: binding("compiler module handoff")?,
            compiler_module_pair_binding: binding("compiler module pair binding")?,
            outer_handoff: binding(self.outer_label)?,
        };
        debug_assert_eq!(reader.offset, SUBJECT_PREIMAGE_BYTES);
        let declared_identity = reader.fixed::<32>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(Failure::ZeroIdentity {
                field: "compiler execution subject",
            });
        }
        if !reader.is_empty() {
            return Err(Failure::TrailingBytes);
        }
        if self.identity(&bytes[..SUBJECT_PREIMAGE_BYTES]) != declared_identity {
            return Err(Failure::SubjectIdentityMismatch);
        }

        let decoded = self.encode(&fields)?;
        if decoded.canonical_bytes.as_slice() != bytes {
            return Err(Failure::NonCanonical);
        }
        Ok((fields, decoded))
    }

    pub(super) fn identity(&self, bytes: &[u8]) -> [u8; SHA256_BYTES] {
        let mut digest = Sha256::new();
        digest.update(self.identity_domain);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }
}
fn encode_compiler_closure(output: &mut [u8], offset: &mut usize, closure: CompilerClosureV2) {
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        put_slice(output, offset, &digest);
    }
    put_slice(
        output,
        offset,
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    put_slice(output, offset, &closure.identity_sha256());
}

fn decode_compiler_closure(reader: &mut Reader<'_>) -> Result<CompilerClosureV2, Failure> {
    let cargo_executable_sha256 = reader.fixed::<32>()?;
    let cargo_binding_trampoline_sha256 = reader.fixed::<32>()?;
    let cargo_fe2o3_binding_wrapper_sha256 = reader.fixed::<32>()?;
    let rustc_executable_sha256 = reader.fixed::<32>()?;
    let rustc_runtime_tree_sha256 = reader.fixed::<32>()?;
    let codegen_backend_sha256 = reader.fixed::<32>()?;
    let transition_version = reader.u16()?;
    let identity_sha256 = reader.fixed::<32>()?;
    CompilerClosureV2::from_pins_and_identity(
        cargo_executable_sha256,
        cargo_binding_trampoline_sha256,
        cargo_fe2o3_binding_wrapper_sha256,
        rustc_executable_sha256,
        rustc_runtime_tree_sha256,
        codegen_backend_sha256,
        transition_version,
        identity_sha256,
    )
    .map_err(Failure::CompilerClosure)
}

fn put_slice(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = offset
        .checked_add(value.len())
        .expect("fixed compiler-execution subject offset cannot overflow");
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], Failure> {
        let end = self.offset.checked_add(N).ok_or(Failure::Truncated)?;
        let value = self.bytes.get(self.offset..end).ok_or(Failure::Truncated)?;
        self.offset = end;
        value.try_into().map_err(|_| Failure::Truncated)
    }

    fn u8(&mut self) -> Result<u8, Failure> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, Failure> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, Failure> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, Failure> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
