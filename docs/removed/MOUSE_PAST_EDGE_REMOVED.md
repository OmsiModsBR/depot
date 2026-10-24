# Mouse steering past the window's edge: removed

Status: **removed** (decision by the maintainer, 2026-10-23).

## What was removed

`App::mouse_past_edge` in `crates/omsi-app/src/input/mouse.rs`, the field `App::mouse_edge` it
kept, its two callers (the raw mouse motion in `app_events.rs` and the input script's `rawmouse`
command in `input_script.rs`) and the lines of the mouse steering that added `mouse_edge` to the
cursor's steering.

With the cursor pinned at the window's left or right edge, the mouse moving on outwards turned the
wheel further, the whole width per full lock, up to full lock at any speed. Moving back gave that
back first, holding the cursor at the edge until it was used up.

## What stays

The mouse steering as OMSI 2 has it: the cursor's place across the window is the steering from
full left to full right lock, divided by the speed in tens of km/h once the bus is faster than
10 km/h. The window's edge is the most the mouse steers, a fifth of the lock at 50 km/h.
The sensitivity (`mouse_sens`) and the smoothing (`mouse_smooth`) are unchanged.

## Why

Past the edge, the mouse steered at speed far beyond what any tyres grip: at 30-50 km/h a bus
slid and spun in a bend, an articulated bus folding to its joint's limit, where up to the edge
it took the same bend without sliding. The player also saw the wheel turn to full lock at speed,
which the mouse steering in OMSI 2 never does.

## How to revert

`mouse-past-edge-removal.patch` in this directory is the code change of the commit that removed
it:

    git apply -R docs/removed/mouse-past-edge-removal.patch
