use std::{fs, path::Path, process::Command};
use twstock_gpui::market::*;

fn run_case(case: &str) {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "isolated_home_case", "--nocapture"])
        .env("HOME", home.path())
        .env("TWSTOCK_TEST_HOME", home.path())
        .env("TWSTOCK_CONFIG_CASE", case)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn write(path: &Path, code: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("# invented fixture\n[[watchlist]]\nmarket='tse'\ncode='{code}'\n"),
    )
    .unwrap();
}
#[test]
fn existing_file_is_loaded() {
    run_case("present");
}
#[test]
fn missing_file_uses_sample_data_and_creates_the_file_on_write() {
    run_case("neither");
}
#[test]
fn write_back_targets_the_loaded_file() {
    run_case("write");
}
#[test]
fn invalid_file_is_an_error() {
    run_case("invalid");
}
#[test]
fn unreadable_path_is_an_error() {
    run_case("directory");
}

// Spawn the test binary with an isolated HOME: no global env mutation or real config access.
#[test]
fn isolated_home_case() {
    let Ok(case) = std::env::var("TWSTOCK_CONFIG_CASE") else {
        return;
    };
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    assert_eq!(
        std::env::var_os("TWSTOCK_TEST_HOME"),
        Some(home.clone().into_os_string())
    );
    assert!(
        home.starts_with(std::env::temp_dir()),
        "fixtures require a temporary HOME"
    );
    let path = home.join(".config/twstock-gpui/watchlist.toml");
    assert!(!path.exists());
    assert_eq!(watchlist_path().unwrap(), path);
    match case.as_str() {
        "invalid" => {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "invalid toml [").unwrap();
            assert!(load_watchlist().is_err());
        }
        "directory" => {
            fs::create_dir_all(&path).unwrap();
            assert!(load_watchlist().is_err());
        }
        "neither" => {
            assert!(
                load_watchlist().unwrap().is_none(),
                "None selects the existing Workspace sample-data branch"
            );
            assert!(!MockProvider::new().store().order.is_empty());
            let item = Instrument::new(Symbol::new(Market::Listed, "991003").unwrap());
            save_watchlist(&path, WatchlistChange::Add(item), 5).unwrap();
            assert!(path.exists());
        }
        "present" => {
            write(&path, "991001");
            assert_eq!(load_watchlist().unwrap().unwrap()[0].symbol.code, "991001");
        }
        "write" => {
            write(&path, "991001");
            let added = Instrument::new(Symbol::new(Market::Otc, "991003").unwrap());
            save_watchlist(&path, WatchlistChange::Add(added.clone()), 5).unwrap();
            assert!(
                parse_watchlist(&fs::read_to_string(&path).unwrap())
                    .unwrap()
                    .iter()
                    .any(|i| i.symbol == added.symbol)
            );
            save_watchlist(&path, WatchlistChange::Remove(added.symbol), 5).unwrap();
            assert_eq!(
                parse_watchlist(&fs::read_to_string(&path).unwrap())
                    .unwrap()
                    .len(),
                1
            );
        }
        other => panic!("unknown case {other}"),
    }
}
