//! Fixture trees written by the test that reads them.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub struct Tree {
    pub root: PathBuf,
}

impl Tree {
    /// An empty tree. `name` must be unique across the test binaries that share the directory.
    pub fn new(name: &str) -> Tree {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("clearing {}: {error}", root.display()),
        }
        fs::create_dir_all(&root).expect("create the root");
        Tree { root }
    }

    pub fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("create the directory");
        fs::write(path, contents).expect("write");
    }
}
