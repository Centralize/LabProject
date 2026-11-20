//! The data the app manages: projects, columns, tasks, labels and members,
//! plus loading/saving and the sample workspace shown on first start.

use chrono::{Duration, Local, NaiveDate, NaiveDateTime};
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Colours offered for projects, columns and labels.
pub const PALETTE: [[u8; 3]; 10] = [
    [99, 102, 241],  // indigo
    [59, 130, 246],  // blue
    [20, 184, 166],  // teal
    [34, 197, 94],   // green
    [234, 179, 8],   // yellow
    [245, 158, 11],  // amber
    [239, 68, 68],   // red
    [236, 72, 153],  // pink
    [139, 92, 246],  // violet
    [100, 116, 139], // slate
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub const ALL: [Priority; 4] = [Priority::Low, Priority::Medium, Priority::High, Priority::Urgent];

    pub fn name(self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Urgent => "Urgent",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Priority::Low => Color32::from_rgb(100, 116, 139),
            Priority::Medium => Color32::from_rgb(59, 130, 246),
            Priority::High => Color32::from_rgb(245, 158, 11),
            Priority::Urgent => Color32::from_rgb(239, 68, 68),
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Medium => 1,
            Priority::High => 2,
            Priority::Urgent => 3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CheckItem {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub text: String,
    pub at: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub due: Option<NaiveDate>,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub labels: Vec<u64>,
    #[serde(default)]
    pub checklist: Vec<CheckItem>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub points: u32,
    pub created: NaiveDateTime,
    pub updated: NaiveDateTime,
    #[serde(default)]
    pub completed: Option<NaiveDateTime>,
}

impl Task {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        let n = now();
        Task {
            id,
            title: title.into(),
            description: String::new(),
            priority: Priority::Medium,
            due: None,
            assignee: String::new(),
            labels: Vec::new(),
            checklist: Vec::new(),
            comments: Vec::new(),
            points: 0,
            created: n,
            updated: n,
            completed: None,
        }
    }

    pub fn checklist_progress(&self) -> (usize, usize) {
        let done = self.checklist.iter().filter(|c| c.done).count();
        (done, self.checklist.len())
    }

    pub fn is_overdue(&self, today: NaiveDate, in_done_column: bool) -> bool {
        !in_done_column && self.due.is_some_and(|d| d < today)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Column {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    /// Work-in-progress limit; 0 means no limit.
    #[serde(default)]
    pub wip_limit: u32,
    /// Tasks in this column count as finished.
    #[serde(default)]
    pub is_done: bool,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub color: [u8; 3],
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub members: Vec<String>,
    pub created: NaiveDateTime,
}

impl Project {
    pub fn all_tasks(&self) -> impl Iterator<Item = (&Column, &Task)> {
        self.columns.iter().flat_map(|c| c.tasks.iter().map(move |t| (c, t)))
    }

    pub fn task_count(&self) -> usize {
        self.columns.iter().map(|c| c.tasks.len()).sum()
    }

    pub fn done_count(&self) -> usize {
        self.columns.iter().filter(|c| c.is_done).map(|c| c.tasks.len()).sum()
    }

    pub fn progress(&self) -> f32 {
        let total = self.task_count();
        if total == 0 {
            0.0
        } else {
            self.done_count() as f32 / total as f32
        }
    }

    pub fn label(&self, id: u64) -> Option<&Label> {
        self.labels.iter().find(|l| l.id == id)
    }

    /// (column index, task index) of a task.
    pub fn locate(&self, task_id: u64) -> Option<(usize, usize)> {
        for (ci, c) in self.columns.iter().enumerate() {
            if let Some(ti) = c.tasks.iter().position(|t| t.id == task_id) {
                return Some((ci, ti));
            }
        }
        None
    }

    pub fn task(&self, task_id: u64) -> Option<&Task> {
        self.locate(task_id).map(|(c, t)| &self.columns[c].tasks[t])
    }

    pub fn task_mut(&mut self, task_id: u64) -> Option<&mut Task> {
        self.locate(task_id).map(move |(c, t)| &mut self.columns[c].tasks[t])
    }

    pub fn column_index(&self, col_id: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.id == col_id)
    }

    /// Move a task into a column, before another task (or at the end).
    pub fn move_task(&mut self, task_id: u64, to_col: u64, before: Option<u64>) -> bool {
        if before == Some(task_id) {
            return false;
        }
        let Some((ci, ti)) = self.locate(task_id) else { return false };
        let Some(target) = self.column_index(to_col) else { return false };
        let mut task = self.columns[ci].tasks.remove(ti);
        let was_done = self.columns[ci].is_done;
        let now_done = self.columns[target].is_done;
        if now_done && !was_done {
            task.completed = Some(now());
        } else if !now_done {
            task.completed = None;
        }
        if ci != target {
            task.updated = now();
        }
        let col = &mut self.columns[target];
        let idx = before
            .and_then(|b| col.tasks.iter().position(|t| t.id == b))
            .unwrap_or(col.tasks.len());
        col.tasks.insert(idx, task);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Simple,
    Software,
    Content,
    Empty,
}

impl Template {
    pub const ALL: [Template; 4] = [Template::Simple, Template::Software, Template::Content, Template::Empty];

    pub fn name(self) -> &'static str {
        match self {
            Template::Simple => "Simple (To Do, Doing, Done)",
            Template::Software => "Software (Backlog to Done, with review)",
            Template::Content => "Content (Ideas to Published)",
            Template::Empty => "Empty board",
        }
    }

    pub fn columns(self) -> Vec<(&'static str, [u8; 3], bool)> {
        match self {
            Template::Simple => vec![
                ("To Do", PALETTE[9], false),
                ("Doing", PALETTE[1], false),
                ("Done", PALETTE[3], true),
            ],
            Template::Software => vec![
                ("Backlog", PALETTE[9], false),
                ("To Do", PALETTE[0], false),
                ("In Progress", PALETTE[1], false),
                ("In Review", PALETTE[5], false),
                ("Done", PALETTE[3], true),
            ],
            Template::Content => vec![
                ("Ideas", PALETTE[8], false),
                ("Drafting", PALETTE[1], false),
                ("Editing", PALETTE[5], false),
                ("Published", PALETTE[3], true),
            ],
            Template::Empty => vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub selected: Option<u64>,
    #[serde(default)]
    pub dark_mode: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace { projects: Vec::new(), next_id: 1, selected: None, dark_mode: false }
    }
}

impl Workspace {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Make sure next_id is above every id in use (e.g. after an import or a hand edit).
    pub fn fix_ids(&mut self) {
        let mut max = self.next_id;
        for p in &self.projects {
            max = max.max(p.id);
            for l in &p.labels {
                max = max.max(l.id);
            }
            for c in &p.columns {
                max = max.max(c.id);
                for t in &c.tasks {
                    max = max.max(t.id);
                }
            }
        }
        self.next_id = max;
    }

    pub fn project(&self, id: u64) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_mut(&mut self, id: u64) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    pub fn create_project(&mut self, name: &str, description: &str, color: [u8; 3], template: Template) -> u64 {
        let pid = self.new_id();
        let mut columns = Vec::new();
        for (n, c, done) in template.columns() {
            let id = self.new_id();
            columns.push(Column { id, name: n.to_string(), color: c, wip_limit: 0, is_done: done, tasks: vec![] });
        }
        let mut labels = Vec::new();
        for (n, c) in [("Feature", PALETTE[1]), ("Bug", PALETTE[6]), ("Improvement", PALETTE[2])] {
            let id = self.new_id();
            labels.push(Label { id, name: n.to_string(), color: c });
        }
        self.projects.push(Project {
            id: pid,
            name: name.to_string(),
            description: description.to_string(),
            color,
            columns,
            labels,
            members: vec![],
            created: now(),
        });
        pid
    }

    /// Give every item of an imported project fresh ids so nothing collides.
    pub fn adopt_project(&mut self, mut p: Project) -> u64 {
        let mut label_map = Vec::new();
        p.id = self.new_id();
        for l in &mut p.labels {
            let new = self.new_id();
            label_map.push((l.id, new));
            l.id = new;
        }
        for c in &mut p.columns {
            c.id = self.new_id();
            for t in &mut c.tasks {
                t.id = self.new_id();
                t.labels = t
                    .labels
                    .iter()
                    .filter_map(|old| label_map.iter().find(|(o, _)| o == old).map(|(_, n)| *n))
                    .collect();
            }
        }
        let id = p.id;
        self.projects.push(p);
        id
    }
}

// ---------------------------------------------------------------------------
// Saving and loading

pub fn data_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("LabProject")
}

pub fn data_file() -> PathBuf {
    data_dir().join("workspace.json")
}

pub enum Loaded {
    Fresh,
    Existing(Workspace),
    /// The file couldn't be read; it was kept aside under this name.
    Broken(String),
}

pub fn load() -> Loaded {
    let path = data_file();
    let Ok(text) = std::fs::read_to_string(&path) else { return Loaded::Fresh };
    match serde_json::from_str::<Workspace>(&text) {
        Ok(mut ws) => {
            ws.fix_ids();
            Loaded::Existing(ws)
        }
        Err(_) => {
            let stamp = Local::now().format("%Y%m%d-%H%M%S");
            let aside = path.with_file_name(format!("workspace.unreadable-{stamp}.json"));
            let _ = std::fs::copy(&path, &aside);
            Loaded::Broken(aside.display().to_string())
        }
    }
}

pub fn save(ws: &Workspace) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(ws).map_err(|e| e.to_string())?;
    let tmp = dir.join("workspace.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, data_file()).map_err(|e| e.to_string())?;
    Ok(())
}

/// A file of projects exported from the app.
#[derive(Serialize, Deserialize)]
pub struct ExportFile {
    pub app: String,
    pub version: u32,
    pub projects: Vec<Project>,
}

pub fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn project_csv(p: &Project) -> String {
    let mut out = String::from("Title,Status,Priority,Due,Assignee,Labels,Points,Checklist,Description\n");
    for (c, t) in p.all_tasks() {
        let labels: Vec<&str> = t.labels.iter().filter_map(|l| p.label(*l)).map(|l| l.name.as_str()).collect();
        let (d, n) = t.checklist_progress();
        let row = [
            csv_escape(&t.title),
            csv_escape(&c.name),
            t.priority.name().to_string(),
            t.due.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            csv_escape(&t.assignee),
            csv_escape(&labels.join("; ")),
            t.points.to_string(),
            if n > 0 { format!("{d}/{n}") } else { String::new() },
            csv_escape(&t.description),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Sample workspace for the first start

struct SampleTask<'a> {
    col: usize,
    title: &'a str,
    desc: &'a str,
    pri: Priority,
    due: Option<i64>,
    who: &'a str,
    labels: &'a [usize],
    check: &'a [(&'a str, bool)],
    comments: &'a [&'a str],
    points: u32,
}

fn add_sample_tasks(ws: &mut Workspace, pid: u64, tasks: &[SampleTask]) {
    let today = today();
    for (i, s) in tasks.iter().enumerate() {
        let id = ws.new_id();
        let p = ws.project_mut(pid).expect("project exists");
        let mut t = Task::new(id, s.title);
        t.description = s.desc.to_string();
        t.priority = s.pri;
        t.due = s.due.map(|d| today + Duration::days(d));
        t.assignee = s.who.to_string();
        t.labels = s.labels.iter().filter_map(|i| p.labels.get(*i).map(|l| l.id)).collect();
        t.checklist = s.check.iter().map(|(text, done)| CheckItem { text: text.to_string(), done: *done }).collect();
        let base = now() - Duration::days(12 - (i as i64 % 10));
        t.created = base;
        t.updated = base + Duration::hours(5);
        t.comments = s
            .comments
            .iter()
            .enumerate()
            .map(|(k, c)| Comment { text: c.to_string(), at: base + Duration::hours(26 * (k as i64 + 1)) })
            .collect();
        t.points = s.points;
        if p.columns[s.col].is_done {
            t.completed = Some(t.updated);
        }
        p.columns[s.col].tasks.push(t);
    }
}

fn set_labels(ws: &mut Workspace, pid: u64, labels: &[(&str, [u8; 3])]) {
    let mut out = Vec::new();
    for (n, c) in labels {
        out.push(Label { id: ws.new_id(), name: n.to_string(), color: *c });
    }
    ws.project_mut(pid).expect("project exists").labels = out;
}

pub fn sample_workspace() -> Workspace {
    let mut ws = Workspace::default();

    // Project 1: a software-style board.
    let p1 = ws.create_project(
        "Website Redesign",
        "Refresh the public website: new look, faster pages and an easier checkout.",
        PALETTE[0],
        Template::Software,
    );
    set_labels(
        &mut ws,
        p1,
        &[
            ("Design", PALETTE[7]),
            ("Frontend", PALETTE[1]),
            ("Backend", PALETTE[2]),
            ("Bug", PALETTE[6]),
            ("Content", PALETTE[5]),
        ],
    );
    {
        let p = ws.project_mut(p1).unwrap();
        p.members = vec!["Alex Morgan".into(), "Sam Lee".into(), "Jordan Kim".into(), "Riley Chen".into()];
        p.columns[2].wip_limit = 3;
    }
    add_sample_tasks(
        &mut ws,
        p1,
        &[
            SampleTask { col: 0, title: "Set up analytics dashboard", desc: "Track visits, sign-ups and checkout drop-off.", pri: Priority::Low, due: None, who: "Riley Chen", labels: &[2], check: &[], comments: &[], points: 3 },
            SampleTask { col: 0, title: "Write accessibility guidelines", desc: "Colour contrast, keyboard navigation and alt text rules for the team.", pri: Priority::Medium, due: Some(21), who: "Sam Lee", labels: &[4], check: &[], comments: &[], points: 2 },
            SampleTask { col: 0, title: "Add dark mode to the site", desc: "", pri: Priority::Low, due: None, who: "", labels: &[0, 1], check: &[], comments: &[], points: 5 },
            SampleTask { col: 1, title: "Design pricing page", desc: "Three plans side by side, with a yearly/monthly switch and an FAQ below.", pri: Priority::High, due: Some(5), who: "Alex Morgan", labels: &[0], check: &[("Wireframe", true), ("High-fidelity mock-up", false), ("Review with team", false)], comments: &["Let's keep the FAQ short, five questions at most."], points: 5 },
            SampleTask { col: 1, title: "Newsletter sign-up form", desc: "Small form in the footer with a thank-you message.", pri: Priority::Medium, due: Some(9), who: "Jordan Kim", labels: &[1, 2], check: &[], comments: &[], points: 2 },
            SampleTask { col: 2, title: "Build new navigation menu", desc: "Responsive top menu with a drop-down for products and a mobile drawer.", pri: Priority::High, due: Some(2), who: "Jordan Kim", labels: &[1], check: &[("Desktop layout", true), ("Mobile drawer", true), ("Keyboard support", false), ("Animations", false)], comments: &["Mobile drawer is done, starting keyboard support next."], points: 8 },
            SampleTask { col: 2, title: "Migrate blog posts", desc: "Move the 120 existing posts to the new content system and fix broken images.", pri: Priority::Medium, due: Some(-1), who: "Sam Lee", labels: &[4, 2], check: &[("Export old posts", true), ("Import", false), ("Check images", false)], comments: &[], points: 5 },
            SampleTask { col: 2, title: "Fix mobile layout on checkout", desc: "The pay button is hidden below the keyboard on small phones.", pri: Priority::Urgent, due: Some(1), who: "Riley Chen", labels: &[3, 1], check: &[], comments: &["Happens on screens narrower than 360px.", "Found the cause: fixed footer height."], points: 3 },
            SampleTask { col: 3, title: "Homepage hero illustrations", desc: "Three illustrations for the top of the homepage, in the new brand colours.", pri: Priority::Medium, due: Some(3), who: "Alex Morgan", labels: &[0], check: &[("Sketches", true), ("Colour versions", true), ("Export for web", true)], comments: &[], points: 3 },
            SampleTask { col: 4, title: "Kick-off meeting and goals", desc: "Agree on goals, scope and the launch date.", pri: Priority::Low, due: None, who: "Alex Morgan", labels: &[], check: &[], comments: &[], points: 1 },
            SampleTask { col: 4, title: "Choose colours and fonts", desc: "", pri: Priority::Medium, due: None, who: "Alex Morgan", labels: &[0], check: &[], comments: &[], points: 2 },
            SampleTask { col: 4, title: "Audit current site speed", desc: "Measure load times of the ten most visited pages.", pri: Priority::High, due: None, who: "Riley Chen", labels: &[2], check: &[], comments: &[], points: 3 },
        ],
    );

    // Project 2: a simple board.
    let p2 = ws.create_project(
        "Mobile App Launch",
        "Get version 1.0 of the app into the stores.",
        PALETTE[2],
        Template::Simple,
    );
    set_labels(&mut ws, p2, &[("iOS", PALETTE[9]), ("Android", PALETTE[3]), ("Marketing", PALETTE[7]), ("Bug", PALETTE[6])]);
    {
        let p = ws.project_mut(p2).unwrap();
        p.members = vec!["Sam Lee".into(), "Taylor Brooks".into(), "Jordan Kim".into()];
    }
    add_sample_tasks(
        &mut ws,
        p2,
        &[
            SampleTask { col: 0, title: "Store screenshots and description", desc: "", pri: Priority::High, due: Some(6), who: "Taylor Brooks", labels: &[2], check: &[("Write description", false), ("Six screenshots", false)], comments: &[], points: 2 },
            SampleTask { col: 0, title: "Beta test with 20 users", desc: "Collect feedback through a short form.", pri: Priority::Medium, due: Some(10), who: "Sam Lee", labels: &[0, 1], check: &[], comments: &[], points: 3 },
            SampleTask { col: 1, title: "Push notifications", desc: "Reminders for upcoming tasks.", pri: Priority::High, due: Some(4), who: "Jordan Kim", labels: &[0, 1], check: &[("iOS", true), ("Android", false)], comments: &[], points: 5 },
            SampleTask { col: 1, title: "Crash when rotating on tablets", desc: "", pri: Priority::Urgent, due: Some(0), who: "Jordan Kim", labels: &[3, 1], check: &[], comments: &[], points: 2 },
            SampleTask { col: 2, title: "Sign-in screen", desc: "", pri: Priority::Medium, due: None, who: "Sam Lee", labels: &[0, 1], check: &[], comments: &[], points: 3 },
        ],
    );

    // Project 3: a content board.
    let p3 = ws.create_project(
        "Spring Marketing Campaign",
        "Blog posts, videos and a newsletter for the spring release.",
        PALETTE[5],
        Template::Content,
    );
    set_labels(&mut ws, p3, &[("Blog", PALETTE[1]), ("Video", PALETTE[6]), ("Social", PALETTE[7]), ("Email", PALETTE[2])]);
    {
        let p = ws.project_mut(p3).unwrap();
        p.members = vec!["Taylor Brooks".into(), "Alex Morgan".into()];
    }
    add_sample_tasks(
        &mut ws,
        p3,
        &[
            SampleTask { col: 0, title: "Customer story interview", desc: "", pri: Priority::Low, due: None, who: "", labels: &[0], check: &[], comments: &[], points: 2 },
            SampleTask { col: 1, title: "\"What's new\" blog post", desc: "", pri: Priority::High, due: Some(3), who: "Taylor Brooks", labels: &[0], check: &[], comments: &[], points: 3 },
            SampleTask { col: 2, title: "Product tour video", desc: "Two minutes, with captions.", pri: Priority::Medium, due: Some(8), who: "Alex Morgan", labels: &[1, 2], check: &[("Script", true), ("Recording", true), ("Edit", false)], comments: &[], points: 5 },
            SampleTask { col: 3, title: "Spring newsletter", desc: "", pri: Priority::Medium, due: None, who: "Taylor Brooks", labels: &[3], check: &[], comments: &[], points: 1 },
        ],
    );

    ws.selected = Some(p1);
    ws
}
