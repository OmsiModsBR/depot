# Environment variables

<!-- Made from `crates/omsi-cfg/src/flags.rs` by `cargo test -p omsi-cfg --lib write_environment_md -- --ignored`; do not edit by hand. -->

The `OMSI_*` variables the game reads, as declared in `omsi_cfg::flags`. "env.txt" marks the ones a phone's `depot/env.txt` may set.

## Settings

| Variable | Value | Read by | env.txt | What it does |
| --- | --- | --- | --- | --- |
| `OMSI_BACKEND` | text | `omsi-app::startup` | yes | The graphics interface to ask first: `vulkan`, `dx12` or `gl` (set by the game itself when it starts again after a lost device, and on phones without a good Vulkan). |
| `OMSI_BASIC_PIPELINES` | switch | `omsi-render::gpu::open` | yes | Only the simple pipelines, for drivers whose shader compiler fails on the others. |
| `OMSI_ENHANCED` | switch | `omsi-app` |  | The enhanced renderer, whatever the setting says. |
| `OMSI_ENHANCED_PLUS` | switch | `omsi-app` |  | The enhanced renderer with its extras (`--enhanced-plus`). |
| `OMSI_FULL_GPU` | switch | `omsi-render::api::renderer` | yes | The settings as they are on a weak graphics chip too (no lighter defaults). |
| `OMSI_GRAPHICS` | text | `omsi-app::settings` | yes | The renderer for one run: `vanilla`, `vanilla_plus` or `enhanced`. |
| `OMSI_IBIS_BUDGET` | number | `omsi-sim::ibis` |  | How long (seconds, 10 by default) the IBIS may look for a way to type a duty in. |
| `OMSI_INTEL_FULL_GPU` | switch | `omsi-render::api::renderer` |  | On an Intel Vulkan adapter on Windows, the settings asked for instead of the stable driver profile. |
| `OMSI_LAN_IP` | text | `omsi-net::addrs` |  | The LAN address(es) to offer, separated by commas, spaces or semicolons. |
| `OMSI_LAN_JOIN_TIMEOUT` | number | `omsi-net` |  | How long (seconds) a joining game waits for the host. |
| `OMSI_LAUNCHER_SIZE` | text | `omsi-app::launcher` | yes | The launcher's window at this size (`WxH`), to look at the layout. |
| `OMSI_MAX_FPS` | number | `omsi-app::app_events` | yes | The frame rate limit, in place of the setting. |
| `OMSI_MOBILE` | switch | `omsi-app::ui` | yes | The phone's layout on a computer (the rail kept clear of a camera hole). |
| `OMSI_MUTE` | switch | `omsi-audio::mixer` | yes | Silence: the mixer plays nothing. |
| `OMSI_NO_AO` | switch | `omsi-render::renderer::frame` | yes | No ambient occlusion. |
| `OMSI_NO_BC` | switch | `omsi-render::api::renderer` | yes | Textures uploaded as RGBA, not compressed on the GPU. |
| `OMSI_NO_BUMP` | switch | `omsi-app::scene` | yes | No bump maps on vehicles and objects. |
| `OMSI_NO_CLOUDS` | switch | `omsi-app` | yes | No clouds. |
| `OMSI_NO_CORONAS` | switch | `omsi-render::passes::main` | yes | No coronas round the lamps. |
| `OMSI_NO_ENHANCED` | switch | `omsi-render::renderer::frame` | yes | The enhanced renderer's HDR picture left out. |
| `OMSI_NO_ENVMAP` | switch | `omsi-app::scene` | yes | No environment maps (reflections) on vehicles and objects. |
| `OMSI_NO_FXAA` | switch | `omsi-render::renderer::frame` | yes | No FXAA. |
| `OMSI_NO_PLUGINS` | switch | `omsi-app::plugins` | yes | No plugins loaded. |
| `OMSI_NO_RT` | switch | `omsi-render::api::renderer` | yes | No ray queries. |
| `OMSI_NO_RT_GRADE` | switch | `omsi-render::passes::post` | yes | The ray-traced frame without its colour grade. |
| `OMSI_NO_SHADOWS` | switch | `omsi-render::lighting::model` | yes | No shadows. |
| `OMSI_NO_SURF` | switch | `omsi-app::scene` | yes | Roads without the bumps of their textures' `.surf` maps. |
| `OMSI_NO_TEXCOMPRESS` | switch | `omsi-render::api::renderer` | yes | No compression of loose pictures. |
| `OMSI_OPENXR` | switch | `omsi-app::settings` |  | VR through OpenXR, whatever the setting says (Windows; the old switch, kept for existing VR runs). |
| `OMSI_OPENXR_MIRROR_RATE` | number | `omsi-app::openxr` |  | How many mirror pictures a second are drawn in VR (-1: every mirror each frame). |
| `OMSI_OPENXR_SCALE` | number | `omsi-app::openxr` |  | The size of the VR picture against the headset's recommendation. |
| `OMSI_RADIO_ALLOW_HTTP` | text | `omsi-audio::url_guard` |  | `1`, `true` or `yes`: radio stations on plain `http` are allowed. |
| `OMSI_SAFE_GPU` | number | `omsi-app::settings` | yes | How many times in a row the game started again after a lost graphics device (set by the game itself; any number starts it with the safe graphics settings). |
| `OMSI_SEED` | number | `omsi-app` | yes | Repeat a session: the scripts' `random` is seeded with this number. |
| `OMSI_TEXTURE_MEMORY` | number | `omsi-app::memory` | yes | The texture budget in MB, in place of the setting (an eighth of the machine's memory when neither says). |
| `OMSI_TOUCH` | switch | `omsi-app::platform` | yes | The on-screen controls of a phone on a computer too. |
| `OMSI_TRACKIR_NATIVE` | text | `omsi-app::headtrack` |  | `0`: TrackIR read through its own interface left off. |

## Paths and programs

| Variable | Value | Read by | env.txt | What it does |
| --- | --- | --- | --- | --- |
| `OMSI_CLOUDFLARED` | path | `omsi-net::tunnel` |  | The `cloudflared` the LAN tunnel runs, in place of the one found or downloaded. |
| `OMSI_CONTENT` | path | `omsi-launcher-core` |  | The folder the content is read from, in place of the one beside the game. |
| `OMSI_CONTENT_ZIP` | path | `omsi-cfg::vfs` |  | Archives mounted as content roots, separated like `PATH`. |
| `OMSI_LAUNCHER` | path | `omsi-app::launcher_link` |  | Another program opened as the launcher. |
| `OMSI_OFFICIAL_KEY` | path | `omsi-app::lan` |  | The official server's signing key (a file): the server says where it is reached. |
| `OMSI_PLUGIN_HOST32` | path | `omsi-plugin` |  | The 32-bit plugin host (`omsi-plugin-host32.exe`), in place of the one beside the game. |
| `OMSI_ROOT` | path | `omsi-launcher-core` |  | The original OMSI 2 installation, read first when the content is looked for (also where the tests find the installed content). |
| `OMSI_WINE` | path | `omsi-plugin` |  | The `wine` that runs the 32-bit plugin host outside Windows. |

## Diagnostics

| Variable | Value | Read by | env.txt | What it does |
| --- | --- | --- | --- | --- |
| `OMSI_AI_MODEL_LOCK` | switch | `omsi-sim::ai` |  | AI cars steer no further than their model's lock, to compare. |
| `OMSI_AI_WAY_ONLY` | switch | `omsi-app::traffic` |  | AI cars on their way and the plain ground sampler, as before (to compare). |
| `OMSI_AUTOPILOT` | number | `omsi-app::offscreen` |  | Offscreen: drive the bus at this speed (km/h) along the straightest lanes. |
| `OMSI_BATCH` | number | `omsi-app::scene` |  | Prepare the tiles of a whole-map load this many at a time, as the window's streaming does. |
| `OMSI_BENCH` | number | `omsi-app::offscreen` |  | Offscreen: draw the final picture this many more times and log the median times. |
| `OMSI_BENCH_FRAMES` | switch | `omsi-app::offscreen` |  | Offscreen: log each frame of `OMSI_BENCH`. |
| `OMSI_BLEND_AB` | switch | `omsi-app::offscreen` |  | Offscreen: the same moment also drawn with the blended draws in the old order. |
| `OMSI_BRIDGE_ONLY` | switch | `omsi-net` |  | A joining game tries only what the rendezvous tells, not the code's own addresses. |
| `OMSI_BUDGET_FROM` | text | `omsi-app::offscreen` |  | Offscreen: meet the texture budget from this place first (`x,y[,MB]`). |
| `OMSI_CAM_VEHICLE` | text | `omsi-app::offscreen` |  | Offscreen: the camera at this place in the player's vehicle. |
| `OMSI_CHECK_ENTRIES` | switch | `omsi-app::offscreen` |  | Offscreen: check that there is ground where a player is put down. |
| `OMSI_CHECK_GROUND` | switch | `omsi-app::humans` |  | Log, every two seconds, the people on foot with a walkable surface over their heads. |
| `OMSI_CHECK_OBJECTS` | switch | `omsi-app::scene` |  | Log the surface objects standing far over or under the ground. |
| `OMSI_CHECK_OBSTACLES` | switch | `omsi-app::offscreen` |  | Offscreen: drive every lane as a bus and list the objects that would stop it. |
| `OMSI_CHECK_OVERLAP` | switch | `omsi-app::traffic` |  | Log the AI vehicles that overlap the player's or one another. |
| `OMSI_CHECK_ROADS` | switch | `omsi-app::offscreen` |  | Walk the driving lanes and report where no road surface is drawn under them. |
| `OMSI_CHECK_SPIKES` | switch | `omsi-app::scene` |  | Log the spline faces standing taller than the profile, gradient and cant allow. |
| `OMSI_CHECK_SPLINES` | switch | `omsi-app::offscreen` |  | Report chained splines whose ends do not meet in height. |
| `OMSI_CHECK_TPOSE` | switch | `omsi-app::humans` |  | Log the people drawn with their arms out (the rest pose). |
| `OMSI_CHECK_TRIPS` | text | `omsi-app::schedule` |  | Log every step of the trip of this name (and check the trips' routes offscreen). |
| `OMSI_CHECK_TYPES` | switch | `omsi-app::scene` |  | List the object and spline files the map names that are not installed. |
| `OMSI_CHECK_WALLS` | switch | `omsi-app::humans` |  | Log the passengers in a bus standing off its walkways. |
| `OMSI_CHECK_WHEELS` | switch | `omsi-app::offscreen` |  | Offscreen: probe the ground along every lane's wheel tracks as a tyre does. |
| `OMSI_CHURN` | text | `omsi-app::world_load` |  | Load the tiles around this far point (`x,y`), then the start area again, so that the picture is drawn from recycled GPU slots. |
| `OMSI_CLICK_ALL` | switch | `omsi-app::offscreen` |  | Offscreen: aim at every clickable part of the cockpit and say which one is hit. |
| `OMSI_CONDENSATION` | text | `omsi-app::offscreen` |  | Offscreen: the cabin air and the condensation on the glass after this long with this many aboard (`minutes,people[,engine 0/1]`). |
| `OMSI_CROSSING_DEFORM` | switch | `omsi-app::scene` |  | The ground under crossings deformed to them. |
| `OMSI_CUT_PLAIN` | switch | `omsi-app::scene` |  | The terrain cut as a plain texture, without mipmaps made on the GPU. |
| `OMSI_DAY_AIR` | text | `omsi-render::sky::air` |  | The air fixed for comparisons (`haze,angstrom[,height,strat]`). |
| `OMSI_DEBUG_AI_WIDE` | switch | `omsi-sim::ai` |  | Log, every tenth of a second, an AI car standing over 1.5 m beside its way. |
| `OMSI_DEBUG_ANIM` | text | `omsi-sim::vehicle` |  | Log the animation of the meshes whose name contains this text. |
| `OMSI_DEBUG_AUDIO` | number | `omsi-audio::mixer` |  | Log, every this many seconds (10 by default), how the sound device's callbacks come (late ones, blocks mixed too slowly, waits for the voice list) and the loops started again soon after they stopped. |
| `OMSI_DEBUG_BOARDS` | switch | `omsi-app::schedule` |  | Log what the stops' departure boards show. |
| `OMSI_DEBUG_CAMERA` | number | `omsi-app::camera_arm` |  | Log the camera's arm (1, or a higher number for more). |
| `OMSI_DEBUG_CAR` | number | `omsi-app::traffic` |  | Log the AI vehicle with this id, every frame. |
| `OMSI_DEBUG_CAREER` | switch | `omsi-app::career` |  | Log the jolts that cost points in the career. |
| `OMSI_DEBUG_COLLISION` | switch | `omsi-app::scene` |  | Log the obstacles' collision boxes and meshes, and the ones a road runs through. |
| `OMSI_DEBUG_CONES` | switch | `omsi-app::lights` |  | Log the lamps' fog cones. |
| `OMSI_DEBUG_CULL` | text | `omsi-render::renderer::lists` |  | Log the culling of instances within 150 m; `x,y,radius` logs those near that point. |
| `OMSI_DEBUG_DOORS` | switch | `omsi-app::traffic` |  | Log the timetable buses' doors twice a second. |
| `OMSI_DEBUG_DRAWS` | switch | `omsi-render::renderer::lists` |  | Log the draw counts and the instances changed each frame. |
| `OMSI_DEBUG_DRIVER` | switch | `omsi-app::driver` |  | Log how the driver's arms reach the wheel and the levers. |
| `OMSI_DEBUG_ENHANCED` | number | `omsi-render::lighting::model` |  | The enhanced main pass shows one of its terms alone (a number). |
| `OMSI_DEBUG_EXPOSURE` | switch | `omsi-render::lighting::exposure` |  | Log the automatic exposure. |
| `OMSI_DEBUG_FLICKER` | switch | `omsi-render::renderer::lists` |  | Log the near instances drawn in one frame and not in the next. |
| `OMSI_DEBUG_FLOAT` | switch | `omsi-app::scene` |  | Log how high the objects standing on the ground are placed. |
| `OMSI_DEBUG_FOG_LAMPS` | switch | `omsi-render::passes::fog_lamps` |  | Log the fog lamps, light shafts and glare of a frame. |
| `OMSI_DEBUG_FOOT` | switch | `omsi-app::on_foot` |  | Log the player walking: the doors and seats near and where the avatar is drawn. |
| `OMSI_DEBUG_HUMANS` | switch | `omsi-app::humans` |  | Log the people's types and what `OMSI_DEBUG_PAX` logs. |
| `OMSI_DEBUG_IBIS` | switch | `omsi-sim::ibis` |  | Log the IBIS's search for a way to type a duty in. |
| `OMSI_DEBUG_INTERIOR` | switch | `omsi-app::scene` |  | Log the vehicles' interior lamps and the meshes they light. |
| `OMSI_DEBUG_JUNCTION` | switch | `omsi-app::traffic` |  | Log why a car at a junction waits. |
| `OMSI_DEBUG_LAMPS` | switch | `omsi-app::scene` |  | Log the traffic lights and the crossings they belong to. |
| `OMSI_DEBUG_LAN` | switch | `omsi-app::lan` |  | Log, every few seconds, what the game knows of every LAN player and the bytes sent. |
| `OMSI_DEBUG_LANES` | text | `omsi-app::traffic` |  | Log these lanes (indices, or `at:x,y,r` for the street lanes near a point). |
| `OMSI_DEBUG_LIGHT` | switch | `omsi-app::lights` |  | A test lamp over the camera, and a log of the static lights. |
| `OMSI_DEBUG_LIGHTS` | text | `omsi-app::traffic` |  | Log the traffic lights of these crossings (indices, or `near`). |
| `OMSI_DEBUG_LIGHT_GRID` | switch | `omsi-render::renderer::lights` |  | Log the lights a full cell of the light grid leaves out. |
| `OMSI_DEBUG_MESHES` | text | `omsi-app::spawn` |  | Log the player vehicle's meshes (and, for the ones whose file contains this text, the part of the texture each slot shows). |
| `OMSI_DEBUG_MIRRORS` | switch | `omsi-app::camera_util` |  | Log where the mirrors' cameras are. |
| `OMSI_DEBUG_MISSING` | switch | `omsi-app::scene` | yes | Log where a texture, an object or an attachment that was not found was looked for. |
| `OMSI_DEBUG_NAN` | switch | `omsi-script::vm` |  | Warn at every write of a NaN or an infinity into a script variable. |
| `OMSI_DEBUG_NAV` | switch | `omsi-app::navigator` | yes | Log the navigator's search for a way back to the route. |
| `OMSI_DEBUG_OBJECTS` | switch | `omsi-app::scene` |  | Log the scenery objects placed and the ones outside their tile. |
| `OMSI_DEBUG_OBJMAT` | text | `omsi-app::scene` |  | Log how the material slots of the objects whose file name contains this text are made. |
| `OMSI_DEBUG_PARTICLES` | switch | `omsi-app::scene` |  | Log the particle emitters near the camera and the smoke they make. |
| `OMSI_DEBUG_PASS` | switch | `omsi-app::traffic` |  | Log why a car standing behind something does not go round it. |
| `OMSI_DEBUG_PAX` | switch | `omsi-app::humans` |  | Log the passengers: who waits, boards and leaves, and why. |
| `OMSI_DEBUG_PHYSICS` | number | `omsi-sim::vehicle` |  | Log the bus's impacts and the obstacles it hits (a number: also the bus's pose every this many seconds). |
| `OMSI_DEBUG_POPULATION` | switch | `omsi-app::traffic` |  | Log the AI cars placed and taken away round the camera. |
| `OMSI_DEBUG_PROPS` | switch | `omsi-app::spawn` |  | Log the player vehicle's mesh properties (and the timetable buses' display variables). |
| `OMSI_DEBUG_PUDDLES` | number | `omsi-render::effects::puddles` |  | The puddle reflection shows one of its terms (a number). |
| `OMSI_DEBUG_RAIN` | switch | `omsi-app::rain` |  | Log the rain's particles and the tyres throwing water. |
| `OMSI_DEBUG_RASTER` | text | `omsi-app::scene` |  | Log the ground's raster around this point (`x,y`). |
| `OMSI_DEBUG_REPEATERS` | switch | `omsi-app::tiles` |  | Log the spline object rows whose start the map and the spline chain disagree about. |
| `OMSI_DEBUG_REST` | switch | `omsi-app::offscreen` |  | Offscreen: log where the bus came to rest on the ground after it was placed. |
| `OMSI_DEBUG_ROUTES` | switch | `omsi-app::schedule` |  | Log where the timetable's routes leave a gap between lanes. |
| `OMSI_DEBUG_RT` | number | `omsi-render::effects::rt` |  | Log the ray-tracing structures; a number shows one of its buffers. |
| `OMSI_DEBUG_SERVICES` | switch | `omsi-app::services` |  | Log the petrol stations and how far the bus is from them. |
| `OMSI_DEBUG_SHADOW` | number | `omsi-render::renderer::lists` |  | Log the shadow casters at least this large (a radius, default 3). |
| `OMSI_DEBUG_SHADOW_FAR` | switch | `omsi-render::passes::shadow` |  | Log why the far shadow map is drawn again. |
| `OMSI_DEBUG_SIGNALS` | switch | `omsi-app::scene` |  | Log what the signals show. |
| `OMSI_DEBUG_SKY` | switch | `omsi-render::renderer::sky` |  | Log the sky, its cloud cover and the lamps' glow on it. |
| `OMSI_DEBUG_SOUND` | number | `omsi-app::weather_setup` |  | Log, every this many seconds (5 by default), what the player's bus and the environment sound. |
| `OMSI_DEBUG_SPLINES` | switch | `omsi-app::scene` |  | Log what the splines of a tile make: lanes, meshes, ground cut. |
| `OMSI_DEBUG_STARTUP` | switch | `omsi-sim::startup` | yes | Log each step of a vehicle's start-up and the candidates it weighs. |
| `OMSI_DEBUG_STOPS` | switch | `omsi-app::bus_service` |  | Log the timetable buses serving or passing their stops. |
| `OMSI_DEBUG_STUCK` | switch | `omsi-app::traffic` |  | Log the cars standing behind an obstacle for over 20 s, and why they wait at junctions. |
| `OMSI_DEBUG_SURFACES` | switch | `omsi-app::scene` |  | Log the painted ground layers and the water of each tile. |
| `OMSI_DEBUG_SWITCHES` | switch | `omsi-app::scene` |  | Log the switches thrown for a train. |
| `OMSI_DEBUG_TEXT` | switch | `omsi-sim::host` |  | Log what the scripts write into the text textures. |
| `OMSI_DEBUG_TEXTURES` | switch | `omsi-app::scene` |  | Log the textures, their sizes and formats. |
| `OMSI_DEBUG_TRAFFIC` | switch | `omsi-app::traffic` |  | Log why the AI cars, the parked cars and the timetabled buses do what they do. |
| `OMSI_DEBUG_TRAILER` | switch | `omsi-sim::vehicle::articulation` |  | Log the gap and the angles of the joints of articulated buses and trailers. |
| `OMSI_DEBUG_TRAILERS` | switch | `omsi-app::traffic` |  | Log the coupled parts of trains and articulated vehicles off the level of what pulls them. |
| `OMSI_DEBUG_TRIGGERS` | switch | `omsi-app::offscreen` |  | Offscreen: log the variables each trigger changes. |
| `OMSI_DEBUG_UNLINKED` | switch | `omsi-sim::traffic` |  | Log a lane end that found no lane to go on with although one starts near it. |
| `OMSI_DEBUG_UPLOAD` | switch | `omsi-app::scene` |  | Log the uploads to the GPU that take longer than 8 ms. |
| `OMSI_DEBUG_VARIANTS` | text | `omsi-app::scene` |  | Log the material of the variant meshes whose variable contains this text. |
| `OMSI_DEBUG_VARS` | text | `omsi-app::offscreen` |  | Offscreen: log these variables of the player's bus at the end (`a,b`). |
| `OMSI_DEBUG_VARS_EVERY` | number | `omsi-app::offscreen` |  | Offscreen: log `OMSI_DEBUG_VARS` every this many seconds of the drive. |
| `OMSI_DEBUG_VAR_SYNC` | text | `omsi-app::lan` |  | Log what arrives for this vehicle variable from the other LAN players. |
| `OMSI_DEBUG_VIEW_LAMPS` | switch | `omsi-render::renderer::frame` |  | Log how much of the view the lamps light. |
| `OMSI_DEBUG_WALLS` | switch | `omsi-app::humans` |  | Log the people kept out of walls. |
| `OMSI_DEBUG_WARP` | switch | `omsi-app::scene` |  | Log how far a crossing's mesh was moved to lie on the ground. |
| `OMSI_DEBUG_WHEELS` | switch | `omsi-app::traffic` |  | Log the AI types' wheels as they stand at start. |
| `OMSI_DRIVER_HANDS` | text | `omsi-app::driver` |  | Both of the driver's hands held at these wheel angles (`left,right`), for close-ups. |
| `OMSI_DRIVER_SHIFTER` | text | `omsi-app::driver` |  | The mesh the driver's hand takes as the gear lever (part of its name). |
| `OMSI_DUMP_CUT` | path | `omsi-app::scene` |  | Write the terrain cut of every tile as a picture into this folder. |
| `OMSI_DUMP_GROUND` | path | `omsi-app::scene` |  | Write every loaded tile's terrain and how much of it the roads cut away to this file. |
| `OMSI_DUMP_SCENERY_TEXT` | path | `omsi-app::scene` |  | Write the scenery objects' text textures, as drawn, into this folder. |
| `OMSI_DUMP_SCRIPTTEX` | path | `omsi-app::offscreen` |  | Offscreen: write the player vehicle's script textures into this folder. |
| `OMSI_ENV_PHOTO` | number | `omsi-render::renderer::sky` |  | The strength of the `[matl_envmap]` photo on reflections (default 1, 0 leaves it out). |
| `OMSI_FIXED_SCALE` | switch | `omsi-app::app_events` |  | The render scale stays as set, not changed with the frame time. |
| `OMSI_FOLLOW_CAM` | text | `omsi-app::camera_util` |  | Offscreen: follow the vehicle with this camera (`dx,dy,dz,yaw,pitch[,1]`). |
| `OMSI_FORCE_OPAQUE` | switch | `omsi-app::scene` |  | Every vehicle material opaque, except the dirt overlays. |
| `OMSI_GLASS_WIND` | number | `omsi-app::offscreen` |  | Offscreen: the rain on the glass as the bus would meet it at this speed (m/s). |
| `OMSI_GL_TEXTURE_UNITS` | switch | `omsi-render::gpu::caps` |  | With `OMSI_GPU_ARRAYS`, keep to OpenGL's sixteen texture units, as such a chip does. |
| `OMSI_GPU_ARRAYS` | text | `omsi-render::api::renderer` |  | `textures` or `nostorage`: read the scene's arrays as an older graphics chip has to. |
| `OMSI_GPU_LIMITS` | text | `omsi-render::api::renderer` | yes | `default` or `downlevel`: pretend the graphics card can do only that much. |
| `OMSI_GPU_TIMERS` | switch | `omsi-render::api::renderer` | yes | Time each pass on the GPU (with `OMSI_PROFILE`). |
| `OMSI_GPU_TIMERS_RAW` | switch | `omsi-render::output::diagnostics` |  | Log the GPU's raw time stamps (with `OMSI_GPU_TIMERS`). |
| `OMSI_GROUND_GAP` | path | `omsi-app::ground_gap` |  | Write how far the drawn ground lies from the wheels to this file (`-`: the log only). |
| `OMSI_GROUND_GAP_RADIUS` | number | `omsi-app::ground_gap` |  | How far round the camera (m, 150 by default) `OMSI_GROUND_GAP` looks. |
| `OMSI_GROUND_LANES` | switch | `omsi-app::ground_gap` |  | Log, along every street lane, how far the drawn ground lies from the lane. |
| `OMSI_GROUND_SAMPLE` | path | `omsi-app::offscreen` |  | Offscreen: write the ground under the lanes near the start to this file. |
| `OMSI_HEIGHTPROFILE_GROUND` | switch | `omsi-app::scene` |  | Wheels run on the splines' height profiles, not on the drawn road. |
| `OMSI_HIDE_MESH` | text | `omsi-app::scene` |  | Leave out the vehicle meshes whose file names contain one of these parts (`a\|b`). |
| `OMSI_HOLE_PHOTO` | switch | `omsi-app::offscreen` |  | Offscreen: also photograph each place from above down to 25 m under the lane. |
| `OMSI_JOINT_ANGLE` | number | `omsi-app::offscreen` |  | Offscreen: the rear section held at this angle (degrees) to the front one. |
| `OMSI_KEEP_ALLOCATOR` | switch | `omsi-app::startup` |  | No restart of the game with the allocator settings it wants (macOS). |
| `OMSI_LANES_NEAR` | text | `omsi-app::offscreen` |  | Offscreen: list the driving lanes passing near this point (`x,y,r`). |
| `OMSI_LAN_TRACE` | path | `omsi-app::lan_world` |  | Write the LAN world's poses to this file. |
| `OMSI_LIST_ALIGNED` | switch | `omsi-app::scene` |  | Log the splines that align the terrain, with their mode. |
| `OMSI_METER` | text | `omsi-render::lighting::exposure` |  | The exposure meter's settings, up to six comma-separated numbers. |
| `OMSI_MIRROR_ENHANCED` | switch | `omsi-app::camera_util` |  | The mirrors drawn with the window's light in the enhanced renderer, not darkened at dusk. |
| `OMSI_MIRROR_HUD` | number | `omsi-app::offscreen` |  | Offscreen: lay the mirror panels (1 to 3) over the picture. |
| `OMSI_NAV_MAP` | switch | `omsi-app::offscreen` |  | Offscreen: the navigator shows its map. |
| `OMSI_NAV_MAP_MPP` | number | `omsi-app::navigator` |  | The navigator's city map at this many metres per pixel (0.25 to 20), for pictures to compare. |
| `OMSI_NAV_PROBE` | text | `omsi-app::navigator` |  | Log the lanes of the network that start or end near this point (`x,y[,r]`), and how they link. |
| `OMSI_NAV_SCHEDULE` | switch | `omsi-app::offscreen` |  | Offscreen: the navigator shows the timetable. |
| `OMSI_NOZCHECK_BIAS` | switch | `omsi-app::scene` |  | `[matl_noZcheck]` read as "no depth test". |
| `OMSI_NO_ANIMPARENT` | switch | `omsi-sim::anim` |  | Every mesh on its own, without `[animparent]`, to compare. |
| `OMSI_NO_ATTACH_FALLBACK` | switch | `omsi-app::scene` |  | Attachments with an index the parent does not have dropped, not placed on it. |
| `OMSI_NO_BOUNDS_CACHE` | switch | `omsi-render::api::scene` |  | Every instance's bounds worked out again each frame. |
| `OMSI_NO_BRIDGE` | switch | `omsi-net::bridge` |  | The LAN session leaves the internet alone (no rendezvous bridge). |
| `OMSI_NO_BUNDLES` | switch | `omsi-render::renderer::encode` |  | The main pass encoded directly, without render bundles. |
| `OMSI_NO_CULL` | switch | `omsi-render::passes::draw` | yes | One-sided faces drawn from both sides. |
| `OMSI_NO_GLARE` | switch | `omsi-render::passes::fog_lamps` |  | No glare from the sun. |
| `OMSI_NO_GLASS_PICTURE` | switch | `omsi-render::renderer::frame` |  | No rain drawn on the glass. |
| `OMSI_NO_GROUND_PAINT` | switch | `omsi-app::scene` |  | No painted ground layers on the tiles. |
| `OMSI_NO_GROUND_SPLINE_BATCHING` | switch | `omsi-app::scene` |  | The ground splines drawn one by one. |
| `OMSI_NO_INTERP` | switch | `omsi-app::lan` |  | The other LAN players' buses drawn where they were last heard of, not interpolated. |
| `OMSI_NO_LAN_MODS` | switch | `omsi-app::lan_mods` |  | A LAN host serves no mods and a joining game fetches none. |
| `OMSI_NO_LIGHT_MAP` | switch | `omsi-app::scene` |  | The tiles' night light maps left out. |
| `OMSI_NO_MAIN_SPLIT` | switch | `omsi-render::passes::main` |  | The main pass in one part, not split in two. |
| `OMSI_NO_MATERIAL_SPLINE_BATCHING` | switch | `omsi-app::scene` |  | The splines batched by cell only, not by material. |
| `OMSI_NO_MODEL_ORDER` | switch | `omsi-app::scene` |  | The opaque parts of a vehicle drawn first, not in model order. |
| `OMSI_NO_MSAA_PREPASS` | switch | `omsi-render::passes::main` |  | No multisampled depth prepass. |
| `OMSI_NO_PBR` | switch | `omsi-app::scene` |  | No PBR maps on the textures. |
| `OMSI_NO_POLL_THREAD` | switch | `omsi-render::gpu::open` |  | No thread that keeps the GPU's work moving between frames. |
| `OMSI_NO_PUDDLE_GLASS_DEPTH` | switch | `omsi-render::effects::puddles` |  | Puddles reflect glass without its depth. |
| `OMSI_NO_PUDDLE_REFLECTIONS` | switch | `omsi-render::renderer::frame` |  | Wet roads reflect no scene. |
| `OMSI_NO_PUDDLE_VEHICLE` | switch | `omsi-render::effects::puddles` |  | Puddles reflect no vehicles. |
| `OMSI_NO_RENDER_POOL` | switch | `omsi-render::gpu::context` |  | The draw lists encoded on the frame's own thread, not on a pool. |
| `OMSI_NO_RT_FRAME` | switch | `omsi-render::renderer::frame` |  | The ray-traced frame of Enhanced+ left out. |
| `OMSI_NO_SMOKE` | switch | `omsi-render::passes::main` |  | No smoke. |
| `OMSI_NO_SNOWFALL` | switch | `omsi-render::passes::main` |  | No falling snow. |
| `OMSI_NO_SPLINE_BATCHING` | switch | `omsi-app::scene` |  | The splines drawn one by one. |
| `OMSI_NO_SPLINE_HOLES` | switch | `omsi-app::scene` |  | No holes cut into the terrain by the splines. |
| `OMSI_NO_SPRAY` | switch | `omsi-app::app_events` |  | No spray from the tyres on wet roads. |
| `OMSI_NO_TUNNEL` | switch | `omsi-app::lan` |  | A LAN host opens no tunnel to the internet. |
| `OMSI_NO_VAR_SYNC` | switch | `omsi-app::lan` |  | The vehicles' variables are not sent to the other LAN players. |
| `OMSI_OLD_WORLD_GRID` | switch | `omsi-map` |  | The one tile size of 371.9 m for maps on world coordinates, to compare. |
| `OMSI_ONLY_MESH` | text | `omsi-app::scene` |  | Draw only the vehicle meshes whose file names contain one of these parts (`a\|b`), and log their materials. |
| `OMSI_ONLY_OBJECT` | text | `omsi-app::scene` |  | Draw only the scenery objects whose file name contains this text. |
| `OMSI_ONLY_SURFACES` | switch | `omsi-render::renderer::lists` |  | Only the surfaces drawn. |
| `OMSI_ORBIT_DIST` | number | `omsi-app::camera_util` |  | Offscreen: how far (m, 18 by default) the outside camera sits from the vehicle. |
| `OMSI_PARKED_PULL_OUT` | number | `omsi-app::traffic` |  | The chance per population pass (about 2 s, 0.035 by default) that a parked car drives off. |
| `OMSI_PARK_IN` | number | `omsi-app::traffic` |  | The chance per population pass (about 2 s, 0.04 by default) that a car parks. |
| `OMSI_PAX_CAM` | number | `omsi-app::offscreen` |  | Offscreen: `--view pax` from the vehicle's passenger camera of this number. |
| `OMSI_PAX_CROSS` | text | `omsi-app::humans` |  | Send a few pedestrians across the signalised crossing nearest this point (`x,y`). |
| `OMSI_PAX_WAITING` | number | `omsi-app::humans` |  | This many people waiting at every stop. |
| `OMSI_POPULATION_SHOTS` | switch | `omsi-app::offscreen` |  | Offscreen: pictures of the cars placed inside the camera's view. |
| `OMSI_PROBE` | text | `omsi-app::offscreen` |  | Offscreen: the terrain and road heights along a line (`x0,y0,x1,y1[,n]`). |
| `OMSI_PROBE_GRID` | text | `omsi-app::offscreen` |  | Offscreen: the wheels' ground on a grid around a point (`x,y,half,step`). |
| `OMSI_PROFILE` | switch | `omsi-app` | yes | Log where the time of a frame, a tile load and the memory go. |
| `OMSI_PROFILE_GPU` | switch | `omsi-app::app_events` |  | Wait for the GPU in each frame, so that its time shows as a stage of its own. |
| `OMSI_PUDDLE_F0` | number | `omsi-render::renderer::sky` |  | The reflectance of puddles head on (default 0.08, kept between 0.02 and 0.2). |
| `OMSI_PUDDLE_THICKNESS` | number | `omsi-render::effects::puddles` |  | The thickness of the puddle reflection's ray test (default 0.12). |
| `OMSI_REPAIR_BODY_DEPTH` | switch | `omsi-app::scene` |  | A blended vehicle body guessed opaque. |
| `OMSI_ROAD_CUT` | switch | `omsi-geometry` |  | The ground cut away under the roads, to compare. |
| `OMSI_ROAD_PHOTO` | switch | `omsi-app::offscreen` |  | Offscreen: photograph the road network from above and say where grass shows. |
| `OMSI_ROAD_PHOTO_N` | number | `omsi-app::offscreen` |  | Offscreen: how many points (400 by default) `OMSI_ROAD_PHOTO` photographs. |
| `OMSI_ROAD_PHOTO_SIDE` | number | `omsi-app::offscreen` |  | Offscreen: how far to either side of the lane `OMSI_ROAD_PHOTO` looks. |
| `OMSI_ROAD_PHOTO_SLANT` | number | `omsi-app::offscreen` |  | Offscreen: `OMSI_ROAD_PHOTO` from a driver's eye this far back along the lane. |
| `OMSI_RT_REFL_HALF` | switch | `omsi-render::effects::rt` |  | The reflection rays traced per 2 x 2 pixels. |
| `OMSI_SHADOW_FAR_EVERY_FRAME` | switch | `omsi-render::passes::shadow` |  | The far shadow map drawn again every frame. |
| `OMSI_SHADOW_NEAR_EVERY_FRAME` | switch | `omsi-render::passes::shadow` |  | The near shadow map drawn again every frame. |
| `OMSI_SKIP_OBJECT` | text | `omsi-app::scene` |  | Leave out the scenery objects whose file name contains this text. |
| `OMSI_SKIP_PIPE` | text | `omsi-render::renderer::lists` |  | The pipelines of these kinds left out (comma-separated numbers), to time what each costs. |
| `OMSI_SPOT_SELECT` | number | `omsi-app::lights` |  | Switch on this spotlight (as `Spot_Select` would), to check the headlights in a picture. |
| `OMSI_SURFACE_BIAS` | number | `omsi-render::renderer::build` |  | The depth bias of surfaces drawn on the ground (default -24). |
| `OMSI_SURFACE_FLUSH` | number | `omsi-app::scene` |  | How far (m, 0.12 by default) the ground may stand over a flush surface before it is cut away. |
| `OMSI_SUSP_TRACE` | path | `omsi-app::offscreen` |  | Offscreen: write each wheel's suspension, frame by frame, to this CSV file. |
| `OMSI_SUSP_TRACE_WINDOW` | path | `omsi-app::player` |  | Write each wheel's suspension travel, frame by frame, to this CSV file (window runs). |
| `OMSI_TERRAIN_ALIGN` | switch | `omsi-app::scene` |  | The terrain moved to the roads' height under them. |
| `OMSI_TEXTURE_RAIN` | switch | `omsi-app::scene` |  | The windows' rain drawn with OMSI 2's texture, not as a film of drops. |
| `OMSI_TONE_CONTRAST` | text | `omsi-render::lighting::exposure` |  | The tone curve's contrast by day and by night (`day[,night]`). |
| `OMSI_TRACE_AI` | path | `omsi-app::traffic` |  | Write every AI vehicle's state, frame by frame, to this CSV file. |
| `OMSI_TRACE_AI_BUSES` | switch | `omsi-app::traffic` |  | `OMSI_TRACE_AI` writes the timetable buses only. |
| `OMSI_TRACE_PAX` | path | `omsi-app::humans` |  | Write the passengers' steps to this file. |
| `OMSI_TRACE_REMOTE` | path | `omsi-app::lan` |  | Write where each other LAN player's bus is drawn, every frame, to this CSV file. |
| `OMSI_TRACE_STEER` | path | `omsi-app::app_events` |  | Write the mouse steering, frame by frame, to this CSV file. |
| `OMSI_TRACE_VARS` | text | `omsi-app::offscreen` |  | Offscreen: log these variables every half second (`a,b,$c`; `$` reads a string). |
| `OMSI_TRAFFIC_ALL_GROUPS` | switch | `omsi-app::traffic` |  | The vehicle groups the map keeps to some roads drive everywhere. |
| `OMSI_TRIGGER_ALL` | switch | `omsi-app::offscreen` |  | Offscreen: fire every trigger of the player's bus and list the ones that change nothing. |
| `OMSI_WARM_FRAMES` | number | `omsi-app::offscreen` |  | Offscreen: draw this many frames before the picture. |
| `OMSI_WATCH_VARS` | text | `omsi-app::app_events` |  | Log every change of these variables of the player's bus (`a,b`). |
| `OMSI_WETNESS` | number | `omsi-app::puddles` |  | How wet the roads are (0 to 1), in place of what the weather made of them. |
| `OMSI_WHEEL_TRACE` | switch | `omsi-app::offscreen` |  | Offscreen: log, once a second, how deep a drawn tyre goes into the road. |
| `OMSI_WINDY_TREES` | text | `omsi-app::settings` |  | `0` or `1`: the trees move in the wind or not, whatever the setting says. |

## Tests

| Variable | Value | Read by | env.txt | What it does |
| --- | --- | --- | --- | --- |
| `OMSI_AUDIT_LINE` | text | `omsi-sim (examples)` |  | The line the `bus_audit` example types in (`5E`), in place of the one from the code. |
| `OMSI_BACKGROUND` | switch | `omsi-app` |  | A test window that does not take the keyboard, and no dialog on a fatal error. |
| `OMSI_CLOUDFLARED_OWN` | switch | `omsi-net::tunnel` |  | Only the game's own `cloudflared`, to test its download. |
| `OMSI_DRIVE_PROFILE` | text | `omsi-app::offscreen` |  | Offscreen: hold the pedals and the wheel from each time on (`t throttle brake [steer]/ …`). |
| `OMSI_DRIVE_V0` | number | `omsi-app::offscreen` |  | Offscreen: the bus's speed (km/h) on the first frame. |
| `OMSI_FAKE_GPU_ERROR` | text | `omsi-render` |  | Fail the graphics on purpose to test the fallbacks (`open`, `open-panic`, `build`, `lost-build`, `pipeline`, `frame`, `lost`, `lost-poll`, `oom`). |
| `OMSI_FLEET_AHEAD` | number | `omsi-app::schedule` |  | How far ahead (minutes, 25 by default) the fleet is read. |
| `OMSI_FLEET_IDLE` | number | `omsi-app::schedule` |  | How long (s) a vehicle set nobody uses is kept, in place of the default. |
| `OMSI_HIDE_WINDOW` | text | `omsi-app::app_events` |  | Treat the window as hidden between these seconds (`from,to`). |
| `OMSI_INPUT` | text | `omsi-app::input_script` |  | Drive the window's handlers (mouse, keys, `look`, `turn`, `touch`) from this script (`t=3 move x,y; t=3.2 press; t=4 key F3; …`). |
| `OMSI_LAN_AUDIO` | switch | `omsi-app::offscreen` |  | Offscreen: with LAN, the other buses' sounds heard at the camera (for the logs). |
| `OMSI_LAN_SAY` | text | `omsi-app::lan` |  | Chat lines said at these seconds of the session (`30=Hallo;45=Tschüss`), for LAN tests. |
| `OMSI_LAUNCHER_EXIT` | number | `omsi-app::launcher` |  | Close the launcher after this many seconds. |
| `OMSI_LAUNCHER_INPUT` | text | `omsi-app::launcher` |  | Work the launcher from this script (`t=2 click 412,60; t=3 type 76; …`). |
| `OMSI_LAUNCHER_PAGE` | text | `omsi-app::launcher` |  | Open the launcher at this page (`drive:2`, `settings:3`, `controls:1`, …). |
| `OMSI_LAUNCHER_SHOT` | text | `omsi-app::launcher` |  | Write a picture of the launcher after this many seconds (`secs:file.png`). |
| `OMSI_ORIGINAL` | path | `omsi-sim (examples)` |  | The original installation for the example programs of `omsi-sim`. |
| `OMSI_PHYSICS_TRACE_DIR` | path | `omsi-sim` |  | Where the physics traces test writes its traces. |
| `OMSI_RENDER_CLOCK` | number | `omsi-render::renderer::build` |  | Start the animation clock this many seconds later, for offscreen stills. |
| `OMSI_RENDER_OCCLUDED` | switch | `omsi-app::app_events` |  | Draw even while the window is hidden. |
| `OMSI_STATE_TRACE` | path | `omsi-app::state_trace` |  | Write the offscreen run's state, step by step, to this file (the B0 trace). |
| `OMSI_TEST_CONTENT` | path | `omsi-o3d` |  | The installed content for the tests, when `OMSI_ROOT` is not set. |
| `OMSI_TEST_WINE_DIR` | path | `omsi-plugin` |  | The folder with Wine for the plugin host's test. |
| `OMSI_UI_PREVIEW` | path | `omsi-app::plugin_ui` |  | Write a picture of the plugins' panels to this file (`plugin_ui`'s preview test). |

## Set by the program itself

| Variable | Value | Read by | env.txt | What it does |
| --- | --- | --- | --- | --- |
| `OMSI_BUILD` | text | `omsi-app::startup` |  | The commit and date the game was built from, set by `omsi-app`'s `build.rs` for the compiler (read with `env!`, not at run time). |
| `OMSI_INSTANCE` | text | `omsi-launcher-core::instances` |  | The launcher's id for a game it started (the game's LAN files are named after it). |
