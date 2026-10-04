# Retained Conditional Fill Producer

Source: `153cc728610dc01ea51ee24653ff94c9fb08f7ff`.
Parent: `e2659b3549b6f554c2f24553830c4e08bf90b834`.
This completes the marker-independent executed-proof producer needed by the
ordinary two-GPU application's future custodian. It does not deploy that service,
transfer remote proof custody, authenticate compiler origin, or authorize a launch.
A3 and ordinary two-GPU application qualification remain open.

## Implementation

`execute_retained_worker_v3_conditional_fill_v1` takes owned exact V2 envelope and
HSACO bytes, one kernel ID, an authenticated analyzer, a protected Verus runtime
lease and an absolute deadline. It bounds inputs before replay or payload copying,
checks the complete compiler closure, derives original conditional inputs/lineage,
executes authenticated machine analysis, checks exact association before proving,
executes the existing protected refinement and independently derives its subject.

The non-Clone result retains the original input allocations, executed proof and
inert subject. There is no receipt import, arbitrary proof constructor, marker,
publication recovery, current-record audit or authority promotion. The consuming
`into_refinement` moves the original proof to the existing local Pending path,
which still independently associates its own retained publication and audits it.
Analyzer/runtime approval remains the deploying caller's responsibility.

Analysis is capped at 60 seconds with 1 MiB stdout and 16 KiB stderr; proving is
capped at 180 seconds and floored to remaining whole seconds. The same absolute
deadline is rechecked between stages and before successful return. Preparation and
cleanup can overrun, so external service supervision is still required for hard
termination. This is not a new formal theorem or proof of the Rust orchestrator.

## Qualification

| Check | Result |
| --- | --- |
| Host library | 228 passed, 3 ignored |
| Host doctests | 14 positive and 41 compile-fail passed |
| Selected vertical regressions | 51 passed, 6 ignored, 18 filtered |
| Protected native campaign | 1 test containing 7 cases passed, 237.24 seconds |
| Host library/tests Clippy | Passed with `-D warnings` |
| Vertical target Clippy | Passed with `--no-deps -- -D warnings` |
| Changed-file rustfmt, whitespace, frozen source | Passed |

These are 335 top-level passing checks, not 341. The protected campaign executes
fresh native compilation/replay, authenticated analysis and protected Verus.
Six cases use the new producer: Good, Marker, HostContract, StaleSuccess,
StaleFailure and ServiceFailure. Payload intentionally retains its original
genuinely proved wrong-HSACO path so that later Pending rejection remains tested.
Original source/analyzer pointer custody, exact subject encoding, one-use service
consumption and publication-currentness controls remain intact.

Eight additional assertions run before the Good proof: corrupt envelope checksum,
short/long payload, changed `.comment`, unknown kernel, expired deadline and both
empty inputs. They require precise early error variants. Changed `.comment`
rejects at finalizer canonical-digest verification. Unit tests cover exact size
limits without large allocation and deadline floor/cap/expiry arithmetic.
There is no new integration test forcing actual analyzer/proof-stage expiry.

The compiler-current-record service still uses test keys. This is CPU/subprocess
evidence, not the no-fork application, deployed compiler/anchor, remote custodian,
native invocation, two-GPU hardware, HIP/HSA parity or performance qualification.
The 18 static startup cases were deliberately excluded from this focused change.

The dependency-inclusive Cargo lint run rejected an existing manual Default impl
at `fe2o3-semantic-import/src/profiler_bundle.rs:791` (`derivable_impls`). That file
was unchanged; no dependency-wide clean lint result is claimed. Development logs
also preserve a corrected missing test import and an intentionally interrupted
unneeded static-fixture rebuild. Its exact process tree and temporary directories
were removed. Final tests use the corrected, frozen source.

## Evidence

[Evidence archive](evidence.tar.gz) SHA-256:
`a2f1612899950b94f030f0cba522f45ef3ffbff03009e0af3c09401de5a34dd0`.
Source patch SHA-256:
`ec9ebe97e7e451bcc53a9498efd36d696a46cb15f5a282327524dcf2588e0e75`.
The archive contains the verified signed source commit, exact patch, 5,500 selected
source/config hashes, five binary hashes, commands, final and development logs,
fresh proof/analyzer/service captures, runtime-input verification and cleanup.
Its manifest and archive comparison passed. `proof.key` is a public verifying key.
Binaries, packages, private keys and runtime installations are excluded.

The native Worker is SHA-256
`fb020a09969938d7fa849e106a146e81668d9d0b0d7dd40ded0e2145e6836715`;
the verified protected runtime manifest is
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Private namespaces exited, test TMPDIRs were empty and the root overlay was
removed. No host protected-runtime installation or MI300X resources were used.
Owned scratch is removed after packaging; existing pinned input/build caches stay.

## Next Gate

Deploy the independently measured keyless custodian, authenticate its session over
the registered application endpoint, and retain this original proof through both
devices' exact invocation settlement. Application EOF or issuer retirement must
not release a leased proof. Then join per-device prepared coverage/storage and
currentness, and run admitted fill -> staging -> H2D -> PUBLIC XGMI -> guarded
readback in both directions. Broader matrices and performance remain deferred.
