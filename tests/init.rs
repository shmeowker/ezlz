//! Successful initialization. This binary has exactly one test because `init`
//! can only succeed once per process.
mod common;

use std::sync::{Arc, Barrier};
use std::thread;

use common::locale_dir;
use ezlz::{Error, t};

#[test]
fn init_succeeds_exactly_once_even_when_racing() {
    let dir = locale_dir(
        "init-race",
        &[
            ("en.yml", "k: \"english {x}\"\n"),
            ("de.yaml", "k: \"deutsch {x}\"\n"),
            // Everything below must be ignored.
            ("notes.txt", "k: \"from notes\"\n"),
            ("README", "k: \"from readme\"\n"),
            ("noext.", "k: \"from noext\"\n"),
        ],
    );
    // A directory with a YAML-like name must be ignored too.
    std::fs::create_dir(dir.join("sub.yml")).unwrap();

    const THREADS: usize = 16;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let dir = dir.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                ezlz::init("en", &dir)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "exactly one init must win: {results:?}");
    assert!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| matches!(e, Error::AlreadyInitialized)),
        "all losers must report AlreadyInitialized: {results:?}"
    );

    // A later call fails too, and must not disturb the stored translations.
    let err = ezlz::init("en", &dir).unwrap_err();
    assert!(matches!(err, Error::AlreadyInitialized), "{err:?}");
    assert!(err.to_string().contains("init"), "{err}");

    // Both `.yml` and `.yaml` are loaded, the locale name is the file stem.
    assert_eq!(t!("en", k, x = 1), "english 1");
    assert_eq!(t!("de", k, x = 2), "deutsch 2");

    // Non-YAML files and directories are not locales: these fall back to `en`.
    assert_eq!(t!("notes", k, x = 3), "english 3");
    assert_eq!(t!("README", k, x = 3), "english 3");
    assert_eq!(t!("sub", k, x = 3), "english 3");
}
