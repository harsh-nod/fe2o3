# Retained Issuer Image Checkpoint

Issue #272 remains incomplete. These results do not qualify a tutorial kernel,
execute protected proof, authenticate a root-control session, or authorize GPU
launch. The tutorial manifest and its hardware classifications are unchanged.

## Implemented

Code checkpoint: `3aed0183cb758a08cf0775b7e82d395e7632703e`, based on
`4feb16ea7dfab9c630b6b34622dcec6b3aedcfc0`.

The existing direct root launcher and its readiness revalidation now call the
broker's image observer on their actual retained child and original policy.
The observer accepts no caller PID, executable descriptor, image bytes, digest
provider or admission claim. Child liveness brackets trusted procfs inspection,
policy-length enforcement before reading, shared static-image/runtime measurement,
and reopening the current executable for a snapshot comparison. Its exact work
and scratch allowance is included in both caller quotas. It never re-enters the
retained payload mutex, cancels the child, or replaces profile/namespace checks.

V2/V3 self-image admission uses the same factored measurement implementation.
The new API observes point-in-time, policy-relative image equality only. It is
not authority, policy provenance, readiness authentication, or protection against
later exec. The installed production entrypoint and descriptor ABI are unchanged.

Channel lifecycle tests now distinguish ordinary denials from explicitly ignored
privileged cases. The real-static-issuer startup matrix also includes a changed
executable-digest rejection followed by original-policy validation. That matrix
was compiled, not executed here.

## Verification

All guarded runs used pinned nightly `2026-04-03`, locked offline dependencies,
one Cargo job, serial tests and disabled GPU visibility. Source and tools were
unchanged during each run. The tested source contained 9305 git-visible files:

`145a5e0326b5561b71187e6c441f0ed2aa203a89df2c3c13d5cccb9a4dcb5ba6`

| Run | Result | Log SHA-256 |
| --- | --- | --- |
| `root-image-integration-rb` | Broker/coordinator/issuer all-target checks pass | `2ed5e6bbeaeec176ce26dc8445cc9f5d760dc8ba71e3455d063f5340fc49206b` |
| `root-image-broker-rb` | 266 pass, 97 fail, 14 ignored | `bd0fdcd9ae024e6384051cd102c479059ddb03b279998e5988d98a827c46c1b5` |
| `root-image-coordinator-tests-rb` | 12 pass, 2 fail in the direct-launch unit tests | `435c1afe3c92ef814431baffc22b7fe7f57d088aab959465d0838112e34020cf` |
| `root-image-issuer-rb` | 27 pass, 2 ignored subprocess helpers | `d0c7fa5b2df9a981c9886c175094d1fcffb369574db2983b8a36a1c6a34344d4` |
| `root-image-docs-rb` | 173 doctests pass: broker 83, coordinator 78, issuer 12 | `1ea534e8ea533f15e89d505e233ddd98be40b6d1c58efb705a2cf432409bce34` |

All six new image-mechanics tests and the new ordinary channel resource test
passed. Seven privileged channel cases and their helper remained ignored.
Foreign TID/PID tests use fault injection into actual channel owners; they do not
establish cross-thread transfer or post-fork behavior. The existing unprivileged
channel helper returns without action in an ordinary unselected run.

The 97 broker failure names exactly match the preceding checkpoint. Each was
reproduced on the preserved pre-change executable, SHA-256
`9d6b3ec25558cee7810d26171f6ce7d762866c4586b025ce823bf71cd6a1c3ea`.
One replay initially hit an outer timeout equal to its own ten-second readiness
deadline; after that process was terminal, a 30-second outer limit reproduced
the same failure. Both coordinator socket-inspection failures also reproduced
with `EPERM` on the older executable, SHA-256
`0d7aeac5678f4cf32e4497bdc86ac6ab9523ba402db96c30b7e999df1b4b3777`.
These comparisons do not make either failing suite green.

The initial coordinator filter selected only three ignored privileged helpers.
It supplied no ordinary-test evidence; the corrected run is the table entry above.
Formatting, whitespace and eight hygiene selftests passed. Private worker branches
were integrated and reviewed; no worker performed a privileged run.
The source-delta hygiene check from `4feb16ea7` to `3aed0183c` also passed.

## Remaining Gates

Actual RootSession/RootConnection admission and the post-readiness challenge are
still required. So are coordinated FD12 migration, root-owned occurrence and
retirement replay custody, the integrated production attempt, protected proof,
safe launch, all 47 positive/negative simulator and target-matched GPU matrices,
website/release evidence, and identical public repository mains.

Ordinary SSH probes to `mi350`, `mi350-2` and `mi300x` failed DNS resolution in
this environment. No remote job or remote scratch directory was created. This
checkpoint provides no privileged channel, retained-child image, protected-proof
or hardware execution credit.
