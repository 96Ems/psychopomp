# Keyed grids: counting and regrouping

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

## Rendering and scope

The concrete `keyed-grid` recipe uses GPU-instanced opaque connected geometry,
a depth buffer, four-sample spatial antialiasing, and cached label textures.
There are no gaps or cell-scale animations. Axis extents and slice cutaways
preserve continuous boundaries, while the projection centers the currently
sampled visible cell bounds—even partway through a step. Side headings do not
skew that centering. Optional `GridCellLabelPlan` values map immutable tuples to
primary symbols and secondary text without changing their identities.
Orthographic camera parameters, cell positions, presence, emphasis,
label visibility, and extents lower into ordinary continuous tracks. Group
headers are text rather than backing cards. No wall-clock callbacks or
integration state are involved.

The lightweight `kinograph::grid` module owns the finite product catalog and
semantic snapshots. Pixel dimensions, group layout, GPU resources, and camera
projection remain in `kinograph-render`. This is one root recipe with up to 256
cells, not a mesh importer, scene graph, or general table widget. Blender and
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
