# Approved inherited DCO exceptions

On 2026-09-18 the maintainer explicitly accepted these two inherited commits
as documented DCO exceptions for publication to `harsh-nod/fe2o3` and
`powderluv/fe2o3`:

| Exact commit | Existing subject |
| --- | --- |
| `3abb7b18ebe5e3cbc32002738bed0f7d72d4f745` | perf(kfd): borrow engineering dispatch payload validators [skip ci] |
| `5ed3840a90db3f03a2cded9becffc0459b737f36` | Amortize CPU-only ordered dispatch preparation fences [skip ci] |

The approval was: "Accept these two inherited exceptions". It was recorded in
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5737431573).
Both commits already existed on canonical main. Their history is preserved;
this record does not add signoff trailers or claim their missing trailers exist.

On 2026-09-23 the maintainer explicitly approved three additional inherited
canonical-main commits for publication to the same two repositories:

| Exact commit | Existing subject |
| --- | --- |
| `7dfbe5cc453f12c1b8b32eeb49f08778876bf6ce` | Refresh exact vendored SDK manifest and target-roster regression [skip ci] |
| `d10f49bfedc26848285d20ec1399c193b3340f47` | feat(kfd): add opt-in ordered64 engineering batches [skip ci] |
| `3d473f9ffc850a3a762efee8cd4f210d363f3354` | feat(kfd): add opt-in ordered64 dispatch timestamps [skip ci] |

The approval was: "Accept these three additional exceptions", recorded in
[#275](https://github.com/harsh-nod/fe2o3/issues/275#issuecomment-5798687623).
Their existing history and missing-trailer status are likewise preserved.

`scripts/check-dco-range.py` recognizes only these full commit identities in
the two named repositories and reports exceptions separately from signoffs.
There is no command-line, environment, prefix, author-wide or ancestry-based
waiver. Every other commit, including merges and future KFD work, still needs
its exact author signoff or the existing independently verified Dependabot rule.
The exceptions do not waive tests, artifact checks or any publication requirement
other than these five missing DCO trailers.
