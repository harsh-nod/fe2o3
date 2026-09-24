# Matched Hot-Batch MI300X Campaign

The original campaign is **rejected**: the controlling SSH session timed out,
and its immediate inventory attempt failed with temporary name resolution.
The remote controller nevertheless completed the entire declared workload.
All durable native records were subsequently recovered and independently
replayed. Recovery does not change the original controller failure into a
clean campaign pass, and no performance acceptance or speedup is claimed.

The unchanged [prospective protocol](PROTOCOL.md) fixes depth 1/16/32, signed
source, trial order, admission, timing interpretation and cleanup. It remains
the gate for a fresh strict campaign.

## Recovered Evidence

On GPUs 5/6, the retained remote records cover all eighteen declared processes,
both directions and complete source/destination payload and guard checks.
All 134 remote commands have successful reaped receipts, all 108 endpoint
observations pass their strict parser, and all eighteen result records match
the maintained parser and independently supplied controls. The native
controller's final record has no failures. All three executed ELFs are
retained and their opening/closing hashes match.

The local controller records preserve both exit-255 failures and their exact
stderr: SSH server timeout, followed by temporary hostname-resolution failure.
No cause is attributed to the GPU, driver, runtime or network infrastructure
beyond those observed errors. The original `collection.json` remains failed,
with remote cleanup false. It is not overwritten by recovery.

Once connectivity returned, `recovery1` first established that no process was
still using the exact owned remote directory. Its inventory, byte-exact
411-file collection, exact-owned removal and separate path/process absence
commands all passed. No GPU workload was restarted. The recovery record
explicitly keeps `campaign_accepted` and `performance_acceptance` false.

The recovered replay verifies signed source/CPU/protocol identities, source
archives, tool continuity, exact commands/environments/deadlines, source and
ELF brackets, all endpoint/result records, collection and cleanup. Endpoint
UTC timestamps, including nested captures, must fit their actual command
receipt; internally consistent stale observations are rejected. Structured
controls and outcomes use type-sensitive JSON comparisons. The incomplete
local SSH stdout must be an exact prefix of the complete remote phase roster.

`audit1` captures the first successful recovered-record replay, 32 checker
tests and nine collection/cleanup tests. The checker tests include the valid
recovered packet, continued rejection by the original strict verifier, and
thirty hostile-record controls. `audit2` repeats these checks with the final
README in its input bracket. These are record checks, not a hardware rerun.

## Retention And Cleanup

The packet retains 492 campaign/recovery files plus the local cleanup receipt.
Collection compared complete inventories and every retained byte before local
removal. The fixed owned source checkout, Cargo target and build-source tar
were removed, reclaiming 424,591,360 allocated bytes. Logs, source manifests,
the small comparator archive and all native executables remain. The transient
build archive's digest is retained; source reconstruction uses signed Git
objects rather than an archived copy of that large tar stream.

The recovery wrapper did not capture its own opening/closing source bracket.
Its four operative commands are independently replayed, including the pinned
ownership-control stdin hashes, exact original marker, outputs and order.
The final audit brackets the recovery and packaging scripts along with the
checker, tests, manifest and README. The toolchain and shared host are not
hermetic, and idle endpoint observations are not an exclusive reservation.

## Preparation

The source benchmark changes and their complete CPU qualification are signed
at `8dc128357ecd55e1ba4f2866eb075899481f9aa0`, published on both topic remotes.
The controller requires the exact signed CPU evidence packets, source maps,
retained parser and a fresh cold-build target. No production source change is
introduced by this protocol.

`protocol3` passes 31 tests across three bounded commands: fifteen prospective
campaign tests, nine existing authenticated bootstrap/cleanup controls and
seven maintained result-parser tests. All process groups are absent and the
five-file protocol input bracket is unchanged. The exact protocol scripts are
also retained with that run. The earlier protocol1/protocol2 records are
superseded development controls, not native acceptance; their input hashes are
retained but their intermediate script bytes were not independently frozen.

## Replay And Next Step

Authenticate this packet's signed Git commit before executing its scripts.
In the original checkout, with the pinned signer list and owned cleanup paths:

```sh
python3 -I -B docs/evidence/dev-xgmi-hot-batch-mi300x-2026-09-24/verify_recovery.py
```

The original `verify.py` intentionally fails with `strict campaign did not
qualify`. Do not replace that failure with the recovered-record result. Raw
timings are retained for audit but are excluded from accepted performance
comparisons. A fresh strict campaign is still required. These observations do
not establish engine concurrency, native fault recovery, aggregate memory
bounds or formal implementation correspondence. A1/A2, A7 and general HIP/HSA
parity remain open.
