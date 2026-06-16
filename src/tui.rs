//! Interactive ratatui picker: fuzzy-filter sessions, preview, and pick one to
//! resume. Returns the chosen session (the caller execs the resume command).

use crate::logo;
use crate::model::{rel_time, Session, Source};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, Paragraph, Wrap};
use std::time::Duration;

// Palette (catppuccin-ish), kept cohesive across the whole picker.
const BORDER: (u8, u8, u8) = (69, 71, 96);
const DIMTXT: (u8, u8, u8) = (127, 132, 156);
const SELBG: (u8, u8, u8) = (35, 40, 60);

fn rgb(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

fn source_color(src: Source) -> Color {
    rgb(match src {
        Source::Claude => (203, 166, 247),
        Source::Codex => (166, 227, 161),
        Source::Gemini => (137, 180, 250),
        Source::Opencode => (148, 226, 213),
        Source::Shell => (249, 226, 175),
    })
}

fn source_icon(src: Source) -> &'static str {
    match src {
        Source::Claude => "◆",
        Source::Codex => "◇",
        Source::Gemini => "✦",
        Source::Opencode => "◈",
        Source::Shell => "❯",
    }
}

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
                let hay = format!("{} {} {}", s.title, s.cwd, s.first_prompt).to_lowercase();
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
        let ter_c = rgb(logo::TER_RGB);
        let mem_c = rgb(logo::MEM_RGB);
        let dim = Style::new().fg(rgb(DIMTXT));
        let border = Style::new().fg(rgb(BORDER));

        let root = Layout::vertical([
            Constraint::Length(6),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

        // ---- header: wordmark + context, then a filter line ----
        let header =
            Layout::vertical([Constraint::Length(5), Constraint::Length(1)]).split(root[0]);
        let top = Layout::horizontal([Constraint::Length(38), Constraint::Min(0)]).split(header[0]);

        let ter = logo::ter_rows();
        let mem = logo::mem_rows();
        let wordmark: Vec<Line> = (0..5)
            .map(|i| {
                Line::from(vec![
                    Span::styled(ter[i].clone(), Style::new().fg(ter_c).bold()),
                    Span::raw("  "),
                    Span::styled(mem[i].clone(), Style::new().fg(mem_c).bold()),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(wordmark), top[0]);

        let (shown, total) = (self.filtered.len(), self.all.len());
        let count_tail = if shown == total {
            " sessions".to_string()
        } else {
            format!(" / {total} sessions")
        };
        let info = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("cross-agent memory & sessions", dim)),
            Line::from(vec![
                Span::styled(format!("{shown}"), Style::new().fg(ter_c).bold()),
                Span::styled(count_tail, dim),
            ]),
            Line::from(Span::styled(clip(&self.cwd, 44), dim)),
        ]);
        f.render_widget(info, top[1]);

        let mut filter_spans = vec![
            Span::raw(" "),
            Span::styled("› ", Style::new().fg(mem_c).bold()),
        ];
        if self.filter.is_empty() {
            filter_spans.push(Span::styled("type to filter", dim));
        } else {
            filter_spans.push(Span::raw(self.filter.clone()));
            filter_spans.push(Span::styled("▏", Style::new().fg(mem_c)));
        }
        f.render_widget(Paragraph::new(Line::from(filter_spans)), header[1]);

        // ---- body: list + preview ----
        let body = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(root[1]);

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .map(|&idx| {
                let s = &self.all[idx];
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{} ", source_icon(s.source)),
                        Style::new().fg(source_color(s.source)),
                    ),
                    Span::styled(format!("{:>4}  ", rel_time(s.updated_at)), dim),
                    Span::raw(clip(&s.title, 48)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(panel(border, " sessions ", ter_c))
            .highlight_style(Style::new().bg(rgb(SELBG)).fg(ter_c).bold())
            .highlight_symbol("▌ ");
        f.render_stateful_widget(list, body[0], &mut self.state);

        let preview = match self.selected_session() {
            Some(s) => {
                let label = |t: &str| Span::styled(format!("{t:<8}"), dim);
                let mut lines = vec![
                    Line::from(Span::styled(
                        clip(&s.title, 60),
                        Style::new().fg(mem_c).bold(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        label("source"),
                        Span::styled(s.source.as_str(), Style::new().fg(source_color(s.source))),
                    ]),
                    Line::from(vec![
                        label("updated"),
                        Span::raw(format!(
                            "{} ago · {} msgs",
                            rel_time(s.updated_at),
                            s.msg_count
                        )),
                    ]),
                    Line::from(vec![
                        label("model"),
                        Span::raw(s.model.clone().unwrap_or_else(|| "—".into())),
                    ]),
                    Line::from(vec![label("cwd"), Span::raw(s.cwd.clone())]),
                ];
                if let Some(b) = &s.git_branch {
                    if !b.is_empty() {
                        lines.push(Line::from(vec![label("branch"), Span::raw(b.clone())]));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("first prompt", dim)));
                lines.push(Line::from(clip(&s.first_prompt, 600)));
                Paragraph::new(lines).wrap(Wrap { trim: true }).block(panel(
                    border,
                    " preview ",
                    mem_c,
                ))
            }
            None => Paragraph::new(Span::styled("no match", dim)).block(panel(
                border,
                " preview ",
                mem_c,
            )),
        };
        f.render_widget(preview, body[1]);

        // ---- footer ----
        let key = |k: &str| {
            Span::styled(
                format!(" {k} "),
                Style::new().fg(Color::Black).bg(rgb(DIMTXT)),
            )
        };
        let footer = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            key("↑↓"),
            Span::styled(" move   ", dim),
            key("⏎"),
            Span::styled(" open   ", dim),
            key("type"),
            Span::styled(" filter   ", dim),
            key("esc"),
            Span::styled(" quit", dim),
        ]));
        f.render_widget(footer, root[2]);
    }
}

/// A rounded panel with a colored title.
fn panel<'a>(border: Style, title: &'a str, title_color: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Line::from(Span::styled(
            title,
            Style::new().fg(title_color).bold(),
        )))
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
            bypass: false,
        }
    }

    /// Renders the picker headlessly (criterion: the TUI launches & lays out).
    #[test]
    fn renders_and_filters_without_panicking() {
        let mut app = App::new(
            vec![
                sess("Alpha session", "/p", Source::Claude),
                sess("Beta session", "/p", Source::Gemini),
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
