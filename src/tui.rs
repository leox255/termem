//! Interactive ratatui picker: fuzzy-filter sessions, preview, and pick one to
//! resume. Returns the chosen session (the caller execs the resume command).

use crate::model::{rel_time, Session, Source};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::time::Duration;

/// Run the picker. `Ok(Some(s))` if the user chose to resume `s`, `Ok(None)` if
/// they quit.
pub fn run(sessions: Vec<Session>, cwd: String) -> Result<Option<Session>> {
    if sessions.is_empty() {
        return Ok(None);
    }
    let mut terminal = ratatui::init();
    let result = App::new(sessions, cwd).run(&mut terminal);
    ratatui::restore();
    result
}

struct App {
    all: Vec<Session>,
    cwd: String,
    filter: String,
    filtered: Vec<usize>,
    state: ListState,
}

impl App {
    fn new(all: Vec<Session>, cwd: String) -> Self {
        let mut app = App {
            all,
            cwd,
            filter: String::new(),
            filtered: Vec::new(),
            state: ListState::default(),
        };
        app.recompute();
        app
    }

    fn recompute(&mut self) {
        let needle = self.filter.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                if needle.is_empty() {
                    return true;
                }
                let hay =
                    format!("{} {} {}", s.title, s.cwd, s.first_prompt).to_lowercase();
                fuzzy(&hay, &needle)
            })
            .map(|(i, _)| i)
            .collect();
        if self.filtered.is_empty() {
            self.state.select(None);
        } else {
            let sel = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.filtered.len() - 1);
            self.state.select(Some(sel));
        }
    }

    fn selected_session(&self) -> Option<&Session> {
        self.state
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|&idx| &self.all[idx])
    }

    fn move_sel(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as isize;
        let cur = self.state.selected().unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(len);
        self.state.select(Some(next as usize));
    }

    fn run(mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<Option<Session>> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
                match k.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if ctrl => return Ok(None),
                    KeyCode::Enter => return Ok(self.selected_session().cloned()),
                    KeyCode::Up => self.move_sel(-1),
                    KeyCode::Down => self.move_sel(1),
                    KeyCode::Char('p') if ctrl => self.move_sel(-1),
                    KeyCode::Char('n') if ctrl => self.move_sel(1),
                    KeyCode::Backspace => {
                        self.filter.pop();
                        self.recompute();
                    }
                    KeyCode::Char(c) => {
                        self.filter.push(c);
                        self.recompute();
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        let header = Paragraph::new(Line::from(vec![
            Span::raw(format!(
                "{} session(s) · {}",
                self.filtered.len(),
                self.cwd
            )),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" filter: {}_ ", self.filter)),
        );
        f.render_widget(header, chunks[0]);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[1]);

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .map(|&idx| {
                let s = &self.all[idx];
                let icon = match s.source {
                    Source::Claude => "◆",
                    Source::Codex => "◇",
                    Source::Shell => "❯",
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{icon} "), source_style(s.source)),
                    Span::styled(
                        format!("{:>4} ", rel_time(s.updated_at)),
                        Style::new().fg(Color::DarkGray),
                    ),
                    Span::raw(clip(&s.title, 58)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" sessions "))
            .highlight_style(
                Style::new()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, body[0], &mut self.state);

        let preview = match self.selected_session() {
            Some(s) => {
                let dim = Style::new().fg(Color::DarkGray);
                let mut lines = vec![
                    Line::from(Span::styled(
                        s.title.clone(),
                        Style::new().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("source  ", dim),
                        Span::styled(s.source.as_str(), source_style(s.source)),
                    ]),
                    Line::from(vec![
                        Span::styled("updated ", dim),
                        Span::raw(format!(
                            "{} ago · {} msgs",
                            rel_time(s.updated_at),
                            s.msg_count
                        )),
                    ]),
                    Line::from(vec![
                        Span::styled("model   ", dim),
                        Span::raw(s.model.clone().unwrap_or_else(|| "—".into())),
                    ]),
                    Line::from(vec![
                        Span::styled("cwd     ", dim),
                        Span::raw(s.cwd.clone()),
                    ]),
                ];
                if let Some(b) = &s.git_branch {
                    if !b.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("branch  ", dim),
                            Span::raw(b.clone()),
                        ]));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("first prompt", dim)));
                lines.push(Line::from(clip(&s.first_prompt, 600)));
                Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .block(Block::default().borders(Borders::ALL).title(" preview "))
            }
            None => Paragraph::new("no match")
                .block(Block::default().borders(Borders::ALL).title(" preview ")),
        };
        f.render_widget(preview, body[1]);

        let help = Paragraph::new(Line::from(vec![
            Span::styled(" ↑↓", Style::new().fg(Color::Cyan)),
            Span::raw(" move  "),
            Span::styled("⏎", Style::new().fg(Color::Cyan)),
            Span::raw(" resume  "),
            Span::styled("type", Style::new().fg(Color::Cyan)),
            Span::raw(" filter  "),
            Span::styled("esc", Style::new().fg(Color::Cyan)),
            Span::raw(" quit"),
        ]));
        f.render_widget(help, chunks[2]);
    }
}

fn source_style(src: Source) -> Style {
    match src {
        Source::Claude => Style::new().fg(Color::Magenta),
        Source::Codex => Style::new().fg(Color::Green),
        Source::Shell => Style::new().fg(Color::Yellow),
    }
}

fn clip(s: &str, max: usize) -> String {
    let first = s.lines().next().unwrap_or("").trim();
    if first.chars().count() <= max {
        first.to_string()
    } else {
        let t: String = first.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// Case-insensitive subsequence match (inputs already lowercased). Spaces in the
/// needle are ignored so multi-word filters match loosely.
fn fuzzy(hay: &str, needle: &str) -> bool {
    let mut chars = hay.chars();
    'outer: for nc in needle.chars() {
        if nc == ' ' {
            continue;
        }
        for hc in chars.by_ref() {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Session;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn fuzzy_subsequence() {
        assert!(fuzzy("build terminal memory", "btm"));
        assert!(fuzzy("build terminal memory", "term mem"));
        assert!(!fuzzy("build terminal memory", "xyz"));
    }

    fn sess(title: &str, cwd: &str, src: Source) -> Session {
        Session {
            id: "id".into(),
            source: src,
            file_path: "/f".into(),
            cwd: cwd.into(),
            title: title.into(),
            first_prompt: "the first prompt".into(),
            last_prompt: "last".into(),
            model: Some("claude-opus-4-8".into()),
            git_branch: None,
            started_at: 0,
            updated_at: 0,
            msg_count: 3,
        }
    }

    /// Renders the picker headlessly (criterion: the TUI launches & lays out).
    #[test]
    fn renders_and_filters_without_panicking() {
        let mut app = App::new(
            vec![
                sess("Alpha session", "/p", Source::Claude),
                sess("Beta session", "/p", Source::Codex),
            ],
            "/p".into(),
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();

        // Type a filter; only the matching row survives and stays selected.
        app.filter.push('b');
        app.recompute();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.selected_session().unwrap().title, "Beta session");
        terminal.draw(|f| app.draw(f)).unwrap();
    }
}
