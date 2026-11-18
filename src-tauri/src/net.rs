//! Network lookups done natively (no CORS limits): song search (iTunes),
//! YouTube video ids, synced lyrics (LRCLIB), image fetch for colour sampling.

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 Wallplayer/0.1")
            .timeout(Duration::from_secs(12))
            .build()
            .expect("http client")
    })
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

#[derive(Serialize, Clone, Debug)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: String,
    pub preview: String,
    pub url: String,
    pub duration_ms: i64,
    /// platform the result came from: ytmusic | soundcloud | spotify | apple
    pub source: String,
    /// what the player needs: YouTube videoId, SoundCloud permalink, Spotify URI, Apple URL
    pub play: String,
}

pub async fn search_music(query: &str) -> Result<Vec<Track>, String> {
    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=song&media=music&limit=25",
        enc(query)
    );
    let v: Value = client()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let s = |x: &Value, k: &str| x.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    Ok(v.get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .map(|x| Track {
                    id: x.get("trackId").map(|v| v.to_string()).unwrap_or_default(),
                    title: s(x, "trackName"),
                    artist: s(x, "artistName"),
                    album: s(x, "collectionName"),
                    art: s(x, "artworkUrl100").replace("100x100bb", "600x600bb"),
                    preview: s(x, "previewUrl"),
                    url: s(x, "trackViewUrl"),
                    duration_ms: x.get("trackTimeMillis").and_then(|v| v.as_i64()).unwrap_or(0),
                    source: "apple".into(),
                    play: s(x, "trackViewUrl"),
                })
                .filter(|t| !t.title.is_empty())
                .collect()
        })
        .unwrap_or_default())
}

pub async fn youtube_search(query: &str) -> Result<Vec<String>, String> {
    // sp=EgIQAQ== → "Videos" filter
    let url = format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAQ%3D%3D",
        enc(query)
    );
    let html = client()
        .get(url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Cookie", "CONSENT=YES+1; SOCS=CAI")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let re = Regex::new(r#""videoId":"([A-Za-z0-9_-]{11})""#).unwrap();
    let mut out: Vec<String> = Vec::new();
    for c in re.captures_iter(&html) {
        let id = c[1].to_string();
        if !out.contains(&id) {
            out.push(id);
        }
        if out.len() >= 10 {
            break;
        }
    }
    Ok(out)
}

#[derive(Serialize, Clone, Debug)]
pub struct Lyrics {
    pub id: i64,
    pub track: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub synced: Option<String>,
    pub plain: Option<String>,
    /// lrclib | genius
    pub source: String,
    /// Genius page (lyrics fetched on demand)
    pub url: Option<String>,
}

fn to_lyrics(x: &Value) -> Lyrics {
    let s = |k: &str| x.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let o = |k: &str| x.get(k).and_then(|v| v.as_str()).map(|v| v.to_string());
    Lyrics {
        id: x.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        track: s("trackName"),
        artist: s("artistName"),
        album: s("albumName"),
        duration: x.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0),
        synced: o("syncedLyrics"),
        plain: o("plainLyrics"),
        source: "lrclib".into(),
        url: None,
    }
}

async fn lrclib(url: String) -> Result<Value, String> {
    client()
        .get(url)
        .header("Lrclib-Client", "Wallplayer v0.1")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

/// Strip "(feat. …)", " - Remastered 2011", "[Official Video]" etc.
fn clean_title(t: &str) -> String {
    let re = Regex::new(r"(?i)\s*([\(\[](feat|ft|with|official|lyric|audio|video|remaster|explicit)[^\)\]]*[\)\]]|\s-\s.*remaster.*$)").unwrap();
    re.replace_all(t, "").trim().to_string()
}

pub async fn lyrics(artist: &str, title: &str, duration: Option<f64>) -> Result<Option<Lyrics>, String> {
    let title = clean_title(title);
    let artist = artist.split(&[',', '&'][..]).next().unwrap_or(artist).trim().to_string();
    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        enc(&title),
        enc(&artist)
    );
    let v = lrclib(url).await?;
    let mut list: Vec<Lyrics> = v.as_array().map(|a| a.iter().map(to_lyrics).collect()).unwrap_or_default();
    if list.is_empty() {
        let v = lrclib(format!("https://lrclib.net/api/search?q={}", enc(&format!("{title} {artist}")))).await?;
        list = v.as_array().map(|a| a.iter().map(to_lyrics).collect()).unwrap_or_default();
    }
    // Prefer synced lyrics whose duration matches.
    let d = duration.unwrap_or(0.0);
    list.sort_by(|a, b| {
        let score = |l: &Lyrics| {
            let mut s = 0.0;
            if l.synced.is_some() {
                s -= 1000.0;
            }
            if d > 0.0 {
                s += (l.duration - d).abs();
            }
            s
        };
        score(a).partial_cmp(&score(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(best) = list.into_iter().next() {
        return Ok(Some(best));
    }
    Ok(genius_lyrics(&artist, &title).await.ok().flatten())
}

pub async fn lyrics_search(query: &str) -> Result<Vec<Lyrics>, String> {
    let mut out: Vec<Lyrics> = match lrclib(format!("https://lrclib.net/api/search?q={}", enc(query))).await {
        Ok(v) => v.as_array().map(|a| a.iter().take(10).map(to_lyrics).collect()).unwrap_or_default(),
        Err(_) => vec![],
    };
    if let Ok(hits) = genius_search(query).await {
        for h in hits.into_iter().take(4) {
            out.push(Lyrics {
                id: 0,
                track: h.title,
                artist: h.artist,
                album: String::new(),
                duration: 0.0,
                synced: None,
                plain: None,
                source: "genius".into(),
                url: Some(h.url),
            });
        }
    }
    Ok(out)
}

// ───────────────────────── Genius ─────────────────────────

#[derive(Serialize, Clone, Debug)]
pub struct GeniusHit {
    pub url: String,
    pub title: String,
    pub artist: String,
}

pub async fn genius_search(q: &str) -> Result<Vec<GeniusHit>, String> {
    let v: Value = client()
        .get(format!("https://genius.com/api/search/song?per_page=6&q={}", enc(q)))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let hits = v.pointer("/response/sections/0/hits").and_then(|h| h.as_array()).cloned().unwrap_or_default();
    Ok(hits
        .iter()
        .filter_map(|h| {
            let r = h.get("result")?;
            Some(GeniusHit {
                url: r.get("url")?.as_str()?.to_string(),
                title: r.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                artist: r
                    .get("artist_names")
                    .or_else(|| r.pointer("/primary_artist/name"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect())
}

fn html_decode(s: &str) -> String {
    let re = Regex::new(r"&(#x[0-9a-fA-F]+|#\d+|amp|lt|gt|quot|apos|nbsp);").unwrap();
    re.replace_all(s, |c: &regex::Captures| {
        let e = &c[1];
        match e {
            "amp" => "&".to_string(),
            "lt" => "<".to_string(),
            "gt" => ">".to_string(),
            "quot" => "\"".to_string(),
            "apos" => "'".to_string(),
            "nbsp" => " ".to_string(),
            _ if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
            _ => e[1..].parse::<u32>().ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
        }
    })
    .into_owned()
}

/// (tag start, content start, index after the matching </div>) for the div containing `attr_pos`.
fn div_block(html: &str, attr_pos: usize) -> Option<(usize, usize, usize)> {
    let start = html[..attr_pos].rfind("<div")?;
    let content = attr_pos + html[attr_pos..].find('>')? + 1;
    let (mut depth, mut i) = (1usize, content);
    while depth > 0 {
        let close = html[i..].find("</div>").map(|x| x + i)?;
        match html[i..].find("<div").map(|x| x + i) {
            Some(o) if o < close => {
                depth += 1;
                i = o + 4;
            }
            _ => {
                depth -= 1;
                i = close + 6;
            }
        }
    }
    Some((start, content, i))
}

fn genius_extract(html: &str) -> String {
    let br = Regex::new(r"(?i)<br\s*/?>").unwrap();
    let tag = Regex::new(r"<[^>]+>").unwrap();
    let mut out = String::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("data-lyrics-container=\"true\"") {
        let at = pos + rel;
        let Some((_, cs, end)) = div_block(html, at) else { break };
        let mut inner = html[cs..end - 6].to_string();
        while let Some(x) = inner.find("data-exclude-from-selection=\"true\"") {
            match div_block(&inner, x) {
                Some((s0, _, e0)) => inner.replace_range(s0..e0, ""),
                None => break,
            }
        }
        let t = br.replace_all(&inner, "\n");
        let t = tag.replace_all(&t, "");
        out.push_str(&html_decode(&t));
        out.push('\n');
        pos = end;
    }
    out.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !(l.starts_with('[') && l.ends_with(']')))
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn genius_page(url: &str) -> Result<String, String> {
    let html = client()
        .get(url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let t = genius_extract(&html);
    if t.is_empty() {
        Err("no lyrics on that page".into())
    } else {
        Ok(t)
    }
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

async fn genius_lyrics(artist: &str, title: &str) -> Result<Option<Lyrics>, String> {
    let hits = genius_search(&format!("{title} {artist}")).await?;
    let (nt, na) = (norm(title), norm(artist));
    let Some(hit) = hits.into_iter().find(|h| {
        let (ht, ha) = (norm(&h.title), norm(&h.artist));
        (ht.contains(&nt) || nt.contains(&ht)) && !ht.is_empty() && (na.is_empty() || ha.contains(&na) || na.contains(&ha))
    }) else {
        return Ok(None);
    };
    let text = genius_page(&hit.url).await?;
    Ok(Some(Lyrics {
        id: 0,
        track: hit.title,
        artist: hit.artist,
        album: String::new(),
        duration: 0.0,
        synced: None,
        plain: Some(text),
        source: "genius".into(),
        url: Some(hit.url),
    }))
}

// ───────────────────────── translation ─────────────────────────

#[derive(Serialize, Clone, Debug)]
pub struct Translated {
    pub lines: Vec<String>,
    pub detected: String,
}

/// Machine-translate lyric lines (keeps 1:1 alignment, so timings are preserved).
pub async fn translate(lines: Vec<String>, target: &str) -> Result<Translated, String> {
    let mut out = vec![String::new(); lines.len()];
    let idx: Vec<usize> = (0..lines.len()).filter(|&i| !lines[i].trim().is_empty()).collect();
    let mut chunks: Vec<Vec<usize>> = Vec::new();
    let (mut cur, mut size) = (Vec::new(), 0usize);
    for &i in &idx {
        if size + lines[i].len() > 1600 && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
            size = 0;
        }
        size += lines[i].len() + 1;
        cur.push(i);
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    let mut detected = String::new();
    for c in chunks {
        let text = c.iter().map(|&i| lines[i].trim()).collect::<Vec<_>>().join("\n");
        let v: Value = client()
            .post(format!(
                "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t",
                enc(target)
            ))
            .form(&[("q", text.as_str())])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let mut joined = String::new();
        if let Some(segs) = v.get(0).and_then(|a| a.as_array()) {
            for seg in segs {
                if let Some(t) = seg.get(0).and_then(|t| t.as_str()) {
                    joined.push_str(t);
                }
            }
        }
        if detected.is_empty() {
            detected = v.get(2).and_then(|d| d.as_str()).unwrap_or("").to_string();
        }
        let parts: Vec<&str> = joined.split('\n').collect();
        for (k, &i) in c.iter().enumerate() {
            out[i] = parts.get(k).map(|p| p.trim().to_string()).unwrap_or_default();
        }
    }
    Ok(Translated { lines: out, detected })
}

pub async fn fetch_image(url: &str) -> Result<String, String> {
    let r = client().get(url).send().await.map_err(|e| e.to_string())?;
    let mime = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = r.bytes().await.map_err(|e| e.to_string())?;
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}


// ───────────────────────── JSON helpers ─────────────────────────

fn walk<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == key {
                    out.push(x);
                } else {
                    walk(x, key, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, key, out)),
        _ => {}
    }
}

fn first_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(s)) = m.get(key) {
                return Some(s);
            }
            m.values().find_map(|x| first_str(x, key))
        }
        Value::Array(a) => a.iter().find_map(|x| first_str(x, key)),
        _ => None,
    }
}

fn runs_text(v: &Value) -> String {
    if let Some(s) = v.get("simpleText").and_then(|s| s.as_str()) {
        return s.to_string();
    }
    v.get("runs")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|x| x.get("text").and_then(|t| t.as_str())).collect::<String>())
        .unwrap_or_default()
}

fn parse_dur(s: &str) -> i64 {
    s.split(':').fold(0i64, |acc, p| acc * 60 + p.trim().parse::<i64>().unwrap_or(0)) * 1000
}

fn last_thumb(v: &Value) -> String {
    let mut ts = Vec::new();
    walk(v, "thumbnails", &mut ts);
    ts.first()
        .and_then(|t| t.as_array())
        .and_then(|a| a.last())
        .and_then(|x| x.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string()
}

fn big_thumb(u: &str) -> String {
    let re = Regex::new(r"=w\d+-h\d+").unwrap();
    let u = re.replace(u, "=w544-h544").to_string();
    if u.starts_with("//") {
        format!("https:{u}")
    } else {
        u
    }
}

// ───────────────────────── YouTube Music ─────────────────────────

/// Songs search on music.youtube.com (InnerTube WEB_REMIX client).
pub async fn ytm_search(q: &str) -> Result<Vec<Track>, String> {
    let body = json!({
        "context": {"client": {"clientName": "WEB_REMIX", "clientVersion": "1.20241111.01.00", "hl": "en", "gl": "US"}},
        "query": q,
        "params": "EgWKAQIIAWoMEA4QChADEAQQCRAF"
    });
    let v: Value = client()
        .post("https://music.youtube.com/youtubei/v1/search?prettyPrint=false")
        .header("Origin", "https://music.youtube.com")
        .header("Referer", "https://music.youtube.com/")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    walk(&v, "musicResponsiveListItemRenderer", &mut items);
    let mut out: Vec<Track> = Vec::new();
    for it in items {
        let Some(vid) = first_str(it, "videoId") else { continue };
        if out.iter().any(|t| t.id == vid) {
            continue;
        }
        let cols = it.get("flexColumns").and_then(|c| c.as_array());
        let col = |i: usize| {
            cols.and_then(|c| c.get(i))
                .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
                .and_then(|c| c.get("text"))
                .map(runs_text)
                .unwrap_or_default()
        };
        let title = col(0);
        let mut parts: Vec<String> = col(1)
            .split(" • ")
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.first().map(|p| p == "Song" || p == "Video").unwrap_or(false) {
            parts.remove(0);
        }
        let dur = parts
            .last()
            .filter(|p| p.contains(':') && p.chars().all(|c| c.is_ascii_digit() || c == ':'))
            .map(|p| parse_dur(p))
            .unwrap_or(0);
        let artist = parts.first().cloned().unwrap_or_default();
        let album = if parts.len() >= 3 { parts[1].clone() } else { String::new() };
        if title.is_empty() {
            continue;
        }
        out.push(Track {
            id: vid.to_string(),
            title,
            artist,
            album,
            art: big_thumb(&last_thumb(it)),
            preview: String::new(),
            url: format!("https://music.youtube.com/watch?v={vid}"),
            duration_ms: dur,
            source: "ytmusic".into(),
            play: vid.to_string(),
        });
    }
    if out.is_empty() {
        return yt_search(q).await;
    }
    Ok(out)
}

/// Regular YouTube video search (InnerTube WEB client) – fallback and MV lookup.
pub async fn yt_search(q: &str) -> Result<Vec<Track>, String> {
    let body = json!({
        "context": {"client": {"clientName": "WEB", "clientVersion": "2.20241111.01.00", "hl": "en", "gl": "US"}},
        "query": q,
        "params": "EgIQAQ=="
    });
    let v: Value = client()
        .post("https://www.youtube.com/youtubei/v1/search?prettyPrint=false")
        .header("Origin", "https://www.youtube.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    walk(&v, "videoRenderer", &mut items);
    Ok(items
        .into_iter()
        .filter_map(|it| {
            let vid = it.get("videoId")?.as_str()?.to_string();
            let title = it.get("title").map(runs_text).unwrap_or_default();
            let artist = it.get("ownerText").map(runs_text).unwrap_or_default();
            let dur = it.get("lengthText").map(runs_text).map(|d| parse_dur(&d)).unwrap_or(0);
            Some(Track {
                id: vid.clone(),
                title,
                artist: artist.trim_end_matches(" - Topic").to_string(),
                album: String::new(),
                art: format!("https://i.ytimg.com/vi/{vid}/hqdefault.jpg"),
                preview: String::new(),
                url: format!("https://music.youtube.com/watch?v={vid}"),
                duration_ms: dur,
                source: "ytmusic".into(),
                play: vid,
            })
        })
        .take(24)
        .collect())
}

// ───────────────────────── SoundCloud ─────────────────────────

static SC_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

async fn sc_client_id(refresh: bool) -> Result<String, String> {
    if !refresh {
        let cached = SC_ID.lock().unwrap().clone();
        if let Some(id) = cached {
            return Ok(id);
        }
    }
    let html = client()
        .get("https://soundcloud.com/")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let sre = Regex::new(r#"src="(https://a-v2\.sndcdn\.com/assets/[^"]+\.js)""#).unwrap();
    let idre = Regex::new(r#"client_id\s*[:=]\s*"([A-Za-z0-9]{32})""#).unwrap();
    let urls: Vec<String> = sre.captures_iter(&html).map(|c| c[1].to_string()).collect();
    for u in urls.iter().rev() {
        if let Ok(r) = client().get(u).send().await {
            if let Ok(js) = r.text().await {
                if let Some(c) = idre.captures(&js) {
                    let id = c[1].to_string();
                    *SC_ID.lock().unwrap() = Some(id.clone());
                    return Ok(id);
                }
            }
        }
    }
    Err("SoundCloud search is unavailable right now".into())
}

pub async fn sc_search(q: &str) -> Result<Vec<Track>, String> {
    let mut id = sc_client_id(false).await?;
    let mut tries = 0;
    let v: Value = loop {
        let r = client()
            .get(format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=24",
                enc(q),
                id
            ))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if (r.status().as_u16() == 401 || r.status().as_u16() == 403) && tries == 0 {
            tries += 1;
            id = sc_client_id(true).await?;
            continue;
        }
        break r.json().await.map_err(|e| e.to_string())?;
    };
    let arr = v.get("collection").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    Ok(arr
        .iter()
        .filter_map(|x| {
            let s = |p: &str| x.pointer(p).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let link = s("/permalink_url");
            if link.is_empty() {
                return None;
            }
            let art = {
                let a = s("/artwork_url");
                let a = if a.is_empty() { s("/user/avatar_url") } else { a };
                a.replace("-large.", "-t500x500.")
            };
            let artist = {
                let a = s("/publisher_metadata/artist");
                if a.is_empty() { s("/user/username") } else { a }
            };
            Some(Track {
                id: x.get("id").map(|i| i.to_string()).unwrap_or_default(),
                title: s("/title"),
                artist,
                album: s("/publisher_metadata/album_title"),
                art,
                preview: String::new(),
                url: link.clone(),
                duration_ms: x.get("full_duration").or_else(|| x.get("duration")).and_then(|d| d.as_i64()).unwrap_or(0),
                source: "soundcloud".into(),
                play: link,
            })
        })
        .collect())
}

#[derive(Serialize, Clone, Debug)]
pub struct Cue {
    pub t: f64,
    pub text: String,
}

/// The YouTube video's own captions (CC), even though the player keeps them hidden.
/// Manual captions win over auto-generated ones. Returns [] when there are none.
pub async fn yt_captions(video_id: &str) -> Result<Vec<Cue>, String> {
    let clients = [
        json!({"clientName": "ANDROID", "clientVersion": "19.09.37", "androidSdkVersion": 30, "hl": "en", "gl": "US"}),
        json!({"clientName": "WEB", "clientVersion": "2.20241111.01.00", "hl": "en", "gl": "US"}),
    ];
    for c in clients {
        let body = json!({"context": {"client": c}, "videoId": video_id, "contentCheckOk": true, "racyCheckOk": true});
        let ua = if c["clientName"] == "ANDROID" { "com.google.android.youtube/19.09.37 (Linux; U; Android 11) gzip" } else { "" };
        let mut req = client().post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false").json(&body);
        if !ua.is_empty() {
            req = req.header("User-Agent", ua);
        } else {
            req = req.header("Origin", "https://www.youtube.com");
        }
        let v: Value = match req.send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let tracks = v
            .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();
        if tracks.is_empty() {
            continue;
        }
        let pick = tracks
            .iter()
            .find(|t| t.get("kind").and_then(|k| k.as_str()) != Some("asr"))
            .or_else(|| tracks.first())
            .unwrap();
        let Some(base) = pick.get("baseUrl").and_then(|b| b.as_str()) else { continue };
        let url = format!("{}&fmt=json3", base.replace("&fmt=srv3", ""));
        let r: Value = match client().get(url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let mut out: Vec<Cue> = Vec::new();
        for e in r.get("events").and_then(|e| e.as_array()).cloned().unwrap_or_default() {
            let Some(segs) = e.get("segs").and_then(|s| s.as_array()) else { continue };
            let t = e.get("tStartMs").and_then(|t| t.as_f64()).unwrap_or(0.0) / 1000.0;
            let text: String = segs.iter().filter_map(|s| s.get("utf8").and_then(|u| u.as_str())).collect::<Vec<_>>().join("");
            let text = text.replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() || (text.starts_with('[') && text.ends_with(']') && text.len() < 4) {
                continue;
            }
            if out.last().map(|l| l.text == text).unwrap_or(false) {
                continue;
            }
            out.push(Cue { t, text });
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    Ok(vec![])
}
