//! The application: state, the top-level layout (menu, sidebar, header),
//! actions, undo/redo, saving and the screenshot mode.

use crate::model::{self, rgb, Label, Loaded, Priority, Project, Task, Template, Workspace, PALETTE};
use crate::theme::{self, Pal};
use crate::widgets;
use chrono::{Datelike, Duration, NaiveDate};
use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Id, Key, KeyboardShortcut, Layout,
    Margin, Modifiers, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Vec2,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// Exit code the program ends with (used by the screenshot mode).
pub static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

const SCREENSHOT_FRAME: u32 = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Board,
    List,
    Overview,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DueFilter {
    #[default]
    Any,
    Overdue,
    ThisWeek,
    NoDate,
}

impl DueFilter {
    pub const ALL: [DueFilter; 4] = [DueFilter::Any, DueFilter::Overdue, DueFilter::ThisWeek, DueFilter::NoDate];
    pub fn name(self) -> &'static str {
        match self {
            DueFilter::Any => "Any due date",
            DueFilter::Overdue => "Overdue",
            DueFilter::ThisWeek => "Due in 7 days",
            DueFilter::NoDate => "No due date",
        }
    }
}

#[derive(Default, Clone)]
pub struct Filters {
    pub text: String,
    pub priority: Option<Priority>,
    pub label: Option<u64>,
    /// Some("") means "unassigned".
    pub assignee: Option<String>,
    pub due: DueFilter,
}

impl Filters {
    pub fn active(&self) -> bool {
        !self.text.trim().is_empty()
            || self.priority.is_some()
            || self.label.is_some()
            || self.assignee.is_some()
            || self.due != DueFilter::Any
    }

    pub fn matches(&self, t: &Task, p: &Project, in_done: bool, today: NaiveDate) -> bool {
        let q = self.text.trim().to_lowercase();
        if !q.is_empty() {
            let in_labels = t
                .labels
                .iter()
                .filter_map(|l| p.label(*l))
                .any(|l| l.name.to_lowercase().contains(&q));
            let hit = t.title.to_lowercase().contains(&q)
                || t.description.to_lowercase().contains(&q)
                || t.assignee.to_lowercase().contains(&q)
                || t.checklist.iter().any(|c| c.text.to_lowercase().contains(&q))
                || in_labels;
            if !hit {
                return false;
            }
        }
        if let Some(pr) = self.priority {
            if t.priority != pr {
                return false;
            }
        }
        if let Some(l) = self.label {
            if !t.labels.contains(&l) {
                return false;
            }
        }
        if let Some(a) = &self.assignee {
            if &t.assignee != a {
                return false;
            }
        }
        match self.due {
            DueFilter::Any => true,
            DueFilter::Overdue => t.is_overdue(today, in_done),
            DueFilter::ThisWeek => {
                !in_done && t.due.is_some_and(|d| d >= today && d <= today + Duration::days(7))
            }
            DueFilter::NoDate => t.due.is_none(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Board,
    Title,
    Status,
    Priority,
    Due,
    Assignee,
    Points,
    Updated,
}

#[derive(Clone)]
pub struct DragState {
    pub task: u64,
    pub offset: Vec2,
    pub size: Vec2,
}

pub enum Action {
    SelectProject(u64),
    SelectTask(Option<u64>),
    NewTask { col: u64, title: String, open: bool },
    MoveTask { task: u64, col: u64, before: Option<u64> },
    DeleteTask(u64),
    DuplicateTask(u64),
    AddColumn(String),
    RenameColumn(u64, String),
    DeleteColumn(u64),
    MoveColumn(u64, i32),
    SetWip(u64, u32),
    SetColumnColor(u64, [u8; 3]),
    ToggleDoneColumn(u64),
    ClearColumn(u64),
    OpenProjectDialog(Option<u64>),
    DeleteProject(u64),
    Ask(Confirm),
}

pub enum Confirm {
    DeleteTask(u64, String),
    DeleteColumn(u64, String, usize),
    ClearColumn(u64, String, usize),
    DeleteProject(u64, String),
}

pub struct ProjectDialog {
    pub editing: Option<u64>,
    pub name: String,
    pub description: String,
    pub color: [u8; 3],
    pub template: Template,
    pub labels: Vec<Label>,
    pub members: Vec<String>,
    pub new_label: String,
    pub new_label_color: [u8; 3],
    pub new_member: String,
    pub error: Option<String>,
    pub focus: bool,
}

struct Shot {
    path: PathBuf,
    frame: u32,
    requested: bool,
    done: Arc<Mutex<Option<Result<(), String>>>>,
}

pub struct App {
    pub(crate) ws: Workspace,
    pub(crate) view: View,
    pub(crate) filters: Filters,
    pub(crate) selected_task: Option<u64>,
    pub(crate) drag: Option<DragState>,
    pub(crate) quick_add: Option<(u64, String)>,
    pub(crate) quick_add_focus: bool,
    pub(crate) renaming_col: Option<(u64, String)>,
    pub(crate) rename_focus: bool,
    pub(crate) adding_column: Option<String>,
    pub(crate) project_dialog: Option<ProjectDialog>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) sort: (SortKey, bool),
    pub(crate) cal_month: (i32, u32),
    pub(crate) new_check: String,
    pub(crate) new_comment: String,
    pub(crate) focus_title: bool,
    pub(crate) focus_search: bool,
    pub(crate) actions: Vec<Action>,
    pub(crate) col_rects: Vec<(u64, Rect)>,
    pub(crate) card_rects: Vec<(u64, u64, Rect)>,
    undo: Vec<Vec<Project>>,
    redo: Vec<Vec<Project>>,
    dirty: bool,
    last_change: f64,
    save_error: Option<String>,
    can_save: bool,
    toast: Option<(String, f64)>,
    shot: Option<Shot>,
    time: f64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        let mut toast = None;
        let ws = match model::load() {
            Loaded::Existing(ws) => ws,
            Loaded::Fresh => model::sample_workspace(),
            Loaded::Broken(aside) => {
                toast = Some((format!("Your saved data couldn't be read. A copy was kept at {aside}"), 0.0));
                model::sample_workspace()
            }
        };
        theme::install(&cc.egui_ctx, ws.dark_mode);
        let today = model::today();
        let shot = screenshot.map(|path| Shot { path, frame: 0, requested: false, done: Arc::new(Mutex::new(None)) });
        let mut app = App {
            can_save: shot.is_none(),
            ws,
            view: View::Board,
            filters: Filters::default(),
            selected_task: None,
            drag: None,
            quick_add: None,
            quick_add_focus: false,
            renaming_col: None,
            rename_focus: false,
            adding_column: None,
            project_dialog: None,
            confirm: None,
            sort: (SortKey::Board, true),
            cal_month: (today.year(), today.month()),
            new_check: String::new(),
            new_comment: String::new(),
            focus_title: false,
            focus_search: false,
            actions: Vec::new(),
            col_rects: Vec::new(),
            card_rects: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            last_change: 0.0,
            save_error: None,
            toast,
            shot,
            time: 0.0,
        };
        if app.ws.selected.is_none() || app.ws.project(app.ws.selected.unwrap_or(0)).is_none() {
            app.ws.selected = app.ws.projects.first().map(|p| p.id);
        }
        // For checking pictures of other screens: PLANBOARD_SHOT_STATE=list,dark,task
        if app.shot.is_some() {
            if let Ok(state) = std::env::var("PLANBOARD_SHOT_STATE") {
                for part in state.split(',').map(str::trim) {
                    match part {
                        "list" => app.view = View::List,
                        "overview" => app.view = View::Overview,
                        "dark" => {
                            app.ws.dark_mode = true;
                            theme::set_dark(&cc.egui_ctx, true);
                        }
                        "task" => {
                            app.selected_task = app.current().and_then(|p| p.columns.get(2).or(p.columns.first())).and_then(|c| c.tasks.first()).map(|t| t.id);
                        }
                        _ => {}
                    }
                }
            }
        }
        app
    }

    pub fn pal(&self) -> Pal {
        theme::pal(self.ws.dark_mode)
    }

    pub fn current(&self) -> Option<&Project> {
        self.ws.selected.and_then(|id| self.ws.project(id))
    }

    fn current_mut(&mut self) -> Option<&mut Project> {
        let id = self.ws.selected?;
        self.ws.project_mut(id)
    }

    /// Remember the current state so it can be undone.
    pub fn snapshot(&mut self) {
        self.undo.push(self.ws.projects.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn push_undo(&mut self, before: Vec<Project>) {
        self.undo.push(before);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.last_change = self.time;
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            let cur = std::mem::replace(&mut self.ws.projects, prev);
            self.redo.push(cur);
            self.after_history();
            self.show_toast("Undone");
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            let cur = std::mem::replace(&mut self.ws.projects, next);
            self.undo.push(cur);
            self.after_history();
            self.show_toast("Redone");
        }
    }

    fn after_history(&mut self) {
        if self.ws.selected.and_then(|id| self.ws.project(id)).is_none() {
            self.ws.selected = self.ws.projects.first().map(|p| p.id);
        }
        if let Some(t) = self.selected_task {
            if self.current().and_then(|p| p.task(t)).is_none() {
                self.selected_task = None;
            }
        }
        self.drag = None;
        self.touch();
    }

    pub fn show_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.time));
    }

    fn save_now(&mut self) {
        if !self.can_save {
            return;
        }
        match model::save(&self.ws) {
            Ok(()) => {
                self.dirty = false;
                self.save_error = None;
            }
            Err(e) => {
                self.save_error = Some(e);
                // try again a bit later
                self.last_change = self.time + 4.0;
            }
        }
    }

    fn select_project(&mut self, id: u64) {
        if self.ws.selected != Some(id) {
            self.ws.selected = Some(id);
            self.selected_task = None;
            self.filters = Filters::default();
            self.quick_add = None;
            self.renaming_col = None;
            self.drag = None;
            self.touch();
        }
    }

    pub fn open_project_dialog(&mut self, editing: Option<u64>) {
        let d = match editing.and_then(|id| self.ws.project(id)) {
            Some(p) => ProjectDialog {
                editing: Some(p.id),
                name: p.name.clone(),
                description: p.description.clone(),
                color: p.color,
                template: Template::Simple,
                labels: p.labels.clone(),
                members: p.members.clone(),
                new_label: String::new(),
                new_label_color: PALETTE[1],
                new_member: String::new(),
                error: None,
                focus: true,
            },
            None => {
                let color = PALETTE[self.ws.projects.len() % PALETTE.len()];
                ProjectDialog {
                    editing: None,
                    name: String::new(),
                    description: String::new(),
                    color,
                    template: Template::Software,
                    labels: Vec::new(),
                    members: Vec::new(),
                    new_label: String::new(),
                    new_label_color: PALETTE[1],
                    new_member: String::new(),
                    error: None,
                    focus: true,
                }
            }
        };
        self.project_dialog = Some(d);
    }

    /// The column a brand-new task goes to: the first one that isn't a "done" column.
    fn default_column(&self) -> Option<u64> {
        let p = self.current()?;
        p.columns.iter().find(|c| !c.is_done).or(p.columns.first()).map(|c| c.id)
    }

    pub fn new_task_here(&mut self) {
        match self.default_column() {
            Some(col) => self.actions.push(Action::NewTask { col, title: String::new(), open: true }),
            None => {
                if self.current().is_some() {
                    self.show_toast("Add a column to the board first");
                } else {
                    self.show_toast("Create a project first");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Actions

    fn apply_actions(&mut self) {
        let actions = std::mem::take(&mut self.actions);
        for a in actions {
            self.apply(a);
        }
    }

    fn apply(&mut self, a: Action) {
        match a {
            Action::SelectProject(id) => self.select_project(id),
            Action::SelectTask(t) => {
                if self.selected_task != t {
                    self.selected_task = t;
                    self.new_check.clear();
                    self.new_comment.clear();
                    if let Some(d) = t.and_then(|t| self.current().and_then(|p| p.task(t))).and_then(|t| t.due) {
                        self.cal_month = (d.year(), d.month());
                    }
                }
            }
            Action::NewTask { col, title, open } => {
                let id = self.ws.new_id();
                self.snapshot();
                let Some(p) = self.current_mut() else { return };
                let Some(ci) = p.column_index(col) else { return };
                let mut t = Task::new(id, title.trim());
                if p.columns[ci].is_done {
                    t.completed = Some(model::now());
                }
                p.columns[ci].tasks.push(t);
                if open {
                    self.selected_task = Some(id);
                    self.focus_title = true;
                    self.new_check.clear();
                    self.new_comment.clear();
                }
                self.touch();
            }
            Action::MoveTask { task, col, before } => {
                let Some(p) = self.current() else { return };
                // Skip moves that change nothing.
                if let (Some((ci, ti)), Some(target)) = (p.locate(task), p.column_index(col)) {
                    if ci == target {
                        let next = p.columns[ci].tasks.get(ti + 1).map(|t| t.id);
                        if next == before {
                            return;
                        }
                    }
                }
                self.snapshot();
                let moved = self.current_mut().map(|p| p.move_task(task, col, before)).unwrap_or(false);
                if moved {
                    self.touch();
                }
            }
            Action::DeleteTask(id) => {
                self.snapshot();
                if let Some(p) = self.current_mut() {
                    if let Some((c, t)) = p.locate(id) {
                        p.columns[c].tasks.remove(t);
                    }
                }
                if self.selected_task == Some(id) {
                    self.selected_task = None;
                }
                self.touch();
                self.show_toast("Task deleted. Press Ctrl+Z to undo.");
            }
            Action::DuplicateTask(id) => {
                let new_id = self.ws.new_id();
                self.snapshot();
                if let Some(p) = self.current_mut() {
                    if let Some((c, t)) = p.locate(id) {
                        let mut copy = p.columns[c].tasks[t].clone();
                        copy.id = new_id;
                        copy.title = format!("{} (copy)", copy.title);
                        copy.created = model::now();
                        copy.updated = copy.created;
                        copy.comments.clear();
                        p.columns[c].tasks.insert(t + 1, copy);
                    }
                }
                self.selected_task = Some(new_id);
                self.touch();
            }
            Action::AddColumn(name) => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                let id = self.ws.new_id();
                self.snapshot();
                if let Some(p) = self.current_mut() {
                    let color = PALETTE[(p.columns.len() + 1) % PALETTE.len()];
                    // Keep "done" columns at the end.
                    let at = p.columns.iter().rposition(|c| !c.is_done).map(|i| i + 1).unwrap_or(0);
                    let at = if p.columns.iter().all(|c| !c.is_done) { p.columns.len() } else { at };
                    p.columns.insert(at, model::Column { id, name, color, wip_limit: 0, is_done: false, tasks: vec![] });
                }
                self.touch();
            }
            Action::RenameColumn(id, name) => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                if self.current().and_then(|p| p.columns.iter().find(|c| c.id == id)).is_some_and(|c| c.name == name) {
                    return;
                }
                self.snapshot();
                if let Some(c) = self.current_mut().and_then(|p| p.columns.iter_mut().find(|c| c.id == id)) {
                    c.name = name;
                }
                self.touch();
            }
            Action::DeleteColumn(id) => {
                self.snapshot();
                let sel = self.selected_task;
                let mut lost_selection = false;
                if let Some(p) = self.current_mut() {
                    if let Some(i) = p.column_index(id) {
                        let c = p.columns.remove(i);
                        lost_selection = sel.is_some_and(|s| c.tasks.iter().any(|t| t.id == s));
                    }
                }
                if lost_selection {
                    self.selected_task = None;
                }
                self.touch();
                self.show_toast("Column deleted. Press Ctrl+Z to undo.");
            }
            Action::MoveColumn(id, dir) => {
                self.snapshot();
                if let Some(p) = self.current_mut() {
                    if let Some(i) = p.column_index(id) {
                        let j = i as i32 + dir;
                        if j >= 0 && (j as usize) < p.columns.len() {
                            p.columns.swap(i, j as usize);
                        }
                    }
                }
                self.touch();
            }
            Action::SetWip(id, limit) => {
                self.snapshot();
                if let Some(c) = self.current_mut().and_then(|p| p.columns.iter_mut().find(|c| c.id == id)) {
                    c.wip_limit = limit;
                }
                self.touch();
            }
            Action::SetColumnColor(id, color) => {
                self.snapshot();
                if let Some(c) = self.current_mut().and_then(|p| p.columns.iter_mut().find(|c| c.id == id)) {
                    c.color = color;
                }
                self.touch();
            }
            Action::ToggleDoneColumn(id) => {
                self.snapshot();
                if let Some(c) = self.current_mut().and_then(|p| p.columns.iter_mut().find(|c| c.id == id)) {
                    c.is_done = !c.is_done;
                    let n = model::now();
                    for t in &mut c.tasks {
                        t.completed = if c.is_done { Some(t.completed.unwrap_or(n)) } else { None };
                    }
                }
                self.touch();
            }
            Action::ClearColumn(id) => {
                self.snapshot();
                let sel = self.selected_task;
                let mut lost = false;
                if let Some(c) = self.current_mut().and_then(|p| p.columns.iter_mut().find(|c| c.id == id)) {
                    lost = sel.is_some_and(|s| c.tasks.iter().any(|t| t.id == s));
                    c.tasks.clear();
                }
                if lost {
                    self.selected_task = None;
                }
                self.touch();
                self.show_toast("Column cleared. Press Ctrl+Z to undo.");
            }
            Action::OpenProjectDialog(id) => self.open_project_dialog(id),
            Action::DeleteProject(id) => {
                self.snapshot();
                self.ws.projects.retain(|p| p.id != id);
                if self.ws.selected == Some(id) {
                    self.ws.selected = self.ws.projects.first().map(|p| p.id);
                    self.selected_task = None;
                    self.filters = Filters::default();
                }
                self.touch();
                self.show_toast("Project deleted. Press Ctrl+Z to undo.");
            }
            Action::Ask(c) => self.confirm = Some(c),
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard shortcuts

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let modal_open = self.project_dialog.is_some() || self.confirm.is_some();
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        if modal_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::N))) {
            self.new_task_here();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::F))) {
            self.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S))) {
            self.save_now();
            if self.save_error.is_none() {
                self.show_toast("All changes saved");
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Num1))) {
            self.view = View::Board;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Num2))) {
            self.view = View::List;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Num3))) {
            self.view = View::Overview;
        }
        if !typing {
            if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)))
                || ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Y)))
            {
                self.redo();
            }
            if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Z))) {
                self.undo();
            }
            if ctx.input(|i| i.key_pressed(Key::Delete)) {
                if let Some(t) = self.selected_task {
                    let title = self.current().and_then(|p| p.task(t)).map(|t| t.title.clone()).unwrap_or_default();
                    self.confirm = Some(Confirm::DeleteTask(t, title));
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.drag.is_some() {
                self.drag = None;
            } else if !typing && self.selected_task.is_some() {
                self.selected_task = None;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Import / export

    fn export_all(&mut self) {
        let file = model::ExportFile { app: "Planboard".into(), version: 1, projects: self.ws.projects.clone() };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Planboard projects", &["json"])
            .set_file_name("planboard-projects.json")
            .save_file()
        else {
            return;
        };
        let res = serde_json::to_string_pretty(&file).map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string()));
        match res {
            Ok(()) => self.show_toast(format!("Exported {} projects", file.projects.len())),
            Err(e) => self.show_toast(format!("Couldn't export: {e}")),
        }
    }

    fn export_csv(&mut self) {
        let Some(p) = self.current() else { return };
        let csv = model::project_csv(p);
        let name: String = p.name.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Spreadsheet (CSV)", &["csv"])
            .set_file_name(format!("{name}.csv"))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, csv) {
            Ok(()) => self.show_toast("Tasks exported as a spreadsheet"),
            Err(e) => self.show_toast(format!("Couldn't export: {e}")),
        }
    }

    fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Planboard projects", &["json"]).pick_file() else {
            return;
        };
        let projects = std::fs::read_to_string(&path).ok().and_then(|t| {
            serde_json::from_str::<model::ExportFile>(&t)
                .map(|f| f.projects)
                .or_else(|_| serde_json::from_str::<Workspace>(&t).map(|w| w.projects))
                .ok()
        });
        match projects {
            Some(list) if !list.is_empty() => {
                self.snapshot();
                let n = list.len();
                let mut last = None;
                for p in list {
                    last = Some(self.ws.adopt_project(p));
                }
                if let Some(id) = last {
                    self.select_project(id);
                }
                self.touch();
                self.show_toast(format!("Imported {n} project{}", if n == 1 { "" } else { "s" }));
            }
            _ => self.show_toast("That file has no Planboard projects in it"),
        }
    }

    // -----------------------------------------------------------------------
    // Layout pieces

    fn menu_bar(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        egui::Panel::top("menu_bar")
            .frame(Frame::new().fill(pal.sidebar).inner_margin(Margin::symmetric(8, 3)))
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.add(Button::new("New project…").shortcut_text("")).clicked() {
                            self.actions.push(Action::OpenProjectDialog(None));
                            ui.close();
                        }
                        if ui.add_enabled(self.current().is_some(), Button::new("New task").shortcut_text("Ctrl+N")).clicked() {
                            self.new_task_here();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Import projects…").clicked() {
                            ui.close();
                            self.import();
                        }
                        if ui.button("Export all projects…").clicked() {
                            ui.close();
                            self.export_all();
                        }
                        if ui.add_enabled(self.current().is_some(), Button::new("Export tasks as spreadsheet…")).clicked() {
                            ui.close();
                            self.export_csv();
                        }
                        ui.separator();
                        if ui.add(Button::new("Save now").shortcut_text("Ctrl+S")).clicked() {
                            self.save_now();
                            self.show_toast("All changes saved");
                            ui.close();
                        }
                        if ui.button("Quit").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.add_enabled(!self.undo.is_empty(), Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() {
                            self.undo();
                            ui.close();
                        }
                        if ui.add_enabled(!self.redo.is_empty(), Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() {
                            self.redo();
                            ui.close();
                        }
                        ui.separator();
                        if ui.add(Button::new("Find tasks").shortcut_text("Ctrl+F")).clicked() {
                            self.focus_search = true;
                            ui.close();
                        }
                        if ui.add_enabled(self.current().is_some(), Button::new("Project settings…")).clicked() {
                            self.actions.push(Action::OpenProjectDialog(self.ws.selected));
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        for (v, name, key) in [(View::Board, "Board", "Ctrl+1"), (View::List, "List", "Ctrl+2"), (View::Overview, "Overview", "Ctrl+3")] {
                            if ui.add(Button::selectable(self.view == v, name).shortcut_text(key)).clicked() {
                                self.view = v;
                                ui.close();
                            }
                        }
                        ui.separator();
                        let mut dark = self.ws.dark_mode;
                        if ui.checkbox(&mut dark, "Dark mode").changed() {
                            self.ws.dark_mode = dark;
                            theme::set_dark(ui.ctx(), dark);
                            self.touch();
                            ui.close();
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        ui.set_min_width(260.0);
                        ui.label(RichText::new("Keyboard shortcuts").strong());
                        for (k, what) in [
                            ("Ctrl+N", "New task"),
                            ("Ctrl+F", "Search tasks"),
                            ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
                            ("Ctrl+1, 2, 3", "Board, list, overview"),
                            ("Delete", "Delete the open task"),
                            ("Esc", "Close the task panel"),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(k).monospace());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(what));
                            });
                        }
                        ui.separator();
                        ui.label(RichText::new("Drag cards between columns to change their status. Double-click a column name to rename it.").size(12.5));
                        ui.separator();
                        ui.label(RichText::new(format!("Your work is saved automatically to:\n{}", model::data_file().display())).size(12.0).weak());
                    });
                });
            });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        egui::Panel::left("sidebar")
            .exact_size(256.0)
            .resizable(false)
            .frame(Frame::new().fill(pal.sidebar).inner_margin(Margin { left: 12, right: 12, top: 14, bottom: 12 }).stroke(Stroke::new(1.0, pal.border)))
            .show(ui, |ui| {
                // Brand
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                    paint_logo(ui.painter(), rect, pal.accent);
                    ui.add_space(2.0);
                    ui.label(RichText::new("Planboard").size(19.0).strong().color(pal.text));
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    widgets::section(ui, "Projects", pal);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new(RichText::new("+").size(16.0).strong()).frame(false)).on_hover_text("New project").clicked() {
                            self.actions.push(Action::OpenProjectDialog(None));
                        }
                    });
                });
                ui.add_space(4.0);
                let bottom_h = 96.0;
                let list_h = (ui.available_height() - bottom_h).max(80.0);
                egui::ScrollArea::vertical().id_salt("projects").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                    let projects: Vec<(u64, String, [u8; 3], usize, usize)> = self
                        .ws
                        .projects
                        .iter()
                        .map(|p| (p.id, p.name.clone(), p.color, p.task_count(), p.done_count()))
                        .collect();
                    for (id, name, color, total, done) in projects {
                        let selected = self.ws.selected == Some(id);
                        let w = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 54.0), Sense::click());
                        let painter = ui.painter();
                        if selected {
                            painter.rect_filled(rect, 8.0, pal.accent_soft);
                        } else if resp.hovered() {
                            painter.rect_filled(rect, 8.0, pal.hover);
                        }
                        let sq = Rect::from_center_size(egui::pos2(rect.min.x + 18.0, rect.min.y + 19.0), Vec2::splat(12.0));
                        painter.rect_filled(sq, 3.0, rgb(color));
                        let name_color = if selected && !pal.dark { Color32::from_rgb(45, 48, 160) } else { pal.text };
                        let galley = painter.layout_no_wrap(name.clone(), FontId::proportional(14.0), name_color);
                        let max_w = rect.width() - 44.0;
                        let clipped = painter.with_clip_rect(Rect::from_min_size(egui::pos2(rect.min.x + 32.0, rect.min.y), Vec2::new(max_w, rect.height())));
                        clipped.galley(egui::pos2(rect.min.x + 32.0, rect.min.y + 10.0), galley, name_color);
                        let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        painter.text(
                            egui::pos2(rect.min.x + 32.0, rect.min.y + 36.0),
                            Align2::LEFT_CENTER,
                            format!("{total} task{} · {}% done", if total == 1 { "" } else { "s" }, (frac * 100.0).round() as i32),
                            FontId::proportional(11.5),
                            pal.weak,
                        );
                        let bar = Rect::from_min_size(egui::pos2(rect.max.x - 46.0, rect.min.y + 33.0), Vec2::new(36.0, 5.0));
                        widgets::paint_progress(painter, bar, frac, rgb(color), pal);
                        if resp.clicked() {
                            self.actions.push(Action::SelectProject(id));
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Open").clicked() {
                                self.actions.push(Action::SelectProject(id));
                                ui.close();
                            }
                            if ui.button("Project settings…").clicked() {
                                self.actions.push(Action::OpenProjectDialog(Some(id)));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(RichText::new("Delete project…").color(pal.danger)).clicked() {
                                self.actions.push(Action::Ask(Confirm::DeleteProject(id, name.clone())));
                                ui.close();
                            }
                        });
                    }
                    if self.ws.projects.is_empty() {
                        ui.label(RichText::new("No projects yet.").color(pal.weak));
                    }
                    ui.add_space(4.0);
                    if ui.add(Button::new(RichText::new("+  New project").color(pal.weak)).frame(false)).clicked() {
                        self.actions.push(Action::OpenProjectDialog(None));
                    }
                });

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    let status = if let Some(e) = &self.save_error {
                        RichText::new(format!("⚠ Couldn't save: {e}")).color(pal.danger).size(11.5)
                    } else if !self.can_save {
                        RichText::new("Preview mode: changes aren't saved").color(pal.weak).size(11.5)
                    } else if self.dirty {
                        RichText::new("Saving…").color(pal.weak).size(11.5)
                    } else {
                        RichText::new("✔ All changes saved").color(pal.weak).size(11.5)
                    };
                    ui.label(status).on_hover_text(model::data_file().display().to_string());
                    ui.add_space(4.0);
                    let label = if self.ws.dark_mode { "☀  Light mode" } else { "🌙  Dark mode" };
                    if ui.add(Button::new(label).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                        self.ws.dark_mode = !self.ws.dark_mode;
                        theme::set_dark(ui.ctx(), self.ws.dark_mode);
                        self.touch();
                    }
                    ui.add_space(2.0);
                    ui.separator();
                });
            });
    }

    fn header(&mut self, ui: &mut egui::Ui, project: &Project, pal: &Pal) {
        let today = model::today();
        egui::Panel::top("header")
            .frame(Frame::new().fill(pal.header).inner_margin(Margin { left: 22, right: 22, top: 14, bottom: 12 }).stroke(Stroke::new(1.0, pal.border)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    ui.painter().rect_filled(r, 4.0, rgb(project.color));
                    ui.label(RichText::new(&project.name).size(22.0).strong().color(pal.text));
                    if ui.add(Button::new(RichText::new("✏").size(13.0).color(pal.weak)).frame(false)).on_hover_text("Project settings").clicked() {
                        self.actions.push(Action::OpenProjectDialog(Some(project.id)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let new_btn = Button::new(RichText::new("+  New task").color(Color32::WHITE).strong())
                            .fill(pal.accent)
                            .corner_radius(8)
                            .min_size(Vec2::new(0.0, 32.0));
                        if ui.add(new_btn).on_hover_text("Ctrl+N").clicked() {
                            self.new_task_here();
                        }
                        ui.add_space(10.0);
                        self.view_tabs(ui, pal);
                    });
                });
                if !project.description.is_empty() {
                    ui.label(RichText::new(&project.description).color(pal.weak).size(13.0));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // Search
                    let search = TextEdit::singleline(&mut self.filters.text)
                        .hint_text("🔍  Search tasks…")
                        .desired_width(220.0)
                        .id_salt("search");
                    let r = ui.add(search);
                    if self.focus_search {
                        r.request_focus();
                        self.focus_search = false;
                    }
                    // Priority
                    let pr_text = self.filters.priority.map(|p| p.name()).unwrap_or("Any priority");
                    egui::ComboBox::from_id_salt("f_priority").selected_text(pr_text).width(120.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filters.priority, None, "Any priority");
                        for p in Priority::ALL {
                            ui.selectable_value(&mut self.filters.priority, Some(p), p.name());
                        }
                    });
                    // Label
                    let lb_text = self.filters.label.and_then(|l| project.label(l)).map(|l| l.name.clone()).unwrap_or_else(|| "Any label".into());
                    egui::ComboBox::from_id_salt("f_label").selected_text(lb_text).width(120.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filters.label, None, "Any label");
                        for l in &project.labels {
                            ui.selectable_value(&mut self.filters.label, Some(l.id), l.name.as_str());
                        }
                    });
                    // Assignee
                    let as_text = match &self.filters.assignee {
                        None => "Anyone".to_string(),
                        Some(a) if a.is_empty() => "Unassigned".to_string(),
                        Some(a) => a.clone(),
                    };
                    egui::ComboBox::from_id_salt("f_assignee").selected_text(as_text).width(130.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filters.assignee, None, "Anyone");
                        ui.selectable_value(&mut self.filters.assignee, Some(String::new()), "Unassigned");
                        for m in &project.members {
                            ui.selectable_value(&mut self.filters.assignee, Some(m.clone()), m.as_str());
                        }
                    });
                    // Due
                    egui::ComboBox::from_id_salt("f_due").selected_text(self.filters.due.name()).width(120.0).show_ui(ui, |ui| {
                        for d in DueFilter::ALL {
                            ui.selectable_value(&mut self.filters.due, d, d.name());
                        }
                    });
                    if self.filters.active() && ui.add(Button::new(RichText::new("× Clear filters").color(pal.accent)).frame(false)).clicked() {
                        self.filters = Filters::default();
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let total = project.task_count();
                        let done = project.done_count();
                        let overdue = project.all_tasks().filter(|(c, t)| t.is_overdue(today, c.is_done)).count();
                        widgets::progress_bar(ui, project.progress(), 90.0, 7.0, pal.success, pal)
                            .on_hover_text(format!("{done} of {total} tasks done"));
                        ui.label(RichText::new(format!("{done}/{total} done")).color(pal.weak).size(12.5));
                        if overdue > 0 {
                            ui.label(RichText::new(format!("{overdue} overdue")).color(pal.danger).size(12.5).strong());
                        }
                        ui.add_space(8.0);
                        // Member avatars, overlapping slightly
                        let shown: Vec<&String> = project.members.iter().take(5).collect();
                        if !shown.is_empty() {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0 * shown.len() as f32 + 10.0, 26.0), Sense::hover());
                            for (i, m) in shown.iter().enumerate().rev() {
                                let c = egui::pos2(rect.min.x + 13.0 + i as f32 * 16.0, rect.center().y);
                                ui.painter().circle_filled(c, 13.0, pal.header);
                                widgets::paint_avatar(ui.painter(), c, 11.5, m);
                            }
                            let resp = ui.interact(rect, Id::unique("members_hover"), Sense::hover());
                            resp.on_hover_text(project.members.join("\n"));
                        }
                    });
                });
            });
    }

    fn view_tabs(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        // right-to-left layout: add in reverse order
        let tabs = [(View::Overview, "Overview"), (View::List, "List"), (View::Board, "Board")];
        let frame = Frame::new().fill(pal.hover).corner_radius(8).inner_margin(Margin::same(3));
        frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (v, name) in tabs {
                let selected = self.view == v;
                let text = RichText::new(name).color(if selected { pal.text } else { pal.weak });
                let text = if selected { text.strong() } else { text };
                let b = Button::new(text)
                    .fill(if selected { pal.card } else { Color32::TRANSPARENT })
                    .stroke(if selected { Stroke::new(1.0, pal.border) } else { Stroke::NONE })
                    .corner_radius(6)
                    .min_size(Vec2::new(78.0, 26.0));
                if ui.add(b).clicked() {
                    self.view = v;
                }
            }
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, pal: &Pal) {
        egui::CentralPanel::default().frame(Frame::new().fill(pal.bg)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                paint_logo(ui.painter(), rect, pal.accent);
                ui.add_space(12.0);
                ui.label(RichText::new("No projects yet").size(22.0).strong());
                ui.label(RichText::new("Create a project to get a Kanban board for its tasks.").color(pal.weak));
                ui.add_space(12.0);
                let b = Button::new(RichText::new("+  Create a project").color(Color32::WHITE).strong()).fill(pal.accent).corner_radius(8).min_size(Vec2::new(180.0, 36.0));
                if ui.add(b).clicked() {
                    self.actions.push(Action::OpenProjectDialog(None));
                }
            });
        });
    }

    fn toast_ui(&mut self, ctx: &egui::Context, pal: &Pal) {
        let Some((msg, at)) = self.toast.clone() else { return };
        let age = self.time - at;
        let life = 3.5;
        if age > life {
            self.toast = None;
            return;
        }
        let alpha = (((life - age) / 0.4).min(1.0)) as f32;
        egui::Area::new(Id::unique("toast"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -28.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                let bg = if pal.dark { Color32::from_rgb(235, 236, 242) } else { Color32::from_rgb(33, 36, 52) };
                let fg = if pal.dark { Color32::from_rgb(25, 27, 35) } else { Color32::WHITE };
                Frame::new()
                    .fill(bg)
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(16, 10))
                    .multiply_with_opacity(alpha)
                    .show(ui, |ui| {
                        ui.label(RichText::new(msg).color(fg.gamma_multiply(alpha)));
                    });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        if self.dirty && self.can_save {
            if self.time - self.last_change > 0.8 {
                self.save_now();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(900));
            }
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frame += 1;
        ctx.request_repaint();
        if shot.frame >= SCREENSHOT_FRAME && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            ctx.request_screenshot(move |img| {
                let r = save_png(&path, &img);
                if let Ok(mut d) = done.lock() {
                    *d = Some(r);
                }
            });
        }
        let finished = shot.done.lock().ok().and_then(|mut d| d.take());
        if let Some(r) = finished {
            match r {
                Ok(()) => {
                    println!("Saved screenshot to {}", shot.path.display());
                    EXIT_CODE.store(0, Ordering::SeqCst);
                }
                Err(e) => {
                    eprintln!("Couldn't save the screenshot: {e}");
                    EXIT_CODE.store(1, Ordering::SeqCst);
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if shot.frame > SCREENSHOT_FRAME + 900 {
            eprintln!("The screenshot never arrived");
            EXIT_CODE.store(1, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.time = ctx.input(|i| i.time);
        if let Some((_, at)) = &mut self.toast {
            if *at == 0.0 {
                *at = self.time;
            }
        }
        self.shortcuts(&ctx);
        let pal = self.pal();

        self.menu_bar(ui, &pal);
        self.sidebar(ui, &pal);

        match self.current().cloned() {
            Some(project) => {
                self.header(ui, &project, &pal);
                if self.selected_task.is_some() {
                    self.details_panel(ui, &pal);
                }
                // The details panel may have changed the project; draw the latest.
                let project = self.current().cloned().unwrap_or(project);
                egui::CentralPanel::default()
                    .frame(Frame::new().fill(pal.bg).inner_margin(Margin { left: 18, right: 18, top: 16, bottom: 12 }))
                    .show(ui, |ui| match self.view {
                        View::Board => self.board_view(ui, &project, &pal),
                        View::List => self.list_view(ui, &project, &pal),
                        View::Overview => self.overview_view(ui, &project, &pal),
                    });
            }
            None => self.empty_state(ui, &pal),
        }

        self.dialogs(&ctx, &pal);
        self.toast_ui(&ctx, &pal);
        self.apply_actions();
        self.autosave(&ctx);
        self.screenshot_step(&ctx);
    }

    fn on_exit(&mut self) {
        if self.dirty {
            self.save_now();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.pal().bg.to_normalized_gamma_f32()
    }
}

pub fn paint_logo(painter: &egui::Painter, rect: Rect, accent: Color32) {
    painter.rect_filled(rect, CornerRadius::same((rect.width() * 0.26) as u8), accent);
    let w = rect.width();
    let bar_w = w * 0.17;
    let gap = w * 0.08;
    let total = bar_w * 3.0 + gap * 2.0;
    let x0 = rect.center().x - total / 2.0;
    let top = rect.min.y + w * 0.24;
    let heights = [0.52, 0.36, 0.44];
    for (i, h) in heights.iter().enumerate() {
        let x = x0 + i as f32 * (bar_w + gap);
        let r = Rect::from_min_size(egui::pos2(x, top), Vec2::new(bar_w, w * h));
        painter.rect_filled(r, CornerRadius::same((bar_w * 0.3) as u8), Color32::WHITE);
    }
    let _ = StrokeKind::Inside;
}

fn save_png(path: &Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let w = std::io::BufWriter::new(file);
    let mut enc = png::Encoder::new(w, img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let data: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// The window icon: the logo drawn into a small bitmap.
pub fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let accent = [91u8, 95, 227];
    let radius = 16.0f32;
    let inside_rounded = |x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32| -> bool {
        if x < x0 || x > x1 || y < y0 || y > y1 {
            return false;
        }
        let cx = x.clamp(x0 + r, x1 - r);
        let cy = y.clamp(y0 + r, y1 - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    let bars = [(13.0f32, 33.0f32), (28.0, 23.0), (43.0, 28.0)];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let i = (y * n + x) * 4;
            if inside_rounded(fx, fy, 2.0, 2.0, 62.0, 62.0, radius) {
                let mut c = [accent[0], accent[1], accent[2], 255];
                for (bx, h) in bars {
                    if inside_rounded(fx, fy, bx, 15.0, bx + 9.0, 15.0 + h, 3.0) {
                        c = [255, 255, 255, 255];
                    }
                }
                rgba[i..i + 4].copy_from_slice(&c);
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}
