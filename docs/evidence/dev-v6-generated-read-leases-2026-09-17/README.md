# V6 Generated Input Qualification

Frozen-source CPU/test qualification above
`ace978213dd4e5e1ff10cb00d875bf24581aae83`, implementing the
[generated input-lifetime contract](../../runtime-context-generated-read-leases-v1.md).
This is development evidence, not V6, A1/A2, native/formal or HIP/HSA acceptance.

## Results

GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 1057 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 788 | 2 |

Each target has 2,116 passes and 23 ignored, with zero failed, measured or
filtered tests. Complete matching rosters contain 2,139 entries. Every entry
from the final ordinary kernel-reader baseline is preserved, with exactly
14 new passing generated-reader tests.

Strict all-feature/all-target Clippy, formatting, no-default checks, unsafe-source
policy (5 passed, 1 ignored) and 87 doctests pass. All sixteen serial command
records close with exit zero; source hashes and all six unit-test executable
hashes match at completion. Parser calibration accepts both baseline logs and
rejects ten malformed-log mutations. Musl uses `FE2O3_HIP_SYS_DISABLE=1`;
unrestricted HIP linkage is not tested.

The manifest covers 12 changed source/doc files: 9 Rust and 3 docs. SHA-256:

- Source manifest: `6abf6f4fc6d0c97205f65c95e261e18496deae118e67484a73e9df39bf86e68e`.
- Base-relative patch: `1004b9725de84ff38a769ecbc6d5481564a1c561b99f8fd0823367f3dc2d09ee`.
- GNU/musl roster: `3c91d6e9e615ac894c970e9e9d373d9415fc7ff66accdbd12e8b63e606c30e50`.

## Coverage And Review

The added tests cover exact domain/source/extent binding; independent reader
capacity and partial headroom; busy inputs; pre-handle ordinary-release rejection;
generic observers; no-handle error/panic custody without writers; neighboring
completion/Stop; marker/root/reference/domain corruption; foreign readers; stale
receipt/live-batch substitution; post-release disposal panic; default and graph
profiles; and early NoEffect rejection.

Before source freeze, the first focused run passed 22 tests and failed one legacy
fixture that tried to install a writer on an already leased input. The corrected
fixture installs a genuine extra model reader instead and checks rejection before
disposal. The subsequent 36-test run passed. Independent review then identified
a private-boundary guard-order defect: NoEffect was rejected only after reader
release. Production callers used only Success/Unknown, but the helper now rejects
NoEffect before any mutation. A new all-read-only/mixed regression and stronger
exact diagnostic/credit-vector assertions pass in the final 37-test focused run.
Strict Clippy and separate production, test-oracle and pre-freeze artifact reviews
passed before this complete archived campaign. Exploratory runs are not counted
as full qualification campaigns.

## Scope

The tests exercise actual Context shell identities and model leases with an
explicitly assumed native completion/disposal premise. They are not native
execution receipts. Foreign-reader and batch-substitution cases are private model
injections, not public races or native faults. Native readback/currentness/retirement
ordering is source-reviewed, not executed by these fixtures. Exact input release
precedes writer settlement; post-release failures may retain other ownership but
cannot be described as retaining the already released read leases.

No protected GPU campaign or matched HIP/HSA benchmark ran. Read-only MI300X checks
found active workloads; no remote files, jobs or deletions were created. A separate
local module-import feasibility check reran the unchanged issuance proof's 69
obligations successfully. It adds no reader theorem, authenticated integration
receipt or Rust/native correspondence and is not part of this CPU campaign.

Production Worker V3/machine refinement, initialized-input authority, ordered
writers, cross-run reuse, aggregate memory/residency, reader proof composition,
native concurrency/fault qualification and matched performance remain open.
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3; A1/A2 and #182 are not closed.
