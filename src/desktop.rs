//! Building the commands that set the desktop wallpaper.
//!
//! Everything here is pure: it decides which desktop is running and which
//! programs to run with which arguments, but never runs them. The binary runs
//! the plan, and only behind the explicit `--set` flag.
//!
//! | Desktop | Mechanism |
//! | --- | --- |
//! | macOS | `osascript`: System Events sets `picture` of every desktop |
//! | Windows | PowerShell calls `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` |
//! | GNOME | `gsettings` `org.gnome.desktop.background` `picture-uri` and `picture-uri-dark` |
//! | Cinnamon | `gsettings` `org.cinnamon.desktop.background` `picture-uri` |
//! | MATE | `gsettings` `org.mate.background` `picture-filename` |
//! | KDE Plasma | `plasma-apply-wallpaperimage` |
//! | Xfce | `xfconf-query`: every `.../last-image` property of `xfce4-desktop` |
//! | sway | `swaymsg output * bg <file> fill` (lasts until sway restarts) |
//!
//! Two tools put the path somewhere it can be read as code: sway joins its
//! arguments into one command line (split on `;` and `,`) and runs the file
//! name through `wordexp(3)` with no flags, and KDE Plasma pastes it into a
//! JavaScript string inside a `file://` URI. For those two, [`plan`] only
//! accepts a "plain" path: letters, digits, single spaces, and `- _ . / + @ : =`.

use std::fmt;

/// A desktop whose wallpaper wallgen knows how to set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    MacOs,
    Windows,
    Gnome,
    Cinnamon,
    Mate,
    Kde,
    Xfce,
    Sway,
}

impl Desktop {
    /// Every supported desktop, in display order.
    pub const ALL: [Desktop; 8] = [
        Desktop::MacOs,
        Desktop::Windows,
        Desktop::Gnome,
        Desktop::Cinnamon,
        Desktop::Mate,
        Desktop::Kde,
        Desktop::Xfce,
        Desktop::Sway,
    ];

    /// The name accepted by `--desktop`.
    pub fn name(self) -> &'static str {
        match self {
            Desktop::MacOs => "macos",
            Desktop::Windows => "windows",
            Desktop::Gnome => "gnome",
            Desktop::Cinnamon => "cinnamon",
            Desktop::Mate => "mate",
            Desktop::Kde => "kde",
            Desktop::Xfce => "xfce",
            Desktop::Sway => "sway",
        }
    }

    /// Look a desktop up by its `--desktop` name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Desktop> {
        let name = name.trim().to_ascii_lowercase();
        Desktop::ALL.into_iter().find(|d| d.name() == name)
    }
}

impl fmt::Display for Desktop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The comma-separated `--desktop` names, for help and errors.
pub fn names() -> String {
    Desktop::ALL.map(Desktop::name).join(", ")
}

/// Work out the desktop from the OS name (as in [`std::env::consts::OS`]),
/// the `XDG_CURRENT_DESKTOP` value, and whether `SWAYSOCK` is set.
///
/// # Errors
/// Returns a message naming what was found when no supported desktop matches.
pub fn detect(
    os: &str,
    xdg_current_desktop: Option<&str>,
    swaysock: bool,
) -> Result<Desktop, String> {
    match os {
        "macos" => return Ok(Desktop::MacOs),
        "windows" => return Ok(Desktop::Windows),
        _ => {}
    }
    // XDG_CURRENT_DESKTOP is a colon-separated list, most specific first
    // (for example "ubuntu:GNOME" or "X-Cinnamon").
    let entries: Vec<String> = xdg_current_desktop
        .unwrap_or_default()
        .split(':')
        .map(|e| e.trim().to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    for entry in &entries {
        let found = match entry.as_str() {
            "x-cinnamon" | "cinnamon" => Some(Desktop::Cinnamon),
            "mate" => Some(Desktop::Mate),
            "kde" => Some(Desktop::Kde),
            "xfce" => Some(Desktop::Xfce),
            "sway" => Some(Desktop::Sway),
            "gnome" => Some(Desktop::Gnome),
            _ => None,
        };
        if let Some(desktop) = found {
            return Ok(desktop);
        }
    }
    if swaysock {
        return Ok(Desktop::Sway);
    }
    let seen = xdg_current_desktop.filter(|v| !v.trim().is_empty());
    Err(format!(
        "cannot tell which desktop to set on {os} (XDG_CURRENT_DESKTOP={}); \
         pass --desktop with one of: {}",
        seen.map_or_else(|| "unset".to_string(), |v| format!("{v:?}")),
        names()
    ))
}

/// One program to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
    /// Extra environment variables for the program.
    pub env: Vec<(String, String)>,
    /// Whether a failure stops the run. Optional steps cover settings that
    /// only newer desktops have (GNOME's dark-mode wallpaper).
    pub required: bool,
}

impl Step {
    fn new(program: &str, args: &[&str]) -> Step {
        Step {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            env: Vec::new(),
            required: true,
        }
    }

    fn optional(mut self) -> Step {
        self.required = false;
        self
    }
}

/// How to set the wallpaper on a desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Run these steps in order.
    Run(Vec<Step>),
    /// Xfce: run [`xfce_list_step`], pick the properties with
    /// [`xfce_image_properties`], then run [`xfce_set_step`] for each.
    Xfce,
}

/// The environment variable that carries the path to the Windows script, so
/// the path never has to be quoted inside PowerShell code.
pub const WINDOWS_PATH_VAR: &str = "WALLGEN_WALLPAPER";

/// The PowerShell script that sets the wallpaper on Windows. It reads the
/// path from [`WINDOWS_PATH_VAR`]. 20 is `SPI_SETDESKWALLPAPER`; 3 is
/// `SPIF_UPDATEINIFILE | SPIF_SENDWININICHANGE` (save to the profile and
/// tell running programs).
pub const WINDOWS_SCRIPT: &str = r#"$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class WallgenWallpaper {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool SystemParametersInfo(int action, int param, string value, int flags);
}
'@
if (-not [WallgenWallpaper]::SystemParametersInfo(20, 0, $env:WALLGEN_WALLPAPER, 3)) {
    throw "SystemParametersInfo failed (error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
}
"#;

/// The plan for setting `path` (absolute) as the wallpaper on `desktop`.
///
/// # Errors
/// Returns a message when the path cannot be passed safely to the desktop.
pub fn plan(desktop: Desktop, path: &str) -> Result<Plan, String> {
    let uri = file_uri(path);
    let steps = match desktop {
        Desktop::MacOs => vec![Step::new(
            "osascript",
            &["-e", &crate::wallpaper_applescript(path)],
        )],
        Desktop::Windows => {
            let mut step = Step::new(
                "powershell",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-EncodedCommand",
                    &powershell_encoded(WINDOWS_SCRIPT),
                ],
            );
            step.env
                .push((WINDOWS_PATH_VAR.to_string(), path.to_string()));
            vec![step]
        }
        Desktop::Gnome => vec![
            Step::new(
                "gsettings",
                &["set", "org.gnome.desktop.background", "picture-uri", &uri],
            ),
            // GNOME 42 added a separate wallpaper for the dark style.
            Step::new(
                "gsettings",
                &[
                    "set",
                    "org.gnome.desktop.background",
                    "picture-uri-dark",
                    &uri,
                ],
            )
            .optional(),
        ],
        Desktop::Cinnamon => vec![Step::new(
            "gsettings",
            &[
                "set",
                "org.cinnamon.desktop.background",
                "picture-uri",
                &uri,
            ],
        )],
        Desktop::Mate => vec![Step::new(
            "gsettings",
            &["set", "org.mate.background", "picture-filename", path],
        )],
        Desktop::Sway | Desktop::Kde if !plain_path(path) => {
            return Err(format!(
                "{desktop} reads the wallpaper path as part of a command or script, so wallgen \
                 only passes it paths made of letters, digits, single spaces, and - _ . / + @ : =; \
                 write the image somewhere else with -o (got {path:?})"
            ));
        }
        Desktop::Sway => vec![Step::new("swaymsg", &["output", "*", "bg", path, "fill"])],
        Desktop::Kde => vec![Step::new("plasma-apply-wallpaperimage", &[path])],
        Desktop::Xfce => return Ok(Plan::Xfce),
    };
    Ok(Plan::Run(steps))
}

/// Whether `path` is absolute and made only of characters that mean nothing
/// to a shell, to sway's command parser, to a JavaScript string, or to a URI:
/// letters, digits, single spaces, and `- _ . / + @ : =`.
pub fn plain_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("  ")
        && path
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || "-_./+@:=".contains(c))
}

/// Xfce: list the `xfce4-desktop` properties.
pub fn xfce_list_step() -> Step {
    Step::new("xfconf-query", &["-c", "xfce4-desktop", "-l"])
}

/// Xfce: the per-monitor, per-workspace image properties in a property list
/// (`/backdrop/screen0/monitor0/workspace0/last-image` and the like).
pub fn xfce_image_properties(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|p| p.starts_with("/backdrop/") && p.ends_with("/last-image"))
        .map(str::to_string)
        .collect()
}

/// Xfce: set one image property.
pub fn xfce_set_step(property: &str, path: &str) -> Step {
    Step::new(
        "xfconf-query",
        &["-c", "xfce4-desktop", "-p", property, "-s", path],
    )
}

/// A `file://` URI for an absolute path, percent-encoding every byte outside
/// the unreserved set and `/`.
pub fn file_uri(path: &str) -> String {
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Encode a script for `powershell -EncodedCommand`: base64 of its UTF-16LE
/// bytes.
pub fn powershell_encoded(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_desktops() {
        assert_eq!(detect("macos", None, false), Ok(Desktop::MacOs));
        assert_eq!(detect("windows", Some("KDE"), false), Ok(Desktop::Windows));
        assert_eq!(
            detect("linux", Some("ubuntu:GNOME"), false),
            Ok(Desktop::Gnome)
        );
        assert_eq!(
            detect("linux", Some("X-Cinnamon"), false),
            Ok(Desktop::Cinnamon)
        );
        assert_eq!(detect("linux", Some("MATE"), false), Ok(Desktop::Mate));
        assert_eq!(detect("linux", Some("KDE"), false), Ok(Desktop::Kde));
        assert_eq!(detect("linux", Some("XFCE"), false), Ok(Desktop::Xfce));
        assert_eq!(detect("linux", Some("sway"), false), Ok(Desktop::Sway));
        assert_eq!(
            detect("freebsd", Some("Budgie:GNOME"), false),
            Ok(Desktop::Gnome)
        );
        assert_eq!(detect("linux", None, true), Ok(Desktop::Sway));
        let err = detect("linux", Some("Hyprland"), false).unwrap_err();
        assert!(
            err.contains("\"Hyprland\"") && err.contains("--desktop"),
            "{err}"
        );
        assert!(detect("linux", None, false).unwrap_err().contains("unset"));
    }

    #[test]
    fn names_round_trip() {
        for d in Desktop::ALL {
            assert_eq!(Desktop::from_name(d.name()), Some(d));
            assert_eq!(Desktop::from_name(&d.name().to_uppercase()), Some(d));
        }
        assert_eq!(Desktop::from_name("hyprland"), None);
    }

    #[test]
    fn file_uris_are_percent_encoded() {
        assert_eq!(file_uri("/home/me/a b.png"), "file:///home/me/a%20b.png");
        assert_eq!(file_uri("/tmp/x#1?.png"), "file:///tmp/x%231%3F.png");
        assert_eq!(file_uri("/tmp/\u{65e5}.png"), "file:///tmp/%E6%97%A5.png");
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), want, "{input:?}");
        }
        // "a" as UTF-16LE is 61 00.
        assert_eq!(powershell_encoded("a"), "YQA=");
    }

    #[test]
    fn plans_pass_the_path_as_an_argument() {
        let path = "/home/me/it's \"here\".png";
        let Ok(Plan::Run(steps)) = plan(Desktop::Gnome, path) else {
            panic!("gnome runs steps");
        };
        assert_eq!(steps.len(), 2);
        assert!(steps[0].required && !steps[1].required);
        assert_eq!(steps[0].args[3], file_uri(path));

        let kde_path = "/home/me/Pictures/wall gen.png";
        let Ok(Plan::Run(steps)) = plan(Desktop::Kde, kde_path) else {
            panic!("kde runs steps");
        };
        assert_eq!(steps[0].args, vec![kde_path.to_string()]);
        assert!(plan(Desktop::Kde, path).unwrap_err().contains("kde reads"));

        let Ok(Plan::Run(steps)) = plan(Desktop::Windows, "C:\\Users\\me\\w.png") else {
            panic!("windows runs steps");
        };
        assert_eq!(steps[0].program, "powershell");
        assert_eq!(
            steps[0].env,
            vec![(
                WINDOWS_PATH_VAR.to_string(),
                "C:\\Users\\me\\w.png".to_string()
            )]
        );
        // The path is not in the script; the script reads the variable.
        assert!(!WINDOWS_SCRIPT.contains("w.png"));
        assert!(WINDOWS_SCRIPT.contains("$env:WALLGEN_WALLPAPER"));

        assert_eq!(plan(Desktop::Xfce, path), Ok(Plan::Xfce));
    }

    #[test]
    fn sway_and_kde_get_only_plain_paths() {
        let Ok(Plan::Run(steps)) = plan(Desktop::Sway, "/home/me/Pictures/wall gen-1.png") else {
            panic!("plain path is fine");
        };
        assert_eq!(steps[0].args[3], "/home/me/Pictures/wall gen-1.png");
        assert!(plan(Desktop::Sway, "/home/me/\u{58c1}\u{7d19}.png").is_ok());
        for bad in [
            "/tmp/$(touch x).png",
            "/tmp/`id`.png",
            "/tmp/$HOME.png",
            "/tmp/a*.png",
            "/tmp/it's.png",
            "/tmp/a\"b.png",
            "/tmp/a\\b.png",
            "/tmp/a;b.png",
            "/tmp/a fill, exec touch proof.png",
            "/tmp/a#b.png",
            "/tmp/a%20b.png",
            "/tmp/a\nb.png",
            "/tmp/two  spaces.png",
            "~/w.png",
            "relative.png",
        ] {
            assert!(plan(Desktop::Sway, bad).is_err(), "{bad}");
            assert!(plan(Desktop::Kde, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn xfce_properties_are_filtered() {
        let list = "/backdrop/screen0/monitor0/workspace0/last-image\n\
                    /backdrop/screen0/monitor0/workspace0/image-style\n\
                    /backdrop/screen0/monitoreDP-1/workspace1/last-image\n\
                    /desktop-icons/style\n";
        assert_eq!(
            xfce_image_properties(list),
            vec![
                "/backdrop/screen0/monitor0/workspace0/last-image",
                "/backdrop/screen0/monitoreDP-1/workspace1/last-image",
            ]
        );
        assert_eq!(
            xfce_set_step("/p", "/x.png").args,
            vec!["-c", "xfce4-desktop", "-p", "/p", "-s", "/x.png"]
        );
    }
}
