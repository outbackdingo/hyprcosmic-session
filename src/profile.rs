// SPDX-License-Identifier: GPL-3.0-only

//! Which shell components this session should launch.
//!
//! Upstream hardcodes a list of `cosmic-*` components. HyprCosmic needs a
//! different set — cosmic-comp for window management, but waybar and rofi in
//! place of cosmic-panel and cosmic-launcher.
//!
//! Rather than delete the upstream calls, this gates them. The default profile
//! is byte-for-byte upstream behaviour, so an unconfigured build of this fork
//! still produces an ordinary COSMIC session; the alternate profile is opt-in
//! through the session's `.desktop` entry. That keeps the diff against upstream
//! to a handful of lines and means a broken HyprCosmic config can never leave
//! you unable to log in — the stock session entry is still there.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Selects the profile. Set by `hyprcosmic.desktop`; absent for the stock
/// COSMIC session entry.
pub const PROFILE_ENV: &str = "HYPRCOSMIC_PROFILE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    /// Upstream components to skip.
    disabled: BTreeSet<String>,
    /// Additional commands to launch after the built-in set.
    extra: Vec<String>,
}

impl Default for Profile {
    /// Stock COSMIC: launch everything upstream launches, add nothing.
    fn default() -> Self {
        Self {
            name: "cosmic".into(),
            disabled: BTreeSet::new(),
            extra: Vec::new(),
        }
    }
}

impl Profile {
    /// The HyprCosmic set: COSMIC's compositor, HyDE's shell.
    ///
    /// cosmic-greeter is deliberately NOT disabled — login stays on a known
    /// working greeter, because a display manager is the easiest thing to lock
    /// yourself out of.
    ///
    /// cosmic-osd and cosmic-idle are also kept: HyDE's equivalents are
    /// separate programs a user may not have, and neither conflicts with
    /// waybar for layer-shell space.
    pub fn hyprcosmic() -> Self {
        Self {
            name: "hyprcosmic".into(),
            disabled: [
                // Replaced by waybar.
                "cosmic-panel",
                // Replaced by rofi, launched on demand via a keybind rather
                // than run as a daemon.
                "cosmic-launcher",
                "cosmic-app-library",
                // COSMIC's overview has no HyDE equivalent and would render
                // over waybar.
                "cosmic-workspaces",
                // HyDE themes ship wallpapers driven by swww.
                "cosmic-bg",
                // cosmic-panel hosts this applet; without the panel it has
                // nowhere to appear.
                "cosmic-files-applet",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            extra: Vec::new(),
        }
    }

    /// Read the active profile from the environment, then layer any
    /// user-specified extra commands on top.
    pub fn active() -> Self {
        let mut profile = match std::env::var(PROFILE_ENV).as_deref() {
            Ok("hyprcosmic") => Self::hyprcosmic(),
            _ => Self::default(),
        };
        if let Some(path) = extras_path() {
            profile.extra = read_extras(&path);
        }
        profile
    }

    /// The active profile, resolved once. `start_component` consults this on
    /// every call, and re-reading the extras file each time would be silly.
    pub fn cached() -> &'static Profile {
        static PROFILE: std::sync::OnceLock<Profile> = std::sync::OnceLock::new();
        PROFILE.get_or_init(Profile::active)
    }

    pub fn is_enabled(&self, component: &str) -> bool {
        !self.disabled.contains(component)
    }

    pub fn extra(&self) -> &[String] {
        &self.extra
    }
}

fn extras_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("hyprcosmic").join("autostart"))
}

/// One command per line; `#` comments and blank lines ignored.
///
/// A missing file is normal, not an error — most sessions will not have one.
/// Deliberately not a shell: each line is exec'd directly, so a stray
/// backtick in a config cannot run something unexpected.
fn read_extras(path: &std::path::Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_extras(&text)
}

fn parse_extras(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| match l.find('#') {
            Some(i) => &l[..i],
            None => l,
        })
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_launches_everything_upstream_does() {
        // The stock session must be unaffected by this fork existing.
        let p = Profile::default();
        for c in [
            "cosmic-panel",
            "cosmic-launcher",
            "cosmic-app-library",
            "cosmic-workspaces",
            "cosmic-osd",
            "cosmic-bg",
            "cosmic-greeter",
            "cosmic-files-applet",
            "cosmic-idle",
        ] {
            assert!(p.is_enabled(c), "default profile must still launch {c}");
        }
    }

    #[test]
    fn hyprcosmic_profile_replaces_the_shell_but_keeps_the_compositor_side() {
        let p = Profile::hyprcosmic();
        for c in ["cosmic-panel", "cosmic-launcher", "cosmic-workspaces", "cosmic-bg"] {
            assert!(!p.is_enabled(c), "{c} should be replaced");
        }
        for c in ["cosmic-osd", "cosmic-idle"] {
            assert!(p.is_enabled(c), "{c} has no HyDE equivalent and should stay");
        }
    }

    #[test]
    fn greeter_is_never_disabled() {
        // Losing the greeter means losing the ability to log in.
        assert!(Profile::hyprcosmic().is_enabled("cosmic-greeter"));
    }

    #[test]
    fn extras_parse_ignoring_comments_and_blanks() {
        let text = "\n# a comment\nwaybar\n  swww-daemon  \n\nrofi -show drun # trailing\n";
        assert_eq!(
            parse_extras(text),
            vec!["waybar", "swww-daemon", "rofi -show drun"]
        );
    }

    #[test]
    fn missing_extras_file_is_not_an_error() {
        assert!(read_extras(std::path::Path::new("/nonexistent/hyprcosmic/autostart")).is_empty());
    }
}
