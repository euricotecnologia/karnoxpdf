//! Online translation through the MyMemory HTTP API, with the checks the service needs:
//!
//! - the source language is `Autodetect` (not `auto`) or an ISO code; the service answers `auto`
//!   with a 403 whose *message* sits in `translatedText`, so the status is always checked;
//! - one query holds at most 500 characters, so text is cut at line and sentence boundaries;
//! - the same source and target language is a 403 as well, so it is skipped before asking;
//! - a failed part is counted and reported instead of being passed off as a translation.
//!
//! The transport is a closure so the logic is tested without a network.

/// One query is limited to 500 characters; bytes are a safe bound for any script.
pub const MAX_CHUNK_BYTES: usize = 450;

/// Consecutive failed parts after which the rest is not even tried (each try can wait seconds).
const GIVE_UP_AFTER: usize = 3;

/// What the service answered for one part.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub text: String,
    /// The language the service recognised, when asked to detect it.
    pub detected: Option<String>,
}

/// A query: text, source language code (or `Autodetect`) and target language code.
pub type Transport<'a> = &'a mut dyn FnMut(&str, &str, &str) -> Result<Reply, String>;

/// The result of translating a text: it is always complete, with untranslated parts kept as they were.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub text: String,
    pub parts: usize,
    pub failed: usize,
    pub error: Option<String>,
}

/// Decode the HTML entities the service puts in its answers, once (so `&amp;quot;` stays `&quot;`).
pub fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let decoded = tail.find(';').filter(|end| *end <= 9).and_then(|end| {
            let name = &tail[1..end];
            let ch = match name {
                "quot" => Some('"'),
                "apos" => Some('\''),
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix('#')
                    .and_then(|n| match n.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => n.parse().ok(),
                    })
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, end + 1))
        });
        match decoded {
            Some((c, used)) => {
                out.push(c);
                rest = &tail[used..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `pt-BR`, `PT_pt`, ` en ` → `pt`, `pt`, `en`.
pub fn normalize_lang(code: &str) -> String {
    code.trim().split(['-', '_']).next().unwrap_or("").to_ascii_lowercase()
}

/// Read the service's JSON answer. Anything but status 200 is an error carrying the service's reason.
pub fn parse_reply(body: &str) -> Result<Reply, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("unreadable reply: {e}"))?;
    let status = &value["responseStatus"];
    let status = status.as_u64().or_else(|| status.as_str().and_then(|s| s.trim().parse().ok()));
    let text = value["responseData"]["translatedText"].as_str().unwrap_or("");
    if status != Some(200) {
        let reason = value["responseDetails"].as_str().filter(|r| !r.trim().is_empty()).unwrap_or(text);
        return Err(if reason.trim().is_empty() {
            format!("the service answered with status {}", status.map_or("?".into(), |s| s.to_string()))
        } else {
            reason.trim().to_string()
        });
    }
    if value["quotaFinished"].as_bool() == Some(true) {
        return Err("the service's daily translation quota is used up".into());
    }
    let text = decode_entities(text);
    if text.trim().is_empty() {
        return Err("the service returned an empty translation".into());
    }
    let detected = value["responseData"]["detectedLanguage"].as_str().map(normalize_lang).filter(|l| !l.is_empty());
    Ok(Reply { text, detected })
}

/// The language a text is written in when its script says so (Japanese kana, Han, Cyrillic).
pub fn script_language(text: &str) -> Option<&'static str> {
    let (mut letters, mut kana, mut han, mut cyrillic) = (0usize, 0usize, 0usize, 0usize);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        match c as u32 {
            0x3040..=0x30FF | 0x31F0..=0x31FF => kana += 1,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF => han += 1,
            0x0400..=0x052F => cyrillic += 1,
            _ => {}
        }
    }
    if letters == 0 {
        None
    } else if kana * 10 >= letters {
        Some("ja")
    } else if han * 2 > letters {
        Some("zh")
    } else if cyrillic * 2 > letters {
        Some("ru")
    } else {
        None
    }
}

/// Cut one line into pieces of at most `max` bytes, at a sentence end, else a space, else a character.
fn split_long(line: &str, max: usize) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = line.trim();
    while rest.len() > max {
        let mut limit = max;
        while limit > 0 && !rest.is_char_boundary(limit) {
            limit -= 1;
        }
        if limit == 0 {
            // `max` is smaller than the first character: take that character whole.
            limit = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        let window = &rest[..limit];
        let mut sentence = None;
        let mut space = None;
        let mut chars = window.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let end = i + c.len_utf8();
            if matches!(c, '。' | '！' | '？') || (matches!(c, '.' | '!' | '?' | ';' | ':') && chars.peek().is_some_and(|(_, n)| n.is_whitespace()))
            {
                sentence = Some(end);
            } else if c.is_whitespace() {
                space = Some(i);
            }
        }
        let cut = sentence.or(space).filter(|cut| *cut > 0).unwrap_or(limit);
        pieces.push(rest[..cut].trim_end());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces
}

/// Cut a paragraph into parts of at most `max` bytes, keeping whole lines together where they fit.
pub fn split_chunks(paragraph: &str, max: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for line in paragraph.lines().map(str::trim).filter(|l| !l.is_empty()) {
        for piece in split_long(line, max) {
            if !current.is_empty() && current.len() + 1 + piece.len() > max {
                parts.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(piece);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// A couple of fixed terms, for parts the service could not translate.
pub fn offline_translate_fallback(text: &str, tgt_lang: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let translated = match (tgt_lang, trimmed.to_lowercase().as_str()) {
            ("pt", "introduction") => "Introdução",
            ("pt", "summary" | "abstract") => "Resumo",
            ("pt", "table of contents" | "contents") => "Sumário",
            ("pt", "conclusion") => "Conclusão",
            ("pt", "references") => "Referências",
            ("pt", "notes") => "Notas",
            ("pt", "page") => "Página",
            ("pt", "invoice") => "Fatura",
            ("pt", "receipt") => "Recibo",
            ("pt", "date") => "Data",
            ("pt", "description") => "Descrição",
            ("pt", "amount") => "Valor",
            ("pt", "quantity") => "Quantidade",
            ("pt", "author") => "Autor",
            ("pt", "title") => "Título",
            ("en", "introdução") => "Introduction",
            ("en", "resumo") => "Abstract",
            ("en", "sumário") => "Table of Contents",
            ("en", "conclusão") => "Conclusion",
            ("en", "referências") => "References",
            ("en", "data") => "Date",
            ("en", "descrição") => "Description",
            ("en", "valor") => "Amount",
            _ => line,
        };
        out.push_str(translated);
        out.push('\n');
    }
    out.trim_end_matches('\n').to_string()
}

/// Translate `text` from `src` (`auto` or a code) into `tgt`. `detected` carries the recognised source
/// language between calls, so a document is detected once and not page by page.
pub fn translate_text(text: &str, src: &str, tgt: &str, detected: &mut Option<String>, send: Transport) -> Outcome {
    let tgt = normalize_lang(tgt);
    let mut outcome = Outcome::default();
    if text.trim().is_empty() {
        return outcome;
    }
    let mut source = if src == "auto" { detected.clone().or_else(|| script_language(text).map(String::from)) } else { Some(normalize_lang(src)) };
    if source.as_deref() == Some(tgt.as_str()) {
        outcome.text = text.to_string();
        return outcome;
    }
    let mut consecutive = 0;
    let mut paragraphs = Vec::new();
    for paragraph in text.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let mut parts = Vec::new();
        for chunk in split_chunks(paragraph, MAX_CHUNK_BYTES) {
            outcome.parts += 1;
            if consecutive >= GIVE_UP_AFTER {
                outcome.failed += 1;
                parts.push(offline_translate_fallback(&chunk, &tgt));
                continue;
            }
            let from = source.clone().unwrap_or_else(|| "Autodetect".to_string());
            match send(&chunk, &from, &tgt) {
                Ok(reply) => {
                    consecutive = 0;
                    if source.is_none()
                        && let Some(found) = reply.detected.clone()
                    {
                        *detected = Some(found.clone());
                        source = Some(found);
                    }
                    parts.push(reply.text);
                }
                // The text is already in the target language.
                Err(e) if e.to_ascii_uppercase().contains("DISTINCT LANGUAGES") => {
                    consecutive = 0;
                    *detected = Some(tgt.clone());
                    source = Some(tgt.clone());
                    parts.push(chunk);
                }
                Err(e) => {
                    consecutive += 1;
                    outcome.failed += 1;
                    outcome.error.get_or_insert(e);
                    parts.push(offline_translate_fallback(&chunk, &tgt));
                }
            }
        }
        paragraphs.push(parts.join("\n"));
    }
    outcome.text = paragraphs.join("\n\n");
    outcome
}

/// Ask the service over HTTP with `curl` (no TLS stack of our own is linked in).
#[cfg(not(target_arch = "wasm32"))]
pub fn query_online(text: &str, src: &str, tgt: &str) -> Result<Reply, String> {
    let program = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut command = std::process::Command::new(program);
    command
        .args(["-s", "--max-time", "15", "-G", "https://api.mymemory.translated.net/get", "--data-urlencode"])
        .arg(format!("q={text}"))
        .arg("--data-urlencode")
        .arg(format!("langpair={src}|{tgt}"));
    // A window application starting a console program makes Windows flash a console window per call.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|e| format!("curl could not run: {e}"))?;
    if !output.status.success() {
        return Err(format!("no connection to the translation service (curl exit {})", output.status.code().unwrap_or(-1)));
    }
    parse_reply(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_arch = "wasm32")]
pub fn query_online(_text: &str, _src: &str, _tgt: &str) -> Result<Reply, String> {
    Err("online translation is not available in the browser build".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str, detected: Option<&str>) -> Result<Reply, String> {
        Ok(Reply { text: text.into(), detected: detected.map(String::from) })
    }

    /// The service's own 403 for `auto`, as captured from the live API.
    const AUTO_REJECTED: &str = r#"{"responseData":{"translatedText":"'AUTO' IS AN INVALID SOURCE LANGUAGE . EXAMPLE: LANGPAIR=EN|IT USING 2 LETTER ISO OR RFC3066 LIKE ZH-CN."},"quotaFinished":null,"responseDetails":"'AUTO' IS AN INVALID SOURCE LANGUAGE . EXAMPLE: LANGPAIR=EN|IT USING 2 LETTER ISO OR RFC3066 LIKE ZH-CN.","responseStatus":"403","matches":""}"#;

    #[test]
    fn an_error_status_is_an_error_not_a_translation() {
        let e = parse_reply(AUTO_REJECTED).unwrap_err();
        assert!(e.contains("INVALID SOURCE LANGUAGE"), "{e}");
        let ok = parse_reply(r#"{"responseData":{"translatedText":"Bom dia &amp; &#39;ol&aacute;&#39;","detectedLanguage":"en-GB"},"responseDetails":"","responseStatus":200}"#).unwrap();
        assert_eq!(ok.text, "Bom dia & 'ol&aacute;'");
        assert_eq!(ok.detected.as_deref(), Some("en"));
        assert!(parse_reply("not json").is_err());
        assert!(parse_reply(r#"{"responseData":{"translatedText":"x"}}"#).is_err());
        assert!(parse_reply(r#"{"responseData":{"translatedText":"x"},"responseStatus":200,"quotaFinished":true}"#).unwrap_err().contains("quota"));
    }

    #[test]
    fn entities_decode_once() {
        assert_eq!(decode_entities("a &amp;quot; b &lt;i&gt; &#233; &#x41; & c &"), "a &quot; b <i> é A & c &");
    }

    #[test]
    fn chunks_respect_the_limit_and_lose_no_text() {
        let long = "Esta é uma frase de teste do sistema. ".repeat(40);
        let parts = split_chunks(&long, MAX_CHUNK_BYTES);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.len() <= MAX_CHUNK_BYTES), "{:?}", parts.iter().map(String::len).collect::<Vec<_>>());
        let words = |s: &str| s.split_whitespace().count();
        assert_eq!(parts.iter().map(|p| words(p)).sum::<usize>(), words(&long));
        // Lines stay together while they fit.
        assert_eq!(split_chunks("um\ndois\ntrês", MAX_CHUNK_BYTES), vec!["um\ndois\ntrês"]);
        // No spaces and multi-byte characters: cut at character boundaries, never inside one.
        let cjk = "漢".repeat(500);
        let parts = split_chunks(&cjk, MAX_CHUNK_BYTES);
        assert!(parts.iter().all(|p| p.len() <= MAX_CHUNK_BYTES));
        assert_eq!(parts.concat(), cjk);
        assert_eq!(split_chunks("日本語のテキストです。", 3).concat(), "日本語のテキストです。");
        assert!(split_chunks("   \n  ", 450).is_empty());
    }

    #[test]
    fn scripts_name_their_language() {
        assert_eq!(script_language("お支払いを受け取りました。"), Some("ja"));
        assert_eq!(script_language("付款已收到，谢谢。"), Some("zh"));
        assert_eq!(script_language("Платёж получен, спасибо."), Some("ru"));
        assert_eq!(script_language("O pagamento foi recebido."), None);
        assert_eq!(script_language("1.2026.0003814569"), None);
    }

    #[test]
    fn auto_asks_for_autodetect_once_then_reuses_the_language() {
        let mut asked = Vec::new();
        let mut detected = None;
        let mut send = |q: &str, from: &str, to: &str| {
            asked.push((q.to_string(), from.to_string(), to.to_string()));
            ok(&format!("T:{q}"), Some("pt"))
        };
        let first = translate_text("Olá\n\nMundo", "auto", "en", &mut detected, &mut send);
        assert_eq!(first.text, "T:Olá\n\nT:Mundo");
        assert_eq!((first.parts, first.failed), (2, 0));
        assert_eq!(detected.as_deref(), Some("pt"));
        let second = translate_text("Outra página", "auto", "en", &mut detected, &mut send);
        assert_eq!(second.text, "T:Outra página");
        let froms: Vec<_> = asked.iter().map(|a| a.1.as_str()).collect();
        assert_eq!(froms, ["Autodetect", "pt", "pt"]);
        assert!(asked.iter().all(|a| a.2 == "en" && a.1 != "auto"));
    }

    #[test]
    fn same_language_and_scripts_skip_the_service() {
        let mut calls = 0;
        let mut send = |_: &str, _: &str, _: &str| {
            calls += 1;
            ok("x", None)
        };
        let mut detected = None;
        assert_eq!(translate_text("Hello", "en", "en", &mut detected, &mut send).text, "Hello");
        assert_eq!(translate_text("お支払い", "auto", "ja", &mut detected, &mut send).text, "お支払い");
        assert_eq!(calls, 0);
        // A script gives the source without asking.
        let mut seen = String::new();
        let mut send = |_: &str, from: &str, _: &str| {
            seen = from.to_string();
            ok("Payment", None)
        };
        translate_text("お支払いを受け取りました", "auto", "en", &mut detected, &mut send);
        assert_eq!(seen, "ja");
        // The service refusing identical languages means the text is already in the target.
        let mut send = |_: &str, _: &str, _: &str| Err("PLEASE SELECT TWO DISTINCT LANGUAGES".to_string());
        let mut detected = None;
        let same = translate_text("Good morning", "auto", "en", &mut detected, &mut send);
        assert_eq!((same.text.as_str(), same.failed), ("Good morning", 0));
        assert_eq!(detected.as_deref(), Some("en"));
    }

    #[test]
    fn failures_are_counted_reported_and_stop_the_waiting() {
        let mut calls = 0;
        let mut send = |_: &str, _: &str, _: &str| {
            calls += 1;
            Err::<Reply, _>("no connection".to_string())
        };
        let text = (1..=8).map(|n| format!("Parágrafo {n}")).collect::<Vec<_>>().join("\n\n");
        let mut detected = None;
        let out = translate_text(&text, "pt", "en", &mut detected, &mut send);
        assert_eq!((out.parts, out.failed), (8, 8));
        assert_eq!(out.error.as_deref(), Some("no connection"));
        assert_eq!(calls, GIVE_UP_AFTER, "the rest is not tried over the network");
        // The original text is kept, nothing is lost.
        assert!(out.text.contains("Parágrafo 8"));
    }

    /// Against the real service: `cargo test -p karnoxpdf-ui-egui live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_service_translates_with_auto_and_long_text() {
        let text = "O pagamento deve ser feito antes do atendimento presencial. ".repeat(20);
        let mut detected = None;
        let out = translate_text(&text, "auto", "en", &mut detected, &mut query_online);
        assert_eq!(out.failed, 0, "{:?}", out.error);
        assert_eq!(detected.as_deref(), Some("pt"));
        assert!(out.text.to_lowercase().contains("payment"), "{}", out.text);
        assert!(!out.text.contains("INVALID") && !out.text.contains("LIMIT EXCEEDED"));
        let jp = translate_text("お支払いを受け取りました。", "auto", "en", &mut None, &mut query_online);
        assert_eq!(jp.failed, 0, "{:?}", jp.error);
        assert!(jp.text.to_lowercase().contains("payment"), "{}", jp.text);
        let same = translate_text("Good morning, the payment was received.", "auto", "en", &mut None, &mut query_online);
        assert_eq!(same.failed, 0, "{:?}", same.error);
    }
}
