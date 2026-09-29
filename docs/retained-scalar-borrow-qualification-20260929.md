# Retained scalar-borrow qualification — 2026-09-29

This private one-shot component preserves original scalar-borrow discovery,
Census/Scan and resolve semantics while retaining the locals, starts and reads
vectors, including partially allocated capacity, through refusal and the
enclosing caller's error/unwind boundary. Its source/type/target binding and
custody checks use the original preparation Budget, work ledger and accepted
credit counter. No replacement budget or internal refund is introduced.

The original discovery order and non-short-circuit candidate scan are unchanged.
No-candidate discovery allocates no Census. Original reserve/resize ordering,
exact place identity, private provenance, lifetime restrictions, escape,
overlap, volatile/atomic exclusions and cross-block/call behavior remain
required. Complete views refuse source/counter/ledger substitution, sticky
denial, retry and live held-credit rollback. A future enclosing owner must
retain state through checked postflight, then drop it before releasing credits.
Typed accounting is logical; it is not allocator-overhead, stack or RSS accounting.

## Qualification

On compiler main `ee7b2795b947faaaa4715636251f67dcf9de1a02`, with the three
reviewed scalar-borrow leaves installed, the full selected regression gate passed:

- 270 authority/capability tests (four ignored), run serially;
- 20 policy/runtime-manifest integration controls;
- 370 model tests;
- 3,390 backend tests (197 ignored), including all 16 new borrow controls;
- backend/extractor builds and whitespace checks.

Receipt: `c681ccd9531b8735f355447aaa6bc2e05c1868f3a84c7e94f85c11eec5b81174`.
Root checked the complete request/stream/input pins and equal before/after
source, inputs and tool identities. Independent source review checked original
algorithm inverses, complete source/data controls and typed accounting.

The 16 controls include original full DATA and resolve comparisons; all lower
work/storage frontiers; no-candidate and malformed source; partial-vector
retention; lifetime/escape/repeated-write/overlap/call exclusions; source and
physical-resource custody; callback error/panic retention; and prohibited
semantic-adapter/refund methods.

## Limits

This is an inert prerequisite for the genuine whole-root path. It does not
activate ordinary production, establish a genuine nonempty borrow/singleton
contract, or replace actual SharedValueReads with an empty stand-in. Shared
analysis ownership, projected views, distinct lazy scopes, slice preparation,
argument writers, bounds and the complete production transaction still require
their exact original source/resource joins.

The five genuine Rust-source sessions were separately qualified at the preceding
[capability-prefix checkpoint](genuine-capability-prefix-qualification-20260929.md);
this borrow regression did not rerun that ignored source ladder.
No GPU run or physical debugger capture is claimed. Broad accepted exits remain
M1/V1/V2/U1/U2/U3 (6/18).
