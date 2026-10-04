# Application Registration Prerequisites

Source: `e364a6dfd258b454982321cad47fb17cd37e3565`.

This checkpoint implements the inert application binding and original-process
transfer required by the [multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).
It does not install the four-input application profile or authenticated root
registration, deploy a proof custodian, or enable ordinary two-GPU execution.

## Changes

`WorkerV3ApplicationRegistrationBindingV1` is exactly 840 bytes: a 24-byte header,
184-byte compiler supervisor handoff, 304-byte four-slot occurrence, four descriptor
coordinates, 196-byte expectation, 84-byte challenge, and 32-byte domain-separated
identity. Decoding checks exact framing and checksum before bounded nested decoding,
then exact re-encoding. It requires slots 1-4, distinct coordinates above stderr,
FD195 exclusion, and exact expectation/occurrence consistency. The shared FD195
constant retains its existing client import path. Neither bytes nor identity
authenticate descriptors, process ownership, freshness, or launch authority.

The non-cloneable retained-child token captures original child and Cargo pidfds
directly after spawn. Its production API has no raw constructor, descriptor
extraction, or `AsFd`. The application compiler-channel path now transfers a
validated duplicate of that original child pidfd, without numeric PID reopening.
Current-parent credentials, CLOEXEC, original-child waitability through P_PIDFD,
service peer identity, liveness and the absolute deadline are rechecked. Ordinary
compiler admission and its exact two-right supervisor handoff are unchanged.

Cargo retains the original handles through ACK, failure and delayed reaping. A
single pre-spawn allocation holds the protocol and space for the captured token;
capture fills that same allocation without post-spawn boxing. Exited-but-unreaped
children may be captured for cleanup. Post-reap descriptor/history custody is not
live-transfer eligibility. The original parent handle is retained but not yet
transferred by the future four-right registration path.

## Qualification

| Check | Result |
| --- | --- |
| Runtime/compiler protocol and client tests, including doc tests | 147 passed |
| Cargo main binary | 401 passed, 5 existing deployment/hardware tests ignored |
| Rustc wrapper and linker proxy | 43 passed |
| Four changed packages, all-target strict Clippy | Passed with `--no-deps -- -D warnings` |
| Package formatting, whitespace and frozen source checks | Passed |
| Signed source commit, source/binary audit and archive manifest | Passed |

The total is 591 passed and five ignored; nested Cargo helper reruns are not counted
again. Six new codec tests cover all 840 single-byte mutations, hostile lengths,
independently resealed substitutions and cross-field mismatches. Seven retained
token tests cover no reopen, exact duplicate identity, wrong child, parent identity,
exit/reaping, deadline and CLOEXEC. Two new compile-fail tests reject Clone/AsFd.
The Cargo composition test rejects service transfer after fast exit while retaining
the exact original cleanup owner. Existing startup/reaper tests still pass, and a
pointer assertion checks that the pre-spawn allocation survives capture unchanged.

Read-only native-agent review found no remaining blocker after targeted hardening.
Development runs found a test integer mismatch, an obsolete import, and enlarged
error values; all were corrected before the final frozen run. One link run failed
because the shared local filesystem filled. Only stale, single-link test executables
from this owned worktree's cache were removed, with the exact list archived; sources,
libraries and other jobs were untouched. Final qualification then passed completely.

Runs used nightly-2026-04-03, locked offline dependencies, four build jobs, one test
thread, disabled HIP discovery/incremental/debug info, test optimization 1, and
enabled debug/overflow checks. All 6,110 captured source/configuration files were
stable and matched the signed commit; 20 test executable hashes are recorded.
These are CPU regression/adversarial tests, not new formal proofs, production
service qualification, GPU execution, HIP/HSA parity or performance measurements.
No MI300X resources were used. Owned scratch is removed after archive validation
and publication; unrelated user files remain untouched.

[Evidence archive](evidence.tar.gz) SHA-256:
`b375cb3a33b9cfd0615d8d579c30d72980d220873ccd0f61ef49e27952b60f2e`.
Source patch SHA-256:
`c962a5188f147b3a81ed5ced3a570a3d0648e37923f965660fc400a239558c47`.
