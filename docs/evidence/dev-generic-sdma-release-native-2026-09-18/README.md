# Single SDMA Retained Teardown: MI300X

The public `kfd-compute-aql-queue --retained-release-sdma` probe passed for
`generic`, engine `0` and engine `1` on MI300X GPU 4. Each separate process
created the original primary and SDMA owners, selected the retained release
path, returned eight resources, refunded the configured backing accounts,
rejected an inert retry and dropped completed public custody before reporting
success. There were no submitted packets, copy transfers or MMIO stores.

## Bound Inputs

- Signed source: `d85b4e991f5df833d5c2895d45098aaa0d8fdc54`.
- Static musl example SHA-256:
  `596aaed466822f7bf75e7b63f152d2c3c79c50bafb4a7e8ebe3e7ff297583330`.
- CPU archive: [accounting correction](../dev-generic-sdma-example-accounting-cpu-2026-09-18/README.md),
  manifest `3190a687c50d70b8b6ae475bcbe1c8acb497ccd27f355899d8873b4c979aca50`.
- Equal 5,555-file source inventories:
  `6618d78464550772733fa643ef42b37fc0d82e8bfb84c2a6499a82c13e216761`.
- Frozen 20-file payload:
  `c18a03390ba24e6c0e9a60f9ba44159a5e00b51a52310a3dd0d424a92e4b0954`.

The exporter verified the containing commit signature, compared all inventoried
source bytes against their Git blobs, and rehashed the CPU-built non-test ELF.
The same uploaded executable was used for all three profiles without a remote
build. Six example tests and one real-ledger test function passed per GNU/musl
target; the latter covers all three profiles with and without dispatch state.
These are scoped CPU suites, not another full runtime/KFD regression campaign.

## Admission And Cleanup

The shared host was not reserved. GPU 4 was bound to UID
`0x54f88318ca05093d`, BDF `0000:85:00.0`, CPUs 48-95 and NUMA node 1.
Each invocation launched within one second of its fresh complete preflight:
zero utilization, VRAM below 512 MiB and no selected-device PID attachment.
The unchanged full observer also admitted all immediate and fixed delayed
postflights. Immediate starts were approximately T0+0.032 s and delayed starts
T0+20.031 s, within the prospectively fixed windows. T0 followed parent reap
and owned-group absence. There were no observation retries or relaxed gates.

Native commands retained their 180-second TERM/KILL limits, zero-core and
16-MiB output limits. All 67 remote files were collected and hash-checked before
removing `/tmp/fe2o3-single-sdma-20260918.0f3uxs8l`. A separate command confirmed
path absence and all 15 original PID/group identities absent. Accessible
same-user exe/cwd/fd/maps references were absent; unreadable entries are retained
as visibility limits. No foreign work or shared directory was removed.

## Preserved Rejection

`rejected` contains the first signed-source campaign (`a0ab05f3`). It failed
before SDMA creation because the new probe incorrectly expected no retained
host credits while a primary completion allocation was live. Retained credits
count healthy live backing, not only failures. The correction checks the live
retained/used relationship, the exact SDMA +4096-byte/+1-record delta, then
explicit zero accounting after release. A constructed test checks the real
ledger transitions, in addition to the small example-oracle unit test.

The first attempt remains rejected, with empty success output and its exact
assertion failure. Its three strict endpoints passed; 43 remote files were
collected and its directory removed, with seven recorded groups independently
absent. `rejected/export-rejected.md` also records an earlier local exporter
filename typo; that incomplete local export was never uploaded. No failed raw
receipt or sealed CPU packet was overwritten.

## Audit Scope

`python3 -B verify.py` checks the sealed archive, frozen payloads, CPU-map and
executable-hash bindings, exact native commands and complete transcripts,
observation timing, inventories and cleanup. It preserves the rejected
disposition. `--allow-unsealed` is for prepublication review. Optional
`--source-root PATH` and `--binary PATH` revalidate current inputs independently.
The fifteen archive calibration tests and frozen protocol/controller tests
exercise transcript, identity, timing, inventory and cleanup rejection.

The two ELF files are deliberately omitted from Git; their original hashes
remain bound by payload and remote inventory. Complete originals remain in the
local campaign bundles. Without `--binary`, the historical audit cannot rehash
omitted executable bytes. Archived signature output is not a fresh signature
verification. The checker never invokes SSH, Cargo or the GPU executable.

This establishes bounded public success-path creation/teardown, not submitted
copy/compute behavior, native fault recovery, formal machine-code correspondence,
global physical-memory disposal or HIP/HSA performance parity. KFD engine indices
are not HSA engine bit masks. R126, A1/A2 and #182 remain open; the accepted Native
CPU/test checkpoint remains R125. The backing-account refund excludes intrinsic
and other unaccounted allocations and is not a process-residency certificate.
