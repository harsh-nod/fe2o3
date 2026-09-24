# Fresh Link Scratch Comparison

This records the controls frozen in the captured `native.py` and binding before
native execution. It is a workload-scoped development experiment, not an A7
performance threshold or HIP/HSA parity gate.

## Source And Build

- Baseline: signed `a852464aee976a1ff1472f2e477e25b6cf2ac933`.
- Candidate: signed `1e13a90cd5e604a4612b079b48f629005a0a047b`.
- The selected source delta is exactly two topology implementation files and
  five test files. Signed Git trees and byte-exact source archives are retained.
- The candidate's CPU qualification is the packet published at signed
  `7fc77eed0c56cb94641c53e3d7365a2a170ccf43`; its inputs match the candidate.
- Build each KFD cohort in a separate, newly created Cargo target directory:
  release musl, default plus `hardware-diagnostic`, pinned nightly-2026-04-03,
  two build jobs and incremental compilation disabled. Match all other build
  settings, retain opening/closing tool identities, and run the benchmark's
  thirteen release tests for each cohort.
- Reject existing or aliased target directories. Reject identical cohort ELF
  hashes before any remote creation. Different hashes alone do not prove source
  provenance; the cold build, signed source and execution receipts supply the
  additional evidence.

## Native Workload

Use MI300X GPUs 5 and 6, with both PCI addresses and unique IDs bound in the
manifest. Use one MiB per copy, depth one, one prime round, ten warmup rounds and
thirty sampled rounds per direction. One process completes forward then reverse
copies per round. Mappings and buffers persist across timed rounds. Complete
source/destination pattern and guard validation occurs after the sequence, not
independently after every timed copy.

The same KFD ELF within each cohort runs diagnostic-on and diagnostic-off modes.
HIP/HSA use their native persistent-hot API paths with the selected physical
devices masked to ordinals zero and one, and XNACK disabled.

Process order:

1. Diagnostic: baseline, candidate, candidate, baseline.
2. Uninstrumented: candidate, HSA, HIP, baseline, baseline, HIP, HSA, candidate.

There are two separate process invocations per backend/mode cell, not sixty
independent process replicates. Diagnostic records are a separate population
and must not enter KFD/HIP/HSA latency ratios. Report each uninstrumented
process's own p50/p95; ranges of these summaries are not pooled percentiles.

KFD measures facade enqueue through aggregate close, including currentness
checks. HIP/HSA measure native enqueue through their explicit completion
observations. Matching bytes, depth and lifecycle does not make their admission
or engine semantics identical. This is host-observed latency, not a device-copy
engine bandwidth measurement or a physical-overlap proof.

## Shared Host And Cleanup

Before every trial, observe both endpoints with the existing strict admission
parser. Repeat observations after a two-second settling delay and again twenty
seconds after both settled observations finish. Stop the remaining suffix on
any failed observation or workload. These point observations do not establish
an exclusive reservation or continuous absence of interference.

Commands are bounded and recorded with exact argv, environment, stdout/stderr
hashes, timestamps, exit status and process-group absence. The campaign checks
source and ELF continuity, collects the complete remote inventory byte-for-byte,
and only then removes its exact owned directory. A separate observation must
report both owned path and owned processes absent. Uncollected evidence must not
be deleted.

## Evidence Limits

The retained KFD executables can be independently hashed. HIP/HSA executable
bytes are not collected; their build receipts and opening/closing recorded
hashes are attestations, not independently retained executable artifacts. This
is not a hermetic compiler, library, driver, firmware or machine snapshot.

The first campaign, `native1`, used a shared Cargo target and produced identical
KFD executables. It is rejected in full, including all timings. Its raw records
and cleanup remain historical evidence only. `native2` uses separate cold
targets and must pass the complete replay independently.

No record here establishes whole-runtime executable refinement, aggregate
process memory bounds, A1/A2 closure, production performance acceptance, or
general HIP/HSA parity.
