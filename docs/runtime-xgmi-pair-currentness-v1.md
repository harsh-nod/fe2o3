# XGMI Pair Currentness V1

Development implementation of one fresh topology observation per full XGMI
pair-validation boundary. The measured baseline in
`docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18` spent roughly
99.99% of its measured lower-call host intervals in currentness checks. That
measurement includes more than discovery and does not establish a speedup for
this revision.

## Observation Boundary

The private Linux path behind
`SharedGttMemorySessionV1::validate_gfx942_xgmi_route_with_peer` retains its pure
opening binding check: active sessions, distinct devices in the exact canonical
GPU roster, and both retained directional routes equal to the requested route.
No observation occurs after a binding rejection.

The full pair sequence is then:

1. Source full prechecks: KFD opener process, process incarnation, reset fence,
   KFD and render descriptors, UAPI, DRM identity and VRAM-loss counter, XNACK,
   and process apertures.
2. Peer full prechecks, retaining that endpoint's own process and DRM values.
3. One fresh host topology discovery; admit and compare the exact directional
   route from that same snapshot, then compare the complete retained base
   snapshot with both endpoints under its existing equality contract.
4. Source full postchecks: KFD and render descriptors, process incarnation,
   XNACK, DRM identity and VRAM-loss counter, and reset fence.
5. Peer full postchecks against the peer's own precheck values.

The snapshot is local to this call. It is not cloned, retained as a certificate,
or reused across calls, directions, opening/closing boundaries, publication,
completion, or mapping operations. Generation-counter equality never bypasses
discovery. The ordinary single-device full check uses the same private pre/post
driver with its original one-discovery sequence and error mapping.

Both endpoint observation intervals contain the shared discovery. They are
overlapping, not a proof of an atomic kernel snapshot or continuous topology
stability. A sysfs-only change after discovery that does not affect a later
endpoint observation can remain undetected. This is not equivalence to the
older four distinct discovery times. Existing reset-subscription windows,
unreported reset classes, wrapping counters, ABA, and final-fence limitations
remain.

The existing base-topology equality excludes the additive SDMA capability
sidecar. Fresh route admission separately checks the source-side engine
inventory required by the selected route. The optimization also consolidates
the older independent route and full process/reset checks into one full
pre/post interval per endpoint; it does not merely substitute discovery values
inside four otherwise unchanged intervals.

## Failure And Custody

After pure binding, both session phases are pre-latched as quarantined. Both
device-currentness flags are also pre-latched before the first observation.
Only complete success restores usable state. An error or unwind therefore
retains both terminal states without invoking a fallible terminalizer or
replacing the original error or panic payload. Prior poison is never cleared.

Observations short-circuit in the stated order. A failed source postcheck does
not attempt the peer postchecks; both endpoints still remain terminal and no
success is issued. Fresh-route rejection precedes base-snapshot rejection.
The new overlapping sequence has its own deterministic first-error order,
rather than preserving every ordering among the old independent audits.

No global-gate behavior or cleanup authority is added. Creation and retirement
retain their existing owning roots and terminalization. The generic memory
backend trait, peer map/unmap envelopes, and BatchScoped per-operation process
and reset checks are unchanged. Full checks at batch edges still discover
separately. Four discoveries per full pair boundary become one; each lower
submit or poll still has separate opening and closing boundaries.

## Verification Boundary

Scripted CPU observations exercise the production shared driver, including
individual callback errors/panics, identity mismatches, both retained snapshots,
directional route rejection, independent fresh calls, inert poisoned reentry,
and original panic identity. The callback sweep composes device latches with
the production session guard. Source-wiring checks are supplementary, not
native execution.

R9's extensional route/currentness predicate and R28's full-audit counts are not
changed. Their existing mathematical proofs do not establish this Rust/native
observation ordering, two-device snapshot semantics, driver correctness, or
machine-code refinement. Native fault injection, new native timing evidence,
matched HIP/HSA acceptance, and general parity remain separate requirements.

## Planned Matched Comparison

This protocol is preparation, not an executed or accepted experiment. Rebuild
the baseline from signed commit `84b61ee39817cee8bfe3cd68312f0a05543bc9af`
and the signed CPU-qualified revision accompanying this document. Bind each
source archive to its own sealed CPU file map and signed Git tree, record the
exact source difference, and use identical toolchain, lockfile, release profile,
and `hardware-diagnostic` features. Use separate build outputs and hash both
binaries. The historical native run is not the matched baseline measurement.

Predeclare these eight separate processes:

1. Baseline diagnostics on.
2. Candidate diagnostics on.
3. Candidate diagnostics on.
4. Baseline diagnostics on.
5. Candidate diagnostics off.
6. Baseline diagnostics off.
7. Baseline diagnostics off.
8. Candidate diagnostics off.

Each process uses the same physical pair, 1 MiB, depth one, ten warmups, thirty
samples, and both directions. Select two currently free GPUs; both must pass
fresh endpoint admission before each workload, followed by the existing
settled checks at least two seconds after process closure and delayed checks
at least twenty seconds after both settled checks finish.
These observations are not an exclusive reservation or continuous-isolation
proof. Stop the workload suffix on a failure, preserve the original failure,
collect before deleting only owned resources, and verify path/process absence.

Keep each process, diagnostic mode, direction, and remap/hot population separate.
Report the two process replicates and descriptive ratios, not thirty independent
replicates or statistical significance. Host-stage timings are not GPU timings
or a discovery counter. Balanced ordering does not remove all shared-host or
interconnect confounding. Keep `performance_acceptance=false`; matched HIP/HSA,
high-depth/concurrent operation, native failure injection, and formal refinement
are outside this proposed source-to-source experiment.
