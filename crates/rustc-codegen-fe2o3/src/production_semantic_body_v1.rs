//! Bounded construction of semantic function bodies from the reviewed rustc MIR subset.
//!
//! This module is deliberately a pure producer. It consumes caller-frozen
//! canonical identities and source records, checks them against the live rustc
//! body, and constructs inert semantic-MIR records. It does not qualify,
//! admit, lower, compile, or authorize the resulting records.

use std::collections::HashMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAggregateKindV1, SemanticAssertMessageV1,
    SemanticAssignmentV1, SemanticAtomicRmwV1, SemanticBasicBlockV1, SemanticBinaryOpV1,
    SemanticBlockIdV1, SemanticBlockIdentityV1, SemanticBorrowKindV1, SemanticCallDestinationV1,
    SemanticCallableIdV1, SemanticCastKindV1, SemanticCheckedBinaryOpV1,
    SemanticCheckedBinaryRvalueV1, SemanticConstGenericArgumentsIdentityV1,
    SemanticConstantBytesV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticControlFlowEdgeV1, SemanticDirectCallV1, SemanticEdgeRoleV1, SemanticFunctionAbiV1,
    SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticFunctionIdentityV1,
    SemanticFunctionRoleV1, SemanticGenericTypeArgumentsIdentityV1,
    SemanticItemDefinitionIdentityV1, SemanticKernelEntryV1, SemanticLinkSymbolV1,
    SemanticLocalDeclV1, SemanticLocalIdV1, SemanticLocalIdentityV1, SemanticLocalRoleV1,
    SemanticMemoryLoadV1, SemanticMirErrorV1, SemanticMirLimitsV1, SemanticMirResourceV1,
    SemanticMonomorphizationIdentityV1, SemanticMutabilityV1, SemanticOperandV1, SemanticPlaceV1,
    SemanticProjectionKindV1, SemanticProjectionV1, SemanticRvalueKindV1, SemanticRvalueV1,
    SemanticScalarValueV1, SemanticSourceProvenanceV1, SemanticStatementKindV1,
    SemanticStatementV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1, SemanticTerminatorKindV1,
    SemanticTerminatorV1, SemanticTypeIdV1, SemanticUnaryOpV1, SemanticUncheckedBinaryOpV1,
    SemanticUncheckedBinaryRvalueV1, SemanticUnwindActionV1, SemanticVolatilityV1,
};
use rustc_hir::Mutability;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::{
    AggregateKind, AssertKind, BinOp, Body, BorrowKind, CastKind, ConstValue, MutBorrowKind,
    NonDivergingIntrinsic, Operand, Place, PlaceTy, ProjectionElem, RETURN_PLACE, RawPtrKind,
    Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnOp, UnwindAction,
};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyCtxt, TyKind, TypingEnv};

use crate::collector::workgroup_scope_custody_v29::{
    PendingWorkgroupScopesV29, ScopeCallableV29, ScopeEventV29,
};
use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};
use crate::production_rustc_intrinsic_v1::ProductionRustcIntrinsicOperationV1;
use crate::production_rustc_slice_metadata_v1::{
    SliceMetadataErrorV1, SliceMetadataPlanV1, SliceMetadataRewriteV1,
};
use crate::production_safe_core_shift_v1::NormalizedCallV1;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;

mod function_commitments_v29;
pub(crate) mod receiver_materialization_v1;
pub(crate) mod receiver_reborrow_v1;
pub(crate) use function_commitments_v29::ExpectedFunctionCommitmentV29;
use function_commitments_v29::{
    PendingFunctionCommitmentsV29, charge_construction_total_v1, construction_resource_error_v1,
};
use receiver_materialization_v1::{ReceiverLocalV1, ReceiverMaterializationV1};
mod primitive_from_materialization_v1;
#[cfg(test)]
pub(crate) mod primitive_from_stage_tests;
mod wrapping_materialization_v1;
use wrapping_materialization_v1::WrappingMaterializationV1;

#[cfg(test)]
#[path = "production_semantic_body_v1/construction_work_tests.rs"]
mod construction_work_tests;
#[cfg(test)]
#[path = "production_semantic_body_v1/receiver_construction_tests.rs"]
mod receiver_construction_tests;

const MAX_ERROR_COMPONENT_CHARS_V1: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticFunctionIdentitiesV1 {
    identity: SemanticFunctionIdentityV1,
    item_definition: SemanticItemDefinitionIdentityV1,
    monomorphization: SemanticMonomorphizationIdentityV1,
    generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
}

impl ProductionSemanticFunctionIdentitiesV1 {
    pub(crate) const fn new(
        identity: SemanticFunctionIdentityV1,
        item_definition: SemanticItemDefinitionIdentityV1,
        monomorphization: SemanticMonomorphizationIdentityV1,
        generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
        const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
    ) -> Self {
        Self {
            identity,
            item_definition,
            monomorphization,
            generic_type_arguments,
            const_generic_arguments,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSemanticFunctionExportV1 {
    None,
    Kernel(SemanticKernelEntryV1),
    DeviceFfi(SemanticLinkSymbolV1),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticTypeBindingV1<'tcx> {
    rustc_type: Ty<'tcx>,
    semantic_type: SemanticTypeIdV1,
}

impl<'tcx> ProductionSemanticTypeBindingV1<'tcx> {
    pub(crate) const fn new(rustc_type: Ty<'tcx>, semantic_type: SemanticTypeIdV1) -> Self {
        Self {
            rustc_type,
            semantic_type,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticLocalBindingV1 {
    rustc_local: u32,
    semantic_local: SemanticLocalIdV1,
    identity: SemanticLocalIdentityV1,
    source: SemanticSourceProvenanceV1,
}

impl ProductionSemanticLocalBindingV1 {
    pub(crate) const fn new(
        rustc_local: u32,
        semantic_local: SemanticLocalIdV1,
        identity: SemanticLocalIdentityV1,
        source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            rustc_local,
            semantic_local,
            identity,
            source,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticBlockBindingV1 {
    rustc_block: u32,
    semantic_block: SemanticBlockIdV1,
    identity: SemanticBlockIdentityV1,
    source: SemanticSourceProvenanceV1,
    statement_sources: Box<[SemanticSourceProvenanceV1]>,
    terminator_source: SemanticSourceProvenanceV1,
}

impl ProductionSemanticBlockBindingV1 {
    pub(crate) fn new(
        rustc_block: u32,
        semantic_block: SemanticBlockIdV1,
        identity: SemanticBlockIdentityV1,
        source: SemanticSourceProvenanceV1,
        statement_sources: Vec<SemanticSourceProvenanceV1>,
        terminator_source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            rustc_block,
            semantic_block,
            identity,
            source,
            statement_sources: statement_sources.into_boxed_slice(),
            terminator_source,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticDirectCallBindingV1<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
}

impl<'tcx> ProductionSemanticDirectCallBindingV1<'tcx> {
    pub(crate) const fn new(
        caller: SemanticFunctionIdV1,
        rustc_block: u32,
        expected_callee: Instance<'tcx>,
    ) -> Self {
        Self {
            caller,
            rustc_block,
            expected_callee,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticTerminalExpansionRecipeV1<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
    expansion: ProductionTerminalExpansionV1,
}

impl<'tcx> ProductionSemanticTerminalExpansionRecipeV1<'tcx> {
    pub(crate) const fn new(
        caller: SemanticFunctionIdV1,
        rustc_block: u32,
        expected_callee: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
    ) -> Self {
        Self {
            caller,
            rustc_block,
            expected_callee,
            expansion,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
    expected_element_type: Ty<'tcx>,
    operation: NormalizedCallV1<'tcx>,
}

impl<'tcx> ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx> {
    pub(crate) const fn new(
        caller: SemanticFunctionIdV1,
        rustc_block: u32,
        expected_callee: Instance<'tcx>,
        expected_element_type: Ty<'tcx>,
        operation: NormalizedCallV1<'tcx>,
    ) -> Self {
        Self {
            caller,
            rustc_block,
            expected_callee,
            expected_element_type,
            operation,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ProductionSemanticCallableOwnerEntryV1<'tcx> {
    Defined {
        rustc_instance: Instance<'tcx>,
        semantic_callable: SemanticCallableIdV1,
    },
    Terminal {
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
        semantic_callable: SemanticCallableIdV1,
    },
}

impl<'tcx> ProductionSemanticCallableOwnerEntryV1<'tcx> {
    pub(crate) const fn defined(
        rustc_instance: Instance<'tcx>,
        semantic_callable: SemanticCallableIdV1,
    ) -> Self {
        Self::Defined {
            rustc_instance,
            semantic_callable,
        }
    }

    pub(crate) const fn terminal(
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
        semantic_callable: SemanticCallableIdV1,
    ) -> Self {
        Self::Terminal {
            rustc_instance,
            expansion,
            semantic_callable,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductionSemanticCallableOwnerKindV1 {
    Defined,
    Terminal(ProductionTerminalExpansionV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductionSemanticCallableOwnerRecordV1 {
    kind: ProductionSemanticCallableOwnerKindV1,
    semantic_callable: SemanticCallableIdV1,
}

/// Request-owned authority for callable identities and cumulative body limits.
///
/// The importer constructs this once, in canonical callable order, from the
/// authenticated preflight plan. Individual call-site recipes cannot name a
/// semantic callable and therefore cannot substitute an ABI-compatible callee.
pub(crate) struct ProductionSemanticBodyRequestOwnerV1<'tcx> {
    limits: SemanticMirLimitsV1,
    totals: ConstructionTotalsV1,
    callables: HashMap<Instance<'tcx>, ProductionSemanticCallableOwnerRecordV1>,
    inline_sources: crate::production_inline_source_occurrences_v30::InlineSourceOccurrencesV30,
    ordered_sources: crate::production_ordered_source_occurrences_v31::OrderedSourceOccurrencesV31,
    program_sources: crate::production_ordered_composition_source_v1::OrderedProgramSourcesV1<'tcx>,
    bf16_inspection: crate::production_tiled_region_source_v1::Bf16MfmaImportCaptureV1<'tcx>,
    bf16_tile_values:
        crate::production_bf16_tile_values_source_v1::Bf16TileValuesImportCaptureV1<'tcx>,
    complete_body_annotation: Option<
        crate::production_complete_body_annotation_vnext::CompleteBodyCallAnnotationVNext<'tcx>,
    >,
    physical_entry_annotations:
        Option<crate::production_physical_entry_annotation_v37::PhysicalEntryAnnotationsV37<'tcx>>,
    physical_global_copy_annotations: Option<
        crate::production_physical_global_copy_annotation_v38::PhysicalGlobalCopyAnnotationsV38<
            'tcx,
        >,
    >,
    physical_lds_exchange_annotations: Option<
        crate::production_physical_lds_exchange_annotation_v39::PhysicalLdsExchangeAnnotationsV39<
            'tcx,
        >,
    >,
    defined_functions: usize,
    context_entries: Vec<crate::collector::RetainedContextEntryV29>,
    function_commitments: Option<PendingFunctionCommitmentsV29<'tcx>>,
    workgroup_scopes: Option<PendingWorkgroupScopesV29>,
}

impl<'tcx> ProductionSemanticBodyRequestOwnerV1<'tcx> {
    #[cfg(test)]
    pub(crate) fn new(
        limits: SemanticMirLimitsV1,
        type_count: usize,
        callable_entries: &[ProductionSemanticCallableOwnerEntryV1<'tcx>],
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        Self::with_preflight_work(
            crate::rustc_semantic_plan_v1::ProductionSemanticConstructionWorkV1::for_test(
                limits, 0,
            ),
            type_count,
            callable_entries,
        )
    }

    pub(crate) fn with_preflight_work(
        work: crate::rustc_semantic_plan_v1::ProductionSemanticConstructionWorkV1,
        type_count: usize,
        callable_entries: &[ProductionSemanticCallableOwnerEntryV1<'tcx>],
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        let (limits, validation_work) = work.into_parts();
        let mut totals = ConstructionTotalsV1 {
            validation_work,
            ..ConstructionTotalsV1::default()
        };
        totals.charge(SemanticMirResourceV1::ValidationWork, 0, limits)?;
        totals.charge(SemanticMirResourceV1::Types, type_count, limits)?;
        totals.charge(
            SemanticMirResourceV1::Callables,
            callable_entries.len(),
            limits,
        )?;
        let mut callables = HashMap::new();
        callables
            .try_reserve(callable_entries.len())
            .map_err(|_| allocation(SemanticMirResourceV1::Callables))?;
        let mut defined_functions = 0;
        for (index, entry) in callable_entries.iter().copied().enumerate() {
            totals.charge(SemanticMirResourceV1::ValidationWork, 1, limits)?;
            let (rustc_instance, record) = match entry {
                ProductionSemanticCallableOwnerEntryV1::Defined {
                    rustc_instance,
                    semantic_callable,
                } => (
                    rustc_instance,
                    ProductionSemanticCallableOwnerRecordV1 {
                        kind: ProductionSemanticCallableOwnerKindV1::Defined,
                        semantic_callable,
                    },
                ),
                ProductionSemanticCallableOwnerEntryV1::Terminal {
                    rustc_instance,
                    expansion,
                    semantic_callable,
                } => (
                    rustc_instance,
                    ProductionSemanticCallableOwnerRecordV1 {
                        kind: ProductionSemanticCallableOwnerKindV1::Terminal(expansion),
                        semantic_callable,
                    },
                ),
            };
            require_canonical_callable_id_v1(index, record.semantic_callable)?;
            if record.kind == ProductionSemanticCallableOwnerKindV1::Defined {
                defined_functions += 1;
            }
            if callables.insert(rustc_instance, record).is_some() {
                return Err(table("callable owner table"));
            }
        }
        Ok(Self {
            limits,
            totals,
            callables,
            inline_sources: Default::default(),
            ordered_sources: Default::default(),
            program_sources: Default::default(),
            bf16_inspection: Default::default(),
            bf16_tile_values: Default::default(),
            complete_body_annotation: None,
            physical_entry_annotations: None,
            physical_global_copy_annotations: None,
            physical_lds_exchange_annotations: None,
            defined_functions,
            context_entries: Vec::new(),
            function_commitments: None,
            workgroup_scopes: None,
        })
    }

    pub(crate) fn with_inline_sources_v30(
        mut self,
        sources: crate::production_inline_source_occurrences_v30::InlineSourceOccurrencesV30,
    ) -> Self {
        self.inline_sources = sources;
        self
    }

    pub(crate) fn require_inline_sources_consumed_v30(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.inline_sources.require_drained().map_err(table)
    }

    pub(crate) fn with_ordered_sources_v31(
        mut self,
        sources: crate::production_ordered_source_occurrences_v31::OrderedSourceOccurrencesV31,
    ) -> Self {
        self.ordered_sources = sources;
        self
    }

    pub(crate) fn require_ordered_sources_consumed_v31(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.ordered_sources.require_drained().map_err(table)
    }

    pub(crate) fn with_program_sources_v32(
        mut self,
        sources: crate::production_ordered_program_source_occurrences_v32::OrderedProgramSourceOccurrencesV32<'tcx>,
    ) -> Self {
        self.program_sources =
            crate::production_ordered_composition_source_v1::OrderedProgramSourcesV1::Singleton(
                sources,
            );
        self
    }

    pub(crate) fn prepare_ordered_composition_sources_v1(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
        types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
        abis: &[SemanticFunctionAbiV1],
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_ordered_composition_source_v1::{
            OrderedCompositionSourcesV1, OrderedProgramSourcesV1,
        };
        if self.totals.functions != 0
            || matches!(
                self.program_sources,
                OrderedProgramSourcesV1::Composition(_)
            )
            || self.program_sources.require_drained().is_err()
        {
            return Err(table(
                "ordered composition source custody cannot be replaced",
            ));
        }
        let work = OrderedCompositionSourcesV1::validation_work(plan).map_err(table)?;
        self.charge(SemanticMirResourceV1::ValidationWork, work)?;
        let sources =
            OrderedCompositionSourcesV1::from_plan(tcx, plan, types, abis).map_err(table)?;
        self.program_sources = OrderedProgramSourcesV1::Composition(sources);
        Ok(())
    }

    pub(crate) fn require_program_sources_consumed_v32(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.program_sources.require_drained().map_err(table)
    }

    pub(crate) fn complete_ordered_source_import_v1(
        &mut self,
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    ) -> Result<
        crate::production_ordered_composition_source_v1::OrderedSourceImportCompletionV1<'tcx>,
        ProductionSemanticBodyErrorV1,
    > {
        self.program_sources.complete(semantic).map_err(table)
    }

    pub(crate) fn prepare_bf16_inspection_source_v1(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_tiled_region_source_v1::{
            Bf16MfmaImportCaptureV1, PendingBf16MfmaSourceSeedV1,
        };
        if self.totals.functions != 0
            || !matches!(self.bf16_inspection, Bf16MfmaImportCaptureV1::Disabled)
            || !matches!(self.bf16_tile_values, crate::production_bf16_tile_values_source_v1::Bf16TileValuesImportCaptureV1::Disabled)
        {
            return Err(table("BF16 source custody cannot be replaced"));
        }
        let work = PendingBf16MfmaSourceSeedV1::validation_work(plan).map_err(table)?;
        self.charge(SemanticMirResourceV1::ValidationWork, work)?;
        self.bf16_inspection = Bf16MfmaImportCaptureV1::Capturing(
            PendingBf16MfmaSourceSeedV1::capture_precharged(tcx, plan).map_err(table)?,
        );
        Ok(())
    }
    pub(crate) fn complete_bf16_inspection_source_v1(
        &mut self,
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    ) -> Result<
        crate::production_tiled_region_source_v1::Bf16MfmaImportCompletionV1<'tcx>,
        ProductionSemanticBodyErrorV1,
    > {
        self.bf16_inspection.complete(semantic).map_err(table)
    }

    pub(crate) fn prepare_bf16_tile_values_source_v1(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_bf16_tile_values_source_v1::{
            Bf16TileValuesImportCaptureV1, PendingBf16TileValuesSourceSeedV1,
        };
        if self.totals.functions != 0
            || !matches!(
                self.bf16_tile_values,
                Bf16TileValuesImportCaptureV1::Disabled
            )
            || !matches!(
                self.bf16_inspection,
                crate::production_tiled_region_source_v1::Bf16MfmaImportCaptureV1::Disabled
            )
        {
            return Err(table("BF16 source custody cannot be replaced"));
        }
        let work = PendingBf16TileValuesSourceSeedV1::validation_work(plan).map_err(table)?;
        self.charge(SemanticMirResourceV1::ValidationWork, work)?;
        self.bf16_tile_values = Bf16TileValuesImportCaptureV1::Capturing(
            PendingBf16TileValuesSourceSeedV1::capture_precharged(tcx, plan).map_err(table)?,
        );
        Ok(())
    }
    pub(crate) fn complete_bf16_tile_values_source_v1(
        &mut self,
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    ) -> Result<
        crate::production_bf16_tile_values_source_v1::Bf16TileValuesImportCompletionV1<'tcx>,
        ProductionSemanticBodyErrorV1,
    > {
        self.bf16_tile_values.complete(semantic).map_err(table)
    }

    pub(crate) fn prepare_physical_entry_annotations_v37(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_physical_entry_annotation_v37 as physical;
        if self.physical_entry_annotations.is_some() {
            return Err(table("physical-entry annotation cannot be replaced"));
        }
        self.charge(
            SemanticMirResourceV1::ValidationWork,
            physical::PREPAID_CONSTRUCTION_WORK_V37,
        )?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
            physical::PREPAID_CONSTRUCTION_WORK_V37,
        );
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        self.physical_entry_annotations = Some(
            physical::prepare(tcx, plan, &mut budget).map_err(|e| unsupported(e, None, None))?,
        );
        Ok(())
    }

    pub(crate) fn require_physical_entry_annotations_consumed_v37(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if let Some(annotations) = &self.physical_entry_annotations {
            annotations.require_drained().map_err(table)?;
        }
        Ok(())
    }

    pub(crate) fn prepare_physical_lds_exchange_annotations_v39(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_physical_lds_exchange_annotation_v39 as physical;
        if self.physical_lds_exchange_annotations.is_some() {
            return Err(table("physical-lds-exchange annotation cannot be replaced"));
        }
        self.charge(
            SemanticMirResourceV1::ValidationWork,
            physical::PREPAID_CONSTRUCTION_WORK_V39,
        )?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
            physical::PREPAID_CONSTRUCTION_WORK_V39,
        );
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        self.physical_lds_exchange_annotations = Some(
            physical::prepare(tcx, plan, &mut budget).map_err(|e| unsupported(e, None, None))?,
        );
        Ok(())
    }

    pub(crate) fn require_physical_lds_exchange_annotations_consumed_v39(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if let Some(annotations) = &self.physical_lds_exchange_annotations {
            annotations.require_drained().map_err(table)?;
        }
        Ok(())
    }

    pub(crate) fn prepare_physical_global_copy_annotations_v38(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_physical_global_copy_annotation_v38 as physical;
        if self.physical_global_copy_annotations.is_some() {
            return Err(table("physical-global-copy annotation cannot be replaced"));
        }
        self.charge(
            SemanticMirResourceV1::ValidationWork,
            physical::PREPAID_CONSTRUCTION_WORK_V38,
        )?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
            physical::PREPAID_CONSTRUCTION_WORK_V38,
        );
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        self.physical_global_copy_annotations = Some(
            physical::prepare(tcx, plan, &mut budget).map_err(|e| unsupported(e, None, None))?,
        );
        Ok(())
    }

    pub(crate) fn require_physical_global_copy_annotations_consumed_v38(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if let Some(annotations) = &self.physical_global_copy_annotations {
            annotations.require_drained().map_err(table)?;
        }
        Ok(())
    }

    pub(crate) fn prepare_complete_body_annotation_vnext(
        &mut self,
        tcx: TyCtxt<'tcx>,
        plan: &crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        use crate::production_complete_body_annotation_vnext as body;
        if self.complete_body_annotation.is_some() {
            return Err(table("complete-body annotation cannot be replaced"));
        }
        // Charge the EXISTING cumulative semantic-construction account first.
        // The bounded local adapter spends only that named prepaid allowance.
        self.charge(
            SemanticMirResourceV1::ValidationWork,
            body::PREPAID_CONSTRUCTION_WORK_VNEXT,
        )?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
            body::PREPAID_CONSTRUCTION_WORK_VNEXT,
        );
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let annotation = body::prepare_complete_body_annotation_vnext(tcx, plan, &mut budget)
            .map_err(|error| unsupported(error, None, None))?;
        self.complete_body_annotation = Some(annotation);
        Ok(())
    }

    pub(crate) fn require_complete_body_annotation_consumed_vnext(
        &self,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if let Some(annotation) = &self.complete_body_annotation {
            annotation.require_drained().map_err(table)?;
        }
        Ok(())
    }

    pub(crate) fn seal_context_entries(
        mut self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<crate::collector::RetainedContextEntriesV29, ProductionSemanticBodyErrorV1> {
        if self.function_commitments.is_some() == self.context_entries.is_empty()
            || self.function_commitments.is_some() != self.workgroup_scopes.is_some()
        {
            return Err(table("function commitment context transaction"));
        }
        if let Some(pending) = self.function_commitments.take() {
            pending.verify(semantic, &mut self.totals, self.limits)?;
        }
        let entries = std::mem::take(&mut self.context_entries);
        let scopes = self
            .workgroup_scopes
            .take()
            .map(|scopes| {
                self.totals
                    .declaration_tables_commitment_v29(
                        semantic.types(),
                        semantic.callables(),
                        self.limits,
                    )
                    .map(|declarations| (scopes, declarations))
            })
            .transpose()?;
        crate::collector::RetainedContextEntriesV29::seal_with_scopes(
            entries,
            scopes,
            semantic,
            |amount| self.charge(SemanticMirResourceV1::ValidationWork, amount),
        )
    }

    pub(crate) fn enable_workgroup_scope_custody_v29(
        &mut self,
        tcx: TyCtxt<'tcx>,
        target: fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
        semantic_types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
        semantic_callables: &[fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1],
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if self.workgroup_scopes.is_some()
            || self.totals.functions != 0
            || self.function_commitments.is_none()
            || semantic_callables.len() != self.callables.len()
        {
            return Err(table("scope custody initialization"));
        }
        // Authenticate once in the rustc importer domain, not at every event.
        let provider = crate::trusted_device_items::definition(
            tcx,
            crate::trusted_device_items::TrustedDeviceItem::ExecutionWithWorkgroup,
        );
        self.charge(SemanticMirResourceV1::ValidationWork, self.callables.len())?;
        let mut classes = try_vec_v1(self.callables.len(), SemanticMirResourceV1::Callables)?;
        classes.resize(self.callables.len(), ScopeCallableV29::Ordinary);
        for (instance, record) in &self.callables {
            self.totals
                .charge(SemanticMirResourceV1::ValidationWork, 1, self.limits)?;
            let slot = classes
                .get_mut(record.semantic_callable.index() as usize)
                .ok_or_else(|| table("scope callable owner index"))?;
            if matches!(instance.def, rustc_middle::ty::InstanceKind::Item(definition)
                if Some(definition) == provider)
            {
                if record.kind != ProductionSemanticCallableOwnerKindV1::Defined {
                    return Err(table("scope provider callable owner"));
                }
                let identity = self
                    .function_commitments
                    .as_ref()
                    .ok_or_else(|| table("scope function source roster"))?
                    .source_identity(record.semantic_callable)?;
                *slot = ScopeCallableV29::Provider {
                    function: SemanticFunctionIdV1::from_index(record.semantic_callable.index()),
                    identity,
                };
            } else if record.kind
                == ProductionSemanticCallableOwnerKindV1::Terminal(
                    ProductionTerminalExpansionV1::WorkgroupDerive,
                )
            {
                *slot = ScopeCallableV29::derive_from_constructed(
                    &semantic_callables[record.semantic_callable.index() as usize],
                )?;
            }
        }
        let declarations = self.totals.declaration_tables_commitment_v29(
            semantic_types,
            semantic_callables,
            self.limits,
        )?;
        self.workgroup_scopes = Some(PendingWorkgroupScopesV29::new(
            classes,
            declarations,
            target,
            self.defined_functions,
        )?);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn retained_context_entry_count(&self) -> usize {
        self.context_entries.len()
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.totals.charge(resource, amount, self.limits)
    }

    fn defined_callable(
        &self,
        rustc_instance: Instance<'tcx>,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let record = self
            .callables
            .get(&rustc_instance)
            .copied()
            .ok_or_else(|| table("callable owner table"))?;
        resolve_owned_callable_record_v1(record, ProductionSemanticCallableOwnerKindV1::Defined)
    }

    fn terminal_callable(
        &self,
        rustc_instance: Instance<'tcx>,
        expansion: ProductionTerminalExpansionV1,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let record = self
            .callables
            .get(&rustc_instance)
            .copied()
            .ok_or_else(|| table("callable owner table"))?;
        resolve_owned_callable_record_v1(
            record,
            ProductionSemanticCallableOwnerKindV1::Terminal(expansion),
        )
    }
}

fn resolve_owned_callable_record_v1(
    record: ProductionSemanticCallableOwnerRecordV1,
    expected: ProductionSemanticCallableOwnerKindV1,
) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
    if record.kind != expected {
        return Err(table("callable owner table"));
    }
    Ok(record.semantic_callable)
}

fn require_canonical_callable_id_v1(
    canonical_index: usize,
    semantic_callable: SemanticCallableIdV1,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let expected = u32::try_from(canonical_index).map_err(|_| table("callable owner table"))?;
    if semantic_callable.index() != expected {
        return Err(table("callable owner table"));
    }
    Ok(())
}

pub(crate) struct ProductionSemanticBodyInputV1<'a, 'tcx> {
    pub(crate) context_entry: Option<crate::collector::BoundContextEntryV29<'tcx>>,
    pub(crate) tcx: TyCtxt<'tcx>,
    pub(crate) instance: Instance<'tcx>,
    pub(crate) body: &'a Body<'tcx>,
    pub(crate) function: SemanticFunctionIdV1,
    pub(crate) identities: ProductionSemanticFunctionIdentitiesV1,
    pub(crate) role: SemanticFunctionRoleV1,
    pub(crate) export: ProductionSemanticFunctionExportV1,
    pub(crate) source: SemanticSourceProvenanceV1,
    pub(crate) abi: SemanticFunctionAbiV1,
    pub(crate) type_bindings: &'a [ProductionSemanticTypeBindingV1<'tcx>],
    pub(crate) local_bindings: &'a [ProductionSemanticLocalBindingV1],
    pub(crate) block_bindings: &'a [ProductionSemanticBlockBindingV1],
    pub(crate) entry: SemanticBlockIdV1,
    pub(crate) direct_calls: &'a [ProductionSemanticDirectCallBindingV1<'tcx>],
    pub(crate) terminal_expansions: &'a [ProductionSemanticTerminalExpansionRecipeV1<'tcx>],
    pub(crate) normalized_intrinsics:
        &'a [ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx>],
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticBodyErrorV1 {
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    Allocation {
        resource: SemanticMirResourceV1,
    },
    IdentityTableMismatch {
        table: &'static str,
    },
    RustcStartBlockPredecessor {
        predecessor: u32,
    },
    Unsupported {
        construct: String,
        block: Option<u32>,
        statement: Option<u32>,
    },
    Schema(SemanticMirErrorV1),
}

impl fmt::Display for ProductionSemanticBodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic body construction exceeded {resource:?}: {actual} > {maximum}",
            ),
            Self::Allocation { resource } => {
                write!(
                    formatter,
                    "semantic body construction could not allocate {resource:?}"
                )
            }
            Self::IdentityTableMismatch { table } => {
                write!(
                    formatter,
                    "semantic body construction rejected inconsistent {table}"
                )
            }
            Self::RustcStartBlockPredecessor { predecessor } => write!(
                formatter,
                "semantic body construction rejected invalid rustc MIR: start block has predecessor block {predecessor}",
            ),
            Self::Unsupported {
                construct,
                block,
                statement,
            } => match (block, statement) {
                (Some(block), Some(statement)) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in block {block}, statement {statement}",
                ),
                (Some(block), None) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in block {block}, terminator",
                ),
                (None, _) => write!(
                    formatter,
                    "semantic body construction rejected {construct} in function metadata",
                ),
            },
            Self::Schema(error) => write!(
                formatter,
                "semantic body construction produced an invalid schema record: {error}",
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticBodyErrorV1 {}

impl From<SemanticMirErrorV1> for ProductionSemanticBodyErrorV1 {
    fn from(error: SemanticMirErrorV1) -> Self {
        Self::Schema(error)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ConstructionTotalsV1 {
    types: u64,
    functions: u64,
    callables: u64,
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    constant_bytes: u64,
    validation_work: u64,
}

impl ConstructionTotalsV1 {
    fn slot_mut(&mut self, resource: SemanticMirResourceV1) -> Option<&mut u64> {
        match resource {
            SemanticMirResourceV1::Types => Some(&mut self.types),
            SemanticMirResourceV1::Functions => Some(&mut self.functions),
            SemanticMirResourceV1::Callables => Some(&mut self.callables),
            SemanticMirResourceV1::Locals => Some(&mut self.locals),
            SemanticMirResourceV1::Blocks => Some(&mut self.blocks),
            SemanticMirResourceV1::Statements => Some(&mut self.statements),
            SemanticMirResourceV1::Projections => Some(&mut self.projections),
            SemanticMirResourceV1::Operands => Some(&mut self.operands),
            SemanticMirResourceV1::CallArguments => Some(&mut self.call_arguments),
            SemanticMirResourceV1::SwitchTargets => Some(&mut self.switch_targets),
            SemanticMirResourceV1::ConstantBytes => Some(&mut self.constant_bytes),
            SemanticMirResourceV1::ValidationWork => Some(&mut self.validation_work),
            SemanticMirResourceV1::Allocations
            | SemanticMirResourceV1::Statics
            | SemanticMirResourceV1::VTables
            | SemanticMirResourceV1::Roots
            | SemanticMirResourceV1::Relocations
            | SemanticMirResourceV1::LinkSymbolBytes
            | SemanticMirResourceV1::CanonicalBytes => None,
        }
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: usize,
        limits: SemanticMirLimitsV1,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let maximum = limits.limit(resource);
        let slot = self.slot_mut(resource).ok_or(
            ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                table: "construction accounting domain",
            },
        )?;
        charge_construction_total_v1(slot, resource, amount, maximum)
            .map_err(construction_resource_error_v1)
    }
}

struct BodyProducerV1<'a, 'owner, 'tcx> {
    scope_events: Vec<ScopeEventV29>,
    context_entry: Option<crate::collector::BoundContextEntryV29<'tcx>>,
    receiver_reborrow: Option<ReceiverMaterializationV1<'tcx>>,
    wrapping_shifts: Option<WrappingMaterializationV1>,
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    function: SemanticFunctionIdV1,
    type_ids: HashMap<Ty<'tcx>, SemanticTypeIdV1>,
    rust_call_tuple_argument: Option<u32>,
    locals_by_raw: Vec<&'a ProductionSemanticLocalBindingV1>,
    locals_by_semantic: Vec<Option<&'a ProductionSemanticLocalBindingV1>>,
    blocks_by_raw: Vec<&'a ProductionSemanticBlockBindingV1>,
    blocks_by_semantic: Vec<&'a ProductionSemanticBlockBindingV1>,
    direct_calls_by_raw: Vec<Option<&'a ProductionSemanticDirectCallBindingV1<'tcx>>>,
    terminal_expansions_by_raw: Vec<Option<&'a ProductionSemanticTerminalExpansionRecipeV1<'tcx>>>,
    normalized_intrinsics_by_raw:
        Vec<Option<&'a ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx>>>,
    consumed_direct_calls: Vec<bool>,
    consumed_terminal_expansions: Vec<bool>,
    consumed_normalized_intrinsics: Vec<bool>,
    slice_metadata: SliceMetadataPlanV1<'a, 'tcx>,
    owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
}

type DirectCallTableV1<'a, 'tcx> = Vec<Option<&'a ProductionSemanticDirectCallBindingV1<'tcx>>>;
type TerminalExpansionTableV1<'a, 'tcx> =
    Vec<Option<&'a ProductionSemanticTerminalExpansionRecipeV1<'tcx>>>;
type NormalizedIntrinsicTableV1<'a, 'tcx> =
    Vec<Option<&'a ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx>>>;
type CallTablesV1<'a, 'tcx> = (
    DirectCallTableV1<'a, 'tcx>,
    TerminalExpansionTableV1<'a, 'tcx>,
    NormalizedIntrinsicTableV1<'a, 'tcx>,
);
type LocalTablesV1<'a> = (
    Vec<&'a ProductionSemanticLocalBindingV1>,
    Vec<Option<&'a ProductionSemanticLocalBindingV1>>,
);

pub(crate) fn construct_production_semantic_body_v1<'a, 'owner, 'tcx>(
    mut input: ProductionSemanticBodyInputV1<'a, 'tcx>,
    owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<SemanticFunctionDeclV1, ProductionSemanticBodyErrorV1> {
    validate_export_role_v1(input.role, &input.export)?;
    owner.begin_function_commitment_v29(&input)?;
    let abi = input.abi.clone();
    let export = input.export.clone();
    let entry = input.entry;
    let context_entry = input.context_entry.take();
    let mut producer = BodyProducerV1::new(&input, owner, context_entry)?;
    let locals = producer.construct_locals()?;
    let blocks = producer.construct_blocks()?;
    producer.require_all_call_bindings_consumed()?;
    let context = producer
        .context_entry
        .take()
        .map(|context| {
            let issuer = producer
                .owner
                .callables
                .get(&context.issuer())
                .ok_or_else(|| table("context issuer callable"))?
                .semantic_callable;
            context.finish(
                input.tcx,
                issuer,
                |local| {
                    producer
                        .locals_by_raw
                        .get(local.index())
                        .map(|binding| binding.semantic_local)
                },
                |ty| {
                    normalize_type_v1(input.tcx, input.instance, ty)
                        .ok()
                        .and_then(|ty| producer.type_ids.get(&ty).copied())
                },
                |amount| {
                    producer
                        .owner
                        .charge(SemanticMirResourceV1::ValidationWork, amount)
                },
            )
        })
        .transpose()?;
    let scope_events = std::mem::take(&mut producer.scope_events);
    drop(producer);

    let ProductionSemanticFunctionIdentitiesV1 {
        identity,
        item_definition,
        monomorphization,
        generic_type_arguments,
        const_generic_arguments,
    } = input.identities;
    let mut function = SemanticFunctionDeclV1::new(
        identity,
        input.role,
        item_definition,
        monomorphization,
        generic_type_arguments,
        const_generic_arguments,
        input.source,
        abi,
        locals,
        entry,
        blocks,
    )?;
    function = match export {
        ProductionSemanticFunctionExportV1::None => function,
        ProductionSemanticFunctionExportV1::Kernel(entry) => function.with_kernel_entry(entry),
        ProductionSemanticFunctionExportV1::DeviceFfi(symbol) => {
            function.with_device_ffi_export_symbol(symbol)
        }
    };
    let commitment = owner.capture_function_commitment_v29(input.function, &function)?;
    let retained = if let Some(context) = context {
        let retained = context.bind_function(&function, |amount| {
            owner.charge(SemanticMirResourceV1::ValidationWork, amount)
        })?;
        if owner
            .context_entries
            .last()
            .is_some_and(|entry| entry.function() >= retained.function())
        {
            return Err(table("context receipt function order"));
        }
        if owner.context_entries.len() == owner.context_entries.capacity() {
            owner.charge(
                SemanticMirResourceV1::ValidationWork,
                owner.context_entries.len() + 1,
            )?;
            owner
                .context_entries
                .try_reserve(1)
                .map_err(|_| allocation(SemanticMirResourceV1::Functions))?;
        }
        Some(retained)
    } else {
        None
    };
    let prepared_scope = match &mut owner.workgroup_scopes {
        Some(scopes) => Some(scopes.prepare(input.function, scope_events, |amount| {
            owner
                .totals
                .charge(SemanticMirResourceV1::ValidationWork, amount, owner.limits)
        })?),
        None if scope_events.is_empty() => None,
        None => return Err(table("scope capture without source owner")),
    };
    let prepared = match (&mut owner.function_commitments, commitment) {
        (Some(pending), Some(commitment)) => Some(pending.prepare(commitment)?),
        (None, None) => None,
        _ => return Err(table("function commitment capture state")),
    };
    // No fallible work after either record becomes visible.
    if let Some(retained) = retained {
        owner.context_entries.push(retained);
    }
    if let Some(prepared) = prepared {
        prepared.publish();
    }
    if let Some(prepared) = prepared_scope {
        prepared.publish();
    }
    Ok(function)
}

impl<'a, 'owner, 'tcx> BodyProducerV1<'a, 'owner, 'tcx> {
    fn new(
        input: &'a ProductionSemanticBodyInputV1<'a, 'tcx>,
        owner: &'owner mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
        context_entry: Option<crate::collector::BoundContextEntryV29<'tcx>>,
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        let owned_callable = owner.defined_callable(input.instance)?;
        if owned_callable.index() != input.function.index() {
            return Err(table("function body owner"));
        }
        owner.charge(SemanticMirResourceV1::Functions, 1)?;
        owner.charge(SemanticMirResourceV1::Locals, input.body.local_decls.len())?;
        owner.charge(SemanticMirResourceV1::Blocks, input.body.basic_blocks.len())?;
        // This is a rustc MIR invariant, not a restriction on generic semantic MIR.
        // Recheck it at the consuming boundary so KIR lowering may rely on it.
        require_no_entry_predecessors_v1(
            START_BLOCK.index(),
            input
                .body
                .basic_blocks
                .iter_enumerated()
                .map(|(predecessor, data)| {
                    (
                        predecessor.index(),
                        data.terminator
                            .iter()
                            .flat_map(|terminator| terminator.successors())
                            .map(|successor| successor.index()),
                    )
                }),
            || owner.charge(SemanticMirResourceV1::ValidationWork, 1),
        )?;

        let type_ids = build_type_table_v1(input.tcx, input.instance, input.type_bindings, owner)?;
        let (blocks_by_raw, blocks_by_semantic) =
            build_block_tables_v1(input.body, input.block_bindings, owner)?;
        let call_tables = build_call_tables_v1(
            input.function,
            input.body.basic_blocks.len(),
            input.direct_calls,
            input.terminal_expansions,
            input.normalized_intrinsics,
            owner,
        )?;
        let mut receiver_reborrow = ReceiverMaterializationV1::derive(
            input,
            &blocks_by_raw,
            &call_tables,
            &type_ids,
            context_entry.is_some(),
            owner,
        )?;
        let wrapping_shifts = WrappingMaterializationV1::derive(
            input,
            &blocks_by_raw,
            &call_tables,
            &type_ids,
            &mut receiver_reborrow,
            owner,
        )?;
        let (locals_by_raw, locals_by_semantic) = build_local_tables_v1(
            input.body,
            input.local_bindings,
            receiver_reborrow.as_ref().map(|reborrow| reborrow.local),
            wrapping_shifts.as_ref(),
            owner,
        )?;
        if let Some(context) = &context_entry {
            owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
            context
                .validate_body(
                    input.instance,
                    input.function,
                    input.body,
                    |raw| locals_by_raw.get(raw).map(|binding| binding.semantic_local),
                    |raw| blocks_by_raw.get(raw).map(|binding| binding.semantic_block),
                )
                .map_err(table)?;
        }
        if blocks_by_raw
            .get(START_BLOCK.index())
            .map(|binding| binding.semantic_block)
            != Some(input.entry)
        {
            return Err(table("entry block binding"));
        }
        let (direct_calls_by_raw, terminal_expansions_by_raw, normalized_intrinsics_by_raw) =
            call_tables;
        let consumed_direct_calls = try_filled_vec_v1(
            input.body.basic_blocks.len(),
            false,
            SemanticMirResourceV1::Blocks,
        )?;
        let consumed_terminal_expansions = try_filled_vec_v1(
            input.body.basic_blocks.len(),
            false,
            SemanticMirResourceV1::Blocks,
        )?;
        let consumed_normalized_intrinsics = try_filled_vec_v1(
            input.body.basic_blocks.len(),
            false,
            SemanticMirResourceV1::Blocks,
        )?;
        let slice_metadata =
            SliceMetadataPlanV1::derive(input.tcx, input.instance, input.body, |amount| {
                owner.charge(SemanticMirResourceV1::ValidationWork, amount)
            })
            .map_err(|error| match error {
                SliceMetadataErrorV1::Resource(error) => error,
                SliceMetadataErrorV1::Allocation => ProductionSemanticBodyErrorV1::Allocation {
                    resource: SemanticMirResourceV1::Locals,
                },
                SliceMetadataErrorV1::Unsupported(location) => unsupported(
                    "fake raw pointer outside the exact shared-slice metadata pair",
                    Some(location.block.index() as u32),
                    Some(location.statement_index as u32),
                ),
            })?;
        let mut producer = Self {
            scope_events: Vec::new(),
            context_entry,
            receiver_reborrow,
            wrapping_shifts,
            tcx: input.tcx,
            instance: input.instance,
            body: input.body,
            function: input.function,
            type_ids,
            rust_call_tuple_argument: None,
            locals_by_raw,
            locals_by_semantic,
            blocks_by_raw,
            blocks_by_semantic,
            direct_calls_by_raw,
            terminal_expansions_by_raw,
            normalized_intrinsics_by_raw,
            consumed_direct_calls,
            consumed_terminal_expansions,
            consumed_normalized_intrinsics,
            slice_metadata,
            owner,
        };
        producer.validate_argument_locals(&input.abi)?;
        Ok(producer)
    }

    fn validate_argument_locals(
        &mut self,
        abi: &SemanticFunctionAbiV1,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let signature = crate::rustc_semantic_plan_v1::source_signature_v1(self.tcx, self.instance)
            .map_err(|detail| unsupported(detail, None, None))?;
        if signature.inputs().len() != abi.source_input_types().len()
            || self.type_id(signature.output(), None, None)? != abi.source_output_type()
            || normalize_type_v1(self.tcx, self.instance, self.body.return_ty())
                .map_err(|_| table("return local normalization"))?
                != signature.output()
        {
            return Err(table("body source signature"));
        }
        for (source, semantic) in signature.inputs().iter().zip(abi.source_input_types()) {
            if self.type_id(*source, None, None)? != *semantic {
                return Err(table("body source argument type"));
            }
        }
        if (signature.abi == rustc_abi::ExternAbi::RustCall)
            != (abi.extern_abi() == fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall)
        {
            return Err(table("body RustCall ABI disagreement"));
        }
        let expanded =
            self.tcx.def_kind(self.instance.def_id()) == rustc_hir::def::DefKind::Closure;
        let (prefix, fields) = if signature.abi == rustc_abi::ExternAbi::RustCall {
            let (tuple, prefix) = signature
                .inputs()
                .split_last()
                .ok_or_else(|| table("RustCall source tuple"))?;
            let TyKind::Tuple(fields) = tuple.kind() else {
                return Err(table("RustCall source tuple"));
            };
            if abi.extern_abi() != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall
                || abi.fixed_count() as usize != prefix.len()
                || self.body.spread_arg.map(|local| local.index())
                    != if expanded {
                        None
                    } else {
                        Some(signature.inputs().len())
                    }
            {
                return Err(table("RustCall body argument form"));
            }
            if expanded {
                (prefix, Some(*fields))
            } else {
                (signature.inputs(), None)
            }
        } else {
            if expanded || self.body.spread_arg.is_some() {
                return Err(table("ordinary body argument form"));
            }
            (signature.inputs(), None)
        };
        let count = prefix
            .len()
            .checked_add(fields.map_or(0, |fields| fields.len()))
            .ok_or_else(|| table("body argument count"))?;
        if self.body.arg_count != count {
            return Err(table("body argument count"));
        }
        for (index, expected) in prefix
            .iter()
            .copied()
            .chain(fields.into_iter().flat_map(|fields| fields.iter()))
            .enumerate()
        {
            self.work()?;
            let raw = self
                .body
                .local_decls
                .get(rustc_middle::mir::Local::from_usize(index + 1))
                .ok_or_else(|| table("body argument local"))?;
            if normalize_type_v1(self.tcx, self.instance, raw.ty)
                .map_err(|_| table("argument local normalization"))?
                != expected
            {
                return Err(table("body argument local type"));
            }
        }
        if expanded {
            self.rust_call_tuple_argument =
                Some(u32::try_from(prefix.len()).map_err(|_| table("RustCall tuple ordinal"))?);
        }
        Ok(())
    }

    fn construct_locals(
        &mut self,
    ) -> Result<Vec<SemanticLocalDeclV1>, ProductionSemanticBodyErrorV1> {
        let mut locals = try_vec_v1(self.locals_by_semantic.len(), SemanticMirResourceV1::Locals)?;
        for index in 0..self.locals_by_semantic.len() {
            self.work()?;
            let Some(binding) = self.locals_by_semantic[index] else {
                if let Some(local) = self
                    .wrapping_shifts
                    .as_ref()
                    .and_then(|shifts| shifts.local(index))
                {
                    locals.push(SemanticLocalDeclV1::new(
                        local.local.identity,
                        local.ty,
                        SemanticLocalRoleV1::Temporary,
                        local.source,
                    ));
                    continue;
                }
                let reborrow = self
                    .receiver_reborrow
                    .as_ref()
                    .filter(|reborrow| reborrow.local.local.index() as usize == index)
                    .ok_or_else(|| table("shared receiver local slot"))?;
                locals.push(SemanticLocalDeclV1::new(
                    reborrow.local.identity,
                    reborrow.ty,
                    SemanticLocalRoleV1::Temporary,
                    self.blocks_by_raw[reborrow.observation.block() as usize].terminator_source,
                ));
                continue;
            };
            let raw = usize::try_from(binding.rustc_local).map_err(|_| table("local table"))?;
            let declaration = self
                .body
                .local_decls
                .get(rustc_middle::mir::Local::from_usize(raw))
                .ok_or_else(|| table("local table"))?;
            let ty = self.type_id(declaration.ty, None, None)?;
            locals.push(SemanticLocalDeclV1::new(
                binding.identity,
                ty,
                semantic_local_role_v1(
                    binding.rustc_local,
                    self.body.arg_count,
                    self.rust_call_tuple_argument,
                )?,
                binding.source,
            ));
        }
        Ok(locals)
    }

    fn construct_blocks(
        &mut self,
    ) -> Result<Vec<SemanticBasicBlockV1>, ProductionSemanticBodyErrorV1> {
        let mut blocks = try_vec_v1(self.blocks_by_semantic.len(), SemanticMirResourceV1::Blocks)?;
        for index in 0..self.blocks_by_semantic.len() {
            let binding = self.blocks_by_semantic[index];
            let raw_block = binding.rustc_block;
            let raw_index = usize::try_from(raw_block).map_err(|_| table("block table"))?;
            let data = self
                .body
                .basic_blocks
                .get(rustc_middle::mir::BasicBlock::from_usize(raw_index))
                .ok_or_else(|| table("block table"))?;
            if binding.statement_sources.len() != data.statements.len() {
                return Err(table("statement source table"));
            }
            let normalized_call = self
                .normalized_intrinsics_by_raw
                .get(raw_index)
                .copied()
                .flatten();
            let semantic_statement_count = data
                .statements
                .len()
                .checked_add(normalized_call.map_or(0, |call| call.operation.statement_count()))
                .and_then(|count| {
                    count.checked_add(usize::from(
                        self.receiver_reborrow
                            .as_ref()
                            .is_some_and(|reborrow| reborrow.observation.block() == raw_block),
                    ))
                })
                .ok_or_else(|| table("statement source table"))?;
            self.owner
                .charge(SemanticMirResourceV1::Statements, semantic_statement_count)?;
            let mut statements =
                try_vec_v1(semantic_statement_count, SemanticMirResourceV1::Statements)?;
            for (statement_index, statement) in data.statements.iter().enumerate() {
                self.work()?;
                let index =
                    u32::try_from(statement_index).map_err(|_| table("statement source table"))?;
                let source = *binding
                    .statement_sources
                    .get(statement_index)
                    .ok_or_else(|| table("statement source table"))?;
                let kind = self.construct_statement(raw_block, index, &statement.kind)?;
                statements.push(SemanticStatementV1::new(source, kind));
            }
            let terminator = data.terminator.as_ref().ok_or_else(|| {
                unsupported("basic block without a terminator", Some(raw_block), None)
            })?;
            self.work()?;
            let kind = if normalized_call
                .is_some_and(|call| matches!(call.operation, NormalizedCallV1::SafeCoreShift(_)))
            {
                self.construct_wrapping_shift(
                    raw_block,
                    &terminator.kind,
                    binding.terminator_source,
                    &mut statements,
                )?
            } else if normalized_call.is_some_and(|call| {
                matches!(call.operation, NormalizedCallV1::CheckedPrimitiveFrom(_))
            }) {
                let (statement, terminator) =
                    self.construct_primitive_from(raw_block, &terminator.kind)?;
                statements.push(SemanticStatementV1::new(
                    binding.terminator_source,
                    statement,
                ));
                terminator
            } else if normalized_call.is_some() {
                let (statement, terminator) =
                    self.construct_normalized_intrinsic(raw_block, &terminator.kind)?;
                statements.push(SemanticStatementV1::new(
                    binding.terminator_source,
                    statement,
                ));
                terminator
            } else {
                self.construct_terminator(raw_block, &terminator.kind, &mut statements)?
            };
            if let Some(scopes) = &self.owner.workgroup_scopes {
                scopes.capture(
                    self.function,
                    binding.semantic_block,
                    statements.len(),
                    &kind,
                    &mut self.scope_events,
                    |amount| {
                        self.owner.totals.charge(
                            SemanticMirResourceV1::ValidationWork,
                            amount,
                            self.owner.limits,
                        )
                    },
                )?;
            }
            blocks.push(SemanticBasicBlockV1::new(
                binding.identity,
                binding.source,
                statements,
                SemanticTerminatorV1::new(binding.terminator_source, kind),
            )?);
        }
        Ok(blocks)
    }

    fn construct_statement(
        &mut self,
        block: u32,
        statement: u32,
        kind: &StatementKind<'tcx>,
    ) -> Result<SemanticStatementKindV1, ProductionSemanticBodyErrorV1> {
        let site = (Some(block), Some(statement));
        self.owner.charge(
            SemanticMirResourceV1::ValidationWork,
            self.slice_metadata.lookup_work(),
        )?;
        let rewrite = self.slice_metadata.at(rustc_middle::mir::Location {
            block: rustc_middle::mir::BasicBlock::from_u32(block),
            statement_index: statement as usize,
        });
        if rewrite == Some(SliceMetadataRewriteV1::ElideTemporary) {
            return Ok(SemanticStatementKindV1::Nop);
        }
        match kind {
            StatementKind::Assign(assignment) => {
                let (destination, value) = &**assignment;
                let destination = self.construct_place(*destination, site.0, site.1)?;
                let value = if let Some(SliceMetadataRewriteV1::ReadLength(slice)) = rewrite {
                    SemanticRvalueV1::new(
                        self.type_id(self.tcx.types.usize, site.0, site.1)?,
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: self.construct_operand(
                                &Operand::Copy(Place::from(slice)),
                                site.0,
                                site.1,
                            )?,
                        },
                    )
                } else {
                    self.construct_rvalue(value, site.0, site.1)?
                };
                Ok(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    value,
                )))
            }
            StatementKind::StorageLive(local) => Ok(SemanticStatementKindV1::StorageLive(
                self.local_id(local.index())?,
            )),
            StatementKind::StorageDead(local) => Ok(SemanticStatementKindV1::StorageDead(
                self.local_id(local.index())?,
            )),
            StatementKind::SetDiscriminant {
                place,
                variant_index,
            } => Ok(SemanticStatementKindV1::SetDiscriminant {
                place: self.construct_place(**place, site.0, site.1)?,
                variant_index: u32::try_from(variant_index.index())
                    .map_err(|_| table("enum variant index"))?,
            }),
            StatementKind::Nop => Ok(SemanticStatementKindV1::Nop),
            StatementKind::FakeRead(..) => Err(unsupported("FakeRead statement", site.0, site.1)),
            StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
                NonDivergingIntrinsic::Assume(condition) => Ok(SemanticStatementKindV1::Assume(
                    self.construct_operand(condition, site.0, site.1)?,
                )),
                NonDivergingIntrinsic::CopyNonOverlapping(_) => Err(unsupported(
                    "copy_nonoverlapping intrinsic statement",
                    site.0,
                    site.1,
                )),
            },
            StatementKind::Retag(..) => Err(unsupported("Retag statement", site.0, site.1)),
            StatementKind::PlaceMention(..) => {
                Err(unsupported("PlaceMention statement", site.0, site.1))
            }
            StatementKind::AscribeUserType(..) => {
                Err(unsupported("AscribeUserType statement", site.0, site.1))
            }
            StatementKind::Coverage(..) => Err(unsupported("Coverage statement", site.0, site.1)),
            StatementKind::ConstEvalCounter => {
                Err(unsupported("ConstEvalCounter statement", site.0, site.1))
            }
            StatementKind::BackwardIncompatibleDropHint { .. } => Err(unsupported(
                "BackwardIncompatibleDropHint statement",
                site.0,
                site.1,
            )),
        }
    }

    fn construct_rvalue(
        &mut self,
        value: &Rvalue<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticRvalueV1, ProductionSemanticBodyErrorV1> {
        let result_type =
            self.type_id(value.ty(&self.body.local_decls, self.tcx), block, statement)?;
        let kind = match value {
            Rvalue::Use(operand) => {
                SemanticRvalueKindV1::Use(self.construct_operand(operand, block, statement)?)
            }
            Rvalue::Ref(_, borrow, place) => {
                let kind = semantic_borrow_kind_v1(*borrow, block, statement)?;
                SemanticRvalueKindV1::Borrow {
                    kind,
                    place: self.construct_place(*place, block, statement)?,
                }
            }
            Rvalue::Discriminant(place) => {
                SemanticRvalueKindV1::Discriminant(self.construct_place(*place, block, statement)?)
            }
            Rvalue::CopyForDeref(place) => SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                self.construct_place(*place, block, statement)?,
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            Rvalue::Aggregate(kind, operands) => {
                let aggregate_kind = match &**kind {
                    AggregateKind::Array(_) => SemanticAggregateKindV1::Array,
                    AggregateKind::Tuple => SemanticAggregateKindV1::Tuple,
                    AggregateKind::Adt(definition, variant, ..) => {
                        let definition = self.tcx.adt_def(*definition);
                        if definition.is_union() {
                            return Err(unsupported("union aggregate rvalue", block, statement));
                        }
                        if definition.is_enum() {
                            SemanticAggregateKindV1::EnumVariant(
                                u32::try_from(variant.index())
                                    .map_err(|_| table("enum variant index"))?,
                            )
                        } else {
                            SemanticAggregateKindV1::Aggregate
                        }
                    }
                    AggregateKind::Closure(..) => SemanticAggregateKindV1::Aggregate,
                    AggregateKind::CoroutineClosure(..) => {
                        return Err(unsupported(
                            "coroutine-closure aggregate rvalue",
                            block,
                            statement,
                        ));
                    }
                    AggregateKind::Coroutine(..) => {
                        return Err(unsupported("coroutine aggregate rvalue", block, statement));
                    }
                    AggregateKind::RawPtr(..) => {
                        return Err(unsupported(
                            "raw-pointer aggregate rvalue",
                            block,
                            statement,
                        ));
                    }
                };
                let mut semantic_operands =
                    try_vec_v1(operands.len(), SemanticMirResourceV1::Operands)?;
                for operand in operands {
                    semantic_operands.push(self.construct_operand(operand, block, statement)?);
                }
                SemanticRvalueKindV1::aggregate(aggregate_kind, semantic_operands)?
            }
            Rvalue::Repeat(operand, count) => {
                let count = count
                    .try_to_target_usize(self.tcx)
                    .and_then(|count| usize::try_from(count).ok())
                    .ok_or_else(|| {
                        unsupported("Repeat count outside host bounds", block, statement)
                    })?;
                let mut semantic_operands = try_vec_v1(count, SemanticMirResourceV1::Operands)?;
                if count != 0 {
                    let operand = self.construct_operand(operand, block, statement)?;
                    self.owner
                        .charge(SemanticMirResourceV1::Operands, count - 1)?;
                    semantic_operands.resize(count, operand);
                }
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Array, semantic_operands)?
            }
            Rvalue::RawPtr(kind, place) => SemanticRvalueKindV1::AddressOf {
                mutability: match kind {
                    RawPtrKind::Const => SemanticMutabilityV1::Immutable,
                    RawPtrKind::Mut => SemanticMutabilityV1::Mutable,
                    RawPtrKind::FakeForPtrMetadata => {
                        return Err(unsupported(
                            "fake raw pointer for metadata rvalue",
                            block,
                            statement,
                        ));
                    }
                },
                place: self.construct_place(*place, block, statement)?,
            },
            Rvalue::Cast(kind, operand, _) => SemanticRvalueKindV1::Cast {
                kind: match kind {
                    CastKind::IntToInt | CastKind::FloatToInt => SemanticCastKindV1::Integer,
                    CastKind::IntToFloat | CastKind::FloatToFloat => SemanticCastKindV1::Float,
                    CastKind::PtrToPtr | CastKind::FnPtrToPtr => SemanticCastKindV1::Pointer,
                    CastKind::PointerExposeProvenance => {
                        SemanticCastKindV1::PointerExposeProvenance
                    }
                    CastKind::PointerWithExposedProvenance => {
                        SemanticCastKindV1::PointerWithExposedProvenance
                    }
                    CastKind::Transmute => SemanticCastKindV1::Transmute,
                    CastKind::PointerCoercion(..) => {
                        return Err(unsupported(
                            "unsupported PointerCoercion Cast rvalue",
                            block,
                            statement,
                        ));
                    }
                    CastKind::Subtype => {
                        return Err(unsupported(
                            "unsupported Subtype Cast rvalue",
                            block,
                            statement,
                        ));
                    }
                },
                operand: self.construct_operand(operand, block, statement)?,
            },
            Rvalue::BinaryOp(operation, operands) => {
                let left = self.construct_operand(&operands.0, block, statement)?;
                let right = self.construct_operand(&operands.1, block, statement)?;
                if let Some(operation) = semantic_checked_binary_operation(*operation) {
                    SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                        operation, left, right,
                    ))
                } else if let Some(operation) = semantic_unchecked_binary_operation(*operation) {
                    SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                        operation, left, right,
                    ))
                } else {
                    let operation = semantic_binary_operation(*operation).ok_or_else(|| {
                        unsupported(
                            format!("unsupported BinaryOp rvalue {operation:?}"),
                            block,
                            statement,
                        )
                    })?;
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    }
                }
            }
            Rvalue::UnaryOp(operation, operand) => SemanticRvalueKindV1::Unary {
                operation: match operation {
                    UnOp::Not => SemanticUnaryOpV1::Not,
                    UnOp::Neg => SemanticUnaryOpV1::Negate,
                    UnOp::PtrMetadata => SemanticUnaryOpV1::PointerMetadata,
                },
                operand: self.construct_operand(operand, block, statement)?,
            },
            Rvalue::ThreadLocalRef(..) => {
                return Err(unsupported("ThreadLocalRef rvalue", block, statement));
            }
            Rvalue::WrapUnsafeBinder(..) => {
                return Err(unsupported("WrapUnsafeBinder rvalue", block, statement));
            }
        };
        Ok(SemanticRvalueV1::new(result_type, kind))
    }

    fn construct_terminator(
        &mut self,
        raw_block: u32,
        terminator: &TerminatorKind<'tcx>,
        statements: &mut Vec<SemanticStatementV1>,
    ) -> Result<SemanticTerminatorKindV1, ProductionSemanticBodyErrorV1> {
        let block = Some(raw_block);
        match terminator {
            TerminatorKind::Return => Ok(SemanticTerminatorKindV1::Return),
            TerminatorKind::Unreachable => Ok(SemanticTerminatorKindV1::Unreachable),
            TerminatorKind::Goto { target } => Ok(SemanticTerminatorKindV1::Goto(
                self.edge(SemanticEdgeRoleV1::Goto, target.index())?,
            )),
            TerminatorKind::SwitchInt { discr, targets } => {
                self.owner
                    .charge(SemanticMirResourceV1::SwitchTargets, targets.iter().count())?;
                let discriminant = self.construct_operand(discr, block, None)?;
                let mut values =
                    try_vec_v1(targets.iter().count(), SemanticMirResourceV1::SwitchTargets)?;
                for (value, target) in targets.iter() {
                    values.push(SemanticSwitchTargetV1::new(
                        value,
                        self.edge(SemanticEdgeRoleV1::SwitchValue, target.index())?,
                    ));
                }
                values.sort_unstable_by_key(|target| target.value());
                let otherwise = self.edge(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    targets.otherwise().index(),
                )?;
                Ok(SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets: SemanticSwitchTargetsV1::new(values, otherwise)?,
                })
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
                ..
            } => {
                self.owner
                    .charge(SemanticMirResourceV1::CallArguments, args.len())?;
                let resolved = resolve_direct_call_v1(self.tcx, self.instance, self.body, func)
                    .map_err(|construct| unsupported(construct, block, None))?;
                let semantic_callee = self.resolve_call_binding(raw_block, resolved, args.len())?;
                let receiver = self.consume_receiver_reborrow(raw_block, resolved)?;
                let replacement = if let Some((statement, operand)) = receiver {
                    statements.push(SemanticStatementV1::new(
                        self.blocks_by_raw[raw_block as usize].terminator_source,
                        statement,
                    ));
                    Some(operand)
                } else {
                    None
                };
                let restored = if let Some(context) = &mut self.context_entry {
                    self.owner
                        .charge(SemanticMirResourceV1::ValidationWork, 1)?;
                    context
                        .consume_call(self.body, raw_block as usize, resolved, args.len())
                        .map_err(table)?
                } else {
                    None
                };
                let mut arguments = try_vec_v1(args.len(), SemanticMirResourceV1::CallArguments)?;
                for (ordinal, argument) in args.iter().enumerate() {
                    if ordinal == 0
                        && let Some(operand) = &replacement
                    {
                        self.owner.charge(SemanticMirResourceV1::Operands, 1)?;
                        self.work()?;
                        arguments.push(operand.clone());
                        continue;
                    }
                    let restored_operand;
                    let operand = if ordinal == 0
                        && let Some(local) = restored
                    {
                        restored_operand = Operand::Move(Place::from(local));
                        &restored_operand
                    } else {
                        &argument.node
                    };
                    arguments.push(self.construct_operand(operand, block, None)?);
                }
                let destination = if let Some(target) = target {
                    Some(SemanticCallDestinationV1::new(
                        self.construct_place(*destination, block, None)?,
                        self.edge(SemanticEdgeRoleV1::CallReturn, target.index())?,
                    ))
                } else {
                    None
                };
                let unwind = match unwind {
                    UnwindAction::Continue => SemanticUnwindActionV1::Continue,
                    UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
                    UnwindAction::Terminate(_) | UnwindAction::Cleanup(_) => {
                        return Err(unsupported("call with executable unwind edge", block, None));
                    }
                };
                let source = self
                    .owner
                    .inline_sources
                    .take(self.function, raw_block, semantic_callee)
                    .map_err(table)?;
                let requires_source = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        matches!(
                            recipe.expansion,
                            ProductionTerminalExpansionV1::Gfx942InlineU32(_)
                        )
                    });
                if requires_source != source.is_some() {
                    return Err(table("assembly call occurrence source binding"));
                }
                let requires_region = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32
                    });
                let registers = if requires_region {
                    if args.len() != 8 {
                        return Err(table("ordered region actual argument cardinality"));
                    }
                    Some(
                        crate::production_ordered_region_v31::actual_registers(
                            self.tcx,
                            [
                                &args[3].node,
                                &args[4].node,
                                &args[5].node,
                                &args[6].node,
                                &args[7].node,
                            ],
                        )
                        .map_err(table)?,
                    )
                } else {
                    None
                };
                let region_source = self
                    .owner
                    .ordered_sources
                    .take(self.function, raw_block, semantic_callee, registers)
                    .map_err(table)?;
                if requires_region != region_source.is_some() {
                    return Err(table("ordered region call occurrence source binding"));
                }
                let requires_program = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32
                    });
                let observed_program = if requires_program {
                    if args.len() != 8
                        || !destination
                            .as_ref()
                            .is_some_and(|destination| destination.place().projections().is_empty())
                    {
                        return Err(table(
                            "ordered program actual argument or destination shape",
                        ));
                    }
                    Some(
                        crate::production_ordered_program_v32::observe_call(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            [
                                &args[0].node,
                                &args[1].node,
                                &args[2].node,
                                &args[3].node,
                                &args[4].node,
                                &args[5].node,
                                &args[6].node,
                                &args[7].node,
                            ],
                        )
                        .map_err(table)?,
                    )
                } else {
                    None
                };
                let program_source = self
                    .owner
                    .program_sources
                    .take(
                        crate::production_ordered_composition_source_v1::ImportedOrderedCallV1 {
                            function: self.function,
                            raw_block,
                            block: self.blocks_by_raw[raw_block as usize].semantic_block,
                            block_identity: self.blocks_by_raw[raw_block as usize].identity,
                            caller: self.instance,
                            callee: resolved,
                            callable: semantic_callee,
                            marker: observed_program,
                            arguments: &arguments,
                            destination: destination.as_ref(),
                            unwind,
                        },
                    )
                    .map_err(table)?;
                if requires_program != program_source.is_some() {
                    return Err(table("ordered program call occurrence source binding"));
                }
                let mut call = SemanticDirectCallV1::new_callable(
                    semantic_callee,
                    arguments,
                    destination,
                    unwind,
                )?;
                self.owner
                    .bf16_inspection
                    .observe(
                        self.instance,
                        self.body,
                        self.function,
                        raw_block,
                        self.blocks_by_raw[raw_block as usize].semantic_block,
                        self.blocks_by_raw[raw_block as usize].identity,
                        resolved,
                        self.terminal_expansions_by_raw
                            .get(raw_block as usize)
                            .copied()
                            .flatten()
                            .map(|recipe| recipe.expansion),
                        &call,
                    )
                    .map_err(table)?;
                self.owner
                    .bf16_tile_values
                    .observe(
                        self.instance,
                        self.body,
                        self.function,
                        raw_block,
                        self.blocks_by_raw[raw_block as usize].semantic_block,
                        self.blocks_by_raw[raw_block as usize].identity,
                        resolved,
                        self.terminal_expansions_by_raw
                            .get(raw_block as usize)
                            .copied()
                            .flatten()
                            .map(|recipe| recipe.expansion),
                        &call,
                    )
                    .map_err(table)?;
                if let Some(source) = source {
                    call = call.with_inline_assembly_source_v30(source);
                }
                if let Some(source) = region_source {
                    call = call.with_ordered_region_source_v31(source);
                }
                if let Some(source) = program_source {
                    call = call.with_ordered_program_source_v32(source);
                }

                let requires_physical = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        crate::production_physical_entry_call_v37::is_physical(recipe.expansion)
                    });
                if requires_physical {
                    let actual = match args.len() {
                        0 => crate::production_physical_entry_call_v37::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[],
                        ),
                        5 => crate::production_physical_entry_call_v37::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[
                                &args[0].node,
                                &args[1].node,
                                &args[2].node,
                                &args[3].node,
                                &args[4].node,
                            ],
                        ),
                        _ => return Err(table("physical-entry actual marker runtime arity")),
                    }
                    .map_err(table)?;
                    let semantic_block = self.block_id(raw_block as usize)?;
                    let annotation = self
                        .owner
                        .physical_entry_annotations
                        .as_mut()
                        .ok_or_else(|| table("physical-entry source annotation absent"))?;
                    call = annotation
                        .attach(
                            self.instance,
                            (self.function, raw_block, semantic_callee),
                            semantic_block,
                            actual,
                            call,
                        )
                        .map_err(table)?;
                }
                let requires_lds_exchange = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        crate::production_physical_lds_exchange_call_v39::is_physical(
                            recipe.expansion,
                        )
                    });
                if requires_lds_exchange {
                    let actual = match args.len() {
                        0 => crate::production_physical_lds_exchange_call_v39::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[],
                        ),
                        2 => crate::production_physical_lds_exchange_call_v39::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[&args[0].node, &args[1].node],
                        ),
                        _ => {
                            return Err(table("physical-lds-exchange actual marker runtime arity"));
                        }
                    }
                    .map_err(table)?;
                    let semantic_block = self.block_id(raw_block as usize)?;
                    let annotation = self
                        .owner
                        .physical_lds_exchange_annotations
                        .as_mut()
                        .ok_or_else(|| table("physical-lds-exchange source annotation absent"))?;
                    call = annotation
                        .attach(
                            self.instance,
                            (self.function, raw_block, semantic_callee),
                            semantic_block,
                            actual,
                            call,
                        )
                        .map_err(table)?;
                }
                let requires_global_copy = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        crate::production_physical_global_copy_call_v38::is_physical(
                            recipe.expansion,
                        )
                    });
                if requires_global_copy {
                    let actual = match args.len() {
                        0 => crate::production_physical_global_copy_call_v38::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[],
                        ),
                        2 => crate::production_physical_global_copy_call_v38::observe(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            &[&args[0].node, &args[1].node],
                        ),
                        _ => return Err(table("physical-global-copy actual marker runtime arity")),
                    }
                    .map_err(table)?;
                    let semantic_block = self.block_id(raw_block as usize)?;
                    let annotation = self
                        .owner
                        .physical_global_copy_annotations
                        .as_mut()
                        .ok_or_else(|| table("physical-global-copy source annotation absent"))?;
                    call = annotation
                        .attach(
                            self.instance,
                            (self.function, raw_block, semantic_callee),
                            semantic_block,
                            actual,
                            call,
                        )
                        .map_err(table)?;
                }
                let requires_complete_body = self
                    .terminal_expansions_by_raw
                    .get(raw_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|recipe| {
                        recipe.expansion == ProductionTerminalExpansionV1::Gfx942CompleteBodyE32
                    });
                if requires_complete_body {
                    if args.len() != 10 {
                        return Err(table("complete-body actual ten arguments"));
                    }
                    let actual =
                        crate::production_complete_body_call_vnext::observe_complete_body_call(
                            self.tcx,
                            self.instance,
                            self.body,
                            func,
                            [
                                &args[0].node,
                                &args[1].node,
                                &args[2].node,
                                &args[3].node,
                                &args[4].node,
                                &args[5].node,
                                &args[6].node,
                                &args[7].node,
                                &args[8].node,
                                &args[9].node,
                            ],
                        )
                        .map_err(table)?;
                    let semantic_block = self.block_id(raw_block as usize)?;
                    let annotation = self
                        .owner
                        .complete_body_annotation
                        .as_mut()
                        .ok_or_else(|| table("complete-body source annotation absent"))?;
                    call = annotation
                        .attach(
                            self.instance,
                            (self.function, raw_block, semantic_callee),
                            semantic_block,
                            &actual,
                            call,
                        )
                        .map_err(table)?;
                }
                Ok(SemanticTerminatorKindV1::Call(call))
            }
            TerminatorKind::TailCall { .. } => Err(unsupported("TailCall terminator", block, None)),
            TerminatorKind::Drop {
                place,
                target,
                unwind,
                ..
            } => {
                if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
                    return Err(unsupported("drop with executable unwind edge", block, None));
                }
                match classify_rustc_drop_v1(self.tcx, self.instance, self.body, *place) {
                    Ok(ProductionRustcDropClassV1::Trivial) => {}
                    Ok(ProductionRustcDropClassV1::RequiresDropGlue) => {
                        return Err(unsupported(
                            "Drop terminator requiring drop glue",
                            block,
                            None,
                        ));
                    }
                    Err(_) => {
                        return Err(unsupported(
                            "Drop place type that failed monomorphic normalization",
                            block,
                            None,
                        ));
                    }
                }
                let _place = self.construct_place(*place, block, None)?;
                Ok(SemanticTerminatorKindV1::Goto(
                    self.edge(SemanticEdgeRoleV1::Goto, target.index())?,
                ))
            }
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                unwind,
            } => {
                let message = match msg.as_ref() {
                    AssertKind::BoundsCheck { len, index } => {
                        SemanticAssertMessageV1::BoundsCheck {
                            length: self.construct_operand(len, block, None)?,
                            index: self.construct_operand(index, block, None)?,
                        }
                    }
                    AssertKind::Overflow(operation, left, right) => {
                        let operation = semantic_binary_operation(*operation).ok_or_else(|| {
                            unsupported("unsupported overflow Assert operation", block, None)
                        })?;
                        SemanticAssertMessageV1::Overflow {
                            operation,
                            left: self.construct_operand(left, block, None)?,
                            right: self.construct_operand(right, block, None)?,
                        }
                    }
                    AssertKind::DivisionByZero(operand) => SemanticAssertMessageV1::DivisionByZero(
                        self.construct_operand(operand, block, None)?,
                    ),
                    AssertKind::RemainderByZero(operand) => {
                        SemanticAssertMessageV1::RemainderByZero(
                            self.construct_operand(operand, block, None)?,
                        )
                    }
                    AssertKind::MisalignedPointerDereference { required, found } => {
                        SemanticAssertMessageV1::MisalignedPointerDereference {
                            required_alignment: self.construct_operand(required, block, None)?,
                            found_alignment: self.construct_operand(found, block, None)?,
                        }
                    }
                    AssertKind::NullPointerDereference => {
                        SemanticAssertMessageV1::NullPointerDereference
                    }
                    AssertKind::ResumedAfterReturn(_) => {
                        SemanticAssertMessageV1::ResumedAfterReturn
                    }
                    AssertKind::ResumedAfterPanic(_) => SemanticAssertMessageV1::ResumedAfterPanic,
                    AssertKind::OverflowNeg(_)
                    | AssertKind::InvalidEnumConstruction(_)
                    | AssertKind::ResumedAfterDrop(_) => {
                        return Err(unsupported("unsupported Assert terminator", block, None));
                    }
                };
                let unwind = match unwind {
                    UnwindAction::Continue => SemanticUnwindActionV1::Continue,
                    UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
                    UnwindAction::Terminate(_) | UnwindAction::Cleanup(_) => {
                        return Err(unsupported(
                            "assert with executable unwind edge",
                            block,
                            None,
                        ));
                    }
                };
                Ok(SemanticTerminatorKindV1::Assert {
                    condition: self.construct_operand(cond, block, None)?,
                    expected: *expected,
                    message,
                    target: self.edge(SemanticEdgeRoleV1::AssertSuccess, target.index())?,
                    unwind,
                })
            }
            TerminatorKind::UnwindResume => {
                Err(unsupported("UnwindResume terminator", block, None))
            }
            TerminatorKind::UnwindTerminate(..) => {
                Err(unsupported("UnwindTerminate terminator", block, None))
            }
            TerminatorKind::Yield { .. } => Err(unsupported("Yield terminator", block, None)),
            TerminatorKind::CoroutineDrop => {
                Err(unsupported("CoroutineDrop terminator", block, None))
            }
            TerminatorKind::FalseEdge { .. } => {
                Err(unsupported("FalseEdge terminator", block, None))
            }
            TerminatorKind::FalseUnwind { .. } => {
                Err(unsupported("FalseUnwind terminator", block, None))
            }
            TerminatorKind::InlineAsm { .. } => {
                Err(unsupported("InlineAsm terminator", block, None))
            }
        }
    }

    fn construct_normalized_intrinsic(
        &mut self,
        raw_block: u32,
        terminator: &TerminatorKind<'tcx>,
    ) -> Result<(SemanticStatementKindV1, SemanticTerminatorKindV1), ProductionSemanticBodyErrorV1>
    {
        let block = Some(raw_block);
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = terminator
        else {
            return Err(table("normalized intrinsic table"));
        };
        if args.len() != 2 {
            return Err(unsupported(
                "normalized atomic intrinsic with unexpected call arity",
                block,
                None,
            ));
        }
        let Some(target) = *target else {
            return Err(unsupported(
                "normalized atomic intrinsic without a return edge",
                block,
                None,
            ));
        };
        if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
            return Err(unsupported(
                "normalized atomic intrinsic with executable unwind edge",
                block,
                None,
            ));
        }

        self.owner
            .charge(SemanticMirResourceV1::CallArguments, args.len())?;
        let resolved = resolve_direct_call_v1(self.tcx, self.instance, self.body, func)
            .map_err(|construct| unsupported(construct, block, None))?;
        let classification = crate::production_rustc_intrinsic_v1::classify(self.tcx, resolved)
            .map_err(|error| {
                unsupported(
                    format!("unsupported rustc compiler intrinsic: {error}"),
                    block,
                    None,
                )
            })?
            .ok_or_else(|| table("normalized intrinsic table"))?;
        let index = usize::try_from(raw_block).map_err(|_| table("normalized intrinsic table"))?;
        let recipe = self
            .normalized_intrinsics_by_raw
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| table("normalized intrinsic table"))?;
        if recipe.caller != self.function
            || recipe.expected_callee != resolved
            || recipe.expected_element_type != classification.element_type
            || !matches!(recipe.operation, NormalizedCallV1::Rustc(operation) if operation == classification.operation)
            || self.direct_calls_by_raw[index].is_some()
            || self.terminal_expansions_by_raw[index].is_some()
        {
            return Err(table("normalized intrinsic table"));
        }

        let expected = normalize_type_v1(self.tcx, self.instance, recipe.expected_element_type)
            .map_err(|_| {
                unsupported(
                    "normalized atomic element type failed monomorphic normalization",
                    block,
                    None,
                )
            })?;
        if expected != recipe.expected_element_type {
            return Err(table("normalized intrinsic element type"));
        }
        let destination_type = normalize_type_v1(
            self.tcx,
            self.instance,
            destination.ty(&self.body.local_decls, self.tcx).ty,
        )
        .map_err(|_| {
            unsupported(
                "normalized atomic destination type failed monomorphic normalization",
                block,
                None,
            )
        })?;
        let address_type = normalize_type_v1(
            self.tcx,
            self.instance,
            args[0].node.ty(&self.body.local_decls, self.tcx),
        )
        .map_err(|_| {
            unsupported(
                "normalized atomic address type failed monomorphic normalization",
                block,
                None,
            )
        })?;
        let value_type = normalize_type_v1(
            self.tcx,
            self.instance,
            args[1].node.ty(&self.body.local_decls, self.tcx),
        )
        .map_err(|_| {
            unsupported(
                "normalized atomic value type failed monomorphic normalization",
                block,
                None,
            )
        })?;
        let TyKind::RawPtr(pointee, Mutability::Mut) = *address_type.kind() else {
            return Err(unsupported(
                "normalized atomic address is not a mutable raw pointer",
                block,
                None,
            ));
        };
        if pointee != expected || value_type != expected || destination_type != expected {
            return Err(unsupported(
                "normalized atomic address, value, and destination types do not agree",
                block,
                None,
            ));
        }

        let address_operand = self.construct_operand(&args[0].node, block, None)?;
        let address_base = match address_operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
            SemanticOperandV1::Constant(_) => {
                return Err(unsupported(
                    "normalized atomic address is not a place operand",
                    block,
                    None,
                ));
            }
        };
        self.owner.charge(SemanticMirResourceV1::Projections, 1)?;
        let projection_count = address_base
            .projections()
            .len()
            .checked_add(1)
            .ok_or_else(|| table("normalized atomic address projection"))?;
        let mut projections = try_vec_v1(projection_count, SemanticMirResourceV1::Projections)?;
        projections.extend_from_slice(address_base.projections());
        let element_type = self.type_id(expected, block, None)?;
        projections.push(SemanticProjectionV1::new(
            SemanticProjectionKindV1::Dereference,
            element_type,
        )?);
        let address = SemanticPlaceV1::new(address_base.local(), projections, element_type)?;
        let destination = self.construct_place(*destination, block, None)?;
        let value = self.construct_operand(&args[1].node, block, None)?;
        let NormalizedCallV1::Rustc(operation) = recipe.operation else {
            return Err(table("normalized atomic intrinsic operation"));
        };
        let (operation, access) = operation
            .atomic_rmw()
            .ok_or_else(|| table("normalized atomic intrinsic operation"))?;

        self.consumed_normalized_intrinsics[index] = true;
        Ok((
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                destination,
                address,
                value,
                operation,
                access,
            )),
            SemanticTerminatorKindV1::Goto(self.edge(SemanticEdgeRoleV1::Goto, target.index())?),
        ))
    }

    fn construct_operand(
        &mut self,
        operand: &Operand<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticOperandV1, ProductionSemanticBodyErrorV1> {
        self.owner.charge(SemanticMirResourceV1::Operands, 1)?;
        self.work()?;
        match operand {
            Operand::Copy(place) => Ok(SemanticOperandV1::Copy(
                self.construct_place(*place, block, statement)?,
            )),
            Operand::Move(place) => Ok(SemanticOperandV1::Move(
                self.construct_place(*place, block, statement)?,
            )),
            Operand::Constant(constant) => {
                let normalized = self
                    .instance
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        self.tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(constant.const_),
                    )
                    .map_err(|_| {
                        unsupported(
                            "constant that failed monomorphic normalization",
                            block,
                            statement,
                        )
                    })?;
                let ty = self.type_id(normalized.ty(), block, statement)?;
                let value =
                    self.construct_constant_value(normalized, constant.span, block, statement)?;
                Ok(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty, value,
                )))
            }
            Operand::RuntimeChecks(..) => {
                Err(unsupported("RuntimeChecks operand", block, statement))
            }
        }
    }

    fn construct_place(
        &mut self,
        place: Place<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticPlaceV1, ProductionSemanticBodyErrorV1> {
        self.owner
            .charge(SemanticMirResourceV1::Projections, place.projection.len())?;
        let local = self.local_id(place.local.index())?;
        let local_ty = self.body.local_decls[place.local].ty;
        let mut derived = PlaceTy::from_ty(local_ty);
        let mut projections =
            try_vec_v1(place.projection.len(), SemanticMirResourceV1::Projections)?;
        for projection in place.projection {
            self.work()?;
            let kind = match projection {
                ProjectionElem::Deref => SemanticProjectionKindV1::Dereference,
                ProjectionElem::Field(field, _) => SemanticProjectionKindV1::Field(
                    u32::try_from(field.index()).map_err(|_| table("field projection"))?,
                ),
                ProjectionElem::Index(index) => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..) | TyKind::Slice(..)) {
                        return Err(unsupported(
                            "Index projection on a non-array/slice place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::Index(self.local_id(index.index())?)
                }
                ProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..)) {
                        return Err(unsupported(
                            "ConstantIndex projection on a non-array place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: min_length,
                        from_end,
                    }
                }
                ProjectionElem::Subslice { from, to, from_end } => {
                    if !matches!(derived.ty.kind(), TyKind::Array(..)) {
                        return Err(unsupported(
                            "Subslice projection on a non-array place",
                            block,
                            statement,
                        ));
                    }
                    SemanticProjectionKindV1::Subslice { from, to, from_end }
                }
                ProjectionElem::Downcast(_, variant) => SemanticProjectionKindV1::Downcast(
                    u32::try_from(variant.index()).map_err(|_| table("downcast projection"))?,
                ),
                ProjectionElem::OpaqueCast(_) => SemanticProjectionKindV1::OpaqueCast,
                ProjectionElem::UnwrapUnsafeBinder(_) => {
                    return Err(unsupported(
                        "UnwrapUnsafeBinder projection",
                        block,
                        statement,
                    ));
                }
            };
            derived = derived.projection_ty(self.tcx, projection);
            let result_type = self.type_id(derived.ty, block, statement)?;
            projections.push(SemanticProjectionV1::new(kind, result_type)?);
        }
        let ty = self.type_id(derived.ty, block, statement)?;
        Ok(SemanticPlaceV1::new(local, projections, ty)?)
    }

    fn construct_constant_value(
        &mut self,
        constant: rustc_middle::mir::Const<'tcx>,
        span: rustc_span::Span,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticConstantValueV1, ProductionSemanticBodyErrorV1> {
        if matches!(
            constant,
            rustc_middle::mir::Const::Val(ConstValue::ZeroSized, _)
        ) {
            return Ok(SemanticConstantValueV1::ZeroSized);
        }
        if let Some(value) =
            constant.try_eval_scalar_int(self.tcx, TypingEnv::fully_monomorphized())
        {
            let size_bytes = u8::try_from(value.size().bytes()).map_err(|_| {
                unsupported("scalar constant wider than 128 bits", block, statement)
            })?;
            return Ok(SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                value.to_bits(value.size()),
                size_bytes,
            )?));
        }

        let layout = LayoutCx::new(self.tcx, TypingEnv::fully_monomorphized())
            .layout_of(constant.ty())
            .map_err(|_| unsupported("constant without target layout", block, statement))?;
        let size = usize::try_from(layout.size.bytes())
            .map_err(|_| unsupported("constant byte size outside host usize", block, statement))?;
        self.owner
            .charge(SemanticMirResourceV1::ConstantBytes, size)?;
        let evaluated = constant
            .eval(self.tcx, TypingEnv::fully_monomorphized(), span)
            .map_err(|_| unsupported("constant evaluation failure", block, statement))?;
        match evaluated {
            ConstValue::ZeroSized => Ok(SemanticConstantValueV1::ZeroSized),
            ConstValue::Scalar(value) => {
                let scalar = value.try_to_scalar_int().map_err(|_| {
                    unsupported("scalar constant with pointer provenance", block, statement)
                })?;
                let size_bytes = u8::try_from(scalar.size().bytes()).map_err(|_| {
                    unsupported("scalar constant wider than 128 bits", block, statement)
                })?;
                Ok(SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                    scalar.to_bits(scalar.size()),
                    size_bytes,
                )?))
            }
            ConstValue::Slice { .. } => Err(unsupported(
                "slice constant requiring allocation provenance",
                block,
                statement,
            )),
            ConstValue::Indirect { alloc_id, offset } => {
                let GlobalAlloc::Memory(allocation) = self.tcx.global_alloc(alloc_id) else {
                    return Err(unsupported(
                        "indirect constant backed by a non-memory allocation",
                        block,
                        statement,
                    ));
                };
                let allocation = allocation.inner();
                let start = offset.bytes_usize();
                let end = start.checked_add(size).ok_or_else(|| {
                    unsupported("indirect constant range overflow", block, statement)
                })?;
                if end > allocation.len() {
                    return Err(unsupported(
                        "indirect constant outside its allocation",
                        block,
                        statement,
                    ));
                }
                let pointer_width = self.tcx.data_layout.pointer_size().bytes_usize();
                for (at, _) in allocation.provenance().ptrs().iter() {
                    self.work()?;
                    let pointer_start = at.bytes_usize();
                    let pointer_end = pointer_start.saturating_add(pointer_width);
                    if pointer_start < end && pointer_end > start {
                        return Err(unsupported(
                            "indirect constant with pointer provenance",
                            block,
                            statement,
                        ));
                    }
                }
                let raw = allocation.inspect_with_uninit_and_ptr_outside_interpreter(start..end);
                self.owner
                    .charge(SemanticMirResourceV1::ValidationWork, raw.len())?;
                let mut bytes = try_vec_v1(size, SemanticMirResourceV1::ConstantBytes)?;
                for (index, byte) in raw.iter().copied().enumerate() {
                    if !allocation
                        .init_mask()
                        .get(rustc_abi::Size::from_bytes(start + index))
                    {
                        return Err(unsupported(
                            "indirect constant with uninitialized bytes",
                            block,
                            statement,
                        ));
                    }
                    bytes.push(byte);
                }
                Ok(SemanticConstantValueV1::Bytes(
                    SemanticConstantBytesV1::new(bytes)?,
                ))
            }
        }
    }

    fn resolve_call_binding(
        &mut self,
        raw_block: u32,
        resolved: Instance<'tcx>,
        argument_count: usize,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticBodyErrorV1> {
        let index = usize::try_from(raw_block).map_err(|_| table("call binding table"))?;
        let classified =
            crate::production_semantic_terminal_v1::classify(self.tcx, resolved.def_id());
        let expansion = match classified {
            Some(
                crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1::Expand(
                    expansion,
                ),
            ) => Some(expansion),
            Some(
                crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1::Reject(_),
            ) => {
                return Err(unsupported(
                    "reviewed terminal without a production expansion",
                    Some(raw_block),
                    None,
                ));
            }
            None => match crate::production_rustc_intrinsic_v1::classify(self.tcx, resolved) {
                Ok(Some(classification))
                    if classification.operation == ProductionRustcIntrinsicOperationV1::FabsF32 =>
                {
                    Some(ProductionTerminalExpansionV1::RustcFabsF32)
                }
                Ok(Some(classification)) => match classification.operation {
                    ProductionRustcIntrinsicOperationV1::SaturatingInteger(operation) => Some(
                        ProductionTerminalExpansionV1::RustcSaturatingInteger(operation),
                    ),
                    _ => return Err(table("normalized intrinsic call binding")),
                },
                Ok(None) => None,
                Err(error) => {
                    return Err(unsupported(
                        format!("unsupported rustc compiler intrinsic: {error}"),
                        Some(raw_block),
                        None,
                    ));
                }
            },
        };
        if let Some(expansion) = expansion {
            if expansion == ProductionTerminalExpansionV1::ContextIssue
                && self.context_entry.is_none()
            {
                return Err(table("context issuer without source custody"));
            }
            let recipe = self
                .terminal_expansions_by_raw
                .get(index)
                .copied()
                .flatten()
                .ok_or_else(|| table("terminal expansion table"))?;
            if recipe.caller != self.function
                || recipe.expected_callee != resolved
                || recipe.expansion != expansion
                || terminal_argument_count_v1(expansion) != Some(argument_count)
            {
                return Err(table("terminal expansion table"));
            }
            self.consumed_terminal_expansions[index] = true;
            if self.direct_calls_by_raw[index].is_some()
                || self.normalized_intrinsics_by_raw[index].is_some()
            {
                return Err(table("call binding table"));
            }
            self.owner.terminal_callable(resolved, expansion)
        } else {
            let binding = self
                .direct_calls_by_raw
                .get(index)
                .copied()
                .flatten()
                .ok_or_else(|| table("direct-call binding table"))?;
            if binding.caller != self.function || binding.expected_callee != resolved {
                return Err(table("direct-call binding table"));
            }
            self.consumed_direct_calls[index] = true;
            if self.terminal_expansions_by_raw[index].is_some()
                || self.normalized_intrinsics_by_raw[index].is_some()
            {
                return Err(table("call binding table"));
            }
            self.owner.defined_callable(resolved)
        }
    }

    fn require_all_call_bindings_consumed(&self) -> Result<(), ProductionSemanticBodyErrorV1> {
        if self
            .receiver_reborrow
            .as_ref()
            .is_some_and(|reborrow| !reborrow.consumed)
        {
            return Err(table("unused shared receiver reborrow"));
        }
        for (index, binding) in self.direct_calls_by_raw.iter().enumerate() {
            if binding.is_some() && !self.consumed_direct_calls[index] {
                return Err(table("unused direct-call binding"));
            }
        }
        for (index, recipe) in self.terminal_expansions_by_raw.iter().enumerate() {
            if recipe.is_some() && !self.consumed_terminal_expansions[index] {
                return Err(table("unused terminal expansion recipe"));
            }
        }
        for (index, recipe) in self.normalized_intrinsics_by_raw.iter().enumerate() {
            if recipe.is_some() && !self.consumed_normalized_intrinsics[index] {
                return Err(table("unused normalized intrinsic recipe"));
            }
        }
        Ok(())
    }

    fn type_id(
        &mut self,
        raw: Ty<'tcx>,
        block: Option<u32>,
        statement: Option<u32>,
    ) -> Result<SemanticTypeIdV1, ProductionSemanticBodyErrorV1> {
        self.work()?;
        let ty = normalize_type_v1(self.tcx, self.instance, raw).map_err(|_| {
            unsupported(
                "type that failed monomorphic normalization",
                block,
                statement,
            )
        })?;
        self.type_ids
            .get(&ty)
            .copied()
            .ok_or_else(|| table("canonical type binding table"))
    }

    fn local_id(
        &self,
        raw_local: usize,
    ) -> Result<SemanticLocalIdV1, ProductionSemanticBodyErrorV1> {
        self.locals_by_raw
            .get(raw_local)
            .map(|binding| binding.semantic_local)
            .ok_or_else(|| table("local table"))
    }

    fn block_id(
        &self,
        raw_block: usize,
    ) -> Result<SemanticBlockIdV1, ProductionSemanticBodyErrorV1> {
        self.blocks_by_raw
            .get(raw_block)
            .map(|binding| binding.semantic_block)
            .ok_or_else(|| table("block table"))
    }

    fn edge(
        &self,
        role: SemanticEdgeRoleV1,
        raw_target: usize,
    ) -> Result<SemanticControlFlowEdgeV1, ProductionSemanticBodyErrorV1> {
        Ok(SemanticControlFlowEdgeV1::new(
            role,
            self.block_id(raw_target)?,
        ))
    }

    fn work(&mut self) -> Result<(), ProductionSemanticBodyErrorV1> {
        self.owner.charge(SemanticMirResourceV1::ValidationWork, 1)
    }
}

fn require_no_entry_predecessors_v1<I, S, F>(
    entry: usize,
    successors_by_block: I,
    mut charge_work: F,
) -> Result<(), ProductionSemanticBodyErrorV1>
where
    I: IntoIterator<Item = (usize, S)>,
    S: IntoIterator<Item = usize>,
    F: FnMut() -> Result<(), ProductionSemanticBodyErrorV1>,
{
    for (predecessor, successors) in successors_by_block {
        for successor in successors {
            charge_work()?;
            if successor == entry {
                let predecessor = u32::try_from(predecessor)
                    .map_err(|_| table("rustc control-flow block identity"))?;
                return Err(ProductionSemanticBodyErrorV1::RustcStartBlockPredecessor {
                    predecessor,
                });
            }
        }
    }
    Ok(())
}

fn build_type_table_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    bindings: &[ProductionSemanticTypeBindingV1<'tcx>],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<HashMap<Ty<'tcx>, SemanticTypeIdV1>, ProductionSemanticBodyErrorV1> {
    let mut by_type = HashMap::new();
    by_type
        .try_reserve(bindings.len())
        .map_err(|_| allocation(SemanticMirResourceV1::Types))?;
    let mut by_id = HashMap::new();
    by_id
        .try_reserve(bindings.len())
        .map_err(|_| allocation(SemanticMirResourceV1::Types))?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        let normalized = normalize_type_v1(tcx, instance, binding.rustc_type)
            .map_err(|_| table("canonical type binding table"))?;
        if normalized != binding.rustc_type
            || by_type
                .insert(binding.rustc_type, binding.semantic_type)
                .is_some()
            || by_id
                .insert(binding.semantic_type, binding.rustc_type)
                .is_some()
        {
            return Err(table("canonical type binding table"));
        }
    }
    Ok(by_type)
}

fn build_local_tables_v1<'a, 'tcx>(
    body: &Body<'_>,
    bindings: &'a [ProductionSemanticLocalBindingV1],
    inserted: Option<ReceiverLocalV1>,
    wrapping: Option<&WrappingMaterializationV1>,
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<LocalTablesV1<'a>, ProductionSemanticBodyErrorV1> {
    if bindings.len() != body.local_decls.len() {
        return Err(table("local table"));
    }
    let mut by_raw = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Locals)?;
    let count = bindings
        .len()
        .checked_add(usize::from(inserted.is_some()))
        .and_then(|count| count.checked_add(wrapping.map_or(0, |wrapping| wrapping.local_count())))
        .ok_or_else(|| table("local table size"))?;
    let mut by_semantic = try_filled_vec_v1(count, None, SemanticMirResourceV1::Locals)?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        insert_dense_binding_v1(&mut by_raw, binding.rustc_local, binding, "local table")?;
        insert_dense_binding_v1(
            &mut by_semantic,
            binding.semantic_local.index(),
            binding,
            "local table",
        )?;
    }
    let mut previous = None;
    for (index, binding) in by_semantic.iter().enumerate() {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        let identity = if inserted.is_some_and(|local| local.local.index() as usize == index) {
            if binding.is_some() {
                return Err(table("occupied shared receiver slot"));
            }
            inserted.unwrap().identity
        } else if let Some(local) = wrapping.and_then(|wrapping| wrapping.local(index)) {
            if binding.is_some() {
                return Err(table("occupied wrapping shift local slot"));
            }
            local.local.identity
        } else {
            binding.ok_or_else(|| table("local table"))?.identity
        };
        if (inserted.is_some() || wrapping.is_some())
            && previous.is_some_and(|previous| previous >= identity)
        {
            return Err(table("noncanonical shared receiver local union"));
        }
        previous = Some(identity);
    }
    Ok((
        collect_dense_bindings_v1(by_raw, "local table")?,
        by_semantic,
    ))
}

fn build_block_tables_v1<'a, 'tcx>(
    body: &Body<'_>,
    bindings: &'a [ProductionSemanticBlockBindingV1],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<
    (
        Vec<&'a ProductionSemanticBlockBindingV1>,
        Vec<&'a ProductionSemanticBlockBindingV1>,
    ),
    ProductionSemanticBodyErrorV1,
> {
    if bindings.len() != body.basic_blocks.len() {
        return Err(table("block table"));
    }
    let mut by_raw = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Blocks)?;
    let mut by_semantic = try_filled_vec_v1(bindings.len(), None, SemanticMirResourceV1::Blocks)?;
    for binding in bindings {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        insert_dense_binding_v1(&mut by_raw, binding.rustc_block, binding, "block table")?;
        insert_dense_binding_v1(
            &mut by_semantic,
            binding.semantic_block.index(),
            binding,
            "block table",
        )?;
    }
    Ok((
        collect_dense_bindings_v1(by_raw, "block table")?,
        collect_dense_bindings_v1(by_semantic, "block table")?,
    ))
}

fn build_call_tables_v1<'a, 'tcx>(
    function: SemanticFunctionIdV1,
    block_count: usize,
    direct_calls: &'a [ProductionSemanticDirectCallBindingV1<'tcx>],
    terminal_expansions: &'a [ProductionSemanticTerminalExpansionRecipeV1<'tcx>],
    normalized_intrinsics: &'a [ProductionSemanticNormalizedRustcIntrinsicRecipeV1<'tcx>],
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<CallTablesV1<'a, 'tcx>, ProductionSemanticBodyErrorV1> {
    let mut direct_by_raw = try_filled_vec_v1(block_count, None, SemanticMirResourceV1::Blocks)?;
    let mut terminal_by_raw = try_filled_vec_v1(block_count, None, SemanticMirResourceV1::Blocks)?;
    let mut normalized_by_raw =
        try_filled_vec_v1(block_count, None, SemanticMirResourceV1::Blocks)?;
    for binding in direct_calls {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        if binding.caller != function {
            return Err(table("direct-call binding table"));
        }
        insert_sparse_binding_v1(
            &mut direct_by_raw,
            binding.rustc_block,
            binding,
            "direct-call binding table",
        )?;
    }
    for recipe in terminal_expansions {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        if recipe.caller != function || terminal_argument_count_v1(recipe.expansion).is_none() {
            return Err(table("terminal expansion table"));
        }
        insert_sparse_binding_v1(
            &mut terminal_by_raw,
            recipe.rustc_block,
            recipe,
            "terminal expansion table",
        )?;
    }
    for recipe in normalized_intrinsics {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        if recipe.caller != function {
            return Err(table("normalized intrinsic table"));
        }
        insert_sparse_binding_v1(
            &mut normalized_by_raw,
            recipe.rustc_block,
            recipe,
            "normalized intrinsic table",
        )?;
    }
    for index in 0..block_count {
        let bindings = usize::from(direct_by_raw[index].is_some())
            + usize::from(terminal_by_raw[index].is_some())
            + usize::from(normalized_by_raw[index].is_some());
        if bindings > 1 {
            return Err(table("call binding table"));
        }
    }
    Ok((direct_by_raw, terminal_by_raw, normalized_by_raw))
}

fn insert_dense_binding_v1<'a, T>(
    table: &mut [Option<&'a T>],
    index: u32,
    value: &'a T,
    table_name: &'static str,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    insert_sparse_binding_v1(table, index, value, table_name)
}

fn insert_sparse_binding_v1<'a, T>(
    bindings: &mut [Option<&'a T>],
    index: u32,
    value: &'a T,
    table_name: &'static str,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let index = usize::try_from(index).map_err(|_| table(table_name))?;
    let slot = bindings.get_mut(index).ok_or_else(|| table(table_name))?;
    if slot.replace(value).is_some() {
        return Err(table(table_name));
    }
    Ok(())
}

fn collect_dense_bindings_v1<T>(
    bindings: Vec<Option<T>>,
    table_name: &'static str,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    bindings
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| table(table_name))
}

fn normalize_type_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    raw: Ty<'tcx>,
) -> Result<Ty<'tcx>, ()> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| ())
}

pub(crate) fn resolve_direct_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    function: &Operand<'tcx>,
) -> Result<Instance<'tcx>, &'static str> {
    let raw = function.ty(body, tcx);
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| "callable type that failed monomorphic normalization")?;
    let TyKind::FnDef(def_id, arguments) = callable.kind() else {
        return Err("indirect or non-function-definition call");
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, arguments)
        .map_err(|_| "direct call whose concrete rustc instance failed resolution")?
        .ok_or("direct call without a concrete rustc instance")
}

fn validate_export_role_v1(
    role: SemanticFunctionRoleV1,
    export: &ProductionSemanticFunctionExportV1,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let valid = matches!(
        (role, export),
        (
            SemanticFunctionRoleV1::KernelRoot,
            ProductionSemanticFunctionExportV1::Kernel(_)
        ) | (
            SemanticFunctionRoleV1::DeviceFfiExport,
            ProductionSemanticFunctionExportV1::DeviceFfi(_)
        ) | (
            SemanticFunctionRoleV1::InternalHelper | SemanticFunctionRoleV1::DropGlue(_),
            ProductionSemanticFunctionExportV1::None
        )
    );
    if valid {
        Ok(())
    } else {
        Err(table("function role/export binding"))
    }
}

fn semantic_local_role_v1(
    raw_local: u32,
    argument_count: usize,
    rust_call_tuple_argument: Option<u32>,
) -> Result<SemanticLocalRoleV1, ProductionSemanticBodyErrorV1> {
    if raw_local == u32::try_from(RETURN_PLACE.index()).unwrap_or(0) {
        return Ok(SemanticLocalRoleV1::Return);
    }
    let raw = usize::try_from(raw_local).map_err(|_| table("local role"))?;
    if raw <= argument_count {
        let argument = raw.checked_sub(1).ok_or_else(|| table("local role"))?;
        if let Some(tuple) = rust_call_tuple_argument
            && argument >= tuple as usize
        {
            return Ok(SemanticLocalRoleV1::RustCallTupleField {
                argument: tuple,
                field: u32::try_from(argument - tuple as usize)
                    .map_err(|_| table("RustCall field ordinal"))?,
            });
        }
        Ok(SemanticLocalRoleV1::Argument(
            u32::try_from(argument).map_err(|_| table("local role"))?,
        ))
    } else {
        Ok(SemanticLocalRoleV1::Temporary)
    }
}

fn semantic_borrow_kind_v1(
    borrow: BorrowKind,
    block: Option<u32>,
    statement: Option<u32>,
) -> Result<SemanticBorrowKindV1, ProductionSemanticBodyErrorV1> {
    match borrow {
        BorrowKind::Shared => Ok(SemanticBorrowKindV1::Shared),
        BorrowKind::Fake(_) => Err(unsupported("fake borrow rvalue", block, statement)),
        BorrowKind::Mut {
            kind: MutBorrowKind::Default | MutBorrowKind::TwoPhaseBorrow,
        } => Ok(SemanticBorrowKindV1::Mutable),
        BorrowKind::Mut {
            kind: MutBorrowKind::ClosureCapture,
        } => Err(unsupported(
            "closure-capture mutable borrow rvalue",
            block,
            statement,
        )),
    }
}

const fn terminal_argument_count_v1(expansion: ProductionTerminalExpansionV1) -> Option<usize> {
    match expansion {
        ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32 => Some(8),
        ProductionTerminalExpansionV1::Gfx942OrderedProgramE32 => Some(8),
        ProductionTerminalExpansionV1::Gfx942CompleteBodyE32 => Some(10),
        ProductionTerminalExpansionV1::Gfx942PhysicalEntryBegin => Some(5),
        ProductionTerminalExpansionV1::Gfx942PhysicalEntryLabel
        | ProductionTerminalExpansionV1::Gfx942PhysicalEntryStep => Some(0),
        ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeBegin => Some(2),
        ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeLabel
        | ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeStep => Some(0),
        ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyBegin => Some(2),
        ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyLabel
        | ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyStep => Some(0),
        ProductionTerminalExpansionV1::Gfx942InlineU32(operation) => Some(
            crate::production_inline_assembly_v30::input_count(operation),
        ),
        ProductionTerminalExpansionV1::WorkgroupDerive
        | ProductionTerminalExpansionV1::MaskedTileIntoFragmentU32
        | ProductionTerminalExpansionV1::LaneFragmentIntoPartsU32 => Some(1),
        ProductionTerminalExpansionV1::MaskedTileLoadU32 => Some(3),
        ProductionTerminalExpansionV1::ContextIssue
        | ProductionTerminalExpansionV1::ThreadIndex(_)
        | ProductionTerminalExpansionV1::WorkgroupIndex(_)
        | ProductionTerminalExpansionV1::WorkgroupDimension(_)
        | ProductionTerminalExpansionV1::GridDimension(_)
        | ProductionTerminalExpansionV1::MathContextCurrent
        | ProductionTerminalExpansionV1::CollectiveContextCurrent
        | ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent
        | ProductionTerminalExpansionV1::WaveLaneCurrent
        | ProductionTerminalExpansionV1::MatrixContextCurrent
        | ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent
        | ProductionTerminalExpansionV1::Gfx950SubgroupCurrent
        | ProductionTerminalExpansionV1::ThreadIndex1d
        | ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent
        | ProductionTerminalExpansionV1::Trap
        | ProductionTerminalExpansionV1::ColdPath => Some(0),
        ProductionTerminalExpansionV1::ThreadIndexGet
        | ProductionTerminalExpansionV1::F32MatrixAccumulatorZero
        | ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues
        | ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero
        | ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues
        | ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero
        | ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent
        | ProductionTerminalExpansionV1::Gfx950LdsTransposePublish
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8
        | ProductionTerminalExpansionV1::DynamicLdsExactCurrent
        | ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts
        | ProductionTerminalExpansionV1::Bf16Conversion(_)
        | ProductionTerminalExpansionV1::WorkgroupPipelineCurrent
        | ProductionTerminalExpansionV1::RustcFabsF32
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen
        | ProductionTerminalExpansionV1::DisjointSliceLen => Some(1),
        ProductionTerminalExpansionV1::SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::SubgroupReduceMaxF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::DisjointBlockComponentIndex
        | ProductionTerminalExpansionV1::MemoryVolatileLoad
        | ProductionTerminalExpansionV1::RustcSaturatingInteger(_)
        | ProductionTerminalExpansionV1::WorkgroupPipelineStage
        | ProductionTerminalExpansionV1::WorkgroupPipelineCommit
        | ProductionTerminalExpansionV1::WorkgroupPipelineWait
        | ProductionTerminalExpansionV1::WorkgroupPipelineConsume
        | ProductionTerminalExpansionV1::WorkgroupPipelineDiscard
        | ProductionTerminalExpansionV1::WorkgroupPipelineRelease => Some(2),
        ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32
        | ProductionTerminalExpansionV1::Gfx942Wave64Shuffle(_)
        | ProductionTerminalExpansionV1::WorkgroupPipelineRead
        | ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum
        | ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum
        | ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint => Some(3),
        ProductionTerminalExpansionV1::MathF32(function) => Some(function.arity() + 1),
        ProductionTerminalExpansionV1::MatrixMultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate
        | ProductionTerminalExpansionV1::WorkgroupReduceSum
        | ProductionTerminalExpansionV1::WorkgroupPipelineWrite
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock => Some(4),
        ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8 => Some(4),
        ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2
        | ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2
        | ProductionTerminalExpansionV1::Bf16MatrixBColumnMajorLoadZeroFilledV1
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16
        | ProductionTerminalExpansionV1::StridedReadView2DLoadOr => Some(4),
        ProductionTerminalExpansionV1::Bf16MatrixARowMajor
        | ProductionTerminalExpansionV1::Bf16MatrixBRowMajor
        | ProductionTerminalExpansionV1::Bf16MatrixBColumnMajor
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor
        | ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice => Some(5),
        ProductionTerminalExpansionV1::DisjointSliceGetMut => Some(2),
        ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint
        | ProductionTerminalExpansionV1::ThreadIndexCheckedShift
        | ProductionTerminalExpansionV1::DisjointIndexGet
        | ProductionTerminalExpansionV1::DisjointIndexCheckedShift => Some(1),
        ProductionTerminalExpansionV1::ThreadIndexCheckedBlock
        | ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d
        | ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d => Some(1),
        ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut => Some(2),
        ProductionTerminalExpansionV1::GridLeaderCurrent => Some(0),
        ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive => Some(3),
        ProductionTerminalExpansionV1::DisjointSliceGetBlockMut => Some(3),
        ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut => Some(6),
        ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut => Some(6),
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d => Some(7),
        ProductionTerminalExpansionV1::WorkgroupBarrier => Some(0),
    }
}

fn semantic_binary_operation(operation: BinOp) -> Option<SemanticBinaryOpV1> {
    match operation {
        BinOp::Add => Some(SemanticBinaryOpV1::Add),
        BinOp::Sub => Some(SemanticBinaryOpV1::Subtract),
        BinOp::Mul => Some(SemanticBinaryOpV1::Multiply),
        BinOp::Div => Some(SemanticBinaryOpV1::Divide),
        BinOp::Rem => Some(SemanticBinaryOpV1::Remainder),
        BinOp::BitXor => Some(SemanticBinaryOpV1::BitXor),
        BinOp::BitAnd => Some(SemanticBinaryOpV1::BitAnd),
        BinOp::BitOr => Some(SemanticBinaryOpV1::BitOr),
        BinOp::Shl | BinOp::ShlUnchecked => Some(SemanticBinaryOpV1::ShiftLeft),
        BinOp::Shr | BinOp::ShrUnchecked => Some(SemanticBinaryOpV1::ShiftRight),
        BinOp::Eq => Some(SemanticBinaryOpV1::Equal),
        BinOp::Lt => Some(SemanticBinaryOpV1::LessThan),
        BinOp::Le => Some(SemanticBinaryOpV1::LessOrEqual),
        BinOp::Ne => Some(SemanticBinaryOpV1::NotEqual),
        BinOp::Ge => Some(SemanticBinaryOpV1::GreaterOrEqual),
        BinOp::Gt => Some(SemanticBinaryOpV1::GreaterThan),
        BinOp::Offset => Some(SemanticBinaryOpV1::Offset),
        BinOp::Cmp
        | BinOp::AddWithOverflow
        | BinOp::SubWithOverflow
        | BinOp::MulWithOverflow
        | BinOp::AddUnchecked
        | BinOp::SubUnchecked
        | BinOp::MulUnchecked => None,
    }
}

const fn semantic_checked_binary_operation(operation: BinOp) -> Option<SemanticCheckedBinaryOpV1> {
    match operation {
        BinOp::AddWithOverflow => Some(SemanticCheckedBinaryOpV1::Add),
        BinOp::SubWithOverflow => Some(SemanticCheckedBinaryOpV1::Subtract),
        BinOp::MulWithOverflow => Some(SemanticCheckedBinaryOpV1::Multiply),
        _ => None,
    }
}

const fn semantic_unchecked_binary_operation(
    operation: BinOp,
) -> Option<SemanticUncheckedBinaryOpV1> {
    match operation {
        BinOp::AddUnchecked => Some(SemanticUncheckedBinaryOpV1::Add),
        BinOp::SubUnchecked => Some(SemanticUncheckedBinaryOpV1::Subtract),
        BinOp::MulUnchecked => Some(SemanticUncheckedBinaryOpV1::Multiply),
        _ => None,
    }
}

fn try_vec_v1<T>(
    capacity: usize,
    resource: SemanticMirResourceV1,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation(resource))?;
    Ok(values)
}

fn try_filled_vec_v1<T: Clone>(
    length: usize,
    value: T,
    resource: SemanticMirResourceV1,
) -> Result<Vec<T>, ProductionSemanticBodyErrorV1> {
    let mut values = try_vec_v1(length, resource)?;
    values.resize(length, value);
    Ok(values)
}

fn table(table: &'static str) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::IdentityTableMismatch { table }
}

fn allocation(resource: SemanticMirResourceV1) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::Allocation { resource }
}

fn unsupported(
    construct: impl Into<String>,
    block: Option<u32>,
    statement: Option<u32>,
) -> ProductionSemanticBodyErrorV1 {
    ProductionSemanticBodyErrorV1::Unsupported {
        construct: construct
            .into()
            .chars()
            .take(MAX_ERROR_COMPONENT_CHARS_V1)
            .collect(),
        block,
        statement,
    }
}

#[cfg(test)]
#[path = "production_complete_body_terminal144_tests.rs"]
mod complete_body_terminal144_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_middle::mir::FakeBorrowKind;

    #[test]
    fn terminal_expansion_arities_are_closed() {
        for (expansion, count) in [
            (ProductionTerminalExpansionV1::ContextIssue, 0),
            (ProductionTerminalExpansionV1::WorkgroupDerive, 1),
            (ProductionTerminalExpansionV1::MaskedTileLoadU32, 3),
            (ProductionTerminalExpansionV1::MaskedTileIntoFragmentU32, 1),
            (ProductionTerminalExpansionV1::LaneFragmentIntoPartsU32, 1),
        ] {
            assert_eq!(terminal_argument_count_v1(expansion), Some(count));
        }
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::ThreadIndex1d),
            Some(0)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent),
            Some(0)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::ThreadIndexGet),
            Some(1)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::DisjointSliceGetMut),
            Some(2)
        );
        assert_eq!(
            terminal_argument_count_v1(ProductionTerminalExpansionV1::DisjointBlockComponentIndex,),
            Some(2)
        );
    }

    #[test]
    fn local_roles_follow_rustc_body_numbering() {
        assert_eq!(
            semantic_local_role_v1(0, 2, None).unwrap(),
            SemanticLocalRoleV1::Return
        );
        assert_eq!(
            semantic_local_role_v1(1, 2, None).unwrap(),
            SemanticLocalRoleV1::Argument(0)
        );
        assert_eq!(
            semantic_local_role_v1(2, 2, None).unwrap(),
            SemanticLocalRoleV1::Argument(1)
        );
        assert_eq!(
            semantic_local_role_v1(3, 2, None).unwrap(),
            SemanticLocalRoleV1::Temporary
        );
    }

    #[test]
    fn expanded_rust_call_roles_preserve_receiver_and_outer_field_ordinals() {
        for arity in 0..=3 {
            assert_eq!(
                semantic_local_role_v1(0, arity + 1, Some(1)).unwrap(),
                SemanticLocalRoleV1::Return
            );
            assert_eq!(
                semantic_local_role_v1(1, arity + 1, Some(1)).unwrap(),
                SemanticLocalRoleV1::Argument(0)
            );
            for field in 0..arity {
                assert_eq!(
                    semantic_local_role_v1((field + 2) as u32, arity + 1, Some(1)).unwrap(),
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: field as u32
                    }
                );
            }
            assert_eq!(
                semantic_local_role_v1((arity + 2) as u32, arity + 1, Some(1)).unwrap(),
                SemanticLocalRoleV1::Temporary
            );
        }
    }

    #[test]
    fn diagnostic_components_are_bounded() {
        let error = unsupported(
            "x".repeat(MAX_ERROR_COMPONENT_CHARS_V1 + 32),
            Some(7),
            Some(3),
        );
        let ProductionSemanticBodyErrorV1::Unsupported { construct, .. } = error else {
            panic!("unexpected error kind");
        };
        assert_eq!(construct.len(), MAX_ERROR_COMPONENT_CHARS_V1);
    }

    #[test]
    fn rustc_entry_contract_accepts_loop_continue_and_break_edges() {
        // b1 is the loop header, b2 continues to it, and b1 may break to b3.
        // The rustc start block b0 remains outside the cycle.
        let successors: &[&[usize]] = &[&[1], &[2, 3], &[1], &[]];
        let mut work = 0_u64;
        require_no_entry_predecessors_v1(
            0,
            successors
                .iter()
                .enumerate()
                .map(|(block, targets)| (block, targets.iter().copied())),
            || {
                work += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(work, 4);
    }

    #[test]
    fn rustc_entry_contract_rejects_the_exact_predecessor() {
        let successors: &[&[usize]] = &[&[1], &[0]];
        let error = require_no_entry_predecessors_v1(
            0,
            successors
                .iter()
                .enumerate()
                .map(|(block, targets)| (block, targets.iter().copied())),
            || Ok(()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::RustcStartBlockPredecessor { predecessor: 1 }
        ));
        assert_eq!(
            error.to_string(),
            "semantic body construction rejected invalid rustc MIR: start block has predecessor block 1"
        );
    }

    #[test]
    fn rustc_entry_contract_charges_the_exact_edge_boundary() {
        let successors: &[&[usize]] = &[&[1], &[2, 3], &[1], &[]];
        let exact_limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, 4)
            .unwrap();
        let mut exact = ConstructionTotalsV1::default();
        require_no_entry_predecessors_v1(
            0,
            successors
                .iter()
                .enumerate()
                .map(|(block, targets)| (block, targets.iter().copied())),
            || exact.charge(SemanticMirResourceV1::ValidationWork, 1, exact_limits),
        )
        .unwrap();
        assert_eq!(exact.validation_work, 4);

        let exhausted_limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, 3)
            .unwrap();
        let mut exhausted = ConstructionTotalsV1::default();
        let error = require_no_entry_predecessors_v1(
            0,
            successors
                .iter()
                .enumerate()
                .map(|(block, targets)| (block, targets.iter().copied())),
            || exhausted.charge(SemanticMirResourceV1::ValidationWork, 1, exhausted_limits),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: 4,
                maximum: 3,
            }
        ));
    }

    #[test]
    fn request_owner_accounting_is_cumulative_across_bodies() {
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Functions, 1)
            .unwrap()
            .with_limit(SemanticMirResourceV1::ConstantBytes, 3)
            .unwrap();
        let mut owner = ProductionSemanticBodyRequestOwnerV1 {
            limits,
            totals: ConstructionTotalsV1::default(),
            callables: HashMap::new(),
            inline_sources: Default::default(),
            ordered_sources: Default::default(),
            program_sources: Default::default(),
            bf16_inspection: Default::default(),
            bf16_tile_values: Default::default(),
            complete_body_annotation: None,
            physical_entry_annotations: None,
            physical_global_copy_annotations: None,
            physical_lds_exchange_annotations: None,
            defined_functions: 0,
            context_entries: Vec::new(),
            function_commitments: None,
            workgroup_scopes: None,
        };

        owner.charge(SemanticMirResourceV1::Functions, 1).unwrap();
        let error = owner
            .charge(SemanticMirResourceV1::Functions, 1)
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: 2,
                maximum: 1,
            }
        ));

        owner
            .charge(SemanticMirResourceV1::ConstantBytes, 2)
            .unwrap();
        let error = owner
            .charge(SemanticMirResourceV1::ConstantBytes, 2)
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ConstantBytes,
                actual: 4,
                maximum: 3,
            }
        ));
    }

    #[test]
    fn callable_owner_rejects_id_and_kind_substitution() {
        let callable = SemanticCallableIdV1::from_index(4);
        assert!(require_canonical_callable_id_v1(4, callable).is_ok());
        assert!(require_canonical_callable_id_v1(3, callable).is_err());

        let record = ProductionSemanticCallableOwnerRecordV1 {
            kind: ProductionSemanticCallableOwnerKindV1::Terminal(
                ProductionTerminalExpansionV1::ThreadIndex1d,
            ),
            semantic_callable: callable,
        };
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Terminal(
                    ProductionTerminalExpansionV1::ThreadIndex1d,
                ),
            )
            .is_ok()
        );
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Terminal(
                    ProductionTerminalExpansionV1::ThreadIndexGet,
                ),
            )
            .is_err()
        );
        assert!(
            resolve_owned_callable_record_v1(
                record,
                ProductionSemanticCallableOwnerKindV1::Defined,
            )
            .is_err()
        );
    }

    #[test]
    fn fake_borrows_fail_closed_before_schema_construction() {
        for kind in [FakeBorrowKind::Deep, FakeBorrowKind::Shallow] {
            let error = semantic_borrow_kind_v1(BorrowKind::Fake(kind), Some(9), Some(2))
                .expect_err("fake borrows must never enter semantic MIR");
            assert!(matches!(
                error,
                ProductionSemanticBodyErrorV1::Unsupported {
                    block: Some(9),
                    statement: Some(2),
                    ..
                }
            ));
        }
    }

    #[test]
    fn checked_binary_mapping_is_exact_and_never_aliases_plain_or_unchecked_arithmetic() {
        assert_eq!(
            semantic_checked_binary_operation(BinOp::AddWithOverflow),
            Some(SemanticCheckedBinaryOpV1::Add)
        );
        assert_eq!(
            semantic_checked_binary_operation(BinOp::SubWithOverflow),
            Some(SemanticCheckedBinaryOpV1::Subtract)
        );
        assert_eq!(
            semantic_checked_binary_operation(BinOp::MulWithOverflow),
            Some(SemanticCheckedBinaryOpV1::Multiply)
        );
        for operation in [
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::AddUnchecked,
            BinOp::SubUnchecked,
            BinOp::MulUnchecked,
        ] {
            assert_eq!(semantic_checked_binary_operation(operation), None);
        }
    }

    #[test]
    fn unchecked_raw_mir_shifts_are_admitted_with_their_exact_direction() {
        for (operation, expected) in [
            (BinOp::ShlUnchecked, SemanticBinaryOpV1::ShiftLeft),
            (BinOp::ShrUnchecked, SemanticBinaryOpV1::ShiftRight),
        ] {
            assert_eq!(semantic_binary_operation(operation), Some(expected));
            assert_eq!(semantic_checked_binary_operation(operation), None);
            assert_eq!(semantic_unchecked_binary_operation(operation), None);
        }
    }
}
