// GENERATED from bls_aggregate_verify/bls12381@1.0.0. Do not edit — `--update` rewrites it.
//
// The types your answer is written against, derived from the signature: one
// field per value parameter, per argument, per result. A tensor is typed,
// because the signature already settled what its elements are.
//
// Fields are named as the signature names them (`N`, not `n`), and a harness
// may have no use for one of these types; neither is worth a warning.
#![allow(non_snake_case, dead_code)]

#[derive(Debug, Clone, Default)]
pub struct Tensor<T> {
    pub shape: Vec<i64>,
    pub data: Vec<T>,          // row-major
}

impl<T> Tensor<T> {
    pub fn count(&self) -> usize {
        self.shape.iter().product::<i64>() as usize
    }
}

#[derive(Debug, Clone, Default)]
pub struct Point {
    pub M: u32,   // u32
    pub K: u32,   // u32
    pub L: u32,   // u32
    pub V: u32,   // u32
    pub F: u32,   // u32
    pub keys_x: Tensor<u64>,   // tensor<V x L x u64>
    pub keys_y: Tensor<u64>,   // tensor<V x L x u64>
}

#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub committee: Tensor<u32>,   // tensor<M x K x u32>
    pub msg_x: Tensor<u64>,   // tensor<M x 2 x L x u64>
    pub msg_y: Tensor<u64>,   // tensor<M x 2 x L x u64>
    pub sig_x: Tensor<u64>,   // tensor<M x 2 x L x u64>
    pub sig_y: Tensor<u64>,   // tensor<M x 2 x L x u64>
}

#[derive(Debug, Clone, Default)]
pub struct Outputs {
    pub ok: Tensor<u8>,   // tensor<M x u8>
}
