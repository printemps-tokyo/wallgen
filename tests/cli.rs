//! CLI tests. They never change the desktop: `--show-set` only prints the
//! commands `--set` would run.

use std::path::PathBuf;
use std::process::{Command, Output};

fn wallgen(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wallgen"))
        .args(args)
        .output()
        .expect("run wallgen")
}

fn temp_png(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wallgen-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}.png"))
}

#[test]
fn show_set_prints_the_desktop_commands() {
    let out_path = temp_png("gnome");
    let out = wallgen(&[
        "--size",
        "32x18",
        "-o",
        out_path.to_str().unwrap(),
        "--show-set",
        "--desktop",
        "gnome",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("gsettings set org.gnome.desktop.background picture-uri file://"));
    assert!(stdout.contains("picture-uri-dark"));
    assert!(stdout.contains("(optional)"));
    assert!(out_path.exists());
    std::fs::remove_dir_all(out_path.parent().unwrap()).unwrap();
}

#[test]
fn show_set_for_xfce_explains_the_two_steps() {
    let out_path = temp_png("xfce");
    let out = wallgen(&[
        "--size",
        "32x18",
        "-o",
        out_path.to_str().unwrap(),
        "--show-set",
        "--desktop",
        "xfce",
    ]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("xfconf-query -c xfce4-desktop -l"));
    assert!(stdout.contains("last-image"));
    std::fs::remove_dir_all(out_path.parent().unwrap()).unwrap();
}

#[test]
fn desktop_needs_set_and_must_be_known() {
    let out = wallgen(&["--size", "32x18", "--desktop", "kde"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--desktop only applies"));

    let out = wallgen(&["--size", "32x18", "--show-set", "--desktop", "amiga"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown desktop"));
}

#[test]
fn list_styles_marks_styles_outside_random() {
    let out = wallgen(&["--list-styles"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().count(), 8);
    assert_eq!(stdout.matches("use --style any").count(), 4);
}

#[test]
fn style_any_and_new_styles_render() {
    for style in ["any", "topo", "truchet", "mesh", "circles"] {
        let out_path = temp_png(style);
        let out = wallgen(&[
            "--seed",
            "5",
            "--size",
            "48x27",
            "--style",
            style,
            "-o",
            out_path.to_str().unwrap(),
        ]);
        assert!(out.status.success(), "{style}");
        assert!(out_path.exists(), "{style}");
        std::fs::remove_dir_all(out_path.parent().unwrap()).unwrap();
    }
}
