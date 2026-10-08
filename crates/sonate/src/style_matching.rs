//! Matching selectors against the document (Selectors Level 3 and parts of Level 4).

use crate::css_parser::{
    AttributeOperator, AttributeSelector, Combinator, ComplexSelector, CompoundSelector,
    PseudoClass, SelectorList, SimpleSelector,
};
use crate::layout::{Document, Node};
use crate::Id;
use std::{cell::RefCell, collections::HashSet, rc::Rc};

/// Interaction states that `:hover`, `:active`, `:focus` and `:focus-within` match.
#[derive(Clone, Debug, Default)]
pub struct ElementStates {
    /// The element under the pointer and its ancestors.
    pub hovered: HashSet<Id>,
    /// The element being pressed and its ancestors.
    pub active: HashSet<Id>,
    pub focused: Option<Id>,
    /// The focused element and its ancestors.
    pub focus_within: HashSet<Id>,
}

/// A node seen as an element.
#[derive(Clone)]
pub struct Element<'a> {
    document: &'a Document,
    node: Rc<RefCell<Node>>,
}

impl<'a> Element<'a> {
    pub fn new(document: &'a Document, node: Rc<RefCell<Node>>) -> Self {
        Self { document, node }
    }

    /// Text-only nodes (text without attributes) are not elements: they match no selector and
    /// don't count as siblings.
    pub fn is_element(node: &Node) -> bool {
        !(node.text.is_some() && node.attributes.is_empty())
    }

    fn id(&self) -> Id {
        self.node.borrow().id
    }

    fn attribute(&self, name: &str) -> Option<String> {
        self.node.borrow().attributes.get(name).cloned()
    }

    fn tag(&self) -> Option<String> {
        self.attribute("tag")
    }

    fn parent(&self) -> Option<Element<'a>> {
        let parent = self.node.borrow().parent?;
        Some(Element::new(self.document, self.document.get_node(parent)?))
    }

    /// The element siblings including this element, and this element's index among them.
    fn siblings(&self) -> (Vec<Element<'a>>, usize) {
        let Some(parent) = self.parent() else {
            return (vec![self.clone()], 0);
        };
        let siblings: Vec<Element<'a>> = parent
            .node
            .borrow()
            .children
            .iter()
            .filter(|child| Element::is_element(&child.borrow()))
            .map(|child| Element::new(self.document, child.clone()))
            .collect();
        let index = siblings
            .iter()
            .position(|sibling| Rc::ptr_eq(&sibling.node, &self.node))
            .unwrap_or(0);
        (siblings, index)
    }

    fn same_type(&self, other: &Element) -> bool {
        match (self.tag(), other.tag()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl SelectorList {
    /// The specificity of the most specific selector in the list that matches, if any.
    pub fn match_specificity(
        &self,
        element: &Element,
        states: &ElementStates,
    ) -> Option<crate::css_parser::Specificity> {
        self.0
            .iter()
            .filter(|selector| matches_complex(selector, element, states))
            .map(ComplexSelector::specificity)
            .max()
    }

    pub fn matches(&self, element: &Element, states: &ElementStates) -> bool {
        self.0
            .iter()
            .any(|selector| matches_complex(selector, element, states))
    }
}

fn matches_complex(selector: &ComplexSelector, element: &Element, states: &ElementStates) -> bool {
    matches_from(selector, selector.compounds.len() - 1, element, states)
}

/// Matches compounds `0..=index` right to left, with `element` matching compound `index`.
fn matches_from(
    selector: &ComplexSelector,
    index: usize,
    element: &Element,
    states: &ElementStates,
) -> bool {
    let (combinator, compound) = &selector.compounds[index];
    if !matches_compound(compound, element, states) {
        return false;
    }
    if index == 0 {
        return true;
    }

    match combinator {
        Combinator::Child => element
            .parent()
            .is_some_and(|parent| matches_from(selector, index - 1, &parent, states)),
        Combinator::Descendant => {
            let mut ancestor = element.parent();
            while let Some(current) = ancestor {
                if matches_from(selector, index - 1, &current, states) {
                    return true;
                }
                ancestor = current.parent();
            }
            false
        }
        Combinator::NextSibling => {
            let (siblings, position) = element.siblings();
            position > 0 && matches_from(selector, index - 1, &siblings[position - 1], states)
        }
        Combinator::SubsequentSibling => {
            let (siblings, position) = element.siblings();
            siblings[..position]
                .iter()
                .any(|sibling| matches_from(selector, index - 1, sibling, states))
        }
    }
}

fn matches_compound(
    compound: &CompoundSelector,
    element: &Element,
    states: &ElementStates,
) -> bool {
    compound
        .0
        .iter()
        .all(|simple| matches_simple(simple, element, states))
}

fn matches_simple(simple: &SimpleSelector, element: &Element, states: &ElementStates) -> bool {
    match simple {
        SimpleSelector::Universal => true,
        SimpleSelector::Type(name) => element
            .tag()
            .is_some_and(|tag| tag.eq_ignore_ascii_case(name)),
        SimpleSelector::Id(id) => element.attribute("id").is_some_and(|value| &value == id),
        SimpleSelector::Class(class) => element
            .attribute("class")
            .is_some_and(|classes| classes.split_whitespace().any(|c| c == class)),
        SimpleSelector::Attribute(attribute) => matches_attribute(attribute, element),
        SimpleSelector::PseudoClass(pseudo) => matches_pseudo_class(pseudo, element, states),
    }
}

fn matches_attribute(selector: &AttributeSelector, element: &Element) -> bool {
    let Some(value) = element.attribute(&selector.name) else {
        return false;
    };
    let Some((operator, expected)) = &selector.matcher else {
        return true;
    };
    let (value, expected) = if selector.case_insensitive {
        (value.to_lowercase(), expected.to_lowercase())
    } else {
        (value, expected.clone())
    };
    match operator {
        AttributeOperator::Equals => value == expected,
        AttributeOperator::Includes => value.split_whitespace().any(|word| word == expected),
        AttributeOperator::DashMatch => {
            value == expected || value.starts_with(&format!("{expected}-"))
        }
        // `^=`, `$=` and `*=` with an empty value match nothing.
        AttributeOperator::Prefix => !expected.is_empty() && value.starts_with(&expected),
        AttributeOperator::Suffix => !expected.is_empty() && value.ends_with(&expected),
        AttributeOperator::Substring => !expected.is_empty() && value.contains(&expected),
    }
}

fn matches_pseudo_class(pseudo: &PseudoClass, element: &Element, states: &ElementStates) -> bool {
    let id = element.id();
    match pseudo {
        PseudoClass::Hover => states.hovered.contains(&id),
        PseudoClass::Active => states.active.contains(&id),
        PseudoClass::Focus => states.focused == Some(id),
        PseudoClass::FocusWithin => states.focus_within.contains(&id),
        PseudoClass::Root => element.parent().is_none(),
        PseudoClass::Empty => {
            let node = element.node.borrow();
            node.children.is_empty() && node.text.as_deref().is_none_or(str::is_empty)
        }
        PseudoClass::FirstChild => element.siblings().1 == 0,
        PseudoClass::LastChild => {
            let (siblings, position) = element.siblings();
            position + 1 == siblings.len()
        }
        PseudoClass::OnlyChild => element.siblings().0.len() == 1,
        PseudoClass::FirstOfType => position_of_type(element, false) == 1,
        PseudoClass::LastOfType => position_of_type(element, true) == 1,
        PseudoClass::OnlyOfType => {
            position_of_type(element, false) == 1 && position_of_type(element, true) == 1
        }
        PseudoClass::NthChild(nth, of) | PseudoClass::NthLastChild(nth, of) => {
            let from_end = matches!(pseudo, PseudoClass::NthLastChild(..));
            if of.as_ref().is_some_and(|of| !of.matches(element, states)) {
                return false;
            }
            let (siblings, position) = element.siblings();
            let counted =
                |sibling: &Element| of.as_ref().is_none_or(|of| of.matches(sibling, states));
            let position = if from_end {
                siblings[position..]
                    .iter()
                    .filter(|sibling| counted(sibling))
                    .count()
            } else {
                siblings[..=position]
                    .iter()
                    .filter(|sibling| counted(sibling))
                    .count()
            };
            nth.matches(position as i32)
        }
        PseudoClass::NthOfType(nth) => nth.matches(position_of_type(element, false) as i32),
        PseudoClass::NthLastOfType(nth) => nth.matches(position_of_type(element, true) as i32),
        PseudoClass::Not(list) => !list.matches(element, states),
        PseudoClass::Is(list) | PseudoClass::Where(list) => list.matches(element, states),
    }
}

/// 1-based position among siblings of the same type, from the start or the end.
fn position_of_type(element: &Element, from_end: bool) -> usize {
    let (siblings, position) = element.siblings();
    let range = if from_end {
        &siblings[position..]
    } else {
        &siblings[..=position]
    };
    range
        .iter()
        .filter(|sibling| sibling.same_type(element))
        .count()
}
