//! What starts Cortex at login (#81), one entry per system, built here so the
//! escaping is tested. The shell only writes it. Each entry runs this
//! executable with [`AUTOSTART_ARG`].

/// Passed to the app when the system starts it at login.
pub const AUTOSTART_ARG: &str = "--autostart";

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
    fn the_run_command_quotes_the_path_and_refuses_impossible_ones() {
        assert_eq!(
            run_command(r"C:\Users\Ana\AppData\Local\Cortex\Cortex.exe").as_deref(),
            Some(r#""C:\Users\Ana\AppData\Local\Cortex\Cortex.exe" --autostart"#)
        );
        assert_eq!(run_command(""), None);
        assert_eq!(run_command("C:\\a\"b"), None);
    }
}
