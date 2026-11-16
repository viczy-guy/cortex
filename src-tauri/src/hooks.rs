//! Hook installer — the I/O half. The merge itself is pure and lives in
//! `cortex_core::hooks`; this module owns the files.
//!
//! Guarantees (docs/SPEC.md — "Hook installer"):
//! - The settings file is never written without an explicit `install(token)`
//!   or `uninstall(token)` call, and only if it still is the exact file the
//!   user reviewed.
//! - A dated backup is made before any write.
//! - Writes are atomic (temp file + rename).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cortex_core::diff::{self, DiffLine, LineKind};
use cortex_core::hooks as merge;
use serde::Serialize;
use serde_json::Value;

pub const HOOKS_EVENT: &str = "cortex://hooks";

#[cfg(windows)]
const HOOK_BINARY: &str = "cortex-hook.exe";
#[cfg(not(windows))]
const HOOK_BINARY: &str = "cortex-hook";

#[derive(Debug, Serialize)]
pub struct Status {
    pub installed: bool,
    pub settings_path: String,
}

#[derive(Debug, Serialize)]
pub struct Preview {
    /// The exact change, line by line.
    pub diff: Vec<DiffLine>,
    /// Identifies the reviewed file; install and uninstall refuse anything else.
    pub token: String,
    pub settings_path: String,
    /// `false` when the proposed settings match the current text.
    pub changes: bool,
}

#[derive(Debug, Serialize)]
pub struct Installed {
    /// `None` when there was no previous settings file to back up.
    pub backup_path: Option<String>,
}

pub fn status() -> Result<Status, String> {
    let settings_path = settings_path()?;
    let (_, settings) = read_settings(&settings_path)?;
    let command = hook_command()?;
    Ok(Status {
        installed: merge::is_installed(&settings, &command),
        settings_path: settings_path.display().to_string(),
    })
}

pub fn preview() -> Result<Preview, String> {
    preview_at(&settings_path()?, proposed_text)
}

pub fn preview_uninstall() -> Result<Preview, String> {
    preview_at(&settings_path()?, uninstalled_text)
}

fn preview_at(
    settings_path: &Path,
    transform: impl FnOnce(&Value) -> Result<String, String>,
) -> Result<Preview, String> {
    let (original_text, settings) = read_settings(settings_path)?;
    let proposed = transform(&settings)?;

    let diff = diff::lines(&original_text, &proposed);
    let changes = diff.iter().any(|l| l.kind != LineKind::Same);
    Ok(Preview {
        diff,
        token: token_of(&original_text),
        settings_path: settings_path.display().to_string(),
        changes,
    })
}

pub fn install(token: &str) -> Result<Installed, String> {
    apply_at(&settings_path()?, token, proposed_text)
}

pub fn uninstall(token: &str) -> Result<Installed, String> {
    apply_at(&settings_path()?, token, uninstalled_text)
}

fn apply_at(
    settings_path: &Path,
    token: &str,
    transform: impl FnOnce(&Value) -> Result<String, String>,
) -> Result<Installed, String> {
    let (original_text, settings) = read_settings(settings_path)?;
    if token_of(&original_text) != token {
        return Err("The settings file changed since you reviewed it. Review it again.".into());
    }
    let proposed = transform(&settings)?;

    let backup_path = if settings_path.exists() {
        let backup = unused_path(backup_path_for(settings_path, now_secs()));
        std::fs::copy(settings_path, &backup).map_err(|e| format!("backup failed: {e}"))?;
        Some(backup.display().to_string())
    } else {
        None
    };

    write_atomic(settings_path, &proposed).map_err(|e| format!("write failed: {e}"))?;
    Ok(Installed { backup_path })
}

/// Copy the relay next to the app's data so the hook path survives app
/// updates and moves. Returns the stable path.
pub fn ensure_hook_binary() -> Result<PathBuf, String> {
    let source = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("executable has no parent directory")?
        .join(HOOK_BINARY);
    let dest = hook_binary_path()?;

    let source_bytes = std::fs::read(&source)
        .map_err(|e| format!("relay not found at {}: {e}", source.display()))?;
    if std::fs::read(&dest).is_ok_and(|current| current == source_bytes) {
        return Ok(dest);
    }

    let dir = dest.parent().ok_or("relay path has no parent")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Write-then-rename so a running hook never sees a half-written binary.
    let tmp = dir.join(format!("{HOOK_BINARY}.tmp-{}", std::process::id()));
    std::fs::write(&tmp, &source_bytes).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

// ---------------------------------------------------------------------------

fn hook_command() -> Result<String, String> {
    Ok(merge::command_for(
        &hook_binary_path()?.display().to_string(),
    ))
}

fn proposed_text(settings: &Value) -> Result<String, String> {
    let merged = merge::install(settings, &hook_command()?).map_err(|e| e.to_string())?;
    settings_text(&merged)
}

fn uninstalled_text(settings: &Value) -> Result<String, String> {
    let removed = merge::uninstall(settings).map_err(|e| e.to_string())?;
    settings_text(&removed)
}

fn settings_text(settings: &Value) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// Read the settings file. A missing or empty file is an empty object; a file
/// that is not valid JSON is an error — Cortex never "repairs" it.
fn read_settings(path: &Path) -> Result<(String, Value), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok((text, Value::Object(Default::default())));
    }
    let value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not valid JSON ({e}). Fix it first.", path.display()))?;
    Ok((text, value))
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("settings path has no parent"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".settings.json.cortex-tmp-{}", std::process::id()));
    std::fs::write(&tmp, contents)?;
    // Keep the original file's permissions when there is one.
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path)
}

fn token_of(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `settings.json` → `settings.json.cortex-backup-YYYYMMDD-HHMMSS` (UTC).
fn backup_path_for(settings: &Path, unix_secs: u64) -> PathBuf {
    let (y, m, d) = civil_from_days((unix_secs / 86_400) as i64);
    let rem = unix_secs % 86_400;
    let stamp = format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    );
    let mut name = settings
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".cortex-backup-{stamp}"));
    settings.with_file_name(name)
}

/// A backup is never overwritten: two writes within the same second get
/// distinct names (`…-152301`, `…-152301-2`, …).
fn unused_path(path: PathBuf) -> PathBuf {
    let mut candidate = path.clone();
    let mut n = 2;
    while candidate.exists() {
        let mut name = path
            .file_name()
            .map(|f| f.to_os_string())
            .unwrap_or_default();
        name.push(format!("-{n}"));
        candidate = path.with_file_name(name);
        n += 1;
    }
    candidate
}

/// Days since the Unix epoch → (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "cannot determine the home directory".to_string())
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(home()?.join(".claude").join("settings.json"))
}

fn hook_binary_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("bin").join(HOOK_BINARY))
}

#[cfg(target_os = "macos")]
pub(crate) fn data_dir() -> Result<PathBuf, String> {
    Ok(home()?
        .join("Library")
        .join("Application Support")
        .join("Cortex"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn data_dir() -> Result<PathBuf, String> {
    match std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir).join("cortex")),
        None => Ok(home()?.join(".local").join("share").join("cortex")),
    }
}

#[cfg(windows)]
pub(crate) fn data_dir() -> Result<PathBuf, String> {
    match std::env::var_os("LOCALAPPDATA").filter(|d| !d.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir).join("Cortex")),
        None => Ok(home()?.join("AppData").join("Local").join("Cortex")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempSettings(PathBuf);

    impl TempSettings {
        fn new(contents: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "cortex-hooks-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("settings.json");
            std::fs::write(&path, contents).unwrap();
            Self(path)
        }
    }

    impl Drop for TempSettings {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    fn installed_text(settings: &Value) -> Result<String, String> {
        settings_text(&merge::install(settings, "cortex-hook").map_err(|e| e.to_string())?)
    }

    #[test]
    fn uninstall_restores_settings_and_backs_up_reviewed_content() {
        let original = r#"{"permissions":{"allow":["Read"]},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other-hook"}]}]}}"#;
        let file = TempSettings::new(original);
        let review = preview_at(&file.0, installed_text).unwrap();
        let installed = apply_at(&file.0, &review.token, installed_text).unwrap();
        assert_eq!(
            std::fs::read_to_string(installed.backup_path.unwrap()).unwrap(),
            original
        );
        let before_uninstall = std::fs::read_to_string(&file.0).unwrap();
        let review = preview_at(&file.0, uninstalled_text).unwrap();
        assert!(review.changes);
        let removed = apply_at(&file.0, &review.token, uninstalled_text).unwrap();
        assert_eq!(
            std::fs::read_to_string(removed.backup_path.unwrap()).unwrap(),
            before_uninstall
        );
        assert_eq!(
            read_settings(&file.0).unwrap().1,
            serde_json::from_str::<Value>(original).unwrap()
        );
    }

    #[test]
    fn same_second_writes_never_overwrite_a_backup() {
        let file = TempSettings::new("{}");
        let review = preview_at(&file.0, installed_text).unwrap();
        let first = apply_at(&file.0, &review.token, installed_text).unwrap();
        let review = preview_at(&file.0, uninstalled_text).unwrap();
        let second = apply_at(&file.0, &review.token, uninstalled_text).unwrap();

        let (first, second) = (first.backup_path.unwrap(), second.backup_path.unwrap());
        assert_ne!(first, second);
        // The first backup still holds the original file.
        assert_eq!(std::fs::read_to_string(first).unwrap(), "{}");
    }

    #[test]
    fn uninstall_keeps_empty_settings_file() {
        let file = TempSettings::new("{}");
        let review = preview_at(&file.0, installed_text).unwrap();
        apply_at(&file.0, &review.token, installed_text).unwrap();
        let review = preview_at(&file.0, uninstalled_text).unwrap();
        apply_at(&file.0, &review.token, uninstalled_text).unwrap();
        assert_eq!(std::fs::read_to_string(&file.0).unwrap(), "{}\n");
    }

    #[test]
    fn uninstall_refuses_stale_token_without_writing_or_backing_up() {
        let file = TempSettings::new("{}");
        let review = preview_at(&file.0, uninstalled_text).unwrap();
        std::fs::write(&file.0, "{ \n}").unwrap();
        let error = apply_at(&file.0, &review.token, uninstalled_text).unwrap_err();
        assert!(error.contains("changed since you reviewed"));
        assert_eq!(std::fs::read_to_string(&file.0).unwrap(), "{ \n}");
        assert_eq!(
            std::fs::read_dir(file.0.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn civil_dates_are_correct() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29)); // leap day
        assert_eq!(civil_from_days(20_773), (2026, 11, 16));
    }

    #[test]
    fn backup_name_is_dated_and_next_to_the_file() {
        // 2026-11-16 15:23:01 UTC
        let secs = 20_773 * 86_400 + 15 * 3600 + 23 * 60 + 1;
        let backup = backup_path_for(Path::new("/h/.claude/settings.json"), secs);
        assert_eq!(
            backup,
            Path::new("/h/.claude/settings.json.cortex-backup-20261116-152301")
        );
    }

    #[test]
    fn token_changes_with_the_file() {
        assert_eq!(token_of("{}"), token_of("{}"));
        assert_ne!(token_of("{}"), token_of("{ }"));
    }
}
