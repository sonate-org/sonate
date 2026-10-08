//! `calc()`, `min()`, `max()` and `clamp()` with px and % values, resolved at layout time when
//! the percentage basis is known. (Other units become px before layout.)
//! <https://www.w3.org/TR/css-values-4/#calc-func>

/// A math expression over px and percentages.
#[derive(Clone, Debug, PartialEq)]
pub enum Calc {
    Px(f64),
    /// A percentage, where `Percent(50.0)` is `50%`.
    Percent(f64),
    Sum(Vec<Calc>),
    /// Multiplication by a number (also used for subtraction and division).
    Scale(Box<Calc>, f64),
    Min(Vec<Calc>),
    Max(Vec<Calc>),
    /// `clamp(min, value, max)`
    Clamp(Box<Calc>, Box<Calc>, Box<Calc>),
}

impl Calc {
    /// Evaluates the expression. Returns `None` if it contains a percentage and the basis is
    /// indefinite.
    pub fn resolve(&self, basis: Option<f64>) -> Option<f64> {
        Some(match self {
            Calc::Px(px) => *px,
            Calc::Percent(percent) => basis? * percent / 100.0,
            Calc::Sum(terms) => {
                let mut sum = 0.0;
                for term in terms {
                    sum += term.resolve(basis)?;
                }
                sum
            }
            Calc::Scale(value, factor) => value.resolve(basis)? * factor,
            Calc::Min(values) => fold(values, basis, f64::min)?,
            Calc::Max(values) => fold(values, basis, f64::max)?,
            // Like CSS, the minimum wins over the maximum.
            Calc::Clamp(min, value, max) => value
                .resolve(basis)?
                .min(max.resolve(basis)?)
                .max(min.resolve(basis)?),
        })
    }

    /// Whether the expression contains a percentage.
    pub fn has_percentage(&self) -> bool {
        match self {
            Calc::Px(_) => false,
            Calc::Percent(_) => true,
            Calc::Scale(value, _) => value.has_percentage(),
            Calc::Sum(values) | Calc::Min(values) | Calc::Max(values) => {
                values.iter().any(Calc::has_percentage)
            }
            Calc::Clamp(min, value, max) => {
                min.has_percentage() || value.has_percentage() || max.has_percentage()
            }
        }
    }
}

fn fold(values: &[Calc], basis: Option<f64>, combine: fn(f64, f64) -> f64) -> Option<f64> {
    let mut result: Option<f64> = None;
    for value in values {
        let value = value.resolve(basis)?;
        result = Some(result.map_or(value, |result| combine(result, value)));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_against_the_basis() {
        // calc(50% - 10px)
        let calc = Calc::Sum(vec![Calc::Percent(50.0), Calc::Scale(Box::new(Calc::Px(10.0)), -1.0)]);
        assert_eq!(calc.resolve(Some(200.0)), Some(90.0));
        assert_eq!(calc.resolve(None), None);
        assert!(calc.has_percentage());
    }

    #[test]
    fn min_max_clamp() {
        let min = Calc::Min(vec![Calc::Percent(100.0), Calc::Px(600.0)]);
        assert_eq!(min.resolve(Some(400.0)), Some(400.0));
        assert_eq!(min.resolve(Some(800.0)), Some(600.0));
        let clamp = Calc::Clamp(
            Box::new(Calc::Px(100.0)),
            Box::new(Calc::Percent(50.0)),
            Box::new(Calc::Px(50.0)),
        );
        // min wins over max
        assert_eq!(clamp.resolve(Some(1000.0)), Some(100.0));
        assert_eq!(Calc::Max(vec![Calc::Px(3.0), Calc::Px(5.0)]).resolve(None), Some(5.0));
    }
}
