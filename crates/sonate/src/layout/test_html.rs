use std::collections::HashMap;

use crate::{css_parser, Id};

use super::LayoutContext;

/// Loads an HTML file and looks for top level nodes with the given example_id, or \<style>.
///
/// The root element has id = 1 and the following nodes are assigned increasing Ids.
pub fn load_html_test_example(
    html: &str,
    example_id: &str,
) -> (LayoutContext, HashMap<String, Id>) {
    let mut reader = Reader::new(example_id);

    let dom = html_parser::Dom::parse(html).expect("couldn't read html test file");

    for node in &dom.children {
        reader.find_nodes(node);
    }

    reader.ctx.layout();
    (reader.ctx, reader.nodes_by_id)
}

/// Loads a `sonate_layout` fixture (see `crates/sonate_layout/tests/fixtures/README.md`): the
/// root element and `<style>` blocks, with the case's `@matrix` overrides appended to the
/// matching elements' inline styles. Returns the element ids in document order.
pub fn load_layout_fixture(
    html: &str,
    overrides: &[sonate_layout::testing::snapshot::Override],
) -> (LayoutContext, Vec<Id>) {
    let mut dom = html_parser::Dom::parse(html).expect("couldn't read fixture");
    for node in &mut dom.children {
        apply_overrides(node, overrides);
    }

    let mut reader = Reader::new("");
    reader.ctx.set_viewport_size(10_000.0, 10_000.0);
    // The part of tools/bless/base.css that Sonate doesn't do by default: a 10px font,
    // inherited from the root like Chrome inherits it from the body. The test text measurer
    // already uses Ahem metrics.
    let root = reader.ctx.document.root_id();
    reader
        .ctx
        .document
        .set_attribute(root, "style".to_owned(), "font-size: 10px".to_owned());
    let mut has_root = false;
    for node in &dom.children {
        let html_parser::Node::Element(element) = node else {
            continue;
        };
        if element.name == "style" {
            reader.find_nodes(node);
        } else if !has_root {
            reader.copy_nodes(node, reader.ctx.document.root_id());
            has_root = true;
        }
    }

    reader.ctx.layout();
    let ids = (1..reader.next_id).map(Id::from_u64).collect();
    (reader.ctx, ids)
}

fn apply_overrides(
    node: &mut html_parser::Node,
    overrides: &[sonate_layout::testing::snapshot::Override],
) {
    let html_parser::Node::Element(element) = node else {
        return;
    };
    for o in overrides {
        let matches = match o.selector.split_at(1) {
            ("#", id) => element.id.as_deref() == Some(id),
            (".", class) => element.classes.iter().any(|c| c == class),
            _ => false,
        };
        if matches {
            let style = element
                .attributes
                .entry("style".to_owned())
                .or_insert(None)
                .get_or_insert_with(String::new);
            style.push_str(&format!("; {}: {}", o.property, o.value));
        }
    }
    for child in &mut element.children {
        apply_overrides(child, overrides);
    }
}

struct Reader {
    ctx: LayoutContext,
    example_id: String,
    next_id: u64,
    nodes_by_id: HashMap<String, Id>,
}

impl Reader {
    fn new(example_id: &str) -> Self {
        Self {
            example_id: example_id.to_owned(),
            next_id: 1,
            ctx: LayoutContext::new(),
            nodes_by_id: HashMap::new(),
        }
    }

    fn find_nodes(&mut self, html_node: &html_parser::Node) {
        if let html_parser::Node::Element(element) = html_node {
            // look for id
            if let Some(id_attr) = &element.id {
                if id_attr == &self.example_id {
                    self.copy_nodes(html_node, self.ctx.document.root_id());
                }
            }

            // look for style
            if element.name == "style" {
                let text = element
                    .children
                    .get(0)
                    .and_then(|child| child.text())
                    .expect("expected style element to have a child text");

                let stylesheet = css_parser::parse_css(text).expect("expected to load stylesheet");

                for rule in stylesheet.rules {
                    self.ctx.style_sheet.add_rule(rule);
                }
            }
        }
    }

    fn copy_nodes(&mut self, html_node: &html_parser::Node, parent: Id) {
        if let html_parser::Node::Element(element) = html_node {
            let id = Id::from_u64(self.next_id);
            self.next_id += 1;

            // we only support text if it's the only child
            let text = element
                .children
                .get(0)
                .and_then(|child| child.text())
                .map(|s| s.to_owned());

            let node = self.ctx.document.create_node(id, text);
            self.ctx.document.set_parent(parent, node).unwrap();

            // Store element tag for tag selector matching.
            self.ctx.document.set_attribute(
                node,
                "tag".to_owned(),
                element.name.to_ascii_lowercase(),
            );

            // get id
            if let Some(id_attr) = &element.id {
                self.nodes_by_id.insert(id_attr.to_owned(), node);
                self.ctx
                    .document
                    .set_attribute(node, "id".to_owned(), id_attr.to_owned());
            }

            // copy attributes
            if element.classes.len() > 0 {
                let class_str = element.classes.join(" ");
                self.ctx
                    .document
                    .set_attribute(node, "class".to_owned(), class_str);
            }

            // Attributes without a value (`<div hidden>`) are present with an empty value.
            for (key, value_opt) in &element.attributes {
                let value = value_opt.clone().unwrap_or_default();
                self.ctx.document.set_attribute(node, key.to_owned(), value);
            }

            // store in map if it has an id
            if let Some(id_attr) = &element.id {
                self.nodes_by_id.insert(id_attr.to_owned(), node);
            }

            // check multiple text children
            if element
                .children
                .iter()
                .filter(|child| child.text().is_some())
                .count()
                > 1
            {
                panic!("multiple text children not supported");
            }

            // recurse into children
            for child in &element.children {
                self.copy_nodes(child, node);
            }
        }
    }
}
