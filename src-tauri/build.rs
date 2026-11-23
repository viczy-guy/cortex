/// Every command in `generate_handler!`. Declaring them makes Tauri refuse any
/// command a window's capability (`capabilities/*.json`) does not grant, so the
/// island cannot quit or touch hooks and rules, and Settings cannot answer cards.
/// A command missing here is refused in every window.
const COMMANDS: &[&str] = &[
    "github_status",
    "github_set_enabled",
    "github_refresh",
    "github_set_account_shown",
    "github_sign_in",
    "github_install",
    "github_cancel_task",
    "open_url",
    "snapshot",
    "island_fit",
    "island_prefs",
    "autostart_status",
    "autostart_set",
    "island_set_mascot",
    "splash_done",
    "open_settings",
    "quit",
    "rules_list",
    "rule_remove",
    "rules_clear",
    "respond",
    "answer",
    "hooks_status",
    "hooks_preview",
    "hooks_install",
    "hooks_uninstall_preview",
    "hooks_uninstall",
    "updates_status",
    "updates_set_enabled",
    "updates_install",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .unwrap_or_else(|err| panic!("{err:#}"));
}
