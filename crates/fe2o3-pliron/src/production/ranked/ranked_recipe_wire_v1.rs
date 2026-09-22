//! Complete inert ranked requests. Framing, constructor normal form and proof
//! admission are deliberately separate operations.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, TensorLayoutPayloadErrorV12,
};
use std::{error::Error, fmt, mem::size_of};

#[path = "ranked_recipe_wire_contracts_v1.rs"]
mod contracts;
#[path = "ranked_recipe_wire_decode_v1.rs"]
mod decode;
#[path = "ranked_recipe_wire_expression_v1.rs"]
mod expression;
#[path = "ranked_recipe_wire_operations_v1.rs"]
mod operations;
#[path = "ranked_recipe_wire_primitives_v1.rs"]
mod primitives;
use primitives::{Reader, Shape, Writer, scope};

pub const RANKED_RECIPE_MAGIC_V1: [u8; 8] = *b"F2RKR1\0\0";
pub const RANKED_RECIPE_DOMAIN_V1: &[u8] = b"FE2O3/RANKED-RECIPE/V1\0";
pub const MAX_RANKED_RECIPE_BYTES_V1: usize = 4 * 1024 * 1024;
const HEADER: usize = 48;
type Kernel = ProductionRankedKernelV1;
type Block = ProductionRankedBlockV1;
type Op = ProductionRankedOperationV1;
type Term = ProductionRankedTerminatorV1;
type Value = ProductionRankedValueV1;
type Id = ProductionRankedValueIdV1;
type Expr = ProductionSemanticExpressionV2;
type Scalar = ProductionSemanticScalarTypeV2;
type Numerical = ProductionNumericalContractV2;

#[derive(Debug)]
pub enum RankedRecipeWireErrorV1 {
    Resource(Resource),
    Tensor(TensorLayoutPayloadErrorV12),
    Constructor(ProductionRankedKernelErrorV1),
    Subjects(fe2o3_functional_proof::FunctionalRefinementImportErrorV2),
    Length,
    Header,
    Reserved,
    Tag {
        field: &'static str,
        tag: u16,
    },
    Limit {
        field: &'static str,
        actual: usize,
        limit: usize,
    },
    Utf8,
    NonCanonical,
    Panicked,
}
impl fmt::Display for RankedRecipeWireErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert ranked recipe: {self:?}")
    }
}
impl Error for RankedRecipeWireErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Tensor(error) => Some(error),
            Self::Constructor(error) => Some(error),
            Self::Subjects(error) => Some(error),
            Self::Length
            | Self::Header
            | Self::Reserved
            | Self::Tag { .. }
            | Self::Limit { .. }
            | Self::Utf8
            | Self::NonCanonical
            | Self::Panicked => None,
        }
    }
}
impl From<Resource> for RankedRecipeWireErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type E = RankedRecipeWireErrorV1;
type R<T> = Result<T, E>;

/// The complete returned header/payload extent; reserve it before more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankedRecipeStorageV1(usize);
impl RankedRecipeStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owned canonical recipe bytes, not imported proof or source authority.
#[derive(Debug)]
pub struct InertRankedRecipeBytesV1 {
    bytes: Vec<u8>,
    identity: [u8; 32],
}
impl InertRankedRecipeBytesV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

/// Closed, bounded wire syntax only. This does NOT attest constructor normal
/// form, verifier acceptance, imported receipts, source or execution custody.
#[derive(Debug)]
pub struct RankedRecipeRefV1<'wire> {
    bytes: &'wire [u8],
    shape: Shape,
    identity: [u8; 32],
}
impl<'wire> RankedRecipeRefV1<'wire> {
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.bytes
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn block_count(&self) -> usize {
        self.shape.blocks
    }
    pub const fn operation_count(&self) -> usize {
        self.shape.operations
    }
}

/// Canonical constructor-normal-form inert recipe, not a compiled lowering.
/// Its underlying recipe remains Clone; copying that inert input outside this
/// controlled API cannot create any signature or source/proof authority.
///
/// ```compile_fail
/// use fe2o3_pliron::DecodedRankedRecipeV1;
/// fn duplicate(recipe: DecodedRankedRecipeV1) { let _ = recipe.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::DecodedRankedRecipeV1;
/// fn mutate(recipe: &mut DecodedRankedRecipeV1) { let _ = recipe.kernel_mut(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{DecodedRankedRecipeV1, ProductionRankedKernelLoweringInputV1};
/// fn promote(recipe: DecodedRankedRecipeV1) -> ProductionRankedKernelLoweringInputV1 { recipe.into() }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::DecodedRankedRecipeV1;
/// use fe2o3_functional_proof::ImportedFunctionalRefinementProofV2;
/// fn promote(recipe: DecodedRankedRecipeV1) -> ImportedFunctionalRefinementProofV2 { recipe.into() }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{DecodedRankedRecipeV1, ProductionRankedKernelV1};
/// fn forge(kernel: ProductionRankedKernelV1) -> DecodedRankedRecipeV1 {
///     DecodedRankedRecipeV1 { kernel, identity: [0; 32] }
/// }
/// ```
#[derive(Debug)]
pub struct DecodedRankedRecipeV1 {
    kernel: Kernel,
    identity: [u8; 32],
}
impl DecodedRankedRecipeV1 {
    pub const fn kernel(&self) -> &Kernel {
        &self.kernel
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

/// Input owners/siblings are prepaid. All new returned backing/header storage
/// transfers unreserved; these bytes do not carry proof admission.
pub fn encode_ranked_recipe_v1(
    kernel: &Kernel,
    budget: &mut Budget<'_>,
) -> R<(InertRankedRecipeBytesV1, RankedRecipeStorageV1)> {
    scope(budget, |budget| {
        let header = size_of::<InertRankedRecipeBytesV1>() + size_of::<RankedRecipeStorageV1>();
        budget.reserve_storage(header + size_of::<Writer<'_, '_>>())?;
        let (body_length, shape) = {
            let mut writer = Writer::counter(budget);
            operations::body(&mut writer, kernel)?;
            writer.shape.finish()?;
            (writer.length, writer.shape)
        };
        let length = HEADER
            .checked_add(body_length)
            .ok_or(Resource::Arithmetic)?;
        primitives::limit("recipe bytes", length, MAX_RANKED_RECIPE_BYTES_V1)?;
        let mut writer = Writer::owned(length, budget)?;
        writer.header(kernel, shape, length)?;
        operations::body(&mut writer, kernel)?;
        if writer.length != length || writer.shape != shape {
            return Err(E::NonCanonical);
        }
        let bytes = writer.into_bytes()?;
        let identity = primitives::identity(&bytes, budget)?;
        let retained = header
            .checked_add(bytes.capacity())
            .ok_or(Resource::Arithmetic)?;
        Ok((InertRankedRecipeBytesV1 { bytes, identity }, retained))
    })
}

/// Pays a complete syntax scan; no typed graph or constructor is materialized.
/// Borrowed bytes stay caller-owned and prepaid for the returned view's life.
pub fn read_ranked_recipe_v1<'wire>(
    bytes: &'wire [u8],
    budget: &mut Budget<'_>,
) -> R<(RankedRecipeRefV1<'wire>, RankedRecipeStorageV1)> {
    scope(budget, |budget| {
        let retained = size_of::<RankedRecipeRefV1<'_>>() + size_of::<RankedRecipeStorageV1>();
        budget.reserve_storage(
            retained
                + size_of::<Reader<'_, '_, '_>>()
                + size_of::<Op>()
                + size_of::<decode::Parsed<'_>>(),
        )?;
        let shape = decode::scan(bytes, budget)?;
        let identity = primitives::identity(bytes, budget)?;
        Ok((
            RankedRecipeRefV1 {
                bytes,
                shape,
                identity,
            },
            retained,
        ))
    })
}

/// Independently decodes the full typed request, runs the UNCHANGED constructor
/// and rejects any preverification normalization by complete byte comparison.
/// Legacy constructor/transform internals remain their bounded child domain;
/// this receipt accounts new codec ownership, not total compiler heap telemetry.
pub fn materialize_ranked_recipe_v1(
    view: &RankedRecipeRefV1<'_>,
    budget: &mut Budget<'_>,
) -> R<(DecodedRankedRecipeV1, RankedRecipeStorageV1)> {
    scope(budget, |budget| {
        let header = size_of::<DecodedRankedRecipeV1>() + size_of::<RankedRecipeStorageV1>();
        budget.reserve_storage(
            header
                + size_of::<Reader<'_, '_, '_>>()
                + size_of::<Op>()
                + size_of::<decode::Parsed<'_>>(),
        )?;
        let shape = decode::scan(view.bytes, budget)?;
        if shape != view.shape {
            return Err(E::NonCanonical);
        }
        let (kernel, kernel_storage) = decode::materialize(view.bytes, budget)?;
        let retained = kernel_storage
            .checked_add(header)
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(size_of::<Writer<'_, '_>>())?;
        let mut count = Writer::counter(budget);
        operations::body(&mut count, &kernel)?;
        let length = HEADER
            .checked_add(count.length)
            .ok_or(Resource::Arithmetic)?;
        let output_shape = count.shape;
        drop(count);
        let mut comparison = Writer::comparing(view.bytes, budget);
        comparison.header(&kernel, output_shape, length)?;
        operations::body(&mut comparison, &kernel)?;
        if !comparison.is_equal() {
            return Err(E::NonCanonical);
        }
        drop(comparison);
        let identity = primitives::identity(view.bytes, budget)?;
        if identity != view.identity {
            return Err(E::NonCanonical);
        }
        Ok((DecodedRankedRecipeV1 { kernel, identity }, retained))
    })
}

#[cfg(test)]
#[path = "ranked_recipe_wire_replay_v1_tests.rs"]
mod replay_tests;
#[cfg(test)]
#[path = "ranked_recipe_wire_resource_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "ranked_recipe_wire_v1_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "ranked_recipe_wire_variant_v1_tests.rs"]
mod variant_tests;
