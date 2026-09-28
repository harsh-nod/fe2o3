//! Inert, bounded inventory of approved compiler code, not evidence of execution.
use crate::{
    COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1, CompilerClosureErrorV2,
    CompilerClosureV2,
};
use sha2::{Digest, Sha256};
use std::{
    convert::Infallible,
    fmt,
    mem::size_of,
    path::{Component, Path},
};

/// Closed V1 wire discriminator.
pub const COMPILER_RUNTIME_MANIFEST_MAGIC_V1: [u8; 8] = *b"F2CRM1\0\0";
/// Manifest identity domain; followed by preimage length as LE u64 and preimage.
pub const COMPILER_RUNTIME_MANIFEST_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-compiler-runtime-manifest-v1\0";
/// Header containing framing, full compiler closure and proof-runtime identity.
pub const COMPILER_RUNTIME_MANIFEST_HEADER_BYTES_V1: usize = 288;
/// Fixed entry extent: role, reserved bytes, path length, file length, hash, path.
pub const COMPILER_RUNTIME_MANIFEST_ENTRY_BYTES_V1: usize = 304;
/// Maximum distinct code files, including all transitive shared libraries.
pub const COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1: usize = 128;
/// Maximum ASCII relative path length; paths are canonical, not loader searches.
pub const COMPILER_RUNTIME_MANIFEST_MAX_PATH_BYTES_V1: usize = 256;
/// Maximum components in a relative code path.
pub const COMPILER_RUNTIME_MANIFEST_MAX_PATH_COMPONENTS_V1: usize = 16;
/// Maximum exact length of any code file.
pub const COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1: u64 = 1024 * 1024 * 1024;
/// Maximum aggregate backing retained by one inventory.
pub const COMPILER_RUNTIME_MANIFEST_MAX_TOTAL_BYTES_V1: u64 = 4 * 1024 * 1024 * 1024;
/// Maximum canonical wire extent, including its terminal digest.
pub const COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1: usize = COMPILER_RUNTIME_MANIFEST_HEADER_BYTES_V1
    + COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 * COMPILER_RUNTIME_MANIFEST_ENTRY_BYTES_V1
    + 32;
/// Fixed prepayment for bounded parsing, copies, path walks and hashing.
pub const COMPILER_RUNTIME_MANIFEST_WORK_V1: usize = 16 * COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1;
/// Result and scratch allowance, excluding caller input backing/callback captures.
/// The codec performs no heap allocation. Reserve before calling either constructor.
pub const COMPILER_RUNTIME_MANIFEST_STORAGE_V1: usize =
    3 * size_of::<CompilerRuntimeManifestV1>() + 4 * size_of::<Sha256>() + 8192;
const HEADER: usize = COMPILER_RUNTIME_MANIFEST_HEADER_BYTES_V1;
const ENTRY: usize = COMPILER_RUNTIME_MANIFEST_ENTRY_BYTES_V1;
const MAX: usize = COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1;

/// Closed code roles, not permission to execute a file or to create descendants.
/// V1 requires exactly one of each singleton and at least one shared library.
/// The interpreter is required for rustc. The proof helper may be static, with
/// no PT_INTERP. If the helper has PT_INTERP, it must resolve to this interpreter.
/// These are inventory requirements for the guard, not ELF parsing by this codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerRuntimeRoleV1 {
    /// The actual rustc executable; digest must equal its existing closure pin.
    Rustc = 1,
    /// The actual fe2o3 codegen DSO; digest must equal its existing closure pin.
    CodegenBackend = 2,
    /// The actual fe2o3_macros DSO, not a stand-in or arbitrary proc macro.
    Fe2o3ProcMacro = 3,
    /// Required rustc ELF interpreter; also used by a helper only if dynamic.
    ElfInterpreter = 4,
    /// Coordinator-owned sibling proof executor, not a proof-signing service.
    /// May be static; this role does not require a PT_INTERP segment.
    ProofExecutorHelper = 5,
    /// Transitive rustc/LLVM, proc-macro or helper shared-library code.
    SharedLibrary = 6,
}
impl CompilerRuntimeRoleV1 {
    /// Exact protected mode: executables/interpreter are 0555, DSOs are 0444.
    pub const fn protected_mode(self) -> u32 {
        match self {
            Self::Rustc | Self::ElfInterpreter | Self::ProofExecutorHelper => 0o555,
            Self::CodegenBackend | Self::Fe2o3ProcMacro | Self::SharedLibrary => 0o444,
        }
    }
    fn decode<E>(value: u16) -> Result<Self, Error<E>> {
        match value {
            1 => Ok(Self::Rustc),
            2 => Ok(Self::CodegenBackend),
            3 => Ok(Self::Fe2o3ProcMacro),
            4 => Ok(Self::ElfInterpreter),
            5 => Ok(Self::ProofExecutorHelper),
            6 => Ok(Self::SharedLibrary),
            _ => Err(Error::Role),
        }
    }
}

/// Borrowed inert entry used for construction and inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerRuntimeEntryV1<'a> {
    /// Closed semantic role; labels alone do not verify ELF dependencies.
    pub role: CompilerRuntimeRoleV1,
    /// Canonical path relative to the fixed protected compiler runtime root.
    pub path: &'a str,
    /// Exact, nonzero file length.
    pub length: u64,
    /// SHA256 of every file byte, not just its executable sections.
    pub sha256: [u8; 32],
}

/// Bounded framing, semantic or work-refusal diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerRuntimeManifestErrorV1<E = Infallible> {
    /// Caller refused prepayment; no payload processing occurred.
    Charge(E),
    /// Unsupported or inconsistent record extent/count.
    Length,
    /// Unknown magic/schema/enforcement version or header extent.
    Header,
    /// Nonzero reserved bytes or unused path padding.
    Reserved,
    /// Invalid existing full compiler closure.
    CompilerClosure(CompilerClosureErrorV2),
    /// Missing/unknown/duplicate singleton role.
    Role,
    /// Noncanonical, duplicate, out-of-order or traversal path.
    Path,
    /// Zero digest, zero proof-runtime identity, or mismatched rustc/backend pin.
    Digest,
    /// Empty, excessive or overflowing file/aggregate length.
    FileLength,
    /// Manifest's terminal content identity differs.
    Identity,
}
type Error<E> = CompilerRuntimeManifestErrorV1<E>;
impl<E: fmt::Debug> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compiler runtime manifest: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {}

/// Canonical inert inventory. No constructor reads a path or approves a compiler.
///
/// Header: magic[8], schema u16=1, header length u16=288, total length u32,
/// count u16, required enforcement u16=1, closure transition u16, reserved[10],
/// seven closure digests[224], proof-runtime identity[32]. Each sorted entry:
/// role u16, reserved[2], path length u16, reserved[2], file length u64,
/// file SHA256[32], zero-padded relative path[256]. All integers are LE.
/// The terminal SHA256 binds the domain, LE u64 preimage length and all preceding
/// bytes. Shortened/padded/alternate encodings and unknown roles refuse.
///
/// This inventory is separate from the six compiler pins. It names possible code,
/// not observed code. ELF PT_LOAD/PF_X range derivation, dependency resolution,
/// descendant limits, memory/FD mediation and isolation belong to the runtime
/// guard; neither decoding nor a complete list proves any of them occurred.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerRuntimeManifestV1 {
    bytes: [u8; MAX],
    length: usize,
    closure: CompilerClosureV2,
    total: u64,
}
impl CompilerRuntimeManifestV1 {
    /// Construct canonical bytes from a strictly path-sorted inventory.
    /// Prepay the documented storage allowance separately; all work is charged
    /// before entry inspection, copying or hashing, and is never refunded.
    pub fn new<E>(
        closure: CompilerClosureV2,
        proof_runtime_identity: [u8; 32],
        entries: &[CompilerRuntimeEntryV1<'_>],
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        let length = wire_length(entries.len())?;
        charge(COMPILER_RUNTIME_MANIFEST_WORK_V1).map_err(Error::Charge)?;
        drop(charge);
        if proof_runtime_identity == [0; 32] {
            return Err(Error::Digest);
        }
        let total = validate_entries(closure, entries.iter().copied())?;
        let mut bytes = [0; MAX];
        bytes[..8].copy_from_slice(&COMPILER_RUNTIME_MANIFEST_MAGIC_V1);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        bytes[16..18].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        bytes[18..20].copy_from_slice(
            &COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1.to_le_bytes(),
        );
        bytes[20..22].copy_from_slice(
            &closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        let pins = [
            closure.cargo_executable_sha256(),
            closure.cargo_binding_trampoline_sha256(),
            closure.cargo_fe2o3_binding_wrapper_sha256(),
            closure.rustc_executable_sha256(),
            closure.rustc_runtime_tree_sha256(),
            closure.codegen_backend_sha256(),
            closure.identity_sha256(),
        ];
        for (slot, pin) in bytes[32..256].chunks_exact_mut(32).zip(pins) {
            slot.copy_from_slice(&pin);
        }
        bytes[256..HEADER].copy_from_slice(&proof_runtime_identity);
        for (slot, entry) in bytes[HEADER..length - 32]
            .chunks_exact_mut(ENTRY)
            .zip(entries)
        {
            slot[..2].copy_from_slice(&(entry.role as u16).to_le_bytes());
            slot[4..6].copy_from_slice(&(entry.path.len() as u16).to_le_bytes());
            slot[8..16].copy_from_slice(&entry.length.to_le_bytes());
            slot[16..48].copy_from_slice(&entry.sha256);
            slot[48..48 + entry.path.len()].copy_from_slice(entry.path.as_bytes());
        }
        let digest = identity(&bytes[..length - 32]);
        bytes[length - 32..length].copy_from_slice(&digest);
        Ok(Self {
            bytes,
            length,
            closure,
            total,
        })
    }

    /// Decode exact canonical bytes, with no heap allocation or trusted origin.
    pub fn decode<E>(
        bytes: &[u8],
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Error<E>> {
        if bytes.len() < HEADER + 32 || bytes.len() > MAX {
            return Err(Error::Length);
        }
        charge(COMPILER_RUNTIME_MANIFEST_WORK_V1).map_err(Error::Charge)?;
        drop(charge);
        if bytes[..8] != COMPILER_RUNTIME_MANIFEST_MAGIC_V1
            || u16_at(bytes, 8) != 1
            || u16_at(bytes, 10) != HEADER as u16
            || u16_at(bytes, 18) != COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1
        {
            return Err(Error::Header);
        }
        let length = wire_length(usize::from(u16_at(bytes, 16)))?;
        if bytes.len() != length
            || u32::from_le_bytes(bytes[12..16].try_into().expect("fixed header")) as usize
                != length
        {
            return Err(Error::Length);
        }
        if bytes[22..32] != [0; 10] {
            return Err(Error::Reserved);
        }
        if bytes[256..HEADER] == [0; 32] {
            return Err(Error::Digest);
        }
        let pins: [[u8; 32]; 7] = bytes[32..256]
            .as_chunks::<32>()
            .0
            .try_into()
            .expect("fixed pins");
        let closure = CompilerClosureV2::from_pins_and_identity(
            pins[0],
            pins[1],
            pins[2],
            pins[3],
            pins[4],
            pins[5],
            u16_at(bytes, 20),
            pins[6],
        )
        .map_err(Error::CompilerClosure)?;
        // Validate framing before exposing the infallible borrowed entry view.
        for slot in bytes[HEADER..length - 32].chunks_exact(ENTRY) {
            decode_entry::<E>(slot)?;
        }
        let total = validate_entries(
            closure,
            bytes[HEADER..length - 32].chunks_exact(ENTRY).map(|s| {
                decode_entry::<E>(s).unwrap_or_else(|_| unreachable!("entry already checked"))
            }),
        )?;
        if bytes[length - 32..] != identity(&bytes[..length - 32]) {
            return Err(Error::Identity);
        }
        let mut owned = [0; MAX];
        owned[..length].copy_from_slice(bytes);
        Ok(Self {
            bytes: owned,
            length,
            closure,
            total,
        })
    }
    /// Exact active wire bytes; unused internal capacity is not serialized.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    /// Exact domain-separated content identity, never a signature or approval.
    pub fn identity(&self) -> &[u8; 32] {
        self.bytes[self.length - 32..self.length]
            .try_into()
            .expect("terminal digest")
    }
    /// The unchanged full compiler closure, including all six pins and transition.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.closure
    }
    /// Existing proof-runtime identity required by the separate proof executor.
    pub fn proof_runtime_identity(&self) -> &[u8; 32] {
        self.bytes[256..HEADER].try_into().expect("fixed identity")
    }
    /// Declared, not observed, executable backing length.
    pub const fn total_file_bytes(&self) -> u64 {
        self.total
    }
    /// Inventory entries in strictly increasing canonical path order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = CompilerRuntimeEntryV1<'_>> {
        self.bytes[HEADER..self.length - 32]
            .chunks_exact(ENTRY)
            .map(|slot| decode_entry::<Infallible>(slot).expect("validated entry"))
    }
    /// An inert inventory does not grant any authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
fn wire_length<E>(count: usize) -> Result<usize, Error<E>> {
    if !(6..=COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1).contains(&count) {
        return Err(Error::Length);
    }
    Ok(HEADER + count * ENTRY + 32)
}
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn decode_entry<E>(slot: &[u8]) -> Result<CompilerRuntimeEntryV1<'_>, Error<E>> {
    let role = CompilerRuntimeRoleV1::decode(u16_at(slot, 0))?;
    let len = usize::from(u16_at(slot, 4));
    if len == 0 || len > COMPILER_RUNTIME_MANIFEST_MAX_PATH_BYTES_V1 {
        return Err(Error::Path);
    }
    if slot[2..4] != [0; 2] || slot[6..8] != [0; 2] || slot[48 + len..].iter().any(|b| *b != 0) {
        return Err(Error::Reserved);
    }
    let path = std::str::from_utf8(&slot[48..48 + len]).map_err(|_| Error::Path)?;
    Ok(CompilerRuntimeEntryV1 {
        role,
        path,
        length: u64::from_le_bytes(slot[8..16].try_into().expect("fixed length")),
        sha256: slot[16..48].try_into().expect("fixed digest"),
    })
}
fn validate_entries<'a, E>(
    closure: CompilerClosureV2,
    entries: impl Iterator<Item = CompilerRuntimeEntryV1<'a>>,
) -> Result<u64, Error<E>> {
    let mut roles = 0_u16;
    let mut previous = "";
    let mut total = 0_u64;
    for entry in entries {
        validate_path::<E>(entry.path)?;
        if entry.path <= previous {
            return Err(Error::Path);
        }
        previous = entry.path;
        let bit = 1 << (entry.role as u16 - 1);
        if entry.role != CompilerRuntimeRoleV1::SharedLibrary && roles & bit != 0 {
            return Err(Error::Role);
        }
        roles |= bit;
        if entry.sha256 == [0; 32]
            || entry.role == CompilerRuntimeRoleV1::Rustc
                && entry.sha256 != closure.rustc_executable_sha256()
            || entry.role == CompilerRuntimeRoleV1::CodegenBackend
                && entry.sha256 != closure.codegen_backend_sha256()
        {
            return Err(Error::Digest);
        }
        if entry.length == 0 || entry.length > COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1 {
            return Err(Error::FileLength);
        }
        total = total
            .checked_add(entry.length)
            .filter(|t| *t <= COMPILER_RUNTIME_MANIFEST_MAX_TOTAL_BYTES_V1)
            .ok_or(Error::FileLength)?;
    }
    if roles != 0b11_1111 {
        return Err(Error::Role);
    }
    Ok(total)
}
fn validate_path<E>(path: &str) -> Result<(), Error<E>> {
    if path.is_empty()
        || path.len() > COMPILER_RUNTIME_MANIFEST_MAX_PATH_BYTES_V1
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._+-".contains(&b))
    {
        return Err(Error::Path);
    }
    let mut extent = 0;
    let mut count = 0;
    for component in Path::new(path).components() {
        let Component::Normal(name) = component else {
            return Err(Error::Path);
        };
        // Components normalizes `//`, interior `./` and trailing `/`. Requiring
        // the original extent rejects those aliases without resolving the path.
        extent += name.len() + usize::from(count != 0);
        count += 1;
    }
    if extent != path.len() || count > COMPILER_RUNTIME_MANIFEST_MAX_PATH_COMPONENTS_V1 {
        return Err(Error::Path);
    }
    Ok(())
}
fn identity(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(COMPILER_RUNTIME_MANIFEST_IDENTITY_DOMAIN_V1);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
