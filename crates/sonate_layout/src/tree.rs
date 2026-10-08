//! The layout tree: an arena of boxes that is rebuilt for every layout pass.

use crate::cache::Cache;
use crate::geometry::{Point, Size};
use crate::style::Style;

/// Identifies a box in a [`LayoutTree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The result of layout for one box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    /// Border-box origin relative to the parent's border-box origin.
    pub location: Point<f64>,
    /// Border-box size.
    pub size: Size<f64>,
    /// Extent of the box's scrollable overflow, measured from its border-box origin.
    pub content_size: Size<f64>,
    /// Distance from the border-box top to the first baseline, if the box has one.
    pub first_baseline: Option<f64>,
}

pub(crate) struct NodeData {
    pub style: Style,
    /// Cached `Style::uses_percentages`.
    pub uses_percentages: bool,
    /// Cached `Style::has_sizing_properties`.
    pub has_sizing_properties: bool,
    pub children: Vec<NodeId>,
    pub measured: bool,
    pub layout: Layout,
    pub cache: Cache,
}

/// An arena of boxes. Children must be pushed before their parent.
#[derive(Default)]
pub struct LayoutTree {
    pub(crate) nodes: Vec<NodeData>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes all boxes so the tree can be rebuilt for the next pass.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Adds a box whose `children` were pushed before it.
    ///
    /// A `measured` box is a leaf whose content size comes from the measure function passed to
    /// [`compute_layout`](crate::compute_layout), e.g. text.
    pub fn push(&mut self, style: Style, children: &[NodeId], measured: bool) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("too many layout nodes"));
        debug_assert!(
            children.iter().all(|child| child.index() < self.nodes.len()),
            "children must be pushed before their parent"
        );
        self.nodes.push(NodeData {
            uses_percentages: style.uses_percentages(),
            has_sizing_properties: style.has_sizing_properties(),
            style,
            children: children.to_vec(),
            measured,
            layout: Layout::default(),
            cache: Cache::default(),
        });
        id
    }

    pub fn style(&self, id: NodeId) -> &Style {
        &self.nodes[id.index()].style
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.index()].children
    }

    pub fn layout(&self, id: NodeId) -> &Layout {
        &self.nodes[id.index()].layout
    }

    pub(crate) fn node(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.index()]
    }

    pub(crate) fn is_measured(&self, id: NodeId) -> bool {
        self.nodes[id.index()].measured
    }

    pub(crate) fn set_layout(&mut self, id: NodeId, layout: Layout) {
        self.nodes[id.index()].layout = layout;
    }

    pub(crate) fn cache_mut(&mut self, id: NodeId) -> &mut Cache {
        &mut self.nodes[id.index()].cache
    }
}
