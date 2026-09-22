//! Reconstructive transport of the existing inert ranked recipe, never a proof owner.

use super::*;
use fe2o3_functional_proof::{FunctionalRefinementImportErrorV2, SafeReferenceKindV2};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{mem::size_of, panic::resume_unwind};

#[path = "ranked_recipe_wire_v1/transport.rs"]
mod transport;
use transport::*;
#[path = "ranked_recipe_wire_v1/contracts.rs"]
mod contracts;
#[path = "ranked_recipe_wire_v1/expressions.rs"]
mod expressions;

#[path = "ranked_recipe_wire_v1/schema.rs"]
mod schema;
#[cfg(test)]
#[path = "ranked_recipe_wire_v1/schema_tests.rs"]
mod schema_tests;
#[cfg(test)]
#[path = "ranked_recipe_wire_v1/wire_tests.rs"]
mod wire_tests;

/// Complete V1 framing; all integers are little-endian, all enum tags explicit.
pub const PRODUCTION_RANKED_RECIPE_MAGIC_V1: [u8; 8] = *b"FE2O3RR\0";
/// Independent aggregate wire bound, in addition to the ranked semantic limits.
pub const MAX_PRODUCTION_RANKED_RECIPE_BYTES_V1: usize = 16 * 1024 * 1024;
/// Decoder-owned logical payload bound; excludes allocator metadata and constructor scratch.
pub const MAX_PRODUCTION_RANKED_RECIPE_STORAGE_V1: usize = 64 * 1024 * 1024;

/// A malformed, noncanonical, invalid or resource-exhausted recipe.
#[derive(Debug)]
pub enum ProductionRankedRecipeWireErrorV1 {
    Resource(Resource),
    Invalid(&'static str),
    UnknownTag { field: &'static str, tag: u8 },
    Kernel(ProductionRankedKernelErrorV1),
    Binding(FunctionalRefinementImportErrorV2),
    TensorEncode(fe2o3_kernel_ir::KernelIrEncodeError),
    TensorDecode(fe2o3_kernel_ir::KernelIrDecodeError),
}

type WireError = ProductionRankedRecipeWireErrorV1;
impl From<Resource> for WireError {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Invalid(field) => write!(f, "invalid ranked recipe {field}"),
            Self::UnknownTag { field, tag } => write!(f, "unknown ranked {field} tag {tag}"),
            Self::Kernel(e) => e.fmt(f),
            Self::Binding(e) => e.fmt(f),
            Self::TensorEncode(e) => e.fmt(f),
            Self::TensorDecode(e) => e.fmt(f),
        }
    }
}
impl Error for WireError {}

/// Resolver failures retain the caller's typed error; no fallback request is fabricated.
#[derive(Debug)]
pub enum ProductionRankedRecipeDecodeErrorV1<E> {
    Wire(WireError),
    Resolver(E),
}
type DecodeError<E> = ProductionRankedRecipeDecodeErrorV1<E>;
impl<E> From<WireError> for DecodeError<E> {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}
impl<E> From<Resource> for DecodeError<E> {
    fn from(value: Resource) -> Self {
        Self::Wire(value.into())
    }
}
impl<E: fmt::Display> fmt::Display for DecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(e) => e.fmt(f),
            Self::Resolver(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for DecodeError<E> {}

/// Exact ordered claim. Coordinates are derived from traversal, not accepted from the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRankedRecipeProofClaimV1 {
    pub ordinal: u32,
    pub block: u32,
    pub operation: u32,
    pub receipt_digest: DigestV1,
    pub binding: FunctionalRefinementBindingV2,
}

/// Restricted shared-work borrow. A resolver cannot replace or release the storage ledger.
pub struct ProductionRankedRecipeResolverWorkV1<'borrow, 'work> {
    budget: &'borrow mut Budget<'work>,
    denied: Option<Resource>,
}
impl ProductionRankedRecipeResolverWorkV1<'_, '_> {
    pub fn charge_work(&mut self, amount: usize) -> Result<(), Resource> {
        let result = self.budget.charge_work(amount);
        if let Err(error) = result {
            self.denied.get_or_insert(error);
        }
        result
    }
}

/// Supplies already-imported identities for exact claims and checks complete consumption.
///
/// Production adapters must import the matching signed receipt under the existing policy.
/// The codec checks the returned digest and full binding independently. Resolution is not
/// a substitute for subsequent independent production proof replay.
pub trait ProductionRankedRecipeProofResolverV1 {
    type Error;
    fn resolve(
        &mut self,
        claim: ProductionRankedRecipeProofClaimV1,
        work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<ProductionReferenceProofV2, Self::Error>;
    fn finish(
        &mut self,
        resolved: u32,
        work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), Self::Error>;
}
use ProductionRankedRecipeProofResolverV1 as Resolver;

/// Additional retained logical bytes transferred to the caller, not launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRankedRecipeStorageV1 {
    retained_storage: usize,
}
impl ProductionRankedRecipeStorageV1 {
    /// Reserve this amount while the returned owner remains live, before further allocation.
    pub const fn retained_storage(self) -> usize {
        self.retained_storage
    }
}

/// Serializes all typed recipe fields, excluding recomputed tree work and live arena identity.
/// The caller retains/account the input. Returned bytes transfer their exact capacity receipt.
pub fn encode_production_ranked_recipe_v1(
    kernel: &ProductionRankedKernelV1,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, ProductionRankedRecipeStorageV1), WireError> {
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut count = Encoder::new(Mode::Count, budget);
        emit_kernel(kernel, &mut count)?;
        let length = count.offset;
        let mut bytes = Vec::new();
        budget.reserve_storage(length)?;
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        if bytes.capacity() != length {
            return Err(Resource::Allocation.into());
        }
        budget.charge_work(length)?;
        bytes.resize(length, 0);
        let mut writer = Encoder::new(Mode::Fill(&mut bytes), budget);
        emit_kernel(kernel, &mut writer)?;
        if writer.offset != length {
            return Err(WireError::Invalid("encoded length"));
        }
        Ok((
            bytes,
            ProductionRankedRecipeStorageV1 {
                retained_storage: length,
            },
        ))
    }));
    let cleanup = budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    );
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(panic) => {
            let _ = cleanup;
            resume_unwind(panic)
        }
    }
}

/// Reconstructs a checked, canonical ranked recipe; it grants no proof or compiler authority.
///
/// Wire, sequence, expression and decoder payload limits are enforced before allocation.
/// Existing checked contract/kernel constructors retain their separate structurally bounded
/// work/temporary-allocation domain. This is not whole-process or whole-constructor metering.
/// The returned receipt measures the normalized owner, not the parsed vectors it replaced.
/// Input/resolver storage remains caller-owned. On success, reserve the transferred receipt
/// before another controlled allocation; errors/unwinds drop decoder owners and restore the
/// incoming storage floor without refunding work or forgetting prior denial history.
pub fn decode_production_ranked_recipe_v1<R: Resolver>(
    bytes: &[u8],
    resolver: &mut R,
    budget: &mut Budget<'_>,
) -> Result<(ProductionRankedKernelV1, ProductionRankedRecipeStorageV1), DecodeError<R::Error>> {
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(1)?;
        if bytes.len() > MAX_PRODUCTION_RANKED_RECIPE_BYTES_V1 {
            return Err(WireError::Invalid("wire extent").into());
        }
        let mut input = Decoder::new(bytes, resolver, budget);
        input.reserve(size_of::<ProductionRankedKernelV1>())?;
        if input.take(8)? != PRODUCTION_RANKED_RECIPE_MAGIC_V1
            || u16::read(&mut input)? != 1
            || u16::read(&mut input)? != 0
        {
            return Err(WireError::Invalid("header").into());
        }
        let name = String::read(&mut input)?;
        let arguments = input.count(HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
        let block_count = input.count(MAX_RANKED_BOUNDS_BLOCKS)?;
        if block_count == 0 {
            return Err(WireError::Invalid("empty blocks").into());
        }
        let mut blocks = input.vector::<ProductionRankedBlockV1>(block_count)?;
        let mut operations = 0usize;
        let mut census = RecipeCensus::default();
        for block in 0..block_count {
            input.block = block as u32;
            let index_arguments = input.count(if block == 0 {
                0
            } else {
                HARD_MAX_PRODUCTION_RANKED_ARGUMENTS
            })? as u32;
            let count = input.count(MAX_RANKED_BOUNDS_OPERATIONS - operations)?;
            operations += count;
            let mut values = input.vector::<ProductionRankedOperationV1>(count)?;
            for operation in 0..count {
                input.operation = operation as u32;
                let value = ProductionRankedOperationV1::read(&mut input)?;
                census.operation(&value, input.nodes, block_count)?;
                values.push(value);
            }
            let terminator = ProductionRankedTerminatorV1::read(&mut input)?;
            census.add(
                ranked_terminator_materialization_v1(&terminator),
                block_count,
            )?;
            blocks.push(ProductionRankedBlockV1::with_index_arguments(
                index_arguments,
                values,
                terminator,
            ));
        }
        if input.offset != bytes.len() {
            return Err(WireError::Invalid("trailing bytes").into());
        }
        input.finish()?;
        drop(input);
        let kernel =
            ProductionRankedKernelV1::new(&name, arguments, blocks).map_err(WireError::Kernel)?;
        drop(name);
        // Keep the parsed owner's reservation until normalized capacities are reconciled.
        let coverage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        let mut comparison = Encoder::new(Mode::Compare(bytes), budget);
        comparison.coverage = Some(coverage);
        emit_kernel(&kernel, &mut comparison)?;
        if comparison.offset != bytes.len() || !comparison.matches {
            return Err(WireError::Invalid("noncanonical normalized recipe").into());
        }
        let retained_storage = comparison.heap;
        Ok((kernel, ProductionRankedRecipeStorageV1 { retained_storage }))
    }));
    let cleanup = budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    );
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(panic) => {
            let _ = cleanup;
            resume_unwind(panic)
        }
    }
}

#[derive(Default)]
struct RecipeCensus {
    materialized: usize,
    tensor_sites: usize,
    tensor_claims: usize,
}
impl RecipeCensus {
    fn add(&mut self, count: usize, blocks: usize) -> Result<(), WireError> {
        self.materialized = self
            .materialized
            .checked_add(count)
            .ok_or(Resource::Arithmetic)?;
        if self.materialized > MAX_RANKED_BOUNDS_OPERATIONS
            || ranked_tree_work(blocks, self.materialized).ok_or(Resource::Arithmetic)?
                > HARD_MAX_SESSION_OPERATION_TREE_ITEMS
        {
            return Err(WireError::Invalid("materialized operation limit"));
        }
        Ok(())
    }

    fn operation(
        &mut self,
        op: &ProductionRankedOperationV1,
        nodes: usize,
        blocks: usize,
    ) -> Result<(), WireError> {
        self.add(
            ranked_operation_materialization_v1(op, |_| Some(nodes)).ok_or(Resource::Arithmetic)?,
            blocks,
        )?;
        match op {
            ProductionRankedOperationV1::TensorLayout { .. } => self.tensor_sites += 1,
            ProductionRankedOperationV1::RequireTensorRefinement { .. }
            | ProductionRankedOperationV1::RequestTensorRefinement { .. } => {
                self.tensor_claims += 1
            }
            _ => {}
        }
        validate_tensor_refinement_resource_counts_v1(self.tensor_sites, self.tensor_claims)
            .map_err(WireError::Kernel)
    }
}

fn emit_kernel(
    kernel: &ProductionRankedKernelV1,
    out: &mut Encoder<'_, '_, '_>,
) -> Result<(), WireError> {
    out.retain(size_of::<ProductionRankedKernelV1>())?;
    out.bytes(&PRODUCTION_RANKED_RECIPE_MAGIC_V1)?;
    1u16.emit(out)?;
    0u16.emit(out)?;
    kernel.function_name.emit(out)?;
    out.count(kernel.argument_count, HARD_MAX_PRODUCTION_RANKED_ARGUMENTS)?;
    out.count(kernel.blocks.len(), MAX_RANKED_BOUNDS_BLOCKS)?;
    out.retain_array::<ProductionRankedBlockV1>(kernel.blocks.capacity())?;
    let mut operations = 0;
    for block in &kernel.blocks {
        block.index_argument_count.emit(out)?;
        out.count(
            block.operations.len(),
            MAX_RANKED_BOUNDS_OPERATIONS - operations,
        )?;
        operations += block.operations.len();
        out.retain_array::<ProductionRankedOperationV1>(block.operations.capacity())?;
        for operation in &block.operations {
            operation.emit(out)?;
        }
        block.terminator.emit(out)?;
    }
    Ok(())
}
