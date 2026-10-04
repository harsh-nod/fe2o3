# Retained scalar inventory component — 2026-09-28

The private scalar inventory component now retains its original counts, block
tables, assignment tables, address-escape data and current block candidate in an
owning frame. This prepares a later source-ordered continuation; it does not yet
connect scalar analysis to the genuine BeforeScalar entry or production routing.

The original inventory algorithm and debit order are preserved after an explicit
32-work-unit typed-frame prefix. Seventeen logical accounting rows are added.
Partial allocations remain attached on backend error or unwind. Terminal state
is recorded before fallible work; wrong source or ledger, occupied retries and
denied resources are refused. These logical charges do not measure allocator
capacity, stack usage or process RSS.

All 13 new controls passed: independent original data/debits, saturation,
success/error/panic custody, partial malformed input, exact and one-short limits,
all shorter work limits, header tables, detached/wrong-ledger/retry cases,
call targets, shared/mutable borrowing and lexical order. Full qualification
passed 356 model and 3,222 backend tests (197 ignored), plus backend and extractor
builds.

Full regression receipt:
`2e33325b3fd92bf966024543f4ac208801ea1c0a70bf25d141efd0480774aaca`.

Independent source review:
`3360ebe61de7cd921e60a84039fcd6f1f39484a1aff2418db7b9107b209a748e`.

This qualification did not run a new genuine scalar hook or a GPU kernel.
A later factory must keep the original source loan and actual owners outside
checked postflight; lexical pointer and ledger tags alone do not grant custody.
Retained FIFO/carrier/provenance/capability preparation, genuine integration,
argument writers, joint bounds and production admission remain separate work.
Broad accepted milestone exits remain M1/V1/V2/U1/U2/U3 (6/18).
