<!-- GENERATED from bls_aggregate_verify/bls12381@1.0.0. Do not edit — `--update` rewrites it. -->
# A slot of attestation aggregates, verified

An answer to `bls_aggregate_verify/bls12381@1.0.0`, in rust.

## What you write

`src/solve.rs`, and nothing else.

```
init(Point)         -> state     setup. Not measured.
run(state, Inputs)  -> Outputs   the task. Measured, and only this.
free(state)                      optional.
```

`init` never sees the data, so there is nothing for it to compute ahead of
time. Reading the case and writing the answer happen outside the measured
call, in generated code — which is why seeing the inputs early is not a way
to answer early.

## The shapes

- `committee` — tensor<M x K x u32>
- `msg_x` — tensor<M x 2 x L x u64>
- `msg_y` — tensor<M x 2 x L x u64>
- `sig_x` — tensor<M x 2 x L x u64>
- `sig_y` — tensor<M x 2 x L x u64>
- returns `ok` — tensor<M x u8>

## Generated, and not yours

Every file but the one above carries a header saying so. They are rewritten
by `fherma implementation init --update`, which never touches yours.
