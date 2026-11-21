//! The user's key overrides: a `keys:` map in the app's per-user config file, from a command's
//! dotted name to its keys:
//!
//! ```yaml
//! keys:
//!   view.next: ["Ctrl-L", "]"]
//!   farm.sync: Ctrl-U
//!   app.help:
//!     - F2
//! ```
//!
//! The same file picks the key family: `keys: standard` as a scalar, or `family: standard`
//! inside the map above (not a command's name, so it cannot clash with one). The legacy
//! `vim_mode: false` is an alias for `standard`. The `keys` family beats `vim_mode`, which beats
//! the app's default.
//!
//! A key is written as [`Key::label`] prints it. An override replaces the command's keys in
//! every context it is bound in. Whatever cannot be used (an unknown command, a key that does
//! not parse, an override that would give two commands one key in a context) is left out and
//! reported.
//!
//! The file is read by a small pass over that YAML subset, with no YAML dependency: only
//! whole-line comments, and a punctuation key such as `]` or `,` must be quoted. The rest of the
//! file is the app's and is ignored here.

use super::{Command, Context, Family, Key, Keymap};

/// One `name: keys` entry of the section: the command's name and its raw key strings, or why
/// the entry could not be read.
type Entry = (String, Result<Vec<String>, String>);

pub(super) fn from_config<M: Command>(text: &str, default: Family) -> (Keymap<M>, Vec<String>) {
    let mut km = Keymap::new(default);
    let (scalar, vim_mode, mut problems) = top_level_family(text);
    let mut family = scalar;
    for (name, raw) in parse_section(text) {
        if name == "family" {
            match raw.as_deref() {
                Ok([f]) => match Family::from_name(f) {
                    Some(f) => family = Some(f),
                    None => {
                        problems.push(format!("family: no such family \"{f}\" (vim, standard)"))
                    }
                },
                _ => problems.push("family: one of vim, standard".into()),
            }
            continue;
        }
        let Some(cmd) = M::from_name(&name) else {
            problems.push(format!("{name}: no such command"));
            continue;
        };
        let raw = match raw {
            Ok(raw) => raw,
            Err(why) => {
                problems.push(format!("{name}: {why}"));
                continue;
            }
        };
        if raw.is_empty() {
            problems.push(format!("{name}: no keys"));
            continue;
        }
        let mut keys: Vec<Key> = Vec::new();
        for r in &raw {
            match Key::parse(r) {
                Some(key) if !keys.contains(&key) => keys.push(key),
                Some(_) => {}
                None => problems.push(format!("{name}: no such key \"{r}\"")),
            }
        }
        if !keys.is_empty() {
            km.overrides.insert(cmd, keys);
        }
    }
    km.family = family.or(vim_mode).unwrap_or(default);
    problems.extend(drop_collisions(&mut km));
    (km, problems)
}

/// Drop each override that gives two commands one key in a context and kind of row (the rule
/// the table itself keeps), saying which.
fn drop_collisions<M: Command>(km: &mut Keymap<M>) -> Vec<String> {
    let mut problems = Vec::new();
    while let Some((victim, key, other, ctx)) = collision(km) {
        km.overrides.remove(&victim);
        problems.push(format!(
            "{}: {} is already {}'s in the {ctx}",
            victim.name(),
            key.label(),
            other.name()
        ));
    }
    problems
}

/// One shared key to resolve: the overridden command to drop, the key, the command it clashes
/// with, and the context's name. Two bindings clash when they share a key in a context and their
/// rows overlap (the same kind, or one is for any kind), because a kind's binding shadows a
/// general one, and a shadowed override would silently not work.
fn collision<M: Command>(km: &Keymap<M>) -> Option<(M, Key, M, &'static str)> {
    for &ctx in M::Ctx::ALL {
        let group: Vec<_> = M::bindings().iter().filter(|b| b.ctx == ctx).collect();
        for (i, a) in group.iter().enumerate() {
            for b in &group[i + 1..] {
                let overlap = a.kind == b.kind || a.kind.is_none() || b.kind.is_none();
                if a.cmd == b.cmd || !overlap {
                    continue;
                }
                let Some(key) = km.keys(a).find(|k| km.keys(b).any(|x| x == *k)) else {
                    continue;
                };
                let (victim, other) = if km.overridden(b.cmd) {
                    (b.cmd, a.cmd)
                } else if km.overridden(a.cmd) {
                    (a.cmd, b.cmd)
                } else {
                    continue;
                };
                return Some((victim, *key, other, ctx.title()));
            }
        }
    }
    None
}

/// The entries of the top-level `keys:` section of `text`.
fn parse_section(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let (mut in_keys, mut block_open) = (false, false);
    for line in text.lines() {
        let bare = line.trim();
        if bare.is_empty() || bare.starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            in_keys = bare
                .strip_prefix("keys:")
                .is_some_and(|v| v.trim().is_empty());
            block_open = false;
            continue;
        }
        if !in_keys {
            continue;
        }
        if let Some(item) = bare.strip_prefix('-') {
            if let (true, Some((_, Ok(keys)))) = (block_open, out.last_mut()) {
                keys.push(unquote(item.trim()));
            }
            continue;
        }
        let Some((name, value)) = bare.split_once(':') else {
            continue;
        };
        let (name, value) = (name.trim().to_string(), value.trim());
        block_open = value.is_empty();
        let keys = match value.strip_prefix('[') {
            Some(list) => flow_list(list),
            None if block_open => Ok(Vec::new()),
            None => Ok(vec![unquote(value)]),
        };
        out.push((name, keys));
    }
    out
}

/// The family the config's top level names, and what it got wrong: the scalar
/// `keys: <family>`, else the legacy `vim_mode:`. (`family:` inside the `keys` map is an
/// [`Entry`], read by [`from_config`].)
fn top_level_family(text: &str) -> (Option<Family>, Option<Family>, Vec<String>) {
    let (mut keys, mut vim_mode, mut problems) = (None, None, Vec::new());
    for line in text.lines() {
        let bare = line.trim();
        if bare.is_empty() || bare.starts_with('#') || line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((name, value)) = bare.split_once(':') else {
            continue;
        };
        let value = unquote(value.trim());
        match name.trim() {
            "keys" if !value.is_empty() => match Family::from_name(&value) {
                Some(f) => keys = Some(f),
                None => problems.push(format!("keys: no such family \"{value}\" (vim, standard)")),
            },
            "vim_mode" => {
                vim_mode = Some(Family::from_vim_mode(matches!(
                    value.as_str(),
                    "true" | "True" | "yes" | "1"
                )));
            }
            _ => {}
        }
    }
    (keys, vim_mode, problems)
}

/// A scalar without its YAML quotes (`"x"`, or `'x'` with `''` for a quote).
fn unquote(s: &str) -> String {
    let mut it = s.chars();
    match (it.next(), it.next_back()) {
        (Some(q @ ('"' | '\'')), Some(end)) if q == end && s.len() >= 2 => {
            unescape(&s[1..s.len() - 1], q)
        }
        _ => s.to_string(),
    }
}

fn unescape(inner: &str, quote: char) -> String {
    let mut out = String::new();
    let mut it = inner.chars().peekable();
    while let Some(c) = it.next() {
        match (quote, c, it.peek()) {
            ('\'', '\'', Some('\'')) => {
                it.next();
                out.push('\'');
            }
            ('"', '\\', Some(_)) => out.extend(it.next()),
            _ => out.push(c),
        }
    }
    out
}

/// The items of a flow list whose `[` is already consumed: quoted or bare strings,
/// comma-separated, up to the closing `]`.
fn flow_list(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        let mut chars = rest.chars();
        match chars.next() {
            None => return Err("the list has no closing ]".into()),
            Some(']') => {
                return match rest[1..].trim() {
                    "" => Ok(out),
                    _ => Err("text after the list (quote a ] key: \"]\")".into()),
                };
            }
            Some(q @ ('"' | '\'')) => {
                // Up to the closing quote (a backslash, or a doubled single quote, escapes one).
                let (mut end, mut esc) = (None, false);
                let mut it = rest.char_indices().skip(1).peekable();
                while let Some((i, c)) = it.next() {
                    if esc {
                        esc = false;
                    } else if q == '"' && c == '\\' {
                        esc = true;
                    } else if c == q {
                        if q == '\'' && it.peek().is_some_and(|(_, n)| *n == '\'') {
                            it.next();
                        } else {
                            end = Some(i);
                            break;
                        }
                    }
                }
                let end = end.ok_or("an unclosed quote")?;
                out.push(unescape(&rest[1..end], q));
                rest = &rest[end + 1..];
            }
            Some(_) => {
                let end = rest.find([',', ']']).unwrap_or(rest.len());
                out.push(rest[..end].trim().to_string());
                rest = &rest[end..];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_section_reads_flow_block_and_scalar_forms() {
        let text = "herd: yak\nkeys:\n  # a comment\n  view.next: [\"Tab\", \"]\", ',', x]\n  farm.sync: Ctrl-U\n  app.help:\n    - F(1)\n    - \"?\"\nother:\n  app.quit: z\n";
        let got = parse_section(text);
        let ok =
            |n: &str, v: &[&str]| (n.to_string(), Ok(v.iter().map(|s| s.to_string()).collect()));
        assert_eq!(
            got,
            vec![
                ok("view.next", &["Tab", "]", ",", "x"]),
                ok("farm.sync", &["Ctrl-U"]),
                ok("app.help", &["F(1)", "?"]),
            ]
        );
        assert!(parse_section("keys:\n  a.b: [Tab, ]]\n")[0].1.is_err());
        assert!(parse_section("keys:\n  a.b: [\"Tab\n")[0].1.is_err());
    }
}
