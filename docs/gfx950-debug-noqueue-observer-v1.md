# Standalone gfx950 no-queue metadata observer

This separately named engineering example calls the unsafe consuming registration
API only from a small, synchronous, disposable-program path. The older
`observe_gfx950_cold_debug_v1` example and its argument grammar are unchanged.
The [root qualification](evidence/gfx950-debug-noqueue-20260924.md) passed CPU/lint
gates, eight exact early refusals and one supervised native registration.
It does not qualify execution, capture, attached-debugger acceptance or cleanup.

## Unsafe caller proof and observable fences

The unsafe call requires whole-process exclusion of foreign KFD/ROCr runtimes
and queues, concurrent or future activity, and external code injection until
process exit. The observer does not establish that general property by reading
/proc. Its safety case is the reviewed standalone program and dependency closure
plus a trusted supervisor controlling fresh exec, inherited descriptors,
environment, debugger/injection access, timeout, termination and reaping.

The standalone source path does not create threads, spawn processes, load plugins
or GPU runtime libraries, create a queue or dispatch a kernel. It calls only
normal file/device admission, actual cold preparation and the private no-queue
successor. Neither copying this module into another program nor adding an
acknowledgment flag authenticates the necessary process-wide exclusion.

Before parsing arguments or opening KFD, entry fences require one process-main
thread, no preexisting KFD or DRM character-device FDs, and a closed environment:
empty, or only LANG/LC_ALL with exact C or C.UTF-8 values. Unknown variables,
including loader/tool/injection variables even with empty values, refuse.
A nonempty /etc/ld.so.preload also refuses.

Bounded /proc/self/maps checks allow the exact current executable and canonical
Ubuntu x86_64 paths for libc.so.6, libgcc_s.so.1 and ld-linux-x86-64.so.2 only.
Known heap/stack/vvar/vdso/vsyscall mappings are allowed; anonymous executable,
writable-executable, deleted or unreviewed foreign file mappings refuse.
Mapped file device/inode identities must match current paths. These checks do
not hash loaded library contents: the supervisor must pin the binary, loader and
dependency bytes separately and prevent mutation/injection.

The fences repeat before device admission, before activation and after activation.
After cold preparation they permit exactly one KFD FD and one DRM FD owned through
the normal checked-device path, and nonexecutable mappings of those exact device
paths. This is a narrow expected roster, not an import of preexisting GPU activity.
Any unsupported platform library layout or unexpected mapping refuses; it must
not be fixed by adding wildcard allowances.

Snapshots are individually racy observations and cannot detect all prior activity,
constructors that hid evidence, concurrent external manipulation, or future
activity. The unsafe proof remains the audited call/dependency closure and
supervisor lifetime contract. The JSON explicitly reports that /proc does not
prove general foreign exclusion.

## Explicit opt-in interface

Built with `engineering-gfx950`, the new example takes exactly:

```text
observe_gfx950_debug_metadata_noqueue_v1
  --allow-vm-mapping
  --retain-until-process-exit
  --acknowledge-isolated-noqueue-activation
  HSACO BYTES SHA256 KERNEL NODE UNIQUE_ID GPU_ID DEVICE_PROFILE_SHA256
```

All three acknowledgments must appear in this order. The artifact/device fields
retain the cold probe's bounded, exact grammar. Artifact handling reuses the
unchanged cold probe's read-only, no-follow, retained-byte/metadata rechecks.
It still requires exact current selected-device identity/profile and the pinned
1116-byte trap digest. Each actual artifact snapshot is rechecked around native
effects; no path string or diagnostic record substitutes for owner custody.

The external supervisor must use a fresh exec with a deliberately empty or
allowed locale-only environment, close unrelated inherited descriptors and
bound wall time externally. Do not run this inside an existing GPU application,
cargo runner that injects runtime libraries, profiler, preload wrapper or reused
worker. The library's exclusive gate does not prove foreign-runtime exclusion.
KFD runtime enable may block for a debugger runtime-event acknowledgment.

## Output and lifetime

The single final JSON record is at most 4096 bytes. Success means the actual
retained owner acknowledged registration and publication, not that an attached
debugger accepted the metadata or any trap executed. It reports:

- Exact process/device/artifact/trap identities and retained-byte preparation facts.
- Metadata version 11, trap/runtime registration and code-object publication.
- No queue, dispatch, GPU trap qualification, physical sample or cleanup acknowledgment.
- Process-lifetime retention and the explicit standalone/supervisor caller contract.

Failure's `native_effects` field concerns debug preparation/activation; ordinary
read-only device admission can already have opened descriptors. Once cold
preparation is attempted, later failures report possible retained native effects.
No rollback, retry, runtime disable or trap-clear operation is attempted. Owner
Drop retains all actual native resources; the supervisor must observe process
exit and reaping, not infer it from JSON.

## Qualification and continuing requirements

The linked root record retains the actual qualification, including review of the
unsafe API and standalone dependency closure, feature-enabled compile-fail
documentation tests, both feature/lint gates and old cold-probe regressions.
A new native run requires the same caller/supervisor review for its exact inputs.

The CPU suite covers acknowledgment/pin grammar, lossless IDs, bounds, exact
environment/thread/FD/mapping policy, malformed maps and bounded readers without
opening /proc or /dev. Negative executable-probe controls should be separately
supervised and classified by whether they stop before native preparation.

The observed native run qualifies no-queue registration only. TMA zero remains
unqualified for actual sampling execution, and V4/same-stop physical visualization
requires separate consuming queue/debug ownership, trap/TMA qualification and
actual capture evidence. This observer does not implement cleanup or close V4.
