# Application Supervisor Startup Qualification

This checkpoint implements the supervisor side of the ordinary multi-GPU
application startup path. Application registration and its root observation gate
remain one owner through issuer launch and readiness. Cargo and host activation,
retained remote proof custody, and ordinary two-GPU execution remain outstanding.

Source: `370f6d859497638134e203d0d33328f9b7bee4fb`.
The signed source commit, exact patch, frozen source inventory, test binary hashes,
commands and logs are in [evidence.tar.gz](evidence.tar.gz).

## Implemented Behavior

- One receive classifies exact ordinary 184-byte/two-right and application
  840-byte/four-right packets. Crossed profiles, ancillary truncation and malformed
  packets reject without trying another receive; the original control snapshot
  predates receipt.
- Opaque application registration retains its full binding, compiler observer and
  original observation gate. Private launch routing preserves required registration
  and issuer binding even when test profile enforcement is disabled. The fourteen
  issuer inputs are unchanged; neither proof peer nor gate enters that source table.
- The public readiness publisher joins exact live issuer readiness with the original
  gate outside the registry mutex. The completed observation owner retains the
  original root and observer endpoint through each send attempt. Failure keeps
  cancellation and exactly-once reaping with the original issuer pidfd owner.
- A distinct canonical 208-byte record binds application registration and unchanged
  compiler readiness. Client admission requires that record, the exact retained
  binding/policy and terminal EOF. Caller deadlines cannot extend the original
  application transfer deadline; supervisor application startup is also bounded
  across its phases.
- The shared client EOF reader now distinguishes empty seqpackets from shutdown
  using ancillary-aware receipt with SO_PASSCRED. Empty packets queued before
  enabling it, including empty packets followed by close or carrying rights, reject.

The root gate precedes root Ready by design. The dedicated supervisor record does
not assert that the application consumed Ready or produced its separate Cargo ACK.

## Qualification

The final frozen run passed 938 top-level checks, excluding nested helper reruns:

| Selection | Passed | Default ignored |
| --- | ---: | ---: |
| Process identity | 23 | 0 |
| Runtime protocol | 45 | 0 |
| Broker | 200 | 17 |
| Client unit, binary and integration | 56 | 2 |
| Supervisor unit, binary and static-image selection | 63 | 4 |
| Cargo binaries | 444 | 5 |
| Doctests | 102 | 0 |
| Separately enabled isolated root campaigns | 5 | included above |

The 28 default-ignored entries include helpers and separately enabled campaigns;
this run does not enable all deployment or static-image tests. Ordinary tests ran
as UID/GID 1000. The new local service-profile fixtures follow existing repository
behavior and skip when the invoking identity cannot represent a non-root service.

New tests cover every-byte codec corruption, resealed registration/launch/policy
substitutions, profile confusion, exact packet/rights combinations, no fallback
past a bad first packet, expired/original deadlines and real EOF. The publication
fixture uses a real pidfd-owned issuer plus an explicitly test-support-only local
registration/gate. It covers success, wrong/short/trailing gate data, missing EOF,
timeout, expired original deadline, closed Cargo/root endpoints, issuer exit and
explicit cancellation. Failure sends no readiness payload and reaps the issuer.
Registration is injected after the ordinary test launch in this fixture; it does
not qualify the production fourteen-input application launch or complete registry
to publication composition.

Five existing root campaigns were rerun in disposable namespaces with root/UID1000
separation: application session, registry transitions, registry transport,
application observation, and client registration transport. They qualify those
primitives, not a complete deployed supervisor startup. Six-package all-target
Clippy with warnings denied, scoped formatting, diff checks, unchanged 5,095
source/config hashes and all 20 selected test binary hashes passed.

## Remaining Work

Activate Cargo's actual four-right application transfer and dedicated readiness
admission, and host registration before ACK. Carry Cargo's original deadline
through child admission, transfer, response and strict ACK/EOF, preserve all cleanup
custody, and repeat host publication currentness after root Ready immediately
before ACK. Qualify this under the actual inherited static application filter.

Then implement the fixed authenticated keyless proof custodian and private retained
remote owner, consume them with native invocation premises, and qualify the ordinary
fill/staging/XGMI/readback pipeline on two selected MI300X devices. No new formal
machine proof, GPU benchmark, HIP/HSA parity or speedup is claimed here. MI300X was
not used for this checkpoint.

## Integrity

Source patch SHA-256:
`5ec886cae211fd572c259c983ed42dc3cf92e9617820555c9643fe47e2499276`

Archive SHA-256:
`f1e851b045963b80c4b51496004e1f1dccf5d37dc8eb05fc037cfe8393cc9933`

The archive's MANIFEST.sha256 covers all bundled files. The source audit verifies
the SSH signature, exact committed patch, source inventory and tested binaries.
