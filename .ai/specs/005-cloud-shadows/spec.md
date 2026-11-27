# Feature Specification: Cloud shadows on the scenery in Enhanced

**Feature Branch**: one per delivery phase (see `plan.md`, to be written with `/speckit-plan`)

**Created**: 2026-11-27

**Status**: Clarified (2026-11-27, `/speckit-clarify` session, 4 questions)

**Input**: User description (maintainer, 2026-11-27): "Include Cumulus 3 and the other cloud types
in the scope, so that every cloud casts a shadow on the scenery, its strength varying with the
cloud's height and opacity." Added the same day: "Also include in the scope the moonlight being
blocked by the clouds." It follows the maintainer's report that in Enhanced, under a cloudy
sky, the game looked very bright and showed no shadows on the scenery, and the maintainer's choice
of a per-point cloud shadow over the other options.

## Context

What Depot does today in the Enhanced graphics mode (code at `main` 5feb348):

- **The sun of the whole scene follows one point.** Since the live clouds (#796), the sunlight the
  scene receives is scaled by how much of the sun the clouds let through **at the camera**. When a
  cumulus stands between the camera and the sun, the sun goes out over the whole scene, every sun
  shadow disappears with it, and the scenery a kilometre away that lies in sunshine is lit as if it
  were under the cloud too. With the `Cumulus 3` type, clouds cover about half of the sky, so this
  is most of the time. The maintainer's session that showed it ran a weather taken from a METAR
  with a broken layer (`BKN009`), which Depot turns into `Cumulus 3`.
- **No cloud casts a shadow on the ground.** There is no cloud shadow anywhere in the picture: the
  scene is either all sun or all shade.
- **An `Overcast` type turns sun shadows off** as a switch, and the physical weather model turns
  them off when its grey deck is nearly closed.
- **The weather's cloud height is not used** by the Enhanced clouds: the `[clouds]` height of the
  weather file is read, but the clouds are drawn and lit in a fixed layer.
- The exposure follows half of the light lost in a cloud's shade, so under a cloud the shadowless
  picture comes out flat and bright.
- **Moonlight ignores the cumulus.** On a clear night with the moon up, Enhanced lights the scene
  with moonlight and draws moon shadows. The clouds only dim the moonlight evenly, by the overall
  grey of the sky; a cumulus passing in front of the moon neither dims the moonlight nor takes the
  moon shadows away, anywhere in the scene.

This feature makes every cloud of the sky cast its own shadow on the scenery, where it stands
between that point and the sun, with a strength that follows the cloud's opacity and an edge that
follows its height. The clouds block the moonlight by the same rule at night. It is a Depot feature of the Enhanced mode. It is not an OMSI 2 behaviour and
the specification makes no claim about OMSI 2 (constitution, principle II).

### Cloud kinds in scope

| Kind | Where it comes from | Expected shadow |
| --- | --- | --- |
| `Cumulus 1`, `Cumulus 2`, `Cumulus 3` | weather file `[clouds]` type, METAR FEW/SCT/BKN | patches of shade that move with the clouds; more of the ground in shade as the number rises |
| `Overcast` | weather file `[clouds]` type, METAR OVC | the whole ground in shade, as dark as the deck is opaque |
| Cumulus heaps of the physical weather model | weather model cumulus cover | patches, as for the cumulus types |
| Grey deck of the physical weather model | weather model deck | an even shade as dark as the deck is closed; patches where it is broken |
| High veil (cirrus) of the physical weather model | weather model veil | a faint, even dimming of the sun and softer sun shadows; no patches |
| Rain or snow falling from a closed deck | weather precipitation | the deck's shade, as for `Overcast` |
| Any of the above at night with the moon up | same sources | the same shade, cast on the moonlight instead of the sunlight |

## Clarifications

### Session 2026-11-27

- Q1: How much should the picture make up for a cloud's shade over the camera? → A (maintainer): a
  quarter of the light lost, in log terms (today: half). A shaded foreground comes out about 1.7
  stops darker than in sun on a clear day, instead of about 1.2 (FR-010, SC-006).
- Q2: Should the clouds use the real height even when it is very low (a METAR `BKN009` puts the
  base near 270 m)? → A (maintainer): yes, the real height with no floor: a low base may cut into
  tall buildings and hills of the map, and that is shown as it is (FR-006, edge cases).
- Q3: Does fixing the over-grey sky under cumulus belong in this spec? → A (maintainer): yes: only
  `Overcast` (and a closed deck, rain or snow) greys the sky and takes the sun away evenly; the
  `Cumulus` types stop doing it, and each cloud's own shade does the darkening (FR-018).
- Q4: Should cloud shadows have their own setting? → A (maintainer): no: they follow the existing
  clouds setting; a setting of their own comes only if the measurements of SC-005 call for it
  (FR-012).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Sun and shade patches under a cumulus sky (Priority: P1)

A player drives on a day with a `Cumulus 3` sky in Enhanced. Parts of the street, the buildings and
the far hills lie in sunshine with sharp sun shadows of trees, poles and shelters; other parts lie in
the shade of a cloud, without sun shadows. The patches drift slowly with the wind, the bus passes
from sun into shade and back, and when the camera is under a cloud the sunlit places further away
stay sunlit.

**Why this priority**: it is the reported problem: a cloudy sky must not take every shadow away
from the whole scene.

**Independent Test**: in the maintainer's scenario (same map, time and `Cumulus 3` weather), take a
wide view with the camera under a cloud and one with the camera in a gap: in both, the picture shows
both sunlit areas with sun shadows and cloud-shaded areas, and the sunlit areas keep their sun
shadows whatever the cloud over the camera does.

**Acceptance Scenarios**:

1. **Given** a `Cumulus 3` sky with the sun above the horizon, **When** the camera stands under a
   cloud, **Then** the places beyond the cloud's shadow on the ground are still lit by the sun and
   show sun shadows.
2. **Given** the same sky, **When** the camera stands in a gap between clouds, **Then** the places
   under a cloud in view are shaded and show no sun shadows.
3. **Given** a steady wind, **When** the player waits, **Then** the shade patches move across the
   ground in the wind's direction at the clouds' speed, without jumps.
4. **Given** the clouds seen in the sky, **When** the player looks from a patch of shade towards the
   sun, **Then** a cloud stands in front of the sun; from a patch of sun, the sun is clear.

---

### User Story 2 - Every cloud kind shades as dark as it is opaque (Priority: P1)

The same scene is driven with each cloud kind of the table above. Thin, small clouds (`Cumulus 1`)
leave most of the ground in sun with a few light patches; `Cumulus 3` leaves about half of it in
deeper shade; an `Overcast` sky and a closed grey deck shade all of it evenly; a high veil only
dims the sun slightly and softens the sun shadows. A cloud's shade is never darker than the light
that the sky alone gives.

**Why this priority**: the maintainer asked for every cloud kind, with strength following opacity.

**Independent Test**: for each cloud kind, measure in the same view the share of the ground in shade
and the light in shade against the light in sun; both follow the order of the table and stay within
the bounds of SC-002 and SC-003.

**Acceptance Scenarios**:

1. **Given** the types `Cumulus 1`, `Cumulus 2` and `Cumulus 3` in turn, **When** the same view is
   taken at the same time of day, **Then** the share of ground in shade grows from one to the next.
2. **Given** an `Overcast` sky, **When** any view is taken, **Then** the whole ground is in shade,
   no sun shadows show, and the scene is darker than under a clear sky at the same time of day.
3. **Given** a weather-model deck that is not fully closed, **When** the view is taken, **Then**
   the ground lies in an even shade whose darkness follows how closed the deck is, and faint sun
   shadows remain while some sun gets through.
4. **Given** only a high veil, **When** the view is taken, **Then** there are no shade patches; the
   sun shadows stay, a little lighter and softer than under a clear sky.
5. **Given** a thicker part of a cumulus and a thinner edge of it, **When** their shadows on the
   ground are compared, **Then** the thicker part shades more than the edge.

---

### User Story 3 - The cloud height shapes the shadow (Priority: P2)

A weather file sets its clouds low (a few hundred metres) or high (a few kilometres). Low clouds
cast shade patches with sharper edges, close below them; high clouds cast patches with wider, softer
edges, shifted further away from the cloud's footprint as the sun gets lower. The clouds drawn in
the sky stand at the same height as the ones that cast the shadows.

**Why this priority**: the maintainer asked for the height to change the shadow; it refines User
Stories 1 and 2 and needs them first.

**Independent Test**: run the same `Cumulus 2` weather with its `[clouds]` height set low and then
high; the shadow edge on flat ground is measurably wider for the high clouds, and the clouds in the
sky appear lower or higher to match.

**Acceptance Scenarios**:

1. **Given** the same cloud type at two heights, **When** the shadow edge on flat ground is
   measured, **Then** the edge of the higher clouds' shadow is wider.
2. **Given** a low sun, **When** a cloud's shadow is located, **Then** it lies on the ground along
   the sun's direction from the cloud, further away for a higher cloud.
3. **Given** a weather file without a usable cloud height, **When** it is loaded, **Then** the
   clouds use a default height and the game reports nothing to the player.

---

### User Story 4 - The picture under a cloud is no longer flat and bright (Priority: P2)

With the camera in a cloud's shade, the player sees a scene darker than the sunlit parts and the
sunny parts in the distance brighter than the shaded foreground, instead of one even, bright picture.

**Why this priority**: the second half of the report ("very bright"); it follows from User Story 1
plus the exposure rule.

**Independent Test**: in the scenario of User Story 1, compare the mean brightness of the shaded
foreground with a sunlit area in the same frame and with the same foreground in sun.

**Acceptance Scenarios**:

1. **Given** the camera under a cloud, **When** the frame is measured, **Then** a sunlit area in
   view is brighter than the shaded foreground.
2. **Given** the camera moving from sun into a cloud's shade, **When** the exposure settles,
   **Then** the shaded foreground stays darker than it looked in the sun (the eye does not fully
   make up for the cloud): the exposure makes up for a quarter of the light lost (FR-010).

### User Story 5 - Clouds block the moonlight (Priority: P2)

On a night with the moon up and a cumulus sky, parts of a dark country road lie in moonlight with
faint moon shadows of trees and poles, and parts lie in the shade of a cloud, darker and without
moon shadows. When a cloud passes in front of the moon seen from the camera, the moonlit places
further away stay moonlit. Under an `Overcast` sky or a closed deck, no moonlight reaches the ground
and there are no moon shadows.

**Why this priority**: the maintainer asked for it; it reuses the cloud shade of User Stories 1 to 3
with the moon in place of the sun, and only shows on moonlit nights away from street lamps.

**Independent Test**: on a moonlit night (moon well above the horizon, more than half lit) with a
`Cumulus 3` sky, on a road without street lamps, take views with a cloud in front of the moon and
with the moon clear; the moonlit and cloud-shaded patches and the moon shadows behave as in User
Story 1, and an `Overcast` sky leaves no moonlight on the ground.

**Acceptance Scenarios**:

1. **Given** a moonlit night with a `Cumulus 3` sky, **When** a view is taken, **Then** the ground
   shows moonlit patches with moon shadows and cloud-shaded patches without them.
2. **Given** a cloud in front of the moon as seen from the camera, **When** the view is taken,
   **Then** the places beyond that cloud's shade on the ground stay moonlit.
3. **Given** an `Overcast` sky or a closed deck at night, **When** any view is taken, **Then** no
   direct moonlight reaches the ground and no moon shadows show.
4. **Given** the clouds seen in the night sky, **When** the player looks from a cloud-shaded patch
   towards the moon, **Then** a cloud stands in front of the moon.

### Edge Cases

- Sun at or below the horizon: no sun cloud shadows (there is no sunlight to block); with the moon
  up, the clouds block the moonlight instead (User Story 5).
- Sun and moon both up (a day moon, or dusk): the sun's cloud shade applies; the moon's light is too
  weak beside the sun to matter and needs no shade of its own.
- New moon, or a moon too low or too dim to light the scene: no moonlight and no moon cloud shade.
- Camera above the cloud base (a mountain map, a free camera flown high): clouds below the camera
  still shade the ground under them; the camera inside a cloud sees the ground in its shade.
- A cloud base lower than the buildings or hills of the map (FR-006 sets no floor): the parts of
  the scenery inside a cloud are shaded by how deep in it they stand, the parts above it get the
  sun, and the clouds are drawn where they are, cutting into the scenery.
- Clouds switched off in the settings: no cloud shadows and no change from the clear-sky picture.
- Sun shadows switched off in the settings: cloud shade still dims the sunlight where clouds stand
  (it is light, not a shadow map), but no object shadows are drawn.
- A weather change blending one cloud kind into another: the shade blends with it, without jumps.
- A very large map or a long drive: the shade patches cover everything in view up to the visible
  distance; there is no visible border where cloud shadows end near the horizon.
- Inside the bus and in mirrors: the sun through the windows follows the shade over the bus;
  mirrors and reflections show the same shade as the main view.
- Vanilla+ and Vanilla (DX9) modes: unchanged; the feature is Enhanced only.
- A graphics card that cannot take the extra resources of the feature: Enhanced still runs, with
  today's behaviour, and the log says why.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: In Enhanced, the sunlight that reaches each visible point MUST be reduced by the
  clouds that stand between that point and the sun, independently for every point, instead of by
  one value taken at the camera.
- **FR-002**: Every cloud kind in the "Cloud kinds in scope" table MUST cast its shade as described
  there, including the `Cumulus 1..3` and `Overcast` weather types, the METAR-derived types and the
  cumulus, deck and veil of the physical weather model.
- **FR-003**: The darkness of a cloud's shade MUST follow the cloud's opacity along the sun's path
  through it: no shade for a clear sky, partial shade through thin cloud or a cloud's edge, full
  loss of direct sun through a thick cloud or a closed deck.
- **FR-004**: A shaded point MUST keep the light of the sky (and of the deck, under an overcast
  sky); a cloud only takes away direct sunlight.
- **FR-005**: The shade patches on the ground MUST match the clouds drawn in the sky: same
  positions, same drift with the wind, same cover for each cloud kind. Clouds MUST be drawn
  wherever they cast shade, also when they stand below or around the camera (a camera above a low
  base, or inside the layer).
- **FR-006**: The clouds MUST stand at the height the weather gives (`[clouds]` height, or the
  METAR layer base), both where they are drawn in the sky and where they cast shade, with no lower
  limit: a base of a few hundred metres or less is used as given; a missing or unusable height
  (absent, zero, negative or not a number) falls back to a default without a message to the
  player.
- **FR-007**: The width of a cloud shadow's soft edge MUST grow with the cloud's height above the
  ground point.
- **FR-008**: Sun shadows of objects (buildings, trees, vehicles, people) MUST show only where
  direct sun reaches, with their strength scaled by the sun that gets through the clouds there.
- **FR-009**: The `Overcast` type and a closed deck MUST stop switching sun shadows off as a special
  case; they shade the scene through FR-001 to FR-004 like every other cloud, which removes direct
  sun everywhere they are opaque. Direct sun (and moon) light MUST be removed only by the per-point
  shade, never also by an even dimming, so that no cloud cuts it twice.
- **FR-010**: The exposure MUST keep adapting to the light where the camera is, but MUST make up
  for only a quarter of the light a cloud's shade over the camera takes away (in log terms; today
  half), while points elsewhere in view receive their own sunlight. The same share applies to the
  moonlight at night.
- **FR-011**: The feature MUST apply to everything lit by the sun in Enhanced: terrain, scenery
  objects, splines, vehicles (outside and through their windows), people, and the views used for
  mirrors and reflections.
- **FR-012**: The feature MUST follow the existing clouds setting (clouds off: no cloud shade) and
  MUST NOT add a new setting unless the performance measurements of SC-005 require one.
- **FR-013**: Vanilla+ and Vanilla (DX9) MUST NOT change.
- **FR-014**: When the graphics card cannot provide what the feature needs, Enhanced MUST still run
  with today's behaviour and write the reason to the log once.
- **FR-015**: The feature MUST NOT alter how a vehicle or object model looks beyond the light it
  receives: no rule based on a model's geometry, materials or position decides whether it is
  shaded (the cloud between the point and the sun, or the moon, is the only rule).
- **FR-016**: At night, the direct moonlight that reaches each visible point MUST be reduced by the
  clouds between that point and the moon by the same rules as the sunlight (FR-001 to FR-008): per
  point, strength from opacity, soft edge from height, sky light kept, patches matching the clouds
  in the sky, moon shadows of objects only where moonlight reaches.
- **FR-017**: The even dimming of the moonlight by the sky's overall cloud cover MUST be replaced by
  FR-016, so that the moonlight is not dimmed twice by the same clouds.
- **FR-018**: The `Cumulus` types (weather file or METAR) and the cumulus heaps of the physical
  weather model MUST NOT grey the whole sky or dim the sunlight evenly over the scene: the sky
  between the clouds keeps its clear colour and the sun its clear strength, and the clouds darken
  the scene only through their own shade (FR-001 to FR-004) and their own picture in the sky. Only
  `Overcast`, a closed deck and a sky that rain or snow falls from grey the sky and its light (the
  sky's colour and the light it gives; direct sun is left to FR-009), and only they raise the
  picture's gloom (the share of night look - lit windows, night maps - a dark weather brings by
  day); the `Cumulus` types raise none. Vanilla+ and DX9 keep their current cumulus lighting
  (FR-013).

### Key Entities

- **Cloud layer**: the clouds of the current weather: their kind, cover, height, thickness and
  opacity, and their drift with the wind. It is the same for the sky picture and for the shade.
- **Cloud shade**: for a ground or object point, the share of direct sunlight (or, at night, direct
  moonlight) that the cloud layer lets through towards the sun (or the moon); 1 under a clear sky, 0
  under a thick cloud.
- **Weather cloud kinds**: `Cumulus 1..3`, `Overcast` (weather file or METAR) and the cumulus, deck
  and veil of the physical weather model.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In the maintainer's scenario with a `Cumulus 3` sky, in 10 views taken over 5 minutes
  of play, every view with sunlit ground in it shows sun shadows of objects there, including the
  views with the camera under a cloud (today: none of those).
- **SC-002**: Over a wide view of flat open ground, the share of ground in shade grows with the
  cumulus number: `Cumulus 1` less than `Cumulus 2` less than `Cumulus 3`, and an `Overcast` sky or a
  closed deck shades all of it.
- **SC-003**: A point in full cloud shade receives the same light as the same point would with the
  sun hidden but the same sky; it is never darker than that.
- **SC-004**: For the same cloud type, the soft edge of a cloud shadow on flat ground is at least
  twice as wide with the clouds at 3000 m as with the clouds at 1000 m.
- **SC-005**: On the maintainer's machine and scenario, the average frame time in Enhanced grows by
  no more than 2 % with the feature on, measured before and after (constitution, principle VII);
  loading time does not grow measurably.
- **SC-006**: With the camera under a cloud on an otherwise clear day, after the exposure settles,
  the shaded foreground is between 1.5 and 1.9 stops darker than the same foreground in sun, and a
  sunlit area in the same frame is at least 1.5 stops brighter than the shaded foreground.
- **SC-007**: Vanilla+ and Vanilla (DX9) pictures of a fixed scene are unchanged pixel for pixel
  (or within the existing test tolerance) before and after.
- **SC-008**: No visible jump in the shade patches over 5 minutes of play with a steady wind, and
  none while one weather blends into another.
- **SC-009**: On a moonlit night with a `Cumulus 3` sky away from street lamps, every view with
  moonlit ground shows moon shadows there, including views with a cloud in front of the moon from
  the camera; under `Overcast`, no view shows moon shadows.
- **SC-010**: Under a `Cumulus 3` sky, a sunlit point receives the same direct sunlight as under a
  clear sky at the same time of day, and the sky between the clouds shows the clear sky's colour.

## Assumptions

- Enhanced only, as the graphics modes direction says (Enhanced is the mode for advanced
  improvements; Vanilla+ stays as is; Vanilla is DX9).
- This feature is Depot's own (principle I); it makes no claim about OMSI 2, and no text in the
  repository will describe it as OMSI 2 behaviour.
- "Opacity" is the cloud's optical thickness along the sun's path: thicker clouds, the denser parts
  of a heap and a closed deck block more; thin edges and a high veil block less.
- "Height" is the cloud base the weather gives, measured from the map's height zero (the vertical
  origin of the map's coordinates); a METAR base (given above the station) is taken as above that
  zero. On a map whose terrain lies far above or below zero the clouds are as much lower or
  higher above the ground; this is accepted (the weather does not know the map's terrain). The
  layer's thickness keeps today's proportions.
- The cover of each `Cumulus` number and of `Overcast` stays as Depot sets it today; this feature
  makes that cover cast shade and, for the `Cumulus` types only, removes the even greying of the
  sky and dimming of the sun (FR-018).
- Performance is the main risk; the external practice the maintainer asked to check (a cloud shade
  computed once, seen from the sun, and read for every point) keeps the per-point cost to one read.
  The plan carries the measurements.
- Moonlight cloud shade follows the existing moonlight and moon shadow conditions of Enhanced (moon
  up, sun down, moon bright enough); this feature does not change when moonlight or moon shadows
  appear, only where the clouds block them.
- Out of scope: cloud shadows in Vanilla+ or DX9 (including Vanilla+'s moonlight), a new
  setting to turn cloud shadows off on their own (unless SC-005 requires one), and rain shafts.
