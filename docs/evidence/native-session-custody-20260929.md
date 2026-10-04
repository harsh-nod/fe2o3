# Native Session Custody Checkpoint

## Scope

Code checkpoint: `b69d2da8b`, following `c508e7a2c`. The shared V2/V3 issuer
retains its actual local occurrence from Prepare through durable publication.
See the [session contract](../compiler-execution-native-session.md).

This does not connect privileged root observation, run a protected proof, or
qualify any additional GPU kernel. Issue #272 and the 47-kernel gate remain open.

## Validation

All final source runs used pinned nightly `2026-04-03`, offline locked Cargo,
one build job, serial tests, a 1,200-second timeout, a 12-GiB address-space cap,
and disabled GPU visibility. Source and tool snapshots remained unchanged.
Tested source snapshot (9,291 git-visible files, before this evidence page):
`c3fcc7b41114082440ff1da945dec3e2caf556eeaf4454b4c63cbb66c358137c`.

| Run | Result |
| --- | --- |
| `issuer-session-fixtures-rc` | All 10 new session tests passed |
| `issuer-session-broker-final-ra` | 246 passed, 97 failed, 6 ignored; not a green package run |
| Native issuer subset of that full run | All 86 passed, including both publication-failure sweeps |
| `issuer-session-docs-final-ra` | All 77 broker doctests passed |
| `issuer-session-integration-final-ra` | Broker, issuer, and coordinator `--all-targets` check passed |
| Hygiene self-tests | All 8 passed; scoped formatting and whitespace checks passed |

The publication sweep checks refusal at every one of 15 continuity checkpoints,
bracketing five durable commits and the anchor exchange. It checks that refusal
returns no ACK, performs no later durable write, preserves the original resource
account, and permits exact journal recovery and ACK replay. These fixtures do
not construct a production Admission or NativeOccurrence.

## Environment Limits

All 97 full-suite failures also failed on the previously built broker executable
from the passing `root-issuer-entry-services-ra` run. Baseline executable SHA256:
`565a4bf9df142cf6cb8af26c0b25f1f4cc6e47b838bf194755eb839371628624`.
The automated comparison reproduced 96 failures; its remaining 10-second timeout
was rerun with 60 seconds and reproduced the same prepared-state refusal at
10.01 seconds. Socket inspection/admission reports `EPERM` in this environment;
related process/helper tests also fail. No admission check or test was weakened.
This baseline comparison does not convert those failures into qualification.

GitHub and MI350 hostname resolution failed here. No remote test job was started,
and this checkpoint grants no protected-runtime or hardware credit. Local cleanup
removed only obsolete owned test binaries and dangling cache links; source,
worktrees, reports, and current baseline executables were preserved.

## Remaining Work

Authenticate an issuer-only root control channel, retain the actual root-owned
occurrence across requests and issuer failure, and require exact idempotent
retirement before Published, Recover, currentness, or the next Prepare. Then
connect the production attempt, runtime/descendant custody, safe host activation,
protected proofs, and the complete target-matched positive/negative kernel matrix.
