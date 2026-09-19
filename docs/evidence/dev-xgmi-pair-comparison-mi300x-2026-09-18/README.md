# XGMI Pair-Currentness Source Comparison

Status: prepared, not yet executed. This packet does not establish HIP/HSA parity,
formal refinement, or performance acceptance.

The baseline is signed commit `84b61ee39817cee8bfe3cd68312f0a05543bc9af`;
the candidate is signed commit `72eb6b3052b803a26a4007dca10fa2e26138eed1`.
Both selected compiler-input trees are independently associated with their sealed
CPU qualifications. Deterministic archives come from Git blob plumbing, not the
working tree or Git archive attributes. Both are rebuilt with the same frozen
lockfile, release flags and reported compiler, in separate fresh target directories.
The candidate consolidates full pair validation into one fresh discovery; this
campaign does not directly count discoveries or prove machine-code refinement.

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
and observer suites. Postcollection calibration: `python3 -I test_verify.py`.
Verification: `python3 -I verify.py --allow-unsealed --summary` before sealing,
then `python3 -I verify.py --seal`, and subsequently `python3 -I verify.py`.
Verification requires the repository's pinned Git objects, CPU packets and signer
trust file. It is offline with respect to the device, not a standalone packet that
can be verified without these dependencies.
