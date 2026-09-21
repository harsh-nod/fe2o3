# Producer-Read Custody Proof Qualification

Signed source: `237f02c6997eff73ee4d0a17d77c2b9019376715`.

This checkpoint proves conditional preservation of the producer-read logical
model. It does not enable pending producer inputs in RuntimeContext. The
[model description](../../runtime-producer-read-reservations-v1.md) records the
assumptions and remaining implementation, refinement and native gates.

## Results

| Check | Result |
| --- | --- |
| Producer custody obligations | 180, including 155 inherited and 25 new |
| Dedicated positive-before and positive-after | 181 verified, 0 errors each |
| Dedicated invariant-sensitivity controls | All 12 failed only the intended postcondition; 180 verified each |
| Full registered runner | Passed all positive proofs, registered mutation campaigns and 691 standalone negatives |
| Source signature, clean-source brackets, shell syntax, Ruff and whitespace | Passed |

The dedicated positives include one appended test obligation, not an additional
production theorem. The mutations test invariant sensitivity, not refinement of
acquisition, release or settlement implementations. The inherited obligation
counts overlap across importing campaigns and must not be summed as independent
theorems.

The full runner started and finished on the clean signed source above. The
earlier dedicated campaign began before that commit was created; its captured
source and checker bytes match the signed commit. It is byte-bound evidence,
not a second clean-signed-tree campaign. An earlier interrupted campaign is not
included or counted.

Verus `0.2026.08.09.92f466f` used a pinned distribution closure of 190 files and
129,019,839 bytes. The dedicated campaign checked that closure before and after;
the full runner repeated its registered source/tool checks. This is not a
hermetic OS, Python, shell, Git or Ruff qualification.

## Evidence And Replay

`qualification/` contains all nine original command receipts and unmodified
stdout/stderr. `producer-campaign/` contains both closure receipts, all fourteen
solver receipts, exact generated Rust inputs, audited bodies, source manifests
and outputs. Tool binaries are not duplicated here. `campaign-summary.json` is
derived from those receipts.

`qualify.py` is the original machine-specific driver; it is retained as
provenance, not a portable replay command. `archive.py` imports only complete
evidence and also provides offline validation from any checkout containing the
signed source commit. From the repository root:

```sh
evidence=docs/evidence/dev-producer-read-custody-verus-2026-09-21
python3 -I -B "$evidence/archive.py" --verify-archive "$evidence" --repo .
python3 -I -B "$evidence/archive-selftest.py" --archive "$evidence" --repo .
```

Offline validation reconstructs the pinned checker/source closure from Git
objects, regenerates every case, checks exact manifest/command rosters and
rechecks whole-crate summaries and diagnostic spans. Original scratch paths in
receipts are compared as identities, not opened. Neither the original scratch
directories nor the Verus installation is needed. This validates recorded
evidence; it does not execute the solver again or establish trust in a signer.

The archive self-test rejects 23 adverse evidence cases. `publication-checks/`
records the post-import lint, offline validation, adverse tests and whitespace
checks. The original receipts use realtime timestamps; a roughly 0.7-second
backwards step occurred between two dedicated cases. Their timestamps do not
independently prove execution order or provide performance evidence.

`SHA256SUMS` covers the archive except itself. Run `sha256sum --check SHA256SUMS`
inside this directory. Checksums check bytes, not the truth of claims; source
and archive authenticity depend on their signed Git commits. The recorded source
signer is `harmenon@amd.com`, ED25519 fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

## Limits

The proved settlement/Unknown transitions assume complete retained-chain
custody. Base-journal reachability, exact reserved-count/issuance history,
outer acquisition/release, concrete error precedence, fallible allocation,
physical storage and Rust/native correspondence remain open. The retained-count
lemma is not a proof of concrete mutation rejection.

No new production Rust code, GPU execution, fault-injection campaign, HIP/HSA
comparison or performance acceptance is part of this checkpoint. No MI300X
files or processes were created. The separate unregistered lifecycle draft is
not included in these counts or claims. Full runtime parity remains open.
