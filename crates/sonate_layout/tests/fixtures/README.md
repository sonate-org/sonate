# Layout fixtures

Each `*.html` file here is one layout scenario. Chrome is the reference: `tools/bless` lays out
every fixture in headless Chrome and stores the result next to it as `*.snap.json`. The tests
lay out the same tree with `sonate_layout` and compare every box.

```
cargo test -p sonate_layout --test fixtures              # all fixtures
cargo test -p sonate_layout --test fixtures -- nested/   # names containing "nested/"
```

## Writing a fixture

```html
<!--
One line saying what this checks.
@matrix #root flex-direction: row | column
@matrix .item align-self: flex-start | center
-->
<div id="root" style="width: 300px; height: 200px">
  <div class="item" style="width: 50px; height: 40px"></div>
  <div class="item">Hello world</div>
</div>
```

- **One root element** with an explicit `width` and `height` in px and no margins. Allowed
  elements: `div`, `span`, `section`, `article`, `header`, `footer`, `nav`, `main`, `aside` and
  `label` (they have no browser default styles that matter). Styles can be inline or in
  `<style>` blocks.
- **Every element is a flex container** (`display: flex`), as in Sonate. `base.css` in
  `tools/bless` sets this, along with `position: relative`; the font is set on `body` and
  inherited.
- **`@matrix <selector> <property>: a | b | c`** repeats the fixture for every value. Several
  lines combine into all combinations (at most 64). The selector is `#id` or `.class`, and the
  value is applied as an inline style, so shorthands like `margin: 0 auto` work.
- **`@state <selector> hover | active | focus | none`** is an axis like `@matrix` that puts the
  element into an interaction state: bless moves or presses the real mouse over the element's
  center, or focuses it (which needs a `tabindex`). Pressing also focuses, like in browsers.
- **`@check color background-color …`** records these computed values (as `getComputedStyle`
  reports them) for every element. Sonate's suite compares them, so CSS that doesn't affect
  layout (selectors, the cascade, colors) can be tested too. The layout engine's own suite
  ignores them.
- **`@tolerance 0.25`** loosens the comparison for this fixture (the default is 1/16 px).
- **Text** may only be an element's only child and may only contain letters and spaces. It uses
  the [Ahem](../../tools/bless/AHEM_LICENSE.txt) font: every glyph is a 1em square, so
  `Hello world` at the default 10px is exactly 110×10. Lines break at spaces.
- **Borders** need a style to have a width: `border: 2px solid`.
- Only the properties `sonate_layout` understands may change; bless rejects anything else
  (`calc()`, `writing-mode`, `aspect-ratio`, …) rather than recording a layout the engine
  can't reproduce.

Keep fixtures small and focused: a few boxes, one idea, and a matrix for the variations. Prefer
sizes that divide evenly so rounding never matters.

## Updating snapshots

Run bless after adding or changing a fixture, or after changing `base.css`:

```
cd crates/sonate_layout/tools/bless
deno task bless            # all fixtures
deno task bless nested/    # fixtures whose name contains "nested/"
```

Bless downloads the Chrome for Testing build pinned in `bless.ts` (`CHROME_VERSION`) into your
cache directory on first use, so everyone records the same layouts. Set
`BLESS_CHROME=/path/to/chrome` to try another build. When upgrading the pin, re-bless
everything and review the snapshot diff: it shows exactly where Chrome's behaviour changed.

The snapshot stores a hash of the fixture and `base.css`, and the tests fail with "run bless"
when they don't match.

## Reading failures

Only the first new failing case of a fixture prints a tree. Rects are `x,y widthxheight`
relative to the parent's border box. The tree stops at the first wrong node, because wrong
descendants usually follow from it:

```
NEW FAILURE nested/column_in_row [#inner justify-content=center, #inner align-items=normal]
    #root                    0,0 300x200
      #inner                   0,0 80x200
        div[0]                   10,10 60x40   ✗ expected 10,55 60x40
        … 1 matching
```

The other failing cases get a one-line summary. Every failing case is also drawn in
`target/sonate_layout-fixtures-report.html`, with Chrome's boxes next to the engine's.

## Known failures

`../known_failures.txt` lists the cases that fail today, so the suite stays green while
features are missing:

- an unlisted case that fails is an error (a regression);
- a listed case that passes is an error too, so the list only ever shrinks;
- `FIXTURES_UPDATE=prune cargo test -p sonate_layout --test fixtures` removes entries that
  pass now;
- `FIXTURES_UPDATE=accept …` also adds the current failures. Only use it deliberately;
- `FIXTURES_IGNORE_KNOWN=1 …` reports listed failures like new ones, with their trees.

## Sonate's own pipeline

`crates/sonate/src/layout/fixture_tests.rs` runs the same fixtures through Sonate's HTML loader,
CSS parser and cascade, so gaps in CSS support show up as failures too. Filter it with
`FIXTURE_FILTER=nested/ cargo test -p sonate layout_fixtures`; its known failures are in
`crates/sonate/src/layout/fixture_known_failures.txt`.
