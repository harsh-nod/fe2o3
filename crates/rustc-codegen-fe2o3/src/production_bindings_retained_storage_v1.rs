//! Complete named ExtractionOnly bindings checkpoint, NOT whole-stage/action memory.
//!
//! Counts one bindings header and the owned logical payload of all eleven
//! fields. Existing branch conventions include logical BTree key/value payload,
//! not physical tree nodes. Vec/String capacities and independent deep clones
//! are retained payload; allocator metadata, peak/RSS, adjacent semantic owners,
//! rustc caches/arenas and Pliron contexts are not covered by this named root.
//! They are NOT thereby excluded from a future whole-action budget.
//!
//! ProtectedV3 is refused: its original custody owners are not yet observed.
//! Observation limits are caller supplied, not constructor/admission caps.
use super::{
    AuthenticatedProductionBindings, ProductionCompilerCustody, ProductionTransactionBindings,
};
use crate::artifact_transaction::ProducerIdentity;
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerFfiLogicalStorageErrorV1, CompilerFfiLogicalStorageLimitsV1,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1, LogicalStorageLimitsV1};
use fe2o3_verifier::portable_reference_v1::retained_storage_v1::ReferenceRetainedStorageErrorV1;
use std::{error::Error, fmt, mem::size_of, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BindingsRetainedStorageV1 {
    pub(crate) header_bytes: usize,
    pub(crate) heap_bytes: usize,
    pub(crate) total_bytes: usize,
    pub(crate) visited_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BindingsStorageErrorV1 {
    Counter(LogicalStorageErrorV1),
    ReferenceExpressionDepthLimit,
    UnsupportedProtectedCustody,
}
impl From<LogicalStorageErrorV1> for BindingsStorageErrorV1 {
    fn from(value: LogicalStorageErrorV1) -> Self {
        Self::Counter(value)
    }
}
impl From<CompilerFfiLogicalStorageErrorV1> for BindingsStorageErrorV1 {
    fn from(value: CompilerFfiLogicalStorageErrorV1) -> Self {
        Self::Counter(match value {
            CompilerFfiLogicalStorageErrorV1::Arithmetic => LogicalStorageErrorV1::Arithmetic,
            CompilerFfiLogicalStorageErrorV1::ByteLimit => LogicalStorageErrorV1::ByteLimit,
            CompilerFfiLogicalStorageErrorV1::ItemLimit => LogicalStorageErrorV1::ItemLimit,
        })
    }
}
impl From<ReferenceRetainedStorageErrorV1> for BindingsStorageErrorV1 {
    fn from(value: ReferenceRetainedStorageErrorV1) -> Self {
        match value {
            ReferenceRetainedStorageErrorV1::Counter(error) => Self::Counter(error),
            ReferenceRetainedStorageErrorV1::ExpressionDepthLimit => {
                Self::ReferenceExpressionDepthLimit
            }
        }
    }
}
impl fmt::Display for BindingsStorageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bindings retained-storage observation refused: {self:?}")
    }
}
impl Error for BindingsStorageErrorV1 {}

type StorageResult = Result<(), BindingsStorageErrorV1>;

/// Limits and counter are created together and never reset. Existing KIR child
/// walkers receive this SAME counter. Only the FFI API has a delegated ledger.
struct Account {
    limits: LogicalStorageLimitsV1,
    counter: LogicalStorageCounterV1,
    header: usize,
}
impl Account {
    fn new(header: usize, limits: LogicalStorageLimitsV1) -> Result<Self, BindingsStorageErrorV1> {
        let mut counter = LogicalStorageCounterV1::new(limits);
        counter.charge(header, 1)?;
        Ok(Self {
            limits,
            counter,
            header,
        })
    }
    fn visit(&mut self) -> StorageResult {
        self.counter.charge(0, 1)?;
        Ok(())
    }
    fn extent(&mut self, count: usize, width: usize) -> StorageResult {
        let bytes = count
            .checked_mul(width)
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        self.counter.charge(bytes, 1)?;
        Ok(())
    }
    fn extraction_only(&mut self, custody: &ProductionCompilerCustody) -> StorageResult {
        self.visit()?;
        match custody {
            ProductionCompilerCustody::ExtractionOnly => Ok(()),
            ProductionCompilerCustody::ProtectedV3 {
                invocation: _,
                attempt: _,
            } => Err(BindingsStorageErrorV1::UnsupportedProtectedCustody),
        }
    }
    fn ffi(&mut self, envelope: Option<&CompilerFfiEnvelopeV1>) -> StorageResult {
        // Charge the Option inspection even for None. Derive all allowances
        // before entering the independent, already-bounded FFI owner walker.
        self.visit()?;
        let Some(envelope) = envelope else {
            return Ok(());
        };
        let remaining_items = self
            .limits
            .max_items
            .checked_sub(self.counter.items())
            .ok_or(LogicalStorageErrorV1::Arithmetic)?;
        let local_bytes = match self.limits.max_bytes {
            Some(limit) => Some(
                limit
                    .checked_sub(self.counter.bytes())
                    .and_then(|remaining| remaining.checked_add(size_of::<CompilerFfiEnvelopeV1>()))
                    .ok_or(LogicalStorageErrorV1::Arithmetic)?,
            ),
            None => None,
        };
        let report = envelope.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
            max_bytes: local_bytes,
            max_items: remaining_items,
        })?;
        // Its standalone root header is ALREADY in the bindings header.
        // Only successful heap/items are merged. An FFI failure invalidates
        // the entire observation; its internal partial prefix is unavailable.
        self.counter
            .charge(report.heap_bytes(), report.visited_items())?;
        Ok(())
    }
    fn transaction_payload(
        &mut self,
        producer: &ProducerIdentity,
        output_dir: &PathBuf,
        envelope: Option<&CompilerFfiEnvelopeV1>,
    ) -> StorageResult {
        self.visit()?;
        producer.visit_retained_heap_storage_v1(|n, w| self.extent(n, w))?;
        // PathBuf::capacity is retained native OsString backing bytes. Do not
        // substitute UTF-8 length, lossy conversion, serialization or a clone.
        self.extent(output_dir.capacity(), size_of::<u8>())?;
        self.ffi(envelope)
    }
    fn finish(self) -> Result<BindingsRetainedStorageV1, BindingsStorageErrorV1> {
        Ok(BindingsRetainedStorageV1 {
            header_bytes: self.header,
            heap_bytes: self
                .counter
                .bytes()
                .checked_sub(self.header)
                .ok_or(LogicalStorageErrorV1::Arithmetic)?,
            total_bytes: self.counter.bytes(),
            visited_items: self.counter.items(),
        })
    }
}

impl AuthenticatedProductionBindings {
    /// Observe the actual owner without cloning, reconstructing or re-admitting
    /// it. Success is a complete named ExtractionOnly bindings report only.
    /// First failure produces no successful report. Earlier counter prefixes
    /// are diagnostics, not a zero value or complete storage observation.
    pub(super) fn logical_retained_storage_v1(
        &self,
        limits: LogicalStorageLimitsV1,
    ) -> Result<BindingsRetainedStorageV1, BindingsStorageErrorV1> {
        let mut account = Account::new(size_of::<Self>(), limits)?;
        let Self {
            context_entries: _,
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            reference_effect_bindings: _,
            debug_source_files: _,
            debug_source_scopes: _,
            debug_source_variables: _,
            debug_capture_gap: _,
            typed_descriptor_roots: _,
            transaction,
        } = self;
        let ProductionTransactionBindings {
            producer,
            output_dir,
            compiler_ffi_envelope,
            compiler_custody,
        } = transaction;
        account.extraction_only(compiler_custody)?;
        self.charge_context_retained_heap_v1(&mut account.counter)?;
        account.visit()?;
        rustc_identity_inventory.visit_retained_heap_storage_v1(|n, w| account.extent(n, w))?;
        account.visit()?;
        rustc_preflight_plan.visit_retained_heap_storage_v1(|n, w| account.extent(n, w))?;
        account.visit()?;
        rustc_target.visit_retained_heap_storage_v1(|n, w| account.extent(n, w))?;
        self.charge_reference_effect_retained_heap_v1(&mut account.counter)?;
        // One invocation covers the four debug fields, including optional gap.
        self.charge_debug_source_retained_heap_v1(&mut account.counter)?;
        self.charge_typed_descriptor_retained_heap_v1(&mut account.counter)?;
        account.transaction_payload(producer, output_dir, compiler_ffi_envelope.as_ref())?;
        account.finish()
    }
}

#[cfg(test)]
#[path = "production_bindings_storage_checkpoint_v1_tests.rs"]
pub(crate) mod checkpoint;
#[cfg(test)]
#[path = "production_bindings_retained_storage_v1_tests.rs"]
mod tests;
