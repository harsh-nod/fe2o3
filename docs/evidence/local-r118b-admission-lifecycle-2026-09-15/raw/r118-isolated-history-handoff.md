# R118 Isolated Candidate History

Read-only inventory review on 2026-09-14 verified all 19 candidate records,
original source maps, predecessor hashes and process/clock closure. This is
historical provenance, not integrated qualification. Original files remain
under `/home/harsh/.codex-tmp`; do not rewrite executed runners or records.

## C1

HEAD: `a2feef229758381b66f962ad8be2b87843eb33a3`.
Runner: `c1-run.js`, SHA-256
`023e54bc986f1fa6aed5d85ab29ac82f9e7cbf93ab97243a9a4f9295a0d2486d`.
Eight records, each with `.json`, `.log`, `-source.json`, `-source-after.json`:

- `c1-initial-format`
- `c1-initial-whitespace`
- `c1-initial-tests`
- `c1-corrected-tests`
- `c1-runtime-lib-tests`
- `c1-runtime-clippy`
- `c1-corrected-format`
- `c1-corrected-whitespace`

The initial tests failed compilation with E0308, not a behavioral negative.
Corrected focused/runtime rosters pass 9/724; Clippy and formatting pass.
Final map hash:
`f850a10749a5edbd6223ac6f695ad0c855bdf24a7238de23e0f6350e38a1f733`.
Final map artifact byte SHA:
`66ce95b3b783774640cfdcd9501e2e9ec2a734f16b8117e5d345d802cbbf0615`.

Preserve contract `c1-raw-utc-boot-monotonic-v1` on original boot
`bc684b4a-2d08-43a8-ad02-b55a8fa1b726`. Its 5,680 entries consist of 5,299
`{sha256}` objects and 381 `{unmaterialized_index_entry}` objects. Validate the
latter against their original Git index descriptors and explicit skip-worktree
status, not fabricated materialized hashes. Do not join this clock chain to the
new boot. The two actual promoted files were materialized and checked exactly.

## C2

HEAD: `4756971168f6b4f2c33d217f5d476ccde8ea2740`.
Runner: `c2candidate-run-v1.js`, SHA-256
`4300b6f2e75fa076d800c77fa3f4501fb089ac11501388499a92ce82db3860ba`.
Five records with the same four artifact suffixes:

- `c2candidate-initial-format`
- `c2candidate-initial-identity`
- `c2candidate-runtime-lib`
- `c2candidate-clippy`
- `c2candidate-format-check`

All pass; focused/runtime rosters are 4/719. The first formatter changed source;
the remaining endpoints match. Final flat map has 5,686 identities and hash
`09292cf22fb94872326a7c054e7e5f98fcca35e961b51d410f150e9157b14bc7`.
Final map artifact byte SHA:
`aa213e9c0d3f1ec911063041ca39b5b9d67d6e1464d7201bc24933fd6e5f7eb4`.
`c2candidate-negative-handoff.md` predicts 19 unexecuted mutations, SHA
`fe871dae755983c8394e98fa41f8b1be3a33df85b18c27a7c767c4765a7e3730`.

## C3

HEAD: `4a49234a03fc3759bd1d3ca330197619b4ae8ab0`.
Runner: `c3candidate-run-v1.js`, SHA-256
`9c14e283db59c90ecc0cc7dab0e840901c22bfb3f2660dad0c420dccac5dbd4e`.
Six records with the same four artifact suffixes:

- `c3candidate-initial-format`
- `c3candidate-reviewed-format`
- `c3candidate-initial-completion`
- `c3candidate-runtime-all`
- `c3candidate-clippy`
- `c3candidate-format-check`

All pass; focused/runtime rosters are 5/720. `runtime-all` ran runtime `--lib`,
not full dependency closure. Both formatters changed source, with intervening
manual review fixes; the final four endpoints match. Final flat map has 5,689
identities and hash
`b4d158f6b305f506d52c1ac0b6364adb1dee16b085dcc96f3f26307b931aa0e1`.
Final map artifact byte SHA:
`866876608b42237c4c3d803766cf50f0f518d56ef86b46494a42060a6c378ae7`.
Original `c3-lifecycle-handoff-r116.md` SHA:
`4eade01ccc0e0a4e9b385076fc7361b3837c2f85ca7d1663f92bddb0faabe654`.
The later `c3candidate-negative-handoff.md` gives unexecuted production and
helper-calibration predictions; pin it separately before qualification.

## Integrated Cohort

Minimum original preservation is 81 artifacts: C1 33, C2 22 and C3 26, including
their runners and the two original negative/design handoffs named above. Pin
later handoffs and candidate documentation separately; do not treat mutable
documentation as an executed source prerequisite.

Initial promotion checked all eight modified-file parent blobs against R117
`a07ec44309e214f2a8ef0e687e610c8e60a36224`; all eleven promoted files matched
their candidates byte-for-byte. The initial integrated map is
`526da8f7036ffed98099eb2ff10c07b3f516e094dd7184c57bb75f07b0ba6929`
with 5,692 identities. Format, all 733 runtime tests and strict Clippy pass in
three separate `r118-initial-*` records, independently reviewed.

Subsequent integrated-only C1 routing observations change context.rs and the C1
test module. Preserve the initial source cohort and later three-field/four-field
review cohorts separately. Original isolated candidate sources remain untouched. Fresh
final-source full, contract, focused, compiled-negative/restoration and archive
gates are still required. No native, formal or performance result is implied.
