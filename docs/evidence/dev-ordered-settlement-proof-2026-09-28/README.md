# Shared Ordered Publication Settlement

Signed source: `215879d8d5be833aa9b69cdf9bb672be5f72e371`.
Tree: `a5c8a4a340b60047d92e265f44e0f6dca9acb1ee`.
Implementation starts at `facd1ec7e8fa81bfd298c3a337a8c9790039be1a`, following
`e735323b0973f10a2781be772df43140c9ce0bc0`. The two subsequent source commits
locate proof assertions in authenticated source and harden mutation generation.
Both remotes' `codex/r65-runtime-drain-versions` heads were observed at the
qualified source at 18:46:50 UTC (harsh-nod) and 18:46:52 UTC (powderluv).

## Runtime Change

The complete `ReturnedOrderedPublicationV1::settle` executable body and its
indexed-root selection now compile in both Rust and Verus. Rust uses an identity
projection for Active fields; Verus projects those fields through the existing
metadata model's opaque payload. Typed private settlement errors preserve the
previous terminal error strings. The public API, native submit callbacks,
exclusive pipeline loan and its constructor are unchanged.

The order remains exact lookup, elapsed-time write, outcome classification,
Retryable withdrawal or Published timestamp, metadata confirmation, exact receipt
deposition, then profile extraction. Clock expressions stay at their production
evaluation sites. No heap allocation, extra arena scan, latency improvement or
throughput improvement is claimed.

## Proof Contract

The proof composes the existing shared metadata lookup, confirmation and
withdrawal functions with the new shared settlement body. Receipt, profile and
remaining Active/performance fields are arbitrary non-Copy values. Whole-state
equalities preserve every unrelated slot and all untouched metadata and fields.

The raw precondition is only that an exact selected entry contains a successor
publication root. It does not assume a healthy global roster, valid neighbors,
an accepted staged identity, or a particular attempt phase. Exact outcomes cover:

- Missing identity: exact error and unchanged state.
- Unattempted/NativeOwned: exact error and only the duration update.
- Retryable refusal: exact error and only the duration update.
- Retryable success: selected entry removed, physical generation retained,
  logical epoch/frontier unchanged, live count decremented, staged marker cleared.
- Published refusal: exact error with duration and pre-confirm timestamp updates;
  the exact receipt and profile remain indexed.
- Published success: exact receipt moved to completion storage, exact profile and
  observation returned, and exact metadata confirmation effects.

Four no-precondition executable witnesses construct and settle opaque values with
an intentionally malformed sibling: fixture construction, successful publication,
failed confirmation with timing preservation, and successful retry withdrawal.
The malformed sibling remains unchanged. These are solver witnesses, not native
receipt issuance or GPU execution.

The proof-local lowering of `unreachable!` calls Verus `unreached`, whose false
precondition must be proved. It adds no assume, admitted obligation or external
body. Wrong execution-variant panic cases are excluded by the stated root-shape
premise; production panic behavior is unchanged.

## Solver Qualification

Final `campaign-03` passed on the signed source: 37 obligations in each of the
original, relocated and closing runs, including 26 inherited metadata obligations
and 11 new obligations. This is not the separate 44-obligation full-chain proof.
Its prior lifecycle/publication negative controls were not rerun by this campaign.

All ten new executable-body controls produced strictly classified logical
failures: keeping Retryable indexed, early confirmation, skipped confirmation,
lost installed receipt, substituted observation ID, suppressed observation,
substituted lookup identity, delayed timestamp, omitted duration and wrong
missing-identity error. Only the copied executable body changes; specifications
and proof annotations remain fixed. Compiler failures, resource limits, wrong
verifier output, foreign paths and malformed diagnostics are not accepted.

The unchanged SHA-authenticated publication controller records source signatures,
exact signed inputs, four-file relocation, source continuity and pinned verifier
closure before/after. The new four-group synthetic settlement calibration runs
separately in `qualify.sh`; the inherited controller runs the earlier calibration.

Campaign-01 correctly rejected a negative diagnostic originating in a relative
Verus-library macro span. Proof-local extensional assertions corrected the source
location without relaxing the classifier. Campaign-02 passed; review then added
missing/duplicate timestamp insertion-anchor rejection before campaign-03.
Exploratory frontend/proof failures and all campaign attempts are retained.

## Runtime Qualification

Final qualification completed at 2026-09-28T18:46:26Z with aggregate status zero:

| Gate | Result |
| --- | --- |
| CPU receipt / ordered-publication focused groups | 10 / 5 passed |
| All-feature runtime library | 1816 passed, 0 failed, 28 existing ignored |
| Runtime doctests | 8 compile-positive and 44 compile-fail passed |
| Strict all-feature/all-target Clippy | Passed |
| No-default-feature compilation and workspace formatting | Passed |
| Signature, implementation continuity and diff check | Passed |
| Standalone settlement calibration | 4 synthetic groups passed |

Focused groups overlap the full suite. These test real lower CPU receipt custody
and scripted caller controls, not GPU execution or native issuance. No ignored
test is waived. No implementation changed during the final solver or CPU run.
Read-only agent review found no runtime/proof blocker; its mutation-anchor
hardening was integrated before final qualification.

## Boundaries And Next Work

This does not prove native result authentication, private exclusive-loan
construction, actual clock behavior, HostMetadataTable representation, outer
error/unwind, profiler effects, destructor effects, or Pending/retained-resource
settlement. Retryable success discards the withdrawn Active, as production does;
it is not a theorem that its resources or profile survive withdrawal. The
cfg(test) scripted completion branch is exercised by CPU tests, not this native
attempt model. Context, protected Worker/compiler and device-machine refinement
remain separate work.

Next run the existing native mixed-duration artifact and out-of-order owner tests,
then timeout/drop/backpressure observer cells and the two-lane 2,048-retained
receipt cell. The old stopped native packet cannot be reused unchanged. Use a
fresh source/ELF binding, device admission, endpoint observations and owned cleanup.
Retained receipts do not establish thousands of simultaneously unfinished GPU
operations; out-of-order completion does not measure physical overlap.

No GPU test or benchmark ran in this packet. MI300X disk space was inspected
read-only; no remote files were created or GPU/build jobs launched. Issue #182 was refreshed
through the API and remains open. A1/A2 and accepted Native R125, Admission R118B
C1/C2/C3 and Resources R116/V3 checkpoints remain unchanged. Full HIP/HSA
behavioral or performance parity is not established.

## Reproduction

From the signed clean source with the pinned verifier:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/test-ordered-publication-settlement.py
python3 -I -B crates/fe2o3-runtime-model/verus/check-ordered-publication-settlement.py \
  --verus "$VERUS" --output "$NEW_ABSOLUTE_OUTPUT_OUTSIDE_REPOSITORY"
```

Raw campaign:
`/home/harsh/.codex-tmp/fe2o3-ordered-settlement-proof-20260928-WvDLJwsT`.
The archive contains the runtime qualification script, logs, exact proof inputs,
diagnostics and controller receipts. No older frozen packet was changed. This
is a standalone development proof, not the global Verus release gate.

Verify `receipts.tar.xz` against the adjacent `SHA256SUMS`. Its SHA-256 is
`699c5b15e99e5dcff9f6affb8101d8906452eb18d1643f41d7251b92296d17dc`.
Raw files are frozen after archive creation. Documentation publication receipts
are separate in
`/home/harsh/.codex-tmp/fe2o3-ordered-settlement-docs-20260928-gxbY9jSa`.
