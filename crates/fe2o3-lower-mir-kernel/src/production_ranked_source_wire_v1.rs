//! Lossless transport of the lowerer's inert, ordered correspondence inputs.
//!
//! This is a new frame, not an extension of any historical coordinate digest.
//! Decode does not establish that coordinates exist, that an extent proposal is
//! true, or that an effect recipe agrees with source. Existing replay owns those
//! checks. Rows are never sorted, deduplicated, repaired, or inferred.

use crate::{
    MAX_SOURCE_OPERATIONS, ProductionRankedAccessSourceV1 as Access,
    ProductionRankedExecutableEffectOriginV1 as Origin,
    ProductionRankedExecutableEffectSourceV1 as Effect,
    ProductionRankedOutputExtentSourceV1 as Extent,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_pliron::{
    MAX_PRODUCTION_RANKED_VALUE_BYTES_V1, ProductionRankedRecipeWireErrorV1,
    ProductionRankedValueV1 as Value, decode_production_ranked_value_prefix_v1,
    encode_production_ranked_value_v1,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

/// V1 frame: magic, u16 version, zero u16 flags, then two u32-counted sequences.
/// Integers are little-endian. Options use tags 0/1; effect origin uses tag 1.
pub const PRODUCTION_RANKED_SOURCE_ROWS_MAGIC_V1: [u8; 8] = *b"FE2O3RS\0";
/// Independent bound on the aggregate access and effect wire payload.
pub const MAX_PRODUCTION_RANKED_SOURCE_ROWS_BYTES_V1: usize = 64 * 1024 * 1024;
/// Bound on decoder-owned logical storage, excluding allocator metadata.
pub const MAX_PRODUCTION_RANKED_SOURCE_ROWS_STORAGE_V1: usize = 128 * 1024 * 1024;

/// Malformed framing, unsupported tags or exhausted shared resources.
#[derive(Debug)]
pub enum ProductionRankedSourceRowsWireErrorV1 {
    /// Work, storage, arithmetic or allocation refusal.
    Resource(Resource),
    /// Invalid version, extent, count, option, origin or trailing data.
    Invalid(&'static str),
    /// Rejection by the existing ranked value leaf codec.
    Value(ProductionRankedRecipeWireErrorV1),
}
type E = ProductionRankedSourceRowsWireErrorV1;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<ProductionRankedRecipeWireErrorV1> for E {
    fn from(value: ProductionRankedRecipeWireErrorV1) -> Self {
        Self::Value(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Invalid(field) => write!(f, "invalid ranked source rows {field}"),
            Self::Value(e) => e.fmt(f),
        }
    }
}
impl Error for E {}

/// Owned inert rows. Construction by decoding grants no correspondence authority.
#[derive(Debug)]
pub struct ProductionRankedSourceRowsV1 {
    access: Vec<Access>,
    effects: Vec<Effect>,
}
impl ProductionRankedSourceRowsV1 {
    /// Complete access sequence, including explicit optional extent proposals.
    pub fn access_sources(&self) -> &[Access] {
        &self.access
    }
    /// Complete generated-effect sequence in its original occurrence order.
    pub fn executable_effect_sources(&self) -> &[Effect] {
        &self.effects
    }
    /// Transfers the original vectors and capacities without changing their
    /// storage receipt or granting source-correspondence authority.
    pub fn into_parts(self) -> (Vec<Access>, Vec<Effect>) {
        (self.access, self.effects)
    }
}

/// Exact logical bytes transferred to the caller, not a verification receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRankedSourceRowsStorageV1 {
    retained_storage: usize,
}
impl ProductionRankedSourceRowsStorageV1 {
    /// Reserve while the returned bytes/rows remain live, before further allocation.
    pub const fn retained_storage(self) -> usize {
        self.retained_storage
    }
}

fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|amount| budget.release_storage(amount));
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

/// Encodes all supplied rows in order using the owning V1 ranked-value grammar.
/// Caller-owned inputs remain borrowed. The receipt covers output capacity only;
/// the nine-byte value scratch is reserved in both sizing and writing passes.
/// Success, refusal and unwind restore the incoming storage floor, never work.
pub fn encode_production_ranked_source_rows_v1(
    access: &[Access],
    effects: &[Effect],
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, ProductionRankedSourceRowsStorageV1), E> {
    scoped(budget, |budget| {
        budget.charge_work(1)?;
        check_counts(access.len(), effects.len())?;
        budget.reserve_storage(MAX_PRODUCTION_RANKED_VALUE_BYTES_V1)?;
        let mut count = Writer {
            output: None,
            offset: 0,
            budget,
        };
        emit(access, effects, &mut count)?;
        let length = count.offset;
        budget.reserve_storage(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        if bytes.capacity() != length {
            return Err(Resource::Allocation.into());
        }
        budget.charge_work(length)?;
        bytes.resize(length, 0);
        let mut writer = Writer {
            output: Some(&mut bytes),
            offset: 0,
            budget,
        };
        emit(access, effects, &mut writer)?;
        if writer.offset != length {
            return Err(E::Invalid("encoded extent"));
        }
        Ok((
            bytes,
            ProductionRankedSourceRowsStorageV1 {
                retained_storage: length,
            },
        ))
    })
}

/// Reconstructs complete inert rows, including `None` versus `Some` extent facts.
/// Limits and prepaid exact capacities precede allocation. The receipt includes
/// the owner header and both vector payloads. No semantic referent is authenticated.
/// Success/error/unwind restore the incoming storage floor without refunding work.
pub fn decode_production_ranked_source_rows_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    (
        ProductionRankedSourceRowsV1,
        ProductionRankedSourceRowsStorageV1,
    ),
    E,
> {
    scoped(budget, |budget| {
        budget.charge_work(1)?;
        if bytes.len() > MAX_PRODUCTION_RANKED_SOURCE_ROWS_BYTES_V1 {
            return Err(E::Invalid("wire extent"));
        }
        let mut reader = Reader {
            bytes,
            offset: 0,
            reserved: 0,
            budget,
        };
        if reader.take(8)? != PRODUCTION_RANKED_SOURCE_ROWS_MAGIC_V1
            || reader.take(2)? != 1u16.to_le_bytes()
            || reader.take(2)? != [0, 0]
        {
            return Err(E::Invalid("framing"));
        }
        reader.reserve(size_of::<ProductionRankedSourceRowsV1>())?;
        let access_count = reader.count()?;
        reader.require_remaining(access_count, 18, 4)?;
        let mut access = reader.vector(access_count)?;
        for _ in 0..access_count {
            let block = reader.u32()?;
            let statement = if reader.option()? {
                Some(reader.u32()?)
            } else {
                None
            };
            let mut row = Access::new(
                block,
                statement,
                reader.u32()?,
                reader.u32()?,
                reader.u32()?,
            );
            if reader.option()? {
                row = row.with_output_extent(Extent::new(
                    reader.u32()?,
                    reader.value()?,
                    reader.value()?,
                    reader.value()?,
                ));
            }
            access.push(row);
        }
        let effect_count = reader.count()?;
        check_counts(access_count, effect_count)?;
        reader.require_remaining(effect_count, 49, 0)?;
        let mut effects = reader.vector(effect_count)?;
        for _ in 0..effect_count {
            let (block, ordinal, ranked_block, operation) =
                (reader.u32()?, reader.u32()?, reader.u32()?, reader.u32()?);
            let origin = match reader.take(1)? {
                [1] => Origin::GeneratedFromSemanticTerminator,
                _ => return Err(E::Invalid("effect origin")),
            };
            let identity = reader
                .take(32)?
                .try_into()
                .map_err(|_| E::Invalid("recipe identity"))?;
            effects.push(Effect::new(
                block,
                ordinal,
                ranked_block,
                operation,
                origin,
                identity,
            ));
        }
        if reader.offset != bytes.len() {
            return Err(E::Invalid("trailing bytes"));
        }
        Ok((
            ProductionRankedSourceRowsV1 { access, effects },
            ProductionRankedSourceRowsStorageV1 {
                retained_storage: reader.reserved,
            },
        ))
    })
}

fn check_counts(access: usize, effects: usize) -> Result<(), E> {
    if access.checked_add(effects).ok_or(Resource::Arithmetic)? > MAX_SOURCE_OPERATIONS {
        return Err(E::Invalid("row count"));
    }
    Ok(())
}

struct Writer<'out, 'borrow, 'work> {
    output: Option<&'out mut [u8]>,
    offset: usize,
    budget: &'borrow mut Budget<'work>,
}
impl Writer<'_, '_, '_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), E> {
        let end = self
            .offset
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?;
        if end > MAX_PRODUCTION_RANKED_SOURCE_ROWS_BYTES_V1 {
            return Err(E::Invalid("wire extent"));
        }
        self.budget
            .charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        if let Some(output) = &mut self.output {
            output
                .get_mut(self.offset..end)
                .ok_or(E::Invalid("output extent"))?
                .copy_from_slice(bytes);
        }
        self.offset = end;
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), E> {
        self.bytes(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), E> {
        self.u32(u32::try_from(value).map_err(|_| Resource::Arithmetic)?)
    }
    fn option(&mut self, present: bool) -> Result<(), E> {
        self.bytes(&[u8::from(present)])
    }
    fn value(&mut self, value: Value) -> Result<(), E> {
        let mut bytes = [0; MAX_PRODUCTION_RANKED_VALUE_BYTES_V1];
        let length = encode_production_ranked_value_v1(value, &mut bytes, self.budget)?;
        self.bytes(&bytes[..length])
    }
}

fn emit(access: &[Access], effects: &[Effect], out: &mut Writer<'_, '_, '_>) -> Result<(), E> {
    out.bytes(&PRODUCTION_RANKED_SOURCE_ROWS_MAGIC_V1)?;
    out.bytes(&1u16.to_le_bytes())?;
    out.bytes(&[0, 0])?;
    out.count(access.len())?;
    for row in access {
        out.u32(row.semantic_block())?;
        out.option(row.semantic_statement().is_some())?;
        if let Some(statement) = row.semantic_statement() {
            out.u32(statement)?;
        }
        out.u32(row.semantic_access_ordinal())?;
        out.u32(row.ranked_block())?;
        out.u32(row.ranked_operation())?;
        out.option(row.output_extent().is_some())?;
        if let Some(extent) = row.output_extent() {
            out.u32(extent.source_argument())?;
            out.value(extent.view())?;
            out.value(extent.extent())?;
            out.value(extent.index())?;
        }
    }
    out.count(effects.len())?;
    for row in effects {
        out.u32(row.semantic_block())?;
        out.u32(row.semantic_effect_ordinal())?;
        out.u32(row.ranked_block())?;
        out.u32(row.ranked_operation())?;
        out.bytes(&[match row.origin() {
            Origin::GeneratedFromSemanticTerminator => 1,
        }])?;
        out.bytes(&row.recipe_identity())?;
    }
    Ok(())
}

struct Reader<'input, 'borrow, 'work> {
    bytes: &'input [u8],
    offset: usize,
    reserved: usize,
    budget: &'borrow mut Budget<'work>,
}
impl<'input> Reader<'input, '_, '_> {
    fn require_remaining(&self, count: usize, minimum: usize, suffix: usize) -> Result<(), E> {
        let needed = count
            .checked_mul(minimum)
            .and_then(|n| n.checked_add(suffix))
            .ok_or(Resource::Arithmetic)?;
        if needed > self.bytes.len() - self.offset {
            return Err(E::Invalid("row extent"));
        }
        Ok(())
    }
    fn take(&mut self, length: usize) -> Result<&'input [u8], E> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(Resource::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(E::Invalid("truncated bytes"))?;
        self.budget
            .charge_work(length.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        self.offset = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, E> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| E::Invalid("u32"))?,
        ))
    }
    fn count(&mut self) -> Result<usize, E> {
        let count = usize::try_from(self.u32()?).map_err(|_| Resource::Arithmetic)?;
        check_counts(count, 0)?;
        Ok(count)
    }
    fn option(&mut self) -> Result<bool, E> {
        match self.take(1)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(E::Invalid("option tag")),
        }
    }
    fn value(&mut self) -> Result<Value, E> {
        let (value, length) =
            decode_production_ranked_value_prefix_v1(&self.bytes[self.offset..], self.budget)?;
        self.offset = self
            .offset
            .checked_add(length)
            .ok_or(Resource::Arithmetic)?;
        Ok(value)
    }
    fn reserve(&mut self, amount: usize) -> Result<(), E> {
        let next = self
            .reserved
            .checked_add(amount)
            .ok_or(Resource::Arithmetic)?;
        if next > MAX_PRODUCTION_RANKED_SOURCE_ROWS_STORAGE_V1 {
            return Err(E::Invalid("retained payload"));
        }
        self.budget.reserve_storage(amount)?;
        self.reserved = next;
        Ok(())
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, E> {
        self.reserve(
            count
                .checked_mul(size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        if values.capacity() != count {
            return Err(Resource::Allocation.into());
        }
        Ok(values)
    }
}

#[cfg(test)]
#[path = "production_ranked_source_wire_v1_tests.rs"]
mod tests;
