# Compiler Proof Broker Component Qualification

This change implements the verifier-owned compiler proof transport needed to run
generated proofs outside Cargo's inherited exec-notification filter. It is a
component qualification, not successful protected compilation, ordinary application
admission, two-GPU execution, formal verification of the transport, or a HIP/HSA
performance result. Base revision: `97b52adf9fc77157589414bfa07eabefc552a13f`.

## Implemented Boundary

- Original pidfds, process generation and exact sealed executable identity for
  the broker, wrapper and compiler; same-PID exec and process death reject.
- A distinct credential-authenticated seqpacket endpoint, session, sequence,
  fresh challenge, invocation/runtime/source binding and exact descriptor roster.
- One immutable read-only source object, at most 2 MiB; at most 16 KiB of observed
  stdout and stderr each; an absolute monotonic deadline capped at 600 seconds.
- A consuming spawn that drops Command-held endpoint aliases before delegation
  and retains the original spawned child handle. Delegation cannot substitute a
  caller-provided child pidfd. Capture errors retain the Child with the caller for
  the same explicit kill/reap path as other delegation errors.
- Sticky failure state, socket shutdown on failure/unwind, deadline-aware locks,
  cancellation checks during initial exec waiting and active proof supervision,
  and existing process-tree kill/reap cleanup.
- A private Local/Brokered runtime variant. Safe local opening remains local;
  only the unsafe compiler-private original-delegation admission can install the
  concrete broker client. No caller-supplied proof-output importer or fallback.
- Fixed child descriptors 224/225; private duplication starts at 226. A subprocess
  checks the conservative eleven-descriptor roster below authority slots 240..242
  under the protected launcher's minimum `RLIMIT_NOFILE=243`.

The server accepts only the existing Ranked generated-proof execution profile.
The closed-fill application proof controller remains separate. Existing source
generation, successful-output checks, receipt construction and seccomp policy
are not weakened.

## Validation

The evidence archive contains the exact source patch and command logs:

- Verifier library: 186 passed, 11 ignored. The eleven include ten existing
  deployment/subprocess cases plus the new low-file-limit helper, which is invoked
  successfully by its ordinary parent test. There are 23 new ordinary tests.
- Two compile-fail documentation tests reject repeated spawn and cloning custody.
- Cargo broker regressions: 12 passed with the changed descriptor normalization floor.
- Production library/binary Clippy uses `-D warnings` without lint suppressions.
- Scoped rustfmt and `git diff --check` pass.

An exploratory all-tests strict Clippy run also exposed pre-existing
`duplicate_mod` fixture reuse and constant-assertion warnings. These unrelated
test sources were not rewritten. Production Clippy is the strict recorded gate;
the library and Cargo tests are executed independently.

## Remaining Work

Wire the protected Cargo release admission and original exec-permit owner to one
proof preparation; preserve the existing initial four-FD response and source/ISA
observer lifecycle. Keep the exact attempt binding. Move the brokered runtime
through original protected compiler custody and both proof joins. Then rerun the
genuine installed application campaign, followed by host binding projection,
ordinary application admission, two-GPU fill and bidirectional PUBLIC XGMI.

No genuine brokered Verus execution or GPU benchmark was run for this component
change. WSL SSH timed out at `sharkmi300x-1:22`, but Windows OpenSSH successfully
queried the same host. Only read-only hostname/device-occupancy checks were issued;
no remote file, GPU allocation, build or cleanup action was performed.
