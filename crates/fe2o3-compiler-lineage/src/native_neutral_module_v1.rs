//! Borrowed native-neutral receipt envelope; constituent admission is separate.

use std::{error::Error, fmt};

use crate::{
    InertNativeNeutralSubjectV1, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
    NATIVE_NEUTRAL_SUBJECT_BYTES_V1, NativeNeutralSubjectErrorV1,
};

const MAGIC: [u8; 8] = *b"F2NMOD1\0";
const HEADER: usize = 16 + NATIVE_NEUTRAL_SUBJECT_BYTES_V1;

/// Borrowed exact native-neutral receipt framing, not executable custody.
#[derive(Clone, Copy, Debug)]
pub struct NativeNeutralModuleRefV1<'a> {
    subject: InertNativeNeutralSubjectV1,
    graph: &'a [u8],
    catalog: &'a [u8],
}

impl<'a> NativeNeutralModuleRefV1<'a> {
    /// Checks exact native framing and constituent lengths without decoding a graph.
    /// Consumers must admit both constituents and compare their derived identities.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, NativeNeutralModuleErrorV1> {
        if bytes.len() < HEADER || bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(NativeNeutralModuleErrorV1::Length);
        }
        if bytes[..8] != MAGIC
            || bytes[8..10] != 1_u16.to_le_bytes()
            || bytes[10..12] != 1_u16.to_le_bytes()
        {
            return Err(NativeNeutralModuleErrorV1::Header);
        }
        let mut declared = [0; 4];
        declared.copy_from_slice(&bytes[12..16]);
        if u32::from_le_bytes(declared) as usize != bytes.len() {
            return Err(NativeNeutralModuleErrorV1::Length);
        }
        let subject = InertNativeNeutralSubjectV1::decode(&bytes[16..HEADER])
            .map_err(NativeNeutralModuleErrorV1::Subject)?;
        let graph_length = usize::try_from(subject.graph_length())
            .map_err(|_| NativeNeutralModuleErrorV1::Length)?;
        let catalog_length = usize::try_from(subject.catalog_length())
            .map_err(|_| NativeNeutralModuleErrorV1::Length)?;
        if exact_length(graph_length, catalog_length)? != bytes.len() {
            return Err(NativeNeutralModuleErrorV1::Length);
        }
        let split = HEADER
            .checked_add(graph_length)
            .ok_or(NativeNeutralModuleErrorV1::Length)?;
        Ok(Self {
            subject,
            graph: &bytes[HEADER..split],
            catalog: &bytes[split..],
        })
    }

    /// Borrows the exact inert graph-plus-catalog subject.
    pub const fn subject(&self) -> &InertNativeNeutralSubjectV1 {
        &self.subject
    }
    /// Borrows graph bytes without granting verified owner custody.
    pub const fn graph_bytes(&self) -> &'a [u8] {
        self.graph
    }
    /// Borrows contract catalog bytes without authenticating their source claims.
    pub const fn catalog_bytes(&self) -> &'a [u8] {
        self.catalog
    }
    /// Framing grants no verification, compiler-origin or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Constructs exact native receipt framing from borrowed constituent bytes.
///
/// Lengths are checked against the inert subject. Digest and semantic checks
/// remain mandatory at actual graph/catalog admission; this is not a validator.
/// The existing aggregate lineage byte cap bounds one fallible allocation.
pub fn encode_native_neutral_module_v1(
    subject: &InertNativeNeutralSubjectV1,
    graph: &[u8],
    catalog: &[u8],
) -> Result<Vec<u8>, NativeNeutralModuleErrorV1> {
    if u64::try_from(graph.len()).ok() != Some(subject.graph_length())
        || u64::try_from(catalog.len()).ok() != Some(subject.catalog_length())
    {
        return Err(NativeNeutralModuleErrorV1::Length);
    }
    let length = exact_length(graph.len(), catalog.len())?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| NativeNeutralModuleErrorV1::Allocation)?;
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| NativeNeutralModuleErrorV1::Length)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(subject.canonical_bytes());
    bytes.extend_from_slice(graph);
    bytes.extend_from_slice(catalog);
    Ok(bytes)
}

fn exact_length(graph: usize, catalog: usize) -> Result<usize, NativeNeutralModuleErrorV1> {
    let length = HEADER
        .checked_add(graph)
        .and_then(|n| n.checked_add(catalog))
        .ok_or(NativeNeutralModuleErrorV1::Length)?;
    if length > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
        return Err(NativeNeutralModuleErrorV1::Length);
    }
    Ok(length)
}

/// Malformed native-neutral envelope, independent of graph/catalog validity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeNeutralModuleErrorV1 {
    /// Invalid or overflowing constituent/aggregate lengths, truncation or trailing bytes.
    Length,
    /// Wrong envelope magic/version/policy.
    Header,
    /// The nested native subject frame is invalid.
    Subject(NativeNeutralSubjectErrorV1),
    /// Fallible allocation failed after the aggregate length bound was checked.
    Allocation,
}
impl fmt::Display for NativeNeutralModuleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => formatter.write_str("native-neutral envelope length rejected"),
            Self::Header => formatter.write_str("native-neutral envelope header rejected"),
            Self::Subject(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("native-neutral envelope allocation failed"),
        }
    }
}
impl Error for NativeNeutralModuleErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_borrowed_constituents_are_not_identity_admission() {
        let subject = InertNativeNeutralSubjectV1::new([1; 32], 3, [2; 32], 4).unwrap();
        let bytes = encode_native_neutral_module_v1(&subject, b"kir", b"meta").unwrap();
        let view = NativeNeutralModuleRefV1::decode(&bytes).unwrap();
        assert_eq!(view.subject(), &subject);
        assert_eq!(view.graph_bytes(), b"kir");
        assert_eq!(view.catalog_bytes(), b"meta");
        assert!(std::ptr::eq(
            view.graph_bytes().as_ptr(),
            bytes[HEADER..].as_ptr()
        ));
        assert!(!view.grants_authority());
        let replacement = encode_native_neutral_module_v1(&subject, b"bad", b"fake").unwrap();
        assert!(NativeNeutralModuleRefV1::decode(&replacement).is_ok());
    }

    #[test]
    fn rejects_exact_framing_and_length_mutations() {
        let subject = InertNativeNeutralSubjectV1::new([1; 32], 3, [2; 32], 4).unwrap();
        let bytes = encode_native_neutral_module_v1(&subject, b"kir", b"meta").unwrap();
        for index in 0..16 {
            let mut altered = bytes.clone();
            altered[index] ^= 1;
            assert!(NativeNeutralModuleRefV1::decode(&altered).is_err());
        }
        for end in 0..bytes.len() {
            assert!(NativeNeutralModuleRefV1::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(NativeNeutralModuleRefV1::decode(&trailing).is_err());
        assert!(encode_native_neutral_module_v1(&subject, b"kir!", b"meta").is_err());
        assert!(exact_length(usize::MAX, 1).is_err());
        assert!(exact_length(MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3, 1).is_err());
    }
}
