//! Always rules on disk: `<data dir>/rules.json`, readable by this user only.
//! The format and its validation are pure and live in `cortex_core::approval`.

use std::path::Path;

use cortex_core::{rules_from_json, rules_to_json, AlwaysRule};

const MAX_FILE_BYTES: u64 = 1024 * 1024;

pub fn load() -> Vec<AlwaysRule> {
    match crate::hooks::data_dir() {
        Ok(dir) => load_at(&dir.join("rules.json")),
        Err(error) => {
            eprintln!("cortex: cannot locate rules: {error}");
            Vec::new()
        }
    }
}

pub fn save(rules: &[AlwaysRule]) -> Result<(), String> {
    save_at(&crate::hooks::data_dir()?.join("rules.json"), rules)
}

fn load_at(path: &Path) -> Vec<AlwaysRule> {
    match crate::files::read_bounded(path, MAX_FILE_BYTES) {
        Ok(Some(text)) => rules_from_json(&text),
        Ok(None) => Vec::new(),
        Err(error) => {
            eprintln!("cortex: cannot read {}: {error}", path.display());
            Vec::new()
        }
    }
}

fn save_at(path: &Path, rules: &[AlwaysRule]) -> Result<(), String> {
    crate::files::write_private(path, rules_to_json(rules).as_bytes())
        .map_err(|e| format!("cannot save rules: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRules(PathBuf);
    impl TempRules {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            Self(
                std::env::temp_dir()
                    .join(format!(
                        "cortex-rules-{}-{}-{}",
                        std::process::id(),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_nanos(),
                        NEXT.fetch_add(1, Ordering::Relaxed)
                    ))
                    .join("rules.json"),
            )
        }
    }
    impl Drop for TempRules {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn saved_rules_survive_repeated_atomic_replacements() {
        let file = TempRules::new();
        assert!(load_at(&file.0).is_empty());
        let rules = vec![AlwaysRule {
            tool_name: "Bash".into(),
            pattern: "cargo".into(),
        }];
        save_at(&file.0, &rules).unwrap();
        assert_eq!(load_at(&file.0), rules);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&file.0).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        save_at(&file.0, &[]).unwrap();
        assert!(load_at(&file.0).is_empty());
        assert_eq!(
            std::fs::read_dir(file.0.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn oversized_and_malformed_files_yield_no_rules() {
        let file = TempRules::new();
        save_at(&file.0, &[]).unwrap();
        std::fs::write(&file.0, "invalid").unwrap();
        assert!(load_at(&file.0).is_empty());
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file.0)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        assert!(load_at(&file.0).is_empty());
    }
}
