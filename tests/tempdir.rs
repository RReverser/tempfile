// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(rust_2018_idioms)]

use std::env;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;

use tempfile::{Builder, TempDir};

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(n) => n,
            Err(e) => panic!("error: {}", e),
        }
    };
}

trait PathExt {
    fn exists(&self) -> bool;
    fn is_dir(&self) -> bool;
}

impl PathExt for Path {
    fn exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }
    fn is_dir(&self) -> bool {
        fs::metadata(self).map(|m| m.is_dir()).unwrap_or(false)
    }
}

#[test]
fn test_customnamed() {
    let tmpfile = Builder::new()
        .prefix("prefix")
        .suffix("suffix")
        .rand_bytes(12)
        .tempdir()
        .unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("prefix"));
    assert!(name.ends_with("suffix"));
    assert_eq!(name.len(), 24);
}

fn spawn<T: 'static + Send>(f: impl 'static + FnOnce() -> T + Send) -> std::thread::Result<T> {
    if cfg!(target_os = "wasi") {
        // On WASI we can't spawn threads, so just execute the function directly.
        Ok(f())
    } else {
        // On other platforms, spawn a separate thread to reliably catch panics.
        std::thread::spawn(f).join()
    }
}

#[cfg(not(target_os = "wasi"))]
use panic as panic_if_catchable;

#[cfg(target_os = "wasi")]
macro_rules! panic_if_catchable {
    ($($arg:tt)*) => {};
}

macro_rules! test_in_tmpdir {
    ($($(# $attr:tt)* fn $func:ident() $body:expr)*) => {
        $($(# $attr)* #[test]
        fn $func() {
            let tmpdir = t!(TempDir::new());
            assert!(env::set_current_dir(tmpdir.path()).is_ok());

            $body
        })*
    };
}

test_in_tmpdir! {
    fn test_tempdir() {
        let path = {
            let p = t!(Builder::new().prefix("foobar").tempdir_in(&Path::new(".")));
            let p = p.path();
            assert!(p.to_str().unwrap().contains("foobar"));
            p.to_path_buf()
        };
        assert!(!path.exists());
    }

    fn test_rm_tempdir() {
        let (tx, rx) = channel();
        let f = move || {
            let tmp = t!(TempDir::new());
            tx.send(tmp.path().to_path_buf()).unwrap();
            panic_if_catchable!("panic to unwind past `tmp`");
        };
        let _ = spawn(f);
        let path = rx.recv().unwrap();
        assert!(!path.exists());

        let tmp = t!(TempDir::new());
        let path = tmp.path().to_path_buf();
        let f = move || {
            let _tmp = tmp;
            panic_if_catchable!("panic to unwind past `tmp`");
        };
        let _ = spawn(f);
        assert!(!path.exists());

        let path;
        {
            let f = move || t!(TempDir::new());

            let tmp = spawn(f).unwrap();
            path = tmp.path().to_path_buf();
            assert!(path.exists());
            drop(tmp);
        }
        assert!(!path.exists());

        let path;
        {
            let tmp = t!(TempDir::new());
            path = tmp.into_path();
        }
        assert!(path.exists());
        t!(fs::remove_dir_all(&path));
        assert!(!path.exists());
    }

    fn test_rm_tempdir_close() {
        let (tx, rx) = channel();
        let f = move || {
            let tmp = t!(TempDir::new());
            tx.send(tmp.path().to_path_buf()).unwrap();
            t!(tmp.close());
            panic_if_catchable!("panic when unwinding past `tmp`");
        };
        let _ = spawn(f);
        let path = rx.recv().unwrap();
        assert!(!path.exists());

        let tmp = t!(TempDir::new());
        let path = tmp.path().to_path_buf();
        let f = move || {
            let tmp = tmp;
            t!(tmp.close());
            panic_if_catchable!("panic when unwinding past `tmp`");
        };
        let _ = spawn(f);
        assert!(!path.exists());

        let path;
        {
            let f = move || t!(TempDir::new());

            let tmp = spawn(f).unwrap();
            path = tmp.path().to_path_buf();
            assert!(path.exists());
            t!(tmp.close());
        }
        assert!(!path.exists());

        let path;
        {
            let tmp = t!(TempDir::new());
            path = tmp.into_path();
        }
        assert!(path.exists());
        t!(fs::remove_dir_all(&path));
        assert!(!path.exists());
    }

    // Ideally these would be in std::os but then core would need
    // to depend on std
    fn recursive_mkdir_rel() {
        let path = Path::new("frob");
        let cwd = env::current_dir().unwrap();
        println!(
            "recursive_mkdir_rel: Making: {} in cwd {} [{}]",
            path.display(),
            cwd.display(),
            path.exists()
        );
        t!(fs::create_dir(&path));
        assert!(path.is_dir());
        t!(fs::create_dir_all(&path));
        assert!(path.is_dir());
    }

    fn recursive_mkdir_dot() {
        let dot = Path::new(".");
        t!(fs::create_dir_all(&dot));
        let dotdot = Path::new("..");
        t!(fs::create_dir_all(&dotdot));
    }

    fn recursive_mkdir_rel_2() {
        let path = Path::new("./frob/baz");
        let cwd = env::current_dir().unwrap();
        println!(
            "recursive_mkdir_rel_2: Making: {} in cwd {} [{}]",
            path.display(),
            cwd.display(),
            path.exists()
        );
        t!(fs::create_dir_all(&path));
        assert!(path.is_dir());
        assert!(path.parent().unwrap().is_dir());
        let path2 = Path::new("quux/blat");
        println!(
            "recursive_mkdir_rel_2: Making: {} in cwd {}",
            path2.display(),
            cwd.display()
        );
        t!(fs::create_dir("quux"));
        t!(fs::create_dir_all(&path2));
        assert!(path2.is_dir());
        assert!(path2.parent().unwrap().is_dir());
    }

    // Ideally this would be in core, but needs TempFile
    fn test_remove_dir_all_ok() {
        let tmpdir = t!(TempDir::new());
        let tmpdir = tmpdir.path();
        let root = tmpdir.join("foo");

        println!("making {}", root.display());
        t!(fs::create_dir(&root));
        t!(fs::create_dir(&root.join("foo")));
        t!(fs::create_dir(&root.join("foo").join("bar")));
        t!(fs::create_dir(&root.join("foo").join("bar").join("blat")));
        t!(fs::remove_dir_all(&root));
        assert!(!root.exists());
        assert!(!root.join("bar").exists());
        assert!(!root.join("bar").join("blat").exists());
    }

    #[should_panic]
    fn dont_double_panic() {
        let tmpdir = TempDir::new().unwrap();
        // Remove the temporary directory so that TempDir sees
        // an error on drop
        t!(fs::remove_dir(tmpdir.path()));
        // Panic. If TempDir panics *again* due to the rmdir
        // error then the process will abort.
        panic!();
    }

    fn pass_as_asref_path() {
        let tempdir = t!(TempDir::new());
        takes_asref_path(&tempdir);

        fn takes_asref_path<T: AsRef<Path>>(path: T) {
            let path = path.as_ref();
            assert!(path.exists());
        }
    }
}
