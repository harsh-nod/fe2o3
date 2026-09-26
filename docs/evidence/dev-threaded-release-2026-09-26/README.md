# Threaded VecAdd Release Closure

Date: 2026-09-26. Development release-artifact evidence for
[#277](https://github.com/harsh-nod/fe2o3/issues/277), not native, formal or
performance qualification. Accepted milestones, A1/A2 and HIP/HSA parity are
unchanged; the issue remains open.

## Source And Profile

Signed implementation: `b8747af738c241f302c50fbbb075a49e7facd699`.
Parent: `4a28bd9052923511c0e80007f531ea7ef7903718`.
The source signature was verified for `harmenon@amd.com` with fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

The actual `gfx942-runtime-vecadd-benchmark` now has a strict release CI gate:
`scripts/ci-local.sh runtime-threaded-release`. Existing `runtime-policy` and
hosted `generic-core` routes invoke it too. Both candidates use the pinned
`nightly-2026-04-03`, `--release --offline --locked --no-default-features
--features fe2o3-runtime/hardware-qualification`, explicit host targets and
matching Cargo metadata. Build directories are fresh and task-owned.

The [supported profile](../../runtime-threaded-release-v1.md) documents musl
static PIE, selected GCC/bfd tools, exact CRT order and resolved link inputs.
Neither production policy nor shared-memory/initialization code changed.
GNU remains a rejected candidate, not an exception to the policy.

## Final Qualification

The full qualifier exited 0. Its only accepted full run is
`qualification/threaded-release-qcs1vmuh`, with `result.json` status `passed`.

| Check | Result |
| --- | --- |
| GNU release | Built; strict audit rejects exactly `dlsym` |
| musl release | Built; unchanged strict audit accepts, zero dynamic dependencies/symbols |
| Metadata | Both 43-package closures pass unchanged policy |
| Thread retention | Nine distinct executable worker/scope/spawn bodies per candidate, covering copy, complete comparison and fill |
| Enabled usage | Both candidates exit 1 with the exact enabled usage diagnostic; no GPU discovery |
| Compiled controls | Dynamic `dlsym` rejects; static `dlsym` passes dynamic-only audit then rejects under the full-symbol guard |
| Input continuity | 5,429 source files, 14 selected tools, 389 target-library/CRT files match before/after and on independent recheck |
| Process completion | All 45 recorded subprocesses have expected exits and absent process groups |
| Calibration | 11 groups pass, including inherited interruption/timeout/process cleanup checks |
| Existing auditor | 28 tests pass |
| CI routing | Independent local dispatch and hosted trust-boundary tests pass |
| Shell syntax / whitespace | Pass |

GNU artifact: 5,416,256 bytes, SHA-256
`04ef87f07a54491048770861621ab9f4c9a81a77b5cbf4dfe77a215228cdec1d`.
musl artifact: 5,530,560 bytes, SHA-256
`3396a5176d30b88ec86fc483fa8b15c482e2ea533496c42536fd42eacae9d676`.
Separate strict-auditor CLI invocations reproduced the GNU rejection (exit 1)
and musl acceptance (exit 0) against these exact retained files. Build times
were approximately 500 and 504 seconds; these are not runtime performance
measurements. The successful run removed its owned Cargo target directory.

## Preserved Attempts

`receipts.tar.xz` retains the full final run, binaries, maps, metadata, source/
tool/library identities, per-command records, disassembly and test logs.
It also preserves the exploratory probe and two interrupted earlier attempts:
`threaded-release-z1wsp4au` and `threaded-release-cy5i0ubu`. Neither interrupted
attempt has a successful result, and neither is a qualification receipt.
Their active build output was not recorded on external interruption; do not
infer successful group cleanup from missing records. Both exec handles became
terminal, their Cargo locks were subsequently observed free, and only their
owned build caches were removed. The final run is independent of those caches.
The probe binary is not the final accepted binary.

Archive SHA-256:
`5fb15563613bfe7515502c21a226b340f6171cd7e77c3f6b71dece16d53b894e`.
The archive comparison against the retained original files passed.

The original logs remain under
`/home/harsh/.codex-tmp/fe2o3-threaded-release-20260926-KTfbZcex`.

## Limits And Remaining Work

These checks establish the selected ELF/dependency/link profile and retained
code, not compiler hermeticity, machine-code refinement or execution of those
thread bodies. Input hashing measures continuity at observation points, not
every executed tool or a hostile-host security boundary. The captured-output
limit applies after collection; it is not a subprocess-memory bound.

Independent read-only review found no code or artifact blocker. It rechecked
the identities and process records, the actual musl symbol table and ELF
headers, and substantive memcpy/memset and full XOR/OR comparison worker code.
This is inspection of retained code, not a proof of its execution or semantics.

The VecAdd benchmark uses HostVisible allocations. Its no-argument check and
ordinary GPU smoke do not exercise DeviceLocal threaded initialization. That
branch requires a separate sufficiently large, multi-worker native campaign,
complete readback, currentness checks and teardown. Debug/release host CI routes
remain in place; no fresh Rust library suite or Verus campaign was run in this
script-only checkpoint.

The newer #277 comment reports the earlier teardown fix and a successful GNU
functional smoke. That historical result cannot qualify this fresh musl
artifact. MI300X hostname resolution still failed before any remote process or
artifact was created. Fresh native lifecycle, protected application execution,
matched HIP/HSA performance and broader parity remain unqualified.
