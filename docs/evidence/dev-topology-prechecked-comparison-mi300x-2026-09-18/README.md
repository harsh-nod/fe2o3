# Prechecked Topology Reads: MI300X Comparison

Status: executed on MI300X GPUs 1 and 2. Both read-only provider probes, all
eight copy processes and 48 endpoint observations passed. Offline replay,
independent artifact review and all 23 postcollection verifier tests passed.
This is native provider compatibility and a source-to-source host-timing result,
not HIP/HSA parity, formal refinement or general performance acceptance.
Tooling was signed and published to both repositories before launch at
`36420a33bcc8640ea89abe3ecdf5c815e73da541`.

This compares signed baseline `fa9413425531458f9ac6b1ceb41d96912a9ae2d4`
with signed candidate `f43c352602296fa548faaff845477c1526f10aae`.
The baseline's selected source equals the sealed pair-currentness CPU cohort.
The candidate is bound to its own sealed GNU/musl qualification. The exact
source delta is changed `topology.rs` plus added
`topology/tests/prechecked_reads.rs`; there are no other selected input changes.

## Observed Results

Host: Linux `6.8.0-124-generic`, ROCm `7.2.4`, Rust toolchain
`nightly-2026-04-03`. Both compiler reports matched both CPU qualifications.
GPUs: `0000:26:00.0` / `0xab83d2ffef0d3cdf` and
`0000:46:00.0` / `0xd2e26fef80cf5c33`.

The two candidate topology probes returned identical 2300-byte summaries with
empty stderr: generation 9, ten nodes, eight gfx942 SPX/NPS1 GPUs, and every
declared physical UID/BDF pair. This qualifies the changed reader against the
real sysfs/proc providers on this host, not native failure-injection behavior.

Per-process p50 host latency in milliseconds, in execution order:

| Process | Hot forward | Hot reverse | Remap forward | Remap reverse |
|---|---:|---:|---:|---:|
| baseline-on1 | 30.083 | 30.088 | 118.801 | 118.750 |
| candidate-on1 | 28.272 | 28.271 | 111.650 | 111.685 |
| candidate-on2 | 28.262 | 28.284 | 111.017 | 110.988 |
| baseline-on2 | 29.959 | 29.966 | 118.564 | 118.585 |
| candidate-off1 | 28.322 | 28.333 | 111.844 | 111.668 |
| baseline-off1 | 29.918 | 29.916 | 118.451 | 118.429 |
| baseline-off2 | 30.040 | 30.057 | 118.553 | 118.698 |
| candidate-off2 | 28.393 | 28.387 | 112.198 | 112.127 |

With diagnostics off, persistent-hot p50 is 29.916482-30.056902 ms for baseline
and 28.321609-28.392846 ms for candidate. Candidate/baseline ratios by process
replicate index and direction are 0.944432-0.947084, or 5.292-5.557% lower.
Remap p50 is 118.428841-118.698114 ms versus 111.667886-112.197658 ms,
with ratios 0.942911-0.946396, or 5.360-5.709% lower. Raw nanoseconds, p95s
and every process result remain in `remote/parsed.json`; `verify.py --summary`
independently reconstructs the ratios. These are descriptive comparisons on a
shared host, not confidence intervals or causal or HIP/HSA speedup claims.

Across the four candidate diagnostic-on hot-sample process/direction buckets,
currentness accounts for 99.977086-99.978054% of summed measured lower-call host
intervals. This is `sum(currentness_total_ns) / sum(all_calls_total_ns)` within
each bucket, not a ratio of independently computed percentiles or GPU latency.
The aggregate currentness stages remain the dominant measured cost; this packet
does not isolate topology-only time. The change removes a redundant path
inspection without amortizing full discovery across calls.

ELF SHA-256 identities:

- Baseline: `38541bfdbe18443a6ddac34c38aad82310d9a0f89dd57bd8686f3f2794917abf`.
- Candidate: `62986512b8995510f93658f8e5881fd8e25931bdaff9c4a0807759b02b8e592b`.
- Topology probe: `a21d51fa62ef197b53e46b5f8be5e79fc5c9fee51257ebd7ddfddafdece31202`.

All 23 local and 65 remote recorded commands exited successfully with their
process groups absent. All 201 remote result files were collected and matched
the remote inventory before cleanup. The controller recorded remote path and
process absence and local payload removal, with no primary or secondary failure.
Fresh external checks also confirmed the owned remote path and matching
processes absent and the local payload absent, including no remaining symlink.

## Protocol

Both revisions are rebuilt from normalized archives of their signed Git blobs,
with separate fresh targets, the same frozen lockfile, release flags and reported
compiler. The candidate also builds the existing `kfd-topology` example.
Two bounded read-only executions must independently parse and agree byte for
byte before any copy workload. The parser requires the complete declared
eight-card UID/BDF roster, ten-node topology and MI300X SPX/NPS1 profile.
All three binary identities and both source trees are checked around probes
and before workloads, with a final identity observation afterward.

The probe exercises real sysfs/proc discovery but grants no device, mapping or
queue authority. Equal printed summaries do not establish equality of unprinted
link properties or the SDMA sidecar, continuous currentness, or atomic snapshots.

- One freshly admitted idle MI300X pair; no exclusive reservation is assumed.
- 1 MiB, depth 1, 10 warmups and 30 measured copies per direction and mode.
- Diagnostics on: baseline, candidate, candidate, baseline.
- Diagnostics off: candidate, baseline, baseline, candidate.
- Strict endpoint checks before each workload and at least two seconds and
  twenty further seconds after process closure.
- 23 local commands, 65 remote commands, two provider probes, eight workloads
  and 48 endpoint observations, with exact receipt and archive membership.

The process is the unit of replication: two per cohort/diagnostic mode. Timing
ratios are descriptive host intervals, not GPU timestamps, confidence intervals,
causal attribution, syscall counts or a matched HIP/HSA result. Prior campaign
timings do not measure this candidate and are not used as its baseline.

## Safety And Replay

Only exact-marker-owned paths and processes may be cleaned. Every native attempt
requires complete result collection and digest equality before remote cleanup;
failed collection retains the owned path and local payload for recovery. No other
user's files, processes, GPU state or work are removed or reset. Builds use two
jobs and require at least 8 GiB initially free on the shared host.

The launch was `python3 -I -B campaign.py run 1 2`, after both repositories
published the signed tooling. Recorded prelaunch suites passed 37 campaign,
21 prior parser and 19 observer tests. Postcollection calibration was
`python3 -I -B test_verify.py`: all 23 tests passed in 456.298 seconds.
Its execution transcript and the independent review and fresh absence checks
are external to this packet. The mutation suite includes changing both provider
probes and cached summary to an impossible zero generation while refreshing
receipt hashes and collection inventory; independent semantic replay rejects it.

Before sealing, replay with `python3 -I -B verify.py --allow-unsealed --summary`.
Seal only a successfully verified run with `python3 -I -B verify.py --seal`;
subsequent replay is `python3 -I -B verify.py`. Offline replay requires pinned
Git objects, CPU packets and signing trust. Historical cleanup receipts are not
fresh host observations. This packet cannot establish general performance
acceptance, HIP/HSA parity, native failure-injection coverage or formal refinement.
