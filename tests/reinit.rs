//! Hot reloading. One test, because the store is global.
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use common::locale_dir;
use ezlz::{Error, t};

fn reinit(fallback: &str, dir: &std::path::Path) -> Result<(), Error> {
    // SAFETY: lookups in this test are far shorter than the cleanup delay.
    unsafe { ezlz::reinit(fallback, dir) }
}

#[test]
fn reinit_swaps_translations() {
    let v1 = locale_dir("reinit-v1", &[("en.yml", "k: \"v1 {x}\"\n")]);
    let v2 = locale_dir("reinit-v2", &[("en.yml", "k: \"v2 {x}\"\n"), ("de.yml", "k: \"de2 {x}\"\n")]);
    let v3 = locale_dir("reinit-v3", &[("en.yml", "k: \"v3 {x}\"\n")]);
    let v4 = locale_dir("reinit-v4", &[("en.yml", "k: \"v4 {x}\"\n")]);

    // On an uninitialized store `reinit` behaves like `init`...
    reinit("en", &v1).unwrap();
    assert_eq!(t!("en", k, x = 1), "v1 1");
    // ...after which `init` refuses to run.
    assert!(matches!(ezlz::init("en", &v2), Err(Error::AlreadyInitialized)));

    // Swapping replaces everything, including the set of locales.
    reinit("en", &v2).unwrap();
    assert_eq!(t!("en", k, x = 1), "v2 1");
    assert_eq!(t!("de", k, x = 1), "de2 1");
    reinit("en", &v1).unwrap();
    assert_eq!(t!("en", k, x = 1), "v1 1");
    assert_eq!(t!("de", k, x = 1), "v1 1", "`de` no longer exists and falls back to `en`");
    reinit("en", &v2).unwrap();

    // A failed reload keeps the old translations in place.
    let missing = std::path::Path::new("tests/does-not-exist");
    assert!(matches!(reinit("en", missing), Err(Error::Io { .. })));
    assert!(matches!(reinit("xx", &v2), Err(Error::FallbackLocaleNotFound { .. })));
    assert_eq!(t!("en", k, x = 1), "v2 1");

    // Readers hammering the store while it is replaced repeatedly must only
    // ever see complete, valid translations.
    ezlz::set_cleanup_delay(std::num::NonZero::new(1).unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let mut lookups = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let text = t!("en", k, x = 7);
                    assert!(
                        matches!(text.as_str(), "v2 7" | "v3 7" | "v4 7"),
                        "torn or foreign value: {text:?}"
                    );
                    lookups += 1;
                }
                lookups
            })
        })
        .collect();

    for i in 0..20 {
        reinit("en", if i % 2 == 0 { &v3 } else { &v4 }).unwrap();
    }
    stop.store(true, Ordering::Relaxed);

    let total: u64 = readers.into_iter().map(|r| r.join().unwrap()).sum();
    assert!(total > 0);
    assert_eq!(t!("en", k, x = 0), "v4 0");
}
