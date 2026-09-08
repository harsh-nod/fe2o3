# gfx942 R57 N3 qualification policy v1

This directory is the independent policy authority for one fixed DeviceLocal
three-binding qualification sequence. It intentionally reuses the already
reviewed source and COV6 bytes from `trusted-gfx942-vecadd-v1`; the policy pins
their hashes again and uses a distinct signature and runtime gate. Changing the
older vecadd qualification policy cannot broaden this lane.

The lane first attempts `A+B -> C` before C has an authenticated complete H2D
write. R57 preflight must reject it before authority is consulted. It then
initializes C and D, completes `A+B -> C`, reuses that resident output as the
read input for `C+B -> D`, reads all four buffers, and cleans every runtime
resource before emitting its sole PASS record.

The source fixture remains:

- `../trusted-gfx942-vecadd-v1/vecadd.ll`
- SHA-256 `b3412c050ce2182feb669d267e3e7208400c4d16f0865efb7aeafd118c8f7e51`

The immutable object remains:

- `../trusted-gfx942-vecadd-v1/vecadd.hsaco`
- SHA-256 `3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab`

`policy-v1.txt` is the canonical sequence, ABI, memory, performance-observation,
readback, and reporting contract. Its SHA-256 is the typed kernel signature.
The lane is qualification-only and grants no Worker V3 or production authority.
The policy SHA-256 is
`7085aff9607dea2aad41ef21e09535f655953ce11c7a479ca59ac84ce0173524`.
