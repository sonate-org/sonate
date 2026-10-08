//! Per-node memoization of layout results within one layout pass.
//!
//! A flex item is typically measured several times (min-content, max-content, at its flexed
//! size) before its final layout. Without caching, nested containers would re-measure their
//! subtrees exponentially often.

use crate::compute::{LayoutInput, LayoutOutput, RunMode};

#[derive(Default)]
pub(crate) struct Cache {
    /// Every size computed in this pass. Inputs are normalized (see `compute::normalize`), so
    /// a node rarely sees more than a handful of distinct ones.
    measure: Vec<(LayoutInput, LayoutOutput)>,
    /// Only one slot: performing layout writes the children's positions, so a result is only
    /// reusable while it was the most recent layout of this subtree.
    layout: Option<(LayoutInput, LayoutOutput)>,
}

impl Cache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn get(&self, input: &LayoutInput) -> Option<LayoutOutput> {
        match input.mode {
            RunMode::PerformLayout => self
                .layout
                .filter(|(key, _)| key == input)
                .map(|(_, output)| output),
            RunMode::ComputeSize => self
                .measure
                .iter()
                .chain(&self.layout)
                .find(|(key, _)| key.same_sizing_inputs(input))
                .map(|(_, output)| *output),
        }
    }

    pub fn store(&mut self, input: LayoutInput, output: LayoutOutput) {
        match input.mode {
            RunMode::PerformLayout => self.layout = Some((input, output)),
            RunMode::ComputeSize => self.measure.push((input, output)),
        }
    }
}
