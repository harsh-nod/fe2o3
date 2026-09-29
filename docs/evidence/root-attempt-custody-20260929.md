# Root Attempt And Publication Custody: September 29, 2026

This is component integration and lifecycle validation for
[#272](https://github.com/harsh-nod/fe2o3/issues/272). **M0-M7 remain incomplete;
the complete protected production-to-safe-GPU-launch matrix remains 0/47.**
Existing runnable kernels are not being classified as nonfunctional by that count.

## Implemented Boundary

- `NativeAttempt` consumes and owns the original compiler trace and root session
  separately from its removable issuer. Cancellation stays armed through the
  outermost fallible startup scope. Returned growth is above both consumed inputs;
  removing the issuer cannot lower the full charge or substitute either owner.
- Original cleanup slots can retain a separately funded late payload. Builders
  preserve acquired custody on failure/unwind; terminal retries preserve the
  original account. Temporary aliases drop before publishing the retry marker.
  Retirement work includes the declared payload storage, not just a fixed header.
- V5 publication custody has a pre-acquisition retained-storage quote and actual
  lease/token constructors borrowing a spawn-exclusion barrier through rollback.
  Nonblocking recovery uses the existing shared recovery engine and returns Busy
  for a contending writer without changing the publication. Filesystem I/O is not
  claimed to have a deadline bound.
- `RootPublicationCustodyV3` installs the funded payload before acquiring either
  owner and preserves partial acquisition in the original cleanup slot. It is
  **not yet attached to the coordinator's production dispatcher or root RPC**.
  Established-connection authentication is separated from live-compiler checking;
  it grants no retirement permission by itself.

See [the root-control contract](../compiler-execution-root-control.md) for the
remaining durable-retirement and replay obligations. No alternate production
selector, authority provider, resource-limit relaxation or issuer restart path
was added.

## Local Validation

The complete six-library run and three static builds exercised the Git-visible
source later committed as `3e14855ed8481136194b7781d70e5463809749e8`.
All 9,374 source files had the same before/after snapshot:
`f3cb7d5a260c1a988702dea72d9f50933947ba616af1a32900f85a2f263c24d5`.
The guards also checked tool hashes. Runs used pinned nightly-2026-04-03,
locked/offline Cargo, one build job, serial tests and disabled GPU access.

| Library | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Artifact transaction | 335 | 0 | 0 |
| Broker authority service | 391 | 0 | 20 |
| Compiler execution coordinator | 253 | 0 | 4 |
| Compiler execution issuer | 31 | 0 | 2 |
| Compiler execution protocol | 110 | 0 | 0 |
| Protected service spawn | 258 | 0 | 7 |

Total: 1,378 passed, zero failed, 33 ignored. Privileged fixtures were not silently
counted as passing. The artifact suite includes shared V3/V4 currentness consumers
and the new actual writer/OFD-lock contention tests. Broker payload tests cover
actual V5 owners and locks on success, scope refusal, unwind, and token-work
exhaustion after retaining the lease. They do not exercise live
`RootPublicationCustodyV3::observe` or a protected proof.

The artifact/broker/spawn documentation tests also passed: 32,
91 + 1 merged, and 63 + 2 merged respectively. Their earlier source was `2b0fa7cdf`, snapshot
`d37b08f061047c70e8643828cd7f1623fd17a3de2744059b2d666bd4852f6991`.
Initial payload-test attempts failed because the isolated fixture lacked explicit
path-guard selection and a mode-0700 runtime directory. Those fixture setup errors
were fixed without changing production guards; the failed reports remain archived.

The later upstream Shared-place changes at `1797b5741` were merged as
`221f64fcb6c9ab9b6d1075bbf72f717c3ce34bb2`. A separate guarded broker/coordinator
library compile check passed on its 9,377-file snapshot
`70f4b8a8502552a7a2f281232404619dd588cb69e092e0225bc2d2c500237d1d`.
The full library and native matrices were not rerun on that later merge.

## MI350 Native Matrix

The existing ten-case driver passed on `mi350` with the freshly built static
issuer, anchor helper and anchor daemon. All three passed secure-entrypoint,
static ET_EXEC, no-loader/dependency and non-executable-stack inspection.
The unchanged isolated image was
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`.
Its root/input mounts were read-only, network/GPU access disabled, CPU limited to
one, memory to 2 GiB, PIDs to 32, and the outer execution timeout to 600 seconds.

Exact successful cases: `ready-cancel`, `ready-unwind`, `ready-image-mismatch`,
`ready-issuer-exit`, `ready-compiler-cancel`, `same-uid`, `corrupt-state`,
`zero-timeout`, `short-work`, and `short-storage`.

The removal/unwind/issuer-exit cases now validate the same actual compiler trace
and root session after issuer removal, reject a second input transfer without
calling its callback, preserve the full charge, and reject a foreign account.
The compiler-cancel case also checks cancellation after an outer accounting scope
fails following a successful original-session join. No compiler instruction is
resumed: the held compiler is still a mechanical fixture, not production rustc.

The driver checked all ten markers, unchanged binary inputs, container exit zero,
and removal of its container, remote scratch and private SSH control directory.
It reported `cleaned: true`, with no cleanup errors and no automatic retry.

| Native Input | SHA-256 |
| --- | --- |
| Coordinator tests | `d533553aa62361acdac43217fae1ca941e5fbcbf36465f6936526d6b34ecce3f` |
| Held compiler fixture | `5cd9ee2d9194b8302fc39d9ed940bd5a2da3166a130596a8e957e5acb2934af5` |
| Static conditional issuer | `820a35e5cbc48f91fccdf5d62e646f0e870b6b4cd27cd7841fdeacee6af9b9ad` |
| Static anchor helper | `da48783f7d32ef89a992634e6c9170921fe6db77190d2ff34bd0dc6bf6881275` |
| Static anchor daemon | `caa2d92420282dffe726c4c8232c4adc7f1bd5f227ffce90ce24e8bc45614fa7` |

## Evidence And Remaining Work

Local reports are under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Report | SHA-256 |
| --- | --- |
| `root-attempt-libraries-r11.log` | `06234022027e583f34b44014f63442206989f5a0a81995fa9286d0ecef314e54` |
| `root-custody-doc-r10.log` | `1e6f6abc57fb437bdb93948ad23b3e4bc90fe43878583fd7ff6b9c1f9304d199` |
| `root-attempt-postmerge-check-r12.log` | `cb25c6d6cd2f6a14c44649c166b657df77948a0fe82d1912050dcf2b32c9b361` |
| `root-attempt-native-r11/outcome.json` | `9bffd4e18ba3b669fefc041be531206e02da71a11d928226f9cbfeb0d25acd15` |
| `root-attempt-native-r11/status.json` | `59b1d84c5010a8f9f7832884003e5425f4bdc09fcc9015e87467a2915f646d54` |

Next: expose complete publication observation/revalidation work and scratch
quotes, attach the concrete owner through a phase-checked `CompilerTrace` adapter,
and test a genuine live compiler/publication association. Then wire authenticated
Observe/Validate/Retire/Reconcile and exact durable acknowledgments/tombstones.
Successful issuer replacement needs a new admitted connection, not reuse of the
closed one-use input transfer. Protected proof execution, generated safe-host
activation, all 47 positive/negative simulator/GPU matrices, and the release/site
gates remain unfinished. This checkpoint does not deploy the tutorial site.
