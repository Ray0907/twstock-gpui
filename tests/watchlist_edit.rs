use twstock_gpui::market::*;
#[test]
fn edits_preserve_comments_alias_format_and_handle_empty_and_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config/watchlist.toml");
    let mut instrument = Instrument::new(Symbol::new(Market::Listed, "889901").unwrap());
    instrument.name = "編造晶圓".into();
    let first = save_watchlist(&path, WatchlistChange::Add(instrument.clone()), 1).unwrap();
    assert_eq!(first.len(), 1);
    std::fs::write(
        &path,
        "# 我的註解\n[[symbol]] # 保留這個\nmarket = 'tse'\ncode = '889901' # 記錄\n",
    )
    .unwrap();
    let before = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        save_watchlist(&path, WatchlistChange::Add(instrument.clone()), 1)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    let other = Symbol::new(Market::Otc, "889902").unwrap();
    assert!(
        save_watchlist(
            &path,
            WatchlistChange::Add(Instrument::new(other.clone())),
            1
        )
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    save_watchlist(&path, WatchlistChange::Remove(other), 1).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    save_watchlist(
        &path,
        WatchlistChange::Add(Instrument::new(Symbol::new(Market::Otc, "889902").unwrap())),
        2,
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("# 我的註解")
            && text.contains("# 保留這個")
            && text.contains("code = '889901' # 記錄")
    );
    save_watchlist(&path, WatchlistChange::Remove(instrument.symbol), 2).unwrap();
    save_watchlist(
        &path,
        WatchlistChange::Remove(Symbol::new(Market::Otc, "889902").unwrap()),
        2,
    )
    .unwrap();
    assert!(
        parse_watchlist(&std::fs::read_to_string(path).unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn inline_array_format_is_editable_without_losing_comments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("watchlist.toml");
    std::fs::write(
        &path,
        "# keep\nwatchlist = [{market='tse', code='889901'}] # inline\n",
    )
    .unwrap();
    let item = Instrument::new(Symbol::new(Market::Otc, "889902").unwrap());
    save_watchlist(&path, WatchlistChange::Add(item.clone()), 2).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# keep") && text.contains("# inline") && text.contains("code='889901'"));
    save_watchlist(&path, WatchlistChange::Remove(item.symbol), 2).unwrap();
    assert_eq!(
        parse_watchlist(&std::fs::read_to_string(path).unwrap())
            .unwrap()
            .len(),
        1
    );
}
