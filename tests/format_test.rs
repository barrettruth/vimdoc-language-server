use std::process::Command;

use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vimdoc-language-server"))
}

fn short_sep() -> String {
    "=".repeat(30)
}

fn full_sep() -> String {
    "=".repeat(78)
}

#[test]
fn format_in_place() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    std::fs::write(&file, format!("{}\nHello world\n", short_sep())).unwrap();

    let output = bin().arg("format").arg(&file).output().unwrap();

    assert!(output.status.success());
    let result = std::fs::read_to_string(&file).unwrap();
    assert!(
        result.starts_with(&full_sep()),
        "separator should be normalized to full width"
    );
}

#[test]
fn format_check_passes_when_formatted() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    std::fs::write(&file, format!("{}\nHello world\n", full_sep())).unwrap();

    let output = bin()
        .args(["format", "--check"])
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn format_check_fails_when_unformatted() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    std::fs::write(&file, format!("{}\nHello world\n", short_sep())).unwrap();

    let output = bin()
        .args(["format", "--check"])
        .arg(&file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Would reformat"));
}

#[test]
fn format_check_does_not_modify_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    let original = format!("{}\nHello world\n", short_sep());
    std::fs::write(&file, &original).unwrap();

    bin()
        .args(["format", "--check"])
        .arg(&file)
        .output()
        .unwrap();

    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(after, original);
}

#[test]
fn format_directory() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    std::fs::write(&file, format!("{}\nHello world\n", short_sep())).unwrap();

    let output = bin().arg("format").arg(dir.path()).output().unwrap();

    assert!(output.status.success());
    let result = std::fs::read_to_string(&file).unwrap();
    assert!(result.starts_with(&full_sep()));
}

#[test]
fn format_no_files_errors() {
    let output = bin().arg("format").output().unwrap();

    assert!(!output.status.success());
}

#[test]
fn format_respects_line_width() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("help.txt");
    std::fs::write(&file, format!("{}\n", short_sep())).unwrap();

    bin()
        .args(["--line-width", "40", "format"])
        .arg(&file)
        .output()
        .unwrap();

    let result = std::fs::read_to_string(&file).unwrap();
    let first_line = result.lines().next().unwrap();
    assert_eq!(first_line.len(), 40);
}
