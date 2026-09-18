# Combined SDMA Public Probe: CPU Qualification

The public `kfd-compute-aql-queue` example accepts
`--retained-release-combined-sdma <striped-count> <unique-id>` for exactly
2, 4, 6, 8, 10, 12 or 14 striped queues alongside two directional queues.
The existing primary, generic, single-engine and standalone striped modes
are unchanged. Combined mode requires one explicit device, not `--all`.

The packetless path uses the public combined constructor, selector, preflight
and retained release APIs. It checks distinct non-primary queue IDs across
both sets, directional H2D/D2H engine placement 1/0, alternating striped
engines, 4096-byte rings, 63 in-flight slots, capacity 2 engines/8 queues per
engine/14 maximum striped queues, and empty pool state. For striped count N,
it checks exact host growth of 4096*(N+2) bytes and N+2 records, unchanged
device accounting, 11+3*N returned resources, configured-account refunds and
an inert rejected retry. The success marker follows completed custody Drop.

Queue IDs need not be contiguous. The directional engine marker is a literal
whose values have already been asserted against the observations; queue IDs,
striped placement and capacity fields are rendered from the observations.
The striped cursor marker is explicitly source-qualified initial state,
not an independent public cursor observation. No packets or MMIO stores are
submitted by this path.

## Qualified Inputs

- Source base: `442aa6c839b909d0af1911746d8a7e9a290859e3`.
- Identical before/after 5,557-file source inventories:
  `9f5447bdbffb3277ff3750f2747a5300d7d69b05065bee149c297a5f4abe08cb`.
- Non-test static musl ELF:
  `08e0ed94feac79cb8126e536a3926994ebda3c4eff43e3edc3276faf11a1586d`.
- Fifteen named example tests passed on each of GNU and musl, with zero failed,
  ignored or filtered tests. These exercise CLI parsing and pure observation,
  resource and host-accounting oracles, not the native child/release path.
- Strict all-feature/all-target KFD Clippy, the non-test musl build, workspace
  formatting, diff checks and unchanged-source comparison passed.
- Three separately recorded post-qualification verifier calibrations passed:
  both real harnesses, source/command binding, and twelve malformed harnesses.

Only the example differs from the final source inventory in the sealed
[combined retained-release CPU packet](../dev-combined-sdma-release-cpu-2026-09-18/README.md).
Its production runtime and constructed fault tests are byte-identical inputs
here; they were not rerun in this example-only campaign. The containing signed
commit, not the source-base field, supplies the final Git binding.

## Audit And Scope

`python3 -B verify.py` performs a read-only historical audit of exact commands,
complete harnesses, source maps, the prior packet's authenticated source map,
recorded binary hash and exact archive closure. `--live` additionally rehashes
the current source and executable; `--seal` creates the manifest once. The ELF
is not committed, so historical verification cannot rehash its bytes. Pinned
helper files and the prior sealed packet are required from the repository.

This packet contains no hardware admission or runtime execution. Native
qualification requires a newly bound executable payload, exact topology and
fresh idle admission before every invocation. Other host work remains active;
an idle observation is not an exclusive reservation.

These CPU tests do not establish native combined teardown, submitted work,
nonzero native cursors, native fault recovery, physical-residency disposal,
formal implementation correspondence, or HIP/HSA performance parity.
R126, A1/A2 and #182 remain open.
