# Psychopomp Motion Audit & Polish

This document tracks the frame-by-frame motion audit of Psychopomp's showrooms, recipes, and narrated films on `main` (`cc48084`), defines the unified motion token vocabulary, and links before/after contact strips and video studies under `output/polish/`.

## Audit Criteria

Every showroom and component is evaluated against the house motion rules (`explainer-motion/SKILL.md`, `explainer-motion/TECHNIQUES.md`, `PRIOR_ART.md`):

1. **Entrance / exit asymmetry**: Exits should be simpler and faster than entrances; avoid floaty linear or long-tailed exits.
2. **Overshoot discipline**: No bounce on persistent layout, opacity, blur, or utility transitions; reserve deliberate overshoot (`PANEL` 0.12, `LIVELY` 0.2) for rigid physical panels and hero arrivals.
3. **Overlapping action & container/content lag**: Rigid containers lead their ink by ~60–65 ms; secondary labels follow primary lines or bars; blur resolves faster than or alongside opacity/scale rather than lingering over readable text.
4. **Continuity & velocity**: Pure functions of time; interrupted moves preserve position and velocity; minimum-jerk (`smootherstep`) for exact point-to-point glides; `cubic-bezier(0.45, 0.0, 0.2, 1.0)` for wire/stroke draw-on.
5. **Pacing, stagger, and dead frames**: ~120 ms row/card stagger; transitions without dead holds or abrupt starts/stops; flashes with instant attack and convex decay.

---

## 1. Component & Showroom Audit

- [ ] **Stage, Camera, Forms & Effects** (`psychopomp-camera`, `psychopomp-stage-forms`, `psychopomp-effects-showroom`)
- [ ] **Reel Transitions & Compare** (`psychopomp-transitions`, `psychopomp-compare`)
- [ ] **Overlays, Callouts, Anchors & Lens** (`psychopomp-callouts`, `psychopomp-anchors`, `psychopomp-loupe`)
- [ ] **Text Surfaces & Lower Third** (`psychopomp-text-surfaces`)
- [ ] **Visualization Overlays & Subtitles** (`psychopomp-viz-components`, `psychopomp-charts`, `psychopomp-tree`, `psychopomp-rolling-number`)
- [ ] **Editor, Diagnostics, Footage & Films** (`psychopomp-diagnostics`, `psychopomp-footage`, `scenes/2password`, `scenes/psychopomp-intro`)

---

## 2. Shared Motion Token System

_Pending audit synthesis._

---

## 3. Polish Changes & Before/After Evidence

_Pending implementation._
