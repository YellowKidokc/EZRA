use regex::Regex;
use std::{path::Path, sync::LazyLock};
use uuid::Uuid;

#[derive(Debug)]
pub enum Anchor {
    Absent,
    Valid(String),
    Conflict,
}

static HTML_META: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<meta\b(?:[^>"']|"[^"]*"|'[^']*')*>"#).unwrap());
static HTML_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)\s(name|content)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).unwrap()
});
static HTML_HEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<head(?:\s[^>]*)?>"#).unwrap());
static HTML_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)</head\s*>"#).unwrap());
static HTML_HIDDEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<!--.*?(?:-->|$)|<script\b[^>]*>.*?</script\s*>|<style\b[^>]*>.*?</style\s*>|<title\b[^>]*>.*?</title\s*>|<template\b[^>]*>.*?</template\s*>"#,
    )
    .unwrap()
});

pub fn supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|x| x.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "htm" | "html")
    )
}

fn markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("md") || x.eq_ignore_ascii_case("markdown"))
}

fn bounds(s: &str) -> Option<(usize, usize)> {
    let start = s.find('\n')? + 1;
    let mut offset = start;
    for line in s[start..].split_inclusive('\n') {
        if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            return Some((start, offset));
        }
        offset += line.len();
    }
    None
}

fn visible_html(s: &str) -> String {
    HTML_HIDDEN
        .replace_all(s, |m: &regex::Captures<'_>| " ".repeat(m[0].len()))
        .into_owned()
}

pub fn read(path: &Path, bytes: &[u8]) -> Anchor {
    let Ok(s) = std::str::from_utf8(bytes) else {
        return Anchor::Conflict;
    };
    let s = s.trim_start_matches('\u{feff}');
    let mut values = Vec::new();
    if markdown(path) {
        if matches!(s.lines().next(), Some("---")) {
            let Some((start, end)) = bounds(s) else {
                return Anchor::Conflict;
            };
            let Ok(frontmatter) = serde_yaml::from_str::<serde_yaml::Value>(&s[start..end]) else {
                return Anchor::Conflict;
            };
            match frontmatter {
                serde_yaml::Value::Mapping(fields) => {
                    for field in ["record_id", "uuid"] {
                        if let Some(value) = fields.get(field) {
                            let Some(value) = value.as_str() else {
                                return Anchor::Conflict;
                            };
                            values.push(value.to_owned());
                        }
                    }
                }
                serde_yaml::Value::Null => {}
                _ => return Anchor::Conflict,
            }
        }
    } else {
        let visible = visible_html(s);
        if let Some(head) = HTML_HEAD.find(&visible) {
            let Some(end) = HTML_CLOSE.find_at(&visible, head.end()) else {
                return Anchor::Conflict;
            };
            for tag in HTML_META.find_iter(&visible[head.end()..end.start()]) {
                let mut names = Vec::new();
                let mut contents = Vec::new();
                for a in HTML_ATTR.captures_iter(tag.as_str()) {
                    let value = a.get(2).or(a.get(3)).or(a.get(4)).unwrap().as_str();
                    if a[1].eq_ignore_ascii_case("name") {
                        names.push(value);
                    } else {
                        contents.push(value);
                    }
                }
                if names
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case("record_id") || n.eq_ignore_ascii_case("uuid"))
                {
                    if names.len() != 1 || contents.len() != 1 {
                        return Anchor::Conflict;
                    }
                    values.push(contents[0].to_owned());
                }
            }
        }
    }
    let mut ids = values.iter().map(|s| Uuid::parse_str(s));
    match ids.next() {
        None => Anchor::Absent,
        Some(Ok(first)) if ids.all(|id| id == Ok(first)) => Anchor::Valid(first.to_string()),
        _ => Anchor::Conflict,
    }
}

pub fn insert(path: &Path, bytes: &[u8], id: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let s = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bom = text.len() - s.len();
    let newline = if s.find('\n').is_some_and(|i| s[..i].ends_with('\r')) {
        "\r\n"
    } else {
        "\n"
    };
    let (offset, addition) = if markdown(path) {
        if matches!(s.lines().next(), Some("---")) {
            (bounds(s)?.0, format!("record_id: {id}{newline}"))
        } else {
            (
                0,
                format!("---{newline}record_id: {id}{newline}---{newline}"),
            )
        }
    } else {
        let visible = visible_html(s);
        let head = HTML_HEAD.find(&visible)?;
        HTML_CLOSE.find_at(&visible, head.end())?;
        (
            head.end(),
            format!("{newline}<meta name=\"record_id\" content=\"{id}\">"),
        )
    };
    let offset = offset + bom;
    let mut result = bytes[..offset].to_vec();
    result.extend_from_slice(addition.as_bytes());
    result.extend_from_slice(&bytes[offset..]);
    Some(result)
}
