//! Typed lazy-operation, stream-sequencing, and event-dependency model.
//!
//! This layer composes [`AsyncQueueRegistryV1`] without adding a queue,
//! signal, polling, or launch implementation. Kernel, stream, and event
//! identities are caller-selected model values. They grant no native authority.
//! A future adapter must retain the concrete generated arguments and memory
//! owners represented by the registry until the modeled release transition.

use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use crate::*;

pub const TYPED_ASYNC_MODEL_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_TYPED_ASYNC_DEPENDENCIES_V1: usize = 256;

fn digest_is_zero(digest: IdentityDigestV1) -> bool {
    digest.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
}

/// Caller-declared kernel type and identity for model-only composition.
///
/// `K` prevents accidental exchange between typed operation states. The digest
/// is not authenticated and does not establish a compiler, artifact, or proof
/// relationship.
#[derive(Debug)]
pub struct TypedAsyncKernelV1<K> {
    identity: IdentityDigestV1,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> Copy for TypedAsyncKernelV1<K> {}

impl<K> Clone for TypedAsyncKernelV1<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> TypedAsyncKernelV1<K> {
    pub fn new_model_only(identity: IdentityDigestV1) -> Result<Self, TypedAsyncErrorV1> {
        if digest_is_zero(identity) {
            return Err(TypedAsyncErrorV1::InvalidKernelIdentity);
        }
        Ok(Self {
            identity,
            marker: PhantomData,
        })
    }

    pub const fn identity(self) -> IdentityDigestV1 {
        self.identity
    }
}

/// Exact declared stream incarnation above one queue registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelAsyncStreamIdentityV1 {
    registry_incarnation: IdentityDigestV1,
    queue: QueueKeyV1,
    stream_incarnation: IdentityDigestV1,
}

impl ModelAsyncStreamIdentityV1 {
    pub const fn registry_incarnation(self) -> IdentityDigestV1 {
        self.registry_incarnation
    }

    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub const fn stream_incarnation(self) -> IdentityDigestV1 {
        self.stream_incarnation
    }
}

/// Failure to add stream policy above an async queue registry.
#[must_use]
pub struct ModelAsyncStreamCreateFailureV1 {
    error: TypedAsyncErrorV1,
    registry: Box<AsyncQueueRegistryV1>,
}

impl ModelAsyncStreamCreateFailureV1 {
    pub const fn error(&self) -> TypedAsyncErrorV1 {
        self.error
    }

    pub fn into_registry(self) -> AsyncQueueRegistryV1 {
        *self.registry
    }
}

impl core::fmt::Debug for ModelAsyncStreamCreateFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ModelAsyncStreamCreateFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Model-only stream policy over one exact reusable queue registry.
///
/// The monotonically increasing sequence is assigned at reservation and never
/// reused, including after pre-publication cancellation. Dropping this value
/// cannot release a native resource because it owns no native authority. A
/// concrete adapter must keep its independently sealed registry alive while
/// any modeled operation remains retained.
pub struct ModelAsyncStreamV1 {
    registry: AsyncQueueRegistryV1,
    identity: ModelAsyncStreamIdentityV1,
    next_sequence: u64,
}

impl core::fmt::Debug for ModelAsyncStreamV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ModelAsyncStreamV1")
            .field("identity", &self.identity)
            .field("next_sequence", &self.next_sequence)
            .field("registry", &self.registry)
            .finish()
    }
}

impl ModelAsyncStreamV1 {
    pub fn new_model_only(
        registry: AsyncQueueRegistryV1,
        stream_incarnation: IdentityDigestV1,
    ) -> Result<Self, ModelAsyncStreamCreateFailureV1> {
        if digest_is_zero(stream_incarnation) {
            return Err(ModelAsyncStreamCreateFailureV1 {
                error: TypedAsyncErrorV1::InvalidStreamIncarnation,
                registry: Box::new(registry),
            });
        }
        let identity = ModelAsyncStreamIdentityV1 {
            registry_incarnation: registry.registry_incarnation(),
            queue: registry.queue(),
            stream_incarnation,
        };
        Ok(Self {
            registry,
            identity,
            next_sequence: 1,
        })
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn identity(&self) -> ModelAsyncStreamIdentityV1 {
        self.identity
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn retained_operation_count(&self) -> usize {
        self.registry.retained_operation_count()
    }

    pub fn available_slot_count(&self) -> usize {
        self.registry.available_slot_count()
    }

    pub fn registry(&self) -> &AsyncQueueRegistryV1 {
        &self.registry
    }

    pub fn validate_global_invariants(&self) -> Result<(), AsyncQueueInvariantViolationV1> {
        self.registry.validate_global_invariants()
    }

    /// Returns the underlying runtime state only after every typed operation
    /// has released its queue slot. On rejection, the exact stream sequence and
    /// registry custody are returned together.
    pub fn into_runtime_state(self) -> Result<RuntimeStateV1, Box<Self>> {
        let Self {
            registry,
            identity,
            next_sequence,
        } = self;
        match registry.into_runtime_state() {
            Ok(runtime) => Ok(runtime),
            Err(registry) => Err(Box::new(Self {
                registry: *registry,
                identity,
                next_sequence,
            })),
        }
    }
}

/// Exact typed operation occurrence assigned by one model stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypedAsyncOperationIdentityV1 {
    kernel_identity: IdentityDigestV1,
    stream: ModelAsyncStreamIdentityV1,
    stream_sequence: u64,
    queue_binding: AsyncOperationBindingV1,
}

impl TypedAsyncOperationIdentityV1 {
    pub const fn kernel_identity(self) -> IdentityDigestV1 {
        self.kernel_identity
    }

    pub const fn stream(self) -> ModelAsyncStreamIdentityV1 {
        self.stream
    }

    pub const fn stream_sequence(self) -> u64 {
        self.stream_sequence
    }

    pub const fn queue_binding(self) -> AsyncOperationBindingV1 {
        self.queue_binding
    }
}

/// Completion event bound to the source kernel type and operation occurrence.
///
/// ```compile_fail
/// use fe2o3_runtime_model::TypedAsyncEventV1;
/// struct Alpha;
/// struct Beta;
/// fn cannot_retype(event: TypedAsyncEventV1<Alpha>) {
///     let _: TypedAsyncEventV1<Beta> = event;
/// }
/// ```
#[derive(Debug)]
pub struct TypedAsyncEventV1<K> {
    operation: TypedAsyncOperationIdentityV1,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> Copy for TypedAsyncEventV1<K> {}

impl<K> Clone for TypedAsyncEventV1<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> TypedAsyncEventV1<K> {
    pub const fn operation(&self) -> TypedAsyncOperationIdentityV1 {
        self.operation
    }

    /// Explicitly erases only the Rust marker so another typed kernel can name
    /// this completed occurrence as a dependency.
    pub const fn as_dependency(&self) -> AsyncEventDependencyV1 {
        AsyncEventDependencyV1 {
            operation: self.operation,
        }
    }
}

/// Type-erased completed event accepted as a cross-kernel dependency.
///
/// Fields are private so arbitrary stream sequences cannot be promoted to
/// completion dependencies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AsyncEventDependencyV1 {
    operation: TypedAsyncOperationIdentityV1,
}

impl AsyncEventDependencyV1 {
    pub const fn operation(self) -> TypedAsyncOperationIdentityV1 {
        self.operation
    }
}

/// Model construction or transition rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedAsyncErrorV1 {
    InvalidKernelIdentity,
    InvalidStreamIncarnation,
    InvalidDependency,
    DuplicateDependency,
    DependencyContextMismatch,
    DependencyOrderingMismatch,
    StreamMismatch,
    StreamSequenceExhausted,
    Queue(AsyncQueueErrorV1),
}

/// Failure from a consuming transition, with exact typed custody returned.
#[must_use]
pub struct TypedAsyncTransitionFailureV1<T> {
    error: TypedAsyncErrorV1,
    retained: Box<T>,
}

impl<T> TypedAsyncTransitionFailureV1<T> {
    pub const fn error(&self) -> TypedAsyncErrorV1 {
        self.error
    }

    pub fn into_retained(self) -> T {
        *self.retained
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for TypedAsyncTransitionFailureV1<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("TypedAsyncTransitionFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

#[derive(Debug)]
struct TypedAsyncMetadataV1<K> {
    identity: TypedAsyncOperationIdentityV1,
    resources: AsyncOperationResourcesV1,
    dependencies: Vec<AsyncEventDependencyV1>,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> TypedAsyncMetadataV1<K> {
    const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.identity
    }
}

/// A lazy typed operation that has not reserved a queue slot or published work.
///
/// Dropping this value is cancellation before reservation and has no modeled
/// device effect.
#[derive(Debug)]
#[must_use = "a lazy operation must be reserved or deliberately dropped"]
pub struct LazyTypedAsyncOperationV1<K> {
    kernel: TypedAsyncKernelV1<K>,
    expected_stream: ModelAsyncStreamIdentityV1,
    dispatch: DispatchKeyV1,
    completion: CompletionKeyV1,
    resources: AsyncOperationResourcesV1,
    dependencies: Vec<AsyncEventDependencyV1>,
}

impl<K> LazyTypedAsyncOperationV1<K> {
    pub fn new_model_only(
        kernel: TypedAsyncKernelV1<K>,
        stream: &ModelAsyncStreamV1,
        dispatch: DispatchKeyV1,
        completion: CompletionKeyV1,
        resources: AsyncOperationResourcesV1,
        dependencies: Vec<AsyncEventDependencyV1>,
    ) -> Result<Self, TypedAsyncErrorV1> {
        if dispatch.queue != stream.identity.queue || completion.dispatch != dispatch {
            return Err(TypedAsyncErrorV1::StreamMismatch);
        }
        if dependencies.len() > MAX_TYPED_ASYNC_DEPENDENCIES_V1 {
            return Err(TypedAsyncErrorV1::InvalidDependency);
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependency.operation.queue_binding.dispatch() == dispatch {
                return Err(TypedAsyncErrorV1::InvalidDependency);
            }
            if dependencies[..index].contains(dependency) {
                return Err(TypedAsyncErrorV1::DuplicateDependency);
            }
            if dependency.operation.stream.queue.vm != stream.identity.queue.vm {
                return Err(TypedAsyncErrorV1::DependencyContextMismatch);
            }
        }
        Ok(Self {
            kernel,
            expected_stream: stream.identity,
            dispatch,
            completion,
            resources,
            dependencies,
        })
    }

    pub const fn kernel_identity(&self) -> IdentityDigestV1 {
        self.kernel.identity
    }

    pub const fn expected_stream(&self) -> ModelAsyncStreamIdentityV1 {
        self.expected_stream
    }

    pub const fn dispatch(&self) -> DispatchKeyV1 {
        self.dispatch
    }

    pub const fn completion(&self) -> CompletionKeyV1 {
        self.completion
    }

    pub fn resources(&self) -> &AsyncOperationResourcesV1 {
        &self.resources
    }

    pub fn dependencies(&self) -> &[AsyncEventDependencyV1] {
        &self.dependencies
    }

    pub fn reserve_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<ReservedTypedAsyncOperationV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.expected_stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Some(next_sequence) = stream.next_sequence.checked_add(1) else {
            return Err(typed_failure(
                TypedAsyncErrorV1::StreamSequenceExhausted,
                self,
            ));
        };
        if self.dependencies.iter().any(|dependency| {
            dependency.operation.stream == stream.identity
                && dependency.operation.stream_sequence >= stream.next_sequence
        }) {
            return Err(typed_failure(
                TypedAsyncErrorV1::DependencyOrderingMismatch,
                self,
            ));
        }
        let reservation = match stream.registry.reserve_model_only(
            self.dispatch,
            self.completion,
            self.resources.clone(),
        ) {
            Ok(reservation) => reservation,
            Err(error) => {
                return Err(typed_failure(TypedAsyncErrorV1::Queue(error), self));
            }
        };
        let identity = TypedAsyncOperationIdentityV1 {
            kernel_identity: self.kernel.identity,
            stream: stream.identity,
            stream_sequence: stream.next_sequence,
            queue_binding: reservation.binding(),
        };
        stream.next_sequence = next_sequence;
        Ok(ReservedTypedAsyncOperationV1 {
            metadata: TypedAsyncMetadataV1 {
                identity,
                resources: self.resources,
                dependencies: self.dependencies,
                marker: PhantomData,
            },
            reservation,
        })
    }
}

/// Move-only typed custody after reservation and before publication.
///
/// ```compile_fail
/// use fe2o3_runtime_model::ReservedTypedAsyncOperationV1;
/// fn cannot_clone<K>(operation: ReservedTypedAsyncOperationV1<K>) {
///     let _ = operation.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "reserved custody must be published or cancelled before publication"]
pub struct ReservedTypedAsyncOperationV1<K> {
    metadata: TypedAsyncMetadataV1<K>,
    reservation: AsyncReservedOperationTokenV1,
}

impl<K> ReservedTypedAsyncOperationV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.metadata.identity()
    }

    pub fn dependencies(&self) -> &[AsyncEventDependencyV1] {
        &self.metadata.dependencies
    }

    pub fn resources(&self) -> &AsyncOperationResourcesV1 {
        &self.metadata.resources
    }

    pub fn publish_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<SubmittedTypedAsyncOperationV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            reservation,
        } = self;
        match reservation.publish_model_only(&mut stream.registry) {
            Ok(submitted) => Ok(SubmittedTypedAsyncOperationV1 {
                metadata,
                submitted,
            }),
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    reservation: failure.into_retained(),
                },
            )),
        }
    }

    pub fn cancel_before_publication_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<TypedAsyncCancelledReceiptV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            reservation,
        } = self;
        match reservation.cancel_before_publication_model_only(&mut stream.registry) {
            Ok(released) => Ok(TypedAsyncCancelledReceiptV1 {
                identity: metadata.identity,
                released,
                marker: PhantomData,
            }),
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    reservation: failure.into_retained(),
                },
            )),
        }
    }
}

/// Move-only typed custody after publication.
///
/// Dropping this value does not mutate the registry and is not cancellation.
#[derive(Debug)]
#[must_use = "submitted custody must reach completion or remain retained"]
pub struct SubmittedTypedAsyncOperationV1<K> {
    metadata: TypedAsyncMetadataV1<K>,
    submitted: AsyncSubmittedOperationTokenV1,
}

impl<K> SubmittedTypedAsyncOperationV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.metadata.identity()
    }

    pub fn dependencies(&self) -> &[AsyncEventDependencyV1] {
        &self.metadata.dependencies
    }

    pub fn resources(&self) -> &AsyncOperationResourcesV1 {
        &self.metadata.resources
    }

    pub fn poll_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
        observation: AsyncCompletionObservationV1,
    ) -> Result<TypedAsyncOperationPollV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            submitted,
        } = self;
        match submitted.poll_model_only(&mut stream.registry, observation) {
            Ok(AsyncOperationPollV1::Pending(submitted)) => {
                Ok(TypedAsyncOperationPollV1::Pending(Self {
                    metadata,
                    submitted,
                }))
            }
            Ok(AsyncOperationPollV1::Completed(completed)) => Ok(
                TypedAsyncOperationPollV1::Completed(CompletedTypedAsyncOperationV1 {
                    metadata,
                    completed,
                }),
            ),
            Ok(AsyncOperationPollV1::Indeterminate(quarantined)) => Ok(
                TypedAsyncOperationPollV1::Indeterminate(QuarantinedTypedAsyncOperationV1 {
                    metadata,
                    quarantined,
                }),
            ),
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    submitted: failure.into_retained(),
                },
            )),
        }
    }

    pub fn observe_timeout_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<TimedOutTypedAsyncOperationV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            submitted,
        } = self;
        match submitted.observe_timeout_model_only(&mut stream.registry) {
            Ok(timeout) => {
                let observation_count = timeout.observation_count();
                Ok(TimedOutTypedAsyncOperationV1 {
                    submitted: Self {
                        metadata,
                        submitted: timeout.into_submitted(),
                    },
                    observation_count,
                })
            }
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    submitted: failure.into_retained(),
                },
            )),
        }
    }

    pub fn request_cancellation_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<TypedAsyncCancellationRequestV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            submitted,
        } = self;
        match submitted.request_cancellation_model_only(&mut stream.registry) {
            Ok(request) => {
                let first_request = request.is_first_request();
                Ok(TypedAsyncCancellationRequestV1 {
                    submitted: Self {
                        metadata,
                        submitted: request.into_submitted(),
                    },
                    first_request,
                })
            }
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    submitted: failure.into_retained(),
                },
            )),
        }
    }
}

#[derive(Debug)]
pub enum TypedAsyncOperationPollV1<K> {
    Pending(SubmittedTypedAsyncOperationV1<K>),
    Completed(CompletedTypedAsyncOperationV1<K>),
    Indeterminate(QuarantinedTypedAsyncOperationV1<K>),
}

#[derive(Debug)]
#[must_use = "timeout retains submitted typed custody"]
pub struct TimedOutTypedAsyncOperationV1<K> {
    submitted: SubmittedTypedAsyncOperationV1<K>,
    observation_count: u64,
}

impl<K> TimedOutTypedAsyncOperationV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.submitted.identity()
    }

    pub const fn observation_count(&self) -> u64 {
        self.observation_count
    }

    pub fn into_submitted(self) -> SubmittedTypedAsyncOperationV1<K> {
        self.submitted
    }
}

#[derive(Debug)]
#[must_use = "post-publication cancellation retains submitted typed custody"]
pub struct TypedAsyncCancellationRequestV1<K> {
    submitted: SubmittedTypedAsyncOperationV1<K>,
    first_request: bool,
}

impl<K> TypedAsyncCancellationRequestV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.submitted.identity()
    }

    pub const fn is_first_request(&self) -> bool {
        self.first_request
    }

    pub fn into_submitted(self) -> SubmittedTypedAsyncOperationV1<K> {
        self.submitted
    }
}

/// Observed typed completion before explicit signal/slot recycling.
#[derive(Debug)]
#[must_use = "completion custody must be recycled before slot reuse"]
pub struct CompletedTypedAsyncOperationV1<K> {
    metadata: TypedAsyncMetadataV1<K>,
    completed: AsyncCompletedOperationTokenV1,
}

impl<K> CompletedTypedAsyncOperationV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.metadata.identity()
    }

    pub const fn event(&self) -> TypedAsyncEventV1<K> {
        TypedAsyncEventV1 {
            operation: self.metadata.identity,
            marker: PhantomData,
        }
    }

    pub fn recycle_model_only(
        self,
        stream: &mut ModelAsyncStreamV1,
    ) -> Result<TypedAsyncCompletionReceiptV1<K>, TypedAsyncTransitionFailureV1<Self>> {
        if self.metadata.identity.stream != stream.identity {
            return Err(typed_failure(TypedAsyncErrorV1::StreamMismatch, self));
        }
        let Self {
            metadata,
            completed,
        } = self;
        match completed.recycle_model_only(&mut stream.registry) {
            Ok(released) => Ok(TypedAsyncCompletionReceiptV1 {
                event: TypedAsyncEventV1 {
                    operation: metadata.identity,
                    marker: PhantomData,
                },
                released,
            }),
            Err(failure) => Err(typed_failure(
                TypedAsyncErrorV1::Queue(*failure.error()),
                Self {
                    metadata,
                    completed: failure.into_retained(),
                },
            )),
        }
    }
}

/// Typed post-publication quarantine with no retry or release transition.
#[derive(Debug)]
#[must_use = "indeterminate typed custody must remain quarantined"]
pub struct QuarantinedTypedAsyncOperationV1<K> {
    metadata: TypedAsyncMetadataV1<K>,
    quarantined: QuarantinedAsyncOperationV1,
}

impl<K> QuarantinedTypedAsyncOperationV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.metadata.identity()
    }

    pub const fn reason(&self) -> AsyncIndeterminateReasonV1 {
        self.quarantined.reason()
    }

    pub fn resources(&self) -> &AsyncOperationResourcesV1 {
        &self.metadata.resources
    }
}

#[derive(Debug)]
pub struct TypedAsyncCancelledReceiptV1<K> {
    identity: TypedAsyncOperationIdentityV1,
    released: AsyncReleasedOperationReceiptV1,
    marker: PhantomData<fn(K) -> K>,
}

impl<K> TypedAsyncCancelledReceiptV1<K> {
    pub const fn identity(&self) -> TypedAsyncOperationIdentityV1 {
        self.identity
    }

    pub const fn released(&self) -> AsyncReleasedOperationReceiptV1 {
        self.released
    }
}

#[derive(Debug)]
pub struct TypedAsyncCompletionReceiptV1<K> {
    event: TypedAsyncEventV1<K>,
    released: AsyncReleasedOperationReceiptV1,
}

impl<K> TypedAsyncCompletionReceiptV1<K> {
    pub const fn event(&self) -> TypedAsyncEventV1<K> {
        self.event
    }

    pub const fn released(&self) -> AsyncReleasedOperationReceiptV1 {
        self.released
    }
}

fn typed_failure<T>(error: TypedAsyncErrorV1, retained: T) -> TypedAsyncTransitionFailureV1<T> {
    TypedAsyncTransitionFailureV1 {
        error,
        retained: Box::new(retained),
    }
}
