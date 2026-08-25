# Device operations

`fe2o3-core` keeps raw pointer APIs unsafe and exposes raw HIP module and launch
authority only with `qualification-raw-hip-test-only`. The device
operation API adds a narrow safe layer for whole-buffer asynchronous copies.
It does not schedule tasks, cancel work, infer kernel aliases, or make raw
kernel launches safe.

## Owned operations

An owned operation moves every participating buffer into the submitted handle:

```rust,no_run
use fe2o3_core::{DeviceBuffer, GpuContext, OwnedDeviceOperation, PinnedHostBuffer};

let context = GpuContext::new(0)?;
let stream = context.create_stream()?;
let host = PinnedHostBuffer::from_slice(&context, &[1_u32, 2, 3])?;
let device = DeviceBuffer::zeroed(&stream, host.len())?;

let operation = OwnedDeviceOperation::copy_to_device(&stream, host, device)?;
// Unrelated host work may run here.
let (host, device) = operation.wait()?;
# let _ = (host, device);
# Ok::<(), fe2o3_core::Error>(())
```

`wait` consumes the one-shot submitted state and returns the buffers only after
completion. Dropping the operation also waits; it is not cancellation. Safely
forgetting the operation leaks the owned buffers, so it cannot cause their
early destruction.

## Borrowed operations

A returned RAII handle cannot safely protect borrowed asynchronous memory:
safe code can call `mem::forget` on the handle and then regain access to the
owner. Borrowed copies therefore use a scoped callback:

```rust,no_run
use fe2o3_core::{BorrowedDeviceOperation, DeviceBuffer, GpuContext, PinnedHostBuffer};

let context = GpuContext::new(0)?;
let stream = context.create_stream()?;
let host = PinnedHostBuffer::from_slice(&context, &[1_u32, 2, 3])?;
let mut device = DeviceBuffer::zeroed(&stream, host.len())?;

BorrowedDeviceOperation::copy_to_device(&stream, &host, &mut device, |operation| {
    // Unrelated host work may run here while the copy is pending.
    let _snapshot = operation.is_complete();
})?;
// The callback cannot take ownership of the handle. Both borrows are released
// only after the completion event or its stream fallback has synchronized.
# Ok::<(), fe2o3_core::Error>(())
```

The destination remains exclusively borrowed. A host-to-device source remains
immutably borrowed, which permits only concurrent host reads. The method also
holds the stream borrow until synchronization finishes.

## Completion policy

Each operation creates a fresh timing-disabled event and records it after the
copy. Normal completion waits on that event. If event synchronization errors,
the implementation synchronizes the entire stream as the stronger fallback.
Successful fallback establishes quiescence but does not erase the event error:
owned resources are safely released and borrowed resources become accessible
again, then the original event error is returned.

If both mechanisms fail, completion is ambiguous:

- An owned operation leaks its event and all retained resources, then returns
  `Error::OperationSynchronizationFailed` from `wait`. Its `Drop` path performs
  the same leak without reporting an error.
- A borrowed operation leaks its event and aborts the process. It cannot return
  control safely because it does not own the resources needed for a leak.
- An enqueue or event-recording error also synchronizes the stream before any
  participating resource can be released. Owned resources are leaked, and a
  borrowed operation aborts, when that recovery synchronization also fails.

The API validates exact element counts and requires every buffer and stream to
name the same HIP device. Empty and zero-sized buffers still record an event,
which preserves stream ordering without passing null pointers to a copy call.

## Limits

This layer currently covers whole-buffer H2D, D2H, and D2D copies involving
`DeviceBuffer` and, for host transfers, `PinnedHostBuffer`. It does not yet
retain modules and typed kernel arguments for safe asynchronous launches. It
also cannot detect unsafe raw work submitted concurrently on another stream;
callers of raw APIs retain their documented synchronization and aliasing
obligations.
