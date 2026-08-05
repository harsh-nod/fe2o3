use crate::{
    DeviceBuffer, DeviceCopy, Error, Event, EventOptions, PinnedHostBuffer, Result, Stream, check,
};
use core::ffi::c_void;
use core::marker::PhantomData;
use fe2o3_completion::{
    Completion, CompletionError, CompletionFailure, PendingOwned, complete_borrowed,
    complete_owned, settle_borrowed, synchronize_with_fallback,
};

/// A non-escapable view of one submitted operation over borrowed resources.
///
/// Instances are exposed only by the callback passed to the `copy_*` methods.
/// The callback runs while the operation may be pending, and the method waits
/// before returning to release the resource borrows. This scoped shape is
/// necessary because a returnable RAII guard could be safely forgotten.
#[derive(Debug)]
pub struct BorrowedDeviceOperation<'stream, 'resources> {
    completion: Option<HipCompletion<'stream>>,
    _resources: PhantomData<&'resources mut ()>,
}

impl<'stream, 'resources> BorrowedDeviceOperation<'stream, 'resources> {
    /// Runs one caller-defined asynchronous operation while retaining all of
    /// its resources until HIP completion is established.
    ///
    /// The operation handle is available only inside `during`; it cannot be
    /// returned or safely forgotten. `resources` is owned by this scope and is
    /// dropped only after the completion event, or the stronger stream
    /// synchronization fallback, establishes quiescence. Ambiguous completion
    /// aborts instead of releasing resources that the device might still use.
    ///
    /// This is a low-level integration point for typed wrappers. It does not
    /// validate a kernel ABI, pointer provenance, aliasing, or backend object
    /// identity.
    ///
    /// # Safety
    ///
    /// `enqueue` must submit work only to `stream`. `resources` must own or
    /// borrow every allocation, module, function, and other object that the
    /// submitted work may access, and those resources must be valid for every
    /// such access. The caller must uphold all operation-specific safety
    /// requirements, including raw kernel ABI and synchronization contracts.
    #[doc(hidden)]
    pub unsafe fn run_scoped_unchecked<R, O>(
        stream: &'stream Stream,
        resources: R,
        enqueue: impl FnOnce(&R) -> Result<()>,
        during: impl for<'operation> FnOnce(
            &'operation BorrowedDeviceOperation<'stream, 'resources>,
        ) -> O,
    ) -> Result<O> {
        run_borrowed_retained::<'stream, 'resources, _, _>(stream, resources, enqueue, during)
    }

    /// Enqueues a pinned-host-to-device copy for the duration of `during`.
    ///
    /// `source` remains immutably borrowed and `destination` remains
    /// exclusively borrowed until HIP completion is established. The callback
    /// may perform unrelated host work and query the operation, but it cannot
    /// take ownership of the operation handle.
    ///
    /// ```compile_fail
    /// use fe2o3_core::{BorrowedDeviceOperation, DeviceBuffer, GpuContext, PinnedHostBuffer};
    ///
    /// let context = GpuContext::new(0)?;
    /// let stream = context.create_stream()?;
    /// let source = PinnedHostBuffer::from_slice(&context, &[1_u32])?;
    /// let mut destination = DeviceBuffer::zeroed(&stream, 1)?;
    /// BorrowedDeviceOperation::copy_to_device(
    ///     &stream,
    ///     &source,
    ///     &mut destination,
    ///     |_| drop(destination),
    /// )?;
    /// # Ok::<(), fe2o3_core::Error>(())
    /// ```
    pub fn copy_to_device<T: DeviceCopy, O>(
        stream: &'stream Stream,
        source: &'resources PinnedHostBuffer<T>,
        destination: &'resources mut DeviceBuffer<T>,
        during: impl for<'operation> FnOnce(&'operation Self) -> O,
    ) -> Result<O> {
        validate_copy(
            stream,
            source.context().device_id(),
            "pinned host source",
            source.len(),
            destination.context().device_id(),
            "device destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_mut_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_device_ptr() }.cast::<c_void>();
        // SAFETY: validation above checks the stream device and lengths. The
        // resource tuple retains both caller borrows through completion.
        unsafe {
            Self::run_scoped_unchecked(
                stream,
                (source, destination),
                |_| {
                    if size == 0 {
                        return Ok(());
                    }
                    check(fe2o3_hip_sys::hipMemcpyAsync(
                        destination_ptr,
                        source_ptr,
                        size,
                        fe2o3_hip_sys::HIP_MEMCPY_HOST_TO_DEVICE,
                        stream.raw(),
                    ))
                },
                during,
            )
        }
    }

    /// Enqueues a device-to-pinned-host copy for the duration of `during`.
    pub fn copy_to_host<T: DeviceCopy, O>(
        stream: &'stream Stream,
        source: &'resources DeviceBuffer<T>,
        destination: &'resources mut PinnedHostBuffer<T>,
        during: impl for<'operation> FnOnce(&'operation Self) -> O,
    ) -> Result<O> {
        validate_copy(
            stream,
            source.context().device_id(),
            "device source",
            source.len(),
            destination.context().device_id(),
            "pinned host destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_device_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_mut_ptr() }.cast::<c_void>();
        // SAFETY: validation above checks the stream device and lengths. The
        // resource tuple retains both caller borrows through completion.
        unsafe {
            Self::run_scoped_unchecked(
                stream,
                (source, destination),
                |_| {
                    if size == 0 {
                        return Ok(());
                    }
                    check(fe2o3_hip_sys::hipMemcpyAsync(
                        destination_ptr,
                        source_ptr,
                        size,
                        fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_HOST,
                        stream.raw(),
                    ))
                },
                during,
            )
        }
    }

    /// Enqueues a device-to-device copy for the duration of `during`.
    pub fn copy_device_to_device<T: DeviceCopy, O>(
        stream: &'stream Stream,
        source: &'resources DeviceBuffer<T>,
        destination: &'resources mut DeviceBuffer<T>,
        during: impl for<'operation> FnOnce(&'operation Self) -> O,
    ) -> Result<O> {
        validate_copy(
            stream,
            source.context().device_id(),
            "device source",
            source.len(),
            destination.context().device_id(),
            "device destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_device_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_device_ptr() }.cast::<c_void>();
        // SAFETY: validation above checks the stream device and lengths. The
        // resource tuple retains both caller borrows through completion.
        unsafe {
            Self::run_scoped_unchecked(
                stream,
                (source, destination),
                |_| {
                    if size == 0 {
                        return Ok(());
                    }
                    check(fe2o3_hip_sys::hipMemcpyAsync(
                        destination_ptr,
                        source_ptr,
                        size,
                        fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_DEVICE,
                        stream.raw(),
                    ))
                },
                during,
            )
        }
    }

    /// Returns whether the operation's completion event has fired.
    pub fn is_complete(&self) -> Result<bool> {
        self.completion
            .as_ref()
            .expect("borrowed operation has completion state")
            .query()
    }

    fn finish(mut self) -> Result<()> {
        self.settle()
    }

    fn settle(&mut self) -> Result<()> {
        finish_borrowed_completion(&mut self.completion)
    }
}

fn finish_borrowed_completion<C: Completion<Error = Error>>(
    completion: &mut Option<C>,
) -> Result<()> {
    let result = settle_borrowed(|| {
        completion
            .as_ref()
            .expect("borrowed operation has completion state")
            .synchronize()
    });
    drop(completion.take());
    result
}

impl Drop for BorrowedDeviceOperation<'_, '_> {
    fn drop(&mut self) {
        if self.completion.is_some() {
            let _ = self.settle();
        }
    }
}

/// A submitted device operation that owns every participating resource.
///
/// The operation has no cancellation API. [`OwnedDeviceOperation::wait`]
/// returns the resources only after HIP establishes completion. Dropping the
/// handle also waits. If event synchronization reports an execution error but
/// stream synchronization establishes quiescence, the error is returned and
/// the resources are released. If both synchronization methods fail, the
/// resources are deliberately leaked because freeing them could race device
/// work whose completion is unknown.
#[derive(Debug)]
#[must_use = "dropping a submitted device operation waits for completion"]
pub struct OwnedDeviceOperation<'stream, R> {
    pending: PendingOwned<R, HipCompletion<'stream>>,
}

impl<'stream, R> OwnedDeviceOperation<'stream, R> {
    /// Returns whether the operation's completion event has fired.
    ///
    /// Errors leave the operation submitted and retain all resources.
    pub fn is_complete(&self) -> Result<bool> {
        self.pending.query()
    }

    /// Waits for completion and returns all retained resources.
    ///
    /// Event synchronization is attempted first. If it errors, the operation
    /// synchronizes the entire stream. If that fallback succeeds, the original
    /// event error is returned after resources are safely released. If both
    /// methods fail, their errors are returned together and the resources and
    /// completion event are leaked.
    pub fn wait(self) -> Result<R> {
        self.pending.wait()
    }
}

impl<'stream, T: DeviceCopy> OwnedDeviceOperation<'stream, (PinnedHostBuffer<T>, DeviceBuffer<T>)> {
    /// Enqueues a pinned-host-to-device copy and retains both buffers.
    pub fn copy_to_device(
        stream: &'stream Stream,
        source: PinnedHostBuffer<T>,
        destination: DeviceBuffer<T>,
    ) -> Result<Self> {
        validate_copy(
            stream,
            source.context().device_id(),
            "pinned host source",
            source.len(),
            destination.context().device_id(),
            "device destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_mut_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_device_ptr() }.cast::<c_void>();
        submit_owned(stream, (source, destination), || {
            if size == 0 {
                return Ok(());
            }
            check(unsafe {
                fe2o3_hip_sys::hipMemcpyAsync(
                    destination_ptr,
                    source_ptr,
                    size,
                    fe2o3_hip_sys::HIP_MEMCPY_HOST_TO_DEVICE,
                    stream.raw(),
                )
            })
        })
    }
}

impl<'stream, T: DeviceCopy> OwnedDeviceOperation<'stream, (DeviceBuffer<T>, PinnedHostBuffer<T>)> {
    /// Enqueues a device-to-pinned-host copy and retains both buffers.
    pub fn copy_to_host(
        stream: &'stream Stream,
        source: DeviceBuffer<T>,
        destination: PinnedHostBuffer<T>,
    ) -> Result<Self> {
        validate_copy(
            stream,
            source.context().device_id(),
            "device source",
            source.len(),
            destination.context().device_id(),
            "pinned host destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_device_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_mut_ptr() }.cast::<c_void>();
        submit_owned(stream, (source, destination), || {
            if size == 0 {
                return Ok(());
            }
            check(unsafe {
                fe2o3_hip_sys::hipMemcpyAsync(
                    destination_ptr,
                    source_ptr,
                    size,
                    fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_HOST,
                    stream.raw(),
                )
            })
        })
    }
}

impl<'stream, T: DeviceCopy> OwnedDeviceOperation<'stream, (DeviceBuffer<T>, DeviceBuffer<T>)> {
    /// Enqueues a device-to-device copy and retains both buffers.
    pub fn copy_device_to_device(
        stream: &'stream Stream,
        source: DeviceBuffer<T>,
        destination: DeviceBuffer<T>,
    ) -> Result<Self> {
        validate_copy(
            stream,
            source.context().device_id(),
            "device source",
            source.len(),
            destination.context().device_id(),
            "device destination",
            destination.len(),
        )?;

        let size = copy_byte_len::<T>(source.len())?;
        let source_ptr = unsafe { source.raw_device_ptr() }.cast::<c_void>();
        let destination_ptr = unsafe { destination.raw_device_ptr() }.cast::<c_void>();
        submit_owned(stream, (source, destination), || {
            if size == 0 {
                return Ok(());
            }
            check(unsafe {
                fe2o3_hip_sys::hipMemcpyAsync(
                    destination_ptr,
                    source_ptr,
                    size,
                    fe2o3_hip_sys::HIP_MEMCPY_DEVICE_TO_DEVICE,
                    stream.raw(),
                )
            })
        })
    }
}

fn submit_owned<'stream, R>(
    stream: &'stream Stream,
    resources: R,
    enqueue: impl FnOnce() -> Result<()>,
) -> Result<OwnedDeviceOperation<'stream, R>> {
    let pending = submit_owned_with(HipOperationRuntime { stream }, resources, enqueue)?;
    Ok(OwnedDeviceOperation { pending })
}

fn submit_owned_with<B: OperationRuntime, R>(
    backend: B,
    resources: R,
    enqueue: impl FnOnce() -> Result<()>,
) -> Result<PendingOwned<R, B::Completion>> {
    let event = backend.create_event()?;
    let mut submission = OwnedSubmission::new(backend, event, resources);
    submission.work_may_be_pending = true;

    if let Err(error) = enqueue() {
        return Err(submission.recover(error));
    }
    if let Err(error) = submission.record_event() {
        return Err(submission.recover(error));
    }

    Ok(submission.into_pending())
}

trait OperationRuntime {
    type Event;
    type Completion: Completion<Error = Error>;

    fn create_event(&self) -> Result<Self::Event>;
    fn record_event(&self, event: &mut Self::Event) -> Result<()>;
    fn synchronize_stream(&self) -> Result<()>;
    fn make_completion(&self, event: Self::Event) -> Self::Completion;
}

#[derive(Clone, Copy)]
struct HipOperationRuntime<'stream> {
    stream: &'stream Stream,
}

fn run_borrowed_retained<'stream, 'resources, R, O>(
    stream: &'stream Stream,
    resources: R,
    enqueue: impl FnOnce(&R) -> Result<()>,
    during: impl for<'operation> FnOnce(&'operation BorrowedDeviceOperation<'stream, 'resources>) -> O,
) -> Result<O> {
    let completion = begin_borrowed_with(HipOperationRuntime { stream }, || enqueue(&resources))?;
    let operation = BorrowedDeviceOperation {
        completion: Some(completion),
        _resources: PhantomData,
    };
    let output = during(&operation);
    operation.finish()?;
    drop(resources);
    Ok(output)
}

fn begin_borrowed_with<B: OperationRuntime>(
    backend: B,
    enqueue: impl FnOnce() -> Result<()>,
) -> Result<B::Completion> {
    let event = backend.create_event()?;
    let mut submission = BorrowedSubmission::new(backend, event);
    submission.work_may_be_pending = true;

    if let Err(error) = enqueue() {
        return Err(submission.recover(error));
    }
    if let Err(error) = submission.record_event() {
        return Err(submission.recover(error));
    }

    Ok(submission.into_completion())
}

struct BorrowedSubmission<B: OperationRuntime> {
    backend: B,
    event: Option<B::Event>,
    work_may_be_pending: bool,
}

impl<B: OperationRuntime> BorrowedSubmission<B> {
    fn new(backend: B, event: B::Event) -> Self {
        Self {
            backend,
            event: Some(event),
            work_may_be_pending: false,
        }
    }

    fn record_event(&mut self) -> Result<()> {
        self.backend
            .record_event(self.event.as_mut().expect("submission event is present"))
    }

    fn recover(mut self, operation: Error) -> Error {
        self.work_may_be_pending = false;
        match complete_borrowed(|| Err(operation), || self.backend.synchronize_stream()) {
            Err(CompletionError::Operation(operation)) => operation,
            Ok(())
            | Err(CompletionError::Synchronization(_))
            | Err(CompletionError::OperationAndSynchronization { .. }) => {
                unreachable!("borrowed recovery always reports its original operation error")
            }
        }
    }

    fn into_completion(mut self) -> B::Completion {
        let event = self.event.take().expect("submission event is present");
        let completion = self.backend.make_completion(event);
        self.work_may_be_pending = false;
        completion
    }
}

impl<B: OperationRuntime> Drop for BorrowedSubmission<B> {
    fn drop(&mut self) {
        if self.work_may_be_pending {
            self.work_may_be_pending = false;
            complete_borrowed(
                || Ok::<(), core::convert::Infallible>(()),
                || self.backend.synchronize_stream(),
            )
            .expect("borrowed submission recovery operation is infallible");
        }
    }
}

impl<'stream> OperationRuntime for HipOperationRuntime<'stream> {
    type Event = Event;
    type Completion = HipCompletion<'stream>;

    fn create_event(&self) -> Result<Self::Event> {
        Event::with_options(self.stream.context(), EventOptions::new().without_timing())
    }

    fn record_event(&self, event: &mut Self::Event) -> Result<()> {
        event.record(self.stream)
    }

    fn synchronize_stream(&self) -> Result<()> {
        self.stream.synchronize()
    }

    fn make_completion(&self, event: Self::Event) -> Self::Completion {
        HipCompletion {
            event,
            stream: self.stream,
        }
    }
}

struct OwnedSubmission<B: OperationRuntime, R> {
    backend: B,
    event: Option<B::Event>,
    resources: Option<R>,
    work_may_be_pending: bool,
}

impl<B: OperationRuntime, R> OwnedSubmission<B, R> {
    fn new(backend: B, event: B::Event, resources: R) -> Self {
        Self {
            backend,
            event: Some(event),
            resources: Some(resources),
            work_may_be_pending: false,
        }
    }

    fn record_event(&mut self) -> Result<()> {
        self.backend
            .record_event(self.event.as_mut().expect("submission event is present"))
    }

    fn recover(mut self, operation: Error) -> Error {
        self.work_may_be_pending = false;
        let retained = self.take_retained();
        match complete_owned(
            retained,
            || Err(operation),
            || self.backend.synchronize_stream(),
        ) {
            Err(CompletionError::Operation(operation)) => operation,
            Err(CompletionError::OperationAndSynchronization {
                operation,
                synchronization,
            }) => Error::OperationRecoveryFailed {
                operation: Box::new(operation),
                synchronization: Box::new(synchronization),
            },
            Ok(_) | Err(CompletionError::Synchronization(_)) => {
                unreachable!("the recovery operation always reports its original error")
            }
        }
    }

    fn into_pending(mut self) -> PendingOwned<R, B::Completion> {
        let event = self.event.take().expect("submission event is present");
        let completion = self.backend.make_completion(event);
        let resources = self
            .resources
            .take()
            .expect("submission resources are present");
        self.work_may_be_pending = false;
        PendingOwned::new(resources, completion)
    }
    fn take_retained(&mut self) -> (R, Option<B::Event>) {
        (
            self.resources
                .take()
                .expect("submission resources are present"),
            self.event.take(),
        )
    }
}

impl<B: OperationRuntime, R> Drop for OwnedSubmission<B, R> {
    fn drop(&mut self) {
        if self.work_may_be_pending {
            self.work_may_be_pending = false;
            let retained = self.take_retained();
            let _ = complete_owned(
                retained,
                || Ok::<(), core::convert::Infallible>(()),
                || self.backend.synchronize_stream(),
            );
        }
    }
}

#[derive(Debug)]
struct HipCompletion<'stream> {
    event: Event,
    stream: &'stream Stream,
}

impl Completion for HipCompletion<'_> {
    type Error = Error;

    fn query(&self) -> Result<bool> {
        self.event.query()
    }

    fn synchronize(&self) -> core::result::Result<(), CompletionFailure<Error>> {
        let result = synchronize_with_fallback(
            || self.event.synchronize(),
            || self.stream.synchronize(),
            |event, stream| Error::OperationSynchronizationFailed {
                event: Box::new(event),
                stream: Box::new(stream),
            },
        );
        #[cfg(test)]
        if matches!(&result, Ok(()) | Err(CompletionFailure::Quiescent(_))) {
            HIP_COMPLETION_OBSERVATIONS.with(|observations| {
                observations.set(observations.get() + 1);
            });
        }
        result
    }
}

#[cfg(test)]
std::thread_local! {
    static HIP_COMPLETION_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn validate_copy(
    stream: &Stream,
    source_device: i32,
    source_role: &'static str,
    source_len: usize,
    destination_device: i32,
    destination_role: &'static str,
    destination_len: usize,
) -> Result<()> {
    let stream_device = stream.context().device_id();
    ensure_operation_device(source_role, source_device, stream_device)?;
    ensure_operation_device(destination_role, destination_device, stream_device)?;
    ensure_copy_lengths(source_len, destination_len)
}

fn ensure_operation_device(
    resource: &'static str,
    resource_device: i32,
    stream_device: i32,
) -> Result<()> {
    if resource_device == stream_device {
        Ok(())
    } else {
        Err(Error::OperationDeviceMismatch {
            resource,
            resource_device,
            stream_device,
        })
    }
}

fn ensure_copy_lengths(source_len: usize, destination_len: usize) -> Result<()> {
    if source_len == destination_len {
        Ok(())
    } else {
        Err(Error::OperationLengthMismatch {
            source_len,
            destination_len,
        })
    }
}

fn copy_byte_len<T>(len: usize) -> Result<usize> {
    len.checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::{
        BorrowedDeviceOperation, HIP_COMPLETION_OBSERVATIONS, OperationRuntime,
        OwnedDeviceOperation, begin_borrowed_with, copy_byte_len, ensure_copy_lengths,
        ensure_operation_device, finish_borrowed_completion, submit_owned_with,
    };
    use crate::{DeviceBuffer, Error, GpuContext, PinnedHostBuffer};
    use fe2o3_completion::{Completion, CompletionFailure, synchronize_with_fallback};
    use std::cell::RefCell;
    use std::process::Command;
    use std::rc::Rc;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    const BORROWED_ABORT_CASE: &str = "FE2O3_BORROWED_OPERATION_ABORT_CASE";

    #[derive(Clone, Copy, Debug, Default)]
    enum Fault {
        #[default]
        Ok,
        Error,
        Panic,
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct Faults {
        record: Fault,
        make_completion: Fault,
        recovery_sync: Fault,
        event_sync: Fault,
        fallback_sync: Fault,
        event_drop: Fault,
    }

    #[derive(Clone)]
    struct FakeRuntime {
        events: Rc<RefCell<Vec<&'static str>>>,
        faults: Faults,
    }

    impl FakeRuntime {
        fn new(faults: Faults) -> Self {
            Self {
                events: Rc::new(RefCell::new(Vec::new())),
                faults,
            }
        }

        fn step(&self, name: &'static str, fault: Fault, error: Error) -> crate::Result<()> {
            self.events.borrow_mut().push(name);
            match fault {
                Fault::Ok => Ok(()),
                Fault::Error => Err(error),
                Fault::Panic => panic!("injected {name} panic"),
            }
        }

        fn resource(&self) -> DropRecorder {
            DropRecorder(self.events.clone())
        }
    }

    struct DropRecorder(Rc<RefCell<Vec<&'static str>>>);

    impl Drop for DropRecorder {
        fn drop(&mut self) {
            self.0.borrow_mut().push("resource-drop");
        }
    }

    struct FakeEvent {
        runtime: FakeRuntime,
    }

    impl Drop for FakeEvent {
        fn drop(&mut self) {
            self.runtime.events.borrow_mut().push("event-drop");
            if matches!(self.runtime.faults.event_drop, Fault::Panic) {
                panic!("injected event destructor panic");
            }
        }
    }

    struct FakeCompletion {
        event: FakeEvent,
    }

    impl Completion for FakeCompletion {
        type Error = Error;

        fn query(&self) -> crate::Result<bool> {
            self.event.runtime.events.borrow_mut().push("query");
            Ok(false)
        }

        fn synchronize(&self) -> core::result::Result<(), CompletionFailure<Self::Error>> {
            let runtime = &self.event.runtime;
            synchronize_with_fallback(
                || {
                    runtime.step(
                        "event-sync",
                        runtime.faults.event_sync,
                        Error::EventTimingDisabled,
                    )
                },
                || {
                    runtime.step(
                        "fallback-sync",
                        runtime.faults.fallback_sync,
                        stream_error(),
                    )
                },
                |event, stream| Error::OperationSynchronizationFailed {
                    event: Box::new(event),
                    stream: Box::new(stream),
                },
            )
        }
    }

    impl OperationRuntime for FakeRuntime {
        type Event = FakeEvent;
        type Completion = FakeCompletion;

        fn create_event(&self) -> crate::Result<Self::Event> {
            self.events.borrow_mut().push("create-event");
            Ok(FakeEvent {
                runtime: self.clone(),
            })
        }

        fn record_event(&self, _event: &mut Self::Event) -> crate::Result<()> {
            self.step("record-event", self.faults.record, Error::EventPending)
        }

        fn synchronize_stream(&self) -> crate::Result<()> {
            self.step("recovery-sync", self.faults.recovery_sync, stream_error())
        }

        fn make_completion(&self, event: Self::Event) -> Self::Completion {
            self.events.borrow_mut().push("into-completion");
            if matches!(self.faults.make_completion, Fault::Panic) {
                panic!("injected completion construction panic");
            }
            FakeCompletion { event }
        }
    }

    fn stream_error() -> Error {
        Error::DeviceMismatch {
            buffer_device: 1,
            stream_device: 2,
        }
    }

    fn log(runtime: &FakeRuntime) -> Vec<&'static str> {
        runtime.events.borrow().clone()
    }

    fn reset_hip_completion_observations() {
        HIP_COMPLETION_OBSERVATIONS.with(|observations| observations.set(0));
    }

    fn hip_completion_observations() -> usize {
        HIP_COMPLETION_OBSERVATIONS.with(std::cell::Cell::get)
    }

    #[test]
    fn copy_shape_validation_is_exact_and_overflow_checked() {
        assert!(ensure_copy_lengths(4, 4).is_ok());
        assert!(matches!(
            ensure_copy_lengths(3, 4),
            Err(Error::OperationLengthMismatch {
                source_len: 3,
                destination_len: 4
            })
        ));
        assert_eq!(copy_byte_len::<u32>(4).unwrap(), 16);
        assert!(matches!(
            copy_byte_len::<u16>(usize::MAX),
            Err(Error::SizeOverflow)
        ));
        assert!(ensure_operation_device("source", 2, 2).is_ok());
        assert!(matches!(
            ensure_operation_device("source", 2, 3),
            Err(Error::OperationDeviceMismatch {
                resource: "source",
                resource_device: 2,
                stream_device: 3
            })
        ));
    }

    #[test]
    fn owned_submission_recovers_enqueue_and_record_errors() {
        let enqueue_runtime = FakeRuntime::new(Faults::default());
        let error = submit_owned_with(enqueue_runtime.clone(), enqueue_runtime.resource(), || {
            enqueue_runtime.events.borrow_mut().push("enqueue");
            Err(Error::SizeOverflow)
        })
        .err()
        .expect("enqueue must fail");
        assert!(matches!(error, Error::SizeOverflow));
        assert_eq!(
            log(&enqueue_runtime),
            [
                "create-event",
                "enqueue",
                "recovery-sync",
                "resource-drop",
                "event-drop"
            ]
        );

        let record_runtime = FakeRuntime::new(Faults {
            record: Fault::Error,
            ..Faults::default()
        });
        let error = submit_owned_with(record_runtime.clone(), record_runtime.resource(), || {
            record_runtime.events.borrow_mut().push("enqueue");
            Ok(())
        })
        .err()
        .expect("record must fail");
        assert!(matches!(error, Error::EventPending));
        assert_eq!(
            log(&record_runtime),
            [
                "create-event",
                "enqueue",
                "record-event",
                "recovery-sync",
                "resource-drop",
                "event-drop"
            ]
        );
    }

    #[test]
    fn owned_submission_drop_recovers_enqueue_and_record_panics() {
        let enqueue_runtime = FakeRuntime::new(Faults::default());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(enqueue_runtime.clone(), enqueue_runtime.resource(), || {
                enqueue_runtime.events.borrow_mut().push("enqueue");
                panic!("injected enqueue panic")
            });
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&enqueue_runtime),
            [
                "create-event",
                "enqueue",
                "recovery-sync",
                "resource-drop",
                "event-drop"
            ]
        );

        let record_runtime = FakeRuntime::new(Faults {
            record: Fault::Panic,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(record_runtime.clone(), record_runtime.resource(), || Ok(()));
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&record_runtime),
            [
                "create-event",
                "record-event",
                "recovery-sync",
                "resource-drop",
                "event-drop"
            ]
        );
    }

    #[test]
    fn completion_construction_panic_runs_owned_submission_recovery() {
        let runtime = FakeRuntime::new(Faults {
            make_completion: Fault::Panic,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(runtime.clone(), runtime.resource(), || Ok(()));
        }));

        assert!(panic.is_err());
        assert_eq!(
            log(&runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-drop",
                "recovery-sync",
                "resource-drop"
            ]
        );
    }

    #[test]
    fn completion_construction_panic_runs_borrowed_submission_recovery() {
        let runtime = FakeRuntime::new(Faults {
            make_completion: Fault::Panic,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = begin_borrowed_with(runtime.clone(), || Ok(()));
        }));

        assert!(panic.is_err());
        assert_eq!(
            log(&runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-drop",
                "recovery-sync"
            ]
        );
    }

    #[test]
    fn completion_construction_panic_leaks_owned_resources_if_recovery_is_ambiguous() {
        let runtime = FakeRuntime::new(Faults {
            make_completion: Fault::Panic,
            recovery_sync: Fault::Error,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(runtime.clone(), runtime.resource(), || Ok(()));
        }));

        assert!(panic.is_err());
        assert_eq!(
            log(&runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-drop",
                "recovery-sync"
            ]
        );
    }

    #[test]
    fn owned_submission_drop_leaks_if_recovery_sync_errors() {
        let runtime = FakeRuntime::new(Faults {
            recovery_sync: Fault::Error,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(runtime.clone(), runtime.resource(), || {
                runtime.events.borrow_mut().push("enqueue");
                panic!("injected enqueue panic")
            });
        }));

        assert!(panic.is_err());
        assert_eq!(log(&runtime), ["create-event", "enqueue", "recovery-sync"]);
    }

    #[test]
    fn owned_submission_leaks_when_recovery_is_ambiguous_or_panics() {
        let error_runtime = FakeRuntime::new(Faults {
            record: Fault::Error,
            recovery_sync: Fault::Error,
            ..Faults::default()
        });
        let error = submit_owned_with(error_runtime.clone(), error_runtime.resource(), || Ok(()))
            .err()
            .expect("record and recovery must fail");
        assert!(matches!(
            error,
            Error::OperationRecoveryFailed {
                operation,
                synchronization,
            } if matches!(*operation, Error::EventPending)
                && matches!(*synchronization, Error::DeviceMismatch { .. })
        ));
        assert_eq!(
            log(&error_runtime),
            ["create-event", "record-event", "recovery-sync"]
        );

        let panic_runtime = FakeRuntime::new(Faults {
            record: Fault::Error,
            recovery_sync: Fault::Panic,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = submit_owned_with(panic_runtime.clone(), panic_runtime.resource(), || Ok(()));
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&panic_runtime),
            ["create-event", "record-event", "recovery-sync"]
        );
    }

    #[test]
    fn owned_pending_distinguishes_quiescent_and_ambiguous_failures() {
        let quiescent_runtime = FakeRuntime::new(Faults {
            event_sync: Fault::Error,
            ..Faults::default()
        });
        let pending = submit_owned_with(
            quiescent_runtime.clone(),
            quiescent_runtime.resource(),
            || Ok(()),
        )
        .expect("submission must succeed");
        assert!(matches!(pending.wait(), Err(Error::EventTimingDisabled)));
        assert_eq!(
            log(&quiescent_runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "fallback-sync",
                "event-drop",
                "resource-drop"
            ]
        );

        let ambiguous_runtime = FakeRuntime::new(Faults {
            event_sync: Fault::Error,
            fallback_sync: Fault::Error,
            ..Faults::default()
        });
        let pending = submit_owned_with(
            ambiguous_runtime.clone(),
            ambiguous_runtime.resource(),
            || Ok(()),
        )
        .expect("submission must succeed");
        assert!(matches!(
            pending.wait(),
            Err(Error::OperationSynchronizationFailed { event, stream })
                if matches!(*event, Error::EventTimingDisabled)
                    && matches!(*stream, Error::DeviceMismatch { .. })
        ));
        assert_eq!(
            log(&ambiguous_runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "fallback-sync"
            ]
        );
    }

    #[test]
    fn owned_pending_leaks_on_fallback_or_destructor_panic() {
        let fallback_runtime = FakeRuntime::new(Faults {
            event_sync: Fault::Error,
            fallback_sync: Fault::Panic,
            ..Faults::default()
        });
        let pending = submit_owned_with(
            fallback_runtime.clone(),
            fallback_runtime.resource(),
            || Ok(()),
        )
        .expect("submission must succeed");
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = pending.wait();
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&fallback_runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "fallback-sync"
            ]
        );

        let destructor_runtime = FakeRuntime::new(Faults {
            event_drop: Fault::Panic,
            ..Faults::default()
        });
        let pending = submit_owned_with(
            destructor_runtime.clone(),
            destructor_runtime.resource(),
            || Ok(()),
        )
        .expect("submission must succeed");
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = pending.wait();
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&destructor_runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "event-drop"
            ]
        );
    }

    #[test]
    fn borrowed_submission_recovers_enqueue_and_record_errors() {
        let enqueue_runtime = FakeRuntime::new(Faults::default());
        let error = begin_borrowed_with(enqueue_runtime.clone(), || Err(Error::SizeOverflow))
            .err()
            .expect("enqueue must fail");
        assert!(matches!(error, Error::SizeOverflow));
        assert_eq!(
            log(&enqueue_runtime),
            ["create-event", "recovery-sync", "event-drop"]
        );

        let record_runtime = FakeRuntime::new(Faults {
            record: Fault::Error,
            ..Faults::default()
        });
        let error = begin_borrowed_with(record_runtime.clone(), || Ok(()))
            .err()
            .expect("record must fail");
        assert!(matches!(error, Error::EventPending));
        assert_eq!(
            log(&record_runtime),
            [
                "create-event",
                "record-event",
                "recovery-sync",
                "event-drop"
            ]
        );
    }

    #[test]
    fn borrowed_submission_drop_recovers_enqueue_and_record_panics() {
        let enqueue_runtime = FakeRuntime::new(Faults::default());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ =
                begin_borrowed_with(enqueue_runtime.clone(), || panic!("injected enqueue panic"));
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&enqueue_runtime),
            ["create-event", "recovery-sync", "event-drop"]
        );

        let record_runtime = FakeRuntime::new(Faults {
            record: Fault::Panic,
            ..Faults::default()
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = begin_borrowed_with(record_runtime.clone(), || Ok(()));
        }));
        assert!(panic.is_err());
        assert_eq!(
            log(&record_runtime),
            [
                "create-event",
                "record-event",
                "recovery-sync",
                "event-drop"
            ]
        );
    }

    #[test]
    fn borrowed_settlement_returns_quiescent_error_and_drops_completion() {
        let runtime = FakeRuntime::new(Faults {
            event_sync: Fault::Error,
            ..Faults::default()
        });
        let mut completion =
            Some(begin_borrowed_with(runtime.clone(), || Ok(())).expect("submission must succeed"));

        assert!(matches!(
            finish_borrowed_completion(&mut completion),
            Err(Error::EventTimingDisabled)
        ));
        assert!(completion.is_none());
        assert_eq!(
            log(&runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "fallback-sync",
                "event-drop"
            ]
        );
    }

    #[test]
    fn borrowed_settlement_propagates_completion_destructor_panic_after_quiescence() {
        let runtime = FakeRuntime::new(Faults {
            event_drop: Fault::Panic,
            ..Faults::default()
        });
        let mut completion =
            Some(begin_borrowed_with(runtime.clone(), || Ok(())).expect("submission must succeed"));

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = finish_borrowed_completion(&mut completion);
        }));
        assert!(panic.is_err());
        assert!(completion.is_none());
        assert_eq!(
            log(&runtime),
            [
                "create-event",
                "record-event",
                "into-completion",
                "event-sync",
                "event-drop"
            ]
        );
    }

    #[test]
    fn borrowed_state_machine_aborts_on_ambiguous_completion() {
        if let Ok(case) = std::env::var(BORROWED_ABORT_CASE) {
            let (faults, enqueue): (Faults, fn() -> crate::Result<()>) = match case.as_str() {
                "enqueue-error-recovery-error" => (
                    Faults {
                        recovery_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || Err(Error::SizeOverflow),
                ),
                "enqueue-error-recovery-panic" => (
                    Faults {
                        recovery_sync: Fault::Panic,
                        ..Faults::default()
                    },
                    || Err(Error::SizeOverflow),
                ),
                "record-error-recovery-error" => (
                    Faults {
                        record: Fault::Error,
                        recovery_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || Ok(()),
                ),
                "enqueue-panic-recovery-error" => (
                    Faults {
                        recovery_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || panic!("injected enqueue panic"),
                ),
                "record-panic-recovery-error" => (
                    Faults {
                        record: Fault::Panic,
                        recovery_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || Ok(()),
                ),
                "completion-construction-panic-recovery-error" => (
                    Faults {
                        make_completion: Fault::Panic,
                        recovery_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || Ok(()),
                ),
                "completion-ambiguous" => (
                    Faults {
                        event_sync: Fault::Error,
                        fallback_sync: Fault::Error,
                        ..Faults::default()
                    },
                    || Ok(()),
                ),
                "completion-fallback-panic" => (
                    Faults {
                        event_sync: Fault::Error,
                        fallback_sync: Fault::Panic,
                        ..Faults::default()
                    },
                    || Ok(()),
                ),
                _ => panic!("unknown borrowed abort case"),
            };
            let runtime = FakeRuntime::new(faults);
            let mut completion = begin_borrowed_with(runtime, enqueue).ok().map(Some);
            if let Some(completion) = completion.as_mut() {
                let _ = finish_borrowed_completion(completion);
            }
            std::process::exit(99);
        }

        for case in [
            "enqueue-error-recovery-error",
            "enqueue-error-recovery-panic",
            "record-error-recovery-error",
            "enqueue-panic-recovery-error",
            "record-panic-recovery-error",
            "completion-construction-panic-recovery-error",
            "completion-ambiguous",
            "completion-fallback-panic",
        ] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "operation::tests::borrowed_state_machine_aborts_on_ambiguous_completion",
                    "--nocapture",
                ])
                .env(BORROWED_ABORT_CASE, case)
                .output()
                .unwrap();
            assert_ne!(
                output.status.code(),
                Some(99),
                "borrowed case {case} returned instead of aborting"
            );
            #[cfg(unix)]
            assert_eq!(
                output.status.signal(),
                Some(6),
                "borrowed case {case} did not terminate with SIGABRT"
            );
        }
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn owned_copies_round_trip_and_return_resources() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.create_stream()?;
        let source = PinnedHostBuffer::from_slice(&context, &[3_u32, 1, 4, 1, 5])?;
        let destination = DeviceBuffer::zeroed(&stream, source.len())?;

        let upload = OwnedDeviceOperation::copy_to_device(&stream, source, destination)?;
        let (source, destination) = upload.wait()?;
        assert_eq!(source.as_slice(), [3, 1, 4, 1, 5]);

        let host_output = PinnedHostBuffer::filled(&context, destination.len(), 0_u32)?;
        let download = OwnedDeviceOperation::copy_to_host(&stream, destination, host_output)?;
        let (_destination, host_output) = download.wait()?;
        assert_eq!(host_output.as_slice(), [3, 1, 4, 1, 5]);
        Ok(())
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn dropping_submitted_owned_copy_observes_hip_completion() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.create_stream()?;
        let source = PinnedHostBuffer::from_slice(&context, &[8_u32, 13, 21])?;
        let destination = DeviceBuffer::zeroed(&stream, source.len())?;

        reset_hip_completion_observations();
        let operation = OwnedDeviceOperation::copy_to_device(&stream, source, destination)?;
        drop(operation);

        assert_eq!(hip_completion_observations(), 1);
        Ok(())
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn scoped_borrowed_copies_round_trip() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.create_stream()?;
        let source = PinnedHostBuffer::from_slice(&context, &[2_u32, 7, 1, 8, 2, 8])?;
        let mut first_device = DeviceBuffer::zeroed(&stream, source.len())?;
        let callback_ran = std::cell::Cell::new(false);

        BorrowedDeviceOperation::copy_to_device(
            &stream,
            &source,
            &mut first_device,
            |operation| {
                let _snapshot = operation.is_complete().unwrap();
                callback_ran.set(true);
            },
        )?;
        assert!(callback_ran.get());

        let mut second_device = DeviceBuffer::zeroed(&stream, source.len())?;
        BorrowedDeviceOperation::copy_device_to_device(
            &stream,
            &first_device,
            &mut second_device,
            |_| {},
        )?;

        let mut output = PinnedHostBuffer::filled(&context, source.len(), 0_u32)?;
        BorrowedDeviceOperation::copy_to_host(&stream, &second_device, &mut output, |_| {})?;
        assert_eq!(output.as_slice(), [2, 7, 1, 8, 2, 8]);
        Ok(())
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn unwinding_borrowed_callback_observes_hip_completion() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.create_stream()?;
        let source = PinnedHostBuffer::from_slice(&context, &[89_u32, 144, 233])?;
        let mut destination = DeviceBuffer::zeroed(&stream, source.len())?;

        reset_hip_completion_observations();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ =
                BorrowedDeviceOperation::copy_to_device(&stream, &source, &mut destination, |_| {
                    panic!("exercise borrowed operation drop")
                });
        }));

        assert!(panic.is_err());
        assert_eq!(hip_completion_observations(), 1);
        Ok(())
    }
}
