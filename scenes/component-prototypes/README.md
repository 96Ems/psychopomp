# Component prototypes — native visual trials

**Question:** can three small visual pieces look restrained on their own and
remain useful when composed, instead of making another lesson-specific renderer?

```bash
bash scenes/component-prototypes/run.sh
```

Four native scenes, switched with **1–4** or **Command+Left/Right**. Ordinary
**Left/Right** changes steps; **R** replays and **M** toggles reduced motion.
This is a native adaptation of the prototype workflow, not a browser route or
another playback implementation. The existing lesson deck and grid are untouched.

1. **Typeset** — large proportional text with measured, stable inline parts.
   `Make room.` → `Make more room.` → `Make even more room.` and back.
2. **Collection** — plain typographic items, a quiet focus underline, insertion,
   row/column arrangement and removal. Item identities survive every destination.
3. **Connectors** — actual Bezier ink, arrowheads and a traveling signal. Endpoints
   attach to the currently sampled bounds of two independently moving collections.
4. **Composition** — those same three recipes together, with no new renderer.

No enclosing cards, fake editor windows, repeated chapter heading, or on-canvas
keyboard legend. Helvetica Neue is requested for proportional text (system font
fallback applies on other platforms); monospace remains available as CommitMono.
Glyphs are shaped before playback, not resized using a character-count heuristic.
Entering and leaving words soften with the existing fade (up to a 4-output-pixel
blur sampling offset), resolving to exactly sharp at full opacity. Retained words,
connectors, underlines, layout, and timing are unaffected; no new motion track or
extra easing window is introduced.

## Slideshow components and themes

```sh
cargo run -p psychopomp-component-prototypes -- --slideshow
cargo run --release -- plan present target/slideshow-components/deck.json
```

The second showroom leaves the original four trials intact. **1–8** selects:

1. Rich text: bold, italic, combined emphasis, inline code, links, strikethrough,
   and a fenced code block.
2. Numbered lists and quotations, with independently revealed blocks.
3. Header entrances: fade/deblur, measured-width reveal, and a masked rise.
4. Stable-segment type expressions, adapted from visual-types' `AnimatedType`.
5. A two-set Venn diagram: disjoint, overlap, nesting, equality, and square boundary.
6. The existing grid as a plain table with **full vertical/horizontal dividers**.
7. The existing Typeset/Collection/Connector composition.
8. Header variants: mirrored rise, word-by-word rise (60 ms offsets), and a
   tighter reflected stagger (25 ms offsets, 280 ms motion).

**T / Shift+T** cycles Original / Evergreen / Tokyo Night / Pure Black and saves
the selection across restarts. It applies on every slide, including while held
or paused. It never resets motion. **C** still auditions grid-line colors;
changing theme restores its default accent. Theme names appear in the window title.
See `SCENE_PLANS.md` for the preference path and explicit `--theme` file exports.
**S / Shift+S** cycles 1x / 0.5x / 0.25x / 0.1x. **D** toggles a live inspection
readout, **, / .** steps through paused scene frames, and **Shift+R** replays the
current step paused at its start. Speed and HUD settings do not change exports.

### Bounded Markdown

`prototype-rich-text` consumes `RichTextPlan { origin, width, font_size, markdown,
vertical_mask, fade_blur }`. It supports paragraphs, headings 1–6, ordered/unordered/nested
lists, quotes, rules, hard/soft line breaks, bold, italic, links, strikethrough,
inline code, and fenced code. Prose uses Helvetica Neue; code uses CommitMono.
Links are styled text, not browser navigation; fenced code is not syntax-highlighted.
HTML/images are rejected. Markdown pipe tables, math, and live document edits
are not part of this pass; tables remain the existing Grid component.

Actor channels are `x`, `y`, `opacity`, `reveal`, and `blur`. Zero-based parsed
blocks may also use `block.N.opacity` and `block.N.y`. Their resting placements
do not collapse when a block fades, preserving surrounding text. Use separate
actors for separately authored identities; block indices are stable only within
one immutable Markdown payload. `vertical_mask` is optional canvas-space
`[top, bottom, linear_feather]`, useful for a header rising through a fixed window.
Header-only holds are Presentation Steps, not delayed callbacks.
Prose stays optically sharp during fades (`fade_blur` defaults to 0). The showroom's
paragraphs, list items, quotations, and supporting notes use 160 ms zero-bounce
fades. Header treatments explicitly opt into `fade_blur: 4`; their rise, clip,
and 400 ms timing remain unchanged.

### Width text and set diagrams

`prototype-width-text` uses the existing `TypesetPlan` payload, stable part IDs,
width measurement, and anchor support. Only changing parts clip/blur; retained
prefixes, infixes, and suffixes remain sharp. It deliberately does not infer a
diff from arbitrary strings or wrap a type into a lesson-specific renderer.
`TextPart.spans` optionally carries the same `StyledSpan`/`SyntaxStyle` roles as
the editor inside one animated part. It must concatenate to `TextPart.text`.
Thus ` | "green"` can have plain punctuation and string-colored literal ink
without splitting its width trajectory; all three literals share the String role.

`prototype-venn` consumes `VennPlan { center, left, right, radius }`. Ordinary
channels control `x`, `y`, `opacity`, `hatch`, and each set's `left.x`/`left.y`/
`left.radius`/`left.roundness` (and corresponding `right.*`). Set positions are
offsets from the actor's center; roundness 1 is circular and 0 is square.
Hatching and attached label leaders use the currently sampled boundaries.
Labels stay outside both sets, including during nesting. This is explicit diagram
geometry, not a type evaluator or an arbitrary-set layout engine.

These three recipes are overlays and share native/video sampling. None introduces
a new animation clock. Typography and theme caches do not change measurements.

### Reflections and real word staggering

`prototype-header` consumes `HeaderPlan`: fixed origin/width, plain text, font size,
`HeaderSplit::Line` or `Words`, entry `stagger_millis`, `duration_seconds`, initial
visibility, and timed `HeaderEvent` visibility destinations. An optional
`HeaderReflection { opacity, depth, gap }` mirrors only visible ink below the
reveal edge, with a stationary linear fade away from it. The full line is shaped
once, so revealing each word does not alter kerning or horizontal layout.
Word partitioning currently requires left-to-right placement; line mode can shape
other text normally. Headers do not parse Markdown; use rich text for prose/styles.

These are real per-word spring start offsets, not another easing applied to a
shared progress value. A reversal cancels unstarted entrances, moving words
redirect immediately, and a skipped step with the same visible header keeps its
original due times. Pause also freezes waiting; reduced motion removes waiting.
Exit starts immediately rather than staggering, and Replay explicitly restarts the entry.
This opt-in scheduling does not silently stagger unrelated existing recipes.
The offsets are overlapping starts, not sequential word completion. To inspect
them, select the entrance step, use Shift+R, and frame-step with the HUD visible.

## Provisional, not a public API commitment

- Payloads live in `psychopomp::component_prototype`; recipe names start with
  `prototype-`. Keep, revise, or remove them after visual review.
- Typeset owns one line of authored parts. Partition at intentional word/slot
  boundaries; this does not claim cross-part shaping of ligatures or scripts.
- Collection owns a finite catalog and row/column snapshots. Layout uses measured
  glyph advances. Incoming ink clips to its sampled presence width while retained
  neighbours make room. It does not own arbitrary nested actors, automatic wrapping,
  collision avoidance, or a general graph layout algorithm.
- Connectors reference an item of a Typeset or Collection. They are deliberately
  not wired into the existing editor Semantic Target system yet. Their paths are
  derived from sampled bounds, not independently delayed endpoint springs.
- Existing scalar tracks own all motion. There is no new clock, callback queue,
  direction-dependent reset, or generic scene graph.
- These are composable overlays. **Code and Grid remain exclusive roots**; this
  prototype does not claim to have solved their separate surface-composition seam.

The renderer preflights identities and links, generates reserved `__component.*`
channels, and uses the normal native/video paths. Curve segments union analytic
coverage before blending so joints do not accumulate opacity. Generated plans
are in `target/component-prototypes/`; pixel evidence goes in
`output/component-prototypes/`.

**Verdict:** awaiting visual review. Do not promote these payloads merely because
the technical checks pass. The previous seven-slide deck passed technical checks
but did not meet the desired aesthetic standard.
