# Kinograph Domain Language

## Code Document

The complete set of code lines that may appear in a scene. Every line has a stable line ID and styled spans.

## Code Snapshot

An ordered list of stable line IDs describing one meaningful state of a code document. A snapshot contains state, not motion.

## Stable Line

A code line whose identity survives between snapshots. Its screen position may move when surrounding lines enter or leave, but its text object is not replaced.

## Code Transition

The compiled relationship between two code snapshots. Sampling a code transition places every stable, entering, and exiting line at an arbitrary progress value.

## Motion State

The position and velocity of one animated scalar at a specific time. Carrying both values allows a later trajectory to preserve momentum.

## Animation

A pure value describing a property change or the composition of other animations. Sequence, parallel, delay, and hold determine relative timing without rendering or mutating scene state.

## Property Track

The compiled trajectory of one scalar actor property. A later spring on the same track begins from the earlier trajectory's sampled position and velocity.

## Semantic Target

A measured visual range resolved from meaningful content, such as a token inside a stable code line. Highlights and pointers attach to semantic targets rather than authored screen coordinates.

## Pointer

A stable visual actor that directs attention to a semantic target. Its position and opacity are ordinary property tracks, so retargeting and temporal sampling use the same motion system as every other actor.

## Temporal Sample

One evaluation of the complete scene within an output frame's shutter interval. Kinograph averages temporal samples to produce motion blur from real scene movement.

## Editor Frame

The renderer-neutral description of one sampled editor scene: panel position, focus state, and placed code lines.
