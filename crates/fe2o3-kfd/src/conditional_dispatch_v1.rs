//! Inert D1 invocation premises, checked against private live mappings.
//!
//! These values do not authenticate a theorem, descriptor, executable, or access
//! permission. The generated host must retain those owners. KFD only checks the
//! numerical premises on the exact request before constructing its AQL packet.

use fe2o3_aql::AqlDispatchGeometryV1;
use sha2::{Digest, Sha256};

use crate::Gfx942KfdDispatchPointerFixupV1;
use crate::shared_memory::SharedGttMappedResourceFactsV1;

const MAX_SLICES: usize = 64;
const MAX_READS: usize = 128;
const MAX_KERNARG: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalDispatchDomainV1 {
    GuardedOutput,
    GlobalLaunch,
}

/// Generated logical-field binding. No address is accepted from a caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalDispatchSliceV1 {
    pub generated_field: u16,
    pub pointer_offset: usize,
    pub length_offset: usize,
    pub buffer_index: Option<usize>,
    pub buffer_byte_offset: usize,
    pub length: u64,
    pub element_bytes: u64,
    pub alignment: u32,
}

/// One occurrence, not one deduplicated input parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalDispatchReadV1 {
    pub slice: u16,
    pub access_domain: ConditionalDispatchDomainV1,
    pub address_domain: ConditionalDispatchDomainV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConditionalDispatchErrorV1 {
    ResourceLimit,
    Allocation,
    Binding,
    Geometry,
    OutputExtent,
    InputExtent,
    Layout,
    Arithmetic,
    LogicalSpan,
    Alignment,
    Alias,
    StaleMapping,
}

impl core::fmt::Display for ConditionalDispatchErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "conditional invocation premise: {self:?}")
    }
}
impl std::error::Error for ConditionalDispatchErrorV1 {}

type Result<T> = core::result::Result<T, ConditionalDispatchErrorV1>;

/// Bounded immutable runtime transport; successful construction is not authority.
///
/// Runtime must include `identity()` in its dispatch-contract identity and carry
/// this exact value into `Gfx942KfdDispatchRequestV1::with_conditional_premises_v1`.
/// A descriptor hash by itself must never select this safe-host transition.
#[derive(Debug)]
pub struct ConditionalDispatchPremisesV1 {
    identity: [u8; 32],
    contract_identity: [u8; 32],
    explicit_kernarg_len: usize,
    explicit_kernarg_sha256: [u8; 32],
    grid: [u32; 3],
    workgroup: [u16; 3],
    slices: Vec<ConditionalDispatchSliceV1>,
    output: usize,
    output_address_domain: ConditionalDispatchDomainV1,
    reads: Vec<ConditionalDispatchReadV1>,
}

impl ConditionalDispatchPremisesV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: [u8; 32],
        packing_identity: [u8; 32],
        kernel_id: [u8; 32],
        explicit_kernarg: &[u8],
        geometry: AqlDispatchGeometryV1,
        slices: &[ConditionalDispatchSliceV1],
        output: usize,
        output_address_domain: ConditionalDispatchDomainV1,
        reads: &[ConditionalDispatchReadV1],
    ) -> Result<Self> {
        use ConditionalDispatchErrorV1 as E;
        if slices.is_empty()
            || slices.len() > MAX_SLICES
            || reads.len() > MAX_READS
            || explicit_kernarg.is_empty()
            || explicit_kernarg.len() > MAX_KERNARG
        {
            return Err(E::ResourceLimit);
        }
        let grid = geometry.grid();
        let workgroup = geometry.workgroup();
        if grid[1..] != [1, 1] || workgroup[1..] != [1, 1] {
            return Err(E::Geometry);
        }
        let out = slices.get(output).ok_or(E::Binding)?;
        if out.length > u64::from(grid[0]) {
            return Err(E::OutputExtent);
        }
        for (i, s) in slices.iter().enumerate() {
            if !matches!(s.element_bytes, 1 | 2 | 4 | 8 | 16)
                || !s.alignment.is_power_of_two()
                || !s.element_bytes.is_multiple_of(u64::from(s.alignment))
            {
                return Err(E::Layout);
            }
            let length_offset = s.pointer_offset.checked_add(8).ok_or(E::Arithmetic)?;
            if !s.pointer_offset.is_multiple_of(8)
                || length_offset != s.length_offset
                || read_word(explicit_kernarg, s.pointer_offset)? != 0
                || read_word(explicit_kernarg, s.length_offset)? != s.length
                || (s.buffer_index.is_none() && (s.length != 0 || s.buffer_byte_offset != 0))
            {
                return Err(E::Binding);
            }
            s.length.checked_mul(s.element_bytes).ok_or(E::Arithmetic)?;
            for prior in &slices[..i] {
                if prior.generated_field == s.generated_field
                    || prior.pointer_offset.abs_diff(s.pointer_offset) < 16
                {
                    return Err(E::Binding);
                }
            }
            if i != output && !reads.iter().any(|r| usize::from(r.slice) == i) {
                return Err(E::Binding);
            }
        }
        for r in reads {
            let i = usize::from(r.slice);
            let s = slices.get(i).ok_or(E::Binding)?;
            if i == output
                || (r.access_domain == ConditionalDispatchDomainV1::GlobalLaunch
                    && r.address_domain != ConditionalDispatchDomainV1::GlobalLaunch)
            {
                return Err(E::Binding);
            }
            if s.length < domain_count(r.access_domain, out.length, u64::from(grid[0])) {
                return Err(E::InputExtent);
            }
        }
        let mut result = Self {
            identity: [0; 32],
            contract_identity,
            explicit_kernarg_len: explicit_kernarg.len(),
            explicit_kernarg_sha256: Sha256::digest(explicit_kernarg).into(),
            grid,
            workgroup,
            slices: bounded_copy(slices)?,
            output,
            output_address_domain,
            reads: bounded_copy(reads)?,
        };
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/KFD/CONDITIONAL-D1-INVOCATION/V1\0");
        for d in [
            contract_identity,
            packing_identity,
            kernel_id,
            result.explicit_kernarg_sha256,
        ] {
            hash.update(d);
        }
        put_usize(&mut hash, result.explicit_kernarg_len);
        for n in grid {
            hash.update(n.to_le_bytes());
        }
        for n in workgroup {
            hash.update(n.to_le_bytes());
        }
        put_usize(&mut hash, output);
        hash.update([domain_tag(output_address_domain)]);
        put_usize(&mut hash, slices.len());
        for s in slices {
            hash.update(s.generated_field.to_le_bytes());
            for n in [s.pointer_offset, s.length_offset, s.buffer_byte_offset] {
                put_usize(&mut hash, n);
            }
            hash.update([u8::from(s.buffer_index.is_some())]);
            put_usize(&mut hash, s.buffer_index.unwrap_or(0));
            hash.update(s.length.to_le_bytes());
            hash.update(s.element_bytes.to_le_bytes());
            hash.update(s.alignment.to_le_bytes());
        }
        put_usize(&mut hash, reads.len());
        for r in reads {
            hash.update(r.slice.to_le_bytes());
            hash.update([domain_tag(r.access_domain), domain_tag(r.address_domain)]);
        }
        result.identity = hash.finalize().into();
        Ok(result)
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Inert origin coordinate already included in `identity()`, not admission.
    pub const fn contract_identity(&self) -> &[u8; 32] {
        &self.contract_identity
    }

    /// The request is consumed by the caller; no reusable satisfied token escapes.
    pub(crate) fn check_request(
        &self,
        kernarg: &[u8],
        fixups: &[Gfx942KfdDispatchPointerFixupV1],
        buffer_lengths: impl ExactSizeIterator<Item = usize> + Clone,
        geometry: AqlDispatchGeometryV1,
    ) -> Result<()> {
        use ConditionalDispatchErrorV1 as E;
        if geometry.grid() != self.grid || geometry.workgroup() != self.workgroup {
            return Err(E::Geometry);
        }
        let bytes = kernarg.get(..self.explicit_kernarg_len).ok_or(E::Binding)?;
        if <[u8; 32]>::from(Sha256::digest(bytes)) != self.explicit_kernarg_sha256 {
            return Err(E::Binding);
        }
        if fixups.len()
            != self
                .slices
                .iter()
                .filter(|s| s.buffer_index.is_some())
                .count()
        {
            return Err(E::Binding);
        }
        for s in &self.slices {
            match s.buffer_index {
                Some(index) => {
                    let byte_len = buffer_lengths.clone().nth(index).ok_or(E::Binding)?;
                    let mut matching = fixups
                        .iter()
                        .filter(|f| f.kernarg_offset() == s.pointer_offset);
                    let f = matching.next().ok_or(E::Binding)?;
                    if matching.next().is_some()
                        || f.buffer_index() != index
                        || f.buffer_byte_offset() != s.buffer_byte_offset
                        || f.required_alignment() != u64::from(s.alignment)
                    {
                        return Err(E::Binding);
                    }
                    check_logical_extent(s, byte_len)?;
                }
                None => {
                    if fixups
                        .iter()
                        .any(|f| f.kernarg_offset() == s.pointer_offset)
                    {
                        return Err(E::Binding);
                    }
                }
            }
        }
        // Every retained dispatch buffer must belong to a logical argument.
        for i in 0..buffer_lengths.len() {
            if !self.slices.iter().any(|s| s.buffer_index == Some(i)) {
                return Err(E::Binding);
            }
        }
        Ok(())
    }

    /// Only queue preparation calls this with facts from retained live tokens.
    pub(crate) fn check_live(&self, facts: &[SharedGttMappedResourceFactsV1]) -> Result<()> {
        let out = &self.slices[self.output];
        let output_span = live_span(out, facts)?;
        let g = u64::from(self.grid[0]);
        check_address(
            out,
            output_span.0,
            self.output_address_domain,
            out.length,
            g,
        )?;
        for (i, s) in self.slices.iter().enumerate() {
            if i == self.output {
                continue;
            }
            let input_span = live_span(s, facts)?;
            if overlaps(output_span, input_span) {
                return Err(ConditionalDispatchErrorV1::Alias);
            }
        }
        for r in &self.reads {
            let s = &self.slices[usize::from(r.slice)];
            let span = live_span(s, facts)?;
            check_address(s, span.0, r.address_domain, out.length, g)?;
        }
        Ok(())
    }
}

fn bounded_copy<T: Copy>(values: &[T]) -> Result<Vec<T>> {
    let mut out = Vec::new();
    out.try_reserve_exact(values.len())
        .map_err(|_| ConditionalDispatchErrorV1::Allocation)?;
    out.extend_from_slice(values);
    Ok(out)
}
fn domain_tag(d: ConditionalDispatchDomainV1) -> u8 {
    match d {
        ConditionalDispatchDomainV1::GuardedOutput => 0,
        ConditionalDispatchDomainV1::GlobalLaunch => 1,
    }
}
fn domain_count(d: ConditionalDispatchDomainV1, n: u64, g: u64) -> u64 {
    match d {
        ConditionalDispatchDomainV1::GuardedOutput => n,
        ConditionalDispatchDomainV1::GlobalLaunch => g,
    }
}
fn put_usize(hash: &mut Sha256, n: usize) {
    hash.update((n as u64).to_le_bytes());
}
fn read_word(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(ConditionalDispatchErrorV1::Arithmetic)?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(ConditionalDispatchErrorV1::Binding)?
            .try_into()
            .map_err(|_| ConditionalDispatchErrorV1::Binding)?,
    ))
}
fn check_logical_extent(s: &ConditionalDispatchSliceV1, available: usize) -> Result<u64> {
    let bytes = s
        .length
        .checked_mul(s.element_bytes)
        .ok_or(ConditionalDispatchErrorV1::Arithmetic)?;
    let end = (s.buffer_byte_offset as u64)
        .checked_add(bytes)
        .ok_or(ConditionalDispatchErrorV1::Arithmetic)?;
    if end > available as u64 {
        return Err(ConditionalDispatchErrorV1::LogicalSpan);
    }
    Ok(bytes)
}
fn live_span(
    s: &ConditionalDispatchSliceV1,
    facts: &[SharedGttMappedResourceFactsV1],
) -> Result<(u64, u64)> {
    use ConditionalDispatchErrorV1 as E;
    let Some(index) = s.buffer_index else {
        return Ok((0, 0));
    };
    let f = facts.get(index).ok_or(E::Binding)?;
    let bytes = check_logical_extent(s, f.logical_bytes())?;
    let offset = s.buffer_byte_offset as u64;
    let end = offset.checked_add(bytes).ok_or(E::Arithmetic)?;
    if end > f.cpu_mapping_bytes() as u64 || end > f.gpu_va_bytes() {
        return Err(E::LogicalSpan);
    }
    let base = f.gpu_va().checked_add(offset).ok_or(E::Arithmetic)?;
    if !base.is_multiple_of(u64::from(s.alignment)) {
        return Err(E::Alignment);
    }
    base.checked_add(bytes).ok_or(E::Arithmetic)?;
    Ok((base, bytes))
}
fn overlaps((a, an): (u64, u64), (b, bn): (u64, u64)) -> bool {
    // live_span checked the ends, including full logical inputs, not read prefixes.
    an != 0 && bn != 0 && a < b + bn && b < a + an
}
fn check_address(
    s: &ConditionalDispatchSliceV1,
    base: u64,
    domain: ConditionalDispatchDomainV1,
    n: u64,
    g: u64,
) -> Result<()> {
    use ConditionalDispatchErrorV1 as E;
    if g == 0 {
        return Ok(());
    }
    // N=0<G still forms the selected zero-offset address. Formation need not
    // dereference memory: do not require launch padding for masked tail lanes.
    let last_index = domain_count(domain, n, g).saturating_sub(1);
    let offset = last_index
        .checked_mul(s.element_bytes)
        .ok_or(E::Arithmetic)?;
    let address = base.checked_add(offset).ok_or(E::Arithmetic)?;
    if !address.is_multiple_of(u64::from(s.alignment)) {
        return Err(E::Alignment);
    }
    Ok(())
}

pub(crate) fn require_same_mapping_v1(
    before: &SharedGttMappedResourceFactsV1,
    now: &SharedGttMappedResourceFactsV1,
) -> Result<()> {
    if before.mapping() != now.mapping()
        || before.publication() != now.publication()
        || before.gpu_va() != now.gpu_va()
        || before.logical_bytes() != now.logical_bytes()
        || before.cpu_mapping_bytes() != now.cpu_mapping_bytes()
        || before.gpu_va_bytes() != now.gpu_va_bytes()
    {
        return Err(ConditionalDispatchErrorV1::StaleMapping);
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_dispatch_v1_tests.rs"]
mod tests;
