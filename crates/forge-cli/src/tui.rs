//! `forgehub ui` — a terminal browser for local and cloud
//! snippets (Step 18).
//!
//! The state machine is deliberately separated from the
//! terminal: [`TuiState::handle`] consumes abstract [`Key`]s
//! and produces [`Action`]s, so the whole interaction is
//! unit-tested without a TTY. The runner only translates
//! crossterm events, draws with ratatui, and — for actions —
//! restores the terminal and hands back to `main`, which runs
//! the action through the *same* command functions the CLI
//! uses (run/trust confirmations included). The TUI never
//! executes anything itself.

use std::io::IsTerminal;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::exit::CliError;
use crate::store::Store;
use crate::sync;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Local,
    Mine,
    Explore,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Local => "Local",
            Tab::Mine => "Mine",
            Tab::Explore => "Explore",
        }
    }
    pub fn next(self) -> Tab {
        match self {
            Tab::Local => Tab::Mine,
            Tab::Mine => Tab::Explore,
            Tab::Explore => Tab::Local,
        }
    }
    pub fn prev(self) -> Tab {
        match self {
            Tab::Local => Tab::Explore,
            Tab::Mine => Tab::Local,
            Tab::Explore => Tab::Mine,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TuiItem {
    /// Cloud owner; `None` for local items.
    pub owner: Option<String>,
    pub name: String,
    pub description: String,
    pub visibility: String,
    pub version: i64,
    /// The numbered command view (same text as the confirm card).
    pub commands: Vec<String>,
}

impl TuiItem {
    /// The reference string the normal commands understand.
    pub fn reference(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}/{}", self.name),
            None => self.name.clone(),
        }
    }
    fn haystack(&self) -> String {
        format!(
            "{} {} {}",
            self.reference(),
            self.description,
            self.visibility
        )
        .to_lowercase()
    }
}

/// What the TUI asks `main` to do after the terminal is
/// restored. Every variant maps onto an existing command
/// function — the safety path is shared, never duplicated.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Run(TuiItem),
    DryRun(TuiItem),
    Push(TuiItem),
    Pull(TuiItem),
    Fork(TuiItem),
    Star(TuiItem),
    Edit(TuiItem),
}

/// Abstract keys, so tests drive the state machine without
/// crossterm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
    Enter,
    Esc,
    Char(char),
    CtrlC,
    Unknown,
}

#[derive(Debug, Default)]
pub struct TabData {
    pub items: Vec<TuiItem>,
    /// `None` until the tab has been loaded (cloud tabs load
    /// lazily); an error message when loading failed.
    pub loaded: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct TuiState {
    pub tab: Tab,
    pub local: TabData,
    pub mine: TabData,
    pub explore: TabData,
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
    pub help: bool,
    /// Set when a cloud tab needs (re)loading; the runner
    /// performs the fetch and calls `fill_tab`.
    pub pending_load: Option<Tab>,
    /// Set by `q` / Ctrl-C; the runner exits the loop.
    pub quit_requested: bool,
}

impl Default for TuiState {
    fn default() -> Self {
        TuiState {
            tab: Tab::Local,
            local: TabData::default(),
            mine: TabData::default(),
            explore: TabData::default(),
            selected: 0,
            filter: String::new(),
            filtering: false,
            help: false,
            pending_load: None,
            quit_requested: false,
        }
    }
}

/// Subsequence fuzzy match with a small scoring function:
/// consecutive and word-start matches score higher. `None`
/// when the query is not a subsequence.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut score: i64 = 0;
    let mut ti = 0usize;
    let mut prev_matched = false;
    for qc in &q {
        let mut found = false;
        while ti < t.len() {
            let tc = t[ti];
            ti += 1;
            if tc == *qc {
                found = true;
                score += 1;
                if prev_matched {
                    score += 2; // consecutive run
                }
                if ti == 1 || !t[ti - 2].is_alphanumeric() {
                    score += 2; // word start
                }
                break;
            }
            prev_matched = false;
        }
        if !found {
            return None;
        }
        prev_matched = true;
    }
    Some(score)
}

impl TuiState {
    pub fn data(&self, tab: Tab) -> &TabData {
        match tab {
            Tab::Local => &self.local,
            Tab::Mine => &self.mine,
            Tab::Explore => &self.explore,
        }
    }
    fn data_mut(&mut self, tab: Tab) -> &mut TabData {
        match tab {
            Tab::Local => &mut self.local,
            Tab::Mine => &mut self.mine,
            Tab::Explore => &mut self.explore,
        }
    }

    pub fn fill_tab(&mut self, tab: Tab, result: Result<Vec<TuiItem>, String>) {
        let data = self.data_mut(tab);
        data.loaded = true;
        match result {
            Ok(items) => {
                data.items = items;
                data.error = None;
            }
            Err(message) => {
                data.items = Vec::new();
                data.error = Some(message);
            }
        }
        if self.tab == tab {
            self.selected = 0;
        }
        if self.pending_load == Some(tab) {
            self.pending_load = None;
        }
    }

    /// Items visible under the current filter, best fuzzy
    /// score first (stable for equal scores).
    pub fn visible(&self) -> Vec<&TuiItem> {
        let data = self.data(self.tab);
        if self.filter.is_empty() {
            return data.items.iter().collect();
        }
        let query = self.filter.to_lowercase();
        let mut scored: Vec<(i64, usize, &TuiItem)> = data
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| fuzzy_score(&query, &item.haystack()).map(|s| (s, i, item)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, item)| item).collect()
    }

    pub fn current(&self) -> Option<TuiItem> {
        self.visible().get(self.selected).map(|i| (*i).clone())
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.selected = 0;
        if !self.data(tab).loaded && tab != Tab::Local {
            self.pending_load = Some(tab);
        }
    }

    /// Handle one abstract key. Returns an action for `main`
    /// to execute (the runner then exits), or `None` to keep
    /// browsing. `quit` is signalled via the action-less
    /// `wants_quit` flag.
    pub fn handle(&mut self, key: Key) -> Option<Action> {
        if key == Key::CtrlC {
            // Ctrl-C quits from anywhere (the runner restores
            // the terminal before returning).
            self.quit_requested = true;
            return None;
        }
        if self.help {
            if key == Key::Esc || key == Key::Char('?') || key == Key::Char('q') {
                self.help = false;
            }
            return None;
        }
        if self.filtering {
            match key {
                Key::Esc | Key::Enter => {
                    self.filtering = false;
                }
                Key::Char(c) => {
                    self.filter.push(c);
                    self.selected = 0;
                }
                Key::Up => self.move_selection(-1),
                Key::Down => self.move_selection(1),
                _ => {}
            }
            return None;
        }
        match key {
            Key::Char('q') => {
                self.quit_requested = true;
            }
            Key::Tab | Key::Right => self.switch_tab(self.tab.next()),
            Key::BackTab | Key::Left => self.switch_tab(self.tab.prev()),
            Key::Up | Key::Char('k') => self.move_selection(-1),
            Key::Down | Key::Char('j') => self.move_selection(1),
            Key::Char('/') => {
                self.filtering = true;
            }
            Key::Esc => {
                if !self.filter.is_empty() {
                    self.filter.clear();
                    self.selected = 0;
                }
            }
            Key::Char('?') => {
                self.help = true;
            }
            Key::Enter | Key::Char('r') => {
                return self.current().map(Action::Run);
            }
            Key::Char('d') => {
                return self.current().map(Action::DryRun);
            }
            Key::Char('p') => {
                return self.current().map(Action::Push);
            }
            Key::Char('l') => {
                return self.current().map(Action::Pull);
            }
            Key::Char('f') => {
                return self.current().map(Action::Fork);
            }
            Key::Char('s') => {
                return self.current().map(Action::Star);
            }
            Key::Char('e') => {
                return self.current().map(Action::Edit);
            }
            _ => {}
        }
        None
    }

    fn move_selection(&mut self, delta: i64) {
        let len = self.visible().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let last = len - 1;
        self.selected = if delta >= 0 {
            let step = usize::try_from(delta).unwrap_or(usize::MAX);
            self.selected.saturating_add(step).min(last)
        } else {
            let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
            self.selected.saturating_sub(step)
        };
    }
}

/// Colour policy: one accent, dim secondary, and the semantic
/// trio only for messages. `NO_COLOR` / `TERM=dumb` → all plain.
#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color,
    pub dim: Color,
    pub error: Color,
    pub warn: Color,
    pub ascii: bool,
}

impl Palette {
    pub fn detect() -> Palette {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let dumb = std::env::var("TERM").is_ok_and(|t| t == "dumb");
        if no_color || dumb {
            Palette {
                accent: Color::Reset,
                dim: Color::Reset,
                error: Color::Reset,
                warn: Color::Reset,
                ascii: dumb,
            }
        } else {
            Palette {
                accent: Color::Blue,
                dim: Color::DarkGray,
                error: Color::Red,
                warn: Color::Yellow,
                ascii: false,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

fn items_from_snippets(snippets: &[forge_core::Snippet], owner: Option<&str>) -> Vec<TuiItem> {
    snippets
        .iter()
        .map(|s| TuiItem {
            owner: owner.map(str::to_string),
            name: s.name.clone(),
            description: s.description.clone(),
            visibility: "local".to_string(),
            version: 0,
            commands: crate::cmds::show_commands(s),
        })
        .collect()
}

fn items_from_json(values: &[serde_json::Value]) -> Vec<TuiItem> {
    values
        .iter()
        .map(|v| {
            let body = v["body"].as_str().unwrap_or_default();
            let commands = match forge_core::Snippet::from_yaml(body) {
                Ok(parsed) => crate::cmds::show_commands(&parsed),
                Err(_) => Vec::new(),
            };
            TuiItem {
                owner: v["owner"].as_str().map(str::to_string),
                name: v["name"].as_str().unwrap_or_default().to_string(),
                description: v["description"].as_str().unwrap_or_default().to_string(),
                visibility: v["visibility"].as_str().unwrap_or_default().to_string(),
                version: v["version"].as_i64().unwrap_or(0),
                commands,
            }
        })
        .collect()
}

/// Hard stop for [`fetch_all_pages`]: 50 pages × 100 rows is
/// 5 000 snippets — ten times the server's per-user cap — so
/// hitting it means the server is misbehaving, not that the
/// list is really that long.
const MAX_CLOUD_PAGES: usize = 50;

/// Page through a list endpoint until `next_cursor` runs out,
/// accumulating every page's `snippets`. (Before 2026-11-20
/// the cloud tabs silently showed only the first page.) A
/// cursor the server repeats ends the walk instead of
/// looping forever.
fn fetch_all_pages(
    mut get: impl FnMut(&str) -> Result<serde_json::Value, String>,
    base_path: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut all = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_CLOUD_PAGES {
        let path = match &cursor {
            None => base_path.to_string(),
            Some(c) => format!("{base_path}&cursor={}", sync::url_encode(c)),
        };
        let body = get(&path)?;
        all.extend(body["snippets"].as_array().cloned().unwrap_or_default());
        let next = body["next_cursor"].as_str().filter(|s| !s.is_empty());
        match next {
            None => return Ok(all),
            Some(n) if cursor.as_deref() == Some(n) => return Ok(all),
            Some(n) => cursor = Some(n.to_string()),
        }
    }
    Ok(all)
}

fn load_cloud_tab(store: &Store, tab: Tab) -> Result<Vec<TuiItem>, String> {
    let mut stdin = std::io::stdin().lock();
    let sess = sync::session(store, &mut stdin, None).map_err(|e| format!("{e}"))?;
    let base_path = match tab {
        Tab::Mine => "/me/snippets?limit=100&sort=updated",
        Tab::Explore => "/explore?feed=recent&limit=100",
        Tab::Local => return Ok(Vec::new()),
    };
    let values = fetch_all_pages(
        |path| {
            let (_, body, _) = sess
                .client
                .get(path, Some(&sess.entry.token))
                .map_err(|e| format!("could not reach {} ({e})", sess.registry))?;
            Ok(body)
        },
        base_path,
    )?;
    Ok(items_from_json(&values))
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

fn translate(event: &Event) -> Key {
    if let Event::Key(KeyEvent {
        code, modifiers, ..
    }) = *event
    {
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            return Key::CtrlC;
        }
        return match code {
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Char(c) => Key::Char(c),
            _ => Key::Unknown,
        };
    }
    Key::Unknown
}

fn draw(frame: &mut ratatui::Frame, state: &TuiState, palette: &Palette) {
    let area = frame.area();
    if area.width < 60 || area.height < 20 {
        frame.render_widget(
            Paragraph::new("Terminal too small — the UI needs at least 60×20.")
                .style(Style::default().fg(palette.warn)),
            area,
        );
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    draw_header(frame, state, palette, rows[0]);
    draw_main(frame, state, palette, rows[1]);
    let footer = if state.filtering {
        "type to filter · Enter/Esc done"
    } else {
        "j/k move · Tab tabs · / filter · Enter run · d dry · p push · l pull · f fork · s star · e edit · ? help · q quit"
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(palette.dim)),
        rows[2],
    );
}

fn draw_header(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    palette: &Palette,
    area: ratatui::layout::Rect,
) {
    // Header: tabs + filter.
    let mut header_spans: Vec<Span> = Vec::new();
    for tab in [Tab::Local, Tab::Mine, Tab::Explore] {
        let style = if state.tab == tab {
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.dim)
        };
        header_spans.push(Span::styled(format!(" {} ", tab.label()), style));
    }
    if state.filtering || !state.filter.is_empty() {
        header_spans.push(Span::styled(
            format!("  /{}", state.filter),
            Style::default().fg(palette.accent),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(header_spans)), area);
}

fn draw_main(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    palette: &Palette,
    area: ratatui::layout::Rect,
) {
    if state.help {
        let help = Paragraph::new(
            "Keys\n\
             j/k or ↑/↓   move\n\
             Tab/←/→      switch tab (Local · Mine · Explore)\n\
             /            fuzzy filter (Esc clears)\n\
             Enter or r   run (with the usual confirmation)\n\
             d            dry-run    p push    l pull\n\
             f            fork       s star    e edit\n\
             ?            this help  q quit",
        )
        .block(Block::default().title("Help").borders(if palette.ascii {
            Borders::NONE
        } else {
            Borders::ALL
        }));
        frame.render_widget(help, area);
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);
        let data = state.data(state.tab);
        if !data.loaded {
            frame.render_widget(Paragraph::new("Loading…"), cols[0]);
        } else if let Some(error) = &data.error {
            frame.render_widget(
                Paragraph::new(crate::cmds::escape_display(error))
                    .style(Style::default().fg(palette.error))
                    .wrap(Wrap { trim: false }),
                cols[0],
            );
        } else {
            let visible = state.visible();
            let items: Vec<ListItem> = visible
                .iter()
                .map(|item| {
                    let label = if item.owner.is_some() {
                        item.reference()
                    } else {
                        item.name.clone()
                    };
                    ListItem::new(Line::from(vec![
                        Span::raw(crate::cmds::escape_display(&label)),
                        Span::styled(
                            format!("  {}", crate::cmds::escape_display(&item.description)),
                            Style::default().fg(palette.dim),
                        ),
                    ]))
                })
                .collect();
            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!("{} ", state.tab.label()))
                        .borders(if palette.ascii {
                            Borders::NONE
                        } else {
                            Borders::ALL
                        }),
                )
                .highlight_style(
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            let mut list_state = ListState::default();
            if !visible.is_empty() {
                list_state.select(Some(state.selected.min(visible.len() - 1)));
            }
            frame.render_stateful_widget(list, cols[0], &mut list_state);

            draw_detail(frame, state, palette, cols[1]);
        }
    }
}

fn draw_detail(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    palette: &Palette,
    area: ratatui::layout::Rect,
) {
    // Detail pane: the same numbered command view as
    // the confirmation card.
    match state.current() {
        Some(item) => {
            let mut lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    crate::cmds::escape_display(&item.reference()),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    crate::cmds::escape_display(&if item.owner.is_some() {
                        format!(
                            "{} · v{} · {}",
                            item.description, item.version, item.visibility
                        )
                    } else {
                        format!("{} · local", item.description)
                    }),
                    Style::default().fg(palette.dim),
                )),
                Line::raw(""),
            ];
            for (i, cmd) in item.commands.iter().enumerate() {
                lines.push(Line::from(format!(
                    "{:>2}. {}",
                    i + 1,
                    crate::cmds::escape_display(cmd)
                )));
            }
            if item.commands.is_empty() {
                lines.push(Line::from(Span::styled(
                    "(no commands)",
                    Style::default().fg(palette.dim),
                )));
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().title("Detail").borders(if palette.ascii {
                        Borders::NONE
                    } else {
                        Borders::ALL
                    }))
                    .wrap(Wrap { trim: false }),
                area,
            );
        }
        None => {
            frame.render_widget(
                Paragraph::new("Nothing here.").style(Style::default().fg(palette.dim)),
                area,
            );
        }
    }
}

/// `forgehub ui`: browse, then hand the chosen action back to
/// `main` (terminal restored first).
pub fn run_ui(store: &Store) -> Result<Option<Action>> {
    if !std::io::stdout().is_terminal() {
        return Err(CliError::failure(
            "forgehub: `ui` needs a terminal\n  what: stdout is not a TTY\n  fix: run it in an interactive terminal, or use the plain commands (`fg list`, `fg run <name>`)",
        )
        .into());
    }
    let mut state = TuiState {
        local: TabData {
            items: items_from_snippets(&store.list(), None),
            loaded: true,
            error: None,
        },
        ..TuiState::default()
    };
    let palette = Palette::detect();

    // Restore the terminal on panic too.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let mut out = std::io::stdout();
        let _ = out.execute(LeaveAlternateScreen);
        original_hook(info);
    }));

    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut result: Option<Action> = None;
    let loop_result = (|| -> Result<()> {
        loop {
            if let Some(tab) = state.pending_load.take() {
                let loaded = load_cloud_tab(store, tab);
                state.fill_tab(tab, loaded);
            }
            term.draw(|frame| draw(frame, &state, &palette))?;
            if event::poll(Duration::from_millis(200))? {
                let key = translate(&event::read()?);
                if key == Key::CtrlC {
                    break;
                }
                let action = state.handle(key);
                if state.quit_requested {
                    break;
                }
                if let Some(action) = action {
                    result = Some(action);
                    break;
                }
            }
        }
        Ok(())
    })();

    terminal::disable_raw_mode()?;
    term.backend_mut().execute(LeaveAlternateScreen)?;
    term.show_cursor()?;
    loop_result?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, description: &str) -> TuiItem {
        TuiItem {
            owner: None,
            name: name.to_string(),
            description: description.to_string(),
            visibility: "local".to_string(),
            version: 0,
            commands: vec!["echo hi".to_string()],
        }
    }

    fn state_with(items: Vec<TuiItem>) -> TuiState {
        TuiState {
            local: TabData {
                items,
                loaded: true,
                error: None,
            },
            ..TuiState::default()
        }
    }

    fn page(names: &[&str], next: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "snippets": names.iter().map(|n| serde_json::json!({ "name": n })).collect::<Vec<_>>(),
            "next_cursor": next,
        })
    }

    #[test]
    fn pagination_walks_cursors_until_they_run_out() {
        let mut requested: Vec<String> = Vec::new();
        let values = fetch_all_pages(
            |path| {
                requested.push(path.to_string());
                Ok(match requested.len() {
                    1 => page(&["a", "b"], Some("cur 1&x")),
                    2 => page(&["c"], Some("cur2")),
                    _ => page(&["d"], None),
                })
            },
            "/me/snippets?limit=100&sort=updated",
        )
        .unwrap();
        let names: Vec<&str> = values.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
        // The cursor is appended URL-encoded.
        assert_eq!(
            requested,
            [
                "/me/snippets?limit=100&sort=updated",
                "/me/snippets?limit=100&sort=updated&cursor=cur%201%26x",
                "/me/snippets?limit=100&sort=updated&cursor=cur2",
            ]
        );
    }

    #[test]
    fn pagination_stops_on_a_repeated_cursor() {
        let mut calls = 0;
        let values = fetch_all_pages(
            |_| {
                calls += 1;
                Ok(page(&["a"], Some("same")))
            },
            "/explore?feed=recent&limit=100",
        )
        .unwrap();
        assert_eq!(values.len(), 2, "first page + one repeat, then stop");
        assert_eq!(calls, 2);
    }

    #[test]
    fn pagination_respects_the_page_cap() {
        let mut calls = 0;
        let values = fetch_all_pages(
            |_| {
                calls += 1;
                Ok(page(&["a"], Some(&format!("cursor-{calls}"))))
            },
            "/explore?feed=recent&limit=100",
        )
        .unwrap();
        assert_eq!(calls, MAX_CLOUD_PAGES);
        assert_eq!(values.len(), MAX_CLOUD_PAGES);
    }

    #[test]
    fn pagination_propagates_errors() {
        let err = fetch_all_pages(|_| Err("boom".to_string()), "/x").unwrap_err();
        assert_eq!(err, "boom");
    }

    #[test]
    fn fuzzy_matching_scores_and_filters() {
        assert!(fuzzy_score("ubs", "ubuntu-setup").is_some());
        assert!(fuzzy_score("xyz", "ubuntu-setup").is_none());
        assert!(fuzzy_score("", "anything").is_some());
        let word_start = fuzzy_score("us", "ubuntu-setup").unwrap();
        let mid_word = fuzzy_score("bu", "ubuntu-setup").unwrap();
        assert!(word_start > 0 && mid_word > 0);
    }

    #[test]
    fn navigation_clamps_and_tabs_cycle() {
        let mut state = state_with(vec![item("a", ""), item("b", ""), item("c", "")]);
        state.handle(Key::Down);
        state.handle(Key::Down);
        state.handle(Key::Down); // clamped at the last item
        assert_eq!(state.selected, 2);
        state.handle(Key::Up);
        assert_eq!(state.selected, 1);
        state.handle(Key::Char('k'));
        assert_eq!(state.selected, 0);
        state.handle(Key::Tab);
        assert_eq!(state.tab, Tab::Mine);
        assert_eq!(state.pending_load, Some(Tab::Mine));
        state.handle(Key::BackTab);
        assert_eq!(state.tab, Tab::Local);
    }

    #[test]
    fn filter_narrows_and_esc_clears() {
        let mut state = state_with(vec![
            item("ubuntu-setup", "packages"),
            item("docker-clean", "containers"),
            item("ubuntu-net", "network"),
        ]);
        state.handle(Key::Char('/'));
        assert!(state.filtering);
        for c in "ubnt".chars() {
            state.handle(Key::Char(c));
        }
        let names: Vec<&str> = state.visible().iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["ubuntu-setup", "ubuntu-net"]);
        state.handle(Key::Enter);
        assert!(!state.filtering);
        state.handle(Key::Esc);
        assert_eq!(state.filter, "");
        assert_eq!(state.visible().len(), 3);
    }

    #[test]
    fn action_keys_emit_the_selected_item() {
        let mut state = state_with(vec![item("a", ""), item("b", "")]);
        state.handle(Key::Down);
        let action = state.handle(Key::Enter).unwrap();
        assert_eq!(action, Action::Run(item("b", "")));
        let action = state.handle(Key::Char('d')).unwrap();
        assert!(matches!(action, Action::DryRun(_)));
        let action = state.handle(Key::Char('p')).unwrap();
        assert!(matches!(action, Action::Push(_)));
    }

    #[test]
    fn quit_and_help_states() {
        let mut state = state_with(vec![item("a", "")]);
        state.handle(Key::Char('?'));
        assert!(state.help);
        state.handle(Key::Char('q')); // closes help, does not quit
        assert!(!state.help);
        assert!(!state.quit_requested);
        state.handle(Key::Char('q'));
        assert!(state.quit_requested);
    }

    #[test]
    fn empty_list_actions_are_none() {
        let mut state = state_with(vec![]);
        assert!(state.handle(Key::Enter).is_none());
        assert!(state.current().is_none());
    }

    #[test]
    fn rendered_output_neutralises_hostile_text() {
        // Step 19 (T-EXEC-3): names/descriptions/commands are
        // untrusted text rendered into a terminal buffer — the
        // rendered frame must contain no raw ESC or bidi
        // controls anywhere.
        let mut hostile = item("evil", "desc \u{1b}[31m red \u{202e}flip");
        hostile.commands = vec!["echo \u{1b}[2K\rhidden".to_string()];
        let state = state_with(vec![hostile]);
        let palette = Palette::detect();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw(frame, &state, &palette))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut text = String::new();
        for cell in buffer.content() {
            text.push_str(cell.symbol());
        }
        assert!(!text.contains('\u{1b}'), "frame leaked raw ESC");
        assert!(!text.contains('\u{202e}'), "frame leaked bidi override");
        assert!(text.contains("hidden"), "command text should stay visible");
    }
}
