//! The language of every text (#154): the one picked in Settings, else the
//! system's. The webviews translate their own texts; Rust only draws the
//! menu-bar item and a window title.

use cortex_core::prefs::{language, Prefs};

/// The language shown now: "en" or "pt".
pub fn current(prefs: &Prefs) -> &'static str {
    language(prefs, &system_locale())
}

/// The Settings window's title.
pub fn settings_title(lang: &str) -> &'static str {
    if lang == "pt" {
        "Ajustes do Cortex"
    } else {
        "Cortex Settings"
    }
}

/// The menu-bar item's two entries (macOS).
#[cfg(target_os = "macos")]
pub fn menu_texts(lang: &str) -> (&'static str, &'static str) {
    if lang == "pt" {
        ("Ajustes…", "Encerrar o Cortex")
    } else {
        ("Settings…", "Quit Cortex")
    }
}

/// The first of the user's preferred languages, as macOS lists them
/// ("pt-BR"); empty when it cannot be read.
#[cfg(target_os = "macos")]
fn system_locale() -> String {
    use std::ffi::{c_char, c_void, CStr};
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> *const c_void;
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(cf: *const c_void);
    }
    const UTF8: u32 = 0x0800_0100;
    let mut buffer = [0 as c_char; 64];
    // SAFETY: the array comes from a Copy function and is released once; its
    // first value is borrowed from it while it lives; the buffer is ours, with
    // its size, and CFStringGetCString writes a terminated string or fails.
    unsafe {
        let languages = CFLocaleCopyPreferredLanguages();
        if languages.is_null() {
            return String::new();
        }
        let mut locale = String::new();
        if CFArrayGetCount(languages) > 0 {
            let first = CFArrayGetValueAtIndex(languages, 0);
            if !first.is_null()
                && CFStringGetCString(first, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0
            {
                locale = CStr::from_ptr(buffer.as_ptr())
                    .to_string_lossy()
                    .into_owned();
            }
        }
        CFRelease(languages);
        locale
    }
}

/// Windows' display language.
#[cfg(windows)]
fn system_locale() -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: no arguments, a plain query.
    let id = unsafe { GetUserDefaultUILanguage() };
    cortex_core::prefs::locale_of_langid(id).into()
}

/// The locale the desktop session runs in.
#[cfg(not(any(target_os = "macos", windows)))]
fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}
