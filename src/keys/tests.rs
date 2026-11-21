use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use super::fixture::{Cmd, Ctx, Kind};
use super::*;

const KINDS: &[Option<Kind>] = &[None, Some(Kind::Track), Some(Kind::Album)];

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}
fn key(ch: char) -> KeyEvent {
    ev(KeyCode::Char(ch), KeyModifiers::NONE)
}
fn text(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn hint(km: &Keymap<Cmd>, ctx: Ctx, kind: Option<Kind>) -> String {
    km.hint_for(ctx, kind, |tag, km| {
        (tag == "find").then(|| format!("{}:find", km.first_label(ctx, Cmd::Find)))
    })
}

#[test]
fn the_fixture_keeps_every_rule() {
    assert_eq!(rules::no_shared_keys::<Cmd>(KINDS), Vec::<String>::new());
    assert_eq!(
        rules::every_command_bound_and_named::<Cmd>(),
        Vec::<String>::new()
    );
    assert_eq!(
        rules::every_key_parses_from_its_label::<Cmd>(),
        Vec::<String>::new()
    );
    assert_eq!(
        rules::lowercase_only_looks::<Cmd>(&[(Cmd::Save, "`*` marks")]),
        Vec::<String>::new()
    );
    assert_eq!(
        rules::every_command_bound_in_both_families::<Cmd>(KINDS),
        Vec::<String>::new()
    );
}

#[test]
fn the_case_rule_names_an_offender_and_a_stale_exception() {
    let problems = rules::lowercase_only_looks::<Cmd>(&[(Cmd::Rename, "no reason")]);
    assert_eq!(
        problems,
        [
            "item.save (List) is on the lowercase key *: Shift changes things, lowercase only looks",
            "album.rename is no longer on a lowercase key: drop its exception (no reason)",
        ]
    );
}

#[test]
fn lookup_tries_modifiers_first_ignores_shift_and_never_lets_ctrl_ride() {
    let km = Keymap::<Cmd>::default();
    let track = Some(Kind::Track);
    assert_eq!(
        km.lookup(
            Ctx::List,
            None,
            &ev(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ),
        Some(Cmd::Quit)
    );
    // An upper-case letter arrives with Shift held.
    assert_eq!(
        km.lookup(
            Ctx::List,
            track,
            &ev(KeyCode::Char('D'), KeyModifiers::SHIFT)
        ),
        Some(Cmd::Remove)
    );
    // Ctrl-J is not `j`.
    assert_eq!(
        km.lookup(
            Ctx::List,
            None,
            &ev(KeyCode::Char('j'), KeyModifiers::CONTROL)
        ),
        None
    );
}

#[test]
fn a_binding_for_the_rows_kind_wins_and_only_applies_to_that_kind() {
    let km = Keymap::<Cmd>::default();
    let (track, album) = (Some(Kind::Track), Some(Kind::Album));
    assert_eq!(km.lookup(Ctx::List, track, &key('q')), Some(Cmd::Close));
    assert_eq!(km.lookup(Ctx::List, album, &key('q')), Some(Cmd::Quit));
    assert_eq!(km.lookup(Ctx::List, album, &key('R')), Some(Cmd::Rename));
    assert_eq!(km.lookup(Ctx::List, track, &key('R')), None);
    // Any-kind bindings reach every kind.
    assert_eq!(km.lookup(Ctx::List, track, &key('j')), Some(Cmd::Down));
    // A track row's own `q` stands in for the list's: the popup lists one.
    let names: Vec<_> = km
        .popup_entries(Ctx::List, track)
        .into_iter()
        .flat_map(|(_, e)| e)
        .map(|(_, n)| n)
        .collect();
    assert!(
        names.contains(&"pane.close") && !names.contains(&"app.quit"),
        "{names:?}"
    );
}

#[test]
fn standard_turns_the_vim_keys_off_and_on_its_own() {
    let std = Keymap::<Cmd>::new(Family::Standard);
    let vim = Keymap::<Cmd>::new(Family::Vim);
    let ctrl_f = ev(KeyCode::Char('f'), KeyModifiers::CONTROL);
    assert_eq!(std.lookup(Ctx::List, None, &key('j')), None);
    assert_eq!(
        std.lookup(Ctx::List, None, &ev(KeyCode::Down, KeyModifiers::NONE)),
        Some(Cmd::Down)
    );
    assert_eq!(std.lookup(Ctx::List, None, &ctrl_f), Some(Cmd::Find));
    assert_eq!(vim.lookup(Ctx::List, None, &ctrl_f), None);
    assert_eq!(vim.lookup(Ctx::List, None, &key('j')), Some(Cmd::Down));
}

#[test]
fn the_hint_follows_the_family_the_overrides_and_the_apps_own_labels() {
    let vim = Keymap::<Cmd>::default();
    assert_eq!(
        hint(&vim, Ctx::List, None),
        "Tab:view  j/k:move  Enter:open  /:find  ?:help"
    );
    let std = Keymap::<Cmd>::new(Family::Standard);
    assert_eq!(
        hint(&std, Ctx::List, None),
        "Tab:view  ↑/↓:move  Enter:open  /:find  ?:help"
    );
    let (km, problems) = Keymap::<Cmd>::from_config("keys:\n  view.next: [Ctrl-L]\n", Family::Vim);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(hint(&km, Ctx::List, None).starts_with("Ctrl-L:view  j/k:move"));
}

#[test]
fn an_override_replaces_a_commands_keys_in_every_context() {
    let (km, problems) = Keymap::<Cmd>::from_config("keys:\n  move.down: [z]\n", Family::Vim);
    assert!(problems.is_empty(), "{problems:?}");
    for ctx in [Ctx::List, Ctx::Pane] {
        assert_eq!(km.lookup(ctx, None, &key('z')), Some(Cmd::Down), "{ctx:?}");
        assert_eq!(km.lookup(ctx, None, &key('j')), None, "{ctx:?}");
    }
    // The user's word stands in standard too, vim-only key or not.
    let (km, _) =
        Keymap::<Cmd>::from_config("keys:\n  family: standard\n  move.down: [j]\n", Family::Vim);
    assert_eq!(km.family, Family::Standard);
    assert_eq!(km.lookup(Ctx::List, None, &key('j')), Some(Cmd::Down));
}

#[test]
fn what_cannot_be_used_is_reported_and_left_out() {
    let text = "keys:\n  view.nxt: [x]\n  view.next: [\"Ctrl-\", \"Ctrl-L\"]\n  move.up: [Hyper]\n  app.help: []\n  move.down: [\"a\"]\n";
    let (km, problems) = Keymap::<Cmd>::from_config(text, Family::Vim);
    assert_eq!(
        problems,
        [
            "view.nxt: no such command",
            "view.next: no such key \"Ctrl-\"",
            "move.up: no such key \"Hyper\"",
            "app.help: no keys",
            "move.down: a is already queue.add's in the list",
        ]
    );
    let ctrl_l = ev(KeyCode::Char('l'), KeyModifiers::CONTROL);
    assert_eq!(km.lookup(Ctx::List, None, &ctrl_l), Some(Cmd::Next));
    assert_eq!(km.lookup(Ctx::List, None, &key('j')), Some(Cmd::Down));
}

#[test]
fn the_family_is_the_keys_scalar_else_vim_mode_else_the_default() {
    let family = |text: &str, default| Keymap::<Cmd>::from_config(text, default).0.family;
    use Family::{Standard, Vim};
    assert_eq!(family("", Standard), Standard);
    assert_eq!(family("keys: standard\n", Vim), Standard);
    assert_eq!(family("keys: vim\nvim_mode: false\n", Standard), Vim);
    assert_eq!(family("vim_mode: false\n", Vim), Standard);
    assert_eq!(
        family("keys:\n  family: standard\nvim_mode: true\n", Vim),
        Standard
    );
    let (_, problems) = Keymap::<Cmd>::from_config("keys: emacs\n", Vim);
    assert_eq!(problems, ["keys: no such family \"emacs\" (vim, standard)"]);
}

#[test]
fn keys_read_back_from_their_labels() {
    for (s, want) in [
        ("Left", k(KeyCode::Left)),
        ("Ctrl--", ctrl('-')),
        ("F3", k(KeyCode::F(3))),
        ("F(3)", k(KeyCode::F(3))),
        ("Shift-F3", shift(KeyCode::F(3))),
        ("F", c('F')),
        ("S-Tab", k(KeyCode::BackTab)),
    ] {
        assert_eq!(Key::parse(s), Some(want), "{s}");
    }
    for bad in ["Ctrl-", "Hyper", ""] {
        assert_eq!(Key::parse(bad), None, "{bad}");
    }
    assert_eq!(
        Key::parse("Alt-F").map(|k| k.label()).as_deref(),
        Some("Alt-F")
    );
}

#[test]
fn the_reference_lists_each_section_then_the_extras() {
    let km = Keymap::<Cmd>::default();
    let sections = [
        Section::<Cmd> {
            ctx: Ctx::List,
            kinds: &[None],
            title: "List",
        },
        Section::<Cmd> {
            ctx: Ctx::List,
            kinds: &[Some(Kind::Album)],
            title: "Album rows",
        },
    ];
    let extras: &[Extra] = &[("Mouse", &[("Click", "Select")])];
    let all = text(&reference(&km, &sections, extras));
    assert!(
        all.starts_with("List\n  Tab             Next view\n"),
        "{all}"
    );
    assert!(
        all.contains(
            "\n\nAlbum rows\n  R               Rename the album\n\nMouse\n  Click           Select"
        ),
        "{all}"
    );
    assert!(!all.ends_with('\n'));
}

#[test]
fn the_popup_groups_by_purpose_and_folds_into_columns() {
    let km = Keymap::<Cmd>::default();
    let groups = km.popup_entries(Ctx::List, Some(Kind::Album));
    let titles: Vec<_> = groups.iter().map(|(t, _)| *t).collect();
    assert_eq!(titles, ["Move", "Change", "App"]);
    // As few rows as fit the width: narrow, one tall column (a line per title and entry);
    // wide, side-by-side columns; narrow and short, it counts what it left out.
    let entries: usize = groups.iter().map(|(_, e)| e.len()).sum();
    assert_eq!(popup_lines(&groups, 24, 40).len(), entries + groups.len());
    assert!(popup_lines(&groups, 200, 40).len() < entries);
    let short = text(&popup_lines(&groups, 30, 3));
    assert!(
        short.contains("more: the full reference has them all"),
        "{short}"
    );
}
