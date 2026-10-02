# #50231 · chore: upgrade Effect to rc.117

A 96-second narrated Stage film of the Effect 4.0.0-rc.112 → rc.117 upgrade:
the compiler caught every rename, and the film covers the three behaviors it
could not catch.

| Segment | Story |
|---|---|
| `intro` | The version rolls five release candidates; the compiler wires into each renamed call and checks it off; three runtime behaviors arrive beyond its reach. |
| `settings` | A saved field the schema does not name strikes the schema gate, falls away, and leaves an empty red slot on save. `legacy(fields)` gives the read shape an open rest; the lower gate opens and both fields make the round trip. |
| `tools` | Three lanes from Effect's emitted JSON Schema to its readers: an encoded `$ref`, `Schema.Number`'s string members, and `Schema.Struct({})`'s `not null`. Each is rejected, then replayed with its named repair; the empty object is checked against four providers. |
| `permissions` | The climax. Config keys wire into last-match-wins rules; rc.117's schema order shuffles `*` to the bottom so `shell` and `edit` probes both match `* allow`. Rewind; `inInputOrder` rebuilds the written order and the probes land on `shell ask` and `edit deny`. |
| `permissions-code` | Zooms from the `inInputOrder` card into the condensed change; the call site keeps its identity while `inInputOrder(` and `, permission)` open inside it. |
| `outro` | Eight CI checks resolve to marks while the count rolls to 8/8; the title card. |

```sh
cargo run -p kinograph-pr-50231
cargo run --release -- plan validate scenes/pr-50231/pr-50231.reel.json
cargo run --release -- plan render scenes/pr-50231/pr-50231.reel.json output/pr-50231/pr-50231.mp4 --theme opencode
```

Narration is Eleven v4 (`narration/script.json`). The `permissions-after` clip
is split in its measured pause before "And the new tests" so the zoom into the
code happens without added silence; `Clip::split` checks the point against the
word timings if the clip is re-voiced.

Sound effects reuse quiet repository stems plus four Eleven Sound Effects v2
stems in `sfx/` (rewind, shuffle, drop, resolution). Regenerate them with
`ELEVENLABS_API_KEY` injected:

```sh
bun scenes/pr-50231/sfx/generate.ts
```

The version (`rc.112 → rc.117`) and the CI count are `rolling-number`
overlays; the count rolls once per check as each mark is drawn.
