# sonate_layout

The layout engine behind Sonate: an implementation of
[CSS Flexible Box Layout Level 1](https://www.w3.org/TR/css-flexbox-1/) that is checked box by
box against Chrome. It has no dependencies and builds and tests in seconds:

```
cargo test -p sonate_layout
```

## Using it

Push boxes into a `LayoutTree` (children before parents), then lay it out. Leaves such as text
are `measured`: the engine asks the measure function for their content size.

```rust
use sonate_layout::*;

let mut tree = LayoutTree::new();
let text = tree.push(Style::default(), &[], true);
let root = tree.push(
    Style { size: Size::new(Dimension::Px(400.0), Dimension::Px(300.0)), ..Style::default() },
    &[text],
    false,
);
compute_layout(
    &mut tree,
    root,
    Size::new(AvailableSpace::Definite(800.0), AvailableSpace::Definite(600.0)),
    &mut |_node, input: MeasureInput| MeasureOutput {
        size: Size::new(input.known.width.unwrap_or(50.0), input.known.height.unwrap_or(10.0)),
        first_baseline: Some(8.0),
    },
);
let layout = tree.layout(text); // location relative to the parent, size, baseline
```

Every box is a flex container, as everywhere in Sonate, and every box is a containing block for
absolutely positioned children.

## How it works

`compute::compute_node` is the single recursive entry point. A parent measures a child under a
min-content, max-content or definite constraint (`RunMode::ComputeSize`) and later lays it out
with the sizes it decided (`RunMode::PerformLayout`), passing them as *known* sizes and saying
which of them are *definite* (§9.8). That is what makes nested layouts work: a stretched or
flexed size becomes the child's own size, and percentages inside it resolve only when the
spec says they may.

Results are cached per box and pass. Inputs that can't change a result are normalized away
before the lookup (`compute::normalize`), which keeps deep trees from re-measuring
exponentially; `tests/api.rs` guards this.

The flex algorithm lives in `src/flex/`, one file per step of §9:

| File | Spec |
|---|---|
| `mod.rs` | Drives §9 step by step, places items, lays out their contents |
| `items.rs` | §9.1 item generation and `order`, §9.2 #3 flex base size and hypothetical main size, §4.5 automatic minimum size |
| `lines.rs` | §9.2 #4 container main size, §9.3 #5 collecting items into lines |
| `flexible_lengths.rs` | §9.7 resolving flexible lengths (the freeze loop) |
| `cross_size.rs` | §9.4 hypothetical cross sizes, line cross sizes, `align-content: stretch`, stretching items |
| `alignment.rs` | §9.5/§9.6 auto margins, `justify-content`, `align-self`, `align-content` |
| `baseline.rs` | §8.3 baseline alignment, §8.5 the container's baseline |
| `intrinsic.rs` | §9.9 intrinsic sizes |
| `abspos.rs` | §4.1 absolutely positioned children |

Positions are computed from main-start and cross-start and converted to physical coordinates
at the end, so reverse directions and `wrap-reverse` share the same code.

## Spec coverage

| Feature | Status |
|---|---|
| `flex-direction`, `flex-wrap` (incl. `row-reverse`, `column-reverse`, `wrap-reverse`), `order` | ✅ |
| `flex-grow`, `flex-shrink`, `flex-basis` (px, %, `auto`), freezing at min/max (§9.7) | ✅ |
| `width`/`height`, `min-*`/`max-*` (px, %), `box-sizing`, automatic minimum size (§4.5) | ✅ |
| Margins (incl. `auto`), padding, borders, `row-gap`/`column-gap` (px, %) | ✅ |
| `calc()`, `min()`, `max()`, `clamp()` with percentages, resolved at layout time | ✅ |
| `justify-content`, `align-items`, `align-self`, `align-content`, incl. `start`/`end` and overflow fallbacks | ✅ |
| Baseline alignment (first baselines, synthesized baselines) | ✅ |
| Definite sizes (§9.8), percentages against indefinite sizes | ✅ |
| Intrinsic sizes of flex containers (§9.9), as Chrome computes them | ✅ |
| `position: absolute` children: static position (§4.1), insets, shrink-to-fit sizes | ✅ |
| `display: none` | ✅ |
| Text in measured leaves: line breaking (`white-space`, `overflow-wrap`, `word-break`), spacing, `text-transform`, line heights and baselines (`sonate_layout::text`); the text is placed like Chrome's anonymous flex item, so `justify-content`/`align-items` position it and `text-align` aligns its lines | ✅ |
| `flex-basis: content`, `aspect-ratio`, `safe`/`unsafe` alignment, `last baseline` | ❌ |
| `visibility: collapse`, writing modes, `direction: rtl` | ❌ |
| Block, inline and grid layout | ❌ (every box is a flex container) |

Where Chrome and the spec text differ, the engine follows Chrome:

- `space-around` and `space-evenly` fall back to `safe center` when the content overflows
  (CSS Box Alignment), so overflowing items start at the writing mode's start. Flexbox Level 1
  says `center`.
- The max-content size of a row container sums its items' `width`-based contributions and
  ignores `flex-basis`; §9.9.1 describes a different algorithm.

## Tests

- `tests/fixtures/` holds HTML fixtures with Chrome's layout of each one (`*.snap.json`). See
  [`tests/fixtures/README.md`](tests/fixtures/README.md) for writing fixtures, regenerating
  snapshots and reading failures.
- `tests/known_failures.txt` lists fixture cases that may fail (empty today).
- `tests/api.rs` covers what fixtures can't express, like the measure-call budget.
- Sonate runs the same fixtures through its HTML and CSS pipeline
  (`crates/sonate/src/layout/fixture_tests.rs`), with its own known-failures list.
