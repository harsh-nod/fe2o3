# Conditional Source Agreement Through J

Compiler candidate: `18d64aecadd105e39f55b2deb979c5e97ac56471`.
This extends the [source-to-I checker](conditional-policy6-source-join-20260925.md)
through the existing redundant-store transformation I-to-J at the production
final-graph entry. It does not construct final F or admit native output.

## Production Integration

- Fixed policy6 and the final-graph entry share one consuming Direct B/I
  preparation. Fixed policy6 does not execute the new J continuation.
- Only conditional Direct input selects the new branch. Ordinary Direct and
  UnitLocal keep their existing routes.
- The existing owned transformation constructs J once from actual I. The lower
  checker independently replays its complete history, derives actual J coverage,
  and joins surviving reads/stores, domains, layouts and complete premises.
- N-to-I is checked once inside the same genuine source/proof/contract replay.
  Source and target retain their separate original accounts and live-owner
  reservations; no executable graph is copied or optimizer rerun added.
- Private prefix custody is installed only after enclosing replay and account
  postchecks. It still consumes itself at `FE2O3-COND-FINALIZER-001`; no ordinary
  receipt, native source admission, artifact or launch authority is returned.

The earlier [protected replay](conditional-policy6-protected-replay-20260925.md)
used pre-J compiler `e333c376`. It is not protected validation of this change.

## Local Validation

The guards below used one unchanged 8,176-file source inventory, SHA256
`de35a148f85e95e5117820641a19ff890744f352485522c36bb68bced9871bab`.
Pinned nightly `2026-04-03`, locked offline dependencies, one Cargo job/test
thread, disabled HIP, hidden GPUs, a 12-GiB virtual-memory ceiling and a
1,200-second deadline were used. This report was added afterward.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `conditional-j-lower-integrated-r1` | 18 passed | `e13d749046dd8576069360fa01c57ee77163537ecb0af3889ef7380c820eaf5f` |
| `conditional-j-lower-regressions-r1` | 56 passed | `f11a719a209bb370a8a4e3b25e594502a76145c63ae2c94cdb623175b54a08fa` |
| `conditional-j-backend-tests-r2` | 4 passed, 2 ignored | `c0dc5b490250f7fb09bf1ddf84ea14c8bb5497671bad957d48e88ab6a321a102` |
| `conditional-j-policy6-backend-regressions-r1` | 44 passed, 2 ignored | `63daa570f1d449bee8e13e648ec804cc68ebb94832b90949577ac61d452f3054` |
| `conditional-j-authority-doctest-r1` | 1 compile-fail passed | `667cbaff4f339481d51540321cfb28cd144fc19617e97983e2a0197fd53bdefa` |
| `conditional-j-backend-normal-check-r1` | library check passed | `4f6a323b3e9784991c13f9dc83fa00496dac0d20847aec2e36ac06bd2365638c` |
| `conditional-j-final-entry-regressions-r1` | **40 passed, 54 failed, 4 ignored** | `a80373a79bb8a9a417311eeb7fdf81e2f6b008c4dda17d798f506649624b5ea7` |

Rows overlap; their pass counts must not be summed as independent tests.
The compile-fail test produced the intended E0308: unit is not a formal memory
owner. The first backend build caught a test-only nonexistent target accessor;
the fixture now compares the retained canonical target name without exposing a
new production accessor. Existing unrelated warnings remain.

## Earlier Failure And Gaps

The broad final-entry failures share `CanonicalAssertions(Resource(Accounting))`
at `production_ranked_projection_v1/checked_output_admission_policy3_v1_fixture.rs:383`,
before the modified preparation/continuation route. The representative
`refined_forwarding_native_both_actual_rewrites_share_one_middle_owner_and_final_text`
test also fails identically in the archived **pre-change e333 compiler binary**.
Its verified executable SHA256 is
`6b3b0347beab0608eccf5c251a16cb98a244f5014b600ad032b2c0158579b5e5`;
baseline log SHA256 is
`eb61d1c1eaf9a8bfb91f4fa83340f0addd2ad5f57b2879c7323cd8e38ca5c2be`.
The archived binary enables internal proof staging; the current suite uses
default features. The failure occurs in both configurations and predates J.
This was not a passing ordinary final-graph regression matrix. The shared
accounting defect is addressed in the subsequent
[induction entry fix](induction-entry-accounting-20260925.md), whose focused
tests and representative final-graph regression pass. The subsequent
[conditional F report](conditional-final-source-join-20260925.md) records the
complete 98-test baseline inventory on `a3efc57a9`: 94 passed, zero failed and
four ignored. That baseline result is separate from the new F candidate tests.

An independent read-only review found no concrete soundness, account or route
regression in the new integration, but identified uncovered failure boundaries:
after agreement and before private-owner installation, and late source-account
refusal/unwind inside genuine proof/postcheck nesting. Current target exhaustion
tests fail earlier and do not cover these cases.

The two new genuine-source tests are ignored, not passing. The subsequent F
implementation now retains actual K/P/H/L/R/F and checks source-to-F agreement
within its documented conditional domain, with local component validation.
Fresh protected F-entry/fixed6 separation and late-refusal execution remain
required. Conditional native recovery, existing finalizer integration, safe
launch and the tutorial target matrix remain incomplete. No #272 milestone
closes and no tutorial entry gains end-to-end credit.
