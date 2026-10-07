//! Independent executable R25 model for one persistent-compute storage bridge.
//!
//! All identities and outcomes are caller-constructed finite model values. The
//! model performs no I/O and does not refine Rust ownership, the runtime, KFD,
//! HSA, HIP, firmware, hardware execution, liveness, parity, or performance.

pub const R25_PERSISTENT_COMPUTE_STORAGE_BRIDGE_SCHEMA_VERSION_V1: u16 = 1;
pub const R25_MAX_STORAGE_BYTES_V1: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct R25PersistentStorageIdentityV1 {
    pub device_id: u64,
    pub vm_id: u64,
    pub allocation_id: u64,
    pub storage_generation: u64,
}

impl R25PersistentStorageIdentityV1 {
    pub const fn is_valid(self) -> bool {
        self.device_id != 0
            && self.vm_id != 0
            && self.allocation_id != 0
            && self.storage_generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25FullStorageRangeV1 {
    pub logical_offset: u64,
    pub logical_bytes: u64,
    pub physical_offset: u64,
    pub physical_bytes: u64,
}

impl R25FullStorageRangeV1 {
    pub const fn is_exact_full_extent(self, storage_bytes: u64) -> bool {
        self.logical_offset == 0
            && self.physical_offset == 0
            && self.logical_bytes == storage_bytes
            && self.physical_bytes == storage_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25ComputeEffectsV1 {
    pub reads_storage: bool,
    pub writes_storage: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25DerivedStorageAuthorizationV1 {
    Read,
    Write,
    ReadWrite,
}

impl R25ComputeEffectsV1 {
    pub const fn derived_authorization(self) -> Option<R25DerivedStorageAuthorizationV1> {
        match (self.reads_storage, self.writes_storage) {
            (true, false) => Some(R25DerivedStorageAuthorizationV1::Read),
            (false, true) => Some(R25DerivedStorageAuthorizationV1::Write),
            (true, true) => Some(R25DerivedStorageAuthorizationV1::ReadWrite),
            (false, false) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25StorageBridgePhaseV1 {
    FullH2dReady,
    PreparedCompute,
    Published,
    Completed,
    Restored,
    Device,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25StorageBridgeQuarantineReasonV1 {
    AmbiguousPublication,
    PostRetentionFault,
    CompletionAuthenticationFailed,
    AmbiguousRestore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25StorageBridgeKeyV1 {
    pub storage: R25PersistentStorageIdentityV1,
    pub operation_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25PrepareComputeRequestV1 {
    pub expected_storage: R25PersistentStorageIdentityV1,
    pub expected_frontier_generation: u64,
    pub range: R25FullStorageRangeV1,
    pub effects: R25ComputeEffectsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25CompletionObservationV1 {
    pub key: R25StorageBridgeKeyV1,
    pub range: R25FullStorageRangeV1,
    pub authorization: R25DerivedStorageAuthorizationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25PublishDispositionV1 {
    Published,
    RetryableNoEffect,
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25CompletionDispositionV1 {
    Pending,
    Completed(R25CompletionObservationV1),
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25RestoreDispositionV1 {
    Restored,
    RetryableNoEffect,
    AmbiguousFailure,
    PostRetentionFault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R25PersistentComputeStorageBridgeErrorV1 {
    InvalidStorage,
    InvalidRange,
    InvalidEffects,
    ReadRequiresInitialization,
    StorageSubstitution,
    StaleGeneration,
    GenerationExhausted,
    IllegalPhase,
    Retryable,
    FastPathFallbackForbidden,
    TerminalQuarantine,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R25PersistentComputeStorageBridgeSnapshotV1 {
    pub storage: R25PersistentStorageIdentityV1,
    pub storage_bytes: u64,
    pub phase: R25StorageBridgePhaseV1,
    pub initialized: bool,
    pub fast_path_selected: bool,
    pub active_generation: Option<u64>,
    pub retired_frontier_generation: u64,
    pub range: Option<R25FullStorageRangeV1>,
    pub authorization: Option<R25DerivedStorageAuthorizationV1>,
    pub completion: Option<R25CompletionObservationV1>,
    pub quarantine_reason: Option<R25StorageBridgeQuarantineReasonV1>,
    pub generic_materialization_count: u64,
}

pub struct R25PersistentComputeStorageBridgeModelV1 {
    state: R25PersistentComputeStorageBridgeSnapshotV1,
}

impl R25PersistentComputeStorageBridgeModelV1 {
    pub fn new_full_h2d_ready_model_only(
        storage: R25PersistentStorageIdentityV1,
        storage_bytes: u64,
    ) -> Result<Self, R25PersistentComputeStorageBridgeErrorV1> {
        Self::new_model_only(
            storage,
            storage_bytes,
            R25StorageBridgePhaseV1::FullH2dReady,
            true,
        )
    }

    pub fn new_quiescent_device_model_only(
        storage: R25PersistentStorageIdentityV1,
        storage_bytes: u64,
        initialized: bool,
    ) -> Result<Self, R25PersistentComputeStorageBridgeErrorV1> {
        Self::new_model_only(
            storage,
            storage_bytes,
            R25StorageBridgePhaseV1::Device,
            initialized,
        )
    }

    fn new_model_only(
        storage: R25PersistentStorageIdentityV1,
        storage_bytes: u64,
        phase: R25StorageBridgePhaseV1,
        initialized: bool,
    ) -> Result<Self, R25PersistentComputeStorageBridgeErrorV1> {
        if !storage.is_valid() || storage_bytes == 0 || storage_bytes > R25_MAX_STORAGE_BYTES_V1 {
            return Err(R25PersistentComputeStorageBridgeErrorV1::InvalidStorage);
        }
        Ok(Self {
            state: R25PersistentComputeStorageBridgeSnapshotV1 {
                storage,
                storage_bytes,
                phase,
                initialized,
                fast_path_selected: false,
                active_generation: None,
                retired_frontier_generation: 0,
                range: None,
                authorization: None,
                completion: None,
                quarantine_reason: None,
                generic_materialization_count: 0,
            },
        })
    }

    pub const fn snapshot(&self) -> R25PersistentComputeStorageBridgeSnapshotV1 {
        self.state
    }

    pub fn prepare_compute_model_only(
        &mut self,
        request: R25PrepareComputeRequestV1,
    ) -> Result<R25StorageBridgeKeyV1, R25PersistentComputeStorageBridgeErrorV1> {
        self.require_not_quarantined()?;
        if !matches!(
            self.state.phase,
            R25StorageBridgePhaseV1::FullH2dReady | R25StorageBridgePhaseV1::Device
        ) {
            return Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase);
        }
        if request.expected_storage != self.state.storage {
            return Err(R25PersistentComputeStorageBridgeErrorV1::StorageSubstitution);
        }
        if request.expected_frontier_generation != self.state.retired_frontier_generation {
            return Err(R25PersistentComputeStorageBridgeErrorV1::StaleGeneration);
        }
        if !request.range.is_exact_full_extent(self.state.storage_bytes) {
            return Err(R25PersistentComputeStorageBridgeErrorV1::InvalidRange);
        }
        let authorization = request
            .effects
            .derived_authorization()
            .ok_or(R25PersistentComputeStorageBridgeErrorV1::InvalidEffects)?;
        if request.effects.reads_storage && !self.state.initialized {
            return Err(R25PersistentComputeStorageBridgeErrorV1::ReadRequiresInitialization);
        }
        let operation_generation = self
            .state
            .retired_frontier_generation
            .checked_add(1)
            .filter(|generation| *generation != 0)
            .ok_or(R25PersistentComputeStorageBridgeErrorV1::GenerationExhausted)?;

        self.state.phase = R25StorageBridgePhaseV1::PreparedCompute;
        self.state.fast_path_selected = true;
        self.state.active_generation = Some(operation_generation);
        self.state.range = Some(request.range);
        self.state.authorization = Some(authorization);
        self.state.completion = None;
        Ok(R25StorageBridgeKeyV1 {
            storage: self.state.storage,
            operation_generation,
        })
    }

    pub fn publish_model_only(
        &mut self,
        key: R25StorageBridgeKeyV1,
        disposition: R25PublishDispositionV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.require_active_key(key)?;
        if self.state.phase != R25StorageBridgePhaseV1::PreparedCompute {
            return Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase);
        }
        match disposition {
            R25PublishDispositionV1::Published => {
                self.state.phase = R25StorageBridgePhaseV1::Published;
                Ok(())
            }
            R25PublishDispositionV1::RetryableNoEffect => {
                Err(R25PersistentComputeStorageBridgeErrorV1::Retryable)
            }
            R25PublishDispositionV1::AmbiguousFailure => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::AmbiguousPublication)
            }
            R25PublishDispositionV1::PostRetentionFault => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::PostRetentionFault)
            }
        }
    }

    pub fn observe_completion_model_only(
        &mut self,
        key: R25StorageBridgeKeyV1,
        disposition: R25CompletionDispositionV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.require_active_key(key)?;
        if self.state.phase != R25StorageBridgePhaseV1::Published {
            return Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase);
        }
        match disposition {
            R25CompletionDispositionV1::Pending => Ok(()),
            R25CompletionDispositionV1::Completed(observation) => {
                if observation.key != key
                    || Some(observation.range) != self.state.range
                    || Some(observation.authorization) != self.state.authorization
                {
                    return self.quarantine(
                        R25StorageBridgeQuarantineReasonV1::CompletionAuthenticationFailed,
                    );
                }
                self.state.phase = R25StorageBridgePhaseV1::Completed;
                self.state.completion = Some(observation);
                Ok(())
            }
            R25CompletionDispositionV1::AmbiguousFailure => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::PostRetentionFault)
            }
            R25CompletionDispositionV1::PostRetentionFault => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::PostRetentionFault)
            }
        }
    }

    pub fn restore_model_only(
        &mut self,
        key: R25StorageBridgeKeyV1,
        disposition: R25RestoreDispositionV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.require_active_key(key)?;
        if self.state.phase != R25StorageBridgePhaseV1::Completed {
            return Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase);
        }
        match disposition {
            R25RestoreDispositionV1::Restored => {
                self.state.phase = R25StorageBridgePhaseV1::Restored;
                Ok(())
            }
            R25RestoreDispositionV1::RetryableNoEffect => {
                Err(R25PersistentComputeStorageBridgeErrorV1::Retryable)
            }
            R25RestoreDispositionV1::AmbiguousFailure => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::AmbiguousRestore)
            }
            R25RestoreDispositionV1::PostRetentionFault => {
                self.quarantine(R25StorageBridgeQuarantineReasonV1::PostRetentionFault)
            }
        }
    }

    pub fn retire_exact_frontier_model_only(
        &mut self,
        key: R25StorageBridgeKeyV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.require_active_key(key)?;
        if self.state.phase != R25StorageBridgePhaseV1::Restored {
            return Err(R25PersistentComputeStorageBridgeErrorV1::IllegalPhase);
        }
        let active_generation = self
            .state
            .active_generation
            .ok_or(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation)?;
        active_generation
            .checked_add(1)
            .filter(|generation| *generation != 0)
            .ok_or(R25PersistentComputeStorageBridgeErrorV1::GenerationExhausted)?;
        let writes_storage = matches!(
            self.state.authorization,
            Some(
                R25DerivedStorageAuthorizationV1::Write
                    | R25DerivedStorageAuthorizationV1::ReadWrite
            )
        );

        self.state.phase = R25StorageBridgePhaseV1::Device;
        self.state.initialized |= writes_storage;
        self.state.fast_path_selected = false;
        self.state.active_generation = None;
        self.state.retired_frontier_generation = active_generation;
        self.state.range = None;
        self.state.authorization = None;
        self.state.completion = None;
        Ok(())
    }

    /// The selected bridge never falls back to the generic materialization path.
    pub fn attempt_generic_materialization_model_only(
        &mut self,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        Err(R25PersistentComputeStorageBridgeErrorV1::FastPathFallbackForbidden)
    }

    pub fn validate_global_invariants(
        &self,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        if !self.state.storage.is_valid()
            || self.state.storage_bytes == 0
            || self.state.storage_bytes > R25_MAX_STORAGE_BYTES_V1
            || self.state.generic_materialization_count != 0
        {
            return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
        }
        let active = self.state.active_generation.is_some();
        let has_operation = self.state.range.is_some() && self.state.authorization.is_some();
        match self.state.phase {
            R25StorageBridgePhaseV1::FullH2dReady | R25StorageBridgePhaseV1::Device => {
                if active
                    || has_operation
                    || self.state.fast_path_selected
                    || self.state.completion.is_some()
                    || self.state.quarantine_reason.is_some()
                {
                    return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
                }
            }
            R25StorageBridgePhaseV1::PreparedCompute | R25StorageBridgePhaseV1::Published => {
                if !active
                    || !has_operation
                    || !self.state.fast_path_selected
                    || self.state.completion.is_some()
                    || self.state.quarantine_reason.is_some()
                {
                    return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
                }
            }
            R25StorageBridgePhaseV1::Completed | R25StorageBridgePhaseV1::Restored => {
                let Some(completion) = self.state.completion else {
                    return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
                };
                if !active
                    || !has_operation
                    || !self.state.fast_path_selected
                    || self.state.quarantine_reason.is_some()
                    || Some(completion.range) != self.state.range
                    || Some(completion.authorization) != self.state.authorization
                    || completion.key.storage != self.state.storage
                    || Some(completion.key.operation_generation) != self.state.active_generation
                {
                    return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
                }
            }
            R25StorageBridgePhaseV1::Quarantined => {
                if !active
                    || !has_operation
                    || !self.state.fast_path_selected
                    || self.state.quarantine_reason.is_none()
                {
                    return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
                }
            }
        }
        if let Some(range) = self.state.range
            && !range.is_exact_full_extent(self.state.storage_bytes)
        {
            return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
        }
        if matches!(
            self.state.authorization,
            Some(
                R25DerivedStorageAuthorizationV1::Read
                    | R25DerivedStorageAuthorizationV1::ReadWrite
            )
        ) && !self.state.initialized
        {
            return Err(R25PersistentComputeStorageBridgeErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn require_not_quarantined(&self) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        if self.state.phase == R25StorageBridgePhaseV1::Quarantined {
            Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
        } else {
            Ok(())
        }
    }

    fn require_active_key(
        &self,
        key: R25StorageBridgeKeyV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.require_not_quarantined()?;
        if key.storage != self.state.storage {
            return Err(R25PersistentComputeStorageBridgeErrorV1::StorageSubstitution);
        }
        if Some(key.operation_generation) != self.state.active_generation {
            return Err(R25PersistentComputeStorageBridgeErrorV1::StaleGeneration);
        }
        Ok(())
    }

    fn quarantine(
        &mut self,
        reason: R25StorageBridgeQuarantineReasonV1,
    ) -> Result<(), R25PersistentComputeStorageBridgeErrorV1> {
        self.state.phase = R25StorageBridgePhaseV1::Quarantined;
        self.state.quarantine_reason = Some(reason);
        Err(R25PersistentComputeStorageBridgeErrorV1::TerminalQuarantine)
    }
}
