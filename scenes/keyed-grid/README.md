# Keyed grids: counting and regrouping

The selected growth-edge disclosure (former experiment C) is now the default.

```sh
cargo run -p kinograph-keyed-grid
cargo run --release -- plan present target/keyed-grid/deck.json
```

Two native slides share the ordinary Scene Plan and video path:

1. **Growing a product:** one cell → three-value row → six-cell table → rotate
   the same geometry → extend depth → 24-cell connected lattice → look straight-on
   without removing depth → isolate board II → restore the full grid.
2. **An isomorphism:** 24 cells in 3D → four A × B tables grouped by C → three
   B × C tables grouped by A → the inverse regrouping → 3D again.

Left/Right change steps; **Command+Left/Right switch slides**, wrapping at the
ends (`'` / Shift+`'` remain aliases); 1/2 select slides. Home/End jump,
Space pauses, R replays, M toggles reduced motion, and Escape closes. Previous
retargets the current pose and velocity, rather than playing a video backward.
The viewing angle and slice focus are authored steps, not mouse orbit/picking.

**C** cycles grid line colors: **Orange → Muted copper → Slate blue → Sage →
Chalk**; **Shift+C** goes backward. The window title names the current choice.
This native preview preference stays selected across steps/slides, including
while paused, without retargeting motion. It resets when the player closes;
exports retain orange, the selected default after comparison. Text and cell
fills are unchanged by the color control.

## Plain tables and configurable paint

```sh
cargo run -p kinograph-keyed-grid -- --styles
cargo run --release -- plan present target/grid-styles/deck.json
```

Use **1–4** to compare a plain table, a row-banded table, an unfilled volume,
and the original volume. The two table examples can grow columns and rows and
rotate into an angled view. They use the same `keyed-grid` recipe and motion path.
Normal tables default to full dividers; the banded trial retains row-only rules
as an optional quieter treatment. **T / Shift+T** cycles saved presentation-wide
themes, independently of this geometry (see `SCENE_PLANS.md`).

```rust
let mut style = GridStylePlan::plain_table(vec![400., 360., 260.]);
let table = style.table.as_mut().unwrap();
table.headers = vec!["Stage".into(), "Status".into(), "Duration".into()];
table.alignments = vec![GridAlignment::Left, GridAlignment::Left, GridAlignment::Right];
recipe.style = Some(style);
```

- **Fill:** `None`, `Uniform { color }`, `Banded { color }`, or `Checkerboard`.
  Explicit colors are sRGB bytes. `None` means opaque background-matching faces,
  so rear text and lines remain hidden in 3D. Banding alternates the supplied
  color with the background by row.
- **Rules:** `Rows`, `Grid`, or `None`, with output-pixel `line_width` and
  `line_opacity`. Row rules draw only on the front plane, not the rear slab rim.
  Native palette cycling remains available; the default table uses quiet orange
  rules. Original exports retain orange; explicit named-theme exports use that
  theme's accent.
- **Layout:** unequal `column_widths`, `row_height`, horizontal `padding`,
  `font_size`, and per-column alignment. Optional `headers` are display text,
  not catalog identity, and are not restricted to eight-character names.
- Table typography uses proportional text at the authored size, clipped to the
  available cell width. It does not auto-shrink long values or wrap/ellipsize them.
- Table headers and retained rows keep their positions during growth. The table
  fits the complete catalog instead of recentering every visible prefix. Headers
  rotate with its plane; cube axis labels retain their upright billboard treatment.

Omitting `style` preserves the original serialized plan and cube presentation.
`GridStylePlan::default()` also selects the original checkerboard/full-grid style.
Table layout currently supports one depth layer in `Table` or `Layers`; the
non-table cube layout retains reassociation. The record-shaped demo is a fixed
row/column catalog: arbitrary row sorting, insertion in the middle, and changing
cell payloads are **not** new capabilities of this pass.

## What remains the same

Every cell retains its `(a, b, c)` tuple identity throughout. The
immutable catalog has 3 pieces × 2 sides × 4 boards, even when some are hidden.
The initial row contains White pieces on board I; the table includes both sides
on board I. Growing the diagram
reveals more tuples, not an assertion that A and A × B are isomorphic.

Reassociation preserves all 24 cells: `((a, b), c) ↔ (a, (b, c))`. The grouping
headings show which coordinate is fixed. Equal cardinality alone is not presented
as proof of an arbitrary mapping; this scene demonstrates that specific
reversible change of grouping. Finite products only—no infinite enumeration or
generic isomorphism solver.

The appearance follows the Scala talk's adjoining orange-bordered grid rather
than separate colored blocks. The grid is always 3D, even straight-on. Rotation
reveals depth without rebuilding it. Volume labels show the front or focused
slice to avoid a tangle of overlaid text; regrouping exposes all tuple labels.
Opaque checkerboard faces hide rear lines. Piece names, sides, and board labels
sit along the edges in addition to the symbols and short labels inside cells.

### Growth-edge disclosure

Labels appear when the growing grid edge reaches their catalog position: Knight
before Bishop, rows top-to-bottom, and board headings along projected −Z.
An 8-output-pixel linear feather affects ink only, including symbols at a clipped
face edge. Retained labels stay readable. Upright headings fit their aperture to
their glyph footprint under severe foreshortening. Shrink/slice navigation moves
the same aperture backward; there are no label timers or direction resets.

Geometry retains its 500 ms visual-duration springs. Only semantic visibility
changes for headings use the 220 ms disclosure spring. Cell ink stays present
on its physical layer until clipping or occlusion removes it; it does not fade
away while an outgoing opaque face still hides the next layer. Reassociation
does not invent a growth wipe for group titles. The earlier baseline/overlap
comparison deck and synchronized text-reveal experiment have been removed.
Slice focus also keeps visible grid lines at normal contrast until clipping
removes them; there is no separate dimming of the outgoing layer.

## Rendering and scope

The concrete `keyed-grid` recipe uses GPU-instanced opaque connected geometry,
a depth buffer, four-sample spatial antialiasing, and cached label textures.
Centered screen-space strokes replace inset per-face borders. A depth prepass
and max-coverage union keep shared edges from doubling and silhouettes from
halving; the cube default uses a 1.7-output-pixel width through rotation and zoom. Final-window
scaling still scales the complete authored frame.
Outside and group headings alpha-blend over the completed grid without writing
depth. Their existing opacity tracks fade ink into the real surface beneath it,
not into a dark glyph-shaped patch. Cell labels remain attached to opaque faces.
There are no gaps or cell-scale animations. Axis extents and slice cutaways
preserve continuous boundaries, while the default cube projection centers the currently
sampled visible cell bounds—even partway through a step. Side headings do not
skew that centering. Optional `GridCellLabelPlan` values map immutable tuples to
primary symbols and secondary text without changing their identities.
Orthographic camera parameters, cell positions, presence, emphasis,
label visibility, and extents lower into ordinary continuous tracks. Group
headers are text rather than backing cards. No wall-clock callbacks or
integration state are involved.

The lightweight `kinograph::grid` module owns the finite product catalog and
semantic snapshots, plus optional explicit table-presentation metrics. Concrete
placement, GPU resources, and camera projection remain in `kinograph-render`.
This is one root recipe with up to 256 cells, not a mesh importer, scene graph,
or general editable/sortable table widget. Blender and
joystick assets remain deferred. Native delivery still reads RGBA back from the
scene GPU and uploads it to the window GPU; this is not yet zero-copy rendering.

## Export / inspect

The program also emits individual plans beside the deck:

```sh
cargo run -- plan validate target/keyed-grid/growing-grid.json
cargo run --release -- plan frame target/keyed-grid/growing-grid.json 17 output/grid-chess/volume.png
cargo run --release -- plan render target/keyed-grid/growing-grid.json output/grid-chess/full-growth.mp4
cargo run --release -- plan render target/keyed-grid/grid-isomorphism.json output/grid-chess/full-isomorphism.mp4
```

Export preserves the authored 60 fps / eight-shutter-sample profile. Native
preview uses one temporal sample, with the same geometry and spatial AA.

GPU-free tests cover shared grid boundaries, identical geometry across camera
views, catalogs, group membership, equal-time coalescing, reserved
channel collisions, settled holds, and position/velocity continuity. Ignored GPU
tests cover actual silhouette centering, opaque occlusion, fractional movement,
coplanar surface priority, navigation-boundary pixels,
skipped steps, reversal, pause, sampling order, and reduced-motion destinations:

```sh
cargo test -p kinograph-render --release grid -- --ignored --test-threads=1
```
