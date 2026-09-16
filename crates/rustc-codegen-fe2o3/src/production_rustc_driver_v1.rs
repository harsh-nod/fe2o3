//! Process-isolated AMD rustc entry for production semantic extraction.

use std::env;
use std::fs::OpenOptions;
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

const EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1: &str =
    "FE2O3_EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX";

#[derive(Default)]
struct ProductionExtractionCallbacksV1 {
    ranked_memory: bool,
    collected_shape: bool,
    amdgpu_llvm_output: Option<PathBuf>,
    expected_llvm_target: Option<&'static str>,
    compiler_handoff_output: Option<(PathBuf, Option<&'static str>)>,
    simulation_bundle_output: Option<PathBuf>,
    simulation_bundle_version: u16,
    result: Option<Result<(), String>>,
}

impl Callbacks for ProductionExtractionCallbacksV1 {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.collected_shape {
            self.result = Some(extract_collected_shape_in_active_session_v1(tcx));
            return Compilation::Stop;
        }
        self.result = Some(
            if let Some(output) = self.simulation_bundle_output.as_deref() {
                match self.simulation_bundle_version {
                    6 => extract_simulation_bundle_in_active_session_v6(tcx, output),
                    5 => extract_simulation_bundle_in_active_session_v5(tcx, output),
                    4 => extract_simulation_bundle_in_active_session_v4(tcx, output),
                    3 => extract_simulation_bundle_in_active_session_v3(tcx, output),
                    2 => extract_simulation_bundle_in_active_session_v2(tcx, output),
                    _ => extract_simulation_bundle_in_active_session_v1(tcx, output),
                }
            } else if let Some((output, expected_target)) = self.compiler_handoff_output.as_ref() {
                extract_amdgpu_compiler_handoff_in_active_session_v1(tcx, output, *expected_target)
            } else if let Some(output) = self.amdgpu_llvm_output.as_deref() {
                extract_amdgpu_llvm_in_active_session_v1(tcx, output, self.expected_llvm_target)
            } else if self.ranked_memory {
                extract_ranked_memory_in_active_session_v1(tcx)
            } else {
                extract_in_active_session_v1(tcx)
            },
        );
        Compilation::Stop
    }
}

fn transaction_in_active_session_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    debug_source_capture: crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2,
) -> Result<
    crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    String,
> {
    let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .map_err(|error| {
            format!("production extraction target authentication failed before monomorphization: {error}")
        })?;
    let partitions = tcx.collect_and_partition_mono_items(());
    let kernel_count = crate::collector::count_kernels_in_cgus(tcx, partitions.codegen_units);
    if kernel_count == 0 {
        return Err(
            "production extraction found no registered kernel in the active AMD rustc session"
                .to_owned(),
        );
    }
    crate::production_pipeline::reject_custom_llvm_configuration(
        crate::has_custom_llvm_configuration(tcx.sess),
    )
    .map_err(|error| format!("production extraction {error}"))?;
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .map_err(|error| format!("production extraction collection failed: {error}"))?;
    let crate_name = tcx.crate_name(LOCAL_CRATE);
    let local_source = tcx
        .sess
        .local_crate_source_file()
        .and_then(|source| source.local_path().map(PathBuf::from));
    let producer = crate::artifact_transaction::ProducerIdentity::from_codegen(
        crate_name.as_str(),
        local_source.as_deref(),
    )
    .map_err(|error| format!("production extraction producer identity failed: {error}"))?;
    let output_dir = env::current_dir()
        .map_err(|error| format!("production extraction working directory failed: {error}"))?;
    match debug_source_capture {
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled => {
            crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_extraction(
                tcx, closure, producer, output_dir,
            )
        }
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables => {
            crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_simulation_v2(
                tcx, closure, producer, output_dir,
            )
        }
    }
    .map_err(|error| format!("production extraction transaction construction failed: {error}"))
}

fn extract_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    Err(transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .require_semantic_mir_import()
    .to_string())
}

const COLLECTED_SHAPE_COMPLETE_V1: &str = "fe2o3 collected-shape: complete; source-proof=not-run; artifact-authority=false; launch-authority=false\n";
// A diagnostic component bound, not an export-frame or transport limit.
const COLLECTED_SHAPE_MAX_BYTES_V1: usize =
    fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectedShapeWriteErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    Capacity,
    Io(std::io::ErrorKind),
    Formatting,
}

struct CollectedShapeWriterV1<'sink, 'budget, 'work, W: std::io::Write> {
    sink: &'sink mut W,
    budget: &'budget mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'work>,
    limit: usize,
    written: usize,
    failure: Option<CollectedShapeWriteErrorV1>,
}

impl<'sink, 'budget, 'work, W: std::io::Write> CollectedShapeWriterV1<'sink, 'budget, 'work, W> {
    fn new(
        sink: &'sink mut W,
        budget: &'budget mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'work>,
        limit: usize,
    ) -> Result<Self, CollectedShapeWriteErrorV1> {
        use CollectedShapeWriteErrorV1 as Error;
        if limit > COLLECTED_SHAPE_MAX_BYTES_V1 - COLLECTED_SHAPE_COMPLETE_V1.len() {
            return Err(Error::Capacity);
        }
        // Prepay the final static marker, written only after both postflights.
        budget
            .charge_work(2 + COLLECTED_SHAPE_COMPLETE_V1.len())
            .map_err(Error::Resource)?;
        budget
            .reserve_storage(std::mem::size_of::<Self>())
            .map_err(Error::Resource)?;
        Ok(Self {
            sink,
            budget,
            limit,
            written: 0,
            failure: None,
        })
    }

    fn fail(&mut self, error: CollectedShapeWriteErrorV1) -> std::fmt::Error {
        self.failure.get_or_insert(error);
        std::fmt::Error
    }

    fn record(&mut self, args: std::fmt::Arguments<'_>) -> Result<(), CollectedShapeWriteErrorV1> {
        use std::fmt::Write as _;
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Err(error) = self.budget.charge_work(1) {
            self.fail(CollectedShapeWriteErrorV1::Resource(error));
        } else if std::fmt::write(self, args)
            .and_then(|()| self.write_str("\n"))
            .is_err()
            && self.failure.is_none()
        {
            self.fail(CollectedShapeWriteErrorV1::Formatting);
        }
        self.failure.map_or(Ok(()), Err)
    }

    fn finish(&mut self) -> Result<(), CollectedShapeWriteErrorV1> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Err(error) = self.budget.charge_work(1) {
            self.fail(CollectedShapeWriteErrorV1::Resource(error));
        }
        self.failure.map_or(Ok(()), Err)
    }
}

impl<W: std::io::Write> std::fmt::Write for CollectedShapeWriterV1<'_, '_, '_, W> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        use CollectedShapeWriteErrorV1 as Error;
        if self.failure.is_some() {
            return Err(std::fmt::Error);
        }
        let Some(next) = self
            .written
            .checked_add(text.len())
            .filter(|next| *next <= self.limit)
        else {
            return Err(self.fail(Error::Capacity));
        };
        let Some(work) = text.len().checked_add(1) else {
            return Err(self.fail(Error::Capacity));
        };
        if let Err(error) = self.budget.charge_work(work) {
            return Err(self.fail(Error::Resource(error)));
        }
        if let Err(error) = self.sink.write_all(text.as_bytes()) {
            return Err(self.fail(Error::Io(error.kind())));
        }
        self.written = next;
        Ok(())
    }
}

fn write_collected_module_v1<W: std::io::Write>(
    writer: &mut CollectedShapeWriterV1<'_, '_, '_, W>,
    label: &'static str,
    module: &fe2o3_kernel_ir::Module,
) -> Result<(), CollectedShapeWriteErrorV1> {
    writer.record(format_args!("graph {label} id={:?}", module.id))?;
    for capability in &module.required_capabilities {
        writer.record(format_args!("graph {label} capability: {capability:?}"))?;
    }
    for (ordinal, kernel) in module.kernels.iter().enumerate() {
        writer.record(format_args!("graph {label} kernel {ordinal}: {kernel:?}"))?;
    }
    for (function_index, function) in module.functions.iter().enumerate() {
        writer.record(format_args!(
            "graph {label} function {function_index}: id={:?} role={:?}",
            function.id, function.role
        ))?;
        for capability in &function.required_capabilities {
            writer.record(format_args!(
                "graph {label} function {function_index} capability: {capability:?}"
            ))?;
        }
        for (ordinal, ty) in function.signature.parameters.iter().enumerate() {
            writer.record(format_args!(
                "graph {label} function {function_index} signature-argument {ordinal}: {ty:?}"
            ))?;
        }
        for (ordinal, ty) in function.signature.results.iter().enumerate() {
            writer.record(format_args!(
                "graph {label} function {function_index} signature-result {ordinal}: {ty:?}"
            ))?;
        }
        if let Some(body) = &function.body {
            for (ordinal, parameter) in body.parameters.iter().enumerate() {
                writer.record(format_args!(
                    "graph {label} function {function_index} argument {ordinal}: {parameter:?}"
                ))?;
            }
            for (block_index, block) in body.blocks.iter().enumerate() {
                writer.record(format_args!(
                    "graph {label} function {function_index} block {block_index}: id={:?}",
                    block.id
                ))?;
                for (ordinal, parameter) in block.parameters.iter().enumerate() {
                    writer.record(format_args!("graph {label} function {function_index} block {block_index} parameter {ordinal}: {parameter:?}"))?;
                }
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    for (result, value) in operation.results.iter().enumerate() {
                        writer.record(format_args!("graph {label} function {function_index} block {block_index} operation {ordinal} result {result}: {value:?}"))?;
                    }
                    writer.record(format_args!("graph {label} function {function_index} block {block_index} operation {ordinal} uses: {:?}", operation.kind))?;
                }
                writer.record(format_args!(
                    "graph {label} function {function_index} block {block_index} terminator: {:?}",
                    block.terminator
                ))?;
            }
        }
    }
    Ok(())
}

fn extract_collected_shape_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    let mut stderr = std::io::stderr().lock();
    let result = transaction.with_collected_shape_observation_v1(
        |source, bound, checked, references, profile, budget| {
            let (coordinates, storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    profile,
                    budget,
                )
                .map_err(|error| error.to_string())?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(|error| error.to_string())?;
            let (view, storage) =
                fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                    source,
                    &coordinates,
                    checked,
                    budget,
                )
                .map_err(|error| error.to_string())?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(|error| error.to_string())?;
            write_collected_shape_report_v1(&view, references, profile, budget, &mut stderr)
        },
    );
    // The original callback value is successful only after both existing
    // postflights. Dynamic text above is Debug-escaped and cannot inject this line.
    complete_collected_shape_observation_v1(result, &mut stderr)
}

fn write_collected_shape_report_v1(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'_, '_>,
    references: &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    sink: &mut impl std::io::Write,
) -> Result<(), String> {
    let source = view.source();
    let bound = view.bound();
    let write_error = |error| format!("collected shape diagnostic failed: {error:?}");
    let mut writer = CollectedShapeWriterV1::new(
        sink,
        budget,
        COLLECTED_SHAPE_MAX_BYTES_V1 - COLLECTED_SHAPE_COMPLETE_V1.len(),
    )
    .map_err(write_error)?;
    writer.record(format_args!("fe2o3 collected-shape: incomplete; source-proof=not-run; artifact-authority=false; launch-authority=false; profile={profile:?}; references={}", references.len())).map_err(write_error)?;
    for (ordinal, reference) in references.iter().enumerate() {
        writer.record(format_args!("source reference {ordinal}: name={:?} kernel={:?} reference={:?} effect-sha256={:?} writes={}", reference.logical_kernel_name, reference.kernel, reference.reference, reference.effect_ir_sha256, reference.observable_output_writes.len())).map_err(write_error)?;
    }
    let ssa = source.semantic_ssa();
    let semantic = ssa.source_semantic();
    let occurrences = ssa
        .occurrences_v1()
        .ok_or_else(|| "collected shape requires actual source SSA capture".to_owned())?;
    for (ordinal, root) in semantic.roots().iter().enumerate() {
        writer
            .record(format_args!("source root {ordinal}: {root:?}"))
            .map_err(write_error)?;
    }
    for (ordinal, ty) in semantic.types().iter().enumerate() {
        writer
            .record(format_args!("source type {ordinal}: {ty:?}"))
            .map_err(write_error)?;
    }
    for (ordinal, callable) in semantic.callables().iter().enumerate() {
        writer
            .record(format_args!("source callable {ordinal}: {callable:?}"))
            .map_err(write_error)?;
    }
    for (index, function) in semantic.functions().iter().enumerate() {
        writer
            .record(format_args!(
                "source function {index}: identity={:?} role={:?} entry={:?}",
                function.identity(),
                function.role(),
                function.entry()
            ))
            .map_err(write_error)?;
        writer
            .record(format_args!(
                "source function {index} export: {:?}",
                function.export()
            ))
            .map_err(write_error)?;
        for (ordinal, ownership) in function
            .abi()
            .source_argument_ownership()
            .iter()
            .enumerate()
        {
            writer
                .record(format_args!(
                    "source function {index} ownership {ordinal}: {ownership:?}"
                ))
                .map_err(write_error)?;
        }
        for (ordinal, argument) in function.abi().arguments().iter().enumerate() {
            writer
                .record(format_args!(
                    "source function {index} abi {ordinal}: {argument:?}"
                ))
                .map_err(write_error)?;
        }
        for (ordinal, local) in function.locals().iter().enumerate() {
            writer
                .record(format_args!(
                    "source function {index} local {ordinal}: {local:?}"
                ))
                .map_err(write_error)?;
        }
        for (block_index, block) in function.blocks().iter().enumerate() {
            for (ordinal, statement) in block.statements().iter().enumerate() {
                writer.record(format_args!("source function {index} block {block_index} statement {ordinal}: {statement:?}")).map_err(write_error)?;
            }
            writer
                .record(format_args!(
                    "source function {index} block {block_index} terminator: {:?}",
                    block.terminator()
                ))
                .map_err(write_error)?;
        }
        let id = fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1::from_index(
            u32::try_from(index)
                .map_err(|_| "collected shape function ordinal overflow".to_owned())?,
        );
        let rows = occurrences
            .function(id)
            .ok_or_else(|| "collected shape missing captured function".to_owned())?;
        for row in rows.entry_definitions() {
            writer
                .record(format_args!("ssa function {index} entry: {row:?}"))
                .map_err(write_error)?;
        }
        for row in rows.events() {
            writer
                .record(format_args!("ssa function {index} event: {row:?}"))
                .map_err(write_error)?;
        }
        for row in rows.constants() {
            writer
                .record(format_args!("ssa function {index} constant: {row:?}"))
                .map_err(write_error)?;
        }
        for row in rows.successors() {
            writer
                .record(format_args!("ssa function {index} successor: {row:?}"))
                .map_err(write_error)?;
        }
        for row in rows.edge_definitions() {
            writer
                .record(format_args!(
                    "ssa function {index} edge-definition: {row:?}"
                ))
                .map_err(write_error)?;
        }
        for row in rows.elisions() {
            writer
                .record(format_args!("ssa function {index} elision: {row:?}"))
                .map_err(write_error)?;
        }
    }
    write_collected_module_v1(&mut writer, "N", source.executable().module())
        .map_err(write_error)?;
    write_collected_module_v1(&mut writer, "B", bound.module()).map_err(write_error)?;
    write_collected_module_v1(&mut writer, "O", view.output().module()).map_err(write_error)?;
    for (ordinal, trace) in source.source_store_value_uses_v1().iter().enumerate() {
        writer
            .record(format_args!("source Store trace {ordinal}: {trace:?}"))
            .map_err(write_error)?;
        let (block, statement) = trace.source_statement();
        let access = view
            .global_access(
                trace.correspondence_owner(),
                trace.semantic_function(),
                block.index(),
                Some(statement),
                0,
                writer.budget,
            )
            .map_err(|error| error.to_string())?;
        let source_block = view
            .block(
                trace.correspondence_owner(),
                trace.semantic_function(),
                block,
                writer.budget,
            )
            .map_err(|error| error.to_string())?;
        use fe2o3_lower_mir_kernel::{
            ProductionSourceOutputBlockV1 as SourceBlock,
            ProductionSourceOutputGlobalAccessV1 as Access,
        };
        writer
            .budget
            .charge_work(8)
            .map_err(|error| error.to_string())?;
        let source_function = match source_block {
            SourceBlock::Materialized { original, .. } => Some(original.function),
            SourceBlock::NotMaterialized => None,
        };
        writer
            .record(format_args!(
                "source Store trace {ordinal} source-block: {source_block:?}"
            ))
            .map_err(write_error)?;
        let original = match access {
            Access::Retained { original, .. } | Access::OmittedUnreachable { original } => {
                Some(original)
            }
            Access::Unsupported(_) => None,
        };
        let matches = collected_store_location_matches_v1(
            source.executable().module(),
            source_function,
            original,
            trace.store(),
        );
        writer.record(format_args!("source Store trace {ordinal} first-access ordinal=0 matches-traced-store={matches}: {access:?}")).map_err(write_error)?;
    }
    writer.finish().map_err(write_error)
}

// Numeric diagnostic comparison only. Its caller obtains the source function
// from the exact paid source-block query; a source block may expand internally.
fn collected_store_location_matches_v1(
    module: &fe2o3_kernel_ir::Module,
    source_function: Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
    original: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
    store: (fe2o3_kernel_ir::BlockId, u32),
) -> bool {
    let (Some(function), Some(coordinate)) = (source_function, original) else {
        return false;
    };
    function == coordinate.block.function
        && module
            .functions
            .get(coordinate.block.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
            .is_some_and(|block| (block.id, coordinate.operation) == store)
}

fn complete_collected_shape_observation_v1(
    postflight_result: Result<(), String>,
    sink: &mut impl std::io::Write,
) -> Result<(), String> {
    postflight_result?;
    sink.write_all(COLLECTED_SHAPE_COMPLETE_V1.as_bytes())
        .map_err(|error| format!("collected shape completion output failed: {error}"))
}

#[cfg(test)]
mod collected_shape_tests_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::fmt::Write as _;

    fn header() -> usize {
        std::mem::size_of::<CollectedShapeWriterV1<'_, '_, '_, Vec<u8>>>()
    }

    #[test]
    fn diagnostic_locator_component_rejects_same_local_coordinates_in_another_function() {
        use fe2o3_kernel_ir::{
            BasicBlock, BlockId, CanonicalKirBlockCoordinateV1 as Block,
            CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
            CanonicalKirOperationCoordinateV1 as OperationCoordinate, Function, Module, Signature,
        };
        // Inert locations exercise only the private diagnostic comparator,
        // not source admission, an executable Store, or any proof authority.
        let mut module = Module::new("diagnostic-locations");
        for name in ["a", "b"] {
            module.functions.push(Function::definition(
                name,
                Signature::new(vec![], vec![]),
                vec![],
                vec![BasicBlock::new(BlockId(7))],
            ));
        }
        let original = |function| {
            Some(OperationCoordinate {
                block: Block {
                    function: FunctionCoordinate(function),
                    block: 0,
                },
                operation: 3,
            })
        };
        assert!(collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(0)),
            original(0),
            (BlockId(7), 3)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(0)),
            original(1),
            (BlockId(7), 3)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            None,
            original(0),
            (BlockId(7), 3)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(0)),
            None,
            (BlockId(7), 3)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(0)),
            original(0),
            (BlockId(8), 3)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(0)),
            original(0),
            (BlockId(7), 4)
        ));
        assert!(!collected_store_location_matches_v1(
            &module,
            Some(FunctionCoordinate(2)),
            original(2),
            (BlockId(7), 3)
        ));
    }

    #[test]
    fn diagnostic_writer_exact_capacity_work_and_live_floor() {
        let initial = 2 + COLLECTED_SHAPE_COMPLETE_V1.len();
        let mut work = Work::new(7 + initial + 4 + 1);
        let mut budget = Budget::new(&mut work, 11 + header());
        budget.charge_work(7).unwrap();
        budget.reserve_storage(11).unwrap();
        let mut bytes = Vec::new();
        {
            let mut writer = CollectedShapeWriterV1::new(&mut bytes, &mut budget, 3).unwrap();
            writer.write_str("abc").unwrap();
            assert_eq!(writer.written, 3);
            writer.finish().unwrap();
        }
        assert_eq!(bytes, b"abc");
        assert_eq!(budget.work(), 7 + initial + 4 + 1);
        assert_eq!(budget.storage(), 11 + header());
        budget.release_storage(header()).unwrap();
        assert_eq!(budget.storage(), 11);
    }

    #[test]
    fn diagnostic_writer_poison_survives_capacity_and_nonsticky_work_denial() {
        let initial = 2 + COLLECTED_SHAPE_COMPLETE_V1.len();
        for work_denial in [false, true] {
            let mut work = Work::new(if work_denial { initial + 1 } else { usize::MAX });
            let mut budget = Budget::new(&mut work, header());
            let mut bytes = Vec::new();
            {
                let mut writer = CollectedShapeWriterV1::new(&mut bytes, &mut budget, 1).unwrap();
                let text = if work_denial { "x" } else { "xx" };
                assert!(writer.write_str(text).is_err());
                let error = writer.failure.unwrap();
                if work_denial {
                    assert!(matches!(
                        error,
                        CollectedShapeWriteErrorV1::Resource(Resource::Work(_))
                    ));
                } else {
                    assert_eq!(error, CollectedShapeWriteErrorV1::Capacity);
                }
                assert!(writer.write_str("").is_err());
                assert_eq!(writer.finish(), Err(error));
                assert_eq!(writer.record(format_args!("")), Err(error));
            }
            assert!(bytes.is_empty());
            budget.release_storage(header()).unwrap();
            // The ledger itself permits a smaller later charge; the writer did not.
            budget.charge_work(1).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn diagnostic_writer_header_and_finish_one_short_fail_without_completion() {
        let initial = 2 + COLLECTED_SHAPE_COMPLETE_V1.len();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, header() - 1);
        let mut bytes = Vec::new();
        assert!(matches!(
            CollectedShapeWriterV1::new(&mut bytes, &mut budget, 0),
            Err(CollectedShapeWriteErrorV1::Resource(Resource::Storage(_)))
        ));
        assert_eq!(budget.storage(), 0);
        let mut work = Work::new(initial);
        let mut budget = Budget::new(&mut work, header());
        {
            let mut writer = CollectedShapeWriterV1::new(&mut bytes, &mut budget, 0).unwrap();
            assert!(matches!(
                writer.finish(),
                Err(CollectedShapeWriteErrorV1::Resource(Resource::Work(_)))
            ));
            assert!(writer.finish().is_err());
        }
        budget.release_storage(header()).unwrap();
        assert!(bytes.is_empty());
        assert!(matches!(
            CollectedShapeWriterV1::new(&mut bytes, &mut budget, COLLECTED_SHAPE_MAX_BYTES_V1),
            Err(CollectedShapeWriteErrorV1::Capacity)
        ));
    }

    struct BrokenSink;
    impl std::io::Write for BrokenSink {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn diagnostic_writer_io_and_format_failures_are_poisoned() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut sink = BrokenSink;
        {
            let mut writer = CollectedShapeWriterV1::new(&mut sink, &mut budget, 32).unwrap();
            assert_eq!(
                writer.record(format_args!("row")),
                Err(CollectedShapeWriteErrorV1::Io(
                    std::io::ErrorKind::BrokenPipe
                ))
            );
            assert_eq!(
                writer.finish(),
                Err(CollectedShapeWriteErrorV1::Io(
                    std::io::ErrorKind::BrokenPipe
                ))
            );
        }
        budget
            .release_storage(std::mem::size_of::<
                CollectedShapeWriterV1<'_, '_, '_, BrokenSink>,
            >())
            .unwrap();
        struct BrokenDebug;
        impl std::fmt::Debug for BrokenDebug {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        let mut bytes = Vec::new();
        {
            let mut writer = CollectedShapeWriterV1::new(&mut bytes, &mut budget, 32).unwrap();
            assert_eq!(
                writer.record(format_args!("{:?}", BrokenDebug)),
                Err(CollectedShapeWriteErrorV1::Formatting)
            );
            assert_eq!(writer.finish(), Err(CollectedShapeWriteErrorV1::Formatting));
        }
        budget.release_storage(header()).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn diagnostic_text_cannot_inject_completion_and_failed_postflight_never_emits_it() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, header());
        let mut bytes = Vec::new();
        {
            let mut writer = CollectedShapeWriterV1::new(&mut bytes, &mut budget, 1024).unwrap();
            writer
                .record(format_args!(
                    "source name={:?}",
                    COLLECTED_SHAPE_COMPLETE_V1
                ))
                .unwrap();
            writer.finish().unwrap();
        }
        budget.release_storage(header()).unwrap();
        let before = bytes.len();
        assert!(
            !String::from_utf8_lossy(&bytes)
                .lines()
                .any(|line| line == COLLECTED_SHAPE_COMPLETE_V1.trim_end())
        );
        assert_eq!(
            complete_collected_shape_observation_v1(Err("postflight".to_owned()), &mut bytes),
            Err("postflight".to_owned())
        );
        assert_eq!(bytes.len(), before);
        complete_collected_shape_observation_v1(Ok(()), &mut bytes).unwrap();
        assert_eq!(&bytes[before..], COLLECTED_SHAPE_COMPLETE_V1.as_bytes());
        assert!(complete_collected_shape_observation_v1(Ok(()), &mut BrokenSink).is_err());
    }

    #[test]
    fn collected_diagnostic_uses_original_captured_scopes_without_source_proof() {
        let source = include_str!("production_checked_output_pipeline_v1.rs");
        let body = source
            .split("pub(crate) fn with_collected_shape_observation_v1(")
            .nth(1)
            .unwrap()
            .split("fn with_source_checked_output_transaction_v1<")
            .next()
            .unwrap();
        assert!(body.contains("compiler_custody.is_extraction_only()"));
        assert!(body.contains("with_captured_materialized_target_neutral_v1"));
        assert!(body.contains("with_checked_output_target_endpoint_v1"));
        assert!(body.contains("stage.bindings.reference_effect_bindings.as_slice()"));
        assert!(body.contains("Ok(next("));
        assert!(!body.contains("with_source_ranked_custody_v1"));
        assert!(!body.contains("prove_and_compile"));
        assert!(!body.contains("reference_effect_bindings.clear"));
    }
}

fn extract_ranked_memory_in_active_session_v1(tcx: TyCtxt<'_>) -> Result<(), String> {
    let ranked = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .verify_general_kernel_checks()
    .map_err(|error| error.to_string())?;
    if let [root] = ranked.ranked_roots() {
        eprintln!(
            "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> safety-verified lowering input for `{}`; {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, all mandatory kernel checks clean {}, bounds clean {}\n{}",
            root.function_name(),
            ranked.semantic_function_count(),
            ranked.semantic_callable_count(),
            ranked.retained_identity_and_transaction_binding_count(),
            ranked.grants_artifact_or_launch_authority(),
            ranked.all_kernel_checks_are_clean(),
            ranked.bounds_are_clean(),
            root.ranked_ir(),
        );
    } else {
        eprintln!(
            "fe2o3 production extraction: Rust -> semantic MIR -> ordered ranked PLIRON roster; {} kernel root(s), {} semantic function(s), {} callable record(s), {} retained identity/transaction binding(s), artifact/launch authority {}, all mandatory kernel checks clean {}, bounds clean {}",
            ranked.ranked_root_count(),
            ranked.semantic_function_count(),
            ranked.semantic_callable_count(),
            ranked.retained_identity_and_transaction_binding_count(),
            ranked.grants_artifact_or_launch_authority(),
            ranked.all_kernel_checks_are_clean(),
            ranked.bounds_are_clean(),
        );
        for (ordinal, root) in ranked.ranked_roots().iter().enumerate() {
            eprintln!(
                "ranked root {ordinal}: `{}`, kernel binding {}, source rank {}, all mandatory kernel checks clean {}, bounds clean {}\n{}",
                root.function_name(),
                lower_hex_v1(root.kernel_binding()),
                root.source_rank(),
                root.all_kernel_checks_are_clean(),
                root.bounds_are_clean(),
                root.ranked_ir(),
            );
        }
    }
    Ok(())
}

fn extract_amdgpu_llvm_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let neutral_provider_observation = match (
        crate::trusted_device_items::definition(
            tcx,
            crate::trusted_device_items::TrustedDeviceItem::WorkgroupCollectivesCurrent,
        ),
        crate::trusted_device_items::definition(
            tcx,
            crate::trusted_device_items::TrustedDeviceItem::WorkgroupReduceSum,
        ),
    ) {
        (None, None) => None,
        (Some(_), Some(_)) => {
            let (collectives_current, provider_closure) =
                crate::trusted_device_items::authenticated_compiler_definition_observation_v1(
                    tcx,
                    crate::trusted_device_items::TrustedDeviceItem::WorkgroupCollectivesCurrent,
                )
                .map_err(|error| {
                    format!("neutral workgroup provider observation failed: {error}")
                })?;
            let (reduce_sum, reduce_provider_closure) =
                crate::trusted_device_items::authenticated_compiler_definition_observation_v1(
                    tcx,
                    crate::trusted_device_items::TrustedDeviceItem::WorkgroupReduceSum,
                )
                .map_err(|error| {
                    format!("neutral workgroup provider observation failed: {error}")
                })?;
            if provider_closure != reduce_provider_closure {
                return Err(
                    "neutral workgroup provider observations name different source closures".into(),
                );
            }
            Some((collectives_current, reduce_sum, provider_closure))
        }
        _ => {
            return Err(
                "neutral workgroup provider observation found an incomplete authenticated pair"
                    .into(),
            );
        }
    };
    let lowered = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_production_target()
    .map_err(|error| error.to_string())?;
    if let Some(expected_target) = expected_target
        && lowered.target_name() != expected_target
    {
        return Err(format!(
            "production LLVM extraction expected live target {expected_target:?}; found {:?}",
            lowered.target_name()
        ));
    }
    std::fs::write(output, lowered.llvm_ir()).map_err(|error| {
        format!(
            "failed to write production {} LLVM extraction `{}`: {error}",
            lowered.target_name(),
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> target-KIR optimizer ({} pass(es), {} mutating, epoch {}..={}) -> {} LLVM; {} semantic function(s), {} semantic u32 induction certificate(s) for {} checked addition(s), {} correspondence block(s), {} formal access(es), {} ranked dynamic-index discharge(s), ordered workgroups {:?}, {} LLVM byte(s), artifact/launch authority {}",
        lowered.canonical_kernel_ir_version(),
        lowered.guarded_store_count(),
        lowered.target_optimization_pass_count(),
        lowered.target_optimization_mutating_pass_count(),
        lowered.target_optimization_initial_epoch(),
        lowered.target_optimization_final_epoch(),
        lowered.target_name(),
        lowered.semantic_function_count(),
        lowered.semantic_u32_induction_certificate_count(),
        lowered.semantic_u32_induction_checked_addition_count(),
        lowered.correspondence_block_count(),
        lowered.formal_access_count(),
        lowered.ranked_dynamic_index_discharge_count(),
        lowered.workgroup_sizes(),
        lowered.llvm_ir().len(),
        lowered.grants_artifact_or_launch_authority(),
    );
    if let Some((collectives_current, reduce_sum, provider_closure)) = neutral_provider_observation
    {
        eprintln!(
            "fe2o3 production extraction: authenticated rustc provider definitions `{collectives_current}` and `{reduce_sum}` in source closure {}; this is a compiler build observation, not package or runtime authority",
            lower_hex_v1(&provider_closure),
        );
    }
    Ok(())
}

fn extract_amdgpu_compiler_handoff_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    if env::var_os(EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1).is_some() {
        return extract_amdgpu_semantic_compiler_handoff_in_active_session_v3(
            tcx,
            output,
            expected_target,
        );
    }
    let lowered = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_production_target()
    .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let target_name = lowered.target_name().to_owned();
    let canonical_kernel_ir_version = lowered.canonical_kernel_ir_version();
    let guarded_store_count = lowered.guarded_store_count();
    let handoff = lowered
        .into_inert_worker_handoff_for_extraction()
        .map_err(|error| error.to_string())?;
    std::fs::write(output, handoff.canonical_bytes()).map_err(|error| {
        format!(
            "failed to write inert production compiler-module handoff extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> {} LLVM -> compiler-bound inert handoff; {} handoff byte(s), artifact/launch authority false",
        canonical_kernel_ir_version,
        guarded_store_count,
        target_name,
        handoff.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_amdgpu_semantic_compiler_handoff_in_active_session_v3(
    tcx: TyCtxt<'_>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let invocation = inert_extraction_invocation_v3()?;
    let lowered = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .lower_production_target()
    .map_err(|error| error.to_string())?;
    validate_compiler_handoff_target(lowered.target_name(), expected_target)?;
    let target_name = lowered.target_name().to_owned();
    let canonical_kernel_ir_version = lowered.canonical_kernel_ir_version();
    let guarded_store_count = lowered.guarded_store_count();
    let handoff = lowered
        .into_inert_semantic_worker_handoff_for_extraction(invocation)
        .map_err(|error| error.to_string())?;
    std::fs::write(output, handoff.canonical_bytes()).map_err(|error| {
        format!(
            "failed to write inert production semantic compiler-module handoff extraction `{}`: {error}",
            output.display()
        )
    })?;
    eprintln!(
        "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR V{} with {} GuardedStore operation(s) -> composed formal/ranked memory -> {} LLVM -> proof-carrying semantic compiler-bound inert handoff; {} handoff byte(s), artifact/launch authority false",
        canonical_kernel_ir_version,
        guarded_store_count,
        target_name,
        handoff.canonical_bytes().len(),
    );
    Ok(())
}

fn validate_compiler_handoff_target(target: &str, expected: Option<&str>) -> Result<(), String> {
    if fe2o3_amd_target::ProductionAmdTargetProfileV1::from_device_target(target).is_none() {
        return Err(format!(
            "production compiler handoff has unsupported live target {target:?}"
        ));
    }
    if let Some(expected) = expected
        && target != expected
    {
        return Err(format!(
            "production compiler handoff expected live target {expected:?}; found {target:?}"
        ));
    }
    Ok(())
}

fn inert_extraction_invocation_v3()
-> Result<fe2o3_rustc_invocation::RustcInvocationDescriptorV3, String> {
    let encoded = env::var(EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1).map_err(|_| {
        format!(
            "semantic compiler-handoff extraction requires exact inert invocation bytes in {EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1}"
        )
    })?;
    if encoded.is_empty()
        || encoded.len() % 2 != 0
        || encoded.len() > fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3.saturating_mul(2)
    {
        return Err(format!(
            "{EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1} has an invalid canonical length"
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(encoded.len() / 2)
        .map_err(|_| "inert extraction invocation is too large".to_owned())?;
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = canonical_hex_nibble(pair[0]).ok_or_else(|| {
            format!("{EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1} is not lowercase hexadecimal")
        })?;
        let low = canonical_hex_nibble(pair[1]).ok_or_else(|| {
            format!("{EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1} is not lowercase hexadecimal")
        })?;
        bytes.push((high << 4) | low);
    }
    fe2o3_rustc_invocation::decode_descriptor_v3(&bytes).map_err(|error| {
        format!(
            "{EXTRACT_INERT_RUSTC_INVOCATION_V3_HEX_ENV_V1} is not a canonical V3 invocation: {error}"
        )
    })
}

const fn canonical_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn extract_simulation_bundle_in_active_session_v1(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .export_simulation_bundle_v1()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle_v1(output, bundle.canonical_bytes())?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> semantic MIR -> ranked PLIRON checks -> sole target-neutral Kernel IR lowering -> exact verified KIR V7 simulation bundle; target {}, {} kernel(s), simulation_bundle_subject {}, content {}, {} byte(s), compiler_execution_binding=extraction_only_unavailable, authenticates_compiler_execution=false, debug map {}, proof/artifact/compiler/hardware/load/launch authority false",
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        bundle.canonical_bytes().len(),
        if bundle.debug_map().is_some() {
            "present"
        } else {
            "none"
        },
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v2(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v2()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle_v2(output, bundle.canonical_bytes())?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> semantic MIR -> ranked PLIRON checks -> sole target-neutral Kernel IR lowering -> explicit simulation bundle V2 with compiler-produced source variables; target {}, {} kernel(s), simulation_bundle_subject {}, content {}, {} byte(s), compiler_execution_binding=extraction_only_unavailable, authenticates_compiler_execution=false, debug map V2 present, proof/artifact/compiler/hardware/load/launch authority false",
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v3(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v3()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle(
        output,
        bundle.canonical_bytes(),
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V3,
    )?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> admitted semantic MIR -> ranked PLIRON checks -> sole target-neutral Kernel IR lowering -> simulation bundle V3 with exact semantic MIR, source variables, and typed storage correspondence; target {}, {} kernel(s), simulation_bundle_subject {}, content {}, semantic_mir {}, storage_map {}, {} byte(s), compiler_execution_binding=extraction_only_unavailable, authenticates_compiler_execution=false, proof/artifact/compiler/hardware/load/launch authority false",
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        lower_hex_v1(bundle.semantic_mir_identity()),
        lower_hex_v1(bundle.storage_map_identity()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v4(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v4()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle(
        output,
        bundle.canonical_bytes(),
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V4,
    )?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> admitted semantic MIR -> target-neutral Kernel IR -> simulation bundle V4 with compiler-rederived aggregate component and physical simulator-kernarg correspondence; target {}, {} kernel(s), content {}, storage_map {}, {} byte(s), KFD packing/launch authority=false",
        bundle.inner_v3().target(),
        bundle.inner_v3().kernel_count(),
        lower_hex_v1(bundle.identity().as_bytes()),
        lower_hex_v1(bundle.storage_map_identity()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v5(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v5()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle(
        output,
        bundle.canonical_bytes(),
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V5,
    )?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> admitted semantic MIR -> target-neutral production KIR V{} -> exact same-module KIR V10 simulation bundle V5; target {}, {} kernel(s), subject {}, content {}, source_map {}, semantic_mir {}, storage_map {}, aggregate_map {}, {} byte(s), compiler_execution=extraction_only_unavailable, proof/compiler/artifact/hardware/load/launch authority false",
        bundle.production_kir_identity().version(),
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        lower_hex_v1(&bundle.debug_map_identity()),
        lower_hex_v1(&bundle.semantic_mir_identity()),
        lower_hex_v1(&bundle.storage_map_identity()),
        lower_hex_v1(&bundle.aggregate_storage_map_identity()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn extract_simulation_bundle_in_active_session_v6(
    tcx: TyCtxt<'_>,
    output: &Path,
) -> Result<(), String> {
    let bundle = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
    )?
    .export_simulation_bundle_v6()
    .map_err(|error| error.to_string())?;
    publish_new_simulation_bundle(
        output,
        bundle.canonical_bytes(),
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V6,
    )?;
    eprintln!(
        "fe2o3 production extraction: ordinary Rust -> admitted semantic MIR -> target-neutral production KIR V{} -> exact same-module KIR V11 simulation bundle V6; target {}, {} kernel(s), subject {}, content {}, source_map {}, semantic_mir {}, storage_map {}, aggregate_map {}, {} byte(s), compiler_execution=extraction_only_unavailable, proof/compiler/artifact/hardware/load/launch authority false",
        bundle.production_kir_identity().version(),
        bundle.target(),
        bundle.kernel_count(),
        lower_hex_v1(bundle.subject_identity()),
        lower_hex_v1(bundle.identity().as_bytes()),
        lower_hex_v1(&bundle.debug_map_identity()),
        lower_hex_v1(&bundle.semantic_mir_identity()),
        lower_hex_v1(&bundle.storage_map_identity()),
        lower_hex_v1(&bundle.aggregate_storage_map_identity()),
        bundle.canonical_bytes().len(),
    );
    Ok(())
}

fn publish_new_simulation_bundle_v1(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_simulation_bundle(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V1,
    )
}

fn publish_new_simulation_bundle_v2(output: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_new_simulation_bundle(
        output,
        bytes,
        fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V2,
    )
}

fn publish_new_simulation_bundle(
    output: &Path,
    bytes: &[u8],
    maximum: usize,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err("refusing to publish an empty or oversized simulation bundle".to_owned());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(output).map_err(|error| {
        format!(
            "failed to create new simulation bundle output `{}`: {error}",
            output.display()
        )
    })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        return Err(format!(
            "failed to publish simulation bundle `{}`; the create-new partial output was retained for fail-closed cleanup: {error}",
            output.display()
        ));
    }
    #[cfg(unix)]
    {
        let descriptor = file.metadata().map_err(|error| {
            format!(
                "failed to inspect published simulation bundle descriptor `{}`: {error}",
                output.display()
            )
        })?;
        let path = std::fs::symlink_metadata(output).map_err(|error| {
            format!(
                "failed to re-inspect published simulation bundle path `{}`: {error}",
                output.display()
            )
        })?;
        if !descriptor.is_file()
            || descriptor.len() != bytes.len() as u64
            || descriptor.dev() != path.dev()
            || descriptor.ino() != path.ino()
            || path.file_type().is_symlink()
        {
            return Err(format!(
                "simulation bundle output `{}` changed identity during publication",
                output.display()
            ));
        }
    }
    Ok(())
}

fn lower_hex_v1(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

/// Runs one already-targeted rustc invocation in this process.
///
/// The caller must provide the complete rustc argument vector, including argv0.
/// No host compiler values cross this boundary. The callback discovers roots,
/// collects, and imports synchronously inside the AMD `TyCtxt` it receives.
pub fn run_production_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    run_production_driver_v1(
        args,
        ProductionExtractionCallbacksV1::default(),
        "production extraction callback did not reach rustc analysis",
    )
}

/// Observes actual collected source/N/B/checked O without executing source proof
/// or admitting artifacts. This is a diagnostic, not a verification result.
pub fn run_production_collected_shape_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    run_production_driver_v1(
        args,
        ProductionExtractionCallbacksV1 {
            collected_shape: true,
            ..Default::default()
        },
        "collected shape callback did not reach rustc analysis",
    )
}

/// Runs the same production importer followed by generic ranked-memory
/// construction and verification, without granting artifact authority.
pub fn run_production_ranked_extraction_driver_v1(args: &[String]) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: true,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_version: 1,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production ranked extraction callback did not reach rustc analysis",
    )
}

/// Runs the complete production analysis and exact live-target lowering
/// transaction, emitting deterministic AMDGPU LLVM text to the selected path.
pub fn run_production_amdgpu_llvm_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: Some(output.to_path_buf()),
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_version: 1,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production AMDGPU extraction callback did not reach rustc analysis",
    )
}

/// Compatibility entry point for the original exact gfx942 extraction API.
pub fn run_production_gfx942_llvm_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: Some(output.to_path_buf()),
        expected_llvm_target: Some(fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1),
        compiler_handoff_output: None,
        simulation_bundle_output: None,
        simulation_bundle_version: 1,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production gfx942 extraction callback did not reach rustc analysis",
    )
}

/// Emits the live gfx942 or gfx950 production target's inert compiler handoff.
/// Target authentication remains in the production transaction; this entry
/// does not provide publication, artifact, load, or launch authority.
pub fn run_production_amdgpu_compiler_handoff_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        compiler_handoff_output: Some((output.to_path_buf(), None)),
        ..ProductionExtractionCallbacksV1::default()
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production AMDGPU compiler-handoff extraction callback did not reach rustc analysis",
    )
}

/// Runs the complete production analysis and lowering transaction and emits
/// its compiler-bound nested handoff for inert worker integration testing.
/// The result carries no publication, artifact, load, or launch authority.
/// This compatibility entry continues to reject every target other than gfx942.
pub fn run_production_gfx942_compiler_handoff_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: Some((
            output.to_path_buf(),
            Some(fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1),
        )),
        simulation_bundle_output: None,
        simulation_bundle_version: 1,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production compiler-handoff extraction callback did not reach rustc analysis",
    )
}

/// Runs the sole production source transaction through target-neutral lowering
/// and publishes one authority-free exact KIR V7 simulation bundle. This path
/// never enters LLVM, artifact publication, a runtime, or hardware fallback.
pub fn run_production_simulation_bundle_extraction_driver_v1(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 1,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V2 simulation export with compiler-produced source-variable
/// records. V1 remains the default and byte-compatible extraction route.
pub fn run_production_simulation_bundle_extraction_driver_v2(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 2,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V2 extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V3 export that embeds exact canonical semantic MIR and a
/// bundle-bound map from semantic locals to retained KIR parameter storage.
pub fn run_production_simulation_bundle_extraction_driver_v3(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 3,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V3 extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V4 export with compiler-produced one-to-many aggregate
/// storage and physical simulator-kernarg correspondence.
pub fn run_production_simulation_bundle_extraction_driver_v4(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 4,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V4 extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V5 export with an exact V10 same-module projection of the
/// producer-owned canonical V8/V9 KIR and self-contained debug/storage maps.
pub fn run_production_simulation_bundle_extraction_driver_v5(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 5,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V5 extraction callback did not reach rustc analysis",
    )
}

/// Runs the opt-in V6 export with one exact producer-owned canonical V11 KIR
/// and self-contained debug/storage maps.
pub fn run_production_simulation_bundle_extraction_driver_v6(
    args: &[String],
    output: &Path,
) -> Result<(), String> {
    let callbacks = ProductionExtractionCallbacksV1 {
        ranked_memory: false,
        collected_shape: false,
        amdgpu_llvm_output: None,
        expected_llvm_target: None,
        compiler_handoff_output: None,
        simulation_bundle_output: Some(output.to_path_buf()),
        simulation_bundle_version: 6,
        result: None,
    };
    run_production_driver_v1(
        args,
        callbacks,
        "production simulation-bundle V6 extraction callback did not reach rustc analysis",
    )
}

fn run_production_driver_v1(
    args: &[String],
    mut callbacks: ProductionExtractionCallbacksV1,
    missing_callback: &'static str,
) -> Result<(), String> {
    require_canonical_overflow_checks_v1(args)?;
    rustc_driver::run_compiler(args, &mut callbacks);
    callbacks
        .result
        .unwrap_or_else(|| Err(missing_callback.to_owned()))
}

fn require_canonical_overflow_checks_v1(args: &[String]) -> Result<(), String> {
    let mut observed = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            break;
        }
        if argument == "-C" || argument == "--codegen" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("production rustc option `{argument}` has no value"))?;
            if value.starts_with("overflow-checks=") {
                observed.push((argument.as_str(), value.as_str()));
            }
            index += 2;
            continue;
        }
        if argument.starts_with("-Coverflow-checks=")
            || argument.starts_with("--codegen=overflow-checks=")
        {
            observed.push((argument.as_str(), ""));
        }
        index += 1;
    }
    if observed == [("-Coverflow-checks=on", "")] {
        Ok(())
    } else {
        Err(format!(
            "production rustc invocation requires exactly one canonical `-Coverflow-checks=on`; observed {observed:?}"
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generic_handoff_accepts_exact_targets_and_legacy_remains_gfx942_only() {
        use super::validate_compiler_handoff_target;
        for target in ["gfx942:xnack-", "gfx950:xnack-"] {
            assert!(validate_compiler_handoff_target(target, None).is_ok());
            assert_eq!(
                validate_compiler_handoff_target(target, Some("gfx942:xnack-")).is_ok(),
                target == "gfx942:xnack-"
            );
        }
        for target in [
            "gfx942",
            "gfx950",
            "gfx950:xnack+",
            "gfx950:sramecc+:xnack-",
            "gfx951:xnack-",
        ] {
            assert!(validate_compiler_handoff_target(target, None).is_err());
        }
    }

    use super::*;

    #[test]
    fn production_driver_requires_one_canonical_overflow_policy() {
        require_canonical_overflow_checks_v1(&[
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "kernel".to_owned(),
            "-Coverflow-checks=on".to_owned(),
        ])
        .unwrap();

        for rejected in [
            vec!["rustc"],
            vec!["rustc", "-Coverflow-checks=off"],
            vec!["rustc", "-C", "overflow-checks=on"],
            vec![
                "rustc",
                "-Coverflow-checks=on",
                "--codegen=overflow-checks=on",
            ],
            vec!["rustc", "--", "-Coverflow-checks=on"],
        ] {
            let args = rejected.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert!(
                require_canonical_overflow_checks_v1(&args)
                    .unwrap_err()
                    .contains("requires exactly one canonical")
            );
        }
    }

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-simulation-bundle-output-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn simulation_bundle_output_is_create_new_exact_and_private() {
        let root = scratch();
        let output = root.join("kernel.fe2sim");
        publish_new_simulation_bundle_v1(&output, b"exact-bundle").unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
        assert!(publish_new_simulation_bundle_v1(&output, b"replacement").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, symlink};
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = root.join("link.fe2sim");
            symlink(&output, &link).unwrap();
            assert!(publish_new_simulation_bundle_v1(&link, b"replacement").is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulation_bundle_output_rejects_empty_and_oversized_payloads() {
        let root = scratch();
        assert!(publish_new_simulation_bundle_v1(&root.join("empty"), b"").is_err());
        assert!(
            publish_new_simulation_bundle_v1(
                &root.join("oversized"),
                &vec![0; fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V1 + 1],
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
