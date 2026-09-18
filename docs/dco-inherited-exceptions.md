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

`scripts/check-dco-range.py` recognizes only these full commit identities in
the two named repositories and reports exceptions separately from signoffs.
There is no command-line, environment, prefix, author-wide or ancestry-based
waiver. Every other commit, including merges and future KFD work, still needs
its exact author signoff or the existing independently verified Dependabot rule.
The exceptions do not waive tests, artifact checks or any publication requirement
other than these two missing DCO trailers.
