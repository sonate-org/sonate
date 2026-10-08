mod borders;
mod colors;
mod named_colors;
mod parser;
mod selectors;
mod values;
mod variables;

pub use parser::{parse_css, parse_declarations, parse_inline_style};
pub use variables::{resolve_custom_properties, substitute_var};
pub use selectors::{
    AttributeOperator, AttributeSelector, Combinator, ComplexSelector, CompoundSelector,
    PseudoClass, SelectorList, SimpleSelector, Specificity,
};

#[cfg(test)]
mod color_tests;

#[cfg(test)]
mod value_tests;

#[cfg(test)]
mod css_parser_tests;

#[cfg(test)]
mod border_tests;

#[cfg(test)]
mod selector_tests;
