# Root Control Envelope Checkpoint

Code checkpoint: `ce37a75437a7f95d17d7e692742432a522d5fcbd`, following
`dc2f79f01`. This is further integration groundwork, not end-to-end root RPC or
new tutorial-kernel qualification.

## Changes

- An inert 4096-byte control envelope binds policy, manifest, root epoch,
  connection generation, sequence, operation and exact request/reply identity.
  Hostile rehashing remains possible; authentication and payload admission belong
  to the broker, not this digest.
- A broker-created `RootLaunchChannelV3` owns both initial socket endpoints,
  enables credentials before exposure, exposes only an issuer-side staging
  borrow, and checks its original account, Budget address, process and kernel
  thread. There is no arbitrary-FD constructor or root-end extraction.
- Both native issuer entrypoints check all current inherited FD3..11 slots before
  input duplication can reuse a missing slot. Existing object admissions still
  follow. V1, V2/V3 launch tables and private duplicate floors are unchanged;
  FD12 is not active.

The [control contract](../compiler-execution-root-control.md) documents framing,
custody, migration and the remaining acceptance gates.

## Validation

Final guarded source snapshot: 9,301 git-visible files,
`01ca11abd7e24b495dd40561ef2a11e460d81f41cf96749654243cf38e008718`.
The evidence page and its link are subsequent documentation-only changes.
Runs used nightly `2026-04-03`, locked offline Cargo, one build job, serial tests,
a 1,200-second timeout, a 12-GiB address-space cap and disabled GPU visibility.
Source and tool snapshots remained unchanged during each final run.

| Run | Result |
| --- | --- |
| `root-envelope-integration-final-ra` | Protocol, broker, issuer and coordinator all-target checks passed |
| `root-wire-final-ra` | All 104 protocol and 27 issuer unit tests passed |
| `root-channel-tests-final-ra` | Two negative tests passed; inactive subprocess helper returned normally |
| `root-wire-docs-final-ra` | All 269 doctests passed: broker 83, issuer 12, protocol 174 |
| Hygiene | Source delta policy, scoped formatting and whitespace checks passed |

The two ignored issuer entries are existing fixed-slot subprocess helpers,
exercised by their parent tests. The eight new V2/V3 intake cases cover every
missing/CLOEXEC slot, valid nonconsuming table checks, subsequent consuming takes,
and exact budget refusal. The valid table uses `/dev/null` fixtures and does not
establish service admission. Malformed slots are injected after exec, immediately
before entrypoint invocation; malformed launcher installation remains a distinct
integration test.

The nine envelope matrices cover all operation/direction combinations, empty,
2090-byte and maximum opaque payloads, malformed and resealed frames, exact
request substitution, association mismatches and cumulative resource bounds.
An opaque payload of carriage length is deliberately not an admitted carriage.

## Limits

Only root-identity refusal and resource preflight ran for the new channel owner.
This environment uses UID1000. Positive root creation, partial socket-failure
cleanup, original-account replacement and alias-closure lifecycle tests remain
unverified. The test's privileged-host subprocess branch was not executed here.
The complete broker/spawn suites were not rerun in this checkpoint; their previous
failures remain unresolved, as recorded in the [preceding evidence](root-control-foundations-20260929.md).
No workspace-wide green claim is made.

All three SSH aliases failed DNS; no remote job or scratch directory was created.
GitHub Git access also failed DNS during validation. Publication is not implied
by local commits or browser access to the issue.

## Remaining Integration

The broker can admit a session from concrete owners without an authority-provider
callback: the original root trace view, actual retained issuer child, broker-owned
pair and retained deployment policy/manifest. It must itself validate the running
issuer image/profile, namespace relationships, compiler binding and liveness,
then challenge the measured issuer after exact readiness and writer EOF. Pinned
measurements do not independently prove arbitrary code implements the durable gate.

Keep actual occurrence custody, pending reply, exact retirement tombstone and
cumulative accounting in the attempt's root session, outside the replaceable
issuer connection. Integrate authenticated poll/retirement with the existing
private durable gate; never offer public retirement from a decoded carriage.
Coordinate both V3 startup routes, or explicitly refuse unsupported indirect V3
execution before clone, while preserving V2.

Production attempt integration, protected proof execution, safe GPU activation,
all 47 positive/negative kernel qualifications, website/release gates and identical
publication to both repositories remain required. No milestone closes here.
