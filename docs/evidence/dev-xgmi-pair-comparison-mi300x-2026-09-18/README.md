# XGMI Pair-Currentness Source Comparison

Status: executed on MI300X GPUs 1 and 2; all eight processes and 48 endpoint
observations passed. Offline replay and independent artifact review passed.
This packet does not establish HIP/HSA parity, formal refinement, or performance
acceptance. Tooling was signed and published to both repositories before launch
at `2d58fb516ba966d1779847c3165453bbbca2698b`.

The baseline is signed commit `84b61ee39817cee8bfe3cd68312f0a05543bc9af`;
the candidate is signed commit `72eb6b3052b803a26a4007dca10fa2e26138eed1`.
Both selected compiler-input trees are independently associated with their sealed
CPU qualifications. Deterministic archives come from Git blob plumbing, not the
working tree or Git archive attributes. Both are rebuilt with the same frozen
lockfile, release flags and reported compiler, in separate fresh target directories.
The candidate consolidates full pair validation into one fresh discovery; this
campaign does not directly count discoveries or prove machine-code refinement.

## Observed Results

Host: Linux `6.8.0-124-generic`, ROCm `7.2.4`, Rust toolchain
`nightly-2026-04-03`. Both compiler reports matched both CPU qualifications.
GPUs: `0000:26:00.0` / `0xab83d2ffef0d3cdf` and
`0000:46:00.0` / `0xd2e26fef80cf5c33`.

Per-process p50 host latency in milliseconds, in execution order:

| Process | Hot forward | Hot reverse | Remap forward | Remap reverse |
|---|---:|---:|---:|---:|
| baseline-on1 | 118.323 | 118.231 | 205.735 | 205.829 |
| candidate-on1 | 30.095 | 30.092 | 118.555 | 118.498 |
| candidate-on2 | 30.082 | 30.069 | 118.825 | 118.779 |
| baseline-on2 | 117.632 | 117.690 | 207.096 | 207.014 |
| candidate-off1 | 29.871 | 29.855 | 118.095 | 118.092 |
| baseline-off1 | 117.677 | 117.670 | 207.589 | 207.491 |
| baseline-off2 | 117.538 | 117.576 | 205.782 | 205.949 |
| candidate-off2 | 29.847 | 29.877 | 118.027 | 118.024 |

With diagnostics off, baseline/candidate p50 ratios by process replicate index
and direction are 3.935-3.941 for persistent hot and 1.744-1.758 for remap.
Raw nanoseconds, p95s and all process values remain in `remote/parsed.json`;
`verify.py --summary` reconstructs the candidate/baseline ratios independently.
These are descriptive ratios, not confidence intervals or a HIP/HSA comparison.

Across the four candidate run/direction hot-sample buckets, currentness accounts
for 99.979-99.981% of summed measured lower-call host intervals. Per-submission p50
summed native-call host intervals are 2.384-2.523 microseconds. These intervals
are not GPU copy latency. Currentness remains the dominant measured stage.
All four diagnostic transcripts contain 162 submissions and 162 completions,
with no pending polls; warmup, prime, remap and hot populations stay separate.

ELF SHA-256 identities are distinct:

- Baseline: `add6984d829e2416537b95904317d9fa3c786b2c5d5ae6e64586911ebdfa4389`.
- Candidate: `38541bfdbe18443a6ddac34c38aad82310d9a0f89dd57bd8686f3f2794917abf`.

All 23 local and 62 remote recorded commands exited successfully with their
process groups absent. All 191 remote result files were collected and matched
the remote inventory before cleanup. Remote path/process absence was recorded;
the controller also reported its local payload removed, with no primary or
secondary failures. Fresh external checks independently confirmed both remote
and local absence after collection.

## Predeclared Protocol

- One freshly admitted physical MI300X pair, not an exclusive reservation.
- 1 MiB, depth 1, 10 warmups and 30 measured copies per direction and mode.
- Diagnostics on: baseline, candidate, candidate, baseline.
- Diagnostics off: candidate, baseline, baseline, candidate.
- Both endpoints checked before each process; both checked at least two seconds
  after process closure, then again at least twenty seconds after settled checks.
- Eight processes, 48 endpoint observations, 62 bounded remote commands.
- Source bytes, modes, payload and both binary identities checked throughout.

The unit of replication is the process: two processes per cohort/diagnostic mode,
not thirty independent process replicates. Ratios are descriptive, with all raw
process values retained. Stage intervals are host wall time, not GPU timestamps.
Shared-host admission cannot establish isolation, causation, statistical
significance, per-discovery cost, or a matched HIP/HSA speedup. Identical binary
digests, if produced, are reported explicitly and cannot establish changed codegen.

## Safety And Verification

The user allows any free pair. Every workload still requires fresh strict endpoint
admission. No other user's process or file is removed. Private owner markers bind
remote paths to the exact tooling commit and input manifest. Initial closure rejects
undeclared target caches, Cargo configuration, links and other files in that path.
Builds use two jobs, no incremental compilation, and at least 8 GiB initial free space.

Pre-native creation/upload failures attempt exact-owner cleanup and absence checks.
After any native attempt, complete result collection and digest equality must precede
cleanup. A failed collection conservatively retains the owned remote path and local
payload for recovery, preserving the primary failure. Recovery must use `owner.json`
and the same ownership-checked helper; never delete shared paths or uncollected data.

Prelaunch calibration: `python3 -I test_campaign.py` plus the pinned prior parser
and observer suites: 27, 21 and 19 tests passed, with archived command receipts.
Postcollection calibration: `python3 -I -B test_verify.py`: all 14 tests passed
in 279.166 seconds. Its external execution transcript is not included in this
sealed packet. An initial external run failed its valid-packet case because
ad-hoc inspection imports had generated an undeclared `__pycache__` directory.
Only that generated cache was removed, then the entire suite was rerun with
bytecode generation disabled. Source, tools, receipts and native transcripts
were unchanged; the strict archive-membership check was not relaxed.
Verification: `python3 -I -B verify.py --allow-unsealed --summary` before sealing,
then `python3 -I -B verify.py --seal`, and subsequently `python3 -I -B verify.py`.
Verification requires the repository's pinned Git objects, CPU packets and signer
trust file. It is offline with respect to the device, not a standalone packet that
can be verified without these dependencies.
Cleanup checks in the verifier replay historical evidence; they do not contact
the host or promise that those paths remain absent forever. The fresh independent
absence checks and independent artifact review above were external to the archive.
