//! A slot of attestation aggregates, verified optimistically.
//!
//! What a client does, and why. A healthy slot has no bad aggregate in it, so
//! checking each one separately pays M final exponentiations for an outcome
//! that is almost always "all of them". This answer checks the whole slot at
//! once — one pairing context, 2M terms, one final exponentiation — and only
//! when that fails does it go back and check them singly to say which.
//!
//! The batch is randomised, which is not decoration. Without it a product of
//! M equations being one does not imply each is one, and two faults that
//! cancel would pass; with a scalar r per aggregate,
//!
//!     Π e(r·Σpk, msg) · e(−r·G1, sig) = 1
//!
//! holds with overwhelming probability only when every term does. The price
//! is two G1 multiplications per aggregate, which is what the measurement
//! against the single-check answer is for: at F = 0 it buys M − 1 final
//! exponentiations, and at F > 0 it is spent and the single checks are paid
//! anyway.
//!
//! The scalars come from a seed drawn once in `init` — not from the case, so
//! a generator cannot aim at them, and not from the clock per case, so the
//! measurement does not include a syscall.
use blst::{
    blst_fp, blst_fp2, blst_fp_from_uint64, blst_p1, blst_p1_add_or_double_affine,
    blst_p1_affine, blst_p1_cneg, blst_p1_from_affine, blst_p1_mult, blst_p1_to_affine,
    blst_p2_affine, blst_pairing_commit, blst_pairing_finalverify, blst_pairing_init,
    blst_pairing_raw_aggregate, blst_pairing_sizeof, limb_t,
};

use crate::fherma::{Inputs, Outputs, Point, Tensor};

/// Bits of the scalars the batch is randomised with. 64 is the usual choice:
/// the chance a cancellation survives is 2^-64 per aggregate, and a wider
/// scalar costs more than the risk it removes.
const RANDOM_BITS: usize = 64;

fn fp(data: &[u64], at: usize, limbs: usize) -> blst_fp {
    let mut words = [0u64; 6];
    words[..limbs].copy_from_slice(&data[at * limbs..(at + 1) * limbs]);
    let mut out = blst_fp::default();
    unsafe { blst_fp_from_uint64(&mut out, words.as_ptr()) };
    out
}

fn g2(xs: &[u64], ys: &[u64], i: usize, limbs: usize) -> blst_p2_affine {
    blst_p2_affine {
        x: blst_fp2 { fp: [fp(xs, 2 * i, limbs), fp(xs, 2 * i + 1, limbs)] },
        y: blst_fp2 { fp: [fp(ys, 2 * i, limbs), fp(ys, 2 * i + 1, limbs)] },
    }
}

fn context() -> Vec<limb_t> {
    let bytes = unsafe { blst_pairing_sizeof() };
    vec![0 as limb_t; bytes / core::mem::size_of::<limb_t>() + 1]
}

/// splitmix64: a scalar per aggregate from one seed, without a syscall in the
/// measured call.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

pub struct State {
    limbs: usize,
    aggregates: usize,
    committee_size: usize,
    keys: Vec<blst_p1_affine>,
    /// −G1, the right-hand term of the equation before it is scaled.
    minus_generator: blst_p1,
    seed: u64,
}

impl Default for State {
    fn default() -> Self {
        State {
            limbs: 6,
            aggregates: 0,
            committee_size: 0,
            keys: Vec::new(),
            minus_generator: blst_p1::default(),
            seed: 0,
        }
    }
}

pub fn init(p: &Point) -> State {
    let limbs = p.L as usize;
    let keys = (0..p.V as usize)
        .map(|i| blst_p1_affine { x: fp(&p.keys_x.data, i, limbs), y: fp(&p.keys_y.data, i, limbs) })
        .collect();

    let mut minus_generator = blst_p1::default();
    unsafe {
        blst_p1_from_affine(&mut minus_generator, &blst::BLS12_381_G1);
        blst_p1_cneg(&mut minus_generator, true);
    }

    // One draw, outside the measurement. The address of a fresh allocation is
    // enough entropy for what this is: scalars a case cannot be built around.
    let anchor = Box::new(0u8);
    let seed = (&*anchor as *const u8 as u64) ^ 0x5DEE_CE66_D000_0005;

    State {
        limbs,
        aggregates: p.M as usize,
        committee_size: p.K as usize,
        keys,
        minus_generator,
        seed,
    }
}

fn committee_key(state: &State, committee: &[u32]) -> blst_p1 {
    let mut sum = blst_p1::default();
    unsafe { blst_p1_from_affine(&mut sum, &state.keys[committee[0] as usize]) };
    for who in &committee[1..] {
        unsafe { blst_p1_add_or_double_affine(&mut sum, &sum, &state.keys[*who as usize]) };
    }
    sum
}

fn affine(point: &blst_p1) -> blst_p1_affine {
    let mut out = blst_p1_affine::default();
    unsafe { blst_p1_to_affine(&mut out, point) };
    out
}

/// `scalar` times a point of G1, affine out.
fn scaled(point: &blst_p1, scalar: u64) -> blst_p1_affine {
    let mut out = blst_p1::default();
    let bytes = scalar.to_le_bytes();
    unsafe { blst_p1_mult(&mut out, point, bytes.as_ptr(), RANDOM_BITS) };
    affine(&out)
}

/// One aggregate, checked on its own: the fall back, and the whole of the
/// other answer.
fn verify_one(state: &State, inp: &Inputs, at: usize) -> bool {
    let limbs = state.limbs;
    let k = state.committee_size;
    let committee = &inp.committee.data[at * k..(at + 1) * k];
    let key = affine(&committee_key(state, committee));
    let msg = g2(&inp.msg_x.data, &inp.msg_y.data, at, limbs);
    let sig = g2(&inp.sig_x.data, &inp.sig_y.data, at, limbs);
    let minus_generator = affine(&state.minus_generator);

    let mut ctx = context();
    let pairing = ctx.as_mut_ptr() as *mut blst::blst_pairing;
    unsafe {
        blst_pairing_init(pairing, false, core::ptr::null(), 0);
        blst_pairing_raw_aggregate(pairing, &msg, &key);
        blst_pairing_raw_aggregate(pairing, &sig, &minus_generator);
        blst_pairing_commit(pairing);
        blst_pairing_finalverify(pairing, core::ptr::null())
    }
}

pub fn run(state: &mut State, inp: &Inputs) -> Outputs {
    let limbs = state.limbs;
    let k = state.committee_size;
    let m = state.aggregates;

    // The whole slot into one context, each aggregate scaled by its own
    // scalar.
    let mut ctx = context();
    let pairing = ctx.as_mut_ptr() as *mut blst::blst_pairing;
    unsafe { blst_pairing_init(pairing, false, core::ptr::null(), 0) };

    let mut random = state.seed;
    for at in 0..m {
        let r = splitmix(&mut random);
        let committee = &inp.committee.data[at * k..(at + 1) * k];
        let key = scaled(&committee_key(state, committee), r);
        let generator = scaled(&state.minus_generator, r);
        let msg = g2(&inp.msg_x.data, &inp.msg_y.data, at, limbs);
        let sig = g2(&inp.sig_x.data, &inp.sig_y.data, at, limbs);
        unsafe {
            blst_pairing_raw_aggregate(pairing, &msg, &key);
            blst_pairing_raw_aggregate(pairing, &sig, &generator);
        }
    }

    let all = unsafe {
        blst_pairing_commit(pairing);
        blst_pairing_finalverify(pairing, core::ptr::null())
    };

    // The optimism paid off, and one exponentiation settled the slot.
    if all {
        return Outputs { ok: Tensor { shape: vec![m as i64], data: vec![1u8; m] } };
    }

    // It did not, and the slot is checked again one aggregate at a time —
    // which is the cost the pair of points exists to show.
    let ok = (0..m).map(|at| u8::from(verify_one(state, inp, at))).collect();
    Outputs { ok: Tensor { shape: vec![m as i64], data: ok } }
}

pub fn free(state: State) {
    drop(state);
}
