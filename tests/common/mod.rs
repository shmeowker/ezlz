#![allow(dead_code)]
//! Helpers shared by the integration tests.

use std::fs;
use std::path::PathBuf;
use std::sync::Once;

/// Initializes ezlz once per test binary from `tests/locales` with `test` as the fallback locale.
pub fn init_test_locales() {
    static INIT: Once = Once::new();
    INIT.call_once(|| ezlz::init("test", "tests/locales").unwrap());
}

/// Creates a fresh directory under the target directory containing the given
/// `(file name, content)` pairs, and returns its path.
pub fn locale_dir(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ezlz-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (file, content) in files {
        fs::write(dir.join(file), content).unwrap();
    }
    dir
}
