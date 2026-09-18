# Striped SDMA Public Probe: CPU Qualification

The public `kfd-compute-aql-queue` example now accepts
`--retained-release-striped-sdma <count> <unique-id>` for exactly 2, 4, 6, 8,
10, 12, 14 or 16 queues. It rejects `--all` for SDMA modes and preserves the
existing generic and single-engine probe modes.

The packetless path uses public standalone striped creation, preflight and
retained release APIs. It checks distinct non-primary queue IDs, alternating
engine indices, 4096-byte rings, 63 in-flight slots, unchanged empty device
accounting, exact host growth of 4096 bytes and one retained record per owner,
an empty pool, `5 + 3*N` returned resources, zero final configured accounts and
an inert rejected retry. Success is printed only after completed custody Drop.
Queue IDs need not be contiguous. No copy/compute packets or MMIO stores are
submitted. `cursor=initial-0-no-advance` follows from creation and absence of
publication; it is not an independently observed public cursor value.

## Qualified Inputs

- Source base: `3cc315d2a4e15b1fc74eb9ee10496ba683bdbb34`.
- Identical before/after 5,556-file inventories:
  `d945cf609b04469bd7880672c035bdebc0d2bf8d0e1ac7ce9765d5412bf3de94`.
- Non-test static musl ELF:
  `d9e2e6d6e9e8557f9e120709c5709bc1a7fd1d38376e6d78e27e2d8e32da00a4`.
- Eleven named example tests passed on each of GNU and musl, with zero ignored
  or filtered tests. Strict all-feature/all-target KFD Clippy, the non-test
  musl build, workspace formatting and diff checks passed.
- Three verifier calibration tests check both real harnesses, source/command
  binding and rejection of twelve malformed harnesses.

Only the example differs from the final source inventory in the sealed
[striped retained-release CPU packet](../dev-striped-sdma-release-cpu-2026-09-18/README.md).
That packet's production implementation and constructed fault tests are
byte-identical inputs here; they were not rerun in this example-only campaign.
The source base is not the containing commit: the latter includes this change
and evidence. The containing signed commit supplies the final Git binding.

## Hardware Status

This packet includes a bounded, read-only SMI inventory from MI300X at
2026-09-18 15:32:26-15:32:29 UTC. GPUs 1-7 reported zero utilization and about
285 MiB VRAM usage at that instant. This inventory is not the full native
admission observer, a reservation, or an exclusive-use guarantee. No runtime
probe was executed, and no remote directory or executable was created by this
CPU qualification campaign. No shared work was stopped or removed.

Native execution still requires a newly frozen signed-source/binary payload,
exact GPU/UID/BDF/topology/NUMA binding, and fresh complete admission before
each of eight isolated counts. The prospective gates remain zero GPU and
memory busy, VRAM below 512 MiB, no reported selected-device attachment,
launch within one second, bounded execution, strict immediate and delayed
postflights, and collection before exact-owned cleanup. No stale inventory
authorizes a later run.

## Audit And Scope

`python3 -B verify.py` is a read-only historical audit of exact commands,
complete harnesses, source maps, the prior packet's source identity, the
recorded executable hash and exact sealed archive membership. `--live` also
rehashes current complete source inputs and the executable; `--seal` creates
the manifest exactly once. The ELF itself is not committed, so historical
verification without `--live` cannot rehash its bytes. The verifier's pinned
helpers and prior archive are also required from this repository.

This establishes CLI and oracle correctness on CPUs, not successful native
striped teardown, native faults, nonzero native cursor coverage, submitted
work, physical-residency disposal, formal implementation correspondence or
HIP/HSA performance parity. R126, A1/A2 and #182 remain open.
