use crate::{index, resolver};
use aho_corasick::{AhoCorasick, MatchKind};
use anyhow::{Context, Result, bail};
use regex::Regex;
use rusqlite::Connection;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::LazyLock,
};

static MARKDOWN_SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?ms)\A\x{FEFF}?---\r?\n.*?^(?:---|\.\.\.)[ \t]*\r?(?:\n|$)|^ {0,3}(?:```|~~~)[^\n]*\n.*?^ {0,3}(?:```|~~~)[^\n]*(?:\n|$)|`[^`\r\n]*`|!?\[[^\]\n]*\](?:\([^\n]*?\)|\[[^\]\n]*\])|<[^>\n]+>|^ {0,3}\[[^\]\n]+\]:[^\n]*|\b[a-zA-Z][a-zA-Z0-9+.-]*://[^\s<>]+").unwrap()
});
static HTML_SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<!--.*?(?:-->|$)|<(?:a|script|style|code|pre|textarea|title)\b[^>]*>.*?</(?:a|script|style|code|pre|textarea|title)\s*>|<(?:[^>"']|"[^"]*"|'[^']*')+>|&(?:#[xX]?[0-9A-Fa-f]+|[A-Za-z][A-Za-z0-9]+);"#).unwrap()
});

// Track offsets because lowercase Unicode characters can change byte length.
fn fold_with_offsets(text: &str) -> (String, Vec<Option<usize>>) {
    let mut folded = String::with_capacity(text.len());
    let mut offsets = Vec::with_capacity(text.len() + 1);
    for (index, character) in text.char_indices() {
        offsets.push(Some(index));
        let previous_len = folded.len();
        folded.extend(character.to_lowercase());
        offsets.extend(std::iter::repeat_n(None, folded.len() - previous_len - 1));
    }
    offsets.push(Some(text.len()));
    (folded, offsets)
}

pub fn run(db: &Connection, input: &Path, out: &Path) -> Result<PathBuf> {
    let ext = input
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    let html = match ext.as_deref() {
        Some("html" | "htm") => true,
        Some("md" | "markdown") => false,
        _ => bail!("Linkify supports Markdown and HTML"),
    };
    let input = crate::platform::absolute(input)?;
    let text = fs::read_to_string(&input)?;
    let mut data: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut disputed = db.prepare(
        "SELECT record_id FROM locations WHERE is_current=1 AND status='conflict' AND record_id IS NOT NULL
         UNION SELECT claimed_id FROM locations WHERE is_current=1 AND status='conflict' AND claimed_id IS NOT NULL",
    )?;
    let disputed = disputed
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    let mut query = db.prepare(
        "SELECT alias,record_id FROM aliases
         UNION ALL SELECT path,record_id FROM locations WHERE is_current=1 AND status='active' AND record_id IS NOT NULL",
    )?;
    for row in query.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (term, id) = row?;
        data.entry(index::lookup_key(&term)).or_default().insert(id);
    }
    let terms: Vec<_> = data.values().collect();
    let matcher = if terms.is_empty() {
        None
    } else {
        Some(
            AhoCorasick::builder()
                .match_kind(MatchKind::LeftmostLongest)
                .build(data.keys())
                .context("Compile indexed link terms")?,
        )
    };
    let skip = if html { &*HTML_SKIP } else { &*MARKDOWN_SKIP };
    let mut result = String::new();
    let mut ambiguous = BTreeSet::new();
    let mut cursor = 0;
    let mut rewrite = |start: usize, end: usize, result: &mut String| {
        let text = &text[start..end];
        let mut cursor = 0;
        let (folded, offsets) = fold_with_offsets(text);
        for matched in matcher
            .iter()
            .flat_map(|matcher| matcher.find_iter(&folded))
        {
            let (Some(start), Some(end)) = (offsets[matched.start()], offsets[matched.end()])
            else {
                continue;
            };
            let name = &text[start..end];
            let word = |ch: char| ch.is_alphanumeric() || ch == '_';
            if (name.starts_with(word) && text[..start].ends_with(word))
                || (name.ends_with(word) && text[end..].starts_with(word))
            {
                continue;
            }
            let ids = terms[matched.pattern().as_usize()];
            if ids.len() != 1 || ids.first().is_some_and(|id| disputed.contains(id)) {
                ambiguous.insert(name.to_owned());
                continue;
            }
            let id = ids.first().unwrap();
            result.push_str(&text[cursor..start]);
            let uri = resolver::link(id);
            if html {
                result.push_str(&format!("<a href=\"{uri}\">{}</a>", resolver::escape(name)));
            } else {
                let safe = name
                    .replace('\\', "\\\\")
                    .replace('[', "\\[")
                    .replace(']', "\\]");
                result.push_str(&format!("[{safe}]({uri})"));
            }
            cursor = end;
        }
        result.push_str(&text[cursor..]);
    };
    for m in skip.find_iter(&text) {
        rewrite(cursor, m.start(), &mut result);
        result.push_str(m.as_str());
        cursor = m.end();
    }
    rewrite(cursor, text.len(), &mut result);

    fs::create_dir_all(out)?;
    let destination = out.join(input.file_name().context("Source file name missing")?);
    let mut temp = tempfile::NamedTempFile::new_in(out)?;
    temp.write_all(result.as_bytes())?;
    temp.persist_noclobber(&destination)
        .with_context(|| format!("Refusing to overwrite output {}", destination.display()))?;
    for term in ambiguous {
        eprintln!("Needs review, ambiguous link text: {term}");
    }
    Ok(destination)
}
