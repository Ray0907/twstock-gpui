use super::*;
use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub fn directory_fresh(fetched_at: Option<i64>, at: i64) -> bool {
    fetched_at.is_some_and(|fetched| (0..86400).contains(&at.saturating_sub(fetched)))
}
pub trait Directory: Send {
    fn instruments(&mut self) -> Result<Vec<Instrument>, String>;
}
#[derive(Default)]
struct Rows {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: Option<String>,
}
impl TokenSink for Rows {
    type Handle = ();
    fn process_token(&mut self, token: Token, _: u64) -> TokenSinkResult<()> {
        match token {
            Token::TagToken(tag) => match (tag.kind, tag.name.as_ref()) {
                (StartTag, "tr") => self.row.clear(),
                (StartTag, "td" | "th") => self.cell = Some(String::new()),
                (EndTag, "td" | "th") => {
                    if let Some(cell) = self.cell.take() {
                        self.row.push(cell.trim().into());
                    }
                }
                (EndTag, "tr") if !self.row.is_empty() => {
                    self.rows.push(std::mem::take(&mut self.row));
                }
                _ => {}
            },
            Token::CharacterTokens(s) => {
                if let Some(cell) = &mut self.cell {
                    cell.push_str(&s);
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}
/// Official section headings identify stock/ETF; CFI and numeric prefixes are not guessed.
pub fn parse_isin(html: &str, market: Market) -> Result<Vec<Instrument>, String> {
    let mut input = BufferQueue::default();
    input.push_back(html.into());
    let mut parser = Tokenizer::new(Rows::default(), Default::default());
    let _ = parser.feed(&mut input);
    parser.end();
    let mut kind = None;
    let mut items = Vec::new();
    for row in parser.sink.rows {
        if row.len() == 1 {
            kind = match row[0].as_str() {
                "股票" => Some(TickRule::Stock),
                "ETF" => Some(TickRule::Etf),
                _ => None,
            };
            continue;
        }
        let Some(tick_rule) = kind else { continue };
        if row.len() != 7 {
            return Err("ISIN 商品列欄數已改變".into());
        }
        if row[3] != market.label() {
            return Err("ISIN 市場欄不符請求市場".into());
        }
        let mut words = row[0].split_whitespace();
        let code = words.next().ok_or("ISIN 缺商品代號")?;
        let name = words.collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err("ISIN 缺商品名稱".into());
        }
        let mut item = Instrument::new(Symbol::new(market, code)?);
        item.name = name;
        item.tick_rule = tick_rule;
        items.push(item);
    }
    if items.is_empty() {
        return Err("ISIN 未取得股票／ETF；來源格式可能改變".into());
    }
    Ok(items)
}
/// Exact code/name, then code prefix, then case-insensitive name substring; code/market break ties.
pub fn search_directory<'a>(items: &'a [Instrument], query: &str) -> Vec<&'a Instrument> {
    let query = query.trim().to_lowercase();
    let mut results = items
        .iter()
        .filter_map(|i| {
            let code = i.symbol.code.to_lowercase();
            let name = i.name.to_lowercase();
            let rank = if code == query || name == query {
                0
            } else if code.starts_with(&query) {
                1
            } else if name.contains(&query) {
                2
            } else {
                return None;
            };
            Some((rank, i))
        })
        .collect::<Vec<_>>();
    results.sort_by(|(a, x), (b, y)| {
        a.cmp(b)
            .then(x.symbol.code.cmp(&y.symbol.code))
            .then(x.symbol.market.exchange().cmp(y.symbol.market.exchange()))
    });
    results.into_iter().map(|(_, i)| i).collect()
}
#[derive(Serialize, Deserialize)]
struct Cache {
    version: u8,
    fetched_at: i64,
    instruments: Vec<Instrument>,
}
pub struct IsinTransport;
impl HttpTransport for IsinTransport {
    fn get(&mut self, url: &str) -> Result<String, String> {
        let output = Command::new("curl")
            .args([
                "--silent",
                "--show-error",
                "--fail",
                "--location",
                "--compressed",
                "--connect-timeout",
                "10",
                "--max-time",
                "25",
                url,
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        let (text, _, errors) = encoding_rs::BIG5.decode(&output.stdout);
        if errors {
            return Err("ISIN Big5 解碼失敗".into());
        }
        Ok(text.into_owned())
    }
}
pub struct IsinDirectory<T: HttpTransport = IsinTransport> {
    transport: T,
    cache: PathBuf,
    fetched_at: Option<i64>,
}
impl IsinDirectory<IsinTransport> {
    pub fn new() -> Result<Self, String> {
        let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
        Ok(Self::with_transport(
            IsinTransport,
            PathBuf::from(home).join(".cache/twstock-gpui/directory.json"),
        ))
    }
}
impl<T: HttpTransport> IsinDirectory<T> {
    pub fn with_transport(transport: T, cache: PathBuf) -> Self {
        Self {
            transport,
            cache,
            fetched_at: None,
        }
    }
    pub fn fetched_at(&self) -> Option<i64> {
        self.fetched_at
    }
    pub fn fetch(&mut self) -> Result<Vec<Instrument>, String> {
        let mut items = Vec::new();
        for (mode, market) in [(2, Market::Listed), (4, Market::Otc)] {
            let html = self
                .transport
                .get(&format!(
                    "https://isin.twse.com.tw/isin/C_public.jsp?strMode={mode}"
                ))
                .map_err(|e| format!("商品清單抓取失敗（{}）：{e}", market.label()))?;
            items.extend(parse_isin(&html, market)?);
        }
        items.sort_by(|a, b| {
            a.symbol
                .code
                .cmp(&b.symbol.code)
                .then(a.symbol.market.exchange().cmp(b.symbol.market.exchange()))
        });
        items.dedup_by(|a, b| a.symbol == b.symbol);
        Ok(items)
    }
    pub fn load_at(&mut self, at: i64) -> Result<Vec<Instrument>, String> {
        if let Ok(bytes) = std::fs::read(&self.cache)
            && let Ok(cache) = serde_json::from_slice::<Cache>(&bytes)
            && cache.version == 1
            && directory_fresh(Some(cache.fetched_at), at)
            && !cache.instruments.is_empty()
            && cache.instruments.iter().all(|i| {
                Symbol::new(i.symbol.market, &i.symbol.code).is_ok()
                    && !i.name.is_empty()
                    && i.lot_size == 1000
            })
        {
            self.fetched_at = Some(cache.fetched_at);
            return Ok(cache.instruments);
        }
        let instruments = self.fetch()?;
        let cache = Cache {
            version: 1,
            fetched_at: at,
            instruments: instruments.clone(),
        };
        atomic_write(
            &self.cache,
            &serde_json::to_vec(&cache).map_err(|e| e.to_string())?,
        )?;
        self.fetched_at = Some(at);
        Ok(instruments)
    }
}
impl<T: HttpTransport> Directory for IsinDirectory<T> {
    fn instruments(&mut self) -> Result<Vec<Instrument>, String> {
        self.load_at(now())
    }
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("檔案缺少目錄")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
