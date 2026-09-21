# Producer-Read Lifecycle Proof Qualification

Signed source: `85aa7f7e02f467ac298a5adc66d6ceb62f185039`.

This checkpoint verifies executable logical producer-read lifecycle relations.
It does not enable pending producer inputs in RuntimeContext or establish
production Rust/native refinement. The
[model description](../../runtime-producer-read-reservations-v1.md) records the
assumptions and remaining integration gates.

## Results

| Check | Result |
| --- | --- |
| Lifecycle obligations | 254, including 180 inherited and 74 new |
| Dedicated positive-before and positive-after | 254 verified, 0 errors each |
| Dedicated executable-body controls | All 19 failed only the intended postcondition; 253 verified each |
| Full registered runner | Passed all positive proofs, registered mutation campaigns and 691 standalone negatives |
| Source signature, clean-source brackets, shell syntax, Ruff and whitespace | Passed |

The lifecycle proves exact logical status and ordered preflight decisions,
acquire/release commit contents, unchanged-on-error state/output, arena and
count preservation, incarnation and shared-budget bounds, stable-wrapper
composition, and the ordered unread guard. Nonempty witnesses cover late-item
rejection, multi-item round trips, observed release-capacity rejection, slot
reuse and stale references, and release in all four legitimate producer states.

No synthetic test obligation is appended. Counts overlap across importing
campaigns and must not be summed as independent theorems. Consumer-kind,
equal-producer-ID and shared-capacity controls insert focused wrong admissions,
not complete guard deletions. Ten mutation-target functions and four recursive
bridges use isolated solver contexts in the positive source; contracts,
resource limits, whole-crate checking and diagnostic requirements are unchanged.
Earlier failed or interrupted qualification attempts are not included or counted.

The full runner started and finished on the clean signed source above.
The dedicated campaign captures pinned inputs and checks their identities before
and after each solver run. Its input bytes match the signed source above.
Verus `0.2026.08.09.92f466f` uses a pinned distribution closure of 190 files and
129,019,839 bytes, checked before and after the campaign. This is not a hermetic
OS, Python, shell, Git or Ruff qualification.

## Evidence And Replay

`qualification/` retains the original full-run command receipts and unmodified
stdout/stderr. `producer-campaign/` contains both closure receipts, all 21 solver
receipts, exact generated Rust inputs, audited bodies, source manifests and
outputs. Tool binaries are not duplicated. `campaign-summary.json` is derived
from those receipts.

`qualify.py` is the original machine-specific driver, retained as provenance,
not a portable replay command. From any checkout containing the source commit:

```sh
evidence=docs/evidence/dev-producer-read-lifecycle-verus-2026-09-21
python3 -I -B "$evidence/archive.py" --verify-archive "$evidence" --repo .
python3 -I -B "$evidence/archive-selftest.py" --archive "$evidence" --repo .
```

Offline validation reconstructs the pinned checker/source closure from Git
objects, regenerates every case, checks exact input/command rosters and rechecks
whole-crate summaries and diagnostic spans. Historical scratch paths are receipt
identities, not files that must still exist. Neither the original scratch trees
nor the Verus installation is needed. This validates recorded evidence; it does
not rerun Verus or independently establish trust in a signer.

The archive self-test rejects 25 adverse evidence cases. `publication-checks/`
records post-import lint, offline validation, adverse tests and whitespace checks.
Receipts use realtime timestamps; they do not independently prove execution
order or provide performance evidence.

`SHA256SUMS` covers the archive except itself. Run `sha256sum --check SHA256SUMS`
inside this directory. Checksums check bytes, not the truth of claims; source
and archive authenticity depend on their signed Git commits. The recorded source
signer is `harmenon@amd.com`, ED25519 fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

## Limits

Complete pending-chain custody is a reachable-state premise. The nonempty
lifecycle fixture directly constructs a Pending journal; it does not prove
constructor-to-begin-write reachability. Exact reserved-count/issuance-history
composition, production settlement preflight, physical Vec storage, fallible
allocation, panic/unwind behavior and Rust/native correspondence remain open.
Observed release capacity is not proved equal to Rust `Vec::capacity()`. The
acquisition induction advances a hypothetical incarnation prefix; its states
are not claims about intermediate machine states or panic atomicity.

No production Rust code, GPU execution, HIP/HSA comparison or performance
acceptance is part of this checkpoint. No MI300X files or processes were created.
RuntimeContext integration and full runtime parity remain open.
