# R118 Qualification Checkpoint

2026-09-14. R118 is not accepted or published. No Rust negative mutation has
been applied or executed. No hardware, solver or matched-performance run was
performed. All commands started in this goal turn are closed.

Repository: `/home/harsh/.codex-tmp/fe2o3-r61-execution`.
Branch: `codex/r65-runtime-drain-versions`.
HEAD and both published topic refs remain R117
`a07ec44309e214f2a8ef0e687e610c8e60a36224`.
R118 source remains 5,692 identities, compact map SHA
`df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb`.

## Completed This Turn

- Immutable history preflight snapshots validate all 30 original records,
  138 preserved artifacts, 12 source maps and eight source transitions.
- `r118-history-v3.js` and the evidence helper reject contradictory failed
  test rows/summaries in positive logs. V1/V2 history bytes remain preserved.
- `r118-history-support-v1.js` validates the runner/schema/preflight tail,
  including the original schema parse failure.
- The first freeze-contract attempt failed because a test-harness parameter
  shadowed the late mutation callback. Preserved unchanged. The v2 tester fixes
  only that shadowing and passes all 50 contracts; independently reviewed.
- `r118-qualification-run.js` implements multi-file application/restoration
  with manifest-bound terminal identity and complete failure diagnostics.
  Twenty-two in-memory `qualifyMutation` integration contracts pass and are
  independently reviewed. They do not exercise `runEntry` or `main`.
- Actual `r118-freeze.js` completed. `r118-environment.json` pins 213 inputs;
  SHA `7bf4b05a238427c9b7e63197b2bfa0330ee20b7de1393ff96045bdb317c6fcf3`.
- Fifteen subsequent source gates pass, in addition to the earlier GNU/musl
  full prerequisites (2,780 passed, five ignored each). Exact source gate
  commands and test rosters match R117; independent review confirms this.
- Ten auxiliary gates and six frozen suites pass. Primary comparison verifies
  exact commands and test rosters; independent auxiliary/focused review was
  requested from `v3_settlement_handoff`.

Frozen focused suites: identity 9, descriptor 4, completion 5, reply 1,
adoption 15, reservation 14. Reply uses the exact distinct-result CO1 test.

## Closed Chain

The canonical 30-record original history still ends at core contracts. The
later integrated chain is preserved separately:

`r118-mutation-core-contracts-v1.json`
-> `r118-runner-contract-tests.json`
-> `r118-history-schema-contracts-v1.json` (preserved parse failure)
-> `r118-history-schema-contracts-v2.json` (13 pass)
-> `r118-history-preflight-v1.json`
-> `r118-freeze-contract-tests.json` (preserved harness failure)
-> `r118-freeze-contract-tests-v2.json` (50 pass)
-> `r118-qualification-lifecycle-contracts-v1.json` (22 pass)
-> `r118-source-campaign.json`
-> `r118-auxiliary-campaign.json`
-> `r118-frozen-identity.json`
-> `r118-frozen-descriptor.json`
-> `r118-frozen-completion.json`
-> `r118-frozen-reply.json`
-> `r118-frozen-adoption.json`
-> `r118-frozen-reservation.json` (latest).

Key record SHA-256 values:

- Freeze v2: `cf3a0a6fbe15b8dcb280726fa2ea2de3389b861019328c5f7c1af7013574a082`.
- Lifecycle: `6bbd5346351caa19d4b476bcb11ade8a409dd12384344ab3a0ec1a1cd39e9ed1`.
- Source: `760d65e124d02ba1335fc53a839e121d7a1c6a25a7575ade73bf26224738f33d`.
- Auxiliary: `3467ef2fe3028c9f741bd8258293c959007e5dd86ffa3ec67fd27cfe73101f15`.
- Latest reservation: `e94d685694dfca77f4ba529cab8c31b75eae13aec5d23b905d3f3985f588be54`.

## Immutability

All executed helpers and records remain unchanged. In particular, the main
plan/evidence helpers, history v3, support v1, freeze, both freeze testers,
qualification-run and lifecycle tests have executed and/or are among the
213 frozen input pins. Do not edit them in place. Preflight snapshot files and
the failed history/freeze versions are intentional retained evidence.

Only these new helpers remain unexecuted drafts and were created after the
real freeze, so they can still be corrected:

- `r118-qualification-collect.js`
- `r118-qualification-prepare.js`

`r118-qualification-tests.js`, a separately pinned execution launcher, and
`r118-qualification-manifest.json` do not exist yet.

## Next Required Work

1. Finish and test the collector and preparation drafts. Collector review found
   missing wrapper/result-file agreement and exact freeze/lifecycle names;
   those draft corrections are now present but unexecuted. Freeze-name digest
   is `b89b9018782ba81f21d7a6ee658a897fabe71da6d5e5308e21783d53c083f796`;
   sorted lifecycle-name digest is
   `5413c005fa131f2647779f80858c96a1d68fbd04938ed3716c9312ba6acc44ce`.
   Collector expects qualification-tests to export its exact `cases` roster
   without executing tests when required as a module.
2. Add a separately pinned launch controller. Do not call immutable runner
   `main` directly: it lacks the full manifest/baseline preflight and does not
   recheck helper pins between focused runs or before collector launch.
   Reuse its exported `qualifyMutation` and `runEntry` instead.
3. Bind launcher identity and its contract evidence into preparation/collection
   separately, without changing frozen `p.helpers`. Read manifest bytes once;
   validate fixed df2 identity/count, original sources, exact entries, qualified
   helpers, historical membership/hashes and manifest predecessor/clock.
4. Require vacant outputs using lstat semantics, including dangling symlinks,
   gate-failure files, restoration files and qualification-failure files.
   Recheck input/manifest/helper/launcher pins before every mutation, focused
   run and collector. After applying a mutation, check the expected mutant map,
   not baseline-only source validation. Guards must finish before beforeSpawn.
5. Preparation now captures historical bytes before checkBaseline, rechecks
   qualified helpers, source identity, membership and output freshness, and
   derives predecessor hash/observation from one buffer. These draft changes
   need decisive no-write-on-rejection tests and launcher metadata integration.
6. Run qualification contracts, create the immutable 163-entry manifest, then
   execute 78 compiled negatives (74 maps) serially. Preserve all failures and
   restore only authenticated owned bytes. Physical restoration alone is not
   acceptance. Rerun all six focused suites, close collector, independently
   review the archive, then sign and push the completed packet to both remotes.

The broader A1/A2, native generated ISSUE/COMPLETE, production journal/resource
bounds, formal correspondence and hardware/performance requirements remain
open. Keep the full goal active. This turn made concrete verification and
implementation progress; there is no current external blocker.
