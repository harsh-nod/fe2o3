//! Bounded standalone request payloads, separate from GPU and observer custody.

use super::*;
use crate::{
    RuntimeArgumentsV1, RuntimeBindingV1, RuntimeLaunchGeometryV1, RuntimeSubmissionV1,
    TypedRuntimeKernelV1,
};
use std::sync::atomic::AtomicUsize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncSnapshotErrorV1 {
    TooManyDependencies,
    DuplicateDependency,
    KernargTooLarge,
    TooManyBindings,
}

impl fmt::Display for RuntimeAsyncSnapshotErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid runtime async snapshot: {self:?}")
    }
}
impl Error for RuntimeAsyncSnapshotErrorV1 {}

fn compact_dependencies(
    dependencies: Vec<RuntimeEventIdV1>,
) -> Result<Box<[RuntimeEventIdV1]>, RuntimeAsyncSnapshotErrorV1> {
    if dependencies.len() > crate::MAX_RUNTIME_DEPENDENCIES_V1 {
        return Err(RuntimeAsyncSnapshotErrorV1::TooManyDependencies);
    }
    for (index, dependency) in dependencies.iter().enumerate() {
        if dependencies[..index].contains(dependency) {
            return Err(RuntimeAsyncSnapshotErrorV1::DuplicateDependency);
        }
    }
    Ok(dependencies.into_boxed_slice())
}

pub(super) struct SnapshotBudgetV1 {
    capacity: usize,
    used: AtomicUsize,
}

impl SnapshotBudgetV1 {
    pub(super) fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            used: AtomicUsize::new(0),
        })
    }

    pub(super) fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }

    fn charge<T>(
        self: &Arc<Self>,
        value: T,
        bytes: usize,
    ) -> Result<Charged<T>, RuntimeAsyncEngineCallErrorV1> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                fe2o3_runtime_model::r64_payload_reserve_v1(used, bytes, self.capacity)
            })
            .map_err(|_| RuntimeAsyncEngineCallErrorV1::SnapshotCapacity)?;
        Ok(Charged {
            value,
            _permit: SnapshotPermitV1 {
                budget: Arc::clone(self),
                bytes,
            },
        })
    }
}

struct SnapshotPermitV1 {
    budget: Arc<SnapshotBudgetV1>,
    bytes: usize,
}
impl Drop for SnapshotPermitV1 {
    fn drop(&mut self) {
        self.budget
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                fe2o3_runtime_model::r64_payload_release_v1(used, self.bytes)
            })
            .expect("snapshot permits return their exact retained charge");
    }
}

// Field order releases payload before its non-cloneable permit. The consuming
// method captures this entire owner, avoiding disjoint closure-field capture.
pub(super) struct Charged<T> {
    value: T,
    _permit: SnapshotPermitV1,
}
impl<T> Charged<T> {
    pub(super) fn with<R>(self, operation: impl FnOnce(&T) -> R) -> R {
        operation(&self.value)
    }
}

pub(super) fn charge_dependencies(
    budget: &Arc<SnapshotBudgetV1>,
    dependencies: Vec<RuntimeEventIdV1>,
) -> Result<Charged<Box<[RuntimeEventIdV1]>>, RuntimeAsyncEngineCallErrorV1> {
    let dependencies = compact_dependencies(dependencies)
        .map_err(RuntimeAsyncEngineCallErrorV1::InvalidSnapshot)?;
    let bytes = std::mem::size_of_val(&*dependencies);
    budget.charge(dependencies, bytes)
}

/// An owned address-free launch snapshot, not context or executable admission.
///
/// The constructor evaluates each argument getter once on the caller thread;
/// encoder panics propagate there. Original argument objects and spare Vec
/// capacity are not retained. Mutable application state is not atomically read.
/// Caller-owned requests and allocations inside encoders are outside the engine
/// budget until enqueue. Owner admission revalidates all live resource identities.
pub struct RuntimeAsyncLaunchRequestV1<A: RuntimeArgumentsV1> {
    stream: RuntimeStreamIdV1,
    kernel: Arc<TypedRuntimeKernelV1<A>>,
    bytes: Box<[u8]>,
    bindings: Box<[RuntimeBindingV1]>,
    geometry: RuntimeLaunchGeometryV1,
    dependencies: Box<[RuntimeEventIdV1]>,
}

impl<A: RuntimeArgumentsV1> RuntimeAsyncLaunchRequestV1<A> {
    pub fn new(
        stream: RuntimeStreamIdV1,
        kernel: Arc<TypedRuntimeKernelV1<A>>,
        arguments: &A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<Self, RuntimeAsyncSnapshotErrorV1> {
        let dependencies = compact_dependencies(dependencies)?;
        let bytes = arguments.encode_explicit_kernarg_v1();
        if bytes.len() > crate::MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(RuntimeAsyncSnapshotErrorV1::KernargTooLarge);
        }
        let bindings = arguments.bindings_v1();
        if bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(RuntimeAsyncSnapshotErrorV1::TooManyBindings);
        }
        Ok(Self {
            stream,
            kernel,
            bytes: bytes.into_boxed_slice(),
            bindings: bindings.into_boxed_slice(),
            geometry,
            dependencies,
        })
    }

    /// Exact retained slice payload charge. Excludes allocator/Arc/record overhead.
    pub fn snapshot_bytes(&self) -> usize {
        self.bytes.len()
            + std::mem::size_of_val(&*self.bindings)
            + std::mem::size_of_val(&*self.dependencies)
    }

    fn submit<B: RuntimeBackendV1>(
        &self,
        context: &mut RuntimeContextV1<B>,
    ) -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<B::Error>> {
        context.launch_snapshot_v1(
            self.stream,
            &self.kernel,
            &self.bytes,
            &self.bindings,
            self.geometry,
            &self.dependencies,
        )
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    /// Consumes a frozen request; byte capacity is shared by all cloned handles.
    /// The payload is charged until submission/disposal, independently of future
    /// Drop, cancellation requests or timeout. Native custody follows normal rules.
    pub fn enqueue_launch<A: RuntimeArgumentsV1>(
        &self,
        request: RuntimeAsyncLaunchRequestV1<A>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let stream = request.stream;
        let bytes = request.snapshot_bytes();
        let request = self.observer.snapshot_budget.charge(request, bytes)?;
        self.enqueue_operation(
            stream,
            Box::new(move |context| request.with(|r| r.submit(context))),
        )
    }

    /// Frozen launch with the same local cancellation and timeout controls.
    pub fn enqueue_launch_tracked<A: RuntimeArgumentsV1>(
        &self,
        request: RuntimeAsyncLaunchRequestV1<A>,
    ) -> Result<RuntimeAsyncTrackedOperationV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let stream = request.stream;
        let bytes = request.snapshot_bytes();
        let request = self.observer.snapshot_budget.charge(request, bytes)?;
        self.enqueue_tracked_operation(
            stream,
            Box::new(move |context| request.with(|r| r.submit(context))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn r64_shared_snapshot_credit_is_linearized_under_contention() {
        let budget = SnapshotBudgetV1::new(32);
        let start = Arc::new(Barrier::new(17));
        let retained = Arc::new(Barrier::new(17));
        let release = Arc::new(Barrier::new(17));
        let accepted = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let (budget, start, retained, release, accepted) = (
                    budget.clone(),
                    start.clone(),
                    retained.clone(),
                    release.clone(),
                    accepted.clone(),
                );
                thread::spawn(move || {
                    start.wait();
                    let payload = budget.charge(vec![0u8; 8].into_boxed_slice(), 8);
                    if payload.is_ok() {
                        accepted.fetch_add(1, Ordering::SeqCst);
                    }
                    retained.wait();
                    release.wait();
                    drop(payload);
                })
            })
            .collect();
        start.wait();
        retained.wait();
        assert_eq!(accepted.load(Ordering::SeqCst), 4);
        assert_eq!(budget.used(), 32);
        release.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn r64_payload_is_dropped_before_credit_refund_even_on_unwind() {
        struct Payload {
            budget: Arc<SnapshotBudgetV1>,
            drops: Arc<AtomicUsize>,
        }
        impl Drop for Payload {
            fn drop(&mut self) {
                assert_eq!(self.budget.used(), 8);
                self.drops.fetch_add(1, Ordering::SeqCst);
            }
        }
        let budget = SnapshotBudgetV1::new(8);
        let drops = Arc::new(AtomicUsize::new(0));
        let payload = budget
            .charge(
                Payload {
                    budget: budget.clone(),
                    drops: drops.clone(),
                },
                8,
            )
            .unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || payload.with(|_| panic!("submission unwind"))
            ))
            .is_err()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.used(), 0);
    }
}
