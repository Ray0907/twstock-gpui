use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct Entry {
    market: Market,
    code: String,
}
#[derive(Deserialize)]
struct Watchlist {
    #[serde(alias = "symbol", alias = "symbols", alias = "instruments")]
    watchlist: Vec<Entry>,
}
pub fn load_watchlist() -> Result<Option<Vec<Instrument>>, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
    let path = std::path::PathBuf::from(home).join(".config/twstock-gpui/watchlist.toml");
    match std::fs::read_to_string(path) {
        Ok(text) => parse_watchlist(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("自選設定讀取失敗：{e}")),
    }
}
pub fn parse_watchlist(text: &str) -> Result<Vec<Instrument>, String> {
    let file: Watchlist = toml::from_str(text).map_err(|e| format!("自選設定格式錯誤：{e}"))?;
    if file.watchlist.is_empty() {
        return Err("自選清單不可為空".into());
    }
    let mut instruments = Vec::new();
    for e in file.watchlist {
        let i = Instrument::new(Symbol::new(e.market, &e.code)?);
        if !instruments
            .iter()
            .any(|old: &Instrument| old.symbol == i.symbol)
        {
            instruments.push(i);
        }
    }
    Ok(instruments)
}
