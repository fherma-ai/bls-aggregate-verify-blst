// GENERATED for bls_aggregate_verify/bls12381@1.0.0. Do not edit — `--update` rewrites it.
//
//     ./fherma-solution <point directory>
//
// Reads the directory a bundle prepared, answers every case in it, and writes
// what each one cost.
//
//     manifest.json     the point, and how many cases      read
//     cases/000000/     one file per argument              read
//     out/000000/       one file per result                written
//     out/results.json  what each case cost                written
//
// Only the call to run() is timed. Reading and writing are outside the window,
// which is also why seeing the inputs early is not a way to answer early:
// nothing of yours runs during the read.
mod fherma;
mod solve;

use fherma::{Inputs, Outputs, Point, Tensor};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Fallible = Result<(), Box<dyn std::error::Error>>;

/// Little-endian on the wire, and on every platform this runs on, so the copy
/// is the decode.
trait Wire: Sized + Copy + Default {
    fn from_wire(bytes: &[u8]) -> Self;
    fn to_wire(self) -> Vec<u8>;
}

macro_rules! wire {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            fn from_wire(bytes: &[u8]) -> Self {
                Self::from_le_bytes(bytes.try_into().expect("width"))
            }
            fn to_wire(self) -> Vec<u8> { self.to_le_bytes().to_vec() }
        }
    )*};
}
wire!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

/// A reader for our own descriptor and nothing else: a number under a key, or
/// a shape array under a name. The schema is fixed and is not going to grow.
fn after(text: &str, key: &str, from: usize) -> Option<usize> {
    let quoted = format!(r#""{key}""#);
    let at = text[from..].find(&quoted)? + from;
    let colon = text[at..].find(':')? + at;
    Some(colon + 1)
}

fn number(text: &str, key: &str) -> Option<f64> {
    let at = after(text, key, 0)?;
    let rest = text[at..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.' && c != 'e')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn read<T: Wire>(
    dir: &Path,
    name: &str,
    shape: Vec<i64>,
) -> Result<Tensor<T>, Box<dyn std::error::Error>> {
    let raw = fs::read(dir.join(format!("{name}.bin")))?;
    let width = std::mem::size_of::<T>();
    let count: usize = shape.iter().product::<i64>() as usize;
    if raw.len() != count * width {
        return Err(format!("{name}: {} bytes for {count} values", raw.len()).into());
    }

    let data = raw.chunks_exact(width).map(T::from_wire).collect();
    Ok(Tensor { shape, data })
}

fn write<T: Wire>(dir: &Path, name: &str, tensor: &Tensor<T>) -> Fallible {
    let mut bytes = Vec::with_capacity(tensor.data.len() * std::mem::size_of::<T>());
    for value in &tensor.data {
        bytes.extend_from_slice(&value.to_wire());
    }
    fs::write(dir.join(format!("{name}.bin")), bytes)?;
    Ok(())
}

fn numbered(i: usize) -> String {
    format!("{i:06}")
}

/// Written after every case, not at the end: a process killed on its timeout
/// has still done the cases before it, and a file written once at the end
/// would throw them away. A panic ends the process the same way, which is why
/// nothing here tries to catch one.
fn report(out: &Path, init_s: f64, cases: &[String]) -> Fallible {
    let body = cases.join(",");
    fs::write(
        out.join("results.json"),
        format!("{{\"init_s\":{init_s:.9},\"cases\":[{body}]}}"),
    )?;
    Ok(())
}

fn ok(i: usize, seconds: f64) -> String {
    format!("{{\"i\":{i},\"seconds\":{seconds:.9},\"status\":\"ok\"}}")
}

fn crashed(i: usize, why: &str) -> String {
    let mut clean = String::new();
    for c in why.chars().take(200) {
        match c {
            '"' | '\\' => {
                clean.push('\\');
                clean.push(c);
            }
            '\n' => clean.push(' '),
            _ => clean.push(c),
        }
    }
    format!("{{\"i\":{i},\"seconds\":null,\"status\":\"crashed\",\"note\":\"{clean}\"}}")
}

fn main() -> Fallible {
    let root = match std::env::args().nth(1) {
        Some(one) => PathBuf::from(one),
        None => {
            eprintln!("usage: fherma-solution <point directory>");
            std::process::exit(2);
        }
    };

    let text = fs::read_to_string(root.join("manifest.json"))?;
    let point_dir = root.join("point");
    let M: u32 = number(&text, "M").unwrap_or(0.0) as u32;
    let K: u32 = number(&text, "K").unwrap_or(0.0) as u32;
    let L: u32 = number(&text, "L").unwrap_or(0.0) as u32;
    let V: u32 = number(&text, "V").unwrap_or(0.0) as u32;
    let F: u32 = number(&text, "F").unwrap_or(0.0) as u32;
    let keys_x = read::<u64>(&point_dir, "keys_x", vec![V as i64, L as i64])?;
    let keys_y = read::<u64>(&point_dir, "keys_y", vec![V as i64, L as i64])?;
    let point = Point { M, K, L, V, F, keys_x, keys_y };
    let total = number(&text, "cases").unwrap_or(0.0) as usize;

    let answers_root = root.join("out");
    fs::create_dir_all(&answers_root)?;

    let setup = Instant::now();
    let mut state = solve::init(&point);
    let init_s = setup.elapsed().as_secs_f64();

    let mut cases: Vec<String> = Vec::new();
    report(&answers_root, init_s, &cases)?;

    for i in 0..total {
        let dir = root.join("cases").join(numbered(i));
        let answers = answers_root.join(numbered(i));

        let mut inp = Inputs::default();
        if let Err(why) = (|inp: &mut Inputs| -> Fallible {
            inp.committee = read(&dir, "committee", vec![point.M as i64, point.K as i64])?;
            inp.msg_x = read(&dir, "msg_x", vec![point.M as i64, 2i64, point.L as i64])?;
            inp.msg_y = read(&dir, "msg_y", vec![point.M as i64, 2i64, point.L as i64])?;
            inp.sig_x = read(&dir, "sig_x", vec![point.M as i64, 2i64, point.L as i64])?;
            inp.sig_y = read(&dir, "sig_y", vec![point.M as i64, 2i64, point.L as i64])?;
            Ok(())
        })(&mut inp)
        {
            cases.push(crashed(i, &format!("reading the case: {why}")));
            report(&answers_root, init_s, &cases)?;
            continue;
        }

        // Monotonic, and around the call and nothing else.
        let started = Instant::now();
        let answer: Outputs = solve::run(&mut state, &inp);
        let seconds = started.elapsed().as_secs_f64();

        if let Err(why) = (|| -> Fallible {
            fs::create_dir_all(&answers)?;
            write(&answers, "ok", &answer.ok)?;
            Ok(())
        })()
        {
            cases.push(crashed(i, &format!("writing the answer: {why}")));
            report(&answers_root, init_s, &cases)?;
            continue;
        }

        cases.push(ok(i, seconds));
        report(&answers_root, init_s, &cases)?;
    }

    solve::free(state);
    report(&answers_root, init_s, &cases)?;
    Ok(())
}
