//! Application state and update logic (no drawing here).

use std::time::{Duration, Instant};

use lazytmux_core::{Location, Pane, ResizeDir, Session, TmuxContext, Tmux, Window};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

const REFRESH_EVERY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sessions,
    Windows,
    Panes,
}

#[derive(Debug, Clone)]
pub enum InputAction {
    NewSession,
    RenameSession { id: String },
    NewWindow { session_id: String },
    RenameWindow { id: String },
    RenamePane { id: String },
}

#[derive(Debug, Clone)]
pub enum PickAction {
    MoveWindow { window_id: String },
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)] // the shared Kill prefix is the point
pub enum ConfirmAction {
    KillSession { id: String },
    KillWindow { id: String },
    KillPane { id: String },
}

#[derive(Debug, Clone)]
pub enum Modal {
    Input {
        title: String,
        buffer: String,
        action: InputAction,
    },
    Confirm {
        text: String,
        action: ConfirmAction,
    },
    /// Pick one of `items` (id, label); j/k + enter.
    Pick {
        title: String,
        items: Vec<(String, String)>,
        selected: usize,
        action: PickAction,
    },
    Help,
}

pub struct App {
    pub tmux: Tmux,
    pub ctx: TmuxContext,
    pub sessions: Vec<Session>,
    pub windows: Vec<Window>,
    pub panes: Vec<Pane>,
    pub preview: String,
    pub focus: Focus,
    pub sessions_state: ListState,
    pub windows_state: ListState,
    pub panes_state: ListState,
    pub modal: Option<Modal>,
    pub status: Option<String>,
    pub should_quit: bool,
    /// Session (or session:window) target to attach to after the TUI exits.
    pub attach_target: Option<String>,
    last_refresh: Instant,
}

impl App {
    pub fn new(tmux: Tmux, ctx: TmuxContext) -> Self {
        Self {
            tmux,
            ctx,
            sessions: Vec::new(),
            windows: Vec::new(),
            panes: Vec::new(),
            preview: String::new(),
            focus: Focus::Sessions,
            sessions_state: ListState::default(),
            windows_state: ListState::default(),
            panes_state: ListState::default(),
            modal: None,
            status: None,
            should_quit: false,
            attach_target: None,
            last_refresh: Instant::now(),
        }
    }

    // ---- selection helpers ------------------------------------------------

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.sessions_state.selected()?)
    }

    pub fn selected_window(&self) -> Option<&Window> {
        self.windows.get(self.windows_state.selected()?)
    }

    pub fn selected_pane(&self) -> Option<&Pane> {
        self.panes.get(self.panes_state.selected()?)
    }

    /// tmux target for the selected window: `$id:index`.
    fn window_target(&self) -> Option<String> {
        let session = self.selected_session()?;
        let window = self.selected_window()?;
        Some(format!("{}:{}", session.id, window.index))
    }

    // ---- data loading -----------------------------------------------------

    pub fn refresh_all(&mut self) {
        self.last_refresh = Instant::now();
        match self.tmux.list_sessions() {
            Ok(sessions) => self.sessions = sessions,
            Err(e) => {
                self.sessions.clear();
                self.report(format!("error: {e}"));
            }
        }
        clamp(&mut self.sessions_state, self.sessions.len());
        // Keep the current session selected on first load.
        if self.sessions_state.selected().is_none() && !self.sessions.is_empty() {
            let current = self
                .ctx
                .current_session()
                .and_then(|(id, _)| self.sessions.iter().position(|s| s.id == id));
            self.sessions_state.select(Some(current.unwrap_or(0)));
        }
        self.refresh_windows();
    }

    pub fn refresh_windows(&mut self) {
        self.windows = match self.selected_session() {
            Some(session) => self.tmux.list_windows(&session.id).unwrap_or_default(),
            None => Vec::new(),
        };
        clamp(&mut self.windows_state, self.windows.len());
        if self.windows_state.selected().is_none() && !self.windows.is_empty() {
            let active = self.windows.iter().position(|w| w.active);
            self.windows_state.select(Some(active.unwrap_or(0)));
        }
        self.refresh_panes();
    }

    pub fn refresh_panes(&mut self) {
        self.panes = match self.window_target() {
            Some(target) => self.tmux.list_panes(&target).unwrap_or_default(),
            None => Vec::new(),
        };
        clamp(&mut self.panes_state, self.panes.len());
        if self.panes_state.selected().is_none() && !self.panes.is_empty() {
            let active = self.panes.iter().position(|p| p.active);
            self.panes_state.select(Some(active.unwrap_or(0)));
        }
        self.refresh_preview();
    }

    pub fn refresh_preview(&mut self) {
        self.preview = match self.selected_pane() {
            // Capturing the pane we are displayed in would show this very
            // TUI (preview inside preview inside...) — hall-of-mirrors.
            Some(pane) if self.is_own_pane(&pane.id) => {
                "\n   you are here — this pane is running lazy-tmux".into()
            }
            Some(pane) => self.tmux.capture_pane(&pane.id).unwrap_or_default(),
            None => String::new(),
        };
    }

    /// Is this the pane our own output is displayed in?
    fn is_own_pane(&self, pane_id: &str) -> bool {
        match &self.ctx {
            TmuxContext::Inside {
                pane_id: own,
                location,
                ..
            } => own == pane_id && !matches!(location, Location::EnvInherited),
            _ => false,
        }
    }

    pub fn on_tick(&mut self) {
        if self.last_refresh.elapsed() >= REFRESH_EVERY {
            self.refresh_all();
        }
    }

    fn report(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
    }

    // ---- input ------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.modal.is_some() {
            self.on_modal_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('R') => self.refresh_all(),
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.jump_selection(0),
            KeyCode::Char('G') => self.jump_selection(usize::MAX),
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => self.cycle_focus(1),
            KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => self.cycle_focus(-1),
            KeyCode::Char('1') => self.focus = Focus::Sessions,
            KeyCode::Char('2') => self.focus = Focus::Windows,
            KeyCode::Char('3') => self.focus = Focus::Panes,
            KeyCode::Enter => self.attach_selected(),
            KeyCode::Char('n') => self.start_new(),
            KeyCode::Char('r') => self.start_rename(),
            KeyCode::Char('d') | KeyCode::Char('x') => self.start_kill(),
            KeyCode::Char('s') => self.split(false),
            KeyCode::Char('v') => self.split(true),
            KeyCode::Char('z') => self.zoom(),
            KeyCode::Char('[') => self.swap_step(true),
            KeyCode::Char(']') => self.swap_step(false),
            KeyCode::Char('o') => self.cycle_layout(),
            KeyCode::Char('b') => self.break_pane_out(),
            KeyCode::Char('m') => self.start_move_window(),
            KeyCode::Char('D') => self.detach_session(),
            KeyCode::Char('M') => self.toggle_mouse(),
            KeyCode::Char('H') => self.resize(ResizeDir::Left),
            KeyCode::Char('J') => self.resize(ResizeDir::Down),
            KeyCode::Char('K') => self.resize(ResizeDir::Up),
            KeyCode::Char('L') => self.resize(ResizeDir::Right),
            _ => {}
        }
    }

    fn cycle_focus(&mut self, dir: isize) {
        let order = [Focus::Sessions, Focus::Windows, Focus::Panes];
        let i = order.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        let next = (i + dir).rem_euclid(order.len() as isize) as usize;
        self.focus = order[next];
    }

    fn move_selection(&mut self, dir: isize) {
        let (state, len) = match self.focus {
            Focus::Sessions => (&mut self.sessions_state, self.sessions.len()),
            Focus::Windows => (&mut self.windows_state, self.windows.len()),
            Focus::Panes => (&mut self.panes_state, self.panes.len()),
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as isize;
        let next = (cur + dir).rem_euclid(len as isize) as usize;
        state.select(Some(next));
        self.on_selection_change();
    }

    fn jump_selection(&mut self, index: usize) {
        let (state, len) = match self.focus {
            Focus::Sessions => (&mut self.sessions_state, self.sessions.len()),
            Focus::Windows => (&mut self.windows_state, self.windows.len()),
            Focus::Panes => (&mut self.panes_state, self.panes.len()),
        };
        if len == 0 {
            return;
        }
        state.select(Some(index.min(len - 1)));
        self.on_selection_change();
    }

    fn on_selection_change(&mut self) {
        match self.focus {
            Focus::Sessions => {
                self.windows_state.select(None);
                self.panes_state.select(None);
                self.refresh_windows();
            }
            Focus::Windows => {
                self.panes_state.select(None);
                self.refresh_panes();
            }
            Focus::Panes => self.refresh_preview(),
        }
    }

    // ---- actions ----------------------------------------------------------

    fn attach_selected(&mut self) {
        let Some(session) = self.selected_session() else {
            return;
        };
        let target = session.id.clone();
        if matches!(self.focus, Focus::Windows | Focus::Panes) {
            if let Some(window_target) = self.window_target() {
                let _ = self.tmux.select_window(&window_target);
            }
            if self.focus == Focus::Panes {
                if let Some(pane) = self.selected_pane() {
                    let id = pane.id.clone();
                    let _ = self.tmux.select_pane(&id);
                }
            }
        }
        self.attach_target = Some(target);
        self.should_quit = true;
    }

    fn start_new(&mut self) {
        match self.focus {
            Focus::Sessions => {
                self.modal = Some(Modal::Input {
                    title: "New session name (empty = default)".into(),
                    buffer: String::new(),
                    action: InputAction::NewSession,
                });
            }
            Focus::Windows | Focus::Panes => {
                let Some(session) = self.selected_session() else {
                    return;
                };
                self.modal = Some(Modal::Input {
                    title: format!("New window in '{}' (empty = default)", session.name),
                    buffer: String::new(),
                    action: InputAction::NewWindow {
                        session_id: session.id.clone(),
                    },
                });
            }
        }
    }

    fn start_rename(&mut self) {
        match self.focus {
            Focus::Sessions => {
                let Some(session) = self.selected_session() else {
                    return;
                };
                self.modal = Some(Modal::Input {
                    title: format!("Rename session '{}'", session.name),
                    buffer: session.name.clone(),
                    action: InputAction::RenameSession {
                        id: session.id.clone(),
                    },
                });
            }
            Focus::Windows => {
                let Some(window) = self.selected_window() else {
                    return;
                };
                self.modal = Some(Modal::Input {
                    title: format!("Rename window '{}'", window.name),
                    buffer: window.name.clone(),
                    action: InputAction::RenameWindow {
                        id: window.id.clone(),
                    },
                });
            }
            Focus::Panes => {
                let Some(pane) = self.selected_pane() else {
                    return;
                };
                self.modal = Some(Modal::Input {
                    title: format!("Title for pane {}", pane.id),
                    buffer: pane.title.clone(),
                    action: InputAction::RenamePane {
                        id: pane.id.clone(),
                    },
                });
            }
        }
    }

    fn start_kill(&mut self) {
        match self.focus {
            Focus::Sessions => {
                let Some(session) = self.selected_session() else {
                    return;
                };
                let current = self
                    .ctx
                    .current_session()
                    .is_some_and(|(id, _)| id == session.id);
                let warn = if current { " — this is YOUR session!" } else { "" };
                self.modal = Some(Modal::Confirm {
                    text: format!("Kill session '{}'{warn}", session.name),
                    action: ConfirmAction::KillSession {
                        id: session.id.clone(),
                    },
                });
            }
            Focus::Windows => {
                let Some(window) = self.selected_window() else {
                    return;
                };
                self.modal = Some(Modal::Confirm {
                    text: format!("Kill window '{}: {}'", window.index, window.name),
                    action: ConfirmAction::KillWindow {
                        id: window.id.clone(),
                    },
                });
            }
            Focus::Panes => {
                let Some(pane) = self.selected_pane() else {
                    return;
                };
                self.modal = Some(Modal::Confirm {
                    text: format!("Kill pane {} ({})", pane.id, pane.command),
                    action: ConfirmAction::KillPane {
                        id: pane.id.clone(),
                    },
                });
            }
        }
    }

    fn zoom(&mut self) {
        if self.focus != Focus::Panes {
            return;
        }
        let Some(pane) = self.selected_pane() else {
            return;
        };
        let id = pane.id.clone();
        match self.tmux.zoom_pane(&id) {
            Ok(()) => {
                self.report(format!("toggled zoom on {id}"));
                self.refresh_panes();
            }
            Err(e) => self.report(format!("zoom failed: {e}")),
        }
    }

    fn resize(&mut self, dir: ResizeDir) {
        if self.focus != Focus::Panes {
            return;
        }
        let Some(pane) = self.selected_pane() else {
            return;
        };
        let id = pane.id.clone();
        if let Err(e) = self.tmux.resize_pane(&id, dir, 3) {
            self.report(format!("resize failed: {e}"));
        } else {
            self.refresh_panes();
        }
    }

    fn swap_step(&mut self, up: bool) {
        match self.focus {
            Focus::Sessions => {
                self.report("sessions are sorted by tmux; nothing to reorder");
            }
            Focus::Windows => {
                let Some(i) = self.windows_state.selected() else {
                    return;
                };
                let Some(j) = (if up { i.checked_sub(1) } else { Some(i + 1) })
                    .filter(|j| *j < self.windows.len())
                else {
                    self.report(if up { "already first" } else { "already last" });
                    return;
                };
                let (a, b) = (self.windows[i].id.clone(), self.windows[j].id.clone());
                match self.tmux.swap_windows(&a, &b) {
                    Ok(()) => {
                        self.windows_state.select(Some(j));
                        self.refresh_windows();
                        self.report("swapped windows");
                    }
                    Err(e) => self.report(format!("swap failed: {e}")),
                }
            }
            Focus::Panes => {
                let Some(pane) = self.selected_pane() else {
                    return;
                };
                let id = pane.id.clone();
                match self.tmux.swap_pane_step(&id, up) {
                    Ok(()) => {
                        let i = self.panes_state.selected().unwrap_or(0);
                        let j = if up {
                            i.saturating_sub(1)
                        } else {
                            (i + 1).min(self.panes.len().saturating_sub(1))
                        };
                        self.panes_state.select(Some(j));
                        self.refresh_panes();
                        self.report("swapped panes");
                    }
                    Err(e) => self.report(format!("swap failed: {e}")),
                }
            }
        }
    }

    fn cycle_layout(&mut self) {
        let Some(target) = self.window_target() else {
            return;
        };
        match self.tmux.next_layout(&target) {
            Ok(()) => {
                self.refresh_panes();
                self.report("cycled window layout");
            }
            Err(e) => self.report(format!("layout failed: {e}")),
        }
    }

    fn break_pane_out(&mut self) {
        if self.focus != Focus::Panes {
            return;
        }
        let Some(pane) = self.selected_pane() else {
            return;
        };
        let id = pane.id.clone();
        match self.tmux.break_pane(&id) {
            Ok(()) => {
                self.refresh_all();
                self.report(format!("broke {id} out into its own window"));
            }
            Err(e) => self.report(format!("break failed: {e}")),
        }
    }

    fn start_move_window(&mut self) {
        if self.focus != Focus::Windows {
            return;
        }
        let (Some(session), Some(window)) = (self.selected_session(), self.selected_window())
        else {
            return;
        };
        let items: Vec<(String, String)> = self
            .sessions
            .iter()
            .filter(|s| s.id != session.id)
            .map(|s| (s.id.clone(), s.name.clone()))
            .collect();
        if items.is_empty() {
            self.report("no other session to move to");
            return;
        }
        self.modal = Some(Modal::Pick {
            title: format!("Move window '{}' to session", window.name),
            items,
            selected: 0,
            action: PickAction::MoveWindow {
                window_id: window.id.clone(),
            },
        });
    }

    fn detach_session(&mut self) {
        if self.focus != Focus::Sessions {
            return;
        }
        let Some(session) = self.selected_session() else {
            return;
        };
        if !session.is_attached() {
            self.report(format!("'{}' has no attached clients", session.name));
            return;
        }
        let (id, name) = (session.id.clone(), session.name.clone());
        match self.tmux.detach_clients(&id) {
            Ok(()) => {
                self.refresh_all();
                self.report(format!("detached clients from '{name}'"));
            }
            Err(e) => self.report(format!("detach failed: {e}")),
        }
    }

    fn toggle_mouse(&mut self) {
        match self.tmux.toggle_global_flag("mouse") {
            Ok(on) => self.report(format!("mouse: {}", if on { "on" } else { "off" })),
            Err(e) => self.report(format!("mouse toggle failed: {e}")),
        }
    }

    fn split(&mut self, right: bool) {
        if self.focus != Focus::Panes {
            return;
        }
        let Some(pane) = self.selected_pane() else {
            return;
        };
        let id = pane.id.clone();
        match self.tmux.split_pane(&id, right) {
            Ok(()) => self.refresh_panes(),
            Err(e) => self.report(format!("split failed: {e}")),
        }
    }

    // ---- modal handling ---------------------------------------------------

    fn on_modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        match modal {
            Modal::Help => {
                // Any key closes help.
            }
            Modal::Input {
                title,
                mut buffer,
                action,
            } => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => self.commit_input(&action, buffer.trim()),
                KeyCode::Backspace => {
                    buffer.pop();
                    self.modal = Some(Modal::Input {
                        title,
                        buffer,
                        action,
                    });
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                    self.modal = Some(Modal::Input {
                        title,
                        buffer,
                        action,
                    });
                }
                _ => {
                    self.modal = Some(Modal::Input {
                        title,
                        buffer,
                        action,
                    });
                }
            },
            Modal::Confirm { text, action } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.commit_confirm(&action)
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {}
                _ => self.modal = Some(Modal::Confirm { text, action }),
            },
            Modal::Pick {
                title,
                items,
                mut selected,
                action,
            } => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let target_id = items[selected].0.clone();
                    self.commit_pick(&action, &target_id);
                }
                code => {
                    match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            selected = (selected + 1) % items.len();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            selected = selected.checked_sub(1).unwrap_or(items.len() - 1);
                        }
                        _ => {}
                    }
                    self.modal = Some(Modal::Pick {
                        title,
                        items,
                        selected,
                        action,
                    });
                }
            },
        }
    }

    fn commit_pick(&mut self, action: &PickAction, target_id: &str) {
        let result = match action {
            PickAction::MoveWindow { window_id } => {
                self.tmux.move_window_to_session(window_id, target_id)
            }
        };
        match result {
            Ok(()) => self.report("moved window"),
            Err(e) => self.report(format!("error: {e}")),
        }
        self.refresh_all();
    }

    fn commit_input(&mut self, action: &InputAction, value: &str) {
        let name = if value.is_empty() { None } else { Some(value) };
        let result = match action {
            InputAction::NewSession => self
                .tmux
                .new_session(name, None)
                .map(|s| self.report(format!("created session '{}'", s.name))),
            InputAction::NewWindow { session_id } => self
                .tmux
                .new_window(session_id, name, None)
                .map(|w| self.report(format!("created window '{}'", w.name))),
            InputAction::RenameSession { id } => match name {
                Some(name) => self.tmux.rename_session(id, name),
                None => Ok(()),
            },
            InputAction::RenameWindow { id } => match name {
                Some(name) => self.tmux.rename_window(id, name),
                None => Ok(()),
            },
            InputAction::RenamePane { id } => match name {
                Some(title) => self.tmux.set_pane_title(id, title),
                None => Ok(()),
            },
        };
        if let Err(e) = result {
            self.report(format!("error: {e}"));
        }
        self.refresh_all();
    }

    fn commit_confirm(&mut self, action: &ConfirmAction) {
        let result = match action {
            ConfirmAction::KillSession { id } => self.tmux.kill_session(id),
            ConfirmAction::KillWindow { id } => self.tmux.kill_window(id),
            ConfirmAction::KillPane { id } => self.tmux.kill_pane(id),
        };
        if let Err(e) = result {
            self.report(format!("error: {e}"));
        }
        self.refresh_all();
    }
}

fn clamp(state: &mut ListState, len: usize) {
    match state.selected() {
        Some(_) if len == 0 => state.select(None),
        Some(i) if i >= len => state.select(Some(len - 1)),
        _ => {}
    }
}
