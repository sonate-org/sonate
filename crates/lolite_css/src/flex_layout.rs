/*!
 * Flexbox Layout Engine
 *
 * This module contains the complete flexbox layout implementation moved from
 * the main engine for better code organization. It includes support for:
 * - All flex directions (row, column, row-reverse, column-reverse)
 * - Flex wrapping (nowrap, wrap, wrap-reverse)
 * - Proper coordinate calculation and child positioning
 */

use crate::engine::{
    AlignContent, AlignItems, Engine, FlexDirection, FlexWrap, JustifyContent, Node, Style,
};
use std::{cell::RefCell, rc::Rc};

/// Flexbox layout engine that handles all flexbox positioning logic
pub struct FlexLayoutEngine;

impl FlexLayoutEngine {
    pub fn new() -> Self {
        Self
    }

    /// Layout children of a flex container according to flexbox rules
    /// This is the main entry point for flex layout logic
    pub fn layout_flex_children(
        &self,
        container: Rc<RefCell<Node>>,
        style: &Style,
        engine: &Engine,
    ) {
        let flex_direction = style.flex_direction.as_ref().unwrap_or(&FlexDirection::Row);
        let flex_wrap = style.flex_wrap.as_ref().unwrap_or(&FlexWrap::NoWrap);

        let container_bounds = container.borrow().layout.bounds;
        let container_x = container_bounds.x;
        let container_y = container_bounds.y;
        let container_width = container_bounds.width;
        let container_height = container_bounds.height;

        // First pass: layout all children to get their natural sizes
        let children = container.borrow().children.clone();
        for child in &children {
            // Recursively layout child first (this will apply its styles and set its dimensions)
            engine.layout_node(child.clone(), 0.0, 0.0);
        }

        // Second pass: position children based on flex direction and wrapping
        match flex_direction {
            FlexDirection::Row => {
                self.layout_row_with_wrap(
                    &children,
                    container_x,
                    container_y,
                    container_width,
                    container_height,
                    flex_wrap,
                    style,
                );
            }
            FlexDirection::Column => {
                self.layout_column_with_wrap(
                    &children,
                    container_x,
                    container_y,
                    container_height,
                    flex_wrap,
                );
            }
            FlexDirection::RowReverse => {
                self.layout_row_reverse_with_wrap(
                    &children,
                    container_x,
                    container_y,
                    container_width,
                    flex_wrap,
                );
            }
            FlexDirection::ColumnReverse => {
                self.layout_column_reverse_with_wrap(
                    &children,
                    container_x,
                    container_y,
                    container_height,
                    flex_wrap,
                );
            }
        }
    }

    /// Layout children in a row with wrapping support
    fn layout_row_with_wrap(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_width: f64,
        container_height: f64,
        flex_wrap: &FlexWrap,
        style: &Style,
    ) {
        match flex_wrap {
            FlexWrap::NoWrap => {
                // Apply justify-content for single line
                self.apply_justify_content_row(
                    children,
                    container_x,
                    container_y,
                    container_width,
                    container_height,
                    style,
                );
            }
            FlexWrap::Wrap => {
                // First pass: organize children into lines
                let mut lines: Vec<Vec<Rc<RefCell<Node>>>> = Vec::new();
                let mut current_line: Vec<Rc<RefCell<Node>>> = Vec::new();
                let mut current_line_width = 0.0;

                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Check if this child would overflow the container width
                    if current_line_width + child_bounds.width > container_width
                        && !current_line.is_empty()
                    {
                        // Start new line
                        lines.push(current_line);
                        current_line = Vec::new();
                        current_line_width = 0.0;
                    }

                    current_line.push(child.clone());
                    current_line_width += child_bounds.width;
                }

                // Add the last line if it has children
                if !current_line.is_empty() {
                    lines.push(current_line);
                }

                // Second pass: calculate line heights
                let mut line_heights: Vec<f64> = Vec::new();
                for line in &lines {
                    let line_height = line
                        .iter()
                        .map(|child| child.borrow().layout.bounds.height)
                        .fold(0.0, f64::max);
                    line_heights.push(line_height);
                }

                // Third pass: apply align-content to position lines
                let align_content = style
                    .align_content
                    .as_ref()
                    .unwrap_or(&AlignContent::FlexStart);

                let line_start_positions = self.calculate_line_positions(
                    &line_heights,
                    container_y,
                    container_height,
                    align_content,
                );

                // Fourth pass: position children within their lines
                for (line_index, line) in lines.iter().enumerate() {
                    let line_y = line_start_positions[line_index];

                    // Position children horizontally within the line
                    let mut current_x = container_x;
                    for child in line {
                        let child_bounds = child.borrow().layout.bounds;
                        let mut child_borrow = child.borrow_mut();

                        child_borrow.layout.bounds.x = current_x;
                        child_borrow.layout.bounds.y = line_y;

                        current_x += child_bounds.width;
                    }
                }
            }
            FlexWrap::WrapReverse => {
                // TODO: Implement wrap-reverse
                // For now, behave like wrap
                self.layout_row_with_wrap(
                    children,
                    container_x,
                    container_y,
                    container_width,
                    container_height,
                    &FlexWrap::Wrap,
                    style,
                );
            }
        }
    }

    /// Layout children in a column with wrapping support
    fn layout_column_with_wrap(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_height: f64,
        flex_wrap: &FlexWrap,
    ) {
        match flex_wrap {
            FlexWrap::NoWrap => {
                // Original nowrap behavior
                let current_x = container_x;
                let mut current_y = container_y;

                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;
                    child_borrow.layout.bounds.y = current_y;

                    current_y += child_bounds.height;
                }
            }
            FlexWrap::Wrap => {
                let mut current_x = container_x;
                let mut current_y = container_y;
                let mut column_width = 0.0;

                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Check if this child would overflow the container height
                    if current_y + child_bounds.height > container_y + container_height
                        && current_y > container_y
                    {
                        // Wrap to next column
                        current_y = container_y;
                        current_x += column_width;
                        column_width = 0.0;
                    }

                    // Position child
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;
                    child_borrow.layout.bounds.y = current_y;

                    current_y += child_bounds.height;
                    column_width = column_width.max(child_bounds.width);
                }
            }
            FlexWrap::WrapReverse => {
                // TODO: Implement wrap-reverse
                // For now, behave like wrap
                self.layout_column_with_wrap(
                    children,
                    container_x,
                    container_y,
                    container_height,
                    &FlexWrap::Wrap,
                );
            }
        }
    }

    /// Layout children in row-reverse with wrapping support
    fn layout_row_reverse_with_wrap(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_width: f64,
        flex_wrap: &FlexWrap,
    ) {
        match flex_wrap {
            FlexWrap::NoWrap => {
                // Original nowrap behavior
                let mut current_x = container_x + container_width;
                let current_y = container_y;

                for child in children.iter().rev() {
                    let child_bounds = child.borrow().layout.bounds;
                    current_x -= child_bounds.width;

                    // Position child
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;
                    child_borrow.layout.bounds.y = current_y;
                }
            }
            FlexWrap::Wrap | FlexWrap::WrapReverse => {
                // TODO: Implement wrapping for row-reverse
                // For now, use nowrap behavior
                self.layout_row_reverse_with_wrap(
                    children,
                    container_x,
                    container_y,
                    container_width,
                    &FlexWrap::NoWrap,
                );
            }
        }
    }

    /// Layout children in column-reverse with wrapping support
    fn layout_column_reverse_with_wrap(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_height: f64,
        flex_wrap: &FlexWrap,
    ) {
        match flex_wrap {
            FlexWrap::NoWrap => {
                // Original nowrap behavior
                let current_x = container_x;
                let mut current_y = container_y + container_height;

                for child in children.iter().rev() {
                    let child_bounds = child.borrow().layout.bounds;
                    current_y -= child_bounds.height;

                    // Position child
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;
                    child_borrow.layout.bounds.y = current_y;
                }
            }
            FlexWrap::Wrap | FlexWrap::WrapReverse => {
                // TODO: Implement wrapping for column-reverse
                // For now, use nowrap behavior
                self.layout_column_reverse_with_wrap(
                    children,
                    container_x,
                    container_y,
                    container_height,
                    &FlexWrap::NoWrap,
                );
            }
        }
    }

    /// Apply justify-content positioning for row direction
    fn apply_justify_content_row(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_width: f64,
        container_height: f64,
        style: &Style,
    ) {
        let justify_content = style
            .justify_content
            .as_ref()
            .unwrap_or(&JustifyContent::FlexStart);
        let align_items = style.align_items.as_ref().unwrap_or(&AlignItems::FlexStart);

        // Calculate total width of all children
        let total_child_width: f64 = children
            .iter()
            .map(|child| child.borrow().layout.bounds.width)
            .sum();

        let free_space = container_width - total_child_width;

        match justify_content {
            JustifyContent::FlexStart => {
                let mut current_x = container_x;
                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child on main axis
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;

                    // Apply cross-axis alignment
                    self.apply_align_items_row(
                        &mut child_borrow,
                        container_y,
                        container_height,
                        align_items,
                    );

                    current_x += child_bounds.width;
                }
            }
            JustifyContent::FlexEnd => {
                let mut current_x = container_x + free_space;
                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child on main axis
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;

                    // Apply cross-axis alignment
                    self.apply_align_items_row(
                        &mut child_borrow,
                        container_y,
                        container_height,
                        align_items,
                    );

                    current_x += child_bounds.width;
                }
            }
            JustifyContent::Center => {
                let mut current_x = container_x + free_space / 2.0;
                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child on main axis
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;

                    // Apply cross-axis alignment
                    self.apply_align_items_row(
                        &mut child_borrow,
                        container_y,
                        container_height,
                        align_items,
                    );

                    current_x += child_bounds.width;
                }
            }
            JustifyContent::SpaceBetween => {
                if children.len() <= 1 {
                    // If only one child, behave like flex-start
                    self.apply_justify_content_row_single(
                        children,
                        container_x,
                        container_y,
                        container_height,
                        align_items,
                    );
                } else {
                    let gap = free_space / (children.len() - 1) as f64;
                    let mut current_x = container_x;

                    for child in children {
                        let child_bounds = child.borrow().layout.bounds;

                        // Position child on main axis
                        let mut child_borrow = child.borrow_mut();
                        child_borrow.layout.bounds.x = current_x;

                        // Apply cross-axis alignment
                        self.apply_align_items_row(
                            &mut child_borrow,
                            container_y,
                            container_height,
                            align_items,
                        );

                        current_x += child_bounds.width + gap;
                    }
                }
            }
            JustifyContent::SpaceAround => {
                let gap = free_space / children.len() as f64;
                let mut current_x = container_x + gap / 2.0;

                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child on main axis
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;

                    // Apply cross-axis alignment
                    self.apply_align_items_row(
                        &mut child_borrow,
                        container_y,
                        container_height,
                        align_items,
                    );

                    current_x += child_bounds.width + gap;
                }
            }
            JustifyContent::SpaceEvenly => {
                let gap = free_space / (children.len() + 1) as f64;
                let mut current_x = container_x + gap;

                for child in children {
                    let child_bounds = child.borrow().layout.bounds;

                    // Position child on main axis
                    let mut child_borrow = child.borrow_mut();
                    child_borrow.layout.bounds.x = current_x;

                    // Apply cross-axis alignment
                    self.apply_align_items_row(
                        &mut child_borrow,
                        container_y,
                        container_height,
                        align_items,
                    );

                    current_x += child_bounds.width + gap;
                }
            }
        }
    }

    /// Apply justify-content for single child (helper method)
    fn apply_justify_content_row_single(
        &self,
        children: &[Rc<RefCell<Node>>],
        container_x: f64,
        container_y: f64,
        container_height: f64,
        align_items: &AlignItems,
    ) {
        for child in children {
            let mut child_borrow = child.borrow_mut();
            child_borrow.layout.bounds.x = container_x;
            self.apply_align_items_row(
                &mut child_borrow,
                container_y,
                container_height,
                align_items,
            );
        }
    }

    /// Apply align-items positioning for row direction
    fn apply_align_items_row(
        &self,
        child: &mut std::cell::RefMut<Node>,
        container_y: f64,
        container_height: f64,
        align_items: &AlignItems,
    ) {
        match align_items {
            AlignItems::FlexStart => {
                child.layout.bounds.y = container_y;
            }
            AlignItems::Center => {
                // Center the child vertically within the container
                let child_height = child.layout.bounds.height;
                child.layout.bounds.y = container_y + (container_height - child_height) / 2.0;
            }
            AlignItems::FlexEnd => {
                // Align child to the bottom of the container
                let child_height = child.layout.bounds.height;
                child.layout.bounds.y = container_y + container_height - child_height;
            }
            AlignItems::Stretch => {
                // Stretch child to fill container height
                child.layout.bounds.y = container_y;
                child.layout.bounds.height = container_height;
            }
            AlignItems::Baseline => {
                // TODO: Implement baseline alignment
                // For now, behave like flex-start
                child.layout.bounds.y = container_y;
            }
        }
    }

    /// Calculate the starting Y positions for each line based on align-content
    fn calculate_line_positions(
        &self,
        line_heights: &[f64],
        container_y: f64,
        container_height: f64,
        align_content: &AlignContent,
    ) -> Vec<f64> {
        let total_lines_height: f64 = line_heights.iter().sum();
        let free_space = container_height - total_lines_height;
        let mut positions = Vec::new();

        match align_content {
            AlignContent::FlexStart => {
                let mut current_y = container_y;
                for &line_height in line_heights {
                    positions.push(current_y);
                    current_y += line_height;
                }
            }
            AlignContent::FlexEnd => {
                let mut current_y = container_y + free_space;
                for &line_height in line_heights {
                    positions.push(current_y);
                    current_y += line_height;
                }
            }
            AlignContent::Center => {
                let mut current_y = container_y + free_space / 2.0;
                for &line_height in line_heights {
                    positions.push(current_y);
                    current_y += line_height;
                }
            }
            AlignContent::SpaceBetween => {
                if line_heights.len() <= 1 {
                    // If only one line, behave like flex-start
                    positions.push(container_y);
                } else {
                    let gap = free_space / (line_heights.len() - 1) as f64;
                    let mut current_y = container_y;
                    for &line_height in line_heights {
                        positions.push(current_y);
                        current_y += line_height + gap;
                    }
                }
            }
            AlignContent::SpaceAround => {
                let gap = free_space / line_heights.len() as f64;
                let mut current_y = container_y + gap / 2.0;
                for &line_height in line_heights {
                    positions.push(current_y);
                    current_y += line_height + gap;
                }
            }
            AlignContent::SpaceEvenly => {
                let gap = free_space / (line_heights.len() + 1) as f64;
                let mut current_y = container_y + gap;
                for &line_height in line_heights {
                    positions.push(current_y);
                    current_y += line_height + gap;
                }
            }
            AlignContent::Stretch => {
                // Stretch lines to fill the container
                if line_heights.is_empty() {
                    return positions;
                }

                let stretched_line_height = container_height / line_heights.len() as f64;
                let mut current_y = container_y;
                for _ in line_heights {
                    positions.push(current_y);
                    current_y += stretched_line_height;
                }
            }
        }

        positions
    }
}

impl Default for FlexLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}
