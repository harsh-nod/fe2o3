# Current-Source Native Runtime Matrix

Source is signed `3540325123fa30b5208d57ba950922a707eb07b8` without runtime
changes. A cold private musl all-feature library test build must pass the exact
1,365-test CPU roster, retain twenty hardware ignores and match all 3,956 prior
CPU-qualified source inputs. The actual executed ELF is retained.

GPU 1 (`0000:26:00.0`, UID `0xab83d2ffef0d3cdf`) is eligible only after fresh
strict admission before each invocation. No exclusive reservation is claimed.
CPU/NUMA placement is frozen at CPUs 0-47/node 0 and checked against fresh PCI,
KFD and process-allowed topology. Global available and local-node free memory
must each provide at least four GiB, with one GiB free remote disk before each case.

The ordered roster in `protocol.py` names all twenty ignored tests. The
auxiliary-allocation failure runs prefixes 0, 1 and 2 separately, yielding
22 top-level commands and 24 expected libtest frames (two parent/child pairs).
Every invocation selects one exact test, with core dumps disabled, 16-MiB
per-file output bounds, a 180-second TERM timeout and five-second KILL grace.
Recorder bounds are 200 seconds for tests and 100 seconds for observers.
The remote controller has a 12,000-second TERM bound and fifteen-second KILL
grace; its SSH observation has a 12,120-second bound.

Before launch, each observer requires exact GPU identity, three idle GPU/memory
sysfs samples, less than 512 MiB VRAM and no selected-GPU PID attachment. The
memory-busy check is an additional conservative parser condition beyond the
observer's own GPU-busy predicate, matching recent XGMI campaigns. Launch
must occur within one second of the preflight's recorded completion. Every
spawned process group is recorded before managed signals are unblocked.

T0 is recorded only after parent reap and owned process-group absence. Exactly
one immediate observer starts at T0+[0,1] seconds; exactly one delayed observer
starts at T0+[20,21] seconds. Both are attempted after a launched/closed test,
including test failure or immediate refusal. A delayed pass cannot rehabilitate
an immediate failure. Any refusal stops later cases; there is no poll-to-pass.

Case-specific parsers require exact names, frame counts, successful summaries,
markers and complete profiler identities/readback extents where emitted. Six
generated tests intentionally emit their one exact marker to stderr. Native
assertions remain bound to the signed test source; marker parsing does not
independently prove every internal assertion or the production Worker path.

The terminal-retention cases end through isolated process exit. Their injected
runtime-envelope failures are not actual KFD ioctl or device faults. Pending
native custody does not prove physical compute/allocation overlap. No formal
production refinement, aggregate memory bound or performance claim follows.

All remote results and payload/ELF copies are inventoried and collected
byte-exactly before marker-bound directory removal and independent absence
checks. If collection fails, retain the remote directory for recovery; SSH
failure alone never proves the remote controller has stopped. No foreign
processes, paths, GPU resets or global cleanup operations are permitted.
