//! Geometry primitives shared by the layout algorithms.

/// A width/height pair.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }

    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Size<U> {
        Size {
            width: f(self.width),
            height: f(self.height),
        }
    }

    pub fn zip_map<U, R>(self, other: Size<U>, mut f: impl FnMut(T, U) -> R) -> Size<R> {
        Size {
            width: f(self.width, other.width),
            height: f(self.height, other.height),
        }
    }

    pub fn get(self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    pub fn set(&mut self, axis: Axis, value: T) {
        match axis {
            Axis::Horizontal => self.width = value,
            Axis::Vertical => self.height = value,
        }
    }
}

impl Size<f64> {
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

impl Size<Option<f64>> {
    pub const NONE: Self = Self::new(None, None);

    /// Fills in missing values from `other`.
    pub fn or(self, other: Self) -> Self {
        self.zip_map(other, Option::or)
    }
}

/// A point, used for positions relative to a parent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    pub fn get(self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.x,
            Axis::Vertical => self.y,
        }
    }

    pub fn set(&mut self, axis: Axis, value: T) {
        match axis {
            Axis::Horizontal => self.x = value,
            Axis::Vertical => self.y = value,
        }
    }
}

/// Values for the four sides of a box (margin, border, padding, inset).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Edges<T> {
    pub const fn all(value: T) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Edges<U> {
        Edges {
            top: f(self.top),
            right: f(self.right),
            bottom: f(self.bottom),
            left: f(self.left),
        }
    }

    /// The edge at the start (left/top) of `axis`.
    pub fn start(self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.left,
            Axis::Vertical => self.top,
        }
    }

    /// The edge at the end (right/bottom) of `axis`.
    pub fn end(self, axis: Axis) -> T {
        match axis {
            Axis::Horizontal => self.right,
            Axis::Vertical => self.bottom,
        }
    }
}

impl Edges<f64> {
    pub const ZERO: Self = Self::all(0.0);

    /// Sum of both edges along `axis`.
    pub fn sum(self, axis: Axis) -> f64 {
        self.start(axis) + self.end(axis)
    }

    pub fn sums(self) -> Size<f64> {
        Size::new(self.sum(Axis::Horizontal), self.sum(Axis::Vertical))
    }

    pub fn plus(self, other: Self) -> Self {
        Self {
            top: self.top + other.top,
            right: self.right + other.right,
            bottom: self.bottom + other.bottom,
            left: self.left + other.left,
        }
    }
}

/// A physical axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn other(self) -> Self {
        match self {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        }
    }
}

/// The space a box may fill along one axis.
///
/// See [CSS Sizing §2.1](https://www.w3.org/TR/css-sizing-3/#available).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvailableSpace {
    /// A definite amount of space in px.
    Definite(f64),
    /// Size the box under a min-content constraint.
    MinContent,
    /// Size the box under a max-content constraint.
    MaxContent,
}

impl AvailableSpace {
    pub fn into_option(self) -> Option<f64> {
        match self {
            AvailableSpace::Definite(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_definite(self) -> bool {
        matches!(self, AvailableSpace::Definite(_))
    }

    /// Shrinks a definite amount of space, never below zero.
    pub fn shrink_by(self, amount: f64) -> Self {
        match self {
            AvailableSpace::Definite(value) => AvailableSpace::Definite((value - amount).max(0.0)),
            other => other,
        }
    }

    /// Replaces the space with `value` when it is known.
    pub fn or_definite(self, value: Option<f64>) -> Self {
        value.map(AvailableSpace::Definite).unwrap_or(self)
    }
}

/// Arithmetic helpers on optional lengths, where `None` means "indefinite".
pub(crate) trait MaybeMath {
    fn maybe_min(self, other: Option<f64>) -> Self;
    fn maybe_max(self, other: Option<f64>) -> Self;
    fn maybe_clamp(self, min: Option<f64>, max: Option<f64>) -> Self;
}

impl MaybeMath for f64 {
    fn maybe_min(self, other: Option<f64>) -> Self {
        other.map_or(self, |other| self.min(other))
    }

    fn maybe_max(self, other: Option<f64>) -> Self {
        other.map_or(self, |other| self.max(other))
    }

    /// Clamps between `min` and `max`. Like CSS, `min` wins when it is larger than `max`.
    fn maybe_clamp(self, min: Option<f64>, max: Option<f64>) -> Self {
        self.maybe_min(max).maybe_max(min)
    }
}

impl MaybeMath for Option<f64> {
    fn maybe_min(self, other: Option<f64>) -> Self {
        self.map(|value| value.maybe_min(other))
    }

    fn maybe_max(self, other: Option<f64>) -> Self {
        self.map(|value| value.maybe_max(other))
    }

    fn maybe_clamp(self, min: Option<f64>, max: Option<f64>) -> Self {
        self.map(|value| value.maybe_clamp(min, max))
    }
}
