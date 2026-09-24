# Matched XGMI Hot Batches

This extends the persistent-hot benchmark producers, not production runtime
authority or the A1/A2 acceptance boundary. The strict depth-1/16/32 native
campaign below qualifies the benchmark source, not subsequent optimizations
or A7 performance thresholds.

## Lifecycle

The KFD non-diagnostic `--aggregate-peer-batch-hot-only` mode and the HIP/HSA
`--persistent-hot` modes admit depths 1 through 32. Both KFD hot diagnostic
modes remain depth-one because their bounded diagnostic rosters are separate
contracts. Ordinary and ordered-segment paths are unchanged.

Each direction prepares every slot with a distinct payload pattern, guarded
source and independently guarded poisoned destination. One untimed prime batch
per direction precedes the warmups and samples. Timed HIP/HSA batches enqueue
every slot before waiting for any completion. HIP uses one nonblocking stream
per slot; HSA uses one signal per slot and resets every observed completion.
The original KFD path already submits the complete exact roster on its ordered
SDMA queue. No host initialization or readback interrupts the hot sequence.

After all timed batches, both native comparators read back and validate the
complete source and destination of every slot, including both guard regions.
A payload mismatch does not skip later buffers, slots or the other direction.
Native API failure retains the existing fatal-process error behavior; this
change does not add recovery or fault qualification.

The shared traversal helpers allocate nothing and are linear in depth. HIP
continues to reuse one synchronous host staging buffer. HSA retains its existing
per-slot upload/download allocations. Timed batches add no host payload copies.

## Reporting

`xgmi_peer_hot_results.py` accepts externally supplied trial controls and only
the matched depths 1, 16 and 32. It requires one exact, non-diagnostic backend
record; canonical ordered endpoint identities and gfx942 targets; the stated
lifecycle; positive ordered percentiles; and all applicable depth fields.
Guarded sizes, batch bytes and prime-round arithmetic must fit u64.

Each latency is for the whole one-direction batch. Bandwidth is
`copy_bytes * depth / batch_nanoseconds`, not one slot's bytes or both directions
combined. Dividing batch time by depth is an amortized cost, not an individual
copy latency percentile. Final canaries establish the final contents, not a
separate functional witness for each repeated intermediate copy.

The different host boundaries remain explicit: KFD facade enqueue through
aggregate close versus HIP/HSA native enqueue through observed completion.
Equal submitted depth does not establish equal physical engine concurrency.
HIP/HSA continue to report `runtime-selected-unknown`; KFD reports
`ordered-single-sdma`. Historical depth-one parsers and evidence are untouched.

## Qualification

The [CPU packet](evidence/dev-xgmi-hot-batch-cpu-2026-09-24/README.md) records
GNU/musl example tests, real native-comparator compilation, shared helper tests,
actual producer callbacks with instrumented CPU APIs, UBSan and expected-negative
mutations, and strict result parsing. Mock callbacks do not link GPU runtime
libraries and do not establish native scheduling, hardware support or speed.

The next native campaign must bind signed source and cold-built artifacts to
freshly admitted endpoints, use the declared depth/backend ordering, retain
each process/direction summary separately, check every buffer and explicit
teardown, and remove only its owned resources. No new Verus proof, whole-runtime
refinement, aggregate memory bound, A7 acceptance or HIP/HSA parity follows from
these benchmark changes.

## Currentness Cost Boundary

Source inspection of the steady-state persistent-hot route shows that facade
enqueues install deferred logical custody without discovering host topology.
The aggregate wait already shares one opening and one closing full-host
discovery across its complete directional roster, independent of depth. The
full-currentness pair helper also shares each discovery between both endpoints.
Thus deeper batches amortize fixed observation cost; removing an imagined
per-enqueue discovery does not offer another optimization.

The existing `peer_copy_segments` route is a separate wider completion unit:
one immutable segment list, one source/destination pair and one final logical
completion. It is an appropriate next amortization control. A distinct-buffer
multi-wave plan would need its own bounded cursor/custody contract, retained
roots, per-publication fences, no intermediate success and final full close.
Neither design can silently reuse a previous batch's topology observation or
change ordinary aggregate completion semantics. This is a source-inspection
finding and proposed direction, not a new proof or performance result.

The [fresh strict depth-1/16/32 campaign](evidence/dev-xgmi-hot-batch-strict-mi300x-2026-09-24/README.md)
now qualifies the unchanged benchmark source on GPUs 5/6. At depth 32 KFD
whole-batch p50 is 15.427-15.540 ms, HSA 1.065-1.087 ms and HIP 1.015-1.041 ms.
The retained per-process/direction summaries show amortization, not parity;
host boundaries and engine selection still differ. This does not qualify the
later link-directory source candidate or supply an A7 threshold pass.

The [link-directory CPU packet](evidence/dev-topology-link-directory-cpu-2026-09-24/README.md)
now qualifies removal of one redundant directory inspection per I/O/P2P link.
All listings, property reads, identity checks and currentness brackets remain.
GNU/musl library tests and exact named-roster replay pass, but the candidate has
no native baseline/candidate measurement yet. The instrumentation tests count
Rust I/O boundaries, not measured kernel syscalls or a demonstrated speedup.

A separately named retained-session profile could instead aim to preserve
execution-relevant authority with cheaper guards. It cannot claim to reproduce
the current full-host snapshot: topology generation alone does not establish
whole-host render/PCI/partition observations, process XNACK/apertures, successful
sysfs access, or every live device field. Such a profile needs reviewed driver
lifetime/invalidation contracts, wrap/ABA handling, exact session ownership and
proofs connecting guard validity to publication/completion/reuse authority.
Existing operational-fence proofs consume contracted observations; they do not
derive full currentness from a generation counter. This is an open architecture
direction, not permission to replace the present checks.
