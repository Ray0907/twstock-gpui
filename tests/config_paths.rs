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
fn new_path_is_preferred_when_both_exist() {
    run_case("both");
}
#[test]
fn legacy_is_used_only_when_new_file_is_absent() {
    run_case("legacy");
}
#[test]
fn neither_file_uses_sample_data_and_new_write_path() {
    run_case("neither");
}
#[test]
fn write_back_targets_the_loaded_file() {
    run_case("write-new");
    run_case("write-legacy");
}
#[test]
fn invalid_new_file_does_not_fall_back_to_legacy() {
    run_case("invalid");
}
#[test]
fn unreadable_new_path_does_not_fall_back_to_legacy() {
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
    let new = home.join(".config/twstock-gpui/watchlist.toml");
    // Intentional legacy fallback fixture, always under the subprocess's temporary HOME.
    let legacy = home.join(".config/twstock-gpui/watchlist.toml");
    assert!(!new.exists() && !legacy.exists());
    if case != "neither" {
        write(&legacy, "991002");
    }
    if ["both", "write-new", "invalid"].contains(&case.as_str()) {
        write(&new, "991001");
    }
    if case == "invalid" {
        fs::write(&new, "invalid toml [").unwrap();
        assert!(load_watchlist().is_err());
        assert_eq!(watchlist_path().unwrap(), new);
        return;
    }
    if case == "directory" {
        fs::create_dir_all(&new).unwrap();
        assert!(load_watchlist().is_err());
        assert_eq!(watchlist_path().unwrap(), new);
        return;
    }
    let loaded = load_watchlist().unwrap();
    let path = watchlist_path().unwrap();
    if case == "neither" {
        assert!(
            loaded.is_none(),
            "None selects the existing Workspace sample-data branch"
        );
        assert!(!MockProvider::new().store().order.is_empty());
        assert_eq!(path, new);
        let item = Instrument::new(Symbol::new(Market::Listed, "991003").unwrap());
        save_watchlist(&path, WatchlistChange::Add(item), 5).unwrap();
        assert!(new.exists());
        assert!(
            !legacy.parent().unwrap().exists(),
            "never create the legacy directory"
        );
        return;
    }
    let use_new = ["both", "write-new"].contains(&case.as_str());
    assert_eq!(
        loaded.unwrap()[0].symbol.code,
        if use_new { "991001" } else { "991002" }
    );
    assert_eq!(path, if use_new { new.clone() } else { legacy.clone() });
    if case.starts_with("write-") {
        let legacy_before = fs::read(&legacy).unwrap();
        // Even if a new config appears later, write back to the already loaded path.
        if !use_new {
            write(&new, "991001");
        }
        let new_before = fs::read(&new).unwrap();
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
        if use_new {
            assert_eq!(fs::read(&legacy).unwrap(), legacy_before);
        } else {
            assert_eq!(fs::read(&new).unwrap(), new_before);
        }
    } else if !use_new {
        assert!(!new.exists(), "do not copy or migrate the legacy file");
    }
}
