//! Drawing (no state mutation here).

use lazytmux_core::{Location, TmuxContext};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus, Modal};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [main, keybar] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).areas(main);
    let [sessions_area, windows_area, panes_area] = Layout::vertical([
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ])
    .areas(left);
    let [info_area, preview_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(right);

    draw_sessions(frame, app, sessions_area);
    draw_windows(frame, app, windows_area);
    draw_panes(frame, app, panes_area);
    draw_info(frame, app, info_area);
    draw_preview(frame, app, preview_area);
    draw_keybar(frame, app, keybar);

    match app.modal.clone() {
        Some(Modal::Help) => draw_help(frame),
        Some(Modal::Input { title, buffer, .. }) => draw_input(frame, &title, &buffer),
        Some(Modal::Confirm { text, .. }) => draw_confirm(frame, &text),
        None => {}
    }
}

fn panel_block(title: &str, index: usize, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::new().fg(ACCENT)
    } else {
        Style::new().fg(DIM)
    };
    let title_style = if focused {
        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Gray)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(border_style)
        .title(Line::from(vec![
            Span::styled(format!("[{index}] "), Style::new().fg(DIM)),
            Span::styled(title.to_string(), title_style),
        ]))
}

fn highlight_style(focused: bool) -> Style {
    if focused {
        Style::new()
            .bg(ACCENT)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::REVERSED)
    }
}

fn draw_sessions(frame: &mut Frame, app: &mut App, area: Rect) {
    let current_id = app.ctx.current_session().map(|(id, _)| id.to_string());
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            let marker = if Some(&s.id) == current_id.as_ref() {
                Span::styled("● ", Style::new().fg(Color::Green))
            } else if s.is_attached() {
                Span::styled("○ ", Style::new().fg(Color::Yellow))
            } else {
                Span::raw("  ")
            };
            ListItem::new(Line::from(vec![
                marker,
                Span::raw(s.name.clone()),
                Span::styled(format!("  {}w", s.windows), Style::new().fg(DIM)),
            ]))
        })
        .collect();
    let focused = app.focus == Focus::Sessions;
    let list = List::new(items)
        .block(panel_block("Sessions", 1, focused))
        .highlight_style(highlight_style(focused));
    frame.render_stateful_widget(list, area, &mut app.sessions_state);
}

fn draw_windows(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .windows
        .iter()
        .map(|w| {
            let active = if w.active {
                Span::styled("* ", Style::new().fg(Color::Green))
            } else {
                Span::raw("  ")
            };
            ListItem::new(Line::from(vec![
                active,
                Span::styled(format!("{}: ", w.index), Style::new().fg(DIM)),
                Span::raw(w.name.clone()),
                Span::styled(format!("  {}p", w.panes), Style::new().fg(DIM)),
            ]))
        })
        .collect();
    let focused = app.focus == Focus::Windows;
    let list = List::new(items)
        .block(panel_block("Windows", 2, focused))
        .highlight_style(highlight_style(focused));
    frame.render_stateful_widget(list, area, &mut app.windows_state);
}

fn draw_panes(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .panes
        .iter()
        .map(|p| {
            let active = if p.active {
                Span::styled("* ", Style::new().fg(Color::Green))
            } else {
                Span::raw("  ")
            };
            ListItem::new(Line::from(vec![
                active,
                Span::styled(format!("{}: ", p.index), Style::new().fg(DIM)),
                Span::raw(p.command.clone()),
                Span::styled(
                    format!("  {}x{}", p.width, p.height),
                    Style::new().fg(DIM),
                ),
            ]))
        })
        .collect();
    let focused = app.focus == Focus::Panes;
    let list = List::new(items)
        .block(panel_block("Panes", 3, focused))
        .highlight_style(highlight_style(focused));
    frame.render_stateful_widget(list, area, &mut app.panes_state);
}

fn draw_info(frame: &mut Frame, app: &App, area: Rect) {
    let ctx_line = context_line(&app.ctx);
    let detail = match app.selected_session() {
        Some(s) => Line::from(vec![
            Span::styled("  path ", Style::new().fg(DIM)),
            Span::raw(s.path.clone()),
            Span::styled("  clients ", Style::new().fg(DIM)),
            Span::raw(s.attached.to_string()),
        ]),
        None => Line::from(Span::styled(
            "  no session selected — press 'n' to create one",
            Style::new().fg(DIM),
        )),
    };
    let para = Paragraph::new(vec![ctx_line, detail]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(DIM))
            .title(Span::styled(" lazy-tmux ", Style::new().fg(ACCENT).bold())),
    );
    frame.render_widget(para, area);
}

fn context_line(ctx: &TmuxContext) -> Line<'static> {
    match ctx {
        TmuxContext::Inside {
            session_name,
            location,
            ..
        } => {
            let (tag, style, extra) = match location {
                Location::DirectPane => (
                    "inside tmux",
                    Style::new().fg(Color::Green).bold(),
                    String::new(),
                ),
                Location::NestedDescendant { via } => (
                    "inside tmux (nested)",
                    Style::new().fg(Color::Yellow).bold(),
                    format!(" via {}", via.join(" > ")),
                ),
                Location::EnvInherited => (
                    "tmux env inherited",
                    Style::new().fg(Color::Yellow).bold(),
                    " — not displayed in tmux".to_string(),
                ),
            };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(tag, style),
                Span::styled(format!(" · session '{session_name}'{extra}"), Style::new()),
            ])
        }
        TmuxContext::StaleEnv { .. } => Line::from(vec![
            Span::raw("  "),
            Span::styled("stale $TMUX", Style::new().fg(Color::Red).bold()),
            Span::raw(" · pointing at a dead server"),
        ]),
        TmuxContext::Outside { server_running } => Line::from(vec![
            Span::raw("  "),
            Span::styled("outside tmux", Style::new().fg(Color::Blue).bold()),
            Span::raw(if *server_running {
                " · server running"
            } else {
                " · no server"
            }),
        ]),
    }
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.selected_pane() {
        Some(p) => format!(" Preview {} · {} ", p.id, p.command),
        None => " Preview ".to_string(),
    };
    let para = Paragraph::new(app.preview.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(DIM))
            .title(Span::styled(title, Style::new().fg(Color::Gray))),
    );
    frame.render_widget(para, area);
}

fn draw_keybar(frame: &mut Frame, app: &App, area: Rect) {
    let line = match &app.status {
        Some(msg) => Line::from(Span::styled(
            format!(" {msg}"),
            Style::new().fg(Color::Yellow),
        )),
        None => {
            let keys: &[(&str, &str)] = match app.focus {
                Focus::Sessions => &[
                    ("↵", "attach"),
                    ("n", "new"),
                    ("r", "rename"),
                    ("d", "kill"),
                    ("tab", "panel"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::Windows => &[
                    ("↵", "open"),
                    ("n", "new"),
                    ("r", "rename"),
                    ("d", "kill"),
                    ("tab", "panel"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::Panes => &[
                    ("↵", "open"),
                    ("s", "split below"),
                    ("v", "split right"),
                    ("d", "kill"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
            };
            let mut spans = vec![Span::raw(" ")];
            for (key, desc) in keys {
                spans.push(Span::styled(*key, Style::new().fg(ACCENT).bold()));
                spans.push(Span::styled(format!(" {desc}  "), Style::new().fg(DIM)));
            }
            Line::from(spans)
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

// ---- modals ---------------------------------------------------------------

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_input(frame: &mut Frame, title: &str, buffer: &str) {
    let area = centered(56, 3, frame.area());
    frame.render_widget(Clear, area);
    let para = Paragraph::new(Line::from(vec![
        Span::raw(buffer.to_string()),
        Span::styled("█", Style::new().fg(ACCENT)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(ACCENT))
            .title(format!(" {title} ")),
    );
    frame.render_widget(para, area);
}

fn draw_confirm(frame: &mut Frame, text: &str) {
    let area = centered(56, 5, frame.area());
    frame.render_widget(Clear, area);
    let para = Paragraph::new(vec![
        Line::from(text.to_string()),
        Line::default(),
        Line::from(vec![
            Span::styled("y", Style::new().fg(Color::Green).bold()),
            Span::raw(" confirm   "),
            Span::styled("n/esc", Style::new().fg(Color::Red).bold()),
            Span::raw(" cancel"),
        ]),
    ])
    .wrap(Wrap { trim: false })
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::Red))
            .title(" Confirm "),
    );
    frame.render_widget(para, area);
}

fn draw_help(frame: &mut Frame) {
    let area = centered(60, 18, frame.area());
    frame.render_widget(Clear, area);
    let rows: &[(&str, &str)] = &[
        ("j/k, ↓/↑", "move selection"),
        ("g / G", "first / last item"),
        ("tab, h/l", "cycle panel focus"),
        ("1 / 2 / 3", "jump to sessions / windows / panes"),
        ("enter", "attach or switch to selection"),
        ("n", "new session / window"),
        ("r", "rename session / window"),
        ("d, x", "kill selection (with confirm)"),
        ("s / v", "split pane below / right"),
        ("R", "refresh now"),
        ("?", "toggle this help"),
        ("q, esc", "quit"),
    ];
    let mut lines = vec![Line::default()];
    for (key, desc) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("  {key:<12}"), Style::new().fg(ACCENT).bold()),
            Span::raw(desc.to_string()),
        ]));
    }
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(ACCENT))
            .title(" Keys (attach = switch-client inside tmux, exec attach outside) "),
    );
    frame.render_widget(para, area);
}
