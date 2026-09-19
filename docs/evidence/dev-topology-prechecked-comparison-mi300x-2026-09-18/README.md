# Prechecked Topology Reads: MI300X Comparison

Status: prepared; no native compatibility or timing result claimed yet.

This compares signed baseline `fa9413425531458f9ac6b1ceb41d96912a9ae2d4`
with signed candidate `f43c352602296fa548faaff845477c1526f10aae`.
The baseline's selected source equals the sealed pair-currentness CPU cohort.
The candidate is bound to its own sealed GNU/musl qualification. The exact
source delta is changed `topology.rs` plus added
`topology/tests/prechecked_reads.rs`; there are no other selected input changes.

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

Publish signed tooling to both repositories before `python3 -I -B campaign.py
run <gpu-a> <gpu-b>`. Prelaunch calibration is `python3 -I -B test_campaign.py`;
the controller also runs the pinned prior parser and observer suites.
After collection, run `python3 -I -B verify.py --allow-unsealed --summary` and
`python3 -I -B test_verify.py`; seal only a successfully verified run with
`python3 -I -B verify.py --seal`. Offline replay requires pinned Git objects,
CPU packets and signing trust. Historical cleanup receipts are not fresh host
observations. This packet cannot establish general performance acceptance,
HIP/HSA parity, native failure-injection coverage or formal refinement.
