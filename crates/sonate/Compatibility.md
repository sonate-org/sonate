# Compatibility

These tables lists the specifications enumerated in **Section 2 (Classification of CSS Specifications)** of [CSS Snapshot 2025](https://www.w3.org/TR/css-2025/).

## CSS 2.1

| Specification | Status |
|---|---|
| [Selectors](https://www.w3.org/TR/CSS2/selector.html) | Supported except pseudo-elements, `:lang()` and `:link`/`:visited` |
| [Assigning Property Values, Cascading, and Inheritance](https://www.w3.org/TR/CSS2/cascade.html) | Supported for author styles: specificity, `!important`, the `style` attribute, inheritance and `inherit`. No `@import` or media types |
| [Media Types](https://www.w3.org/TR/CSS2/media.html) | |
| [Box Model](https://www.w3.org/TR/CSS2/box.html) | Margins, padding and borders are supported |
| [Visual Formatting Model](https://www.w3.org/TR/CSS2/visuren.html) | Partial support. Every box is a flex container; `position: absolute` and `display: none` are supported |
| [Visual Formatting Model Details](https://www.w3.org/TR/CSS2/visudet.html) | |
| [Visual Effects (Overflow and Clipping)](https://www.w3.org/TR/CSS2/visufx.html) | |
| [Generated Content, Automatic Numbering, and Lists](https://www.w3.org/TR/CSS2/generate.html) | |
| [Paged Media](https://www.w3.org/TR/CSS2/page.html) | |
| [Colors and Backgrounds](https://www.w3.org/TR/CSS2/colors.html) | Colors are supported |
| [Fonts](https://www.w3.org/TR/CSS2/fonts.html) | |
| [Text](https://www.w3.org/TR/CSS2/text.html) | |
| [Tables](https://www.w3.org/TR/CSS2/tables.html) | |
| [User Interface](https://www.w3.org/TR/CSS2/ui.html) | |

## CSS 3

| Specification | Shortname | Status |
|---|---|---|
| CSS Syntax Module Level 3 | [CSS-SYNTAX-3](https://www.w3.org/TR/css-syntax-3/) | |
| CSS Style Attributes | [CSS-STYLE-ATTR](https://www.w3.org/TR/css-style-attr/) | Supported |
| Media Queries Level 3 | [CSS3-MEDIAQUERIES](https://www.w3.org/TR/css3-mediaqueries/) | |
| CSS Conditional Rules Module Level 3 | [CSS-CONDITIONAL-3](https://www.w3.org/TR/css-conditional-3/) | |
| Selectors Level 3 | [SELECTORS-3](https://www.w3.org/TR/selectors-3/) | Supported except pseudo-elements, namespaces, `:lang()`, `:target`, `:link`/`:visited` and the UI element states (`:checked`, `:disabled`, …). `:focus` applies to text inputs and elements with `tabindex` |
| CSS Namespaces Module Level 3 | [CSS3-NAMESPACE](https://www.w3.org/TR/css-namespaces/) | |
| CSS Cascading and Inheritance Level 4 | [CSS-CASCADE-4](https://www.w3.org/TR/css-cascade-4/) | Partial support. Author origin only: specificity, order, `!important`, inheritance, `inherit`/`initial`/`unset` (`revert` acts as `unset`). No `@import` or `all` |
| CSS Values and Units Module Level 3 | [CSS-VALUES-3](https://www.w3.org/TR/css-values-3/) | |
| CSS Custom Properties for Cascading Variables Module Level 1 | [CSS-VARIABLES-1](https://www.w3.org/TR/css-variables-1/) | |
| CSS Box Model Module Level 3 | [CSS-BOX-3](https://www.w3.org/TR/css-box-3/) | |
| CSS Color Module Level 4 | [CSS-COLOR-4](https://www.w3.org/TR/css-color-4/) | Partial support. Missing `opacity`, `color: inherit`, `lab`, `lch`, `oklab`, `oklch`, `color`, `currentcolor`. Only sRGB color space supported. |
| CSS Backgrounds and Borders Module Level 3 | [CSS-BACKGROUNDS-3](https://www.w3.org/TR/css-backgrounds-3/) | Partial support. Only background colors and solid borders are supported. Different `border-width` properties for each side is not supported. |
| CSS Images Module Level 3 | [CSS-IMAGES-3](https://www.w3.org/TR/css-images-3/) | |
| CSS Fonts Module Level 3 | [CSS-FONTS-3](https://www.w3.org/TR/css-fonts-3/) | |
| CSS Writing Modes Level 3 | [CSS-WRITING-MODES-3](https://www.w3.org/TR/css-writing-modes-3/) | |
| CSS Multi-column Layout Module Level 1 | [CSS-MULTICOL-1](https://www.w3.org/TR/css-multicol-1/) | |
| CSS Flexible Box Layout Module Level 1 | [CSS-FLEXBOX-1](https://www.w3.org/TR/css-flexbox-1/) | Supported except `flex-basis: content` and `visibility: collapse`; checked against Chrome. See [sonate_layout](../sonate_layout/README.md#spec-coverage) |
| CSS Basic User Interface Module Level 3 | [CSS-UI-3](https://www.w3.org/TR/css-ui-3/) | |
| CSS Containment Module Level 1 | [CSS-CONTAIN-1](https://www.w3.org/TR/css-contain-1/) | |
| CSS Transforms Module Level 1 | [CSS-TRANSFORMS-1](https://www.w3.org/TR/css-transforms-1/) | |
| Compositing and Blending Level 1 | [COMPOSITING](https://www.w3.org/TR/compositing-1/) | |
| CSS Easing Functions Level 1 | [CSS-EASING-1](https://www.w3.org/TR/css-easing-1/) | |
| CSS Counter Styles Level 3 | [CSS-COUNTER-STYLES-3](https://www.w3.org/TR/css-counter-styles-3/) | |
| Media Queries Level 4 | [MEDIAQUERIES-4](https://www.w3.org/TR/mediaqueries-4/) | |
| CSS Scroll Snap Module Level 1 | [CSS-SCROLL-SNAP-1](https://www.w3.org/TR/css-scroll-snap-1/) | |
| CSS Scrollbars Styling Module Level 1 | [CSS-SCROLLBARS-1](https://www.w3.org/TR/css-scrollbars-1/) | |
| CSS Grid Layout Module Level 1 | [CSS-GRID-1](https://www.w3.org/TR/css-grid-1/) | |
| CSS Grid Layout Module Level 2 | [CSS-GRID-2](https://www.w3.org/TR/css-grid-2/) | |
| CSS Cascading and Inheritance Module Level 5 | [CSS-CASCADE-5](https://www.w3.org/TR/css-cascade-5/) | |
| CSS Color Adjustment Module Level 1 | [CSS-COLOR-ADJUST-1](https://www.w3.org/TR/css-color-adjust-1/) | |
| CSS Conditional Rules Module Level 4 | [CSS-CONDITIONAL-4](https://www.w3.org/TR/css-conditional-4/) | |
| CSS Display Module Level 3 | [CSS-DISPLAY-3](https://www.w3.org/TR/css-display-3/) | |
| CSS Writing Modes Level 4 | [CSS-WRITING-MODES-4](https://www.w3.org/TR/css-writing-modes-4/) | |
| CSS Fragmentation Module Level 3 | [CSS-BREAK-3](https://www.w3.org/TR/css-break-3/) | |
| CSS Box Alignment Module Level 3 | [CSS-ALIGN-3](https://www.w3.org/TR/css-align-3/) | Partial support. Flexbox alignment incl. `normal`, `start`, `end` and gaps; no `safe`/`unsafe`, `last baseline` or `left`/`right` |
| CSS Shapes Module Level 1 | [CSS-SHAPES-1](https://www.w3.org/TR/css-shapes-1/) | |
| CSS Text Module Level 3 | [CSS-TEXT-3](https://www.w3.org/TR/css-text-3/) | |
| CSS Text Decoration Module Level 3 | [CSS-TEXT-DECOR-3](https://www.w3.org/TR/css-text-decor-3/) | |
| CSS Masking Module Level 1 | [CSS-MASKING-1](https://www.w3.org/TR/css-masking-1/) | |
| CSS Speech Module Level 1 | [CSS-SPEECH-1](https://www.w3.org/TR/css-speech-1/) | |
| CSS View Transitions Module Level 1 | [CSS-VIEW-TRANSITIONS-1](https://www.w3.org/TR/css-view-transitions-1/) | |
| CSS Transitions | [CSS-TRANSITIONS-1](https://www.w3.org/TR/css-transitions-1/) | |
| CSS Animations Level 1 | [CSS-ANIMATIONS-1](https://www.w3.org/TR/css-animations-1/) | |
| CSS Will Change Module Level 1 | [CSS-WILL-CHANGE-1](https://www.w3.org/TR/css-will-change-1/) | |
| Filter Effects Module Level 1 | [FILTER-EFFECTS-1](https://www.w3.org/TR/filter-effects-1/) | |
| CSS Font Loading Module Level 3 | [CSS-FONT-LOADING-3](https://www.w3.org/TR/css-font-loading/) | |
| CSS Box Sizing Module Level 3 | [CSS-SIZING-3](https://www.w3.org/TR/css-sizing-3/) | Partial support. `width`/`height`, `min-*`/`max-*` and `box-sizing` in px and %; no `min-content`/`max-content`/`fit-content` keywords |
| CSS Transforms Module Level 2 | [CSS-TRANSFORMS-2](https://www.w3.org/TR/css-transforms-2/) | |
| CSS Lists and Counters Module Level 3 | [CSS-LISTS-3](https://www.w3.org/TR/css-lists-3/) | |
| CSS Logical Properties and Values Level 1 | [CSS-LOGICAL-1](https://www.w3.org/TR/css-logical-1/) | |
| CSS Positioned Layout Module Level 3 | [CSS-POSITION-3](https://www.w3.org/TR/css-position-3/) | Partial support. `position: absolute` with `top`/`right`/`bottom`/`left`/`inset`; every box is a containing block |
| Resize Observer | [RESIZE-OBSERVER-1](https://www.w3.org/TR/resize-observer-1/) | |
| Web Animations | [WEB-ANIMATIONS-1](https://www.w3.org/TR/web-animations-1/) | |
| CSS Fonts Module Level 4 | [CSS-FONTS-4](https://www.w3.org/TR/css-fonts-4/) | |
| Motion Path Module Level 1 | [MOTION-1](https://www.w3.org/TR/motion-1/) | |
| CSS Scroll Anchoring Module Level 1 | [CSS-SCROLL-ANCHORING-1](https://www.w3.org/TR/css-scroll-anchoring-1/) | |
| CSS Object Model (CSSOM) | [CSSOM-1](https://www.w3.org/TR/cssom-1/) | |
| CSS Color Module Level 5 | [CSS-COLOR-5](https://www.w3.org/TR/css-color-5/) | |
| Selectors Level 4 | [SELECTORS-4](https://www.w3.org/TR/selectors-4/) | Partial support. `:is()`, `:where()`, `:not()` with lists, `:nth-child(An+B of S)`, `:focus-within` and attribute case flags |
| CSS Containment Module Level 2 | [CSS-CONTAIN-2](https://www.w3.org/TR/css-contain-2/) | |
| CSSOM View Module | [CSSOM-VIEW-1](https://www.w3.org/TR/cssom-view-1/) | |
