//! Inert mixed-access invocation data checked on the existing live KFD path.
//! Source/formation-domain provenance and executable authority remain with the
//! authenticated generated host; none can be supplied by this public payload.

use fe2o3_aql::AqlDispatchGeometryV1;
use sha2::{Digest, Sha256};

use crate::Gfx942KfdDispatchPointerFixupV1;
use crate::conditional_dispatch_v1::{
    ConditionalDispatchErrorV1 as Error, ConditionalDispatchSliceV1 as Slice, bounded_copy,
    check_logical_extent, live_span, overlaps, put_usize, read_word, template_pointer,
};
use crate::shared_memory::SharedGttMappedResourceFactsV1;

const MAX_SLICES: usize = 64;
const MAX_ACCESSES: usize = 256;
const MAX_KERNARG: usize = 64 * 1024;
type Result<T> = core::result::Result<T, Error>;

/// A compiler-proved upper envelope, never inferred from a dereference guard.
/// `LogicalExtent` is empty when the named logical slice is empty. A formation
/// executed outside that guard needs its own wider envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedConditionalIndexDomainV26 {
    LogicalExtent { slice: u16 },
    InvocationAxis { axis: u8 },
    UnsignedWidth { bits: u8 },
}

/// One source/current-graph occurrence, not a deduplicated argument count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedConditionalAccessV26 {
    pub slice: u16,
    pub writing: bool,
    /// Bound by the complete authenticated source/formation contract upstream.
    pub occurrence_identity: [u8; 32],
    pub access_domain: MixedConditionalIndexDomainV26,
    pub address_domain: MixedConditionalIndexDomainV26,
    /// Exact global invocation projection used for cross-invocation separation.
    pub invocation_axis: Option<u8>,
}

/// An explicit zero-access source/ABI row, never a synthetic memory occurrence.
/// A consuming compiler/Worker must authenticate this complete row's origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedConditionalUnusedSliceV26 {
    pub slice: u16,
    pub source_argument_identity: [u8; 32],
}

/// Complete bounded transport. Construction authenticates no source theorem,
/// descriptor, current publication, executable, Rust borrow, or access right.
#[derive(Debug)]
pub struct MixedConditionalDispatchPremisesV26 {
    identity: [u8; 32],
    contract_identity: [u8; 32],
    explicit_kernarg_len: usize,
    explicit_kernarg_sha256: [u8; 32],
    grid: [u32; 3],
    workgroup: [u16; 3],
    source_rank: u8,
    index_width: u8,
    slices: Vec<Slice>,
    accesses: Vec<MixedConditionalAccessV26>,
    unused_slices: Vec<MixedConditionalUnusedSliceV26>,
}

impl MixedConditionalDispatchPremisesV26 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: [u8; 32],
        packing_identity: [u8; 32],
        kernel_id: [u8; 32],
        explicit_kernarg: &[u8],
        geometry: AqlDispatchGeometryV1,
        source_rank: u8,
        exact_grid: [u64; 3],
        index_width: u8,
        slices: &[Slice],
        accesses: &[MixedConditionalAccessV26],
    ) -> Result<Self> {
        if slices.is_empty() != accesses.is_empty() {
            return Err(Error::ResourceLimit);
        }
        Self::new_with_unused_slices_v26(
            contract_identity,
            packing_identity,
            kernel_id,
            explicit_kernarg,
            geometry,
            source_rank,
            exact_grid,
            index_width,
            slices,
            accesses,
            &[],
        )
    }

    /// Preserves all mapping/fixup rows while separately declaring exact unused
    /// arguments. The historical constructor still requires every slice to have
    /// an access. An accessed argument cannot be mislabeled unused here.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_unused_slices_v26(
        contract_identity: [u8; 32],
        packing_identity: [u8; 32],
        kernel_id: [u8; 32],
        explicit_kernarg: &[u8],
        geometry: AqlDispatchGeometryV1,
        source_rank: u8,
        exact_grid: [u64; 3],
        index_width: u8,
        slices: &[Slice],
        accesses: &[MixedConditionalAccessV26],
        unused_slices: &[MixedConditionalUnusedSliceV26],
    ) -> Result<Self> {
        if slices.len() > MAX_SLICES
            || accesses.len() > MAX_ACCESSES
            || unused_slices.len() > MAX_SLICES
            || (slices.is_empty() && (!accesses.is_empty() || !unused_slices.is_empty()))
            || explicit_kernarg.len() > MAX_KERNARG
        {
            return Err(Error::ResourceLimit);
        }
        let grid = geometry.grid();
        let workgroup = geometry.workgroup();
        if !(1..=3).contains(&source_rank)
            || !matches!(index_width, 32 | 64)
            || grid.map(u64::from) != exact_grid
            || exact_grid[usize::from(source_rank)..]
                .iter()
                .any(|n| *n != 1)
            || geometry.dimensions() > u16::from(source_rank)
        {
            return Err(Error::Geometry);
        }
        for (i, slice) in slices.iter().enumerate() {
            if !matches!(slice.element_bytes, 1 | 2 | 4 | 8 | 16)
                || !slice.alignment.is_power_of_two()
                || !slice
                    .element_bytes
                    .is_multiple_of(u64::from(slice.alignment))
            {
                return Err(Error::Layout);
            }
            if !slice.pointer_offset.is_multiple_of(8)
                || slice.pointer_offset.checked_add(8) != Some(slice.length_offset)
                || read_word(explicit_kernarg, slice.pointer_offset)? != template_pointer(slice)
                || read_word(explicit_kernarg, slice.length_offset)? != slice.length
                || (slice.buffer_index.is_none()
                    && (slice.length != 0 || slice.buffer_byte_offset != 0))
            {
                return Err(Error::Binding);
            }
            slice
                .length
                .checked_mul(slice.element_bytes)
                .ok_or(Error::Arithmetic)?;
            if index_width == 32 && slice.length > u64::from(u32::MAX) {
                return Err(Error::Arithmetic);
            }
            for prior in &slices[..i] {
                if prior.generated_field == slice.generated_field
                    || prior.pointer_offset.abs_diff(slice.pointer_offset) < 16
                {
                    return Err(Error::Binding);
                }
            }
            let used = accesses.iter().any(|access| usize::from(access.slice) == i);
            let unused = unused_slices.iter().any(|row| usize::from(row.slice) == i);
            if used == unused {
                return Err(Error::Binding);
            }
        }
        for (i, row) in unused_slices.iter().enumerate() {
            if usize::from(row.slice) >= slices.len()
                || unused_slices[..i].iter().any(|prior| {
                    prior.slice == row.slice
                        || prior.source_argument_identity == row.source_argument_identity
                })
            {
                return Err(Error::Binding);
            }
        }
        for (i, access) in accesses.iter().enumerate() {
            let slice = slices
                .get(usize::from(access.slice))
                .ok_or(Error::Binding)?;
            let last = maximum_index(access.access_domain, slices, grid, source_rank, index_width)?;
            if last.is_some_and(|last| last >= slice.length) {
                return Err(if access.writing {
                    Error::OutputExtent
                } else {
                    Error::InputExtent
                });
            }
            maximum_index(
                access.address_domain,
                slices,
                grid,
                source_rank,
                index_width,
            )?;
            if let Some(axis) = access.invocation_axis {
                if axis >= source_rank {
                    return Err(Error::Geometry);
                }
            }
            if access.writing {
                let axis = access.invocation_axis.ok_or(Error::Binding)?;
                if grid
                    .iter()
                    .enumerate()
                    .any(|(i, n)| i != usize::from(axis) && *n != 1)
                {
                    return Err(Error::Geometry);
                }
                // One writable argument uses one injective projection. This
                // also preserves read/write conflict requirements for that arg.
                if accesses
                    .iter()
                    .any(|other| other.slice == access.slice && other.invocation_axis != Some(axis))
                {
                    return Err(Error::Binding);
                }
            }
            if accesses[..i]
                .iter()
                .any(|prior| prior.occurrence_identity == access.occurrence_identity)
            {
                return Err(Error::Binding);
            }
        }
        let explicit_kernarg_sha256 = Sha256::digest(explicit_kernarg).into();
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/KFD/CONDITIONAL-MIXED-INVOCATION/V26\0");
        for identity in [
            contract_identity,
            packing_identity,
            kernel_id,
            explicit_kernarg_sha256,
        ] {
            hash.update(identity);
        }
        put_usize(&mut hash, explicit_kernarg.len());
        hash.update([source_rank, index_width]);
        for n in grid {
            hash.update(n.to_le_bytes());
        }
        for n in workgroup {
            hash.update(n.to_le_bytes());
        }
        put_usize(&mut hash, slices.len());
        for slice in slices {
            hash.update(slice.generated_field.to_le_bytes());
            for n in [
                slice.pointer_offset,
                slice.length_offset,
                slice.buffer_byte_offset,
            ] {
                put_usize(&mut hash, n);
            }
            hash.update([u8::from(slice.buffer_index.is_some())]);
            put_usize(&mut hash, slice.buffer_index.unwrap_or(0));
            hash.update(slice.length.to_le_bytes());
            hash.update(slice.element_bytes.to_le_bytes());
            hash.update(slice.alignment.to_le_bytes());
        }
        put_usize(&mut hash, accesses.len());
        for access in accesses {
            hash.update(access.slice.to_le_bytes());
            hash.update([u8::from(access.writing)]);
            hash.update(access.occurrence_identity);
            hash_domain(&mut hash, access.access_domain);
            hash_domain(&mut hash, access.address_domain);
            hash.update([access.invocation_axis.map_or(0, |axis| axis + 1)]);
        }
        // Keep the previous V26 identity unchanged when there are no unused
        // arguments; an explicit extra roster has its own unambiguous suffix.
        if !unused_slices.is_empty() {
            hash.update(b"FE2O3/KFD/CONDITIONAL-MIXED-UNUSED-SLICES/V26\0");
            put_usize(&mut hash, unused_slices.len());
            for row in unused_slices {
                hash.update(row.slice.to_le_bytes());
                hash.update(row.source_argument_identity);
            }
        }
        Ok(Self {
            identity: hash.finalize().into(),
            contract_identity,
            explicit_kernarg_len: explicit_kernarg.len(),
            explicit_kernarg_sha256,
            grid,
            workgroup,
            source_rank,
            index_width,
            slices: bounded_copy(slices)?,
            accesses: bounded_copy(accesses)?,
            unused_slices: bounded_copy(unused_slices)?,
        })
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn contract_identity(&self) -> &[u8; 32] {
        &self.contract_identity
    }
    pub fn unused_slices(&self) -> &[MixedConditionalUnusedSliceV26] {
        &self.unused_slices
    }
    pub fn slices(&self) -> &[Slice] {
        &self.slices
    }
    pub fn accesses(&self) -> &[MixedConditionalAccessV26] {
        &self.accesses
    }

    pub(crate) fn check_request(
        &self,
        kernarg: &[u8],
        fixups: &[Gfx942KfdDispatchPointerFixupV1],
        buffer_lengths: impl ExactSizeIterator<Item = usize> + Clone,
        geometry: AqlDispatchGeometryV1,
    ) -> Result<()> {
        if geometry.grid() != self.grid || geometry.workgroup() != self.workgroup {
            return Err(Error::Geometry);
        }
        let bytes = kernarg
            .get(..self.explicit_kernarg_len)
            .ok_or(Error::Binding)?;
        if <[u8; 32]>::from(Sha256::digest(bytes)) != self.explicit_kernarg_sha256
            || fixups.len()
                != self
                    .slices
                    .iter()
                    .filter(|slice| slice.buffer_index.is_some())
                    .count()
        {
            return Err(Error::Binding);
        }
        for slice in &self.slices {
            match slice.buffer_index {
                Some(index) => {
                    let available = buffer_lengths.clone().nth(index).ok_or(Error::Binding)?;
                    let mut matched = fixups
                        .iter()
                        .filter(|fixup| fixup.kernarg_offset() == slice.pointer_offset);
                    let fixup = matched.next().ok_or(Error::Binding)?;
                    if matched.next().is_some()
                        || fixup.buffer_index() != index
                        || fixup.buffer_byte_offset() != slice.buffer_byte_offset
                        || fixup.required_alignment() != u64::from(slice.alignment)
                    {
                        return Err(Error::Binding);
                    }
                    check_logical_extent(slice, available)?;
                }
                None if fixups
                    .iter()
                    .any(|fixup| fixup.kernarg_offset() == slice.pointer_offset) =>
                {
                    return Err(Error::Binding);
                }
                None => (),
            }
        }
        if (0..buffer_lengths.len()).any(|index| {
            !self
                .slices
                .iter()
                .any(|slice| slice.buffer_index == Some(index))
        }) {
            return Err(Error::Binding);
        }
        Ok(())
    }

    pub(crate) fn check_live(&self, facts: &[SharedGttMappedResourceFactsV1]) -> Result<()> {
        // Full logical spans, not just accessed prefixes. Multiple accesses of
        // one argument are one binding; different writable arguments may not alias.
        for (i, slice) in self.slices.iter().enumerate() {
            let span = live_span(slice, facts)?;
            if self
                .accesses
                .iter()
                .any(|access| usize::from(access.slice) == i && access.writing)
            {
                for (j, other) in self.slices.iter().enumerate() {
                    if i != j && overlaps(span, live_span(other, facts)?) {
                        return Err(Error::Alias);
                    }
                }
            }
        }
        for access in &self.accesses {
            let slice = &self.slices[usize::from(access.slice)];
            let (base, _) = live_span(slice, facts)?;
            if let Some(index) = maximum_index(
                access.address_domain,
                &self.slices,
                self.grid,
                self.source_rank,
                self.index_width,
            )? {
                let offset = index
                    .checked_mul(slice.element_bytes)
                    .ok_or(Error::Arithmetic)?;
                let address = base.checked_add(offset).ok_or(Error::Arithmetic)?;
                if !address.is_multiple_of(u64::from(slice.alignment)) {
                    return Err(Error::Alignment);
                }
            }
        }
        Ok(())
    }
}

fn maximum_index(
    domain: MixedConditionalIndexDomainV26,
    slices: &[Slice],
    grid: [u32; 3],
    rank: u8,
    width: u8,
) -> Result<Option<u64>> {
    Ok(match domain {
        MixedConditionalIndexDomainV26::LogicalExtent { slice } => slices
            .get(usize::from(slice))
            .ok_or(Error::Binding)?
            .length
            .checked_sub(1),
        MixedConditionalIndexDomainV26::InvocationAxis { axis } if axis < rank => {
            u64::from(grid[usize::from(axis)]).checked_sub(1)
        }
        MixedConditionalIndexDomainV26::UnsignedWidth { bits }
            if (1..=64).contains(&bits) && bits <= width =>
        {
            Some(u64::MAX >> (64 - bits))
        }
        _ => return Err(Error::Binding),
    })
}

fn hash_domain(hash: &mut Sha256, domain: MixedConditionalIndexDomainV26) {
    match domain {
        MixedConditionalIndexDomainV26::LogicalExtent { slice } => {
            hash.update([0]);
            hash.update(slice.to_le_bytes());
        }
        MixedConditionalIndexDomainV26::InvocationAxis { axis } => hash.update([1, axis]),
        MixedConditionalIndexDomainV26::UnsignedWidth { bits } => hash.update([2, bits]),
    }
}

#[cfg(test)]
#[path = "mixed_conditional_dispatch_v26_tests.rs"]
mod tests;
