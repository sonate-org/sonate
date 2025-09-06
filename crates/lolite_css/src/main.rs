use core::borrow;
use std::{cell::RefCell, rc::Rc};

mod engine;
mod flex_layout;
mod painter;
mod plumbing;

fn main() -> anyhow::Result<()> {
    let engine = Rc::new(RefCell::new(engine::Engine::new()));

    // style
    engine.borrow_mut().style_sheet.add_rule(engine::Rule {
        selector: engine::Selector::Class("flex_container".to_owned()),
        declarations: vec![engine::Style {
            display: engine::Display::Flex,
            flex_direction: Some(engine::FlexDirection::Row),
            gap: Some(engine::Length::Px(10.0)),
            padding: Some(engine::Extend {
                top: engine::Length::Px(10.0),
                right: engine::Length::Px(10.0),
                bottom: engine::Length::Px(10.0),
                left: engine::Length::Px(10.0),
            }),
            ..Default::default()
        }],
    });

    engine.borrow_mut().style_sheet.add_rule(engine::Rule {
        selector: engine::Selector::Class("red_box".to_owned()),
        declarations: vec![engine::Style {
            background_color: Some(engine::Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }),
            border_radius: Some(engine::BorderRadius {
                top_left: engine::Length::Px(4.0),
                top_right: engine::Length::Px(4.0),
                bottom_right: engine::Length::Px(4.0),
                bottom_left: engine::Length::Px(4.0),
            }),
            border_width: Some(engine::Length::Px(2.0)),
            border_color: Some(engine::Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
            margin: Some(engine::Extend {
                top: engine::Length::Px(10.0),
                right: engine::Length::Px(10.0),
                bottom: engine::Length::Px(10.0),
                left: engine::Length::Px(10.0),
            }),
            ..Default::default()
        }],
    });

    engine.borrow_mut().style_sheet.add_rule(engine::Rule {
        selector: engine::Selector::Class("green_box".to_owned()),
        declarations: vec![engine::Style {
            background_color: Some(engine::Rgba {
                r: 0,
                g: 255,
                b: 0,
                a: 255,
            }),
            border_radius: Some(engine::BorderRadius {
                top_left: engine::Length::Px(8.0),
                top_right: engine::Length::Px(8.0),
                bottom_right: engine::Length::Px(8.0),
                bottom_left: engine::Length::Px(8.0),
            }),
            ..Default::default()
        }],
    });

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

    let params = plumbing::Params {
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

    plumbing::run_gui(&RefCell::new(params))?;

    Ok(())
}
