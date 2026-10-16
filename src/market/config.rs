use super::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct Entry {
    market: Market,
    code: String,
    etf: Option<bool>,
    name: Option<String>,
}
#[derive(Deserialize)]
struct Watchlist {
    #[serde(alias = "symbol", alias = "symbols", alias = "instruments")]
    watchlist: Vec<Entry>,
}
pub fn load_watchlist() -> Result<Option<Vec<Instrument>>, String> {
    load_watchlist_at(&watchlist_path()?)
}
/// Read the resolved path once; callers keep it for subsequent edits.
pub fn load_watchlist_at(path: &std::path::Path) -> Result<Option<Vec<Instrument>>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_watchlist(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("自選設定讀取失敗：{e}")),
    }
}
pub fn parse_watchlist(text: &str) -> Result<Vec<Instrument>, String> {
    let file: Watchlist = toml::from_str(text).map_err(|e| format!("自選設定格式錯誤：{e}"))?;
    let mut instruments = Vec::new();
    for e in file.watchlist {
        let mut i = Instrument::new(Symbol::new(e.market, &e.code)?);
        if let Some(etf) = e.etf {
            i.tick_rule = if etf { TickRule::Etf } else { TickRule::Stock };
        }
        if let Some(name) = e.name {
            i.name = name;
        }
        if !instruments
            .iter()
            .any(|old: &Instrument| old.symbol == i.symbol)
        {
            instruments.push(i);
        }
    }
    Ok(instruments)
}

pub fn watchlist_path() -> Result<std::path::PathBuf, String> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unavailable")?);
    Ok(home.join(".config/twstock-gpui/watchlist.toml"))
}
fn matches_symbol(market: Option<&str>, code: Option<&str>, symbol: &Symbol) -> bool {
    code == Some(symbol.code.as_str())
        && market.is_some_and(|m| match symbol.market {
            Market::Listed => ["tse", "listed", "Listed"].contains(&m),
            Market::Otc => ["otc", "tpex", "Otc"].contains(&m),
        })
}
pub enum WatchlistChange {
    Add(Instrument),
    Remove(Symbol),
}
pub fn save_watchlist(
    path: &std::path::Path,
    change: WatchlistChange,
    max: usize,
) -> Result<Vec<Instrument>, String> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};
    let original = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("自選設定讀取失敗：{e}")),
    };
    let mut doc = original
        .parse::<DocumentMut>()
        .map_err(|e| format!("自選設定格式錯誤：{e}"))?;
    let key = ["watchlist", "symbol", "symbols", "instruments"]
        .into_iter()
        .find(|key| doc.contains_key(key))
        .unwrap_or("watchlist");
    let mut entries = if original.trim().is_empty() {
        Vec::new()
    } else {
        parse_watchlist(&original)?
    };
    match change {
        WatchlistChange::Add(item) => {
            if entries.iter().any(|i| i.symbol == item.symbol) {
                return Ok(entries);
            }
            if entries.len() >= max {
                return Err(format!("自選已達來源訂閱上限（{max} 檔），請先移除商品"));
            }
            if doc.get(key).is_none() {
                doc[key] = Item::ArrayOfTables(ArrayOfTables::new());
            }
            let mut table = Table::new();
            table["market"] = value(item.symbol.market.exchange());
            table["code"] = value(&item.symbol.code);
            table["name"] = value(&item.name);
            table["etf"] = value(item.tick_rule == TickRule::Etf);
            if let Some(tables) = doc[key].as_array_of_tables_mut() {
                tables.push(table);
            } else if let Some(array) = doc[key].as_array_mut() {
                let mut inline = toml_edit::InlineTable::new();
                for (key, item) in table {
                    inline.insert(key, item.into_value().map_err(|_| "商品設定值非純量")?);
                }
                array.push(inline);
            } else {
                return Err("自選設定須使用表格陣列".into());
            }
            entries.push(item);
        }
        WatchlistChange::Remove(symbol) => {
            let Some(index) = entries.iter().position(|i| i.symbol == symbol) else {
                return Ok(entries);
            };
            if let Some(array) = doc[key].as_array_mut() {
                array.retain(|value| {
                    !value.as_inline_table().is_some_and(|t| {
                        matches_symbol(
                            t.get("market").and_then(toml_edit::Value::as_str),
                            t.get("code").and_then(toml_edit::Value::as_str),
                            &symbol,
                        )
                    })
                });
            } else {
                let tables = doc[key]
                    .as_array_of_tables_mut()
                    .ok_or("自選設定須使用表格陣列")?;
                // Match source tables, not the deduplicated projection's row index.
                let positions = tables
                    .iter()
                    .enumerate()
                    .filter_map(|(n, t)| {
                        matches_symbol(
                            t.get("market").and_then(Item::as_str),
                            t.get("code").and_then(Item::as_str),
                            &symbol,
                        )
                        .then_some(n)
                    })
                    .collect::<Vec<_>>();
                let mut prefix = String::new();
                for n in positions.into_iter().rev() {
                    if n == 0 {
                        prefix = tables
                            .get(0)
                            .unwrap()
                            .decor()
                            .prefix()
                            .and_then(|r| r.as_str())
                            .unwrap_or("")
                            .into();
                    }
                    tables.remove(n);
                }
                if let Some(first) = tables.get_mut(0) {
                    let current = first
                        .decor()
                        .prefix()
                        .and_then(|r| r.as_str())
                        .unwrap_or("")
                        .to_owned();
                    first.decor_mut().set_prefix(format!("{prefix}{current}"));
                } else {
                    doc[key] = value(toml_edit::Array::new());
                    let trailing = doc.trailing().as_str().unwrap_or("");
                    doc.set_trailing(format!("{prefix}{trailing}"));
                }
            }
            entries.remove(index);
        }
    }
    super::directory::atomic_write(path, doc.to_string().as_bytes())
        .map_err(|e| format!("自選設定儲存失敗：{e}"))?;
    Ok(entries)
}
