//! Private preferences storage.
pub use cortex_core::prefs::Prefs;
use cortex_core::prefs::{checked_prefs_from_json, prefs_to_json};
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
