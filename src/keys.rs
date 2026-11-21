//! A command table for a ratatui app's keys: one place that binds them, from which the lookup,
//! the `?` popup, the full reference and the bottom hint are all generated, so a key cannot be
//! bound without being listed and the four cannot drift.
//!
//! The app owns its commands, contexts and row kinds; this module owns the mechanism:
//!
//! - A [`Command`] is one intent with a stable dotted name (`view.next`, `queue.remove`): the
//!   name a user's override uses and the `?` popup prints. It names its [`Context`] type (a
//!   surface that takes keys: a list, a pane, a viewer), its row kind type, and its table.
//! - A [`Binding`] says: in this context (for this kind of row, or any), these [`Key`]s run this
//!   command, with a line for the reference and a label for the hint bar.
//! - A [`Keymap`] is the table in effect: filtered by the user's key [`Family`] (vim or
//!   standard) and with their overrides applied ([`Keymap::from_config`]). Everything that reads
//!   a key goes through it.
//!
//! The lookup ([`Keymap::lookup`]): a binding that names modifiers is tried before a plain one
//! (so Ctrl-C is quit, not `c`); a key with extra modifiers still matches its plain binding (an
//! upper-case letter arrives with Shift), except Ctrl, which never rides along; and a binding for
//! the selected row's kind beats one for any kind.
//!
//! The rules an app's table should keep are functions in [`rules`], for its tests to call.

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

mod config;
pub mod rules;

/// A surface that takes keys when no modal overlay is up.
pub trait Context: Copy + Eq + Debug + 'static {
    /// Every context.
    const ALL: &'static [Self];
    /// What a message calls it (`list`, `detail pane`).
    fn title(self) -> &'static str;
}

/// One action a key can run.
pub trait Command: Copy + Eq + Hash + Debug + 'static {
    /// The surfaces its bindings name.
    type Ctx: Context;
    /// The kinds of row a list binding may be for. An app with one kind uses `()`.
    type Kind: Copy + Eq + Debug + 'static;

    /// Every command.
    const ALL: &'static [Self];
    /// The `?` popup's groups, in order, by title.
    const GROUPS: &'static [&'static str];
    /// The keys only the vim family has: off in standard.
    const VIM_ONLY: &'static [Key] = VIM_ONLY;
    /// The keys only the standard family has: off in vim.
    const STANDARD_ONLY: &'static [Key] = STANDARD_ONLY;

    /// The stable dotted name.
    fn name(self) -> &'static str;
    /// The title of its group in the `?` popup: one of [`Command::GROUPS`].
    fn group_title(self) -> &'static str;
    /// Whether it changes something that persists; the case rule
    /// ([`rules::lowercase_only_looks`]) is checked over these.
    fn mutates(self) -> bool;
    /// The table: every binding, in the hint's and the reference's order within a context.
    fn bindings() -> &'static [Binding<Self>];

    /// The command a dotted name stands for.
    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}

/// The vim family's own keys (`hjkl`, `d`/`u` half pages, `g`/`G`, `Ctrl-N`/`Ctrl-P`), off in
/// standard. A command bound to one of these needs another key too, which
/// [`rules::every_command_bound_in_both_families`] checks.
pub const VIM_ONLY: &[Key] = &[
    c('h'),
    c('j'),
    c('k'),
    c('l'),
    c('d'),
    c('u'),
    c('g'),
    c('G'),
    ctrl('n'),
    ctrl('p'),
];

/// The standard family's own keys (`Ctrl-F` find), off in vim.
pub const STANDARD_ONLY: &[Key] = &[ctrl('f')];

/// The key family, chosen once by the user: it decides which of the table's keys are on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Family {
    /// `hjkl`, `d`/`u`, `g`/`G`, `Ctrl-N`/`Ctrl-P`.
    #[default]
    Vim,
    /// Arrows, `PgUp`/`PgDn`, `Home`/`End`, `Ctrl-F` find.
    Standard,
}

impl Family {
    /// The family a config value names (`vim`, `standard`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Family> {
        match name.trim() {
            "vim" => Some(Family::Vim),
            "standard" => Some(Family::Standard),
            _ => None,
        }
    }

    /// The legacy `vim_mode: true|false` setting as a family.
    #[must_use]
    pub fn from_vim_mode(vim: bool) -> Family {
        if vim { Family::Vim } else { Family::Standard }
    }

    #[must_use]
    pub fn is_vim(self) -> bool {
        self == Family::Vim
    }

    /// Whether the table's key `key` is on in this family.
    fn allows<M: Command>(self, key: &Key) -> bool {
        match self {
            Family::Vim => !M::STANDARD_ONLY.contains(key),
            Family::Standard => !M::VIM_ONLY.contains(key),
        }
    }
}

/// A key: a code and the modifiers that must be held with it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl Key {
    /// Whether a key event is this key. Ctrl never rides along on a key that does not name it
    /// (`Ctrl-N` is not `n`), so a key a family turns off goes dead rather than falling through
    /// to its plain letter; Ctrl with Alt is AltGr on some layouts and counts as plain. Other
    /// extra modifiers are ignored (an upper-case letter arrives with Shift).
    #[must_use]
    pub fn matches(&self, k: &KeyEvent) -> bool {
        let ctrl_only =
            k.modifiers.contains(KeyModifiers::CONTROL) && !k.modifiers.contains(KeyModifiers::ALT);
        if ctrl_only && !self.mods.contains(KeyModifiers::CONTROL) {
            return false;
        }
        self.code == k.code && k.modifiers.contains(self.mods)
    }

    /// How help writes it: `j`, `↓`, `Space`, `Ctrl-N`, `S-Tab`, `F3`.
    #[must_use]
    pub fn label(&self) -> String {
        let ctrl = self.mods.contains(KeyModifiers::CONTROL);
        let alt = self.mods.contains(KeyModifiers::ALT);
        let name = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) if ctrl || alt => c.to_ascii_uppercase().to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Up => "↑".into(),
            KeyCode::Down => "↓".into(),
            KeyCode::Left => "←".into(),
            KeyCode::Right => "→".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::BackTab => "S-Tab".into(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::PageUp => "PgUp".into(),
            KeyCode::PageDown => "PgDn".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::F(n) => format!("F{n}"),
            other => format!("{other:?}"),
        };
        let mut label = String::new();
        if ctrl {
            label.push_str("Ctrl-");
        }
        if alt {
            label.push_str("Alt-");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            label.push_str("Shift-");
        }
        label + &name
    }

    /// The inverse of [`Key::label`] (what an override writes), with `Up`/`Down`/`Left`/`Right`
    /// as ASCII spellings of the arrows and `F(3)` as an older spelling of `F3`. A `Ctrl-` or
    /// `Alt-` letter is read as the lower-case key, as a table spells it.
    #[must_use]
    pub fn parse(s: &str) -> Option<Key> {
        let mut rest = s;
        let mut mods = KeyModifiers::NONE;
        loop {
            let (r, m) = if let Some(r) = rest.strip_prefix("Ctrl-") {
                (r, KeyModifiers::CONTROL)
            } else if let Some(r) = rest.strip_prefix("Shift-") {
                (r, KeyModifiers::SHIFT)
            } else if let Some(r) = rest.strip_prefix("Alt-") {
                (r, KeyModifiers::ALT)
            } else {
                break;
            };
            if r.is_empty() {
                break;
            }
            mods |= m;
            rest = r;
        }
        let mut chars = rest.chars();
        let code = match rest {
            "Space" => KeyCode::Char(' '),
            "Tab" => KeyCode::Tab,
            "S-Tab" => KeyCode::BackTab,
            "Enter" => KeyCode::Enter,
            "Esc" => KeyCode::Esc,
            "\u{2191}" | "Up" => KeyCode::Up,
            "\u{2193}" | "Down" => KeyCode::Down,
            "\u{2190}" | "Left" => KeyCode::Left,
            "\u{2192}" | "Right" => KeyCode::Right,
            "PgUp" => KeyCode::PageUp,
            "PgDn" => KeyCode::PageDown,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "Backspace" => KeyCode::Backspace,
            "Delete" => KeyCode::Delete,
            "Insert" => KeyCode::Insert,
            _ => match (chars.next(), chars.next()) {
                (Some(ch), None) if mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    KeyCode::Char(ch.to_ascii_lowercase())
                }
                (Some(ch), None) => KeyCode::Char(ch),
                _ => {
                    let n = match rest.strip_prefix("F(") {
                        Some(r) => r.strip_suffix(')')?,
                        None => rest.strip_prefix('F')?,
                    };
                    KeyCode::F(n.parse().ok()?)
                }
            },
        };
        Some(Key { code, mods })
    }
}

/// A plain character key.
#[must_use]
pub const fn c(ch: char) -> Key {
    Key {
        code: KeyCode::Char(ch),
        mods: KeyModifiers::NONE,
    }
}

/// A plain key by code (`Enter`, `Down`, `F(1)`).
#[must_use]
pub const fn k(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

/// Ctrl and a character.
#[must_use]
pub const fn ctrl(ch: char) -> Key {
    Key {
        code: KeyCode::Char(ch),
        mods: KeyModifiers::CONTROL,
    }
}

/// Ctrl and a key by code.
#[must_use]
pub const fn ctrl_k(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::CONTROL,
    }
}

/// Shift and a key by code (`Shift-↓`; a capital letter is just `c('S')`).
#[must_use]
pub const fn shift(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::SHIFT,
    }
}

/// What a binding adds to the bottom hint bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hint {
    /// Nothing.
    None,
    /// This label (`Tab:view`), in the table's order.
    Text(&'static str),
    /// A label that differs by family: the vim one (`j/k:move`), then the standard one
    /// (`↑/↓:move`).
    Family(&'static str, &'static str),
    /// A label only the app can write (it depends on its state): [`Keymap::hint_for`] asks its
    /// callback for the label of this tag.
    Custom(&'static str),
}

/// One row of the table: in `ctx`, on a row of `kind` (or any kind, for `None`), any of `keys`
/// runs `cmd`.
pub struct Binding<M: Command> {
    pub ctx: M::Ctx,
    pub kind: Option<M::Kind>,
    pub keys: &'static [Key],
    pub cmd: M,
    /// The reference's line for it; empty: not listed.
    pub help: &'static str,
    pub hint: Hint,
}

impl<M: Command> Clone for Binding<M> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M: Command> Copy for Binding<M> {}
impl<M: Command> Debug for Binding<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Binding")
            .field("ctx", &self.ctx)
            .field("kind", &self.kind)
            .field("keys", &self.keys)
            .field("cmd", &self.cmd)
            .finish_non_exhaustive()
    }
}

/// A binding for any kind of row.
#[must_use]
pub const fn bind<M: Command>(
    ctx: M::Ctx,
    keys: &'static [Key],
    cmd: M,
    help: &'static str,
    hint: Hint,
) -> Binding<M> {
    Binding {
        ctx,
        kind: None,
        keys,
        cmd,
        help,
        hint,
    }
}

/// A binding for one kind of row.
#[must_use]
pub const fn bind_kind<M: Command>(
    ctx: M::Ctx,
    kind: M::Kind,
    keys: &'static [Key],
    cmd: M,
    help: &'static str,
    hint: Hint,
) -> Binding<M> {
    Binding {
        ctx,
        kind: Some(kind),
        keys,
        cmd,
        help,
        hint,
    }
}

/// The `?` popup's content: titled groups of (keys, command name) entries.
pub type PopupGroups = Vec<(&'static str, Vec<(String, &'static str)>)>;

/// The keys in effect: the table's, filtered by the key [`Family`], with the user's overrides
/// applied. An override replaces a command's keys in every context it is bound in, and is the
/// user's word: the family does not filter it. Everything that reads keys goes through it, so
/// none can show a key the lookup does not take.
#[derive(Clone, Debug)]
pub struct Keymap<M: Command> {
    pub family: Family,
    overrides: HashMap<M, Vec<Key>>,
}

impl<M: Command> Default for Keymap<M> {
    fn default() -> Self {
        Self::new(Family::default())
    }
}

impl<M: Command> Keymap<M> {
    /// The table's keys in `family`, with no overrides.
    #[must_use]
    pub fn new(family: Family) -> Self {
        Self {
            family,
            overrides: HashMap::new(),
        }
    }

    /// Whether the user rebound `cmd`.
    #[must_use]
    pub fn overridden(&self, cmd: M) -> bool {
        self.overrides.contains_key(&cmd)
    }

    /// The keys of `b` in effect.
    pub fn keys<'a>(&'a self, b: &'a Binding<M>) -> impl Iterator<Item = &'a Key> + 'a {
        let (keys, own) = match self.overrides.get(&b.cmd) {
            Some(v) => (v.as_slice(), true),
            None => (b.keys, false),
        };
        keys.iter()
            .filter(move |k| own || self.family.allows::<M>(k))
    }

    /// `b`'s keys in effect, their labels joined by `sep`.
    #[must_use]
    pub fn labels(&self, b: &Binding<M>, sep: &str) -> String {
        self.keys(b).map(Key::label).collect::<Vec<_>>().join(sep)
    }

    /// The command `k` runs in `ctx` on a row of `kind` (`None` where there is no row): a
    /// binding that names modifiers first, else a plain one; at each step a binding for the
    /// row's kind beats one for any kind.
    #[must_use]
    pub fn lookup(&self, ctx: M::Ctx, kind: Option<M::Kind>, k: &KeyEvent) -> Option<M> {
        let find = |modified: bool, kind: Option<M::Kind>| {
            M::bindings()
                .iter()
                .filter(|b| b.ctx == ctx && b.kind == kind)
                .find(|b| {
                    self.keys(b)
                        .any(|key| key.mods.is_empty() != modified && key.matches(k))
                })
                .map(|b| b.cmd)
        };
        [true, false].into_iter().find_map(|modified| {
            kind.and_then(|kind| find(modified, Some(kind)))
                .or_else(|| find(modified, None))
        })
    }

    /// The bindings that act in `ctx` on a row of `kind`, in table order: those for this kind,
    /// and those for any kind except where one for this kind takes their keys.
    pub fn visible(
        &self,
        ctx: M::Ctx,
        kind: Option<M::Kind>,
    ) -> impl Iterator<Item = &'static Binding<M>> + '_ {
        let all = M::bindings();
        all.iter().filter(move |b| {
            b.ctx == ctx
                && match (b.kind, kind) {
                    (None, None) => true,
                    (None, Some(kind)) => !all.iter().any(|o| {
                        o.ctx == ctx
                            && o.kind == Some(kind)
                            && self.keys(o).any(|k| self.keys(b).any(|x| x == k))
                    }),
                    (b_kind, kind) => b_kind == kind,
                }
        })
    }

    /// The first key of `cmd` in `ctx`, as a hint writes it (`f`); `?` when it has none.
    #[must_use]
    pub fn first_label(&self, ctx: M::Ctx, cmd: M) -> String {
        M::bindings()
            .iter()
            .find(|b| b.ctx == ctx && b.cmd == cmd)
            .and_then(|b| self.keys(b).next())
            .map_or_else(|| "?".into(), Key::label)
    }

    /// The bottom bar's key hints for `ctx` on a row of `kind`: each hinted binding's label, in
    /// table order, two spaces apart. A command whose keys are overridden shows them in place of
    /// the label's own (`Tab:view` becomes `Ctrl-N:view`). `custom` writes a
    /// [`Hint::Custom`] label (it is given the tag and this keymap).
    pub fn hint_for(
        &self,
        ctx: M::Ctx,
        kind: Option<M::Kind>,
        custom: impl Fn(&'static str, &Self) -> Option<String>,
    ) -> String {
        self.visible(ctx, kind)
            .filter_map(|b| {
                let label = match b.hint {
                    Hint::None => return None,
                    Hint::Text(t) => t,
                    Hint::Family(vim, standard) => {
                        if self.family.is_vim() {
                            vim
                        } else {
                            standard
                        }
                    }
                    Hint::Custom(tag) => return custom(tag, self),
                };
                if !self.overridden(b.cmd) {
                    return Some(label.to_string());
                }
                let what = label.split_once(':').map_or(label, |(_, what)| what);
                let keys = self.labels(b, "/");
                (!keys.is_empty()).then(|| format!("{keys}:{what}"))
            })
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// The `?` popup's groups for `ctx` on a row of `kind`: each binding's keys and its command's
    /// name, under its command's group, in [`Command::GROUPS`] order; an empty group is left out.
    #[must_use]
    pub fn popup_entries(&self, ctx: M::Ctx, kind: Option<M::Kind>) -> PopupGroups {
        M::GROUPS
            .iter()
            .filter_map(|&g| {
                let entries: Vec<_> = self
                    .visible(ctx, kind)
                    .filter(|b| b.cmd.group_title() == g && self.keys(b).next().is_some())
                    .map(|b| (self.labels(b, "/"), b.cmd.name()))
                    .collect();
                (!entries.is_empty()).then_some((g, entries))
            })
            .collect()
    }

    /// The overrides in the `keys:` section of a user config `text`, and what was left out;
    /// the family is the config's own, else `default`. See [`config`] for the format.
    #[must_use]
    pub fn from_config(text: &str, default: Family) -> (Self, Vec<String>) {
        config::from_config(text, default)
    }

    /// The user's keymap from the config file at `path`: the table's keys in `default` when
    /// there is no file. The notice says, in one line, what was left out.
    #[must_use]
    pub fn load(path: &Path, default: Family) -> (Self, Option<String>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return (Self::new(default), None);
        };
        let (km, problems) = Self::from_config(&text, default);
        let more = problems.len().saturating_sub(2);
        let more = if more > 0 {
            format!("; +{more} more")
        } else {
            String::new()
        };
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |f| f.to_string_lossy().into(),
        );
        let notice = (!problems.is_empty()).then(|| {
            let shown = problems.iter().take(2).cloned().collect::<Vec<_>>();
            format!("keys in {file}: {}{more} (ignored)", shown.join("; "))
        });
        (km, notice)
    }
}

/// A section of the full reference: the bindings of `ctx` whose kind is in `kinds` (`None`
/// standing for the bindings for any kind).
pub struct Section<M: Command> {
    pub ctx: M::Ctx,
    pub kinds: &'static [Option<M::Kind>],
    pub title: &'static str,
}

impl<M: Command> Section<M> {
    /// Its bindings, in table order.
    pub fn bindings(&self) -> impl Iterator<Item = &'static Binding<M>> + '_ {
        M::bindings()
            .iter()
            .filter(|b| b.ctx == self.ctx && self.kinds.contains(&b.kind))
    }
}

/// A titled list of rows for the reference that are not bindings: (label, text).
pub type Extra = (&'static str, &'static [(&'static str, &'static str)]);

/// The look every app's help shares: section titles, the key column, quiet notes.
fn title_style() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}
fn key_style() -> Style {
    Style::new().fg(Color::Yellow)
}
fn quiet_style() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// The full key reference: a block per section, each binding with a help line as its effective
/// keys and that line, then the `extras`; a blank line between blocks.
#[must_use]
pub fn reference<M: Command>(
    km: &Keymap<M>,
    sections: &[Section<M>],
    extras: &[Extra],
) -> Vec<Line<'static>> {
    fn entry(key: &str, desc: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {key:<16}"), key_style()),
            Span::raw(desc.to_string()),
        ])
    }
    let mut lines = Vec::new();
    for sect in sections {
        lines.push(Line::from(Span::styled(sect.title, title_style())));
        for b in sect.bindings().filter(|b| !b.help.is_empty()) {
            lines.push(entry(&km.labels(b, " / "), b.help));
        }
        lines.push(Line::from(String::new()));
    }
    for (title, rows) in extras {
        lines.push(Line::from(Span::styled(*title, title_style())));
        lines.extend(rows.iter().map(|(key, desc)| entry(key, desc)));
        lines.push(Line::from(String::new()));
    }
    lines.pop();
    lines
}

/// The reference's `sections` as popup groups, for [`popup_lines`] to fold into columns: each
/// section's title, then each of its bindings that has a help line, as its effective keys and
/// that line. A section with nothing to list is left out. For an app whose whole reference fits
/// a popup, so it has no separate scrolling reference.
#[must_use]
pub fn help_groups<M: Command>(km: &Keymap<M>, sections: &[Section<M>]) -> PopupGroups {
    sections
        .iter()
        .filter_map(|sect| {
            let entries: Vec<_> = sect
                .bindings()
                .filter(|b| !b.help.is_empty() && km.keys(b).next().is_some())
                .map(|b| (km.labels(b, " "), b.help))
                .collect();
            (!entries.is_empty()).then_some((sect.title, entries))
        })
        .collect()
}

/// One line of the popup before it is laid out in columns.
enum PopupRow<'a> {
    Head(&'static str),
    Entry(&'a str, &'static str),
}

/// The `?` popup's lines: `groups` (a title, then its keys and names) laid out in columns of at
/// most `max_rows` rows, as many as fit in `width`, filling down each column in turn (a title
/// never ends a column, away from its entries). Entries that do not fit are counted on a last
/// line, `… N more: {more}`, where `more` says where to find them.
#[must_use]
pub fn popup_lines(
    groups: &PopupGroups,
    width: usize,
    max_rows: usize,
    more: &str,
) -> Vec<Line<'static>> {
    const GAP: usize = 2;
    let len = |s: &str| s.chars().count();
    let items: Vec<PopupRow> = groups
        .iter()
        .flat_map(|(title, entries)| {
            std::iter::once(PopupRow::Head(title))
                .chain(entries.iter().map(|(k, n)| PopupRow::Entry(k, n)))
        })
        .collect();
    let total_entries = items
        .iter()
        .filter(|r| matches!(r, PopupRow::Entry(..)))
        .count();
    let columns = |rows: usize| -> Vec<Vec<&PopupRow>> {
        let mut cols: Vec<Vec<&PopupRow>> = vec![Vec::new()];
        for it in &items {
            let at = cols.last().map_or(0, Vec::len);
            let orphan = matches!(it, PopupRow::Head(_)) && at + 1 >= rows;
            if (at >= rows || orphan) && at > 0 {
                cols.push(Vec::new());
            }
            if let Some(col) = cols.last_mut() {
                col.push(it);
            }
        }
        cols
    };
    // A column's key width and its whole width.
    let widths = |col: &[&PopupRow]| {
        let (mut kw, mut nw, mut hw) = (0, 0, 0);
        for r in col {
            match r {
                PopupRow::Head(t) => hw = hw.max(len(t)),
                PopupRow::Entry(k, n) => {
                    kw = kw.max(len(k));
                    nw = nw.max(len(n));
                }
            }
        }
        (kw, (kw + 1 + nw).max(hw))
    };
    let total = |rows: usize| {
        let cols: Vec<_> = columns(rows).iter().map(|c| widths(c).1).collect();
        cols.iter().sum::<usize>() + GAP * cols.len().saturating_sub(1)
    };
    if items.is_empty() || max_rows == 0 {
        return Vec::new();
    }
    let rows = (1..=max_rows)
        .find(|&r| total(r) <= width)
        .unwrap_or(max_rows);
    let mut cols: Vec<Vec<&PopupRow>> = Vec::new();
    let mut used = 0;
    for col in columns(rows) {
        let w = widths(&col).1 + if cols.is_empty() { 0 } else { GAP };
        if !cols.is_empty() && used + w > width {
            break;
        }
        used += w;
        cols.push(col);
    }
    let shown: usize = cols
        .iter()
        .flat_map(|c| c.iter())
        .filter(|r| matches!(r, PopupRow::Entry(..)))
        .count();
    let height = cols.iter().map(Vec::len).max().unwrap_or(0);
    let mut lines: Vec<Line<'static>> = (0..height)
        .map(|r| {
            let mut spans = Vec::new();
            for (i, col) in cols.iter().enumerate() {
                let (kw, w) = widths(col);
                if i > 0 {
                    spans.push(Span::raw(" ".repeat(GAP)));
                }
                match col.get(r) {
                    Some(PopupRow::Head(title)) => {
                        spans.push(Span::styled(format!("{title:<w$}"), title_style()));
                    }
                    Some(PopupRow::Entry(keys, name)) => {
                        spans.push(Span::styled(format!("{keys:<kw$}"), key_style()));
                        spans.push(Span::raw(format!(" {name:<0$}", w - kw - 1)));
                    }
                    None => spans.push(Span::raw(" ".repeat(w))),
                }
            }
            Line::from(spans)
        })
        .collect();
    if shown < total_entries {
        lines.push(Line::from(Span::styled(
            format!("… {} more: {more}", total_entries - shown),
            quiet_style(),
        )));
    }
    lines
}

#[cfg(test)]
pub(crate) mod fixture;

#[cfg(test)]
mod tests;
