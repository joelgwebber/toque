//! The rules a table should keep, as checks an app's tests call. Each returns the problems it
//! found, one line each, so a test reads `assert!(rules::x::<Cmd>().is_empty(), ...)` and fails
//! naming every offender at once. They are rules, not requirements: an app with a reason to
//! break one simply doesn't call it.

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::{Command, Context, Family, Key, Keymap};

/// No two bindings in one context, for one kind of row, share a key: the lookup would only ever
/// find the first. `kinds` lists every row kind, `None` (any kind) included.
#[must_use]
pub fn no_shared_keys<M: Command>(kinds: &[Option<M::Kind>]) -> Vec<String> {
    let mut problems = Vec::new();
    for &ctx in M::Ctx::ALL {
        for &kind in kinds {
            let mut seen: Vec<(Key, M)> = Vec::new();
            for b in M::bindings()
                .iter()
                .filter(|b| b.ctx == ctx && b.kind == kind)
            {
                for key in b.keys {
                    if let Some((_, other)) = seen.iter().find(|(s, _)| s == key) {
                        problems.push(format!(
                            "{ctx:?} {kind:?}: {} is bound to {} and {}",
                            key.label(),
                            other.name(),
                            b.cmd.name()
                        ));
                    }
                    seen.push((*key, b.cmd));
                }
            }
        }
    }
    problems
}

/// Every command is bound somewhere, names are unique and dotted, and every group a command
/// names is in [`Command::GROUPS`].
#[must_use]
pub fn every_command_bound_and_named<M: Command>() -> Vec<String> {
    let mut problems = Vec::new();
    for cmd in M::ALL {
        if !M::bindings().iter().any(|b| b.cmd == *cmd) {
            problems.push(format!("{} is bound nowhere", cmd.name()));
        }
        if !cmd.name().contains('.') {
            problems.push(format!("{} is not a dotted name", cmd.name()));
        }
        if !M::GROUPS.contains(&cmd.group_title()) {
            problems.push(format!(
                "{}: no group {:?} in GROUPS",
                cmd.name(),
                cmd.group_title()
            ));
        }
    }
    let mut names: Vec<&str> = M::ALL.iter().map(|c| c.name()).collect();
    names.sort_unstable();
    for pair in names.windows(2) {
        if pair[0] == pair[1] {
            problems.push(format!("two commands are named {}", pair[0]));
        }
    }
    problems
}

/// Every key in the table reads back from its own label, so a user can write any key the help
/// shows.
#[must_use]
pub fn every_key_parses_from_its_label<M: Command>() -> Vec<String> {
    M::bindings()
        .iter()
        .flat_map(|b| b.keys)
        .filter(|k| Key::parse(&k.label()) != Some(**k))
        .map(|k| format!("{} does not parse back", k.label()))
        .collect()
}

/// Case is a tier: lowercase looks and moves, Shift changes, Ctrl is the app. A command that
/// [`mutates`](Command::mutates) must not sit on an unshifted character key, unless it is one of
/// the `exceptions` (each with why, for the test to document); an exception no longer on a
/// lowercase key is reported as stale.
#[must_use]
pub fn lowercase_only_looks<M: Command>(exceptions: &[(M, &str)]) -> Vec<String> {
    let lowercase = |k: &Key| {
        matches!(k.code, KeyCode::Char(ch) if !ch.is_uppercase())
            && !k
                .mods
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    };
    let mut problems = Vec::new();
    for b in M::bindings().iter().filter(|b| b.cmd.mutates()) {
        for key in b.keys.iter().filter(|k| lowercase(k)) {
            if !exceptions.iter().any(|(cmd, _)| *cmd == b.cmd) {
                problems.push(format!(
                    "{} ({:?}) is on the lowercase key {}: Shift changes things, lowercase only looks",
                    b.cmd.name(),
                    b.ctx,
                    key.label()
                ));
            }
        }
    }
    for (cmd, why) in exceptions {
        if !M::bindings()
            .iter()
            .any(|b| b.cmd == *cmd && b.keys.iter().any(lowercase))
        {
            problems.push(format!(
                "{} is no longer on a lowercase key: drop its exception ({why})",
                cmd.name()
            ));
        }
    }
    problems
}

/// The standard family is the same commands on other keys: every binding with a key in vim keeps
/// one in standard, no key left on in standard is vim-only (or vice versa), and every context
/// lists the same commands in both. `kinds` lists every row kind, `None` included.
#[must_use]
pub fn every_command_bound_in_both_families<M: Command>(kinds: &[Option<M::Kind>]) -> Vec<String> {
    let (vim, std) = (
        Keymap::<M>::new(Family::Vim),
        Keymap::<M>::new(Family::Standard),
    );
    let mut problems = Vec::new();
    for b in M::bindings() {
        if vim.keys(b).next().is_some() && std.keys(b).next().is_none() {
            problems.push(format!(
                "{} ({:?}, {:?}) has no key in standard (it has {})",
                b.cmd.name(),
                b.ctx,
                b.kind,
                vim.labels(b, "/")
            ));
        }
        for key in std.keys(b).filter(|k| M::VIM_ONLY.contains(k)) {
            problems.push(format!(
                "{} keeps the vim-only key {}",
                b.cmd.name(),
                key.label()
            ));
        }
        for key in vim.keys(b).filter(|k| M::STANDARD_ONLY.contains(k)) {
            problems.push(format!(
                "{} keeps the standard-only key {}",
                b.cmd.name(),
                key.label()
            ));
        }
    }
    for &ctx in M::Ctx::ALL {
        for &kind in kinds {
            let names = |km: &Keymap<M>| -> Vec<&'static str> {
                km.popup_entries(ctx, kind)
                    .into_iter()
                    .flat_map(|(_, e)| e.into_iter().map(|(_, n)| n))
                    .collect()
            };
            if names(&vim) != names(&std) {
                problems.push(format!(
                    "{ctx:?} {kind:?}: the families list different commands"
                ));
            }
        }
    }
    problems
}
