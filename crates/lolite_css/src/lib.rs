mod backend;
mod css_parser;
mod engine;
mod flex_layout;
pub mod painter;
mod windowing;

#[cfg(test)]
mod css_parser_tests;

use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, RwLock,
};
use std::thread;
use std::time::{Duration, Instant};

// Re-export commonly used types for easier access
pub use css_parser::{parse_css, CssParser};
pub use engine::{Id, RenderNode, Rule, Style, StyleSheet};
pub use flex_layout::FlexLayoutEngine;
pub use windowing::{run, run_with_backend, Params};

/// Thread-safe CSS engine proxy that communicates with a dedicated data thread
pub struct CssEngine {
    sender: Sender<Command>,
    snapshot: Arc<RwLock<Option<RenderNode>>>,
}

enum Command {
    AddStylesheet(String, Sender<Result<(), String>>),
    CreateNode(Option<String>, Sender<Id>),
    SetParent(Id, Id, Sender<Result<(), String>>),
    SetAttribute(Id, String, String),
    RootId(Sender<Id>),
    Layout,
}

impl CssEngine {
    /// Create a new CSS engine instance
    pub fn new() -> Self {
        let (tx, rx): (Sender<Command>, Receiver<Command>) = mpsc::channel();
        let snapshot: Arc<RwLock<Option<RenderNode>>> = Arc::new(RwLock::new(None));
        let snapshot_for_thread = Arc::clone(&snapshot);

        // Spawn data thread owning the mutable engine
        thread::spawn(move || data_thread(rx, snapshot_for_thread));

        Self {
            sender: tx,
            snapshot,
        }
    }

    /// Add a CSS stylesheet
    pub fn add_stylesheet(&self, css_content: &str) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::AddStylesheet(
                // serialize to rules via stylesheet; send rules vector
                // We can't move stylesheet across easily via custom type, so we re-parse on thread.
                // Simpler: send original CSS string and parse on thread
                css_content.to_string(),
                tx,
            ))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    /// Create a new document node with optional text content
    pub fn create_node(&self, text: Option<String>) -> Id {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::CreateNode(text, tx))
            .expect("data thread down");
        rx.recv().expect("data thread closed")
    }

    /// Set a parent-child relationship between nodes
    pub fn set_parent(&self, parent_id: Id, child_id: Id) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::SetParent(parent_id, child_id, tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    /// Set an attribute on a node
    pub fn set_attribute(&self, node_id: Id, key: String, value: String) {
        self.sender
            .send(Command::SetAttribute(node_id, key, value))
            .expect("data thread down");
    }

    /// Get the root node ID of the document
    pub fn root_id(&self) -> Id {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(Command::RootId(tx))
            .expect("data thread down");
        rx.recv().expect("data thread closed")
    }

    /// Perform layout calculation
    pub fn layout(&self) {
        self.sender.send(Command::Layout).expect("data thread down");
    }

    /// Find elements at a specific position (for hit testing)
    pub fn find_element_at_position(&self, x: f64, y: f64) -> Vec<Id> {
        let _ = (x, y);
        // TODO: implement hit-test on snapshot
        vec![]
    }

    /// Build a render snapshot that is safe to use from any thread
    pub fn snapshot(&self) -> RenderNode {
        if let Some(snap) = self.snapshot.read().unwrap().as_ref() {
            return snap.clone();
        }
        // Fallback empty tree if layout not yet run
        RenderNode {
            id: Id::from_u64(0),
            bounds: engine::Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            style: std::sync::Arc::new(Style::default()),
            text: None,
            children: vec![],
        }
    }
}

impl Clone for CssEngine {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            snapshot: Arc::clone(&self.snapshot),
        }
    }
}

impl Default for CssEngine {
    fn default() -> Self {
        Self::new()
    }
}

// No Drop needed; when all senders are dropped, data thread exits on channel disconnect

fn data_thread(rx: Receiver<Command>, snapshot: Arc<RwLock<Option<RenderNode>>>) {
    let mut eng = engine::Engine::new();
    let mut deadline: Option<Instant> = None;

    loop {
        // Determine timeout based on debounce deadline
        let timeout = match deadline {
            Some(dl) => {
                let now = Instant::now();
                if dl <= now {
                    // Deadline expired: run layout now
                    eng.layout();
                    let root = eng.document.root_node();
                    let snap = engine::build_render_tree(root);
                    *snapshot.write().unwrap() = Some(snap);
                    deadline = None;
                    // After layout, continue to next iteration
                    continue;
                } else {
                    dl - now
                }
            }
            None => Duration::from_millis(u64::MAX / 2), // effectively wait forever
        };

        match rx.recv_timeout(timeout) {
            Ok(cmd) => match cmd {
                Command::AddStylesheet(css, resp) => match parse_css(&css) {
                    Ok(sheet) => {
                        for rule in sheet.rules {
                            eng.style_sheet.add_rule(rule);
                        }
                        let _ = resp.send(Ok(()));
                        // Start debounce only if none active; batch further changes until expiry
                        if deadline.is_none() {
                            deadline = Some(Instant::now() + Duration::from_millis(100));
                        }
                    }
                    Err(e) => {
                        let _ = resp.send(Err(e));
                    }
                },
                Command::CreateNode(text, resp) => {
                    let id = eng.document.create_node_autoid(text);
                    let _ = resp.send(id);
                    if deadline.is_none() {
                        deadline = Some(Instant::now() + Duration::from_millis(100));
                    }
                }
                Command::SetParent(p, c, resp) => {
                    let r = eng.document.set_parent(p, c).map_err(|e| e.to_string());
                    let _ = resp.send(r);
                    if deadline.is_none() {
                        deadline = Some(Instant::now() + Duration::from_millis(100));
                    }
                }
                Command::SetAttribute(id, k, v) => {
                    eng.document.set_attribute(id, k, v);
                    if deadline.is_none() {
                        deadline = Some(Instant::now() + Duration::from_millis(100));
                    }
                }
                Command::RootId(resp) => {
                    let _ = resp.send(eng.document.root_id());
                }
                Command::Layout => {
                    // Immediate layout flush
                    eng.layout();
                    let root = eng.document.root_node();
                    let snap = engine::build_render_tree(root);
                    *snapshot.write().unwrap() = Some(snap);
                    deadline = None;
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // handled at top loop when checking expired deadline
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
