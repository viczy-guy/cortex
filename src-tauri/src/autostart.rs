//! Launch at login (#81), with no dependency: a LaunchAgent on macOS, the
//! `Run` key on Windows, an XDG autostart entry on Linux. The entries come
//! from `cortex_core::autostart`; this file only writes and removes them.

#[derive(serde::Serialize, Clone)]
pub struct AutostartStatus {
    pub enabled: bool,
    /// `false` for a copy that cannot start at login: a dev build, or an app
    /// still on a disk image.
    pub available: bool,
}

/// The executable the entry points at.
fn exe() -> Option<String> {
    if cfg!(debug_assertions) {
        return None;
    }
    // An AppImage runs from a temporary mount; the file itself is the app.
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(image) = std::env::var_os("APPIMAGE") {
        return image.into_string().ok();
    }
    let path = std::env::current_exe().ok()?.to_str()?.to_owned();
    #[cfg(target_os = "macos")]
    if path.starts_with("/Volumes/") || path.contains("/AppTranslocation/") {
        return None;
    }
    Some(path)
}

pub fn status() -> AutostartStatus {
    AutostartStatus {
        enabled: platform::is_enabled(),
        available: exe().is_some(),
    }
}

/// Write or remove the entry. Writing again is how the entry follows an app
/// that moved; nothing is written when it is already right.
pub fn apply(enabled: bool) -> Result<(), String> {
    if !enabled {
        return platform::remove();
    }
    let exe = exe().ok_or("This copy of Cortex cannot open at login.")?;
    platform::write(&exe)
}

/// On from the first launch (decided for 0.3.0); afterwards the user's
/// choice in Settings stands.
pub fn on_launch() {
    let prefs = crate::prefs::load();
    match prefs.launch_at_login {
        Some(false) => {}
        Some(true) => {
            if let Err(err) = apply(true) {
                eprintln!("cortex: launch at login not refreshed: {err}");
            }
        }
        None => {
            if apply(true).is_ok() {
                if let Err(err) = crate::prefs::update(|p| p.launch_at_login = Some(true)) {
                    eprintln!("cortex: {err}");
                }
            }
        }
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
    fn entry(exe: &str) -> Option<String> {
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
    fn entry(exe: &str) -> Option<String> {
        cortex_core::autostart::desktop_entry(exe)
    }

    pub fn is_enabled() -> bool {
        entry_path().is_ok_and(|p| p.exists())
    }

    pub fn write(exe: &str) -> Result<(), String> {
        let content = entry(exe).ok_or("The app's path cannot be used to open it at login.")?;
        let path = entry_path()?;
        if crate::files::read_bounded(&path, 64 * 1024)
            .ok()
            .flatten()
            .as_deref()
            == Some(content.as_str())
        {
            return Ok(());
        }
        crate::files::write_private(&path, content.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    pub fn remove() -> Result<(), String> {
        match std::fs::remove_file(entry_path()?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
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

    pub fn write(exe: &str) -> Result<(), String> {
        let command = cortex_core::autostart::run_command(exe)
            .ok_or("The app's path cannot be used to open it at login.")?;
        registry::set(RUN_KEY, VALUE, &command)
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
    }
}
