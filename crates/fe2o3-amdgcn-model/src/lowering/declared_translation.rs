//! Exact source-to-physical custody and writer replay. Records are inert;
//! they do not authenticate compiler origin or establish machine refinement.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, ReusablePhaseCheckLimitsV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredPhysicalCarrierV1 {
    Erased,
    Value(ValueId),
    StaticView { value: ValueId, elements: u64 },
    DynamicView { value: ValueId, elements: ValueId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredPhysicalResultV1 {
    pub source: ValueId,
    pub carrier: DeclaredPhysicalCarrierV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredPhysicalOperationV1 {
    pub function: u32,
    pub block: BlockId,
    pub source_operation: u32,
    pub physical_start: u32,
    pub physical_count: u32,
    pub result_start: u32,
    pub result_count: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeclaredPhysicalTraceV1 {
    operations: Vec<DeclaredPhysicalOperationV1>,
    results: Vec<DeclaredPhysicalResultV1>,
}
impl DeclaredPhysicalTraceV1 {
    pub fn operations(&self) -> &[DeclaredPhysicalOperationV1] {
        &self.operations
    }
    pub fn results(&self) -> &[DeclaredPhysicalResultV1] {
        &self.results
    }
    fn retained_bytes(&self, module: &Module) -> Result<usize, LoweringErrors> {
        sum(
            module,
            bytes::<DeclaredPhysicalOperationV1>(module, self.operations.capacity())?,
            bytes::<DeclaredPhysicalResultV1>(module, self.results.capacity())?,
        )
    }
}

pub(super) struct Builder {
    trace: DeclaredPhysicalTraceV1,
    operation_count: usize,
    result_count: usize,
    retained: usize,
    work: usize,
    remaining_work: usize,
    temporary_bytes: usize,
}
impl Builder {
    pub(super) fn new(
        module: &Module,
        limits: ReusablePhaseCheckLimitsV1,
    ) -> Result<Self, LoweringErrors> {
        let mut operations = 0usize;
        let mut results = 0usize;
        let mut work = 0usize;
        let max_work = limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work);
        for f in &module.functions {
            work = sum(module, work, 1)?;
            if let Some(body) = &f.body {
                for block in &body.blocks {
                    work = sum(module, work, 1)?;
                    for op in &block.operations {
                        operations = sum(module, operations, 1)?;
                        results = sum(module, results, op.results.len())?;
                        work = sum(module, work, sum(module, 2, op.results.len())?)?;
                        if work > max_work {
                            return Err(error(module, "declared projection trace work ceiling"));
                        }
                    }
                }
            }
            if work > max_work {
                return Err(error(module, "declared projection trace work ceiling"));
            }
        }
        let requested = sum(
            module,
            bytes::<DeclaredPhysicalOperationV1>(module, operations)?,
            bytes::<DeclaredPhysicalResultV1>(module, results)?,
        )?;
        let ceiling = limits
            .temporary_bytes
            .min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes);
        if requested > ceiling {
            return Err(error(module, "declared projection trace storage ceiling"));
        }
        let mut trace = DeclaredPhysicalTraceV1 {
            operations: Vec::new(),
            results: Vec::new(),
        };
        trace
            .operations
            .try_reserve_exact(operations)
            .map_err(|_| error(module, "declared projection operation allocation failed"))?;
        let first = bytes::<DeclaredPhysicalOperationV1>(module, trace.operations.capacity())?;
        if sum(
            module,
            first,
            bytes::<DeclaredPhysicalResultV1>(module, results)?,
        )? > ceiling
        {
            return Err(error(
                module,
                "declared projection actual operation capacity ceiling",
            ));
        }
        trace
            .results
            .try_reserve_exact(results)
            .map_err(|_| error(module, "declared projection result allocation failed"))?;
        let retained = sum(
            module,
            first,
            bytes::<DeclaredPhysicalResultV1>(module, trace.results.capacity())?,
        )?;
        if retained > ceiling {
            return Err(error(
                module,
                "declared projection actual result capacity ceiling",
            ));
        }
        Ok(Self {
            trace,
            operation_count: operations,
            result_count: results,
            retained,
            work,
            remaining_work: max_work - work,
            temporary_bytes: ceiling,
        })
    }
    pub(super) fn remaining(
        &self,
        limits: ReusablePhaseCheckLimitsV1,
    ) -> ReusablePhaseCheckLimitsV1 {
        ReusablePhaseCheckLimitsV1 {
            work: limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work) - self.work,
            temporary_bytes: limits
                .temporary_bytes
                .min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes)
                - self.retained,
        }
    }
    pub(super) fn after_phase(&mut self, remaining: ReusablePhaseCheckLimitsV1) {
        self.remaining_work = remaining.work;
    }
    pub(super) fn writer_limits(&self) -> ReusablePhaseCheckLimitsV1 {
        ReusablePhaseCheckLimitsV1 {
            work: self.remaining_work,
            temporary_bytes: self.temporary_bytes - self.retained,
        }
    }
    pub(super) fn result(
        &mut self,
        module: &Module,
        source: ValueId,
        carrier: DeclaredPhysicalCarrierV1,
    ) -> Result<(), LoweringErrors> {
        if self.trace.results.len() >= self.result_count {
            return Err(error(module, "declared projection result census overflow"));
        }
        self.trace
            .results
            .push(DeclaredPhysicalResultV1 { source, carrier });
        Ok(())
    }
    pub(super) fn record(
        &mut self,
        module: &Module,
        function: usize,
        block: BlockId,
        operation: usize,
        start: usize,
        end: usize,
        result_start: usize,
    ) -> Result<(), LoweringErrors> {
        if self.trace.operations.len() >= self.operation_count
            || end < start
            || self.trace.results.len() < result_start
        {
            return Err(error(
                module,
                "declared projection operation census overflow",
            ));
        }
        self.trace.operations.push(DeclaredPhysicalOperationV1 {
            function: index(module, function)?,
            block,
            source_operation: index(module, operation)?,
            physical_start: index(module, start)?,
            physical_count: index(module, end - start)?,
            result_start: index(module, result_start)?,
            result_count: index(module, self.trace.results.len() - result_start)?,
        });
        Ok(())
    }
    pub(super) fn result_count(&self) -> usize {
        self.trace.results.len()
    }
    pub(super) fn finish(self, module: &Module) -> Result<DeclaredPhysicalTraceV1, LoweringErrors> {
        if self.trace.operations.len() != self.operation_count
            || self.trace.results.len() != self.result_count
        {
            return Err(error(
                module,
                "declared projection incomplete source/result census",
            ));
        }
        Ok(self.trace)
    }
}
fn sum(module: &Module, a: usize, b: usize) -> Result<usize, LoweringErrors> {
    a.checked_add(b)
        .ok_or_else(|| error(module, "declared projection accounting overflow"))
}
fn bytes<T>(module: &Module, count: usize) -> Result<usize, LoweringErrors> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| error(module, "declared projection byte overflow"))
}
fn index(module: &Module, value: usize) -> Result<u32, LoweringErrors> {
    u32::try_from(value).map_err(|_| error(module, "declared projection coordinate overflow"))
}
pub(super) fn error(module: &Module, message: &'static str) -> LoweringErrors {
    LoweringErrors::one(
        LoweringLocation::module(module),
        LoweringDiagnosticCode::ResourceLimit,
        message,
    )
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeclaredKirToLlvmReplayV1 {
    version: Version,
    kir_digest: [u8; 32],
    kir_length: u64,
    closure_identity: [u8; 32],
    profile: ProductionAmdTargetProfileV1,
    physical_digest: [u8; 32],
    physical_length: u64,
    llvm_digest: [u8; 32],
    llvm_length: u64,
    projection: DeclaredPhysicalTraceV1,
    structured: Option<StructuredDeclaredKirToLlvmV1>,
}
impl DeclaredKirToLlvmReplayV1 {
    pub const fn version(&self) -> Version {
        self.version
    }
    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }
    pub const fn projection(&self) -> &DeclaredPhysicalTraceV1 {
        &self.projection
    }
    pub const fn structured(&self) -> Option<&StructuredDeclaredKirToLlvmV1> {
        self.structured.as_ref()
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn establishes_machine_refinement(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(super) fn structured_mut_for_test(&mut self) -> &mut StructuredDeclaredKirToLlvmV1 {
        self.structured
            .as_mut()
            .expect("positive complete writer fixture")
    }

    pub(super) fn new(
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1,
        closure: &crate::ProductionTargetCapabilityClosureKirV1,
        profile: ProductionAmdTargetProfileV1,
        physical: &Module,
        llvm: &str,
        projection: DeclaredPhysicalTraceV1,
        structured: Option<StructuredDeclaredKirToLlvmV1>,
        limits: ReusablePhaseCheckLimitsV1,
    ) -> Result<Self, LoweringErrors> {
        let structured_bytes = structured.as_ref().map_or(0, |s| s.retained_bytes());
        let retained = sum(physical, projection.retained_bytes(physical)?, structured_bytes)?;
        let mut remaining_work = limits.work.min(ReusablePhaseCheckLimitsV1::DEFAULT.work);
        let identity = physical_identity(physical, retained, limits.temporary_bytes, &mut remaining_work)?;
        remaining_work.checked_sub(sum(physical, llvm.len(), 1)?)
            .ok_or_else(|| error(physical, "declared replay LLVM digest work ceiling"))?;
        Ok(Self {
            version: owner.version(),
            kir_digest: *owner.identity().digest(),
            kir_length: owner.identity().canonical_length(),
            closure_identity: closure.identity(),
            profile,
            physical_digest: *identity.digest(),
            physical_length: identity.canonical_length(),
            llvm_digest: Sha256::digest(llvm.as_bytes()).into(),
            llvm_length: llvm.len() as u64,
            projection,
            structured,
        })
    }
    pub fn replay(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1,
        epoch: u64,
        launch: &crate::ProductionTargetLaunchEvidenceKirV1,
        profile: ProductionAmdTargetProfileV1,
        llvm: &[u8],
    ) -> Result<(), ProductionV13AmdLoweringErrorV1> {
        if owner.version() != self.version
            || *owner.identity().digest() != self.kir_digest
            || owner.identity().canonical_length() != self.kir_length
            || profile != self.profile
            || llvm.len() as u64 != self.llvm_length
            || <[u8; 32]>::from(Sha256::digest(llvm)) != self.llvm_digest
        {
            return Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch);
        }
        // The existing record stays live while the new trace is reconstructed.
        // Charge both capacities, rather than restarting the trace storage budget.
        let limits = self.replay_limits(ReusablePhaseCheckLimitsV1::DEFAULT)?;
        let output = lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(
            owner, epoch, launch, profile, limits,
        )?;
        if output.capability_closure().identity() != self.closure_identity
            || output.llvm_ir().as_bytes() != llvm
            || output.declared_replay.as_ref() != Some(self)
        {
            return Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch);
        }
        Ok(())
    }
    fn replay_limits(
        &self,
        limits: ReusablePhaseCheckLimitsV1,
    ) -> Result<ReusablePhaseCheckLimitsV1, ProductionV13AmdLoweringErrorV1> {
        let comparison_work = self
            .projection
            .operations
            .len()
            .checked_add(self.projection.results.len())
            .and_then(|n| n.checked_add(self.structured.as_ref().map_or(0, |s| s.segments().len())))
            .ok_or(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)?;
        let work = limits
            .work
            .min(ReusablePhaseCheckLimitsV1::DEFAULT.work)
            .checked_sub(comparison_work)
            .ok_or(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)?;
        let retained = self
            .projection
            .operations
            .capacity()
            .checked_mul(std::mem::size_of::<DeclaredPhysicalOperationV1>())
            .and_then(|n| {
                self.projection
                    .results
                    .capacity()
                    .checked_mul(std::mem::size_of::<DeclaredPhysicalResultV1>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                n.checked_add(self.structured.as_ref().map_or(0, |s| s.retained_bytes()))
            });
        let temporary_bytes = retained
            .and_then(|n| {
                limits
                    .temporary_bytes
                    .min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes)
                    .checked_sub(n)
            })
            .ok_or(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)?;
        Ok(ReusablePhaseCheckLimitsV1 {
            work,
            temporary_bytes,
        })
    }
}

fn physical_identity(
    physical: &Module,
    retained: usize,
    temporary_bytes: usize,
    remaining_work: &mut usize,
) -> Result<fe2o3_kernel_ir::CanonicalModuleDigestV1, LoweringErrors> {
    // Only fixed digest state is retained, not the physical wire image. Existing
    // field encoders and role validation keep their separate codec bounds.
    if sum(physical, retained, fe2o3_kernel_ir::KERNEL_IR_DIGEST_WORKSPACE_BYTES_V1)?
        > temporary_bytes.min(ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes)
    {
        return Err(error(physical, "declared replay digest workspace and retained trace exceed ceiling"));
    }
    fe2o3_kernel_ir::canonical_module_digest_v1(
        physical, Version::V13, fe2o3_kernel_ir::MAX_MODULE_BYTES_V1, remaining_work,
    ).map_err(|error_value| match error_value {
        fe2o3_kernel_ir::KernelIrEncodeError::LimitExceeded { field: "canonical digest work", .. }
        | fe2o3_kernel_ir::KernelIrEncodeError::Overflow { field: "canonical digest role work" } =>
            error(physical, "declared replay canonical digest work ceiling"),
        _ => error(physical, "declared physical replay is not representable as exact physical V13"),
    })
}

#[cfg(test)]
#[path = "declared_translation/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "declared_translation/stream_tests.rs"]
mod stream_tests;
