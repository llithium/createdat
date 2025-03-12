use assert_cmd::assert::OutputAssertExt;
use assert_cmd::Command;
use assert_fs::prelude::PathCreateDir;
use assert_fs::prelude::{FileTouch, PathChild};
use chrono::{DateTime, Local, Utc};
use predicates::prelude::predicate;
use std::{
    fs::read_dir,
    io::{self, Write},
};

fn setup() -> (
    assert_fs::TempDir,
    chrono::format::DelayedFormat<chrono::format::StrftimeItems<'static>>,
) {
    let temp = assert_fs::TempDir::new().unwrap();
    let test_image = temp.child("test.jpg");
    let test_video = temp.child("test.mp4");
    let test_dotfile = temp.child(".gitignore");
    test_image.touch().unwrap();
    test_video.touch().unwrap();
    test_dotfile.touch().unwrap();

    let now = Utc::now();
    let now_local: DateTime<Local> = now.into();
    let now_formatted = now_local.format("%Y-%m-%d_%H-%M-%S");

    (temp, now_formatted)
}

#[test]
fn no_flags() {
    let temp = assert_fs::TempDir::new().unwrap();
    let test_image = temp.child("test.jpg");
    test_image.touch().unwrap();

    let mut cmd = Command::cargo_bin("createdat").unwrap();
    let now = Utc::now();
    let now_local: DateTime<Local> = now.into();
    let now_formatted = now_local.format("%Y-%m-%d_%H-%M-%S");
    let output = cmd.current_dir(temp.path()).output().unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files = read_dir(temp.path().join("renamed")).unwrap();

    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files
            .filter_map(Result::ok)
            .next()
            .unwrap()
            .file_name()
            .into_string()
            .unwrap()
    );
    temp.close().unwrap();
}

#[test]
fn all() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd.current_dir(temp.path()).arg("-a").output().unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}

#[test]
fn no_name() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd.current_dir(temp.path()).arg("-an").output().unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}
#[test]
fn name() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd
        .current_dir(temp.path())
        .args(["-a", "testing"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("testing-{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("testing-test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("testing-test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}

#[test]
fn target() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd
        .current_dir(temp.path())
        .args(["-a", "-T", "testing"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("testing"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}

#[test]
fn twelve() {
    let (temp, _) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let now = Utc::now();
    let now_local: DateTime<Local> = now.into();
    let now_formatted = now_local.format("%Y-%m-%d_%I-%M-%S-%p");
    let output = cmd.current_dir(temp.path()).arg("-at").output().unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}
#[test]
fn front() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd
        .current_dir(temp.path())
        .args(["-af", "testing"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}-testing-test.jpg", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("{}-testing-test.mp4", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("{}-testing.gitignore", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}
#[test]
fn suffix() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd
        .current_dir(temp.path())
        .args(["-as", "testing"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}-testing.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}-testing.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}-testing.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}
#[test]
fn date() {
    let (temp, _) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let now = Utc::now();
    let now_local: DateTime<Local> = now.into();
    let now_formatted = now_local.format("%Y-%m-%d");
    let output = cmd
        .current_dir(temp.path())
        .args(["-asd", "testing"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}-testing.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}-testing.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}-testing.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}
#[test]
fn source() {
    let (temp, now_formatted) = setup();
    let test_dir = temp.child("test");
    test_dir.create_dir_all().unwrap();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd
        .current_dir(temp.path().join(test_dir.path()))
        .args(["-a", "-S", "../"])
        .output()
        .unwrap();
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();
    let files: Vec<_> = read_dir(temp.path().join(test_dir.path().join("renamed")))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );
    temp.close().unwrap();
}

#[test]
fn count() {
    let (temp, now_formatted) = setup();

    let mut cmd = Command::cargo_bin("createdat").unwrap();

    let output = cmd.current_dir(temp.path()).arg("-a").output().unwrap();

    let files: Vec<_> = read_dir(temp.path().join("renamed"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    assert_eq!(
        format!("{}.gitignore", now_formatted),
        files.first().unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.jpg", now_formatted),
        files.get(1).unwrap().file_name().into_string().unwrap()
    );
    assert_eq!(
        format!("test-{}.mp4", now_formatted),
        files.get(2).unwrap().file_name().into_string().unwrap()
    );

    output
            .assert()
            .success()
            .stdout(predicate::str::contains("\u{1b}[42m \u{1b}[49m\u{1b}[30;42m3\u{1b}[0m\u{1b}[30;42m/\u{1b}[0m\u{1b}[30;42m3\u{1b}[0m\u{1b}[42m \u{1b}[49m\u{1b}[32m Files renamed in"));

    temp.close().unwrap();
}
