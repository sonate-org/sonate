mod backend;
mod css_parser;
mod engine;
mod flex_layout;
pub mod painter;
mod windowing;

#[cfg(test)]
mod css_parser_tests;

use parking_lot::FairMutex;
use std::sync::Arc;

// Re-export commonly used types for easier access
pub use css_parser::{parse_css, CssParser};
pub use engine::{Id, Rule, Style, StyleSheet};
pub use flex_layout::FlexLayoutEngine;
pub use windowing::{run, run_with_backend, Params};

/// Thread-safe CSS engine that can be shared across multiple threads
pub struct CssEngine {
    inner: Arc<FairMutex<engine::Engine>>,
}

impl CssEngine {
    /// Create a new CSS engine instance
    pub fn new() -> Self {
        Self {
            inner: Arc::new(FairMutex::new(engine::Engine::new())),
        }
    }

    /// Add a CSS stylesheet
    pub fn add_stylesheet(&self, css_content: &str) -> Result<(), String> {
        let stylesheet = parse_css(css_content)?;
        let mut engine = self.inner.lock();

        // Add all parsed rules to the engine
        for rule in stylesheet.rules {
            engine.style_sheet.add_rule(rule);
        }

        Ok(())
    }

    /// Create a new document node with optional text content
    pub fn create_node(&self, text: Option<String>) -> Id {
        let mut engine = self.inner.lock();
        engine.document.create_node_autoid(text)
    }

    /// Set a parent-child relationship between nodes
    pub fn set_parent(&self, parent_id: Id, child_id: Id) -> Result<(), String> {
        let mut engine = self.inner.lock();
        engine
            .document
            .set_parent(parent_id, child_id)
            .map_err(|e| e.to_string())
    }

    /// Set an attribute on a node
    pub fn set_attribute(&self, node_id: Id, key: String, value: String) {
        let mut engine = self.inner.lock();
        engine.document.set_attribute(node_id, key, value);
    }

    /// Get the root node ID of the document
    pub fn root_id(&self) -> Id {
        let engine = self.inner.lock();
        engine.document.root_id()
    }

    /// Perform layout calculation
    pub fn layout(&self) {
        let mut engine = self.inner.lock();
        engine.layout();
    }

    /// Find elements at a specific position (for hit testing)
    pub fn find_element_at_position(&self, x: f64, y: f64) -> Vec<Id> {
        let engine = self.inner.lock();
        engine.find_element_at_position(x, y)
    }

    /// Get a clone of the inner engine for drawing operations
    /// This is needed because drawing requires access to the document
    pub fn with_engine<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&engine::Engine) -> R,
    {
        let engine = self.inner.lock();
        f(&*engine)
    }

    /// Get a mutable reference to the inner engine for drawing operations
    pub fn with_engine_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut engine::Engine) -> R,
    {
        let mut engine = self.inner.lock();
        f(&mut *engine)
    }
}

impl Clone for CssEngine {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for CssEngine {
    fn default() -> Self {
        Self::new()
    }
}
