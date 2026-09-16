use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as ExportBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1 as ExportResourceV1,
};

#[derive(Debug)]
pub(crate) enum CheckedOutputExportErrorV1 {
    Resource(ExportResourceV1),
    Capacity,
    Poisoned,
}

impl fmt::Display for CheckedOutputExportErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Capacity => f.write_str("inert export buffer capacity exceeded"),
            Self::Poisoned => f.write_str("inert export buffer construction previously failed"),
        }
    }
}

impl std::error::Error for CheckedOutputExportErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Capacity | Self::Poisoned => None,
        }
    }
}

impl From<ExportResourceV1> for CheckedOutputExportErrorV1 {
    fn from(error: ExportResourceV1) -> Self {
        Self::Resource(error)
    }
}

fn export_error_v1(error: CheckedOutputExportErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputExport(error)
}

// A single conservative component bound, not a new full-frame/wire allowance.
const EXPORT_BUFFER_COMPONENT_CAP_V1: usize =
    fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;

struct CheckedOutputExportWriterV1 {
    bytes: Vec<u8>,
    logical_cap: usize,
    retained: usize,
    floor: usize,
    ledger: usize,
    poisoned: bool,
}

// The receipt is inseparable from its private, move-only bytes. In transit it
// names a phase transfer, not a still-live reservation on the producing ledger.
struct TransferredExportBytesV1 {
    bytes: Vec<u8>,
    retained: usize,
}

struct PreparedInertExportV1<Bindings> {
    packet: TransferredExportBytesV1,
    bindings: Bindings,
}

fn export_header_v1<Bindings>() -> usize {
    let carrier =
        std::mem::size_of::<PreparedInertExportV1<Bindings>>() - std::mem::size_of::<Bindings>();
    std::mem::size_of::<CheckedOutputExportWriterV1>().max(carrier)
}

fn export_end_v1(
    start: usize,
    length: usize,
    cap: usize,
) -> Result<usize, CheckedOutputExportErrorV1> {
    let end = start
        .checked_add(length)
        .ok_or(ExportResourceV1::Arithmetic)?;
    if end > cap {
        return Err(CheckedOutputExportErrorV1::Capacity);
    }
    Ok(end)
}

fn restore_export_floor_v1(
    budget: &mut ExportBudgetV1<'_>,
    floor: usize,
) -> Result<(), CheckedOutputExportErrorV1> {
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ExportResourceV1::Accounting)?;
    budget.release_storage(release)?;
    Ok(())
}

impl CheckedOutputExportWriterV1 {
    fn try_new<Bindings>(
        cap: usize,
        budget: &mut ExportBudgetV1<'_>,
    ) -> Result<Self, CheckedOutputExportErrorV1> {
        Self::try_new_with_v1::<Bindings>(cap, budget, |bytes, count| {
            bytes
                .try_reserve_exact(count)
                .map_err(|_| ExportResourceV1::Allocation)
        })
    }

    fn try_new_with_v1<Bindings>(
        cap: usize,
        budget: &mut ExportBudgetV1<'_>,
        reserve: impl FnOnce(&mut Vec<u8>, usize) -> Result<(), ExportResourceV1>,
    ) -> Result<Self, CheckedOutputExportErrorV1> {
        let floor = budget.storage();
        // Cap/arithmetic, allocation and immediate capacity reconciliation.
        budget.charge_work(4)?;
        if cap > EXPORT_BUFFER_COMPONENT_CAP_V1 {
            return Err(CheckedOutputExportErrorV1::Capacity);
        }
        let requested = export_header_v1::<Bindings>()
            .checked_add(cap)
            .ok_or(ExportResourceV1::Arithmetic)?;
        budget.reserve_storage(requested)?;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Self, CheckedOutputExportErrorV1> {
                let mut bytes = Vec::new();
                reserve(&mut bytes, cap)?;
                if !bytes.is_empty() {
                    return Err(ExportResourceV1::Accounting.into());
                }
                let excess = bytes
                    .capacity()
                    .checked_sub(cap)
                    .ok_or(ExportResourceV1::Accounting)?;
                budget.reserve_storage(excess)?;
                let retained = requested
                    .checked_add(excess)
                    .ok_or(ExportResourceV1::Arithmetic)?;
                Ok(Self {
                    bytes,
                    logical_cap: cap,
                    retained,
                    floor,
                    ledger: budget as *const ExportBudgetV1<'_> as usize,
                    poisoned: false,
                })
            },
        ));
        match outcome {
            Ok(Ok(writer)) => Ok(writer),
            other => {
                restore_export_floor_v1(budget, floor)?;
                match other {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => std::panic::resume_unwind(payload),
                    Ok(Ok(_)) => unreachable!(),
                }
            }
        }
    }

    fn require_live_v1(
        &self,
        budget: &ExportBudgetV1<'_>,
    ) -> Result<(), CheckedOutputExportErrorV1> {
        let floor = self
            .floor
            .checked_add(self.retained)
            .ok_or(ExportResourceV1::Arithmetic)?;
        if self.ledger != budget as *const ExportBudgetV1<'_> as usize || budget.storage() < floor {
            return Err(ExportResourceV1::Accounting.into());
        }
        Ok(())
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "#271 B2 serializer is not approved; B1 has no default writer caller"
        )
    )]
    fn append_v1(
        &mut self,
        bytes: &[u8],
        budget: &mut ExportBudgetV1<'_>,
    ) -> Result<std::ops::Range<usize>, CheckedOutputExportErrorV1> {
        let was_poisoned = self.poisoned;
        self.poisoned = true;
        // Two shape/state checks, one extent calculation, plus copied bytes.
        budget.charge_work(
            3usize
                .checked_add(bytes.len())
                .ok_or(ExportResourceV1::Arithmetic)?,
        )?;
        self.require_live_v1(budget)?;
        if was_poisoned {
            return Err(CheckedOutputExportErrorV1::Poisoned);
        }
        let start = self.bytes.len();
        let end = export_end_v1(start, bytes.len(), self.logical_cap)?;
        if end > self.bytes.capacity() {
            return Err(ExportResourceV1::Accounting.into());
        }
        self.bytes.extend_from_slice(bytes);
        self.poisoned = false;
        Ok(start..end)
    }

    fn transfer_v1(
        self,
        budget: &mut ExportBudgetV1<'_>,
    ) -> Result<TransferredExportBytesV1, CheckedOutputExportErrorV1> {
        // Final state/floor check and explicit producer phase transfer.
        budget.charge_work(3)?;
        self.require_live_v1(budget)?;
        if self.poisoned {
            return Err(CheckedOutputExportErrorV1::Poisoned);
        }
        if budget.storage()
            != self
                .floor
                .checked_add(self.retained)
                .ok_or(ExportResourceV1::Arithmetic)?
        {
            return Err(ExportResourceV1::Accounting.into());
        }
        budget.release_storage(self.retained)?;
        Ok(TransferredExportBytesV1 {
            bytes: self.bytes,
            retained: self.retained,
        })
    }
}

impl<Bindings> PreparedInertExportV1<Bindings> {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "#271 B1 receiver remains inert and has no publication caller"
        )
    )]
    fn with_accepted_v1(
        self,
        budget: &mut ExportBudgetV1<'_>,
        next: impl FnOnce(
            &[u8],
            &Bindings,
            &mut ExportBudgetV1<'_>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<(), ProductionPipelineError> {
        let floor = budget.storage();
        // Acceptance precedes payload inspection or callback work.
        budget
            .charge_work(2)
            .map_err(|e| export_error_v1(e.into()))?;
        budget
            .reserve_storage(self.packet.retained)
            .map_err(|e| export_error_v1(e.into()))?;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            next(&self.packet.bytes, &self.bindings, budget)
        }));
        drop(self);
        restore_export_floor_v1(budget, floor).map_err(export_error_v1)?;
        match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

// Generic only to test drop/transfer mechanics without fabricating protected
// bindings. The sole production caller supplies the actual owned stage/bindings.
fn with_owned_export_stage_v1<Stage, Bindings>(
    stage: Stage,
    mut writer: CheckedOutputExportWriterV1,
    source_retained: usize,
    budget: &mut ExportBudgetV1<'_>,
    run: impl FnOnce(
        &Stage,
        &mut CheckedOutputExportWriterV1,
        &mut ExportBudgetV1<'_>,
    ) -> Result<(), ProductionPipelineError>,
    split_after_borrows: impl FnOnce(Stage) -> Bindings,
) -> Result<PreparedInertExportV1<Bindings>, ProductionPipelineError> {
    let floor = writer.floor;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget
            .charge_work(5)
            .map_err(|e| export_error_v1(e.into()))?;
        writer.require_live_v1(budget).map_err(export_error_v1)?;
        if writer.retained
            != export_header_v1::<Bindings>()
                .checked_add(writer.bytes.capacity())
                .ok_or_else(|| export_error_v1(ExportResourceV1::Arithmetic.into()))?
        {
            return Err(export_error_v1(ExportResourceV1::Accounting.into()));
        }
        let expected = floor
            .checked_add(writer.retained)
            .and_then(|n| n.checked_add(source_retained))
            .ok_or_else(|| export_error_v1(ExportResourceV1::Arithmetic.into()))?;
        if budget.storage() != expected {
            return Err(export_error_v1(ExportResourceV1::Accounting.into()));
        }
        run(&stage, &mut writer, budget)?;
        // The source owner (including captured SSA) drops inside this split.
        Ok(split_after_borrows(stage))
    }));
    // Stage/source has dropped on success, error and unwind. Only original
    // bindings may now survive in the result; their historical domain is separate.
    let cleanup = (|| {
        let expected = floor
            .checked_add(writer.retained)
            .and_then(|n| n.checked_add(source_retained))
            .ok_or(ExportResourceV1::Arithmetic)?;
        let extra = budget
            .storage()
            .checked_sub(expected)
            .ok_or(ExportResourceV1::Accounting)?;
        budget.release_storage(extra)?;
        budget.release_storage(source_retained)?;
        Ok::<_, CheckedOutputExportErrorV1>(())
    })();
    if let Err(error) = cleanup {
        drop(outcome);
        drop(writer);
        restore_export_floor_v1(budget, floor).map_err(export_error_v1)?;
        return Err(export_error_v1(error));
    }
    match outcome {
        Ok(Ok(bindings)) => {
            let result = writer
                .transfer_v1(budget)
                .map(|packet| PreparedInertExportV1 { packet, bindings });
            if result.is_err() {
                restore_export_floor_v1(budget, floor).map_err(export_error_v1)?;
            }
            result.map_err(export_error_v1)
        }
        other => {
            drop(writer);
            restore_export_floor_v1(budget, floor).map_err(export_error_v1)?;
            match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    // No caller/default route is added. Arbitrary B1 bytes are inert and do not
    // constitute a source/O codec, protected frame or publication owner.
    #[expect(
        dead_code,
        reason = "#271 B1 ownership mechanics only; native export framing remains unapproved"
    )]
    fn prepare_inert_checked_output_buffer_v1(
        self,
        cap: usize,
        write_source: impl for<'evidence> FnOnce(
            &'evidence fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &'evidence [crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRootV1],
            &'evidence [crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
            &mut CheckedOutputExportWriterV1,
            &mut ExportBudgetV1<'_>,
        ) -> Result<(), ProductionPipelineError>,
        write_output: impl FnOnce(
            &fe2o3_kernel_opt::CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>,
            &dialect_amdgcn::ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>,
            &mut CheckedOutputExportWriterV1,
            &mut ExportBudgetV1<'_>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<PreparedInertExportV1<AuthenticatedProductionBindings>, ProductionPipelineError>
    {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .with_materialized_capture_initialization_v1(
                true,
                |budget| CheckedOutputExportWriterV1::try_new::<AuthenticatedProductionBindings>(cap, budget)
                    .map_err(export_error_v1).map_err(Box::new),
                |stage, writer, budget| {
                    budget.charge_work(3).map_err(|error| Box::new(export_error_v1(error.into())))?;
                    let capture = stage.materialized.semantic_ssa().occurrence_storage()
                        .ok_or_else(|| Box::new(export_error_v1(ExportResourceV1::Accounting.into())))?;
                    let source_retained = stage.materialized.executable_storage().retained_storage()
                        .checked_add(stage.materialized.assert_origin_storage().payload_storage())
                        .and_then(|bytes| bytes.checked_add(capture.retained_storage()))
                        .ok_or_else(|| Box::new(export_error_v1(ExportResourceV1::Arithmetic.into())))?;
                    with_owned_export_stage_v1(stage, writer, source_retained, budget,
                        |stage, writer, budget| {
                            crate::production_ranked_projection_v1::with_source_ranked_custody_v1(
                                &stage.materialized, &stage.ranked_roots, &stage.bindings.reference_effect_bindings,
                                budget, |source, budget| {
                                    crate::production_ranked_projection_v1::with_source_export_inputs_v1(
                                        source, budget, |materialized, roots, references, budget| {
                                            write_source(materialized, roots, references, writer, budget)
                                        },
                                    )?;
                                    with_checked_output_target_endpoint_v1(
                                        &stage.materialized, stage.bindings.rustc_target.profile(), budget,
                                        |bound, checked, budget| {
                                            use crate::production_ranked_projection_v1::CheckedOutputModuleJoinErrorV1 as Join;
                                            use fe2o3_lower_mir_kernel::{ProductionScopedFormalMemoryErrorV1 as Formal, ProductionSourceOutputErrorV1 as Output};
                                            let mut callback_error = None;
                                            let result = crate::production_ranked_projection_v1::with_source_checked_output_module_catalog_v1(
                                                source, bound, checked, stage.bindings.rustc_target.profile(), budget,
                                                |formals, catalog, budget| {
                                                    let result = (|| {
                                                        CheckedOutputMemoryPreparedProductionCompilation {
                                                            source: &stage.materialized, bindings: &stage.bindings,
                                                            checked, formals,
                                                        }.validate_inert_target_geometry_v1(budget)?;
                                                        crate::production_ranked_projection_v1::with_source_checked_output_local_relations_v1(
                                                            source, bound, checked, catalog, formals,
                                                            &stage.bindings.typed_descriptor_roots,
                                                            stage.bindings.rustc_target.profile(),
                                                            stage.bindings.rustc_target.device_target(),
                                                            stage.bindings.transaction.compiler_ffi_envelope.as_ref(), budget,
                                                            |local, native, budget| write_output(local, native, writer, budget),
                                                        )
                                                    })();
                                                    result.map_err(|error| {
                                                        callback_error = Some(error);
                                                        Formal::SourceOutput(Output::Invalid(CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1))
                                                    })
                                                },
                                            );
                                            match result {
                                                Err(Join::Formal(Formal::SourceOutput(Output::Invalid(CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1)))) =>
                                                    Err(callback_error.take().unwrap_or_else(|| export_error_v1(ExportResourceV1::Accounting.into()))),
                                                other => other.map_err(|error| ProductionPipelineError::CheckedOutputMemoryTarget(CheckedOutputMemoryTargetErrorV1::Join(error))),
                                            }
                                        },
                                    )
                                },
                            )
                        },
                        |stage| {
                            let MaterializedNeutralProductionCompilation { materialized, ranked_roots, bindings } = stage;
                            drop((materialized, ranked_roots));
                            bindings
                        },
                    ).map_err(Box::new)
                },
            ).map_err(ProductionPipelineError::from)
    }
}

#[cfg(test)]
include!("production_checked_output_export_buffer_v1_tests.rs");
