//! Localization loader.

use std::path::PathBuf;

pub mod yaml;
pub use yaml::*;

use crate::{Error, HashMap, HashMapExt, Template, Translations};

/// Attempts to build [`Translations`] using the `loader`.
pub(crate) fn load_with<L: Loader>(loader: L) -> Result<*mut Translations, Error> {
    Harness::<L>::new(loader).load()
}

/// Experimental unstable interface for custom localization loaders.
///
/// A loader should internally implement 2 iterators:
/// - The first one obtains the next locale data, puts it into the internal state
///   of the loader and yields the name of that locale.
/// - The second one yields the next key-value pair from the current locale.
///
/// [`Self::next_lang`] and [`Self::next_kv`] functions are the handles for those iterators.
/// The key-value handle is never called before the locale handle. After the key-value
/// handle returns `Ok(None)`, the harness calls the locale handle. If the latter
/// returns `Ok(None)`, the loader is considered exhausted. The harness verifies that
/// the fallback locale exists, and the loading succeeds. If an error is yielded
/// somewhere during the process, the loading fails with that error.
///
/// Missing fallback locale, duplicate keys and locales are validated
/// in the harness and should not be checked by loaders.
pub trait Loader {
    /// Get name of the fallback locale.
    ///
    /// Called after exhausting both locale and translation iterators
    /// to verify that the fallback locale is not missing.
    fn fallback(&self) -> Box<str>;

    /// Get path of the current locale. Used only for error messages.
    ///
    /// May only be called after [`Self::next_lang`] yields a locale.
    fn path(&self) -> PathBuf;

    /// Get the root path of the loader. Used only for [`Error::FallbackLocaleNotFound`].
    fn root(&self) -> PathBuf;

    /// Advance an internal iterator to the next locale, returning the name of that locale.
    ///
    /// If there is no more locales, return `Ok(None)`.
    /// If something goes wrong, return `Err`.
    fn next_lang(&mut self) -> Result<Option<String>, Error>;

    /// Advance an internal iterator to the next key-value pair of the current locale
    /// and return that pair as (key, value) tuple.
    ///
    /// If a key is built from a nested mapping, its path must use dots as separators.
    ///
    /// If there is no more translations in the locale, return `Ok(None)`.
    /// If something goes wrong, return `Err`.
    ///
    /// This handle is never called before [`Self::next_lang`].
    fn next_kv(&mut self) -> Result<Option<(String, String)>, Error>;
}

/// Runs a loader and validates its outputs.
pub(crate) struct Harness<L>
where
    L: Loader,
{
    frame: Translations,
    loader: L,
}

impl<L> Harness<L>
where
    L: Loader,
{
    /// Sets up a new harness for the loader instance.
    fn new(loader: L) -> Self {
        Self {
            frame: Translations {
                fallback: Box::from(""),
                locales: HashMap::new(),
            },
            loader,
        }
    }

    /// Executes the loading procedure.
    ///
    /// Iterates over each translation of each locale in the loader
    /// by calling [`Loader::next_lang`] for each locale and [`Loader::next_kv`]
    /// for each translation. Exits with an error on duplicate locale, duplicate key,
    /// missing fallback locale, or any error produced by the loader.
    fn load(mut self) -> Result<*mut Translations, Error> {
        while let Some(lang) = self.loader.next_lang()? {
            self.reserve_locale(&lang)?;

            while let Some((key, value)) = self.loader.next_kv()? {
                self.insert_template(&lang, key, value)?;
            }
        }

        let fallback = self.loader.fallback();

        if !self.frame.locales.contains_key(&fallback) {
            return Err(Error::FallbackLocaleNotFound {
                name: fallback.into_string(),
                path: self.loader.root(),
            });
        }

        self.frame.fallback = fallback;

        let ptr = Box::into_raw(Box::new(self.frame));
        Ok(ptr)
    }

    /// Reserves a locale name.
    ///
    /// Returns [`Error::DuplicateLocale`] if it was already used.
    fn reserve_locale(&mut self, lang: &str) -> Result<(), Error> {
        if self.frame.locales.insert(lang.into(), HashMap::new()).is_some() {
            return Err(Error::DuplicateLocale { name: lang.to_string() });
        }

        Ok(())
    }

    /// Attempts to parse a template and insert it into the current locale.
    ///
    /// Returns [`Error::InvalidTemplate`] if the template failed to parse or
    /// [`Error::DuplicateKey`] if its key was already used.
    fn insert_template(&mut self, lang: &str, key: String, value: String) -> Result<(), Error> {
        let Some(locale) = self.frame.locales.get_mut(lang) else {
            unreachable!("lang was reserved before calling this");
        };

        let template = match Template::try_from(&*value) {
            Err(error) =>
                return Err(Error::InvalidTemplate {
                    path: self.loader.path(),
                    key,
                    message: error.to_string(),
                }),
            Ok(template) => template,
        };

        if locale.insert((&*key).into(), template).is_some() {
            return Err(Error::DuplicateKey {
                path: self.loader.path(),
                key,
            });
        }

        Ok(())
    }
}
