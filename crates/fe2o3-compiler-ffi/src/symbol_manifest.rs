use std::{collections::BTreeMap, error::Error, fmt, str};

use sha2::{Digest, Sha256};

const SYMBOL_MANIFEST_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-MODULE-SYMBOL-MANIFEST/V1\0";
const ENCODED_SYMBOL_FIXED_BYTES_V1: usize = 1 + 4;

/// Maximum symbols classified by one compiler module manifest.
pub const MAX_COMPILER_MODULE_SYMBOLS_V1: usize = 16_384;
/// Maximum UTF-8 bytes in one compiler module symbol.
pub const MAX_COMPILER_MODULE_SYMBOL_BYTES_V1: usize = 1_024;
/// Maximum exact canonical bytes in one compiler module symbol manifest.
pub const MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1: usize = 16 * 1024 * 1024;

/// One mutually exclusive compiler-observed role for an LLVM module symbol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerModuleSymbolRoleV1 {
    KernelEntry = 1,
    KernelDescriptor = 2,
    DeviceFfiExport = 3,
    InternalHelper = 4,
    UnresolvedExternalImport = 5,
}

/// SHA-256 and byte length of one exact canonical symbol manifest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleSymbolManifestIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerModuleSymbolManifestIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        self.byte_len == bytes.len() as u64 && self.sha256 == actual
    }

    pub(super) fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }

    pub(super) const fn from_parts(sha256: [u8; 32], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct SymbolRoleEntryV1 {
    role: CompilerModuleSymbolRoleV1,
    symbol: String,
}

/// Bounded canonical symbol-role observation emitted alongside one compiler module.
///
/// Entries are ordered first by role tag and then by exact UTF-8 symbol bytes. A symbol has
/// exactly one role. This records a producer's compiler-origin claim, but public construction and
/// decoding do not authenticate that producer or grant compiler, link, load, or launch authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerModuleSymbolManifestV1 {
    entries: Vec<SymbolRoleEntryV1>,
    canonical_bytes: Vec<u8>,
    identity: CompilerModuleSymbolManifestIdentityV1,
}

impl fmt::Debug for CompilerModuleSymbolManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleSymbolManifestV1")
            .field("symbol_count", &self.entries.len())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl CompilerModuleSymbolManifestV1 {
    /// Constructs a manifest from entries already in strict canonical order.
    pub fn new<I, S>(entries: I) -> Result<Self, CompilerModuleSymbolManifestErrorV1>
    where
        I: IntoIterator<Item = (CompilerModuleSymbolRoleV1, S)>,
        S: Into<String>,
    {
        let mut retained = Vec::new();
        let mut roles_by_symbol = BTreeMap::new();
        let mut exact_size = SYMBOL_MANIFEST_DOMAIN_V1.len() + 4;

        for (role, symbol) in entries {
            if retained.len() == MAX_COMPILER_MODULE_SYMBOLS_V1 {
                return Err(CompilerModuleSymbolManifestErrorV1::TooManySymbols {
                    count: retained.len() + 1,
                });
            }
            let symbol = symbol.into();
            validate_symbol(&symbol)?;

            if let Some(existing_role) = roles_by_symbol.get(symbol.as_str()) {
                return Err(if *existing_role == role {
                    CompilerModuleSymbolManifestErrorV1::DuplicateSymbol
                } else {
                    CompilerModuleSymbolManifestErrorV1::RoleOverlap
                });
            }
            if let Some(previous) = retained.last() {
                let previous: &SymbolRoleEntryV1 = previous;
                if (role, symbol.as_str()) <= (previous.role, previous.symbol.as_str()) {
                    return Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalOrder);
                }
            }

            exact_size = exact_size
                .checked_add(ENCODED_SYMBOL_FIXED_BYTES_V1)
                .and_then(|size| size.checked_add(symbol.len()))
                .ok_or(CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded)?;
            if exact_size > MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 {
                return Err(CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded);
            }

            roles_by_symbol.insert(symbol.clone(), role);
            retained.push(SymbolRoleEntryV1 { role, symbol });
        }

        let mut canonical_bytes = Vec::with_capacity(exact_size);
        canonical_bytes.extend_from_slice(SYMBOL_MANIFEST_DOMAIN_V1);
        push_u32(&mut canonical_bytes, retained.len())?;
        for entry in &retained {
            canonical_bytes.push(entry.role as u8);
            push_u32(&mut canonical_bytes, entry.symbol.len())?;
            canonical_bytes.extend_from_slice(entry.symbol.as_bytes());
        }
        debug_assert_eq!(canonical_bytes.len(), exact_size);
        let identity = CompilerModuleSymbolManifestIdentityV1::calculate(&canonical_bytes);

        Ok(Self {
            entries: retained,
            canonical_bytes,
            identity,
        })
    }

    /// Strictly decodes one complete canonical manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerModuleSymbolManifestErrorV1> {
        if bytes.len() > MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 {
            return Err(CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded);
        }
        let mut cursor = ManifestCursor::new(bytes);
        if cursor.take(SYMBOL_MANIFEST_DOMAIN_V1.len())? != SYMBOL_MANIFEST_DOMAIN_V1 {
            return Err(CompilerModuleSymbolManifestErrorV1::InvalidMagic);
        }
        let count = cursor.u32_as_usize()?;
        if count > MAX_COMPILER_MODULE_SYMBOLS_V1 {
            return Err(CompilerModuleSymbolManifestErrorV1::TooManySymbols { count });
        }
        let minimum_bytes = count
            .checked_mul(ENCODED_SYMBOL_FIXED_BYTES_V1)
            .ok_or(CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded)?;
        if cursor.remaining() < minimum_bytes {
            return Err(CompilerModuleSymbolManifestErrorV1::Truncated);
        }

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let role = decode_role(cursor.byte()?)?;
            let symbol = cursor.text(MAX_COMPILER_MODULE_SYMBOL_BYTES_V1)?;
            entries.push((role, symbol.to_owned()));
        }
        cursor.finish()?;

        let decoded = Self::new(entries)?;
        if decoded.canonical_bytes() != bytes {
            return Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalEncoding);
        }
        Ok(decoded)
    }

    pub const fn identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn entries(
        &self,
    ) -> impl Clone + DoubleEndedIterator<Item = (CompilerModuleSymbolRoleV1, &str)> + ExactSizeIterator
    {
        self.entries
            .iter()
            .map(|entry| (entry.role, entry.symbol.as_str()))
    }

    pub fn symbols(
        &self,
        role: CompilerModuleSymbolRoleV1,
    ) -> impl Clone + DoubleEndedIterator<Item = &str> {
        self.entries
            .iter()
            .filter(move |entry| entry.role == role)
            .map(|entry| entry.symbol.as_str())
    }

    pub fn symbol_count(&self) -> usize {
        self.entries.len()
    }

    pub fn role_count(&self, role: CompilerModuleSymbolRoleV1) -> usize {
        self.symbols(role).count()
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<'a> TryFrom<&'a [u8]> for CompilerModuleSymbolManifestV1 {
    type Error = CompilerModuleSymbolManifestErrorV1;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Failure to construct or strictly decode a compiler module symbol manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerModuleSymbolManifestErrorV1 {
    TooManySymbols { count: usize },
    SymbolByteBoundExceeded,
    ManifestByteBoundExceeded,
    EmptySymbol,
    NulSymbol,
    DuplicateSymbol,
    RoleOverlap,
    NonCanonicalOrder,
    InvalidMagic,
    InvalidRole,
    Truncated,
    TrailingBytes,
    InvalidUtf8,
    NonCanonicalEncoding,
}

impl fmt::Display for CompilerModuleSymbolManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySymbols { count } => {
                write!(
                    formatter,
                    "compiler module symbol count {count} exceeds the bound"
                )
            }
            Self::SymbolByteBoundExceeded => {
                formatter.write_str("compiler module symbol byte bound exceeded")
            }
            Self::ManifestByteBoundExceeded => {
                formatter.write_str("compiler module symbol manifest byte bound exceeded")
            }
            Self::EmptySymbol => formatter.write_str("compiler module symbol is empty"),
            Self::NulSymbol => formatter.write_str("compiler module symbol contains NUL"),
            Self::DuplicateSymbol => formatter.write_str("duplicate compiler module symbol role"),
            Self::RoleOverlap => {
                formatter.write_str("compiler module symbol has incompatible roles")
            }
            Self::NonCanonicalOrder => {
                formatter.write_str("compiler module symbols are not in canonical order")
            }
            Self::InvalidMagic => {
                formatter.write_str("invalid compiler module symbol manifest magic")
            }
            Self::InvalidRole => formatter.write_str("invalid compiler module symbol role"),
            Self::Truncated => formatter.write_str("truncated compiler module symbol manifest"),
            Self::TrailingBytes => {
                formatter.write_str("trailing compiler module symbol manifest bytes")
            }
            Self::InvalidUtf8 => {
                formatter.write_str("invalid UTF-8 in compiler module symbol manifest")
            }
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical compiler module symbol manifest encoding")
            }
        }
    }
}

impl Error for CompilerModuleSymbolManifestErrorV1 {}

fn validate_symbol(symbol: &str) -> Result<(), CompilerModuleSymbolManifestErrorV1> {
    if symbol.is_empty() {
        return Err(CompilerModuleSymbolManifestErrorV1::EmptySymbol);
    }
    if symbol.len() > MAX_COMPILER_MODULE_SYMBOL_BYTES_V1 {
        return Err(CompilerModuleSymbolManifestErrorV1::SymbolByteBoundExceeded);
    }
    if symbol.as_bytes().contains(&0) {
        return Err(CompilerModuleSymbolManifestErrorV1::NulSymbol);
    }
    Ok(())
}

fn decode_role(
    value: u8,
) -> Result<CompilerModuleSymbolRoleV1, CompilerModuleSymbolManifestErrorV1> {
    match value {
        1 => Ok(CompilerModuleSymbolRoleV1::KernelEntry),
        2 => Ok(CompilerModuleSymbolRoleV1::KernelDescriptor),
        3 => Ok(CompilerModuleSymbolRoleV1::DeviceFfiExport),
        4 => Ok(CompilerModuleSymbolRoleV1::InternalHelper),
        5 => Ok(CompilerModuleSymbolRoleV1::UnresolvedExternalImport),
        _ => Err(CompilerModuleSymbolManifestErrorV1::InvalidRole),
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), CompilerModuleSymbolManifestErrorV1> {
    let value = u32::try_from(value)
        .map_err(|_| CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

struct ManifestCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> ManifestCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CompilerModuleSymbolManifestErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(CompilerModuleSymbolManifestErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(CompilerModuleSymbolManifestErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CompilerModuleSymbolManifestErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerModuleSymbolManifestErrorV1::Truncated)
    }

    fn byte(&mut self) -> Result<u8, CompilerModuleSymbolManifestErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u32_as_usize(&mut self) -> Result<usize, CompilerModuleSymbolManifestErrorV1> {
        Ok(u32::from_le_bytes(self.fixed::<4>()?) as usize)
    }

    fn text(&mut self, max: usize) -> Result<&'a str, CompilerModuleSymbolManifestErrorV1> {
        let len = self.u32_as_usize()?;
        if len == 0 {
            return Err(CompilerModuleSymbolManifestErrorV1::EmptySymbol);
        }
        if len > max {
            return Err(CompilerModuleSymbolManifestErrorV1::SymbolByteBoundExceeded);
        }
        str::from_utf8(self.take(len)?)
            .map_err(|_| CompilerModuleSymbolManifestErrorV1::InvalidUtf8)
    }

    fn finish(self) -> Result<(), CompilerModuleSymbolManifestErrorV1> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(CompilerModuleSymbolManifestErrorV1::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<(CompilerModuleSymbolRoleV1, String)> {
        use CompilerModuleSymbolRoleV1 as Role;
        [
            (Role::KernelEntry, "kernel_a"),
            (Role::KernelEntry, "kernel_b"),
            (Role::KernelDescriptor, "kernel_a.kd"),
            (Role::KernelDescriptor, "kernel_b.kd"),
            (Role::DeviceFfiExport, "rust_helper"),
            (Role::InternalHelper, "_Rinternal"),
            (Role::UnresolvedExternalImport, "external_add"),
        ]
        .into_iter()
        .map(|(role, symbol)| (role, symbol.to_owned()))
        .collect()
    }

    fn manifest() -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new(entries()).unwrap()
    }

    fn entry_offsets(bytes: &[u8]) -> Vec<(usize, usize, usize)> {
        let mut position = SYMBOL_MANIFEST_DOMAIN_V1.len() + 4;
        let count = u32::from_le_bytes(
            bytes[SYMBOL_MANIFEST_DOMAIN_V1.len()..SYMBOL_MANIFEST_DOMAIN_V1.len() + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let mut offsets = Vec::with_capacity(count);
        for _ in 0..count {
            let role = position;
            let len =
                u32::from_le_bytes(bytes[position + 1..position + 5].try_into().unwrap()) as usize;
            let symbol = position + 5;
            offsets.push((role, position + 1, symbol));
            position = symbol + len;
        }
        assert_eq!(position, bytes.len());
        offsets
    }

    fn unchecked_encoding(entries: &[(u8, &[u8])]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SYMBOL_MANIFEST_DOMAIN_V1);
        bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (role, symbol) in entries {
            bytes.push(*role);
            bytes.extend_from_slice(&(symbol.len() as u32).to_le_bytes());
            bytes.extend_from_slice(symbol);
        }
        bytes
    }

    #[test]
    fn canonical_round_trip_and_role_projections_are_exact() {
        use CompilerModuleSymbolRoleV1 as Role;
        let first = manifest();
        let second = CompilerModuleSymbolManifestV1::decode(first.canonical_bytes()).unwrap();
        let via_try_from =
            CompilerModuleSymbolManifestV1::try_from(first.canonical_bytes()).unwrap();

        assert_eq!(second, first);
        assert_eq!(via_try_from, first);
        assert_eq!(first.symbol_count(), 7);
        assert_eq!(first.role_count(Role::KernelEntry), 2);
        assert_eq!(
            first.symbols(Role::KernelDescriptor).collect::<Vec<_>>(),
            ["kernel_a.kd", "kernel_b.kd"]
        );
        assert_eq!(first.entries().collect::<Vec<_>>().len(), 7);
        assert!(first.identity().matches(first.canonical_bytes()));
        assert!(!first.authenticates_compiler_origin());
        assert!(!first.grants_compiler_authority());
        assert!(!first.grants_link_authority());
        assert!(!first.grants_load_authority());
        assert!(!first.grants_launch_authority());
    }

    #[test]
    fn constructor_rejects_invalid_names_duplicates_overlap_and_order() {
        use CompilerModuleSymbolRoleV1 as Role;
        for (symbol, expected) in [
            ("", CompilerModuleSymbolManifestErrorV1::EmptySymbol),
            ("bad\0name", CompilerModuleSymbolManifestErrorV1::NulSymbol),
        ] {
            assert_eq!(
                CompilerModuleSymbolManifestV1::new([(Role::KernelEntry, symbol)]),
                Err(expected)
            );
        }
        assert_eq!(
            CompilerModuleSymbolManifestV1::new([
                (Role::KernelEntry, "kernel"),
                (Role::KernelEntry, "kernel"),
            ]),
            Err(CompilerModuleSymbolManifestErrorV1::DuplicateSymbol)
        );
        assert_eq!(
            CompilerModuleSymbolManifestV1::new([
                (Role::KernelEntry, "same"),
                (Role::InternalHelper, "same"),
            ]),
            Err(CompilerModuleSymbolManifestErrorV1::RoleOverlap)
        );
        assert_eq!(
            CompilerModuleSymbolManifestV1::new([
                (Role::KernelEntry, "z"),
                (Role::KernelEntry, "a"),
            ]),
            Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalOrder)
        );
        assert_eq!(
            CompilerModuleSymbolManifestV1::new([
                (Role::InternalHelper, "helper"),
                (Role::KernelEntry, "kernel"),
            ]),
            Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalOrder)
        );
        assert_eq!(
            CompilerModuleSymbolManifestV1::new([(
                Role::KernelEntry,
                "x".repeat(MAX_COMPILER_MODULE_SYMBOL_BYTES_V1 + 1),
            )]),
            Err(CompilerModuleSymbolManifestErrorV1::SymbolByteBoundExceeded)
        );
    }

    #[test]
    fn strict_decoder_rejects_all_truncations_and_trailing_bytes() {
        let encoded = manifest().canonical_bytes().to_vec();
        for length in 0..encoded.len() {
            assert!(
                CompilerModuleSymbolManifestV1::decode(&encoded[..length]).is_err(),
                "accepted prefix of length {length}"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&trailing),
            Err(CompilerModuleSymbolManifestErrorV1::TrailingBytes)
        );
    }

    #[test]
    fn decoder_rejects_invalid_role_utf8_nul_order_duplicates_and_overlap() {
        let original = manifest();
        let offsets = entry_offsets(original.canonical_bytes());

        let mut encoded = original.canonical_bytes().to_vec();
        encoded[offsets[0].0] = 0xff;
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::InvalidRole)
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[offsets[0].2] = 0xff;
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::InvalidUtf8)
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[offsets[0].2] = 0;
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::NulSymbol)
        );

        encoded = original.canonical_bytes().to_vec();
        encoded[offsets[1].0] = CompilerModuleSymbolRoleV1::InternalHelper as u8;
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalOrder)
        );

        let duplicate = unchecked_encoding(&[(1, b"same"), (1, b"same")]);
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&duplicate),
            Err(CompilerModuleSymbolManifestErrorV1::DuplicateSymbol)
        );
        let overlap = unchecked_encoding(&[(1, b"same"), (4, b"same")]);
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&overlap),
            Err(CompilerModuleSymbolManifestErrorV1::RoleOverlap)
        );
        let noncanonical = unchecked_encoding(&[(1, b"z"), (1, b"a")]);
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&noncanonical),
            Err(CompilerModuleSymbolManifestErrorV1::NonCanonicalOrder)
        );
        let empty = unchecked_encoding(&[(1, b"")]);
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&empty),
            Err(CompilerModuleSymbolManifestErrorV1::EmptySymbol)
        );
    }

    #[test]
    fn declared_count_and_symbol_bounds_fail_closed() {
        let original = manifest();
        let mut encoded = original.canonical_bytes().to_vec();
        encoded[SYMBOL_MANIFEST_DOMAIN_V1.len()..SYMBOL_MANIFEST_DOMAIN_V1.len() + 4]
            .copy_from_slice(&((MAX_COMPILER_MODULE_SYMBOLS_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::TooManySymbols {
                count: MAX_COMPILER_MODULE_SYMBOLS_V1 + 1
            })
        );

        encoded = original.canonical_bytes().to_vec();
        let length_offset = entry_offsets(&encoded)[0].1;
        encoded[length_offset..length_offset + 4]
            .copy_from_slice(&((MAX_COMPILER_MODULE_SYMBOL_BYTES_V1 as u32) + 1).to_le_bytes());
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&encoded),
            Err(CompilerModuleSymbolManifestErrorV1::SymbolByteBoundExceeded)
        );

        let oversized = vec![0; MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 + 1];
        assert_eq!(
            CompilerModuleSymbolManifestV1::decode(&oversized),
            Err(CompilerModuleSymbolManifestErrorV1::ManifestByteBoundExceeded)
        );
    }

    #[test]
    fn identity_binds_roles_symbols_and_order() {
        use CompilerModuleSymbolRoleV1 as Role;
        let first = manifest();
        let changed_symbol = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel_c"),
            (Role::KernelDescriptor, "kernel_c.kd"),
        ])
        .unwrap();
        let changed_role = CompilerModuleSymbolManifestV1::new([
            (Role::InternalHelper, "kernel_a"),
            (Role::UnresolvedExternalImport, "external_add"),
        ])
        .unwrap();

        assert_ne!(first.identity(), changed_symbol.identity());
        assert_ne!(first.identity(), changed_role.identity());
        assert_ne!(first.canonical_bytes(), changed_symbol.canonical_bytes());
    }

    #[test]
    fn canonical_encoding_has_a_stable_golden_value() {
        use CompilerModuleSymbolRoleV1 as Role;
        let value = CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "k"),
            (Role::KernelDescriptor, "k.kd"),
            (Role::DeviceFfiExport, "e"),
            (Role::InternalHelper, "h"),
            (Role::UnresolvedExternalImport, "i"),
        ])
        .unwrap();
        let hex = value
            .canonical_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "4645324f332f434f4d50494c45522d4d4f44554c452d53594d424f4c2d4d414e49464553542f5631000500000001010000006b02040000006b2e6b64030100000065040100000068050100000069"
        );
        assert_eq!(
            value.identity().sha256(),
            &[
                0x2d, 0x95, 0xc0, 0x3f, 0xfa, 0x81, 0x33, 0x6f, 0xa9, 0x70, 0xeb, 0xd9, 0xd2, 0xb6,
                0x37, 0xbc, 0x96, 0x89, 0x8d, 0xf1, 0x59, 0x70, 0x13, 0xa5, 0x5a, 0xfc, 0x6d, 0x0e,
                0x7f, 0x64, 0x8e, 0x8f,
            ]
        );
    }

    #[test]
    fn debug_output_does_not_expose_symbols() {
        let debug = format!("{:?}", manifest());
        for symbol in ["kernel_a", "rust_helper", "external_add"] {
            assert!(
                !debug.contains(symbol),
                "debug output leaked `{symbol}`: {debug}"
            );
        }
    }
}
