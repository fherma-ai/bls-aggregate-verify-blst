# A slot of attestation aggregates, verified

Two answers to `bls-aggregate-verify/bls12381@1.0.0` on FHERMA with
[blst](https://github.com/supranational/blst) 0.3.17, and the pair is the
point of the kernel:

| directory | the measured call |
| --- | --- |
| `single` | each aggregate checked on its own: Σ public keys, one pairing equation, one final exponentiation — M of them |
| `optimistic` | the whole slot in one randomised pairing context: one final exponentiation, and a fall back to the single checks when it fails |

Ethereum's slot has 64 committees of 128 validators, and a client verifies
their aggregates before it can attest. A healthy slot contains no bad
aggregate, which is why clients verify optimistically; `F`, the number of
corrupt aggregates in a case, is a parameter of the point, so the board shows
both sides of that bet rather than only the half that flatters it.

Measured locally at M = 16 before either was submitted:

| | `single` | `optimistic` |
| --- | --- | --- |
| F = 0 | 8.9 ms | 5.1 ms |
| F = 2 | 8.8 ms | 13.6 ms |

The batch is randomised with a 64-bit scalar per aggregate, because without
it a product of equations being one does not imply each is one and two faults
that cancel would pass. The scalars come from a seed drawn in `init`: not
from the case, so a generator cannot aim at them, and not from the clock per
case, so the measurement holds no syscall.

The validator set is a parameter of the point. A set is fixed for an epoch
and a client has had it for hours before the slot it verifies, so converting
it is `init`'s work and is not measured.
