//! A small app's table, for the tests: two contexts, two kinds of row, enough commands to
//! exercise families, kinds, modifiers and overrides.

use ratatui::crossterm::event::KeyCode::{Down, Enter, Esc, F, Tab, Up};

use super::{Binding, Command, Context, Hint, bind, bind_kind, c, ctrl, k};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ctx {
    List,
    Pane,
}

impl Context for Ctx {
    const ALL: &'static [Self] = &[Ctx::List, Ctx::Pane];
    fn title(self) -> &'static str {
        match self {
            Ctx::List => "list",
            Ctx::Pane => "pane",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Track,
    Album,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cmd {
    Quit,
    Help,
    Down,
    Up,
    Open,
    Close,
    Next,
    Find,
    Enqueue,
    Save,
    Rename,
    Remove,
}

impl Command for Cmd {
    type Ctx = Ctx;
    type Kind = Kind;
    const ALL: &'static [Self] = &[
        Cmd::Quit,
        Cmd::Help,
        Cmd::Down,
        Cmd::Up,
        Cmd::Open,
        Cmd::Close,
        Cmd::Next,
        Cmd::Find,
        Cmd::Enqueue,
        Cmd::Save,
        Cmd::Rename,
        Cmd::Remove,
    ];
    const GROUPS: &'static [&'static str] = &["Move", "Change", "App"];

    fn name(self) -> &'static str {
        match self {
            Cmd::Quit => "app.quit",
            Cmd::Help => "app.help",
            Cmd::Down => "move.down",
            Cmd::Up => "move.up",
            Cmd::Open => "pane.open",
            Cmd::Close => "pane.close",
            Cmd::Next => "view.next",
            Cmd::Find => "find.open",
            Cmd::Enqueue => "queue.add",
            Cmd::Save => "item.save",
            Cmd::Rename => "album.rename",
            Cmd::Remove => "track.remove",
        }
    }
    fn group_title(self) -> &'static str {
        match self {
            Cmd::Down | Cmd::Up | Cmd::Open | Cmd::Close | Cmd::Next | Cmd::Find => "Move",
            Cmd::Enqueue | Cmd::Save | Cmd::Rename | Cmd::Remove => "Change",
            Cmd::Quit | Cmd::Help => "App",
        }
    }
    fn mutates(self) -> bool {
        matches!(self, Cmd::Save | Cmd::Rename | Cmd::Remove)
    }
    fn bindings() -> &'static [Binding<Self>] {
        TABLE
    }
}

#[rustfmt::skip]
static TABLE: &[Binding<Cmd>] = &[
    bind(Ctx::List, &[k(Tab)], Cmd::Next, "Next view", Hint::Text("Tab:view")),
    bind(Ctx::List, &[c('j'), k(Down)], Cmd::Down, "Down", Hint::Family("j/k:move", "↑/↓:move")),
    bind(Ctx::List, &[c('k'), k(Up)], Cmd::Up, "Up", Hint::None),
    bind(Ctx::List, &[k(Enter)], Cmd::Open, "Open", Hint::Text("Enter:open")),
    bind(Ctx::List, &[c('/'), ctrl('f')], Cmd::Find, "Find", Hint::Custom("find")),
    bind(Ctx::List, &[c('a')], Cmd::Enqueue, "Add to the queue", Hint::None),
    bind(Ctx::List, &[c('*')], Cmd::Save, "Save", Hint::None),
    bind_kind(Ctx::List, Kind::Album, &[c('R')], Cmd::Rename, "Rename the album", Hint::None),
    bind_kind(Ctx::List, Kind::Track, &[c('D')], Cmd::Remove, "Remove the track", Hint::None),
    bind_kind(Ctx::List, Kind::Track, &[c('q')], Cmd::Close, "", Hint::None),
    bind(Ctx::List, &[c('?'), k(F(1))], Cmd::Help, "Help", Hint::Text("?:help")),
    bind(Ctx::List, &[c('q'), ctrl('c')], Cmd::Quit, "Quit", Hint::None),
    bind(Ctx::Pane, &[k(Esc)], Cmd::Close, "Back to the list", Hint::Text("Esc:list")),
    bind(Ctx::Pane, &[c('j'), k(Down)], Cmd::Down, "Down", Hint::Family("j/k:move", "↑/↓:move")),
    bind(Ctx::Pane, &[c('k'), k(Up)], Cmd::Up, "Up", Hint::None),
    bind(Ctx::Pane, &[c('q'), ctrl('c')], Cmd::Quit, "Quit", Hint::Text("q:quit")),
];
