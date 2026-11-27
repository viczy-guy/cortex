//! Private preferences storage.
pub use cortex_core::prefs::Prefs;
use cortex_core::prefs::{
    checked_prefs_from_json, is_language, is_mascot, prefs_to_json, DEFAULT_MASCOT,
};
use tauri::{AppHandle, Emitter};

/// Island preferences shared by the webviews.
pub const ISLAND_PREFS_EVENT: &str = "cortex://island-prefs";

#[derive(serde::Serialize, Clone)]
pub struct IslandPrefs {
    pub mascot: String,
    pub position: String,
    /// Where it was dragged along its edge (#184); `None` is the middle.
    pub offset: Option<f64>,
    /// Whether it can be dragged there: the bottom corners stay (#184).
    pub drags: bool,
    pub screen: Option<String>,
    pub sound: bool,
    pub lively: bool,
    /// The language shown, "en" or "pt" (#154).
    pub language: String,
    /// The one picked in Settings; `None` follows the system.
    pub language_choice: Option<String>,
}

impl From<&Prefs> for IslandPrefs {
    fn from(prefs: &Prefs) -> Self {
        Self {
            mascot: prefs.mascot.as_deref().unwrap_or(DEFAULT_MASCOT).to_owned(),
            position: prefs.island_position.as_deref().unwrap_or("top").to_owned(),
            offset: prefs.island_offset,
            drags: cortex_core::placement::drags(prefs.island_position.as_deref().unwrap_or("top")),
            screen: prefs.island_screen.clone(),
            sound: prefs.sound.unwrap_or(true),
            lively: prefs.lively.unwrap_or(true),
            language: crate::language::current(prefs).into(),
            language_choice: prefs.language.clone(),
        }
    }
}

#[tauri::command]
pub fn island_prefs() -> IslandPrefs {
    IslandPrefs::from(&load())
}

#[tauri::command]
pub fn island_set_mascot(app: AppHandle, mascot: String) -> Result<(), String> {
    if !is_mascot(&mascot) {
        return Err("Unknown mascot.".into());
    }
    let prefs = update(|p| p.mascot = Some(mascot))?;
    crate::gaze::set_lively(&prefs);
    emit(&app, IslandPrefs::from(&prefs));
    Ok(())
}

/// Every window hears the preferences. What Rust draws follows the language
/// too: on System, it may have changed since the last time (#154).
pub fn emit(app: &AppHandle, shown: IslandPrefs) {
    crate::show_language(app, &shown.language);
    let _ = app.emit(ISLAND_PREFS_EVENT, shown);
}
/// English, Portuguese, or `None` to follow the system (#154): every window
/// hears it at once.
#[tauri::command]
pub fn language_set(app: AppHandle, language: Option<String>) -> Result<(), String> {
    if language.as_deref().is_some_and(|code| !is_language(code)) {
        return Err("Unknown language.".into());
    }
    let prefs = update(|p| p.language = language)?;
    emit(&app, IslandPrefs::from(&prefs));
    Ok(())
}

#[tauri::command]
pub fn island_set_sound(app: AppHandle, enabled: bool) -> Result<(), String> {
    let prefs = update(|p| p.sound = Some(enabled))?;
    emit(&app, IslandPrefs::from(&prefs));
    Ok(())
}
#[tauri::command]
pub fn island_set_lively(app: AppHandle, enabled: bool) -> Result<(), String> {
    let prefs = update(|p| p.lively = Some(enabled))?;
    crate::gaze::set_lively(&prefs);
    emit(&app, IslandPrefs::from(&prefs));
    Ok(())
}

pub fn load() -> Prefs {
    let read = || -> Result<Option<String>, String> {
        let path = crate::hooks::data_dir()?.join("prefs.json");
        crate::files::read_bounded(&path, 4096).map_err(|e| e.to_string())
    };
    match read() {
        Ok(Some(text)) => checked_prefs_from_json(&text).unwrap_or_else(|| {
            eprintln!("cortex: invalid preferences; using defaults");
            Prefs::default()
        }),
        // Absent on first launch; too large means not ours. Both mean defaults.
        Ok(None) => Prefs::default(),
        Err(e) => {
            eprintln!("cortex: cannot load preferences: {e}");
            Prefs::default()
        }
    }
}
/// The saved preferences, the defaults when there is no file yet, and
/// `None` when the file cannot be read or is not valid.
pub fn load_valid() -> Option<Prefs> {
    let path = crate::hooks::data_dir().ok()?.join("prefs.json");
    match crate::files::read_bounded(&path, 4096) {
        Ok(Some(text)) => checked_prefs_from_json(&text),
        Ok(None) => Some(Prefs::default()),
        Err(_) => None,
    }
}

/// Read, change and save under one lock, so the GitHub and update settings
/// never overwrite each other's fields.
pub fn update(change: impl FnOnce(&mut Prefs)) -> Result<Prefs, String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut prefs = load();
    change(&mut prefs);
    save(&prefs)?;
    Ok(prefs)
}
fn save(prefs: &Prefs) -> Result<(), String> {
    crate::files::write_private(
        &crate::hooks::data_dir()?.join("prefs.json"),
        prefs_to_json(prefs).as_bytes(),
    )
    .map_err(|e| format!("cannot save preferences: {e}"))
}
