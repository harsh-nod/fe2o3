# Native XGMI Backing-Budget Qualification

Status: the source-bound native correctness campaign passes on MI300X GPUs 5/6.
This qualifies the exact [budget witness](../../runtime-xgmi-backing-budget-witness-v1.md),
not faults, aggregate memory bounds, formal refinement or HIP/HSA performance.

## Native Result

Signed source: `419fe481803bbaf7a681330024abe39121e67580`. The campaign builds a
default-feature release musl executable from a clean private sparse checkout.
Every source-selector path is present; excluded historical evidence is not a
build input. The exact retained executable SHA256 is
`a08890f3b736cd251e19a10f429d0fdb3d3f6c10e9aacb1d9d3b7a89d7837d0a`.
The ancestry/delta gate binds unchanged production sources to the qualified
budget implementation `685879ab5b3a3e0b31c10f4a44944e0e0962e6ff`.

The argument-ordered endpoints are GPU 5, PCI `0000:a6:00.0`, unique ID
`b7baafd0fb173d8e`, and GPU 6, PCI `0000:c6:00.0`, unique ID `10a254ce4987e716`.
Device byte/record budgets are `8192/3` and `24576/2`; ordinary coherent budgets
are `4096/1` and `8192/2`.

Both deliberate one-byte requests reject with exact Capacity and unchanged
backing, requested-credit and journal accounts. Distinct byte/record pressure
is inferred from the independently checked budget arithmetic. Releasing one
owner on each endpoint permits the original request to succeed, round-trip its
byte and release, with fresh Context identities.

Both directed copies succeed. Two retained-publication snapshots and settled
snapshots show no doubled device-backing charge. Coherent completion charges
follow `0/0 -> 4096/0 -> 4096/4096 -> 0/0`. All 36,875 checked bytes pass the
independent full requested-buffer payload/guard/source and retry oracles.
Requested and device charges refund after resource release; native shutdown
refunds cached coherent completion charges. Final accounts are zero and healthy.

All six strict host observations pass: both GPUs before work, after the fixed
two-second settling delay, and after the fixed twenty-second delayed interval.
These are point-in-time observations, not exclusive reservation or proof of
continuous host idleness. No native failures or physical OOM were injected.

## Evidence And Cleanup

The exact remote inventory was collected before removing only the marker-owned
directory `/home/harsh/fe2o3-xgmi-backing-budget-20260924.e0196987e85029c1`.
Separate remote path/process absence checks pass. Runtime cleanup comes from
the checked executable receipt; remote owned cleanup is a separate controller
fact. Their authority is not interchangeable.

The private 102-MiB sparse checkout and 1.2-GiB build cache are removed after
collection and replay. Their paths are independently absent. Raw campaigns and
the retained payload remain available; no other user's resources were removed.

`qualification/` is a byte-exact copy of the signed CPU witness packet, retaining
the interrupted and superseded attempts. Its final twelve stages pass five
example tests per GNU/musl target, twenty Python tests without skips, formatting,
strict Clippy, tool continuity and 3,917 unchanged source hashes. The native
campaign independently passes five release-musl tests and twenty Python tests
with the actual release CLI. Earlier full production library qualification is
linked from that packet; these example tests do not replace it.
The release test output is quiet: its complete five-dot/result frame is checked,
but it does not provide a named roster. Exact named rosters come from the two
CPU target transcripts and their identical signed example source.

Run `python3 -I -B docs/evidence/dev-xgmi-backing-budget-native-mi300x-2026-09-24/verify.py`
for sealed offline replay. The verifier binds signed source, authenticated
helpers and bootstrap, source/ELF/tool identities, exact commands and deadlines,
receipt bytes, timestamp-contained endpoint observations, fixed postflight
delays, collection and cleanup. `test_verify.py` calibrates hostile changes,
including rehashed records and CPU semantic checks beneath the signed-copy gate.

This does not close native fault coverage, aggregate residency, unified
multi-device compute, production-code proof correspondence, A1/A2 or parity.
No new benchmark or speedup is claimed.
