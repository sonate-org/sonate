use std::{cell::RefCell, rc::Rc};

mod backend;
mod css_parser;
mod engine;
mod flex_layout;
mod painter;
mod windowing;

#[cfg(test)]
mod css_parser_tests;

fn main() -> anyhow::Result<()> {
    let engine = Rc::new(RefCell::new(engine::Engine::new()));

    // Example: Parse CSS from a string
    let css_content = r#"
        .flex_container {
            display: flex;
            flex-direction: row;
            gap: 10px;
            padding: 10px;
        }

        .red_box {
            background-color: #ff0000;
            border-width: 2px;
            border-color: black;
            border-radius: 8px;
            margin: 10px;
        }

        .green_box {
            background-color: green;
            border-radius: 12px 4px;
        }
    "#;

    // Parse the CSS and load it into the engine
    match css_parser::parse_css(css_content) {
        Ok(stylesheet) => {
            println!(
                "Successfully parsed CSS with {} rules",
                stylesheet.rules.len()
            );
            // Add all parsed rules to the engine
            for rule in stylesheet.rules {
                engine.borrow_mut().style_sheet.add_rule(rule);
            }
        }
        Err(err) => {
            eprintln!("Failed to parse CSS: {}", err);
        }
    }

    // document
    let mut borrow_engine = engine.borrow_mut();
    let document = &mut borrow_engine.document;

    let root = document.root_id();
    let a = document.create_node_autoid(Some("Hello".to_string()));
    let b = document.create_node_autoid(Some("World".to_string()));
    let c = document.create_node_autoid(Some("xD".to_string()));
    document.set_parent(root, a).unwrap();
    document.set_parent(root, b).unwrap();
    document.set_parent(root, c).unwrap();
    document.set_attribute(root, "class".to_owned(), "flex_container".to_owned());
    document.set_attribute(a, "class".to_owned(), "red_box".to_owned());
    document.set_attribute(b, "class".to_owned(), "green_box".to_owned());

    drop(borrow_engine);

    // run
    let engine_for_draw = engine.clone();
    let engine_for_click = engine.clone();

    let params = windowing::Params {
        on_draw: Box::new(move |canvas| {
            // Layout and paint the engine
            engine_for_draw.borrow_mut().layout();

            let mut painter = painter::Painter::new(canvas);
            painter.paint(&engine_for_draw.borrow().document);
        }),
        on_click: Some(Box::new(move |x, y| {
            // Perform hit testing in main.rs where the engine is available
            let elements = engine_for_click.borrow().find_element_at_position(x, y);

            if elements.is_empty() {
                println!("Click detected on background at ({:.1}, {:.1})", x, y);
            } else {
                println!(
                    "Click detected at ({:.1}, {:.1}) on {} elements:",
                    x,
                    y,
                    elements.len()
                );
                for (i, element_id) in elements.iter().enumerate() {
                    println!("  Level {}: Element ID {:?}", i, element_id.value());
                }
            }
        })),
    };

    windowing::run(&RefCell::new(params))?;

    Ok(())
}
