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
    /// Additional commands to launch after the built-in set, each already
    /// split into argv.
    extra: Vec<Vec<String>>,
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

    pub fn extra(&self) -> &[Vec<String>] {
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
fn read_extras(path: &std::path::Path) -> Vec<Vec<String>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_extras(&text)
}

fn parse_extras(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(split_argv)
        .filter(|argv| !argv.is_empty())
        .collect()
}

/// Split one line into argv the way a user expects, without being a shell.
///
/// Quoting groups words that contain spaces, which config paths do, and `#`
/// begins a comment where a word would start -- both matching shell intuition.
/// Nothing else is interpreted: no variable expansion, no globbing, no command
/// substitution. So a backtick or `$(...)` in this file is inert text, and the
/// file cannot be turned into an execution vector by something that can write
/// to it but not to the binaries it names.
fn split_argv(line: &str) -> Vec<String> {
    let mut argv = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;

    let mut push = |word: &mut String, started: &mut bool| {
        if *started {
            argv.push(std::mem::take(word));
            *started = false;
        }
    };

    for c in line.chars() {
        match quote {
            // Closing quote. `started` stays set, so `""` yields an empty arg.
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                started = true;
            }
            None if c == '#' && !started => break,
            None if c.is_whitespace() => push(&mut word, &mut started),
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    push(&mut word, &mut started);
    argv
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
            vec![
                vec!["waybar"],
                vec!["swww-daemon"],
                vec!["rofi", "-show", "drun"],
            ]
        );
    }

    #[test]
    fn extras_split_into_argv_not_one_long_program_name() {
        // Regression: `Process::with_executable` takes the whole string as the
        // program name, so an unsplit line looks for a binary literally called
        // "waybar -c /path". The bar silently never starts.
        assert_eq!(
            split_argv("waybar -c /etc/waybar/config.jsonc"),
            vec!["waybar", "-c", "/etc/waybar/config.jsonc"]
        );
    }

    #[test]
    fn quotes_hold_arguments_containing_spaces_together() {
        assert_eq!(
            split_argv("waybar -c '/home/a b/config.jsonc' -s \"/home/a b/style.css\""),
            vec![
                "waybar",
                "-c",
                "/home/a b/config.jsonc",
                "-s",
                "/home/a b/style.css",
            ]
        );
    }

    #[test]
    fn hash_is_a_comment_between_words_but_data_inside_one() {
        // Matches shell intuition, and keeps hex colours usable as arguments.
        assert_eq!(split_argv("swaybg -c #1a1b26"), vec!["swaybg", "-c"]);
        assert_eq!(
            split_argv("swaybg -c '#1a1b26'"),
            vec!["swaybg", "-c", "#1a1b26"]
        );
        assert_eq!(
            split_argv("swaybg --color=#1a1b26"),
            vec!["swaybg", "--color=#1a1b26"]
        );
    }

    #[test]
    fn nothing_is_expanded_so_the_file_is_not_an_execution_vector() {
        // A writer of this file gets to name a program and its arguments, and
        // nothing more: no subshell, no variable, no glob.
        assert_eq!(
            split_argv("waybar $(id) `id` $HOME *"),
            vec!["waybar", "$(id)", "`id`", "$HOME", "*"]
        );
    }

    #[test]
    fn missing_extras_file_is_not_an_error() {
        assert!(read_extras(std::path::Path::new("/nonexistent/hyprcosmic/autostart")).is_empty());
    }

    /// The template we ship is mostly prose, and `#` starts a comment wherever
    /// a word would start — so a comment marker in the wrong column drops a
    /// command silently instead of failing. Parse the real file and assert
    /// what it yields, so editing the prose cannot quietly disable the bar.
    #[test]
    fn the_shipped_autostart_parses_to_the_commands_it_documents() {
        let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../config/autostart"));
        let got = parse_extras(text);
        assert_eq!(got.len(), 5, "{got:?}");
        // First, so its startup compile lands before the compositor settles.
        assert_eq!(got[0], vec!["cosmic-conf", "watch"]);
        // The bar is a shell invocation, because its stylesheet lives under a
        // home directory that this file cannot expand on its own.
        assert_eq!(got[1][0], "sh");
        assert_eq!(got[2], vec!["awww-daemon"]);
        // Last, so the fallback terminal is the topmost window.
        assert_eq!(got[4], vec!["cosmic-term"]);
    }

    /// The wallpaper line is the one entry that is a shell invocation, so the
    /// whole script has to arrive as a single argument. Getting the quoting
    /// wrong splits it and awww sets nothing -- which is exactly the silent
    /// blank screen this line was added to fix, so assert the argv rather than
    /// trusting it by eye.
    ///
    /// It once named a wallpaper directly and had to survive the space in
    /// "Tokyo Night"; it now names the `current` symlink that
    /// `cosmic-conf import-theme --assets` maintains, which has no space in it
    /// and does not change when the theme does. Quoting a path that contains a
    /// space is still covered, on synthetic input, by
    /// `quotes_hold_arguments_containing_spaces_together`.
    #[test]
    fn the_wallpaper_line_survives_its_nested_quoting() {
        let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../config/autostart"));
        let argv = &parse_extras(text)[3];
        assert_eq!(argv[0], "sh");
        assert_eq!(argv[1], "-c");
        // One argument, not several: the script stayed in one piece.
        assert_eq!(argv.len(), 3, "{argv:?}");
        // The symlink, not one of the copies beside it. Naming a copy would
        // strand this line on a path the next theme import deletes.
        //
        // `$HOME` rather than a real home directory: sh expands it, and the
        // shipped template has to work for whoever installed it, not only for
        // whoever wrote it. The double quotes are asserted with it, because
        // dropping them is what would split the path at a space in $HOME.
        assert!(
            argv[2].contains(r#""$HOME/.local/share/wallpapers/hyprcosmic/current""#),
            "{}",
            argv[2]
        );
        // The daemon is not listening the instant it is forked, so the image
        // must not be set until it answers.
        assert!(argv[2].starts_with("until awww query"), "{}", argv[2]);
    }

    /// The template is installed verbatim into every user's config directory,
    /// so a literal `/home/someone` in it is a file that works for exactly one
    /// person -- and it fails quietly for everyone else, because a waybar with
    /// an unreadable stylesheet still starts and a wallpaper that was never set
    /// looks the same as one that failed to load.
    ///
    /// Both lines carried an author's home directory until the repository was
    /// about to be published. This is here so that neither can carry one again.
    #[test]
    fn the_shipped_autostart_names_nobodys_home_directory() {
        let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../config/autostart"));
        for (n, line) in text.lines().enumerate() {
            assert!(
                !line.contains("/home/"),
                "config/autostart:{}: absolute home directory: {line}",
                n + 1
            );
        }
    }
}
