//! Canonical inert import contracts; graph and source admission are separate checks.

use std::{error::Error, fmt, mem::size_of};

use sha2::{Digest, Sha256};

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

/// Maximum canonical catalog length, independent of the graph's wire bound.
pub const MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1: usize = 4 * 1024 * 1024;
const MAGIC: [u8; 8] = *b"F2KCAT1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-KIR-IMPORT-CONTRACT-CATALOG/V1\0";
const HEADER: usize = 56;
const DEFINITION_BYTES: usize = 48;
const BINDING_BYTES: usize = 20;

/// Inert source pipeline geometry and exact packed payload layout.
/// Source authentication comes from replay of the bound semantic source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPipelineContractDefinitionV1 {
    /// Dense zero-based catalog key used by KIR verification markers.
    pub key: u32,
    /// Pipeline type index in the bound semantic source.
    pub semantic_pipeline_type: u32,
    /// Payload type index in the bound semantic source.
    pub semantic_payload_type: u32,
    /// Number of rotating workgroup storage buffers.
    pub buffers: u32,
    /// Number of packed payload elements in each buffer.
    pub elements: u64,
    /// Source-declared prefetch distance, strictly less than the buffer count.
    pub prefetch_distance: u32,
    /// Packed unsigned scalar width, one of 8, 16, 32, 64 or 128 bits.
    pub packed_bits: u16,
    /// Exact source payload size, without rounding or ignored padding.
    pub source_size_bytes: u64,
    /// Exact source payload alignment.
    pub source_alignment_bytes: u64,
}

/// One physical storage definition, shared by every source alias of that allocation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KernelIrPipelineStorageBindingV1 {
    /// Ordinal in the actual canonical graph's function roster.
    pub function: u32,
    /// Actual storage result ValueId; it never sizes an allocation.
    pub storage: u32,
    /// Dense contract definition key.
    pub key: u32,
    /// Ordinal in the actual function's block roster, not a source block or BlockId.
    pub block: u32,
    /// Ordinal in the actual block's operation roster.
    pub operation: u32,
}

/// Canonical graph-independent catalog framing, with no source or proof authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalKernelIrContractCatalogV1 {
    canonical: Vec<u8>,
    digest: [u8; 32],
    semantic_source: [u8; 32],
    definitions: Vec<KernelIrPipelineContractDefinitionV1>,
    bindings: Vec<KernelIrPipelineStorageBindingV1>,
}

/// Logical retained catalog payload transferred to the caller, not allocator/RSS use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrContractCatalogStorageV1(usize);

impl KernelIrContractCatalogStorageV1 {
    /// Reserve this payload before another ledger allocation while the catalog lives.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Framing, geometry, allocation or logical-resource failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrContractCatalogErrorV1 {
    /// Logical resource accounting rejected work or storage before execution.
    Resource(Resource),
    /// A closed framing or catalog geometry rule failed.
    Invalid(&'static str),
    /// Checked count or payload arithmetic overflowed.
    Arithmetic,
    /// Fallible allocation failed after logical admission.
    Allocation,
}

impl From<Resource> for KernelIrContractCatalogErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for KernelIrContractCatalogErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Invalid(rule) => {
                write!(formatter, "native KIR contract catalog rejected: {rule}")
            }
            Self::Arithmetic => {
                formatter.write_str("native KIR contract catalog arithmetic overflow")
            }
            Self::Allocation => {
                formatter.write_str("native KIR contract catalog allocation failed")
            }
        }
    }
}
impl Error for KernelIrContractCatalogErrorV1 {}
type CatalogError = KernelIrContractCatalogErrorV1;

impl InertCanonicalKernelIrContractCatalogV1 {
    /// Frames inert rows in their required canonical order without repairing them.
    ///
    /// Every row visit, byte write/hash, and allocation is admitted first. All
    /// exits restore the incoming floor after drops or explicit payload transfer;
    /// accepted work, peak, and first failure are not rewound. Caller input rows
    /// are borrowed/excluded. This admits framing, not source or graph correctness.
    pub fn from_rows_with_budget(
        semantic_source: [u8; 32],
        definitions: &[KernelIrPipelineContractDefinitionV1],
        bindings: &[KernelIrPipelineStorageBindingV1],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, KernelIrContractCatalogStorageV1), CatalogError> {
        let floor = budget.storage();
        let result = Self::build(semantic_source, definitions, bindings, budget);
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(CatalogError::Arithmetic)?;
        budget.release_storage(retained)?;
        result.map(|catalog| (catalog, KernelIrContractCatalogStorageV1(retained)))
    }

    fn build(
        semantic_source: [u8; 32],
        definitions: &[KernelIrPipelineContractDefinitionV1],
        bindings: &[KernelIrPipelineStorageBindingV1],
        budget: &mut Budget<'_>,
    ) -> Result<Self, CatalogError> {
        let length = wire_length(definitions.len(), bindings.len())?;
        validate_rows(semantic_source, definitions, bindings, budget)?;
        let retained = size_of::<Self>()
            .checked_add(length)
            .and_then(|n| {
                n.checked_add(
                    definitions
                        .len()
                        .checked_mul(size_of::<KernelIrPipelineContractDefinitionV1>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    bindings
                        .len()
                        .checked_mul(size_of::<KernelIrPipelineStorageBindingV1>())?,
                )
            })
            .ok_or(CatalogError::Arithmetic)?;
        budget.reserve_storage(retained)?;
        budget.charge_work(
            length
                .checked_mul(2)
                .and_then(|n| n.checked_add(DOMAIN.len() + 8))
                .ok_or(CatalogError::Arithmetic)?,
        )?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(length)
            .map_err(|_| CatalogError::Allocation)?;
        canonical.extend_from_slice(&MAGIC);
        canonical.extend_from_slice(&1_u16.to_le_bytes());
        canonical.extend_from_slice(&1_u16.to_le_bytes());
        push_u32(
            &mut canonical,
            u32::try_from(length).map_err(|_| CatalogError::Arithmetic)?,
        );
        canonical.extend_from_slice(&semantic_source);
        push_u32(
            &mut canonical,
            u32::try_from(definitions.len()).map_err(|_| CatalogError::Arithmetic)?,
        );
        push_u32(
            &mut canonical,
            u32::try_from(bindings.len()).map_err(|_| CatalogError::Arithmetic)?,
        );
        for row in definitions {
            for value in [
                row.key,
                row.semantic_pipeline_type,
                row.semantic_payload_type,
                row.buffers,
            ] {
                push_u32(&mut canonical, value);
            }
            canonical.extend_from_slice(&row.elements.to_le_bytes());
            push_u32(&mut canonical, row.prefetch_distance);
            canonical.extend_from_slice(&row.packed_bits.to_le_bytes());
            canonical.extend_from_slice(&0_u16.to_le_bytes());
            canonical.extend_from_slice(&row.source_size_bytes.to_le_bytes());
            canonical.extend_from_slice(&row.source_alignment_bytes.to_le_bytes());
        }
        for row in bindings {
            for value in [row.function, row.storage, row.key, row.block, row.operation] {
                push_u32(&mut canonical, value);
            }
        }
        if canonical.len() != length {
            return Err(CatalogError::Arithmetic);
        }
        let mut owned_definitions = Vec::new();
        owned_definitions
            .try_reserve_exact(definitions.len())
            .map_err(|_| CatalogError::Allocation)?;
        let mut owned_bindings = Vec::new();
        owned_bindings
            .try_reserve_exact(bindings.len())
            .map_err(|_| CatalogError::Allocation)?;
        budget.charge_work(
            definitions
                .len()
                .checked_add(bindings.len())
                .ok_or(CatalogError::Arithmetic)?,
        )?;
        owned_definitions.extend_from_slice(definitions);
        owned_bindings.extend_from_slice(bindings);
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update((length as u64).to_le_bytes());
        hash.update(&canonical);
        Ok(Self {
            canonical,
            digest: hash.finalize().into(),
            semantic_source,
            definitions: owned_definitions,
            bindings: owned_bindings,
        })
    }

    /// Strictly decodes native catalog bytes under the same explicit ledger.
    /// Decoding alone does not authenticate the bound semantic source or graph.
    pub fn decode_with_budget(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, KernelIrContractCatalogStorageV1), CatalogError> {
        let floor = budget.storage();
        let result = (|| {
            if bytes.len() < HEADER || bytes.len() > MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1 {
                return Err(CatalogError::Invalid("catalog length"));
            }
            budget.charge_work(bytes.len())?;
            let mut reader = CatalogReader { bytes, offset: 0 };
            if reader.fixed::<8>()? != MAGIC
                || reader.u16()? != 1
                || reader.u16()? != 1
                || reader.u32()? as usize != bytes.len()
            {
                return Err(CatalogError::Invalid("catalog header"));
            }
            let semantic_source = reader.fixed::<32>()?;
            let definition_count = reader.u32()? as usize;
            let binding_count = reader.u32()? as usize;
            if wire_length(definition_count, binding_count)? != bytes.len() {
                return Err(CatalogError::Invalid("catalog row counts"));
            }
            let scratch = size_of::<Vec<KernelIrPipelineContractDefinitionV1>>()
                .checked_add(size_of::<Vec<KernelIrPipelineStorageBindingV1>>())
                .and_then(|n| {
                    n.checked_add(
                        definition_count
                            .checked_mul(size_of::<KernelIrPipelineContractDefinitionV1>())?,
                    )
                })
                .and_then(|n| {
                    n.checked_add(
                        binding_count.checked_mul(size_of::<KernelIrPipelineStorageBindingV1>())?,
                    )
                })
                .ok_or(CatalogError::Arithmetic)?;
            budget.reserve_storage(scratch)?;
            let mut definitions = Vec::new();
            definitions
                .try_reserve_exact(definition_count)
                .map_err(|_| CatalogError::Allocation)?;
            let mut bindings = Vec::new();
            bindings
                .try_reserve_exact(binding_count)
                .map_err(|_| CatalogError::Allocation)?;
            for _ in 0..definition_count {
                let row = KernelIrPipelineContractDefinitionV1 {
                    key: reader.u32()?,
                    semantic_pipeline_type: reader.u32()?,
                    semantic_payload_type: reader.u32()?,
                    buffers: reader.u32()?,
                    elements: reader.u64()?,
                    prefetch_distance: reader.u32()?,
                    packed_bits: reader.u16()?,
                    source_size_bytes: {
                        if reader.u16()? != 0 {
                            return Err(CatalogError::Invalid("definition reserved bytes"));
                        }
                        reader.u64()?
                    },
                    source_alignment_bytes: reader.u64()?,
                };
                definitions.push(row);
            }
            for _ in 0..binding_count {
                bindings.push(KernelIrPipelineStorageBindingV1 {
                    function: reader.u32()?,
                    storage: reader.u32()?,
                    key: reader.u32()?,
                    block: reader.u32()?,
                    operation: reader.u32()?,
                });
            }
            let (catalog, receipt) =
                Self::from_rows_with_budget(semantic_source, &definitions, &bindings, budget)?;
            budget.reserve_storage(receipt.retained_storage())?;
            budget.charge_work(bytes.len())?;
            if catalog.canonical_bytes() != bytes {
                return Err(CatalogError::Invalid("noncanonical catalog"));
            }
            drop(definitions);
            drop(bindings);
            budget.release_storage(scratch)?;
            Ok((catalog, receipt))
        })();
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(CatalogError::Arithmetic)?;
        budget.release_storage(release)?;
        result
    }

    /// Returns the complete canonical catalog bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    /// Returns the domain-separated catalog identity, not the graph identity.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Returns the exact source semantic identity to replay before authentication.
    pub const fn semantic_source(&self) -> &[u8; 32] {
        &self.semantic_source
    }
    /// Returns dense canonical source contract definitions.
    pub fn definitions(&self) -> &[KernelIrPipelineContractDefinitionV1] {
        &self.definitions
    }
    /// Returns canonical function-qualified physical storage bindings.
    pub fn bindings(&self) -> &[KernelIrPipelineStorageBindingV1] {
        &self.bindings
    }
    /// Inert source claims and framing grant no compiler or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn wire_length(definitions: usize, bindings: usize) -> Result<usize, CatalogError> {
    let length = definitions
        .checked_mul(DEFINITION_BYTES)
        .and_then(|n| n.checked_add(bindings.checked_mul(BINDING_BYTES)?))
        .and_then(|n| n.checked_add(HEADER))
        .ok_or(CatalogError::Arithmetic)?;
    if length > MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1 {
        return Err(CatalogError::Invalid("catalog length"));
    }
    Ok(length)
}

fn validate_rows(
    source: [u8; 32],
    definitions: &[KernelIrPipelineContractDefinitionV1],
    bindings: &[KernelIrPipelineStorageBindingV1],
    budget: &mut Budget<'_>,
) -> Result<(), CatalogError> {
    budget.charge_work(1)?;
    if source == [0; 32] {
        return Err(CatalogError::Invalid("zero semantic source"));
    }
    for (index, row) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        if row.key as usize != index
            || (index > 0
                && definitions[index - 1].semantic_pipeline_type >= row.semantic_pipeline_type)
        {
            return Err(CatalogError::Invalid("definition order"));
        }
        if !(2..=8).contains(&row.buffers)
            || row.elements == 0
            || row.prefetch_distance == 0
            || row.prefetch_distance >= row.buffers
            || !matches!(row.packed_bits, 8 | 16 | 32 | 64 | 128)
            || row.source_size_bytes != u64::from(row.packed_bits) / 8
            || !row.source_alignment_bytes.is_power_of_two()
            || row.source_alignment_bytes > u64::from(u32::MAX)
            || row
                .elements
                .checked_mul(u64::from(row.buffers))
                .is_none_or(|extent| extent > u64::from(u32::MAX))
        {
            return Err(CatalogError::Invalid("pipeline geometry or layout"));
        }
    }
    for (index, row) in bindings.iter().enumerate() {
        budget.charge_work(1)?;
        if row.key as usize >= definitions.len()
            || (index > 0
                && (bindings[index - 1].function, bindings[index - 1].storage)
                    >= (row.function, row.storage))
        {
            return Err(CatalogError::Invalid("storage binding order or key"));
        }
    }
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
struct CatalogReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl CatalogReader<'_> {
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CatalogError> {
        let end = self.offset.checked_add(N).ok_or(CatalogError::Arithmetic)?;
        let source = self
            .bytes
            .get(self.offset..end)
            .ok_or(CatalogError::Invalid("truncated catalog"))?;
        let mut output = [0; N];
        output.copy_from_slice(source);
        self.offset = end;
        Ok(output)
    }
    fn u16(&mut self) -> Result<u16, CatalogError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32, CatalogError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn u64(&mut self) -> Result<u64, CatalogError> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalKernelIrWorkBudgetV1;

    fn definition() -> KernelIrPipelineContractDefinitionV1 {
        KernelIrPipelineContractDefinitionV1 {
            key: 0,
            semantic_pipeline_type: 9,
            semantic_payload_type: 3,
            buffers: 2,
            elements: 32,
            prefetch_distance: 1,
            packed_bits: 32,
            source_size_bytes: 4,
            source_alignment_bytes: 4,
        }
    }

    #[test]
    fn canonical_roundtrip_with_actual_binding_and_empty_catalog() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let binding = KernelIrPipelineStorageBindingV1 {
            function: 1,
            storage: 800,
            key: 0,
            block: 2,
            operation: 4,
        };
        for (definitions, bindings) in [(&[][..], &[][..]), (&[definition()][..], &[binding][..])] {
            let (catalog, receipt) =
                InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                    [1; 32],
                    definitions,
                    bindings,
                    &mut budget,
                )
                .unwrap();
            assert_eq!(budget.storage(), 0);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (decoded, _) = InertCanonicalKernelIrContractCatalogV1::decode_with_budget(
                catalog.canonical_bytes(),
                &mut budget,
            )
            .unwrap();
            assert_eq!(decoded, catalog);
            assert!(!decoded.grants_authority());
            drop(decoded);
            drop(catalog);
            budget.release_storage(receipt.retained_storage()).unwrap();
        }
    }

    #[test]
    fn exact_and_one_under_empty_catalog_admission() {
        let exact_work = 1 + 2 * HEADER + DOMAIN.len() + 8;
        let exact_storage = size_of::<InertCanonicalKernelIrContractCatalogV1>() + HEADER;
        for (work_limit, storage_limit, succeeds) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            let result = InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                [1; 32],
                &[],
                &[],
                &mut budget,
            );
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(budget.storage(), 0);
            if let Ok((_, storage)) = result {
                assert_eq!(budget.work(), exact_work);
                assert_eq!(storage.retained_storage(), exact_storage);
            }
        }
    }

    #[test]
    fn empty_decode_has_exact_work_and_coexisting_storage_limits() {
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut setup = Budget::new(&mut setup_work, 100_000);
        let (source, source_storage) =
            InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                [1; 32],
                &[],
                &[],
                &mut setup,
            )
            .unwrap();
        let scratch = size_of::<Vec<KernelIrPipelineContractDefinitionV1>>()
            + size_of::<Vec<KernelIrPipelineStorageBindingV1>>();
        let retained = size_of::<InertCanonicalKernelIrContractCatalogV1>() + HEADER;
        assert_eq!(source_storage.retained_storage(), retained);
        // Input scan, encoding, hashing, and comparison each visit HEADER bytes;
        // the empty source validation and domain/length hashing are additional.
        let exact_work = 4 * HEADER + 1 + DOMAIN.len() + 8;
        let floor = 17 + retained;
        for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - work_under);
            let mut budget = Budget::new(&mut work, floor + scratch + retained - storage_under);
            budget.charge_work(11).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = InertCanonicalKernelIrContractCatalogV1::decode_with_budget(
                source.canonical_bytes(),
                &mut budget,
            );
            assert_eq!(budget.storage(), floor);
            match result {
                Ok((decoded, storage)) => {
                    assert_eq!((work_under, storage_under), (0, 0));
                    assert_eq!(decoded, source);
                    assert_eq!(storage.retained_storage(), retained);
                    assert_eq!(budget.work(), 11 + exact_work);
                    assert_eq!(budget.peak_storage(), floor + scratch + retained);
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    drop(decoded);
                    budget.release_storage(storage.retained_storage()).unwrap();
                }
                Err(CatalogError::Resource(Resource::Work(_))) => {
                    assert_eq!((work_under, storage_under), (1, 0));
                    assert_eq!(budget.work(), 11 + exact_work - HEADER);
                    assert_eq!(budget.peak_storage(), floor + scratch + retained);
                }
                Err(CatalogError::Resource(Resource::Storage(_))) => {
                    assert_eq!((work_under, storage_under), (0, 1));
                    assert_eq!(budget.work(), 11 + HEADER + 1);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                    assert_eq!(budget.failed_storage(), Some(floor + scratch + retained));
                }
                Err(error) => panic!("unexpected catalog rejection: {error}"),
            }
            assert_eq!(budget.storage(), floor);
        }
    }

    #[test]
    fn nonempty_decode_has_exact_work_and_coexisting_storage_limits() {
        let binding = KernelIrPipelineStorageBindingV1 {
            function: 1,
            storage: u32::MAX,
            key: 0,
            block: 2,
            operation: 4,
        };
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut setup = Budget::new(&mut setup_work, 100_000);
        let (source, source_storage) =
            InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                [1; 32],
                &[definition()],
                &[binding],
                &mut setup,
            )
            .unwrap();
        let length = HEADER + DEFINITION_BYTES + BINDING_BYTES;
        let row_storage = size_of::<KernelIrPipelineContractDefinitionV1>()
            + size_of::<KernelIrPipelineStorageBindingV1>();
        let scratch = size_of::<Vec<KernelIrPipelineContractDefinitionV1>>()
            + size_of::<Vec<KernelIrPipelineStorageBindingV1>>()
            + row_storage;
        let retained = size_of::<InertCanonicalKernelIrContractCatalogV1>() + length + row_storage;
        // Decode and comparison each visit L bytes; construction writes and hashes
        // 2L bytes, validates three rows/source, copies two rows, and hashes its
        // domain and length.
        let exact_work = 4 * length + 5 + DOMAIN.len() + 8;
        let floor = 17 + source_storage.retained_storage();
        for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - work_under);
            let mut budget = Budget::new(&mut work, floor + scratch + retained - storage_under);
            budget.charge_work(11).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = InertCanonicalKernelIrContractCatalogV1::decode_with_budget(
                source.canonical_bytes(),
                &mut budget,
            );
            assert_eq!(budget.storage(), floor);
            match result {
                Ok((decoded, storage)) => {
                    assert_eq!((work_under, storage_under), (0, 0));
                    assert_eq!(decoded, source);
                    assert_eq!(storage.retained_storage(), retained);
                    assert_eq!(budget.work(), 11 + exact_work);
                    assert_eq!(budget.peak_storage(), floor + scratch + retained);
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    drop(decoded);
                    budget.release_storage(storage.retained_storage()).unwrap();
                }
                Err(CatalogError::Resource(Resource::Work(_))) => {
                    assert_eq!((work_under, storage_under), (1, 0));
                    assert_eq!(budget.work(), 11 + exact_work - length);
                    assert_eq!(budget.peak_storage(), floor + scratch + retained);
                }
                Err(CatalogError::Resource(Resource::Storage(_))) => {
                    assert_eq!((work_under, storage_under), (0, 1));
                    assert_eq!(budget.work(), 11 + length + 3);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                    assert_eq!(budget.failed_storage(), Some(floor + scratch + retained));
                }
                Err(error) => panic!("unexpected catalog rejection: {error}"),
            }
            assert_eq!(budget.storage(), floor);
        }
    }

    #[test]
    fn nonempty_decode_rejects_reserved_bytes_row_counts_and_truncation() {
        let binding = KernelIrPipelineStorageBindingV1 {
            function: 1,
            storage: 800,
            key: 0,
            block: 2,
            operation: 4,
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let (source, storage) = InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
            [1; 32],
            &[definition()],
            &[binding],
            &mut budget,
        )
        .unwrap();
        let floor = 17 + storage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        for (offset, expected) in [
            (48, "catalog row counts"),
            (52, "catalog row counts"),
            (HEADER + 30, "definition reserved bytes"),
            (
                HEADER + DEFINITION_BYTES + 8,
                "storage binding order or key",
            ),
        ] {
            let mut bytes = source.canonical_bytes().to_vec();
            bytes[offset] ^= 1;
            assert_eq!(
                InertCanonicalKernelIrContractCatalogV1::decode_with_budget(&bytes, &mut budget),
                Err(CatalogError::Invalid(expected)),
            );
            assert_eq!(budget.storage(), floor);
        }
        for end in 0..source.canonical_bytes().len() {
            let mut bytes = source.canonical_bytes()[..end].to_vec();
            if end >= 16 {
                bytes[12..16].copy_from_slice(&(end as u32).to_le_bytes());
            }
            assert!(
                InertCanonicalKernelIrContractCatalogV1::decode_with_budget(&bytes, &mut budget,)
                    .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
    }

    #[test]
    fn rejects_catalog_key_geometry_order_and_all_header_changes() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let original = definition();
        let mut invalid = [original; 6];
        invalid[0].key = 1;
        invalid[1].buffers = 1;
        invalid[2].prefetch_distance = 2;
        invalid[3].source_size_bytes = 8;
        invalid[4].source_alignment_bytes = 3;
        invalid[5].elements = u64::MAX;
        for row in invalid {
            assert!(
                InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                    [1; 32],
                    &[row],
                    &[],
                    &mut budget
                )
                .is_err()
            );
        }
        let (catalog, receipt) = InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
            [1; 32],
            &[original],
            &[],
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        for index in 0..16 {
            let mut bytes = catalog.canonical_bytes().to_vec();
            bytes[index] ^= 1;
            assert!(
                InertCanonicalKernelIrContractCatalogV1::decode_with_budget(&bytes, &mut budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), receipt.retained_storage());
        }
        let duplicate = KernelIrPipelineStorageBindingV1 {
            function: 0,
            storage: 4,
            key: 0,
            block: 0,
            operation: 0,
        };
        assert!(
            InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                [1; 32],
                &[original],
                &[duplicate, duplicate],
                &mut budget
            )
            .is_err()
        );
        drop(catalog);
        budget.release_storage(receipt.retained_storage()).unwrap();
    }
}
