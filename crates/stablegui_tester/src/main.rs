#![allow(dead_code, unused_imports)]

use std::time::Duration;

use libloading::{Library, Symbol};
use stablegui::{stablegui_add_element, stablegui_init, Element};

/*
fn main() {
    unsafe {
        let lib = Library::new("./stablegui").expect("Failed to load library");
        let func_init: Symbol<unsafe extern "C" fn()> = lib
            .get(b"stablegui_init")
            .expect("Failed to get symbol 'stablegui_init'");
        let func_add_element: Symbol<unsafe extern "C" fn(*const Element)> = lib
            .get(b"stablegui_add_element")
            .expect("Failed to get symbol 'stablegui_add_element'");

        func_init();

        let element = Element {
            value: 0,
            parent: 0,
            attribute_count: 0,
            attributes: std::ptr::null(),
            string_value: std::ptr::null(),
        };

        func_add_element(&element as *const Element);
    }
}
*/

fn main() {
    stablegui_init();

    let element = Element {
        value: 0,
        parent: 0,
        attribute_count: 0,
        attributes: std::ptr::null(),
        string_value: std::ptr::null(),
    };

    stablegui_add_element(&element as *const Element);

    std::thread::sleep(Duration::from_secs(20));
}
