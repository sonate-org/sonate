use anyhow::{Context, Result};
use sonate::{Engine, Id, Params};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let html_path = args
        .next()
        .context("Usage: sonate_html <path/to/file.html>")?;

    if args.next().is_some() {
        anyhow::bail!("Usage: sonate_html <path/to/file.html>");
    }

    let html = std::fs::read_to_string(&html_path)
        .with_context(|| format!("Failed to read HTML file: {html_path}"))?;

    let engine = Engine::new();
    load_html_into_engine(&engine, &html)?;

    engine
        .run(Params::default())
        .map_err(|e| anyhow::anyhow!("Engine failed: {e:?}"))
}

fn load_html_into_engine(engine: &Engine, html: &str) -> Result<()> {
    let dom = html_parser::Dom::parse(html).context("Failed to parse HTML")?;

    let mut next_id: u64 = 1;
    let root = engine.root_id();

    for node in &dom.children {
        copy_nodes(engine, node, root, &mut next_id)?;
    }

    Ok(())
}

fn copy_nodes(
    engine: &Engine,
    html_node: &html_parser::Node,
    parent: Id,
    next_id: &mut u64,
) -> Result<()> {
    match html_node {
        html_parser::Node::Element(element) => {
            // Treat <style> as stylesheet input only.
            if element.name.eq_ignore_ascii_case("style") {
                if let Some(text) = element.children.get(0).and_then(|child| child.text()) {
                    engine.add_stylesheet(text);
                }
                return Ok(());
            }

            let id = Id::from_u64(*next_id);
            *next_id += 1;

            // Sonate currently only supports "text if it's the only child".
            let text = element
                .children
                .get(0)
                .and_then(|child| child.text())
                .map(|s| s.to_owned());

            engine.create_node(id, text);
            engine.set_parent(parent, id);

            // Preserve element id as an attribute.
            if let Some(id_attr) = &element.id {
                engine.set_attribute(id, "id".to_owned(), id_attr.to_owned());
            }

            // Preserve classes.
            if !element.classes.is_empty() {
                engine.set_attribute(id, "class".to_owned(), element.classes.join(" "));
            }

            // Preserve other attributes.
            for (key, value_opt) in &element.attributes {
                if key == "id" || key == "class" {
                    continue;
                }
                if let Some(value) = value_opt {
                    engine.set_attribute(id, key.to_owned(), value.to_owned());
                }
            }

            // Recurse into element children.
            for child in &element.children {
                // Skip the text child we already captured.
                if child.text().is_some() {
                    continue;
                }
                copy_nodes(engine, child, id, next_id)?;
            }
        }
        html_parser::Node::Text(_) => {
            // Text nodes are handled as element text (when they're the only child).
        }
        _ => {
            // Ignore comments, doctypes, etc.
        }
    }

    Ok(())
}
