//! Checked application of optimizer operation lineage to compiler source maps.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapErrorV1,
    DebugSourceMapErrorV2, DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, Module,
    VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13,
};

use crate::{
    CheckedCanonicalTransformationErrorV1, CheckedCanonicalTransformationV13V1,
    LoopMemoryTransformErrorV1, LoopMemoryTransformLimitsV1, LoopMemoryTransformReportV1,
    LoopMemoryTransformV1, OperationCoordinateLineageV1, OperationCoordinateV1,
    ProductionTransformationV1, check_loop_memory_transform_relation_v1,
    execute_checked_canonical_transformation_v13_v1,
};

pub const HARD_MAX_SOURCE_LINEAGE_COORDINATES_V1: usize = 4_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCoordinateLineageLimitsV1 {
    max_coordinates: usize,
}

impl SourceCoordinateLineageLimitsV1 {
    pub fn new(max_coordinates: usize) -> Result<Self, SourceCoordinateLineageErrorV1> {
        if max_coordinates == 0 || max_coordinates > HARD_MAX_SOURCE_LINEAGE_COORDINATES_V1 {
            return Err(SourceCoordinateLineageErrorV1::InvalidLimits);
        }
        Ok(Self { max_coordinates })
    }

    pub const fn max_coordinates(self) -> usize {
        self.max_coordinates
    }
}

impl Default for SourceCoordinateLineageLimitsV1 {
    fn default() -> Self {
        Self {
            max_coordinates: HARD_MAX_SOURCE_LINEAGE_COORDINATES_V1,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CheckedSourceCoordinateLineageV1 {
    document: DebugSourceMapDocumentV2,
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    transform_report: LoopMemoryTransformReportV1,
    mapped_coordinates: usize,
    eliminated_coordinates: usize,
}

impl CheckedSourceCoordinateLineageV1 {
    pub const fn document(&self) -> &DebugSourceMapDocumentV2 {
        &self.document
    }

    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input_identity
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output_identity
    }

    pub const fn transform_report(&self) -> &LoopMemoryTransformReportV1 {
        &self.transform_report
    }

    pub const fn mapped_coordinates(&self) -> usize {
        self.mapped_coordinates
    }

    pub const fn eliminated_coordinates(&self) -> usize {
        self.eliminated_coordinates
    }

    pub const fn grants_semantic_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum SourceCoordinateLineageErrorV1 {
    InvalidLimits,
    Input(VerifiedCanonicalKernelIrErrorV13),
    Output(VerifiedCanonicalKernelIrErrorV13),
    Relation(LoopMemoryTransformErrorV1),
    Binding(DebugSourceMapErrorV1),
    SourceMap(DebugSourceMapErrorV2),
    InputBindingMismatch,
    OutputBindingMismatch,
    InvalidCoordinate,
    IncompleteCoordinateLineage,
    OperationVariableLineageUnsupported,
    ResourceLimit { required: usize, limit: usize },
    AllocationFailure,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CheckedLoopTransformationWithSourceLineageV1 {
    transformation: CheckedCanonicalTransformationV13V1,
    source_lineage: CheckedSourceCoordinateLineageV1,
}

impl CheckedLoopTransformationWithSourceLineageV1 {
    pub const fn transformation(&self) -> &CheckedCanonicalTransformationV13V1 {
        &self.transformation
    }

    pub const fn source_lineage(&self) -> &CheckedSourceCoordinateLineageV1 {
        &self.source_lineage
    }

    pub const fn grants_semantic_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum CheckedLoopTransformationWithSourceLineageErrorV1 {
    NotLoopMemoryTransformation,
    Transformation(CheckedCanonicalTransformationErrorV1),
    SourceLineage(SourceCoordinateLineageErrorV1),
}

impl fmt::Display for CheckedLoopTransformationWithSourceLineageErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLoopMemoryTransformation => {
                formatter.write_str("source-lineage transaction requires a loop/memory transform")
            }
            Self::Transformation(error) => error.fmt(formatter),
            Self::SourceLineage(error) => error.fmt(formatter),
        }
    }
}

impl Error for CheckedLoopTransformationWithSourceLineageErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transformation(error) => Some(error),
            Self::SourceLineage(error) => Some(error),
            Self::NotLoopMemoryTransformation => None,
        }
    }
}

impl fmt::Display for SourceCoordinateLineageErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => {
                formatter.write_str("source-coordinate lineage limits are invalid")
            }
            Self::Input(error) => write!(formatter, "source-lineage input is invalid: {error}"),
            Self::Output(error) => write!(formatter, "source-lineage output is invalid: {error}"),
            Self::Relation(error) => write!(formatter, "source-lineage relation failed: {error}"),
            Self::Binding(error) => error.fmt(formatter),
            Self::SourceMap(error) => error.fmt(formatter),
            Self::InputBindingMismatch => formatter.write_str(
                "source map is not bound to the exact pre-transform canonical V13 graph",
            ),
            Self::OutputBindingMismatch => formatter.write_str(
                "output source-map binding does not name the exact post-transform canonical V13 graph",
            ),
            Self::InvalidCoordinate => {
                formatter.write_str("source map or transform contains an invalid KIR coordinate")
            }
            Self::IncompleteCoordinateLineage => formatter.write_str(
                "transform lineage does not account for every output operation coordinate",
            ),
            Self::OperationVariableLineageUnsupported => formatter.write_str(
                "changed operation-local variable locations require value-lineage replay",
            ),
            Self::ResourceLimit { required, limit } => write!(
                formatter,
                "source-coordinate lineage requires {required} coordinates but the limit is {limit}",
            ),
            Self::AllocationFailure => {
                formatter.write_str("source-coordinate lineage allocation failed")
            }
        }
    }
}

impl Error for SourceCoordinateLineageErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) | Self::Output(error) => Some(error),
            Self::Relation(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::SourceMap(error) => Some(error),
            _ => None,
        }
    }
}

pub fn execute_checked_loop_transformation_with_source_lineage_v1(
    transformation: ProductionTransformationV1,
    input: &Module,
    input_epoch: u64,
    source_map: &DebugSourceMapDocumentV2,
    output_binding: DebugSourceMapBindingV1,
    lineage_limits: SourceCoordinateLineageLimitsV1,
) -> Result<
    CheckedLoopTransformationWithSourceLineageV1,
    CheckedLoopTransformationWithSourceLineageErrorV1,
> {
    let transform = transformation
        .loop_memory_transform()
        .ok_or(CheckedLoopTransformationWithSourceLineageErrorV1::NotLoopMemoryTransformation)?;
    let output =
        execute_checked_canonical_transformation_v13_v1(transformation, input, input_epoch)
            .map_err(CheckedLoopTransformationWithSourceLineageErrorV1::Transformation)?;
    let source_lineage = apply_checked_loop_source_coordinate_lineage_v1(
        transform,
        input,
        output.module(),
        source_map,
        output_binding,
        LoopMemoryTransformLimitsV1::default(),
        lineage_limits,
    )
    .map_err(CheckedLoopTransformationWithSourceLineageErrorV1::SourceLineage)?;
    Ok(CheckedLoopTransformationWithSourceLineageV1 {
        transformation: output,
        source_lineage,
    })
}

pub fn apply_checked_loop_source_coordinate_lineage_v1(
    transform: LoopMemoryTransformV1,
    before: &Module,
    after: &Module,
    source_map: &DebugSourceMapDocumentV2,
    output_binding: DebugSourceMapBindingV1,
    transform_limits: LoopMemoryTransformLimitsV1,
    lineage_limits: SourceCoordinateLineageLimitsV1,
) -> Result<CheckedSourceCoordinateLineageV1, SourceCoordinateLineageErrorV1> {
    if lineage_limits.max_coordinates == 0
        || lineage_limits.max_coordinates > HARD_MAX_SOURCE_LINEAGE_COORDINATES_V1
    {
        return Err(SourceCoordinateLineageErrorV1::InvalidLimits);
    }
    let input = VerifiedCanonicalKernelIrV13::from_module(before.clone())
        .map_err(SourceCoordinateLineageErrorV1::Input)?;
    let output = VerifiedCanonicalKernelIrV13::from_module(after.clone())
        .map_err(SourceCoordinateLineageErrorV1::Output)?;
    let binding = source_map.binding().canonical_kir();
    if binding.digest() != *input.identity().digest()
        || binding.canonical_bytes() != input.identity().canonical_length()
    {
        return Err(SourceCoordinateLineageErrorV1::InputBindingMismatch);
    }
    let output_kir = output_binding.canonical_kir();
    if output_kir.digest() != *output.identity().digest()
        || output_kir.canonical_bytes() != output.identity().canonical_length()
    {
        return Err(SourceCoordinateLineageErrorV1::OutputBindingMismatch);
    }
    let report =
        check_loop_memory_transform_relation_v1(transform, before, after, transform_limits)
            .map_err(SourceCoordinateLineageErrorV1::Relation)?;
    if report.changed()
        && source_map
            .variables()
            .iter()
            .any(|variable| !variable.locations().is_empty())
    {
        return Err(SourceCoordinateLineageErrorV1::OperationVariableLineageUnsupported);
    }

    let before_coordinates = module_coordinates(before)?;
    let after_coordinates = module_coordinates(after)?;
    let required = before_coordinates
        .len()
        .checked_add(after_coordinates.len())
        .and_then(|count| count.checked_add(report.coordinate_lineage().len()))
        .ok_or(SourceCoordinateLineageErrorV1::ResourceLimit {
            required: usize::MAX,
            limit: lineage_limits.max_coordinates,
        })?;
    if required > lineage_limits.max_coordinates {
        return Err(SourceCoordinateLineageErrorV1::ResourceLimit {
            required,
            limit: lineage_limits.max_coordinates,
        });
    }

    let mut successors =
        BTreeMap::<DebugSourceMapKirSiteV1, BTreeSet<DebugSourceMapKirSiteV1>>::new();
    let mut eliminated = BTreeSet::new();
    for lineage in report.coordinate_lineage() {
        match lineage {
            OperationCoordinateLineageV1::Retained {
                before: source,
                after: destination,
            }
            | OperationCoordinateLineageV1::Moved {
                before: source,
                after: destination,
            } => {
                insert_lineage(source, destination, before, after, &mut successors)?;
            }
            OperationCoordinateLineageV1::Duplicated { source, copy } => {
                insert_lineage(source, copy, before, after, &mut successors)?;
            }
            OperationCoordinateLineageV1::Eliminated { before: coordinate } => {
                eliminated.insert(coordinate_site(before, coordinate)?);
            }
        }
    }
    for coordinate in &before_coordinates {
        let site = coordinate_site(before, coordinate)?;
        if successors.contains_key(&site) || eliminated.contains(&site) {
            continue;
        }
        if after_coordinates.contains(coordinate) {
            successors
                .entry(site)
                .or_default()
                .insert(coordinate_site(after, coordinate)?);
        }
    }
    let covered = successors
        .values()
        .flat_map(BTreeSet::iter)
        .copied()
        .collect::<BTreeSet<_>>();
    if after_coordinates
        .iter()
        .map(|coordinate| coordinate_site(after, coordinate))
        .collect::<Result<BTreeSet<_>, _>>()?
        != covered
    {
        return Err(SourceCoordinateLineageErrorV1::IncompleteCoordinateLineage);
    }

    let mut mapped = BTreeMap::<DebugSourceMapKirSiteV1, BTreeSet<_>>::new();
    let mut eliminated_spans = source_map
        .eliminated()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for source in source_map.sites() {
        let site = source.site();
        if !before_coordinates.contains(&site_coordinate(before, site)?) {
            return Err(SourceCoordinateLineageErrorV1::InvalidCoordinate);
        }
        if let Some(destinations) = successors.get(&site) {
            for destination in destinations {
                mapped
                    .entry(*destination)
                    .or_default()
                    .extend(source.spans().iter().copied());
            }
        } else if eliminated.contains(&site) {
            eliminated_spans.extend(source.spans().iter().copied());
        } else {
            return Err(SourceCoordinateLineageErrorV1::IncompleteCoordinateLineage);
        }
    }
    let sites = mapped
        .into_iter()
        .map(|(site, spans)| {
            DebugSourceMapSiteV1::new(site, spans.into_iter().collect())
                .map_err(SourceCoordinateLineageErrorV1::Binding)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let document = DebugSourceMapDocumentV2::new(
        output_binding,
        source_map.files().to_vec(),
        sites,
        eliminated_spans.into_iter().collect(),
        source_map.scopes().to_vec(),
        source_map.variables().to_vec(),
    )
    .map_err(SourceCoordinateLineageErrorV1::SourceMap)?;
    Ok(CheckedSourceCoordinateLineageV1 {
        document,
        input_identity: *input.identity(),
        output_identity: *output.identity(),
        mapped_coordinates: successors.values().map(BTreeSet::len).sum(),
        eliminated_coordinates: eliminated.len(),
        transform_report: report,
    })
}

fn insert_lineage(
    source: &OperationCoordinateV1,
    destination: &OperationCoordinateV1,
    before: &Module,
    after: &Module,
    successors: &mut BTreeMap<DebugSourceMapKirSiteV1, BTreeSet<DebugSourceMapKirSiteV1>>,
) -> Result<(), SourceCoordinateLineageErrorV1> {
    successors
        .entry(coordinate_site(before, source)?)
        .or_default()
        .insert(coordinate_site(after, destination)?);
    Ok(())
}

fn module_coordinates(
    module: &Module,
) -> Result<BTreeSet<OperationCoordinateV1>, SourceCoordinateLineageErrorV1> {
    module
        .functions
        .iter()
        .flat_map(|function| {
            function.body.as_ref().into_iter().flat_map(move |body| {
                body.blocks.iter().flat_map(move |block| {
                    block
                        .operations
                        .iter()
                        .enumerate()
                        .map(move |(operation, _)| {
                            OperationCoordinateV1::new(&function.id, block.id, operation)
                                .map_err(SourceCoordinateLineageErrorV1::Relation)
                        })
                })
            })
        })
        .collect()
}

fn coordinate_site(
    module: &Module,
    coordinate: &OperationCoordinateV1,
) -> Result<DebugSourceMapKirSiteV1, SourceCoordinateLineageErrorV1> {
    let function = module
        .functions
        .iter()
        .position(|function| &function.id == coordinate.function())
        .ok_or(SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let body = module.functions[function]
        .body
        .as_ref()
        .ok_or(SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let block = body
        .blocks
        .iter()
        .position(|block| block.id == coordinate.block())
        .ok_or(SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let operation = usize::try_from(coordinate.operation())
        .map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    if operation >= body.blocks[block].operations.len() {
        return Err(SourceCoordinateLineageErrorV1::InvalidCoordinate);
    }
    Ok(DebugSourceMapKirSiteV1::operation(
        u64::try_from(function).map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?,
        u64::try_from(block).map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?,
        u64::from(coordinate.operation()),
    ))
}

fn site_coordinate(
    module: &Module,
    site: DebugSourceMapKirSiteV1,
) -> Result<OperationCoordinateV1, SourceCoordinateLineageErrorV1> {
    let function_ordinal = usize::try_from(site.function_ordinal())
        .map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let block_ordinal = usize::try_from(site.block_ordinal())
        .map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let operation_ordinal = usize::try_from(site.operation_ordinal())
        .map_err(|_| SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let function = module
        .functions
        .get(function_ordinal)
        .ok_or(SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    let block = function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(block_ordinal))
        .ok_or(SourceCoordinateLineageErrorV1::InvalidCoordinate)?;
    OperationCoordinateV1::new(&function.id, block.id, operation_ordinal)
        .map_err(SourceCoordinateLineageErrorV1::Relation)
}
