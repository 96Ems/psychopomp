# Functional Data Modeling · native first cut

Seven slides adapting the opening types/cardinality material and selected examples
from the original Scala talk. This is an authored mini-deck, not a complete port.
The existing `scenes/keyed-grid` chess demonstration is unchanged.

| Slide | Steps | What changes |
| --- | ---: | --- |
| Types and cardinality | 5 | Enumerate `true`, then `false`; introduce `\|Boolean\| = 2` only once the whole set is visible. |
| Small, huge, unbounded | 3 | Retain Boolean while introducing signed 32-bit `Int` and the abstract unbounded-length String model. |
| Boolean ↔ Toggle | 5 | Reveal two explicit pairings; trace `true → On → true` without moving the values. |
| Does the type fit? | 5 | Keep the five domain states fixed; compare Boolean, String, and a five-case Joystick in separate columns. |
| OR adds | 4 | Two Toggle alternatives plus five Joystick alternatives, with explicit constructor tags. |
| AND multiplies | 4 | Grow the existing connected Keyed Grid from one row to two: ten distinct pairs. |
| Illegal states | 4 | A nullable pair has four presence combinations. Remove the two unwanted shapes by changing to `UserOrError`; retain the valid payload actors. |

## Run

```bash
cargo run -p psychopomp-data-modeling
cargo run --release -- plan present target/data-modeling/deck.json
cargo run -- plan steps target/data-modeling/illegal-states.json
cargo run --release -- plan render target/data-modeling/boolean-toggle.json \
  output/data-modeling/round-trip.mp4 --range 9..13.5
```

Arrows change steps. **Command+Left/Right** changes slides. **1–7** selects a
slide directly. **R** replays an entry, **M** toggles reduced motion, **X** toggles
display filtering, and **Space** pauses/resumes. The native player is silent;
individual Scene Plans also export through the ordinary shutter-sampled renderer.

## Components and stability

- `value-token` is one small overlay recipe: an immutable label/detail on a
  rounded tile, with ordinary x/y/opacity/emphasis channels. It reuses the existing
  card compositor and fractional cached glyph rendering. Emphasis changes the
  border, never the text's presence. There is no generic graph API, second
  animation clock, or new state channel implementation.
- `stage.rs` contains scene-local layout and step helpers. Only changed
  destinations emit spring events. Captions roll through stationary edge fades.
- Domain values stay still while candidate representations enter separate
  columns. Boolean and String remain available for comparison; their labels do
  not crossfade over Joystick's labels.
- Mapping arrows are fixed annotations between fixed values. The traveling dot
  illustrates the authored correspondence, not a function runtime.
- OR and AND are separate slides. Seven alternatives do **not** morph into ten
  tuples as if the two sets were isomorphic.
- AND uses the grid's optional ordinary `scale` channel to make its table readable
  without changing the old chess deck's default fit. The one-element depth axis
  has no outside heading; it is not an extra column choice.
- The illegal-state signature retains its prefix, suffix, and line position.
  Only `(User, Error)` / `UserOrError` exchange. The new constructor lines enter
  below it. `plan steps` reports one common-text warning for `Error` within those
  two type spellings. This is intentional: the tuple's `Error` type and the
  substring of the single `UserOrError` identifier are not the same semantic
  role. There are no unsettled-hold warnings.

## Source fidelity and limits

Sources are `Slide_1_Types.scala` and `Slide_4_Modeling.scala` under
`/Users/kit/code/lessons/scala-course/frontend/src/main/scala/slides/content/modeling/`.
The examples and counts follow that source; layout and motion are native
adaptations, not frame-matched recreations of the old Laminar implementation.

- `Int` has 2³² values. `String` is called unbounded only in the abstract model of
  arbitrary finite strings; real runtime length limits are explicitly noted.
- Boolean and Toggle demonstrate a particular invertible finite mapping, not
  the stronger claim that arbitrary types with matching counts are semantically
  interchangeable.
- The joystick's compact declaration uses Scala 3 enum syntax. The sum-type
  equation is diagram notation; the `UserOrError` editor uses the original
  sealed-trait/case-class form.
- Four versus two in the error example counts **presence/case shapes**, not the
  cardinality of all possible User and Error payloads. Nullability is the old
  example's convention, not a recommendation for modern Scala code.
- Full Alphabet/Alterbet message decoding, live joystick input, infinite
  enumeration, and the IceCreamOrder grid remain deferred.

Generated plans live in `target/data-modeling/`; validation and pixel artifacts
live in `output/data-modeling/`.
