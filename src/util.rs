use rand::distributions::Alphanumeric;
use rand::{self, Rng};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::{env, io, str};

use crate::error::IoResultExt;

pub fn temp_root() -> PathBuf {
    if cfg!(target_os = "wasi") {
        env::var_os("TMPDIR")
            .expect("expected TMPDIR to be set to a root directory for temporary files")
            .into()
    } else {
        env::temp_dir()
    }
}

fn tmpname(prefix: &OsStr, suffix: &OsStr, rand_len: usize) -> OsString {
    let mut buf = OsString::with_capacity(prefix.len() + suffix.len() + rand_len);
    buf.push(prefix);

    // Push each character in one-by-one. Unfortunately, this is the only
    // safe(ish) simple way to do this without allocating a temporary
    // String/Vec.
    unsafe {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(rand_len)
            .for_each(|b| buf.push(str::from_utf8_unchecked(&[b as u8])))
    }
    buf.push(suffix);
    buf
}

pub fn create_helper<F, R>(
    base: &Path,
    prefix: &OsStr,
    suffix: &OsStr,
    random_len: usize,
    f: F,
) -> io::Result<R>
where
    F: Fn(PathBuf) -> io::Result<R>,
{
    let num_retries = if random_len != 0 {
        crate::NUM_RETRIES
    } else {
        1
    };

    for _ in 0..num_retries {
        let path = base.join(tmpname(prefix, suffix, random_len));
        return match f(path) {
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            res => res,
        };
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many temporary files exist",
    ))
    .with_err_path(|| base)
}
