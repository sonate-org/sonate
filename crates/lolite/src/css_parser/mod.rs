mod colors;
mod named_colors;
mod parser;
mod values;

pub use parser::parse_css;

#[cfg(test)]
mod color_tests;

#[cfg(test)]
mod value_tests;

#[cfg(test)]
mod css_parser_tests;
