# Runtime Producer Input Composition

This proof connects the actual producer-input fold to the actual per-input
validator. It preserves reached-call order, first-error propagation, family
cursors, receipt construction, and exact status/error results across iterations.
It does not prove the native journal, credit-account, live-allocation, or Arc
implementations behind the typed observation interface.

## Source Structure

The native `producer_input_fold_body.rs` file is unchanged. The validator and
fold specifications have been extracted into shared proof modules; the native
runtime still invokes the same allocation-free validation and scan bodies.

- The leaf proof has three inputs: its root, shared validator definitions, and
  the actual native macro file.
- The standalone fold has three inputs: its root, shared fold specification,
  and the actual native macro file.
- The composition has four inputs: its root, both shared proof modules, and
  the actual native macro file.

The composition adapter executes the actual validator with the correct input
index, submission identity, launch flag, and active/queued cursors. Ghost
receipts and call traces describe only reached executions. An error ends the
reached prefix; `Unknown` is an aggregate status, not permission to skip later
validation. No new assumed or external proof body is introduced.

Each input has independently supplied typed observation returns. These values
are not an assumed coherent snapshot of an interior-mutable native account.
Each reached native credit predicate must still be related to its actual fresh
call by subsequent refinement work.

## Current Evidence

Complete, unfiltered component proofs have reported 42 leaf obligations,
13 fold obligations, and 64 composition obligations, each with zero errors.
These are separate verifier obligation counts, not additive runtime properties.
Source-only controls reconstruct the preceding complete proof definitions and
bind all 323 native Rust files and five compared model-schema files in the
qualified candidate. The integrated guards bind 327 native files plus the same
five schemas, as separately recorded below.

Signed candidate `6b9e5d87c4386692f7c213a95dd786ccdf42cb45` now passes all
102 qualification stages: nine complete proof runs (each family before,
relocated and after), 81 fresh actual-body logical negatives, six verifier
release checks, five source-control stages and a signed-commit check. Every
recorded process group closed. Independent agent and root readbacks agree,
including reconstructed mutation bytes, raw diagnostics and signed Git blobs.
The result hash is
`735a86e580e37d66390953ed372f61fbf1bc18b6b45622cf1b5bbe5691e14fa0`;
the independent readback hash is
`0aa2bda66cccacc40c57627a8f0bc09a35904c6b97e7e76cba2bee9e81302d70`.
The following calibration history remains separate from those fresh results.

The first 33-stage diagnostic campaign was interrupted by SIGTERM after twenty
launched groups. Its recorder closed all twenty groups. Four selected
observations and five full-positive checks passed, but the original parser
rejected ten full-composition captures because the full-result schema includes
`success: false`. The original campaign remains incomplete and those historical
classifications are unchanged.

A separately reviewed fifteen-stage continuation ran only the eleven previously
unlaunched composition observations. Fresh full64 checks before and after them,
both verifier-release checks, and all eleven observations passed; its recorder
closed all fifteen groups. Neither packet qualifies mutation kills.

Across the twenty-one full-composition captures, the verifier reported 63
verified obligations and one failed obligation per case. Six failures were
assertions, fourteen were postconditions, and one was an end-of-loop invariant.
The twelve validator-boundary cases also reported the exact active/queued
cursor recommendation-note pair. Credit, count, and fold cases did not report
that pair. No unknown diagnostic family is accepted on that basis.

The portable fixture corpus retains normalized structured diagnostics and raw
capture hashes for these twenty-one cases and four selected observations. Its
generator only projects parser fixtures; fixture replay is not fresh
verification. The strict classifier checks typed result schemas, every direct
and nested source span, byte/line/column/text correspondence, macro invocation
and declaration identity, the intended failed function/macro, calibrated notes,
and the exact terminal error summary.

| Local Record | Result SHA-256 |
| --- | --- |
| Interrupted `calibration-attempt-1` | `0456177f95c05639185731f1929b749264fe7497f83ecb7fc60cd2060cc88820` |
| `calibration-continuation-attempt-1` | `87042b1898bde383704e3b54e96318206fe37fd3877c496b5cc86e1ec4e3ee27` |

These identify retained local measurement packets, not public artifact links.

## Signed Qualification

The completed signed campaign freshly runs all 38 unchanged leaf mutations,
all 22 unchanged fold mutations, and all 21 composition mutations. Every
composition mutation requires its complete unfiltered four-file root; selecting
only a caller could omit a changed callee. Each family needs complete positive
and relocated-positive checks, plus a closing positive check. The strict
diagnostic predicate additionally requires signed source, tool, mutation, and
fresh owned-process binding by the campaign recorder.

Legacy standalone campaign entry points remain fail-closed on this extracted
candidate. The unified qualification uses the source-bound classifier exposed
by `check-producer-input-composition.py`; it does not enable an older,
less-specific diagnostic acceptance policy.

CPU evidence for the unchanged native sources is a separate binding. No
compiler-to-machine-code theorem, native journal/credit refinement, allocation
failure or unwind proof, GPU validation, speedup, protected launch authority,
or HIP/HSA parity is established by this component.

## Integration

Signed integration `6c0718c1d` imports the exact qualified proof bodies and
diagnostic corpus. Four guard/test files refresh only the complete current
native inventory and dependent checker hashes. No runtime/model/accounting/KFD
implementation changes occur in this integration. The
[integration audit and CI replay](evidence/dev-producer-composition-integration-2026-10-01/README.md)
record all sixteen candidate-path comparisons and 21 passing source-control
commands. These are not fresh solver or native results. The original signed
candidate campaign remains the proof evidence, with helper-contract limits
unchanged; actual journal, live-allocation and fresh credit observations remain
the next refinement work.
