# Producer Custody And Issuance Qualification

Signed source: `5f3b864937ce359de8c8934dbce557036d12c631`.

This checkpoint composes logical producer-read custody with exact writer issuance.
It does not enable pending producer inputs in RuntimeContext or establish
production Rust/native refinement. The
[model description](../../runtime-producer-read-reservations-v1.md) records the
remaining integration gates.

## Results

| Check | Result |
| --- | --- |
| Whole-crate obligations | 201, including 181 inherited and 20 new |
| Dedicated positive-before and positive-after | 201 verified, 0 errors each |
| Executable-body controls | All 13 failed only the intended postcondition; 200 verified each |
| Full registered runner | Passed all positive proofs, mutation campaigns and 691 standalone negatives |
| Source signature, clean-source brackets, shell syntax, Ruff and whitespace | Passed |

Constructor, registration and Reserved-abort wrappers preserve the combined
invariant over the same journal, storage projection and successful-registration
history. They preserve reader storage, producer counts, and unrelated retained
Pending/Unknown chains. Success/NoEffect reservation status remains independent
of reuse of the former producer slot. Registration appends history only on
success; abort preserves history and the registration watermark.

A constructor-based executable witness covers replay and capacity rejection,
abort, slot reuse with a newer key, and stale-reference rejection. A separate
directly initialized mixed-state fixture exercises all four reservation statuses
and nonempty retained chains while reusing and aborting a retired producer slot.
That fixture is not constructor-to-Pending reachability.

No synthetic test obligation is appended. Counts overlap across importing
campaigns and must not be summed as independent theorems. A new inherited
pointwise reader-prefix lemma preserves the original predicate and contracts;
explicit triggers and isolated solver contexts stabilize its induction and
reader-release preservation. Resource limits and whole-crate checks are unchanged.
Earlier development failures are not included as acceptance evidence.

The full runner started and finished on the clean signed source above. The
dedicated checker authenticates its source/tool inputs before and after every
case. Verus `0.2026.08.09.92f466f` uses a pinned distribution closure of 190 files
and 129,019,839 bytes, checked before and after the campaign. This is not a
hermetic OS, Python, shell, Git or Ruff qualification.

## Evidence And Replay

`qualification/` retains all ten outer command receipts and original stdout/stderr.
`producer-campaign/` contains both closure receipts, all 15 solver receipts,
generated Rust inputs, audited bodies, source manifests and outputs. Tool binaries
are not duplicated. `campaign-summary.json` is derived from the raw receipts.

`qualify.py` is the original machine-specific driver, retained as provenance.
From a checkout containing the source commit:

```sh
evidence=docs/evidence/dev-producer-journal-issuance-verus-2026-09-21
python3 -I -B "$evidence/archive.py" --verify-archive "$evidence" --repo .
python3 -I -B "$evidence/archive-selftest.py" --archive "$evidence" --repo .
```

Offline validation reconstructs the pinned source/checker closure from Git
objects, regenerates every case, checks exact input/command rosters, and rechecks
whole-crate summaries and diagnostic spans. Historical scratch paths identify
receipts; they need not exist. Neither the original scratch tree nor Verus
installation is needed. Validation does not rerun the solver or independently
establish signer trust.

The archive self-test rejects 25 adverse evidence cases. `publication-checks/`
records post-import lint, offline validation, adverse tests and whitespace checks.
The whitespace check excludes the unchanged raw Cargo output in the separate
enrollment packet; its final blank line is retained as evidence. An initial
publication check found that raw blank line and extra final blank lines in two
packaging scripts. The script formatting was corrected before the accepted rerun;
the original qualification receipts were not edited.
Realtime receipt timestamps do not independently establish execution order or
performance. `SHA256SUMS` covers every archive file except itself; run
`sha256sum --check SHA256SUMS` in this directory. Checksums authenticate bytes
only through their signed Git publication, not the truth of a claim. The recorded
source signer is `harmenon@amd.com`, ED25519 fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

## Limits

Enrollment, begin-write, settlement and retirement still need composition with
issuance history and complete retained-chain custody. Physical Vec storage,
fallible allocation, panic/unwind behavior, production Rust correspondence and
native execution remain open. Observed free capacity is not proved equal to
Rust `Vec::capacity()`. The mixed fixture is explicit initialization, not a
reachable production execution.

This packet contains no GPU execution, HIP/HSA comparison or performance
acceptance. No MI300X files or processes were created. The separately tested
[linear enrollment restoration](../dev-enrollment-qualification-2026-09-21/README.md)
does not extend these formal claims. RuntimeContext integration and full runtime
parity remain open.
