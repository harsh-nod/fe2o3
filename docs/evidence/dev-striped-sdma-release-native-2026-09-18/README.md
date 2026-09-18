# Striped SDMA Retained Teardown: MI300X

The public `kfd-compute-aql-queue --retained-release-striped-sdma` probe passed
for all eight admitted counts on MI300X GPU 1. Each isolated process created
the original primary and standalone striped owners, preflighted retained
release, checked original identities and alternating engine placement, released
the configured backing accounts, rejected an inert retry and dropped completed
public custody before reporting success. No packets, copies or MMIO stores
were submitted.

| Queues | Returned Resources | Host Growth Bytes | Host Growth Records |
| --- | --- | --- | --- |
| 2 | 11 | 8192 | 2 |
| 4 | 17 | 16384 | 4 |
| 6 | 23 | 24576 | 6 |
| 8 | 29 | 32768 | 8 |
| 10 | 35 | 40960 | 10 |
| 12 | 41 | 49152 | 12 |
| 14 | 47 | 57344 | 14 |
| 16 | 53 | 65536 | 16 |

The cursor marker is source-qualified creation state without publication, not
an independently observed public cursor. Nonzero cursor and injected fault
coverage remain constructed CPU tests, not native results here.

## Bound Inputs

- Signed source: `602fda830307f9818cbff5d57d4be68a897e75b7`.
- Static musl ELF:
  `d9e2e6d6e9e8557f9e120709c5709bc1a7fd1d38376e6d78e27e2d8e32da00a4`.
- [CPU qualification](../dev-striped-sdma-example-cpu-2026-09-18/README.md)
  seal: `8fa5941a2cad5ff2d52fe3b040204c66db0757607e5a45fa9898cd144adae87d`.
- Equal 5,556-file source inventories:
  `d945cf609b04469bd7880672c035bdebc0d2bf8d0e1ac7ce9765d5412bf3de94`.
- Frozen 23-file GPU 1 payload:
  `503683e379673cebf6711e99f82dcccf9a39f1c4c8d3b787c6f940a375c164c2`.

The exporter checked the signed containing commit, all inventoried source Git
blobs, the sealed CPU packet, the non-test executable and a retained reviewed
public signer file. The same executable ran in all eight cases; no remote build
occurred. Eleven example tests passed per GNU/musl target. Production retained
release code is unchanged from the prior
[constructed CPU qualification](../dev-striped-sdma-release-cpu-2026-09-18/README.md).

## Admission And Cleanup

The successful campaign ran on 2026-09-18 from 15:47:42 to 15:51:34 UTC.
GPU 1 was bound to UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, CPUs 0-47 and
NUMA node 0. It was observed available, never reserved. Every case launched
within one second of a fresh complete admission: zero GPU and memory busy,
VRAM below 512 MiB and no reported selected-device PID. All 24 endpoints passed.
Immediate postflights started about T0+0.032-0.033 seconds and delayed ones
T0+20.031-20.034 seconds, within fixed windows after parent/group closure.

Each case retained its 180-second TERM/five-second KILL bound, zero-core and
16-MiB output limits. All 130 remote files were collected and hash-checked
before removing `/tmp/fe2o3-striped-sdma-20260918.ee30bc89`. Independent checks
confirmed path absence and all 35 recorded process groups absent. Accessible
same-user exe/cwd/fd/maps references were absent. Unreadable unrelated entries
remain explicit visibility limitations, not an all-user absence claim.

## Preserved Rejections

The earlier GPU 4 campaign is retained under `rejected/`. Its first preflight
found another attached process, PID 4127510, GPU busy readings of 92-97%, and
36,341,137,408 VRAM bytes. It stopped before launching the runtime binary; no
count was tested there. All 37 remote files were collected, the exact private
directory was removed and its four recorded process groups were independently
absent. The checker replays the archived captures through the pinned observer
without device access. No admission gate was relaxed, no failed native result
was promoted, and no other user's work was stopped. The separate GPU 1 campaign
has its own frozen topology and payload.

An additional setup directory had an underscore suffix rejected by the local
creator's alphanumeric path guard. Its marker-only creation and exact cleanup
receipts are retained in `raw/rejected-create/`; the failed wrapper traceback
was not archived. The audit therefore claims only an invalid marker name and
verified marker-only cleanup, not a replayed wrapper-failure receipt. No payload
or runtime executable was uploaded there. The creator now uses an eight-digit
hex suffix and exclusive private-directory creation. All three remote paths
created in this work were removed; no shared directory was cleaned.

## Audit Scope

`python3 -B verify.py` is a read-only historical audit. It verifies payload and
CPU/source bindings, complete native and outer transcripts, timing, raw
observations, collection, cleanup and the preserved rejection. The 18 archive
calibration tests and four frozen protocol plus four controller tests exercise
fail-closed checks. `--allow-unsealed` is for prepublication review;
`--source-root PATH` and `--binary PATH` additionally rehash supplied inputs.
The two identical ELF copies are omitted from Git; their original hashes stay
bound to collected inventories. Without `--binary`, historical verification
cannot rehash executable bytes. Archived signature output is not a fresh
cryptographic signature verification. Do not rerun historical control scripts.

This is bounded public success-path creation/teardown qualification, not
submitted compute/copy behavior, native fault recovery, aggregate physical
memory disposal, multi-device concurrency, formal machine-code correspondence
or HIP/HSA performance parity. KFD engine indices are not HSA engine masks.
This does not establish completion of R126, A1/A2 or #182.
