use crate::style::{Selector, Style, StyleSheet};
use std::collections::HashMap;

pub fn apply_matching_rules(
    style: &mut Style,
    attributes: &HashMap<String, String>,
    style_sheet: &StyleSheet,
) {
    let tag_name = attributes.get("tag").map(|s| s.as_str());
    let class_attr = attributes.get("class").map(|s| s.as_str());

    for rule in &style_sheet.rules {
        let matches = match &rule.selector {
            Selector::Tag(tag) => tag_name.is_some_and(|t| t == tag.as_str()),
            Selector::Class(class_name) => class_attr
                .is_some_and(|classes| classes.split_whitespace().any(|c| c == class_name)),
        };

        if matches {
            for declaration in &rule.declarations {
                style.merge(declaration);
            }
        }
    }
}
