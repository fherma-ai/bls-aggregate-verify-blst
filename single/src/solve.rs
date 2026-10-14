//! A slot of attestation aggregates, verified one at a time.
//!
//! The baseline every client has: for each aggregate, add up the committee's
//! public keys and check one pairing equation,
//!
//!     e(Σ pk, msg) · e(−G1, sig) = 1,
//!
//! through blst's own pairing context — `raw_aggregate` twice, `commit`,
//! `finalverify`. One final exponentiation per aggregate, which is what the
//! optimistic answer beside this one tries to avoid.
//!
//! The validator set is a parameter of the point, so converting it into blst's
//! form is `init`'s work and is not measured: a set is fixed for an epoch and
//! a client has had it for hours before the slot it verifies.
use blst::{
    blst_fp, blst_fp2, blst_fp_from_uint64, blst_p1, blst_p1_add_or_double_affine,
    blst_p1_affine, blst_p1_cneg, blst_p1_from_affine, blst_p1_to_affine, blst_p2_affine,
    blst_pairing_commit, blst_pairing_finalverify, blst_pairing_init, blst_pairing_raw_aggregate,
    blst_pairing_sizeof, blst_scalar, limb_t, BLST_ERROR,
};

use crate::fherma::{Inputs, Outputs, Point, Tensor};

/// The standard G1 generator, which the right-hand side of the equation
/// pairs the signature against. blst carries it as a constant.
fn minus_g1() -> blst_p1_affine {
    let mut generator = blst_p1::default();
    unsafe {
        blst_p1_from_affine(&mut generator, &blst::BLS12_381_G1);
        // The equation is checked as a product that must be one, so the
        // right-hand term enters negated.
        blst_p1_cneg(&mut generator, true);
    }
    let mut affine = blst_p1_affine::default();
    unsafe { blst_p1_to_affine(&mut affine, &generator) };
    affine
}

fn fp(data: &[u64], at: usize, limbs: usize) -> blst_fp {
    let mut words = [0u64; 6];
    words[..limbs].copy_from_slice(&data[at * limbs..(at + 1) * limbs]);
    let mut out = blst_fp::default();
    unsafe { blst_fp_from_uint64(&mut out, words.as_ptr()) };
    out
}

/// The `i`-th G2 point of a tensor laid out as M × 2 × L: (c0, c1) of x, then
/// (c0, c1) of y.
fn g2(xs: &[u64], ys: &[u64], i: usize, limbs: usize) -> blst_p2_affine {
    blst_p2_affine {
        x: blst_fp2 { fp: [fp(xs, 2 * i, limbs), fp(xs, 2 * i + 1, limbs)] },
        y: blst_fp2 { fp: [fp(ys, 2 * i, limbs), fp(ys, 2 * i + 1, limbs)] },
    }
}

/// A pairing context, sized by the library.
fn context() -> Vec<limb_t> {
    let bytes = unsafe { blst_pairing_sizeof() };
    vec![0 as limb_t; bytes / core::mem::size_of::<limb_t>() + 1]
}

pub struct State {
    limbs: usize,
    aggregates: usize,
    committee_size: usize,
    /// The validator set in blst's form, prepared outside the measurement.
    keys: Vec<blst_p1_affine>,
    minus_generator: blst_p1_affine,
}

impl Default for State {
    fn default() -> Self {
        State {
            limbs: 6,
            aggregates: 0,
            committee_size: 0,
            keys: Vec::new(),
            minus_generator: blst_p1_affine::default(),
        }
    }
}

pub fn init(p: &Point) -> State {
    let limbs = p.L as usize;
    let keys = (0..p.V as usize)
        .map(|i| blst_p1_affine { x: fp(&p.keys_x.data, i, limbs), y: fp(&p.keys_y.data, i, limbs) })
        .collect();
    State {
        limbs,
        aggregates: p.M as usize,
        committee_size: p.K as usize,
        keys,
        minus_generator: minus_g1(),
    }
}

/// The sum of a committee's public keys.
fn committee_key(state: &State, committee: &[u32]) -> blst_p1_affine {
    let mut sum = blst_p1::default();
    unsafe { blst_p1_from_affine(&mut sum, &state.keys[committee[0] as usize]) };
    for who in &committee[1..] {
        unsafe { blst_p1_add_or_double_affine(&mut sum, &sum, &state.keys[*who as usize]) };
    }
    let mut affine = blst_p1_affine::default();
    unsafe { blst_p1_to_affine(&mut affine, &sum) };
    affine
}

pub fn run(state: &mut State, inp: &Inputs) -> Outputs {
    let limbs = state.limbs;
    let k = state.committee_size;
    let mut ok = Vec::with_capacity(state.aggregates);

    for at in 0..state.aggregates {
        let committee = &inp.committee.data[at * k..(at + 1) * k];
        let key = committee_key(state, committee);
        let msg = g2(&inp.msg_x.data, &inp.msg_y.data, at, limbs);
        let sig = g2(&inp.sig_x.data, &inp.sig_y.data, at, limbs);

        let mut ctx = context();
        let pairing = ctx.as_mut_ptr() as *mut blst::blst_pairing;
        let verified = unsafe {
            blst_pairing_init(pairing, false, core::ptr::null(), 0);
            blst_pairing_raw_aggregate(pairing, &msg, &key);
            blst_pairing_raw_aggregate(pairing, &sig, &state.minus_generator);
            blst_pairing_commit(pairing);
            blst_pairing_finalverify(pairing, core::ptr::null())
        };
        ok.push(u8::from(verified));
    }

    // `blst_scalar` and the error type are named so that a reader sees what
    // this answer does not use: no randomisation, no batching, one
    // exponentiation per aggregate.
    let _ = (blst_scalar::default(), BLST_ERROR::BLST_SUCCESS);

    Outputs { ok: Tensor { shape: vec![state.aggregates as i64], data: ok } }
}

pub fn free(state: State) {
    drop(state);
}
