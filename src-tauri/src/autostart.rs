//! Launch at login (#81), with no dependency: a LaunchAgent on macOS, the
//! `Run` key on Windows, an XDG autostart entry on Linux. The entries come
//! from `cortex_core::autostart`; this file only writes and removes them.

use cortex_core::autostart::{is_installed_path, launch_action, LaunchAction};

#[derive(serde::Serialize, Clone)]
pub struct AutostartStatus {
    pub enabled: bool,
    /// `false` for a copy that cannot start at login: a dev build, or an app
    /// still on a disk image.
    pub available: bool,
}

/// The executable the entry points at, when this copy is an installed app.
fn exe() -> Option<String> {
    if cfg!(debug_assertions) {
        return None;
    }
    let path = std::env::current_exe().ok()?.to_str()?.to_owned();
    // An AppImage runs from a temporary mount; the image file is the app.
    // `APPIMAGE` counts only from inside that mount: a variable inherited
    // from another AppImage would point the entry at another program.
    #[cfg(all(unix, not(target_os = "macos")))]
    if let (Some(image), Some(dir)) = (std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR")) {
        if !dir.is_empty() && std::path::Path::new(&path).starts_with(&dir) {
            return image.into_string().ok();
        }
    }
    is_installed_path(&path, cfg!(target_os = "macos")).then_some(path)
}

/// The entry for this copy, if it can have one.
fn entry_for_this_app() -> Option<String> {
    exe().and_then(|exe| platform::entry(&exe))
}

pub fn status() -> AutostartStatus {
    AutostartStatus {
        enabled: platform::is_enabled(),
        available: entry_for_this_app().is_some(),
    }
}

/// Write or remove the entry.
pub fn apply(enabled: bool) -> Result<(), String> {
    if !enabled {
        return platform::remove();
    }
    let entry = entry_for_this_app().ok_or("This copy of Cortex cannot open at login.")?;
    platform::write(&entry)
}

pub fn on_launch() {
    // Unreadable or invalid preferences are not a first launch: never turn
    // the entry on, nor save over the file, on their account.
    let Some(prefs) = crate::prefs::load_valid() else {
        return;
    };
    match launch_action(prefs.launch_at_login, platform::is_enabled()) {
        LaunchAction::TurnOn => {
            if apply(true).is_ok() {
                if let Err(err) = crate::prefs::update(|p| p.launch_at_login = Some(true)) {
                    eprintln!("cortex: {err}");
                }
            }
        }
        LaunchAction::Refresh => {
            if let Err(err) = apply(true) {
                eprintln!("cortex: launch at login not refreshed: {err}");
            }
        }
        LaunchAction::Nothing => {}
    }
}

#[tauri::command]
pub fn autostart_status() -> AutostartStatus {
    status()
}

#[tauri::command]
pub fn autostart_set(enabled: bool) -> Result<AutostartStatus, String> {
    apply(enabled)?;
    crate::prefs::update(|p| p.launch_at_login = Some(enabled))?;
    Ok(status())
}

#[cfg(unix)]
mod platform {
    use std::path::PathBuf;

    /// Must match `identifier` in tauri.conf.json.
    #[cfg(target_os = "macos")]
    const LABEL: &str = "io.github.viczyguy.cortex";

    #[cfg(target_os = "macos")]
    fn entry_path() -> Result<PathBuf, String> {
        Ok(crate::hooks::home()?
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{LABEL}.plist")))
    }

    #[cfg(target_os = "macos")]
    pub fn entry(exe: &str) -> Option<String> {
        Some(cortex_core::autostart::launch_agent_plist(LABEL, exe))
    }

    #[cfg(not(target_os = "macos"))]
    fn entry_path() -> Result<PathBuf, String> {
        let config = match std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => crate::hooks::home()?.join(".config"),
        };
        Ok(config.join("autostart").join("cortex.desktop"))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn entry(exe: &str) -> Option<String> {
        cortex_core::autostart::desktop_entry(exe)
    }

    pub fn is_enabled() -> bool {
        entry_path().is_ok_and(|p| p.exists())
    }

    pub fn write(content: &str) -> Result<(), String> {
        write_at(&entry_path()?, content)
    }

    pub fn remove() -> Result<(), String> {
        remove_at(&entry_path()?)
    }

    /// Nothing is written when the entry is already right.
    fn write_at(path: &std::path::Path, content: &str) -> Result<(), String> {
        if crate::files::read_bounded(path, 64 * 1024)
            .ok()
            .flatten()
            .as_deref()
            == Some(content)
        {
            return Ok(());
        }
        crate::files::write_private(path, content.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    fn remove_at(path: &std::path::Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_entry_is_written_once_and_removed_once() {
            let dir = std::env::temp_dir().join(format!("cortex-autostart-{}", std::process::id()));
            let path = dir.join("LaunchAgents").join("entry");
            write_at(&path, "one").unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
            let written = std::fs::metadata(&path).unwrap().modified().unwrap();
            write_at(&path, "one").unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().modified().unwrap(),
                written,
                "unchanged"
            );
            write_at(&path, "two").unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
            remove_at(&path).unwrap();
            assert!(!path.exists());
            remove_at(&path).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Cortex";

    pub fn is_enabled() -> bool {
        registry::exists(RUN_KEY, VALUE)
    }

    pub fn entry(exe: &str) -> Option<String> {
        cortex_core::autostart::run_command(exe)
    }

    pub fn write(command: &str) -> Result<(), String> {
        registry::set(RUN_KEY, VALUE, command)
    }

    pub fn remove() -> Result<(), String> {
        registry::remove(RUN_KEY, VALUE)
    }

    /// Three calls of the current user's registry, declared by hand like the
    /// pipe's security descriptor in `server.rs`: no crate for three functions.
    pub(super) mod registry {
        use super::c_void;

        /// `HKEY_CURRENT_USER`: `(HKEY)(ULONG_PTR)(LONG)0x80000001`, sign-extended.
        fn hkcu() -> *mut c_void {
            (0x8000_0001_u32 as i32 as isize) as *mut c_void
        }
        const REG_SZ: u32 = 1;
        const RRF_RT_REG_SZ: u32 = 0x2;
        const ERROR_FILE_NOT_FOUND: i32 = 2;

        #[link(name = "advapi32")]
        extern "system" {
            fn RegSetKeyValueW(
                key: *mut c_void,
                subkey: *const u16,
                value: *const u16,
                kind: u32,
                data: *const c_void,
                size: u32,
            ) -> i32;
            fn RegDeleteKeyValueW(key: *mut c_void, subkey: *const u16, value: *const u16) -> i32;
            #[cfg(test)]
            fn RegDeleteKeyW(key: *mut c_void, subkey: *const u16) -> i32;
            fn RegGetValueW(
                key: *mut c_void,
                subkey: *const u16,
                value: *const u16,
                flags: u32,
                kind: *mut u32,
                data: *mut c_void,
                size: *mut u32,
            ) -> i32;
        }

        fn wide(text: &str) -> Vec<u16> {
            text.encode_utf16().chain([0]).collect()
        }

        pub fn set(subkey: &str, value: &str, data: &str) -> Result<(), String> {
            let data = wide(data);
            // SAFETY: every pointer is to a NUL-terminated buffer alive for the
            // call, and `size` is the data's length in bytes, its NUL included.
            let code = unsafe {
                RegSetKeyValueW(
                    hkcu(),
                    wide(subkey).as_ptr(),
                    wide(value).as_ptr(),
                    REG_SZ,
                    data.as_ptr().cast(),
                    (data.len() * 2) as u32,
                )
            };
            (code == 0)
                .then_some(())
                .ok_or_else(|| format!("registry error {code}"))
        }

        pub fn remove(subkey: &str, value: &str) -> Result<(), String> {
            // SAFETY: NUL-terminated buffers alive for the call.
            let code =
                unsafe { RegDeleteKeyValueW(hkcu(), wide(subkey).as_ptr(), wide(value).as_ptr()) };
            match code {
                0 | ERROR_FILE_NOT_FOUND => Ok(()),
                _ => Err(format!("registry error {code}")),
            }
        }

        #[cfg(test)]
        pub fn delete_key(subkey: &str) {
            // SAFETY: a NUL-terminated buffer alive for the call.
            unsafe { RegDeleteKeyW(hkcu(), wide(subkey).as_ptr()) };
        }

        pub fn exists(subkey: &str, value: &str) -> bool {
            let mut size = 0u32;
            // SAFETY: NUL-terminated buffers alive for the call; a null data
            // pointer only asks for the size.
            unsafe {
                RegGetValueW(
                    hkcu(),
                    wide(subkey).as_ptr(),
                    wide(value).as_ptr(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                ) == 0
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::platform::registry;

    #[test]
    fn a_registry_value_is_set_found_and_removed() {
        let key = format!(r"Software\CortexTest{}", std::process::id());
        assert!(!registry::exists(&key, "Cortex"));
        registry::set(&key, "Cortex", r#""C:\x\Cortex.exe" --autostart"#).unwrap();
        assert!(registry::exists(&key, "Cortex"));
        registry::remove(&key, "Cortex").unwrap();
        assert!(!registry::exists(&key, "Cortex"));
        registry::remove(&key, "Cortex").unwrap();
        registry::delete_key(&key);
    }
}
