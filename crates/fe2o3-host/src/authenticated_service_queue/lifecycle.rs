/// Currentness rejection paired with an unchanged typestate owner.
#[must_use = "currentness rejection retains the exact queue owner"]
pub struct AuthenticatedServiceCurrentnessFailureV1<T> {
    error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
    retained: Box<T>,
}

impl<T> AuthenticatedServiceCurrentnessFailureV1<T> {
    /// Returns the exact currentness error.
    pub const fn error(&self) -> &AuthenticatedWorkerV3ProgramMaterializationErrorV1 {
        &self.error
    }

    /// Returns the error and unchanged retained owner.
    pub fn into_parts(self) -> (AuthenticatedWorkerV3ProgramMaterializationErrorV1, T) {
        (*self.error, *self.retained)
    }
}

impl<T> fmt::Debug for AuthenticatedServiceCurrentnessFailureV1<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceCurrentnessFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Live unbound queue retaining every previously active authenticated program set.
#[must_use = "the authenticated unbound queue must be rebound, rolled over, or released"]
pub struct AuthenticatedServiceQueueUnboundSessionV1 {
    queue: ServiceQueueUnboundSessionV1,
    programs: AuthenticatedProgramCustodyV1,
}

impl fmt::Debug for AuthenticatedServiceQueueUnboundSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueUnboundSessionV1")
            .field("queue", &self.queue)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedServiceQueueUnboundSessionV1 {
    /// Returns a redacted observation of the still-live native queue.
    pub const fn observation(&self) -> fe2o3_kfd::ComputeAqlQueueObservationV1 {
        self.queue.observation()
    }

    /// Returns the completed dispatch generation that authorized detachment.
    pub const fn detached_dispatch_generation(&self) -> u64 {
        self.queue.detached_dispatch_generation()
    }

    /// Replaces one retained partitioned device-local allocation without changing program custody.
    pub fn replace_initialized_partitioned_device_local<R, const OLD_N: usize, const NEW_N: usize>(
        self,
        old: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, OLD_N>,
        bytes: Box<[u8]>,
        alignment: u64,
        content: fe2o3_kfd::Gfx942DeviceContentDescriptorV1,
        new_members: [(u64, u64, u64); NEW_N],
    ) -> Result<
        AuthenticatedServiceQueuePartitionedDataUpdateV1<R, NEW_N>,
        AuthenticatedServiceQueueDataUpdateFailureV1,
    >
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let Self { queue, programs } = self;
        match queue.replace_initialized_partitioned_device_local::<R, OLD_N, NEW_N>(
            old,
            bytes,
            alignment,
            content,
            new_members,
        ) {
            Ok(inner) => Ok(AuthenticatedServiceQueuePartitionedDataUpdateV1 { inner, programs }),
            Err(ServiceQueueDataUpdateFailureV1::Rejected { error, queue }) => {
                Err(AuthenticatedServiceQueueDataUpdateFailureV1::Rejected {
                    error: Box::new(error),
                    queue: Box::new(Self {
                        queue: *queue,
                        programs,
                    }),
                })
            }
            Err(ServiceQueueDataUpdateFailureV1::Terminal { error, retained }) => {
                Err(AuthenticatedServiceQueueDataUpdateFailureV1::Quarantined {
                    error: Box::new(error),
                    retained: Box::new(AuthenticatedQuarantinedServiceQueueV1 {
                        queue: *retained,
                        programs,
                    }),
                })
            }
        }
    }

    /// Replaces one retained host-visible allocation without changing program custody.
    pub fn replace_initialized_host_visible<R>(
        self,
        old: ServiceHostDispatchRangeV1,
        bytes: Box<[u8]>,
    ) -> Result<
        AuthenticatedServiceQueueHostDataUpdateV1,
        AuthenticatedServiceQueueDataUpdateFailureV1,
    >
    where
        R: fe2o3_service_host::HostAllocationRoleMarkerV1,
    {
        let Self { queue, programs } = self;
        match queue.replace_initialized_host_visible::<R>(old, bytes) {
            Ok(inner) => Ok(AuthenticatedServiceQueueHostDataUpdateV1 { inner, programs }),
            Err(ServiceQueueDataUpdateFailureV1::Rejected { error, queue }) => {
                Err(AuthenticatedServiceQueueDataUpdateFailureV1::Rejected {
                    error: Box::new(error),
                    queue: Box::new(Self {
                        queue: *queue,
                        programs,
                    }),
                })
            }
            Err(ServiceQueueDataUpdateFailureV1::Terminal { error, retained }) => {
                Err(AuthenticatedServiceQueueDataUpdateFailureV1::Quarantined {
                    error: Box::new(error),
                    retained: Box::new(AuthenticatedQuarantinedServiceQueueV1 {
                        queue: *retained,
                        programs,
                    }),
                })
            }
        }
    }

    /// Revalidates and rebinds the retained active program set to the same native queue.
    pub fn bind_retained<const N: usize>(
        self,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Result<
        AuthenticatedServiceQueueSessionV1<N>,
        AuthenticatedServiceQueueRetainedBindFailureV1<N>,
    > {
        let Self {
            queue,
            mut programs,
        } = self;
        let retained = programs.take_most_recent_retired();
        match (Self { queue, programs }).bind(retained, packets) {
            Ok(queue) => Ok(queue),
            Err(AuthenticatedServiceQueueBindFailureV1::Program {
                error,
                queue,
                replacement,
                packets,
            }) => {
                let Self {
                    queue,
                    mut programs,
                } = *queue;
                programs.restore_most_recent_retired(replacement);
                Err(AuthenticatedServiceQueueRetainedBindFailureV1::Program {
                    error,
                    queue: Box::new(Self { queue, programs }),
                    packets,
                })
            }
            Err(AuthenticatedServiceQueueBindFailureV1::QueueRejected {
                error,
                queue,
                replacement,
                packets,
            }) => {
                let Self {
                    queue,
                    mut programs,
                } = *queue;
                programs.restore_most_recent_retired(replacement);
                Err(
                    AuthenticatedServiceQueueRetainedBindFailureV1::QueueRejected {
                        error,
                        queue: Box::new(Self { queue, programs }),
                        packets,
                    },
                )
            }
            Err(AuthenticatedServiceQueueBindFailureV1::Quarantined { error, retained }) => {
                Err(AuthenticatedServiceQueueRetainedBindFailureV1::Quarantined { error, retained })
            }
        }
    }

    /// Revalidates the retained active program set while replacing the native queue.
    pub fn rollover_retained<const N: usize>(
        self,
        ring_bytes: u32,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Result<
        AuthenticatedServiceQueueRolloverSuccessV1<N>,
        AuthenticatedServiceQueueRetainedRolloverFailureV1<N>,
    > {
        let Self {
            queue,
            mut programs,
        } = self;
        let retained = programs.take_most_recent_retired();
        match (Self { queue, programs }).rollover(ring_bytes, retained, packets) {
            Ok(success) => Ok(success),
            Err(AuthenticatedServiceQueueRolloverFailureV1::Program {
                error,
                queue,
                replacement,
                packets,
            }) => {
                let Self {
                    queue,
                    mut programs,
                } = *queue;
                programs.restore_most_recent_retired(replacement);
                Err(
                    AuthenticatedServiceQueueRetainedRolloverFailureV1::Program {
                        error,
                        queue: Box::new(Self { queue, programs }),
                        packets,
                    },
                )
            }
            Err(AuthenticatedServiceQueueRolloverFailureV1::QueueRejected {
                error,
                queue,
                replacement,
                packets,
            }) => {
                let Self {
                    queue,
                    mut programs,
                } = *queue;
                programs.restore_most_recent_retired(replacement);
                Err(
                    AuthenticatedServiceQueueRetainedRolloverFailureV1::QueueRejected {
                        error,
                        queue: Box::new(Self { queue, programs }),
                        packets,
                    },
                )
            }
            Err(AuthenticatedServiceQueueRolloverFailureV1::Terminal {
                error,
                previous_queue_destroyed,
                previous_dispatch_generation,
                retained,
            }) => Err(
                AuthenticatedServiceQueueRetainedRolloverFailureV1::Terminal {
                    error,
                    previous_queue_destroyed,
                    previous_dispatch_generation,
                    retained,
                },
            ),
        }
    }

    /// Rebinds authenticated replacement programs to the same native queue.
    pub fn bind<const N: usize>(
        self,
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Result<AuthenticatedServiceQueueSessionV1<N>, AuthenticatedServiceQueueBindFailureV1<N>>
    {
        let derived = match replacement.derive_programs() {
            Ok(derived) => derived,
            Err(error) => {
                return Err(AuthenticatedServiceQueueBindFailureV1::Program {
                    error: Box::new(error),
                    queue: Box::new(self),
                    replacement,
                    packets: Box::new(packets),
                });
            }
        };
        let Self {
            queue,
            mut programs,
        } = self;
        let batch = ServiceFixedBatchV1::new(derived, packets);
        match queue.bind(batch) {
            Ok(queue) => {
                programs.install_active(replacement);
                Ok(AuthenticatedServiceQueueSessionV1 { queue, programs })
            }
            Err(ServiceQueueBindFailureV1::Rejected {
                error,
                queue,
                batch,
            }) => {
                let (_derived, packets) = (*batch).into_parts();
                Err(AuthenticatedServiceQueueBindFailureV1::QueueRejected {
                    error: Box::new(error),
                    queue: Box::new(Self {
                        queue: *queue,
                        programs,
                    }),
                    replacement,
                    packets: Box::new(packets),
                })
            }
            Err(ServiceQueueBindFailureV1::Terminal { error, retained }) => {
                programs.install_active(replacement);
                Err(AuthenticatedServiceQueueBindFailureV1::Quarantined {
                    error: Box::new(error),
                    retained: Box::new(AuthenticatedQuarantinedServiceQueueV1 {
                        queue: *retained,
                        programs,
                    }),
                })
            }
        }
    }

    /// Revalidates and installs authenticated programs while replacing the native queue.
    pub fn rollover<const N: usize>(
        self,
        ring_bytes: u32,
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        packets: [ServiceFixedDispatchPacketV1; N],
    ) -> Result<
        AuthenticatedServiceQueueRolloverSuccessV1<N>,
        AuthenticatedServiceQueueRolloverFailureV1<N>,
    > {
        let derived = match replacement.derive_programs() {
            Ok(derived) => derived,
            Err(error) => {
                return Err(AuthenticatedServiceQueueRolloverFailureV1::Program {
                    error: Box::new(error),
                    queue: Box::new(self),
                    replacement,
                    packets: Box::new(packets),
                });
            }
        };
        let Self {
            queue,
            mut programs,
        } = self;
        let batch = ServiceFixedBatchV1::new(derived, packets);
        match queue.rollover(ring_bytes, batch) {
            Ok(inner) => {
                programs.install_active(replacement);
                Ok(AuthenticatedServiceQueueRolloverSuccessV1 { inner, programs })
            }
            Err(ServiceQueueRolloverFailureV1::Rejected {
                error,
                queue,
                batch,
            }) => {
                let (_derived, packets) = (*batch).into_parts();
                Err(AuthenticatedServiceQueueRolloverFailureV1::QueueRejected {
                    error: Box::new(error),
                    queue: Box::new(Self {
                        queue: *queue,
                        programs,
                    }),
                    replacement,
                    packets: Box::new(packets),
                })
            }
            Err(ServiceQueueRolloverFailureV1::Terminal {
                error,
                previous_queue_destroyed,
                previous_dispatch_generation,
            }) => {
                programs.install_active(replacement);
                Err(AuthenticatedServiceQueueRolloverFailureV1::Terminal {
                    error: Box::new(error),
                    previous_queue_destroyed,
                    previous_dispatch_generation,
                    retained: AuthenticatedServiceTerminalProgramCustodyV1 { programs },
                })
            }
        }
    }

    /// Destroys the unbound queue and returns every retained program set.
    pub fn destroy_and_release(
        self,
    ) -> Result<AuthenticatedServiceQueueReleaseV1, AuthenticatedServiceQueueReleaseFailureV1> {
        let Self { queue, programs } = self;
        finish_release(queue.destroy_and_release(), programs)
    }
}

/// Fresh authenticated queue and partition custody after detached replacement.
#[must_use = "the authenticated queue and fresh partition custody must remain retained"]
pub struct AuthenticatedServiceQueuePartitionedDataUpdateV1<R, const N: usize>
where
    R: DeviceAllocationRoleMarkerV1,
{
    inner: ServiceQueuePartitionedDataUpdateV1<R, N>,
    programs: AuthenticatedProgramCustodyV1,
}

impl<R, const N: usize> AuthenticatedServiceQueuePartitionedDataUpdateV1<R, N>
where
    R: DeviceAllocationRoleMarkerV1,
{
    /// Separates the authenticated queue, fresh partition witness, and exact ranges.
    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedServiceQueueUnboundSessionV1,
        ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
        [ServiceDeviceDispatchRangeV1; N],
    ) {
        let (queue, subleases, ranges) = self.inner.into_parts();
        (
            AuthenticatedServiceQueueUnboundSessionV1 {
                queue,
                programs: self.programs,
            },
            subleases,
            ranges,
        )
    }
}

impl<R, const N: usize> fmt::Debug for AuthenticatedServiceQueuePartitionedDataUpdateV1<R, N>
where
    R: DeviceAllocationRoleMarkerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueuePartitionedDataUpdateV1")
            .field("inner", &self.inner)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Fresh authenticated queue and host-visible range custody after replacement.
#[must_use = "the authenticated queue and fresh host-visible range must remain retained"]
pub struct AuthenticatedServiceQueueHostDataUpdateV1 {
    inner: ServiceQueueHostDataUpdateV1,
    programs: AuthenticatedProgramCustodyV1,
}

impl AuthenticatedServiceQueueHostDataUpdateV1 {
    /// Separates the authenticated queue, fresh range, and initialized snapshot.
    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedServiceQueueUnboundSessionV1,
        ServiceHostDispatchRangeV1,
        Option<ServiceHostDispatchSnapshotRangeV1>,
    ) {
        let (queue, range, snapshot) = self.inner.into_parts();
        (
            AuthenticatedServiceQueueUnboundSessionV1 {
                queue,
                programs: self.programs,
            },
            range,
            snapshot,
        )
    }
}

impl fmt::Debug for AuthenticatedServiceQueueHostDataUpdateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueHostDataUpdateV1")
            .field("inner", &self.inner)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Detached-data update rejection or terminal authenticated queue quarantine.
#[must_use = "authenticated data-update failure preserves every available owner"]
pub enum AuthenticatedServiceQueueDataUpdateFailureV1 {
    /// Validation rejected the replacement before native mutation.
    Rejected {
        /// Exact lower rejection.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged unbound authenticated queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
    },
    /// Native replacement became ambiguous and retry is forbidden.
    Quarantined {
        /// Exact lower transition error.
        error: Box<ServiceQueueErrorV1>,
        /// Opaque queue plus every authenticated roster owner.
        retained: Box<AuthenticatedQuarantinedServiceQueueV1>,
    },
}

impl AuthenticatedServiceQueueDataUpdateFailureV1 {
    /// Returns the exact failure without discarding retained custody.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        match self {
            Self::Rejected { error, .. } | Self::Quarantined { error, .. } => error,
        }
    }
}

impl fmt::Debug for AuthenticatedServiceQueueDataUpdateFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { error, .. } => formatter
                .debug_struct("Rejected")
                .field("error", error)
                .finish_non_exhaustive(),
            Self::Quarantined { error, retained } => formatter
                .debug_struct("Quarantined")
                .field("error", error)
                .field("retained", retained)
                .finish(),
        }
    }
}

/// Retained-program rebind rejection or terminal native transition failure.
#[must_use = "retained authenticated rebind failure preserves every available owner"]
pub enum AuthenticatedServiceQueueRetainedBindFailureV1<const N: usize> {
    /// Currentness or exact program derivation failed before KFD mutation.
    Program {
        /// Exact derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged unbound authenticated queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Structural preflight rejected unchanged inputs.
    QueueRejected {
        /// Exact structural queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged unbound authenticated queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Native replacement became ambiguous and all available custody is quarantined.
    Quarantined {
        /// Exact queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Opaque queue plus every authenticated roster owner.
        retained: Box<AuthenticatedQuarantinedServiceQueueV1>,
    },
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueRetainedBindFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => formatter.debug_tuple("Program").field(error).finish(),
            Self::QueueRejected { error, .. } => {
                formatter.debug_tuple("QueueRejected").field(error).finish()
            }
            Self::Quarantined { error, .. } => {
                formatter.debug_tuple("Quarantined").field(error).finish()
            }
        }
    }
}

/// Retained-program rollover rejection or terminal native replacement failure.
#[must_use = "retained authenticated rollover failure preserves every available owner"]
pub enum AuthenticatedServiceQueueRetainedRolloverFailureV1<const N: usize> {
    /// Currentness or exact program derivation failed before native destruction.
    Program {
        /// Exact derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged old unbound queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Structural preflight rejected unchanged inputs.
    QueueRejected {
        /// Exact structural error.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged old unbound queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Native rollover consumed the queue; program owners remain retained.
    Terminal {
        /// Exact native error.
        error: Box<ServiceQueueErrorV1>,
        /// Confirmed predecessor destruction, when observed.
        previous_queue_destroyed: Option<fe2o3_kfd::ComputeAqlQueueDestroyedV1>,
        /// Exact predecessor dispatch generation.
        previous_dispatch_generation: u64,
        /// Every retained authenticated program set.
        retained: AuthenticatedServiceTerminalProgramCustodyV1,
    },
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueRetainedRolloverFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => formatter.debug_tuple("Program").field(error).finish(),
            Self::QueueRejected { error, .. } => {
                formatter.debug_tuple("QueueRejected").field(error).finish()
            }
            Self::Terminal {
                error,
                previous_queue_destroyed,
                previous_dispatch_generation,
                ..
            } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .field("previous_queue_destroyed", previous_queue_destroyed)
                .field("previous_dispatch_generation", previous_dispatch_generation)
                .finish_non_exhaustive(),
        }
    }
}

/// Rebind failure retaining all recoverable queue and program inputs.
#[must_use = "authenticated rebind failure retains queue and program custody"]
pub enum AuthenticatedServiceQueueBindFailureV1<const N: usize> {
    /// Currentness or exact program derivation failed before KFD mutation.
    Program {
        /// Exact derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged unbound queue and historical custody.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged replacement program set.
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Structural preflight rejected unchanged inputs.
    QueueRejected {
        /// Exact structural queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged unbound authenticated queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged replacement program set.
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Native replacement became ambiguous and all available custody is quarantined.
    Quarantined {
        /// Exact queue error.
        error: Box<ServiceQueueErrorV1>,
        /// Opaque queue plus every old and replacement roster owner.
        retained: Box<AuthenticatedQuarantinedServiceQueueV1>,
    },
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueBindFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => formatter.debug_tuple("Program").field(error).finish(),
            Self::QueueRejected { error, .. } => {
                formatter.debug_tuple("QueueRejected").field(error).finish()
            }
            Self::Quarantined { error, .. } => {
                formatter.debug_tuple("Quarantined").field(error).finish()
            }
        }
    }
}

/// Successful authenticated quiescent queue rollover.
#[must_use = "the replacement authenticated queue requires an explicit transition"]
pub struct AuthenticatedServiceQueueRolloverSuccessV1<const N: usize> {
    inner: ServiceQueueRolloverSuccessV1<N>,
    programs: AuthenticatedProgramCustodyV1,
}

impl<const N: usize> AuthenticatedServiceQueueRolloverSuccessV1<N> {
    /// Returns confirmed predecessor queue destruction.
    pub const fn previous_queue_destroyed(&self) -> fe2o3_kfd::ComputeAqlQueueDestroyedV1 {
        self.inner.previous_queue_destroyed()
    }

    /// Returns the predecessor dispatch generation.
    pub const fn previous_dispatch_generation(&self) -> u64 {
        self.inner.previous_dispatch_generation()
    }

    /// Returns the replacement queue observation.
    pub const fn replacement_queue_observation(&self) -> fe2o3_kfd::ComputeAqlQueueObservationV1 {
        self.inner.replacement_queue_observation()
    }

    /// Returns the replacement dispatch generation.
    pub const fn replacement_dispatch_generation(&self) -> u64 {
        self.inner.replacement_dispatch_generation()
    }

    /// Consumes rollover evidence into the replacement authenticated queue.
    pub fn into_queue(self) -> AuthenticatedServiceQueueSessionV1<N> {
        AuthenticatedServiceQueueSessionV1 {
            queue: self.inner.into_queue(),
            programs: self.programs,
        }
    }
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueRolloverSuccessV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueRolloverSuccessV1")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

/// Authenticated rollover rejection or terminal replacement failure.
#[must_use = "authenticated rollover failure retains every available program owner"]
pub enum AuthenticatedServiceQueueRolloverFailureV1<const N: usize> {
    /// Currentness or exact program derivation failed before native destruction.
    Program {
        /// Exact derivation error.
        error: Box<AuthenticatedWorkerV3ProgramMaterializationErrorV1>,
        /// Unchanged old unbound queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged replacement program set.
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Structural preflight rejected unchanged inputs.
    QueueRejected {
        /// Exact structural error.
        error: Box<ServiceQueueErrorV1>,
        /// Unchanged old unbound queue.
        queue: Box<AuthenticatedServiceQueueUnboundSessionV1>,
        /// Unchanged replacement program set.
        replacement: AuthenticatedWorkerV3ProgramSetV1,
        /// Unchanged addressless packets.
        packets: Box<[ServiceFixedDispatchPacketV1; N]>,
    },
    /// Native rollover consumed the queue; program owners remain retained.
    Terminal {
        /// Exact native error.
        error: Box<ServiceQueueErrorV1>,
        /// Confirmed predecessor destruction, when observed.
        previous_queue_destroyed: Option<fe2o3_kfd::ComputeAqlQueueDestroyedV1>,
        /// Exact predecessor dispatch generation.
        previous_dispatch_generation: u64,
        /// Every old and replacement authenticated program set.
        retained: AuthenticatedServiceTerminalProgramCustodyV1,
    },
}

impl<const N: usize> fmt::Debug for AuthenticatedServiceQueueRolloverFailureV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program { error, .. } => formatter.debug_tuple("Program").field(error).finish(),
            Self::QueueRejected { error, .. } => {
                formatter.debug_tuple("QueueRejected").field(error).finish()
            }
            Self::Terminal {
                error,
                previous_queue_destroyed,
                previous_dispatch_generation,
                ..
            } => formatter
                .debug_struct("Terminal")
                .field("error", error)
                .field("previous_queue_destroyed", previous_queue_destroyed)
                .field("previous_dispatch_generation", previous_dispatch_generation)
                .finish_non_exhaustive(),
        }
    }
}

/// Lower queue-operation failure retaining all authenticated program owners.
#[must_use = "queue-operation failure retains opaque queue and program custody"]
pub struct AuthenticatedServiceQueueOperationFailureV1 {
    inner: Box<ServiceQueueOperationFailureV1>,
    programs: AuthenticatedProgramCustodyV1,
}

impl AuthenticatedServiceQueueOperationFailureV1 {
    /// Returns the exact lower queue error.
    pub const fn error(&self) -> &ServiceQueueErrorV1 {
        self.inner.error()
    }

    /// Returns an addressless timeout observation when the lower error carries one.
    pub fn timeout_observation(&self) -> Option<&fe2o3_kfd::Gfx942TimeoutExecutionObservationV1> {
        self.inner.timeout_observation()
    }

    /// Consumes the failure into opaque queue and authenticated-program quarantine.
    pub fn into_quarantined(self) -> AuthenticatedQuarantinedServiceQueueV1 {
        AuthenticatedQuarantinedServiceQueueV1 {
            queue: (*self.inner).into_quarantined(),
            programs: self.programs,
        }
    }
}

impl fmt::Debug for AuthenticatedServiceQueueOperationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueOperationFailureV1")
            .field("inner", &self.inner)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Opaque quarantine retaining the native queue and every authenticated roster owner.
#[must_use = "authenticated quarantined queue custody must remain retained"]
pub struct AuthenticatedQuarantinedServiceQueueV1 {
    queue: QuarantinedServiceQueueV1,
    programs: AuthenticatedProgramCustodyV1,
}

impl fmt::Debug for AuthenticatedQuarantinedServiceQueueV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedQuarantinedServiceQueueV1")
            .field("queue", &self.queue)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

/// Successful queue teardown paired with released authenticated program owners.
#[must_use = "released authenticated program sets should be explicitly consumed"]
pub struct AuthenticatedServiceQueueReleaseV1 {
    observation: ServiceQueueReleaseObservationV1,
    programs: Vec<AuthenticatedWorkerV3ProgramSetV1>,
}

impl AuthenticatedServiceQueueReleaseV1 {
    /// Returns redacted native teardown and allocation-release evidence.
    pub const fn observation(&self) -> ServiceQueueReleaseObservationV1 {
        self.observation
    }

    /// Returns every now-released authenticated program-set owner.
    pub fn into_program_sets(self) -> Vec<AuthenticatedWorkerV3ProgramSetV1> {
        self.programs
    }
}

impl fmt::Debug for AuthenticatedServiceQueueReleaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueReleaseV1")
            .field("observation", &self.observation)
            .field("program_set_count", &self.programs.len())
            .finish_non_exhaustive()
    }
}

/// Teardown failure retaining authenticated program custody beside lower quarantine.
#[must_use = "teardown failure retains authenticated program custody"]
pub struct AuthenticatedServiceQueueReleaseFailureV1 {
    inner: Box<ServiceQueueReleaseFailureV1>,
    programs: AuthenticatedProgramCustodyV1,
}

impl AuthenticatedServiceQueueReleaseFailureV1 {
    /// Returns the exact lower teardown failure.
    pub const fn error(&self) -> &ServiceQueueReleaseFailureV1 {
        &self.inner
    }

    /// Returns the lower teardown failure and every retained program-set owner.
    pub fn into_parts(
        self,
    ) -> (
        ServiceQueueReleaseFailureV1,
        Vec<AuthenticatedWorkerV3ProgramSetV1>,
    ) {
        (*self.inner, self.programs.into_program_sets())
    }
}

impl fmt::Debug for AuthenticatedServiceQueueReleaseFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedServiceQueueReleaseFailureV1")
            .field("inner", &self.inner)
            .field("programs", &self.programs)
            .finish_non_exhaustive()
    }
}

fn finish_release(
    result: Result<ServiceQueueReleaseObservationV1, ServiceQueueReleaseFailureV1>,
    programs: AuthenticatedProgramCustodyV1,
) -> Result<AuthenticatedServiceQueueReleaseV1, AuthenticatedServiceQueueReleaseFailureV1> {
    match route_release_custody(result, programs) {
        Ok((observation, programs)) => Ok(AuthenticatedServiceQueueReleaseV1 {
            observation,
            programs,
        }),
        Err((inner, programs)) => Err(AuthenticatedServiceQueueReleaseFailureV1 {
            inner: Box::new(inner),
            programs,
        }),
    }
}

fn route_release_custody<T, E>(
    result: Result<T, E>,
    programs: AuthenticatedProgramCustodyV1,
) -> Result<(T, Vec<AuthenticatedWorkerV3ProgramSetV1>), (E, AuthenticatedProgramCustodyV1)> {
    match result {
        Ok(value) => Ok((value, programs.into_program_sets())),
        Err(error) => Err((error, programs)),
    }
}
