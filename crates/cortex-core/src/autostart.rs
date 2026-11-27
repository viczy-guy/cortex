//! What starts Cortex at login (#81), one entry per system, built here so the
//! escaping is tested. The shell only writes it. Each entry runs this
//! executable with [`AUTOSTART_ARG`].

/// Passed to the app when the system starts it at login.
pub const AUTOSTART_ARG: &str = "--autostart";

/// What a launch does about the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAction {
    /// First launch of a version with the feature: write it, then remember "on".
    TurnOn,
    /// On, and the entry is there: rewrite it if the app moved.
    Refresh,
    Nothing,
}

/// On from the first launch (decided for 0.3.0); afterwards the user's
/// choice stands. An entry the system's own tool removed is not brought back:
/// Settings then shows it off.
pub fn launch_action(launch_at_login: Option<bool>, entry_exists: bool) -> LaunchAction {
    match launch_at_login {
        None => LaunchAction::TurnOn,
        Some(true) if entry_exists => LaunchAction::Refresh,
        _ => LaunchAction::Nothing,
    }
}

/// The screen a launch opens with (#80, #127).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningScreen {
    Welcome,
    /// "Update complete": shown even at login, since an update restart keeps
    /// the old arguments (`--autostart`, `--recovered-at`).
    Updated,
    Nothing,
}

/// A launch at login or a recovery restart (#115) shows nothing, unless
/// this is the first launch of a new version.
pub fn opening_screen(autostart: bool, recovered: bool, updated: bool) -> OpeningScreen {
    if updated {
        OpeningScreen::Updated
    } else if autostart || recovered {
        OpeningScreen::Nothing
    } else {
        OpeningScreen::Welcome
    }
}

/// The first launch of a new version: another one ran last (#127). Before
/// 0.3.2 no version was saved; the launch-at-login answer, saved by the first
/// launch of an installed 0.3.0 or later, tells an update from a new install.
pub fn was_updated(
    last_version: Option<&str>,
    launch_at_login: Option<bool>,
    current: &str,
) -> bool {
    match last_version {
        Some(last) => last != current,
        None => launch_at_login.is_some(),
    }
}

/// The update screen's page, told the version used last: What's new shows
/// the versions skipped since (#180). `last` is a checked version
/// (`prefs::is_version`); of its characters only `+` means something else in
/// a query.
pub fn updated_query(last: Option<&str>) -> String {
    match last {
        Some(last) => format!("mode=updated&from={}", last.replace('+', "%2B")),
        None => "mode=updated".into(),
    }
}

/// A new install (#152): no version saved (every launch since 0.3.2 saves
/// one) and no launch-at-login answer (which 0.3.0 and 0.3.1 did save).
pub fn is_first_launch(last_version: Option<&str>, launch_at_login: Option<bool>) -> bool {
    last_version.is_none() && launch_at_login.is_none()
}

/// A path that cannot be an installed app: a cargo build, or on macOS a
/// binary outside an app bundle, an app still on its disk image
/// (`/Volumes/<image>/<App>.app`) or one macOS runs from a translocated copy.
pub fn is_installed_path(exe: &str, macos: bool) -> bool {
    let unix = exe.replace('\\', "/");
    if unix.contains("/target/release/") || unix.contains("/target/debug/") {
        return false;
    }
    if !macos {
        return true;
    }
    if !exe.contains(".app/Contents/MacOS/") || exe.contains("/AppTranslocation/") {
        return false;
    }
    match exe.strip_prefix("/Volumes/") {
        Some(rest) => rest
            .split('/')
            .nth(1)
            .is_some_and(|second| !second.ends_with(".app")),
        None => true,
    }
}

/// A macOS LaunchAgent that runs `exe --autostart` when the user logs in.
pub fn launch_agent_plist(label: &str, exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>{AUTOSTART_ARG}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
        xml_escape(label),
        xml_escape(exe)
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// An XDG autostart entry (`~/.config/autostart`). `None` for a path the
/// entry's own quoting rules would make ambiguous; no autostart beats a
/// wrong command.
pub fn desktop_entry(exe: &str) -> Option<String> {
    if exe.is_empty() || exe.contains(['"', '`', '$', '\\', '%', '\n', '\r']) {
        return None;
    }
    Some(format!(
        "[Desktop Entry]\nType=Application\nName=Cortex\nExec=\"{exe}\" {AUTOSTART_ARG}\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
    ))
}

/// The command line for the Windows `Run` key. Windows paths cannot hold a
/// quote, so one means the path is not real.
pub fn run_command(exe: &str) -> Option<String> {
    if exe.is_empty() || exe.contains(['"', '\n', '\r', '\0']) {
        return None;
    }
    Some(format!("\"{exe}\" {AUTOSTART_ARG}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_says_so_even_at_login_or_after_a_recovery() {
        use OpeningScreen::*;
        for (autostart, recovered, updated, screen) in [
            (false, false, false, Welcome),
            (true, false, false, Nothing),
            (false, true, false, Nothing),
            (false, false, true, Updated),
            (true, false, true, Updated),
            (false, true, true, Updated),
        ] {
            assert_eq!(opening_screen(autostart, recovered, updated), screen);
        }
    }

    #[test]
    fn a_new_version_is_an_update_and_so_is_the_first_launch_after_0_3_1() {
        assert!(was_updated(Some("0.3.2"), Some(true), "0.3.3"));
        assert!(!was_updated(Some("0.3.3"), Some(true), "0.3.3"));
        // 0.3.0 and 0.3.1 saved no version, but did save launch at login.
        assert!(was_updated(None, Some(false), "0.3.2"));
        // A new install: nothing saved yet.
        assert!(!was_updated(None, None, "0.3.2"));
    }

    #[test]
    fn the_update_screen_learns_the_version_used_last() {
        assert_eq!(updated_query(Some("0.9.0")), "mode=updated&from=0.9.0");
        assert_eq!(
            updated_query(Some("1.0.0+abc")),
            "mode=updated&from=1.0.0%2Babc"
        );
        // Updated from 0.3.0 or 0.3.1, which saved no version.
        assert_eq!(updated_query(None), "mode=updated");
    }

    #[test]
    fn only_a_launch_with_nothing_saved_is_a_first_launch() {
        assert!(is_first_launch(None, None));
        // The second launch saved its version.
        assert!(!is_first_launch(Some("0.6.0"), Some(true)));
        // 0.3.0 and 0.3.1 saved no version, but did save launch at login.
        assert!(!is_first_launch(None, Some(false)));
        // A dev build never saves launch at login, but does save its version.
        assert!(!is_first_launch(Some("0.6.0"), None));
    }

    #[test]
    fn the_launch_agent_runs_the_app_with_the_autostart_argument_and_escapes_the_path() {
        let plist = launch_agent_plist(
            "io.example.app",
            "/Applications/R&D <x>.app/Contents/MacOS/app",
        );
        assert!(plist.contains("<string>io.example.app</string>"));
        assert!(plist
            .contains("<string>/Applications/R&amp;D &lt;x&gt;.app/Contents/MacOS/app</string>"));
        assert!(plist.contains("<string>--autostart</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"));
    }

    #[test]
    fn a_desktop_entry_quotes_the_path_and_refuses_ambiguous_ones() {
        let entry = desktop_entry("/home/ana/Apps/Cortex 1.AppImage").unwrap();
        assert!(entry.contains("Exec=\"/home/ana/Apps/Cortex 1.AppImage\" --autostart\n"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
        for bad in [
            "",
            "/a/\"b",
            "/a/$HOME",
            "/a/`x`",
            "/a\\b",
            "/a/100%",
            "/a\nExec=x",
        ] {
            assert_eq!(desktop_entry(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_first_launch_turns_it_on_and_then_the_users_choice_stands() {
        assert_eq!(launch_action(None, false), LaunchAction::TurnOn);
        assert_eq!(launch_action(None, true), LaunchAction::TurnOn);
        assert_eq!(launch_action(Some(true), true), LaunchAction::Refresh);
        assert_eq!(launch_action(Some(true), false), LaunchAction::Nothing);
        assert_eq!(launch_action(Some(false), true), LaunchAction::Nothing);
        assert_eq!(launch_action(Some(false), false), LaunchAction::Nothing);
    }

    #[test]
    fn only_an_installed_app_gets_an_entry() {
        let mac = |p| is_installed_path(p, true);
        assert!(mac("/Applications/Cortex.app/Contents/MacOS/cortex-app"));
        assert!(mac(
            "/Users/ana/Applications/Cortex.app/Contents/MacOS/cortex-app"
        ));
        assert!(mac(
            "/Volumes/Data/Applications/Cortex.app/Contents/MacOS/cortex-app"
        ));
        assert!(
            !mac("/Volumes/Cortex/Cortex.app/Contents/MacOS/cortex-app"),
            "the disk image"
        );
        assert!(!mac(
            "/private/var/folders/x/AppTranslocation/y/d/Cortex.app/Contents/MacOS/cortex-app"
        ));
        assert!(!mac("/Users/ana/cortex/target/release/cortex-app"));
        assert!(!mac("/usr/local/bin/cortex-app"), "not a bundle");
        let other = |p| is_installed_path(p, false);
        assert!(other("/usr/bin/cortex-app"));
        assert!(other("/home/ana/Apps/Cortex.AppImage"));
        assert!(other(r"C:\Users\Ana\AppData\Local\Cortex\Cortex.exe"));
        assert!(!other(r"C:\src\cortex\target\release\cortex-app.exe"));
        assert!(!other("/home/ana/cortex/target/release/cortex-app"));
    }

    #[test]
    fn the_run_command_quotes_the_path_and_refuses_impossible_ones() {
        assert_eq!(
            run_command(r"C:\Users\Ana\AppData\Local\Cortex\Cortex.exe").as_deref(),
            Some(r#""C:\Users\Ana\AppData\Local\Cortex\Cortex.exe" --autostart"#)
        );
        assert_eq!(run_command(""), None);
        assert_eq!(run_command("C:\\a\"b"), None);
    }
}
