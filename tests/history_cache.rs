use chrono::{NaiveDate, TimeZone};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use twstock_gpui::market::*;

#[derive(Clone, Default)]
struct Clock(Arc<Mutex<(Duration, Vec<Duration>)>>);
impl HistoryClock for Clock {
    fn elapsed(&self) -> Duration {
        self.0.lock().unwrap().0
    }
    fn sleep(&mut self, duration: Duration) {
        let mut state = self.0.lock().unwrap();
        state.0 += duration;
        state.1.push(duration);
    }
}
#[derive(Default)]
struct Fake {
    log: Arc<Mutex<Vec<(String, Duration)>>>,
    replies: VecDeque<Result<String, String>>,
    clock: Clock,
}
impl HttpTransport for Fake {
    fn get(&mut self, url: &str) -> Result<String, String> {
        self.log
            .lock()
            .unwrap()
            .push((url.to_owned(), self.clock.elapsed()));
        self.replies.pop_front().unwrap_or_else(|| {
            let date = url
                .split("date=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap();
            Ok(body(&date[..4], &date[4..6]))
        })
    }
}
fn body(year: &str, month: &str) -> String {
    json!({"stat":"OK","data":[[format!("{}/{}/01",year.parse::<i32>().unwrap()-1911,month),"1,000","0","10","12","9","11"]]}).to_string()
}
fn at(date: &str) -> i64 {
    chrono::FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .from_local_datetime(
            &NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
        )
        .unwrap()
        .timestamp()
}
fn symbol() -> Symbol {
    Symbol::new(Market::Listed, "TEST").unwrap()
}
fn cache(month: &str, fetched: &str) -> Value {
    json!({"version":1,"months":{month:{"fetched_on":fetched,"rows":[{"time":format!("{month}-01"),"open":100000,"high":120000,"low":90000,"close":110000,"volume":1000}]}}})
}
fn setup(dir: &std::path::Path) -> (HistoryProvider<Fake, Clock>, Fake) {
    let fake = Fake::default();
    let observer = Fake {
        log: fake.log.clone(),
        clock: fake.clock.clone(),
        ..Default::default()
    };
    (
        HistoryProvider::with_clock(fake, dir.to_owned(), observer.clock.clone()),
        observer,
    )
}

#[test]
fn cache_paths_use_home_and_measurement_override_without_touching_private_paths() {
    for case in ["home", "override", "missing"] {
        let home = tempfile::tempdir().unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "isolated_history_cache_path", "--nocapture"])
            .env("HOME", home.path())
            .env("HC_PATH_CASE", case)
            .env("HC_TEST_ROOT", home.path())
            .env_remove("TWSTOCK_GPUI_CACHE_DIR");
        if case == "override" {
            command
                .env("TWSTOCK_GPUI_CACHE_DIR", home.path().join("measurement"))
                .env_remove("HOME");
        }
        if case == "missing" {
            command.env_remove("HOME");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    }
}
#[test]
fn isolated_history_cache_path() {
    let Ok(case) = std::env::var("HC_PATH_CASE") else {
        return;
    };
    let root = std::path::PathBuf::from(std::env::var_os("HC_TEST_ROOT").unwrap());
    match case.as_str() {
        "home" => assert_eq!(
            history_cache_dir().unwrap(),
            root.join(".cache/twstock-gpui/history")
        ),
        "override" => assert_eq!(
            history_cache_dir().unwrap(),
            root.join("measurement/history")
        ),
        "missing" => assert!(history_cache_dir().is_err()),
        _ => panic!("unknown case"),
    }
}

#[cfg(unix)]
#[test]
fn unreadable_directory_falls_back_to_identical_network_data() {
    use std::os::unix::fs::PermissionsExt;
    let baseline_dir = tempfile::tempdir().unwrap();
    let (mut baseline, _) = setup(baseline_dir.path());
    let expected = baseline.recent(&symbol(), 2, at("2026-10-16")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tse-TEST.json");
    let old = cache("2026-09", "2026-10-01").to_string();
    std::fs::write(&path, &old).unwrap();
    let permissions = std::fs::metadata(dir.path()).unwrap().permissions();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let (mut provider, observer) = setup(dir.path());
    let result = provider.recent(&symbol(), 2, at("2026-10-16"));
    std::fs::set_permissions(dir.path(), permissions).unwrap();
    assert_eq!(result.unwrap(), expected);
    assert_eq!(observer.log.lock().unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(path).unwrap(), old);
}

#[test]
fn final_months_persist_across_provider_instances() {
    let dir = tempfile::tempdir().unwrap();
    let (mut cold, _) = setup(dir.path());
    let expected = cold.recent(&symbol(), 3, at("2026-10-16")).unwrap();
    let (mut warm, observer) = setup(dir.path());
    assert_eq!(
        warm.recent(&symbol(), 3, at("2026-10-16")).unwrap(),
        expected
    );
    assert_eq!(observer.log.lock().unwrap().len(), 1);
    assert!(observer.clock.0.lock().unwrap().1.is_empty());
}

#[cfg(unix)]
#[test]
fn failed_atomic_write_keeps_old_cache_and_does_not_fail_history() {
    use std::os::unix::fs::PermissionsExt;
    struct ReadOnlyOnFetch {
        directory: std::path::PathBuf,
    }
    impl HttpTransport for ReadOnlyOnFetch {
        fn get(&mut self, _: &str) -> Result<String, String> {
            std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o555))
                .unwrap();
            Ok(body("2026", "10"))
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tse-TEST.json");
    let old = cache("2026-09", "2026-10-01").to_string();
    std::fs::write(&path, &old).unwrap();
    let permissions = std::fs::metadata(dir.path()).unwrap().permissions();
    let mut provider = HistoryProvider::with_clock(
        ReadOnlyOnFetch {
            directory: dir.path().to_owned(),
        },
        dir.path().to_owned(),
        Clock::default(),
    );
    let result = provider.recent(&symbol(), 2, at("2026-10-16"));
    // Restore before asserting so a failure still permits temp-dir cleanup.
    std::fs::set_permissions(dir.path(), permissions).unwrap();
    assert_eq!(result.unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(path).unwrap(), old);
}

#[test]
fn final_months_hit_without_sleep_current_month_always_refetched() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = cache("2026-09", "2026-10-01");
    value["months"]["2026-08"] = cache("2026-08", "2026-09-01")["months"]["2026-08"].clone();
    std::fs::write(dir.path().join("tse-TEST.json"), value.to_string()).unwrap();
    let (mut provider, observer) = setup(dir.path());
    assert_eq!(
        provider
            .recent(&symbol(), 3, at("2026-10-16"))
            .unwrap()
            .len(),
        3
    );
    assert_eq!(observer.log.lock().unwrap().len(), 1);
    assert!(observer.clock.0.lock().unwrap().1.is_empty());
    provider.recent(&symbol(), 3, at("2026-10-16")).unwrap();
    let log = observer.log.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert!(log.iter().all(|(url, _)| url.contains("date=20261001")));
    assert!(log[1].1 - log[0].1 >= Duration::from_secs(2));
}
#[test]
fn midmonth_and_end_of_month_snapshots_are_not_final() {
    for fetched in ["2026-09-15", "2026-09-30"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("tse-TEST.json"),
            cache("2026-09", fetched).to_string(),
        )
        .unwrap();
        let (mut provider, observer) = setup(dir.path());
        provider.recent(&symbol(), 2, at("2026-10-16")).unwrap();
        assert_eq!(observer.log.lock().unwrap().len(), 2);
        assert_eq!(
            observer.clock.0.lock().unwrap().1,
            vec![Duration::from_secs(2)]
        );
        let persisted: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("tse-TEST.json")).unwrap())
                .unwrap();
        assert_eq!(persisted["months"]["2026-09"]["fetched_on"], "2026-10-16");
    }
}
#[test]
fn corrupt_wrong_version_invalid_rows_and_unusable_directory_fall_back() {
    let baseline_dir = tempfile::tempdir().unwrap();
    let (mut baseline, _) = setup(baseline_dir.path());
    let expected = baseline.recent(&symbol(), 2, at("2026-10-16")).unwrap();
    for contents in [
        "{",
        "{\"version\":99,\"months\":{}}",
        "{\"version\":1,\"months\":{\"2026-09\":{\"fetched_on\":\"2026-10-01\",\"rows\":[{}]}}}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tse-TEST.json"), contents).unwrap();
        let (mut provider, observer) = setup(dir.path());
        assert_eq!(
            provider.recent(&symbol(), 2, at("2026-10-16")).unwrap(),
            expected
        );
        assert_eq!(observer.log.lock().unwrap().len(), 2);
    }
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("not-a-directory");
    std::fs::write(&blocked, "keep").unwrap();
    let (mut provider, observer) = setup(&blocked);
    assert_eq!(
        provider.recent(&symbol(), 2, at("2026-10-16")).unwrap(),
        expected
    );
    assert_eq!(observer.log.lock().unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(blocked).unwrap(), "keep");
}
#[test]
fn errors_are_never_cached_and_are_retried() {
    for reply in [
        Err("HTTP 429".into()),
        Ok("Too many requests".into()),
        Ok("{\"stat\":\"too many requests\"}".into()),
        Ok("{\"stat\":\"OK\",\"data\":[[]]}".into()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let clock = Clock::default();
        let fake = Fake {
            clock: clock.clone(),
            replies: VecDeque::from([reply]),
            ..Default::default()
        };
        let log = fake.log.clone();
        let mut provider = HistoryProvider::with_clock(fake, dir.path().to_owned(), clock);
        assert!(provider.recent(&symbol(), 1, at("2026-10-16")).is_err());
        assert!(!dir.path().join("tse-TEST.json").exists());
        assert_eq!(
            provider
                .recent(&symbol(), 1, at("2026-10-16"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(log.lock().unwrap().len(), 2);
    }
}
#[test]
fn empty_past_month_is_final_and_candles_uses_same_cache_and_throttle() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = cache("2026-09", "2026-10-01");
    value["months"]["2026-09"]["rows"] = json!([]);
    std::fs::write(dir.path().join("tse-TEST.json"), value.to_string()).unwrap();
    let (mut provider, observer) = setup(dir.path());
    let rows = provider
        .candles(
            &symbol(),
            Timeframe::Day,
            DateRange {
                start: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            },
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(observer.log.lock().unwrap().len(), 1);
    let other = Symbol::new(Market::Listed, "NEXT").unwrap();
    provider.recent(&other, 2, at("2026-10-16")).unwrap();
    let log = observer.log.lock().unwrap();
    assert_eq!(log.len(), 3);
    assert!(
        log.windows(2)
            .all(|p| p[1].1 - p[0].1 >= Duration::from_secs(2))
    );
}
#[test]
fn one_month_steps_allow_selection_to_preempt_inflight_history() {
    let dir = tempfile::tempdir().unwrap();
    let (mut provider, observer) = setup(dir.path());
    let mut first = HistoryLoad::new(symbol(), 2, at("2026-10-16")).unwrap();
    let other = Symbol::new(Market::Listed, "NEXT").unwrap();
    let mut second = HistoryLoad::new(other, 1, at("2026-10-16")).unwrap();
    assert!(first.advance(&mut provider).unwrap().is_none());
    assert_eq!(first.ready_in(&mut provider), Duration::from_secs(2));
    assert_eq!(second.advance(&mut provider).unwrap().unwrap().len(), 1);
    assert_eq!(first.advance(&mut provider).unwrap().unwrap().len(), 2);
    let log = observer.log.lock().unwrap();
    assert!(log[0].0.contains("stockNo=TEST"));
    assert!(log[1].0.contains("stockNo=NEXT"));
    assert!(log[2].0.contains("stockNo=TEST"));
}
#[test]
fn successful_empty_response_is_cached_but_foreign_month_rows_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Clock::default();
    let fake = Fake {
        clock: clock.clone(),
        replies: VecDeque::from([
            Ok("{\"stat\":\"很抱歉，沒有符合條件的資料!\"}".into()),
            Ok(body("2026", "08")),
        ]),
        ..Default::default()
    };
    let mut provider = HistoryProvider::with_clock(fake, dir.path().to_owned(), clock);
    assert!(provider.recent(&symbol(), 1, at("2026-10-16")).is_err());
    let value: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("tse-TEST.json")).unwrap()).unwrap();
    assert_eq!(value["months"].as_object().unwrap().len(), 1);
    assert_eq!(value["months"]["2026-10"]["rows"], json!([]));
}
