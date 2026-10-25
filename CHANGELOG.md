# Changelog

Changes made by the Project Depot team. History inherited from openOMSI (up to the rebranding
release 0.2.17) is frozen in [CHANGELOG_UPSTREAM.md](CHANGELOG_UPSTREAM.md).
Releases are published manually after review; see [docs/VERSIONING.md](docs/VERSIONING.md).

## Unreleased

### Changed
- Alt bar: the tiles' icons are drawn anew, a little larger, on one grid: a city bus seen from the side with a badge for Add, Switch, Remove and Reposition (a plus, two arrows, a cross, a pin), a steering wheel with a spark for AI drives, a timetable sheet and a calendar with a clock, a car with a warning sign for the accident, a wrench, a fuel pump, a direction arrow on the road for the help marker, a cloud with the sun and rain, a clipboard with the driver, and for Controls a keyboard, a mouse or a game controller after what the bus is driven with (the keyboard showed a steering wheel). A tile's name (or why it cannot be used) shows above the bar once the mouse has rested on the tile for a second, no longer at once while the mouse passes over it (the fold arrow's "Fold the bar" too); the keyboard's tile and the open window's name show as before.
- Settings → Interface: the launcher now opens with the Scene background and the top bar for navigation by default (it was Plain dark and the sidebar); both stay one choice away, and a choice already saved in `settings.cfg` (`ui_nav`, `ui_background`) is kept. Without a picture (no map, bus or screenshot of yours), Scene is the plain dark as before; in a narrow window the top bar shows icons, a name on hover.

### Added
- Vehicle integrity check (`omsi-check <OMSI 2 folder> --integrity-vehicle Vehicles/<folder>/<vehicle>.bus`), the same report as for a map: the model.cfg and its meshes and textures, the scripts and variable lists, the sound set and its sound files, and the paths and passenger-cabin files the vehicle file names. A missing model or script is an error, a missing sound or other file a warning, and a texture whose size is not a power of two is listed as not optimized.
- Map integrity check (`omsi-check <OMSI 2 folder> --integrity <map folder name>`; the first piece of a check the launcher will offer on request, for maps and later vehicles). It follows what the map names - global.cfg, tiles, objects and splines, models and meshes, textures - and lists what is wrong in three kinds: errors (an object, spline, model or mesh the map needs is missing or unreadable), warnings (a tile or texture missing, a path longer than 259 characters, a picture the game cannot read) and "not optimized" (works, but wasteful). The first "not optimized" rule is a texture whose size is not a power of two: DirectX 9, as in OMSI 2, enlarges it to the next power of two, so a 1400x1400 file takes 2048x2048 in video memory (behavior observed in OMSI 2 by the maintainer); the finding says so and by how much. The report names each missing file once with the first file that uses it and how many do. (For an unreadable object, spline or mesh the count of users can be lower than the real one.) Deeper than a check of the map's control files alone; written from the formats' documentation and the project's own loaders, with no code from another checker. Limits: the lookups follow the project's loaders, which can be more lenient than the original in places (for example season folders), so a clean report is not a guarantee. The chrono folders' patches are checked whether or not they are active. Texture slots not looked at: light maps, raindrop maps, free textures and the textures a material changes to. An old OS/2-style bmp header is reported as unreadable. Not yet checked: vehicles, sounds and scripts, textures that models name only for paints, and the launcher page.
- Information bar: a fourth layout, Custom bar (Settings → On screen, in the launcher and in the game's World options), shows the full bar less the items switched off one by one: time, speed and limit, trip distance, odometer, temperatures, tank, passengers, line and destination, next stop and delay (keys `info_item_time`, `info_item_speed`, `info_item_trip`, `info_item_odometer`, `info_item_temps`, `info_item_tank`, `info_item_pax`, `info_item_line`, `info_item_next`; `info_bar_mode=custom`). The line, destination and next stop from the map and the timetable are drawn as they are, no longer looked up as interface texts (a destination "Depot" or "Station" came out in another language), and the frame rate, when asked for, is in the bar of every layout. The option's description no longer says the bar is at the top of the screen.
- Information bar (Shift+Y) redesigned in the interface's look, with three layouts, a position and a frame-rate part. A dark plate with a hairline between groups: the time, the speed with the limit of the street as a white sign with a red frame (the limit the map gives the lane the vehicle drives along within eight metres, to five km/h; nothing off the streets, in a street the map gives no limit, or on a map without lanes), the trip distance, the odometer, the temperatures, the tank, the passengers aboard with the places the bus has ("36 / 111 Passengers", the places its scripts switch off not counted), the line as an accent-coloured badge with its destination, and the next stop with the delay in red when late and green when ahead. Settings → On screen (in the launcher and in the game's World options): Information bar layout (Full bar; Compact bar: time, speed and limit, line, next stop and delay; Essentials only: speed, limit and delay; the frame rate, when asked for, is added to each), Information bar position (top, or bottom of the screen; a navigator in a bottom corner stands above a bar along the bottom, and on a touch screen the bar stays between the buttons along the top) and Frame rate in the information bar. The frame rate now comes with its "1 % low" (the rate of the slowest one percent of the frames of the last ten seconds), in the bar and in the corner chip (`show_fps`), which has a green, amber or red dot by the rate. New keys `info_bar_mode` (`full`, `compact`, `essential`), `info_bar_pos` (`top`, `bottom`) and `info_bar_fps`; Shift+Y still only shows and hides the bar. Deliberate improvement beyond the original game's behavior, requested by the maintainer: the bar and its Shift+Y key come from the openOMSI baseline (MIT, key named in its changelog 0.1.1668) and the passenger capacity follows the public behavior of upstream 0.2.17 (MIT); the layouts, the speed limit and the 1 % low are a clean implementation designed from the project's own mockups. Under "DirectX9 (D3D9Ex)" it is drawn as before, as flat plates and text.
- Setup's Free up space card can compress the content with NTFS (LZX), the "compact" kind of Windows, and undo it. It offers the Depot's content folder (ticked) and the original OMSI 2 folder (not ticked) apart, estimates what each would free from a sample of its files (marked as an estimate), and compresses in the background with progress and a Cancel. Files the game writes, archives (.zip) and media that are compressed already are left alone. It is offered only on a fixed NTFS disk (never in a cloud-sync folder, on a network or removable disk), and says why when it is not. For the original OMSI 2 folder it strongly recommends playing through OmsiLaunch with the Omsi Prefetch module, with links; on a hard disk (or a disk that could not be identified) it warns that loading can be much slower and asks to confirm. Undo checks the free space first. Still to measure on real hardware: the effect of compressed files on loading times.
- The in-game windows follow the Windows 11 look: a title in plain case, rounded corners with a shadow, rounded buttons, and the buttons of a window's foot at the right with its hint at the left (every window: the menus, Add vehicle and the end-of-trip report too). The running timetable marks served stops and stops to come with outline icons, and a stop skipped or missed with a muted sign of its own.
- The Alt bar follows the Windows 11 look: tinted tiles with outline icons in the accent colour (the destination button shows a small destination sign) (filled while a tile's window is open, grey with the reason when it cannot be used), the name of the tile under the mouse above the bar, "Alt closes the bar · the simulation keeps running" at the bottom left while it is unfolded, and an arrow at its side that folds it to a small tab (the tab or Alt+Down unfolds it; Alt shows it unfolded again). While a tour runs, its button ("Timetable") opens "Timetable running": the trip's stops with their times and how each went, the next stop marked, and "Skip this stop" (or "Keep this stop") beside every stop still to come; "Cancel the timetable" asks first ("Cancel the timetable?", "Yes, cancel it" / "Keep it"). Skipping a stop is a deliberate improvement, not in OMSI 2: a stop skipped counts with no mark in the timetable evaluation and the journey log ("skipped", neither missed nor late, and not in "Stops served"). A stop skipped is skipped as soon as it is the next stop, so the IBIS moves past it and never announces it. The skip key (Ctrl+Shift+H) and the game menu's "Skip the next stop" skip the next stop as before, but it now shows as "skipped" with no mark where it showed as "missed". A skipped stop the bus serves after all (the IBIS sent back to it) counts as served.
- While a game started from the launcher runs, the launcher steps aside as the Windows 11 concept has it: on Windows its window leaves the taskbar for an icon in the system tray ("Project Depot: a game is running"), whose menu shows that game and how long it has run, and offers Show launcher (a double click on the icon, or Enter on it from the keyboard, does the same), Open log (that game's log, with the system's program for it), Stop game (that game only) and Quit launcher (the game keeps running). The icon comes back if Explorer restarts. On other systems the window is minimized instead (on Wayland, where a program cannot bring its window back, it stays open). When that game ends the window comes back by itself. A game that already ran when the launcher was opened leaves it where it is.
- Timetable is a planning page that follows the Windows 11 concept, with three tabs. Map draws the map's streets with its lines on the very lanes their trips drive, every way they run, in their colours (lines that share a road side by side; a stretch whose road the map or its timetable does not have is left out, never drawn straight from stop to stop, and the map says so), each shown or hidden with its switch or all at once, the stops with their names, zoom buttons, a search for a stop (by name, also on a map that does not say where its stops stand) or a street (by the map's street name signs) and, for the stop clicked or found, the lines that serve it with their next departures, the next departures from it on the start's date and time, "Show only these lines" and "Edit this line's trips". Schematic draws the chosen line as a diagram in either direction, scrolling when the line is long: the stops it calls at in order (not the ones it passes), the minutes between them as its trip profile has them, the other lines at each stop, the whole route's time and a table of the interchanges with those lines' next departures. Trips is the timetable editor: the line's tours in a list with new, copy and delete beside it, and the tour's trips with their departure, route, first and last stop, profile and AI group. The times shown are the timetable as saved (the map says so while a change is unsaved); a timetable that cannot be read says why on the page and in the log. Stops, streets and routes are read from the map; only departures can be changed, saved in the content folder as before (OMSI 2's own files stay as they are).
- Setup on a computer follows the Windows 11 concept. Folders has the OMSI 2 folder, the game program and the Depot content folder, each with Browse and a line under it saying what was found there: the essential files, the program beside the launcher with its version, and where mods go. The content folder can now be chosen. Left empty, it stays beside the game program. The game started by the launcher and the one started by itself both use it (`OMSI_CONTENT` still wins), and it must be a full path outside the OMSI 2 folder and any other OMSI 2 installation; a choice that cannot be used is left aside (with a line in the log) for the folder beside the game. Choosing another one moves nothing: the mods, controls, situations and screenshots of the one before stay there, as the page and Save say. The Depot's own files shows `~/.depot` with Open folder. The Content card has two tabs. What was found counts the maps, vehicles, scenery objects, splines and archives in both folders, each time the page opens. Disk usage shows a chart of rings, read inside out: folder, then type of content, then what is in it, then kind of file. Clicking a slice looks into it and clicking the middle goes back; a legend gives the size of each type. A Free up space card shows the content's size; compressing it with NTFS (LZX) is not offered yet. Revert and Save are at the foot of the page.
- Settings on a computer follows the Windows 11 concept: the sections on the left (General, Simulation, Graphics, Advanced graphics, Display and VR, Sound, Traffic and people, Driving devices, Plug-ins, Presets and files, Interface, each with how many options it shows and a dot when one is changed), the section's options in cards of a few rows on the right, a search through every option, All / Changed, how many options differ from the default and a Presets button. Each row says what the option does and when a change takes effect (the launcher's own at once, the game's at its next start), marks a change with a dot and takes it back to the default with its own button. The Graphics API comes first, in a card of its own across the page, with what it does to the other options; under "DirectX9 (D3D9Ex)" an option it limits is shown locked with its fixed value and why. Presets and files lists OMSI 2's own option presets (`option_presets/*.oop`) to apply, the graphics profiles, Reset every setting, the settings folder and this build; Plug-ins lists the OMSI 2 folder's plug-ins (not loaded in this build). The season, the cars around, the passengers, the timetable buses and putting the bus into service on start (the Maps page's temporary window, now gone) are in Simulation and in Traffic and people; a season chosen moves the start's date into it, as it did on the Drive page. A narrow window shows the sections as a dropdown and one column.
- Controls → Keyboard on a computer follows the Windows 11 concept: the actions in groups on the left (All, Driving, Cab and lights, Doors and tickets, Cameras and views, Game and menus, VR, Advanced, and Conflicts, Changed by me, Not assigned at the foot, each with its count), the group's actions in the middle with their key and second key (a click on a key waits for the new one; "changed" and "shared key" marked; a search looks through every action), and the chosen action on the right: its key and second key, its key in your OMSI 2 controls file and OMSI 2's key as it comes (its `keyboard_reset.cfg`), Back to original (to that one) and Clear; keys are named as the game's language names them (`Inputs/<lang>.kyb`). A key used twice is flagged, never refused, and only when OMSI 2's own layout does not share it so (it puts the manual and automatic gears on one key on purpose). Above them, how the keys compare with the original OMSI 2's controls file (when it was saved, when they were last reloaded), with Preview changes / Reload from OMSI 2: a window lists what would change, what is already the same and what only the Depot has (kept as they are: the original's file is read as it is), keeps a backup first (`keyboard.cfg.backup-<date-time>` beside it) and can read the game controller assignments too, device by device by their names (the Depot's other devices kept, its file backed up as well); the original's files are only read. Add a control lists the actions the game knows that have no key yet, waits for the key and warns when another action has it ("Add anyway"); a bus's own trigger is added when its whole name is typed. The phone keeps its keyboard page.
- Profile / Sessions: Profile and Sessions are one page in the sidebar, as the games open belong to the driver. The driver card, the level and the record (twelve tiles; on time / early / late counts the stops served neither early nor late as on time; four across on a 720 p window), the driver's recent drives on the right (most recent first; one is chosen, and Open details or a second click shows everything the personnel file keeps of it in a window), and the open sessions along the foot (each game with its driver and how long it runs, Open log and Stop). Sessions is no longer listed on a computer (its name still opens this page; the phone keeps its own page), and the number of games running sits on Profile / Sessions in the sidebar.
- Vehicles: "Use for the next start" takes the bus shown with its paint, depot file (Garage: automatic, or one of the bus's), fleet number and number plate for the next start from Maps; it is optional, the ALT menu in the game works as always. The card shows what the next start uses when it is that bus. Vehicle details say whether the bus's files were found (a badge; each file under the mouse) and the author's description scrolls (on a short window the details and the description scroll together, so the description always has room); the card starts over from what the next start uses each time the page opens, and Clear on the Start card drops the depot file, number and plate picked with the bus too; the separate "Files found" and "Launch preparation" cards are gone, and the paints run under the library and the preview.
- Maps is the launcher's start page and the first in the sidebar: the map library on the left, the chosen map's picture (with an Explore map button) and its scrolling description in the middle, and a Start card on the right with the date (typed, or picked on a calendar), the time (typed, or Now), the session (New, Saved, or Without bus: the game starts with the player on foot, vehicles are added in the game with the ALT menu), the start point, the weather, and traffic and passengers (season, cars around, passengers, timetable buses and putting the bus into service on start, in a window of their own until Settings is regrouped), and Start driving ("Start another game" while one runs, after a second click). With "Use the real date" or "Use the real time" on, the card shows today's date or the clock instead of a field. Each map in the library has its own picture, small, beside its name, its start points and where it comes from (an installed mod or the original OMSI 2); the pictures are read on a thread of their own, only for the rows on screen. The start point opens a window with the map's start points listed and on the map (a pin clicked chooses it), the weather a window with Natural or one chosen, and Explore map the map's radar view with its streets, stops and start points, read-only. The line and the bus are chosen inside the game, as in OMSI 2; a bus picked on Vehicles is used for the next start and can be cleared from the Start card; no bus is picked for the player otherwise (a choice kept from an older launcher drops its bus once), and a start without a bus puts the player on foot at the chosen start point. The Drive page is no longer listed on a computer, and the map's "Files found" card (its map file, depot file and start points) is gone with it: the start points are in the Start point window.
- Settings → Interface → Layout: the launcher's pages in the sidebar (the default) or in a bar along the top (`ui_nav=top`), which gives the pages more width; when the names do not all fit, the bar shows the pages' icons, the open page with its name, and an entry under the mouse opens to show its own. Background: plain dark (the default) or Scene (`ui_background=scene`): behind the pages a blurred picture, the chosen map's on Maps and Drive, the chosen bus's preview on Vehicles (the picture before stays while the next bus is drawn), the player's latest screenshot on the other pages (plain dark when there is none; read again after a game, which may have taken newer ones), darkened by the Dimming slider (`ui_dimming`, 30 to 90 %, 62 by default), with panels one sees through. Each picture is blurred once, on a thread of its own, and kept while the launcher runs.
- The launcher wears the Windows 11 look: Fluent greys (the window, a lighter sidebar, cards a layer lighter with hairline strokes), the sidebar as Windows 11's navigation view (thin outline icons drawn for Depot, the chosen page a layer lighter with a short accent bar), the page headings and card titles in the new type sizes, buttons, fields (a lighter line under them, the accent while typing), switches, sliders and segmented controls as Windows 11 draws them, and the logo's coloured part in the accent. The window has no system title bar any more: the launcher draws its own minimise, maximise and close buttons, the strip along the top moves the window (a double click maximises it) and its edges resize it. Settings → Interface → Corners chooses square corners instead (`ui_corners=square`; flatter, closer to Content Manager; the launcher only).
- Settings → Interface: the accent colour of the interface, from a short palette (Amber, the default, Orange, Red, Pink, Purple, Blue, Teal, Green), kept in `settings.cfg` as `ui_accent`. The launcher follows a change at once; every menu of the game (the Esc menu and its pages, the Alt bar and its windows, the loading screen, the city map, the HUD's menu on a triple screen or in VR) wears it from the next start. The text on the accent stays dark, the colour that reads better on every colour of the palette.
- `omsi-render-d3d9`, the isolated foundation of a future legacy Direct3D 9 renderer: a small backend interface with a GPU-less test implementation, and (Windows only) device bring-up with capability reading, off-screen clear/read-back and device-loss handling. Nothing depends on it, the game renders as before, and no new hardware is declared supported.
- `omsi-render-d3d9` draws: Direct3D 9Ex off-screen or in a window, textures, meshes, depth test and the opaque, test and blend alpha modes, with Shader Model 3.0 shaders compiled at run time. Still not used by the game.
- `omsi-render-d3d9` draws the game's scene: meshes and textures (RGBA with mip chains, DXT1/3/5) under the game's ids, and frames of instanced batches with the vanilla shading (sun, light from above, ambient, the material's colours, fog), transmaps, the alpha kinds, back-face culling, the road surfaces' depth bias and reversed depth. Still not used by the game.
- The game can draw its world with Direct3D 9Ex (`OMSI_BACKEND=d3d9`; Windows, not with VR): `omsi-render` keeps the books on wgpu's noop device and the Direct3D 9Ex device draws and presents the window. The UI, the sky and the OMSI material maps are not drawn in that mode yet. The default renderer is unchanged.
- Under "DirectX9 (D3D9Ex)" the new interface (the Esc menu, the Alt bar, the loading screen) and the HUD's pictures are drawn over the scene, as `omsi-ui` draws them; `omsi-ui`'s `Gpu` can hand its draws to a second renderer (`Gpu::set_mirror`).
- Under "DirectX9 (D3D9Ex)" the materials' maps are drawn: light maps (and the terrain's detail texture), night maps, the `[matl_envmap]` reflections with their masks and bump maps, the specular highlight and the materials' texture addressing. The lamps are not drawn yet.
- Under "DirectX9 (D3D9Ex)" the lamps are drawn: the map's lamps and the vehicles' lights (point lights and spots), and the vehicles' interior lamps on the meshes they light. The tile light maps baked into the map are not drawn yet.
- Under "DirectX9 (D3D9Ex)" the tile light maps baked into the map are drawn: at night they light the roads, the plates and the objects that take them.
- Under "DirectX9 (D3D9Ex)" the minimap and the city map are drawn again (they were missing).
- Under "DirectX9 (D3D9Ex)", a graphics card whose pixel shaders are too small for the world's shader now says so clearly in the log.
- Settings → Graphics API offers "DirectX9 (D3D9Ex)" on Windows (off by default; "Automatic" never picks it). The launcher and the game both keep the choice in `settings.cfg` as `graphics_api=d3d9`.

### Changed
- Vehicles page: the library is grouped by manufacturer (a heading with how many models it has) and then model, as the `[friendlyname]` lines of the bus files give them, so a long list is easy to scan. The details card names the model in bold with the manufacturer under it, no longer repeats the type and the folder (both stay under the mouse over the name) and no longer counts paints, depot files and fleet numbers in a table: each count sits in the corner of the list it belongs to (Paints, and Paint, Garage and Fleet number of "Use for the next start"). The author's description is the card's main part and is shown whole. The paints are tiles with a small picture of each paint, drawn ahead of time by the page's pre-render button (until a paint has its picture, its tile shows its initial). The Alt bar's Add vehicle window picks the manufacturer first and then the model among its models, as the original's lists do (a manufacturer with one model has it chosen at once).
- Vehicles: the library's manufacturers fold away and back with a click on their heading (a search shows them all), and the lists scroll. The page has a button "Detect new vehicles and pre-render images": it reads the content again for new vehicles and draws, in the background with a preview helper of its own (same interface fallback and 240 s watchdog as the live preview, never while a game runs), the pictures not kept yet, with how far it is and Cancel. The same run starts by itself once the content is read after another OMSI 2 folder is saved under Setup. The paint tiles show those pictures. The helper keeps its own mark of the bus it reads (never confused with the live preview's), is stopped when a game starts or the window is hidden and goes on afterwards, and one run draws at most 4000 pictures; the small pictures have a folder and a limit (6000) of their own and are not drawn again with every build.
- Fix: the author's description of a bus was cut at 600 characters by the catalog (and 1200 by the page); it is kept whole now (20,000 characters at most, only as a guard against a runaway file). The content index is read again once for it.
- The passengers aboard the player's bus weigh on it: each person's `.hum` `[mass]` (in tonnes; 1 t for a file without one, as a missing `[mass]` reads for a vehicle) adds to the mass of the section they are in. A full bus sits lower on its springs, gathers speed more slowly and takes longer to stop under the same brakes; its pitch and roll damping follows the heavier body. The air drag and the inertia about the centre of gravity stay those of the empty bus. The AI vehicles are unchanged.
- Steering linearity is on by default: the steering keys turn the wheel at one steady pace whatever the speed (the curvature grows by the same amount every millisecond a key is held), and it comes back at that pace. Off, the keys turn it slower the faster the bus goes, as before. A `settings.cfg` that already has `steering_linear` keeps its value; the launcher's settings page saves every switch, so anyone who has saved settings before keeps steering as they did and can turn the switch on under Driving help.
- Mods installs a .zip only, with no exception (Choose a .zip file, or drop one onto the window); the .zip is used in place from Archives. Loose folders are no longer installed by the launcher, from the page, a drop or the Mods folder: the page says to copy them by hand into the original OMSI 2 folder (the mod then works in OMSI 2 too) or into the Depot content folder (it then is not available in OMSI 2), each with an Open folder button (it says so when Setup has no OMSI 2 folder yet). A folder named like a .zip is no .zip. From a shell, `--cli mods` and `--cli poll` with `watch` likewise take only the .zip files in the Mods folder, and `--cli install` refuses a folder. A pack an older version kept aside in `Mods/waiting` (a repaint for a bus not installed then) is no longer installed from there: it stays put, to be copied by hand. The content folder's README and marker say so, and one an older version wrote is brought up to date; an archive the launcher cannot read says to copy its unpacked folder by hand. The content folder card shows the free space beside its title.
- A trip's stop times in the launcher's line lists (Maps, Drive, the Timetable page) and in `--cli lines` now follow the trip's profile as the game times it, the manual arrival and departure times included, and each stop says whether the trip calls there or passes it (`stops`); they were spread evenly over the trip before.
- Under "DirectX9 (D3D9Ex)" the graphics options follow what OMSI 2 has and what that renderer draws: the quality preset and the improvements stay hidden, and anti-aliasing, anisotropic filtering, render scale, mirrors and windy trees show as fixed lines with the reason, in the launcher and in the game's menu. Clouds show as "As OMSI 2": the weather's own clouds, and no fair-weather clouds added to a clear sky when no weather is chosen (the sky itself is not drawn by DirectX9 yet; today the clouds only set the weather's light there). One table (`api_limits`) holds the list; saved values are not rewritten, so they come back with another graphics API.
- Launcher finish: Profile shows no "Delete this driver" while there is no driver (the new driver's line takes the first row), the status dot in Diagnostics' OMSI 2 folder card sits on its line, and a page's title is cut short of the tabs beside it on a narrow window (Controls).
- The launcher's other pages wear the v5 heading as Vehicles and Maps do: "PAGE › PLACE" letter-spaced, a large title and one line on what the page is for (Profile "Your driver and their record", Settings "Set up the game" with the tab shown as its place, Controls "Set your keys and controllers" with its tabs beside the title, Sessions "Your games and who drives with you", Mods "Add buses, maps and scenery", Setup "Find the original game", Tutorials "Learn to drive the bus", Timetable "Plan a map's lines and tours", Diagnostics "Check what goes wrong", and Multiplayer where it is built), and their cards' titles are letter-spaced. The pages' bodies start under the bigger heading and still fit a 1280 x 720 window (the lists that scrolled still scroll, and the Mods page's content folder card now scrolls too instead of running past its foot); an empty Tutorials, Timetable or Multiplayer page says so in a card, as Sessions does, and the Diagnostics log card's lines no longer sit on its dividers. A phone keeps its compact headings.
- The launcher's pages follow the v5 concept closer: its colours are now the shared interface colours (`omsi_ui::theme`, so a launcher page and a game window match), card titles and page headings are letter-spaced as the concept draws them, Vehicles and Maps carry the concept's headings ("Choose and check your bus", "Choose your map and duty"), the Vehicles page shows the bus picked in its library live in its Preview card (the kept picture at once, "preview helper: running" while the helper draws it), the Maps page shows the picture in the chosen map's folder, and the foot of every page shows what runs as chips (game, OMSI 2 folder, preview helper). The Vehicles page's details and files agree on the paints (the bus's own livery and its repaints) and no note runs under its label.
- The launcher opens at once: it no longer builds the game's 3D renderer before showing its window (that compiled every scene, sky, shadow and post shader first, most of a minute on a slow driver). It draws on a plain graphics device of its own, and the bus preview is drawn by a helper process (`depot --preview-helper`). The Drive page shows the kept picture of the chosen bus ("Image from cache · drag it to see it live") and starts the helper only to draw and keep a picture it does not have yet, or when the picture is dragged; the helper ends a moment after its page is left, at once when the window is minimized, and with the launcher, which ends with `depot-launcher` if that is stopped. The launcher writes its own `launcher.log` (and the helper `preview-helper.log`) in the data folder, both listed on Diagnostics; it says how long the window took (`launcher: first picture in the window N ms after the program started`).
- The game's launcher wears the new look's sidebar (logo, the pages, Settings and Setup at its foot, the driver under them) and gains three pages: Vehicles and Maps (the library, details, paints or start points and the files found; "Drive this bus" and "Drive on this map" take the choice to the Drive page) and Diagnostics (this build and the graphics adapter, the OMSI 2 folder's check, the game's log and the games running). Every other page is the one it was. The separate `depot-launcher-next` program is gone; `launcher_shot` (an example of `omsi-launcher-ui`) still draws the pages into a picture.
- New Esc menu: the game menu's top level is drawn by `omsi-ui` over the paused picture (the renderer's overlay hook) instead of the HUD's lines. The lines, their keys and what they do are the game menu's own. It is the only Esc menu on a plain window; the triple screen and VR keep the HUD's menu.
- Every page behind the Esc menu's lines is drawn in the new look too: Options, Controls (the keyboard's bindings and the game controllers'), Vehicle options, World options, Line and tour (with the timetable of the line or tour chosen), the lists of drivers, fleet numbers, destinations, route numbers, depot files, start points and placing a vehicle, the administration, and the drop-downs. A page sits right of the Esc menu's column (its line marked, the bar saying the page) over the dimmed, paused picture: a settings window has its pages in a card and their rows (switches, sliders, steppers, selects, key bindings) in a card beside it; a list has its lines in a card, "Back" pinned under them. The keys, the clicks, the scrolling and what every row does are the HUD's as before; a click on a line of the column closes the page and does what that line does. The triple screen and VR keep the HUD's pages.
- New Alt bar: a tap of Alt (with no other key or button between) shows or hides the bar of icon buttons at the bottom right; it never pauses the game and is hidden while the Esc menu is open. Switch vehicle and repair run directly; what the game does not have yet is dim, with the reason. Alt+Down puts the keyboard on the bar (arrows, Enter or Space, Esc to leave).
- The in-game loading screen is finished after its concept: the logo stands on the photo with no black box round it (the Esc menu's too), the photo is darkened at its top and its foot as in the concept so the text reads on bright screenshots, the bar's track is see-through, the version is letter-spaced and right-aligned, "Preparing the world" sits under the bar beside the percentage, and the heading names the line when there is one. The "Loading" card has its header band, letter-spaced title, icon thumbnails and the concept's spacing; the bus shows its own name, maker, paint and description (read from its file in the background) instead of its folder name, a block is as tall as its text, long names and descriptions are cut with an ellipsis, and on a narrow or 150% screen the status block makes room for the card. The default background (no screenshots of the map) gains the concept's road. The slideshow still holds each screenshot 6 s with a 3.5 s fade.
- The Alt bar's "Add vehicle" opens a window of the new interface with what the game menu's "Place a vehicle" list offers in one place: the vehicles (manufacturer and type), the chosen one's liveries ("Random livery" first) and depot files (the map's first). "Place" puts the vehicle on the mouse until a click sets it down, as the list does; Esc or "Cancel" closes the window. It never pauses the game. The bar's "Remove vehicle" removes the vehicle driven, as the game menu's line does.
- End-of-trip report: once the bus has stood at its trip's last stop with a passenger door open, a window of the new interface opens over the game and pauses it (not in a LAN session): the trip's stops with their planned and actual times, how far off and whether on time, late, left early or missed, and the stops served, late arrivals and early departures, as the journey log counts them. "Continue", Enter, Space or Esc closes it and the game goes on as it was. The triple screen and the headset have no report.
- Everything on the Alt bar works from the bar; no button sends the player to the Esc menu any more. The bar has the original's fifteen buttons in its order, five by three: add vehicle, switch vehicle, remove vehicle, reposition, AI drives, line and tour, line and destination, clear accident, repair, wash and refuel, help marker, date and time, weather, controls, timetable evaluation and driver profile. "Skip the next stop" and "Screenshot" are no longer on the bar (their keys work as before). Reposition, date and time, weather, controls and the new "Line and tour", "Line and destination", "Wash and refuel" and "Timetable evaluation and driver profile" buttons open small windows of the new interface over the game: an entry point list; the time and date with dawn, day, dusk and night (sunrise and sunset at the map's place) and the computer's clock; the map's ready-made weather or the real weather (METAR); game controller, mouse, keyboard only and head tracking, each checked when on and dim when not available; the line, tour, trip and first stop to start a tour (while one runs the button is pressed and asks before ending it); the route number and destination of the bus driven; the wash and the pump as two switches; and the timetable evaluation and the driver profile. None of them pauses the game: driving keys still reach the bus, except while the route number field is being typed in. Esc or Cancel closes a window, and the bar waits while one is open, except for the wash and refuel menu. "Wash and refuel" is usable only while the bus stands at a petrol station or in the depot's wash yard, and is pressed while the wash or the pump runs; each switch is ticked while it runs, a second click stops it, and it switches off by itself once done or when the bus drives off. The timetable evaluation opens while a tour runs (dim, with the reason, otherwise): the trip's stops with their planned and actual times, how far off and whether on time, late, left early or missed, as the journey log has them so far, the deviation at the last stop reached and the stops served, late arrivals and early departures; it is updated at every stop, steps back to the tour's earlier trips, and closes when the tour ends. AI drives, clear accident and help marker stay dim, with the reason, until the game has them.
- Draw-order characterization tests for blended slots (#41): a window and an opaque panel behind it, drawn with and without model order, recorded as they stand. Test-only, run with `--ignored` on a graphics adapter.
- A panic inside a script block now logs the name of every block it passes through (the macros
  and the trigger or frame block that called them), then goes on as before (#47). Nothing is
  caught or hidden.
- Offscreen test of a tunnel mouth cut into a hillside, on synthetic content (#42): records that the `[terrainhole]` cutter takes the ground away on the tile's plan, up to the sky. Test-only.
- With "DirectX9 (D3D9Ex)" selected, Graphics is Vanilla and fixed: the Vanilla/Vanilla+/Enhanced choice is not offered (Settings → Graphics in the launcher, the Graphics page in the game), since this mode draws the picture as OMSI 2 does. The saved choice stays in `settings.cfg` and is back when another graphics API is picked.

### Removed
- The `--physics simple` command-line option: the player's vehicle always runs on the rigid body, except on rails, where it keeps the simple model the track needs.

### Fixed
- The game no longer drops to a few frames a second while paused in traffic (#43). With the sound turned down for the pause, every playing voice faded towards silence in ever tinier numbers that the processor works through dozens of times slower; mixing a sound block came to take longer than the block lasted, and each frame waited for it. A voice that has gone silent now is silent (it only moves on in time), and the bridge echo dies out the same way.
- Switching mouse steering off keeps the wheel where the mouse left it until a steering key is pressed (or a wheel or gamepad steers), as in OMSI 2; it sprang back to the middle within a second or two.
- Passengers stepping into the player's bus at night complain again that it is too dark when its saloon light is off (#179). How dark it is outside is now the sky's light alone: the street lamps' light maps at a lit stop kept passengers from ever finding the bus dark. Whether the saloon is lit is now the bus's `AI_Interiorlight`, which its script writes, instead of the brightest of the model's interior lights, where a driver's light alone counted as a lit saloon. A bus whose script does not write `AI_Interiorlight` counts as dark inside.
- Mouse steering stops at the window's edge, as in OMSI: at speed the most the mouse steers is the lock divided by the speed in tens of km/h. Moving the mouse on past the edge turned the wheel on up to full lock at any speed, which threw a bus into a slide and a spin in a bend at 30-50 km/h. That steering past the edge is removed (see `docs/removed/MOUSE_PAST_EDGE_REMOVED.md`).
- A pusher articulated bus no longer folds in two (into an L at the joint's limit) when the wheel is turned sharply at 30-50 km/h on uneven ground. The tyres of the section behind the joint grip sideways only as much as the road takes, and slide when the joint pushes them harder; only the steered section keeps turning on the steering's bend at once. The front section's tyres, too, let go when the rear section pushes them sideways harder than they grip, not only when the bend's own pull is too much.
- A coupled section whose `.bus` has no `[coupling_front_character]` is hitched by a free ball: no turntable and no angle limits, where the game gave it an articulated bus's turntable of 55° either way and 15° up and down. A section whose file names a type other than 0 (a turntable) stops at the file's own angles, even ones the game used to replace (an alpha of 0, a beta range that is empty); a lorry's hitch (type 0) no longer stops at its beta angles. How to check it in OMSI 2 is in issue #49.
- The AI's vehicles steer by their own `.bus` files: they turn about `[rot_pnt_long]` as the file gives it (no longer the rear axle instead when the file puts it close to the front axle; 0 for a file without one), their lock is the one `[inv_min_turnradius]` gives (no longer held to 15-55°, and none for a file without it), every axle turns towards the centre of the turn (an axle behind the turning line the other way; no 0.5 m dead zone and no 69° limit) and, at speed, they turn no tighter than 20 m/s² sideways. Where the map's way bends tighter than the vehicle's lock, at a junction, the AI still turns as far as the bend needs (up to 60°) so that it keeps to its lane (#249).
- A bus whose `[inv_min_turnradius]` and axle positions call for a steering angle beyond 60° now gets it: the game held every axle and tyre to about 60° (1.05 rad) whatever the file said, so such a bus turned wider than its own lock. Buses that steer less than 60° are unchanged.
- The player's bus reads `[momentofintertia]` in OMSI's y-up frame again: the first value about the lateral axis (pitch), the second about the vertical one (yaw) and the third about the longitudinal one (roll), as the AI's buses already read it. Since the order of the SDK's comment was adopted, the second value was taken as roll and the third as yaw, so a bus with a larger third value turned in too slowly and leaned too far in a curve. How to check it in OMSI 2 is in issue #49.
- The last ring of an articulated bus's bellows stays on the rear section when the joint is bent both ways at once (round a bend over a dip, a crest or a change of grade): its up-and-down angle (`articulation_<n>_beta`) read short by the cosine of the side-to-side one (3.3° for 4° bent 35°, more the further the bus was bent), and the ring stood several centimetres off the rear body. Both angles are now the two turns of the joint itself, in the order a puller's joint bends and its cfg turns the rings (beta, then alpha, about the ball). A joint with `[coupling_pitch_offset]` and the joint's physics are unchanged.
- The bellows of an articulated bus no longer open a gap or fold over in a curve on a grade or a crowned road: the joint's up-and-down angle (`articulation_<n>_beta`) is now measured from the rear section, so two sections lying on the same slope read 0 however far the bus is bent (it read up to 4° before, from the bend alone). The side-to-side angle (`articulation_<n>_alpha`) is measured the same way and is unchanged on level ground. The joint's physics is unchanged.
- Maps: "Now" sets the start's time to the computer's and leaves the date chosen as it is.
- Profile / Sessions: the Trips tile counts every drive of the driver, not only the 40 newest the list shows, and a drive's details window says it shows the drive as saved after it (the per-drive record), not the personnel file.
- The launcher's bus preview starts on Vulkan on Windows even when the game's graphics setting names DirectX 12 (whose shader compiler overflowed the helper's stack on an Intel UHD 630), then on the interface chosen. A helper that crashes ends at once, with no "stopped working" window; one that goes four minutes without a word before its first picture is stopped and started on the next interface; and every helper ends with the launcher, however the launcher ends, so none is left running after it closes.
- The city map's view turns smoothly (the short way round) when the compass changes how it is turned, or
  the vehicle's heading changes, instead of jumping.
- The city map's lines panel (and the "you are on" chip) is translated into every interface language through the
  catalog, not only Portuguese and English, and decimals take the comma in every language that writes them so
  (before, only Portuguese did). A test checks every text of the panel exists in all languages.
- A map package's icon path can no longer point outside the package: absolute paths, drives (`C:`),
  shares and `.`/`..`/empty parts leave the marker out (before, only `..` was refused).
- In the lines panel's timetable, two trips leaving at the same minute are told apart: picking one
  shows its own stops, not the other's.
- The lines panel no longer stalls the frame reading a big map's timetable: it is read in a background
  thread ("Loading timetable…" meanwhile), and only when the timetable is opened, not when a line is.
- The terrain view of the city map writes the heights of the ordinary contour lines too when zoomed in
  (the index lines' only from farther away), and the contour lines are drawn fainter as the view widens.
- The lines panel's timetable follows the session's date like the AI buses do: a tour that runs only in
  term time or only in school holidays (`Holidays.txt`) no longer shows up together with the other, and
  the timetable is read again when the date changes.
- Destination signs, IBIS screens and ticket printer output come back with a saved situation
  (#34): the scripts draw them only when something changes, so the variables alone brought
  them back empty. Each vehicle's `[scripttexture]` images are saved next to the situation as
  OMSI 2 does, `<situation>.osn_<vehicle>_<texture>.dds` (A8R8G8B8, the vehicle counted in the
  order the file lists them), and loaded back after the scripts' init at the file's own size.
  Situations saved by OMSI 2 itself bring their displays back as well. A missing file leaves
  the texture as the scripts draw it; a damaged one leaves it empty with a line in the log.
- A bus stopped under a canopy or a bridge measures the ground under its lift and ramp again,
  not the roof over them: `GetHeightAbovePoint` (OMSI's name, kept for compatibility, though it
  measures under the point) asks the faces the wheels stand on for the highest one at or below
  the point, where the plain height sampler gave the highest face there and the lift could not
  be lowered (#95).
- Splines that declare `[matl_alpha] 2` on a DXT3 / DXT5 texture whose alpha is 255 throughout
  (DavidM's walls and bridges, used across Krefrath) are drawn opaque and write depth again: they
  went into the blended pass without depth writes, so a wall further away showed over the wall in
  front and the road under a bridge showed through its deck. The same holds for surface objects.
- Passengers no longer board an articulated bus by its last door alone (#44): walking up while
  the doors were still shut, everybody made for the rear door with a button, and they stayed there
  once the driver opened the others. With a door open, a shut door with a button now draws nobody
  and people walk to the nearest open one; with every door shut the button door is still the one
  to wait at. (Same change as openOMSI 3bebe136, #2509, MIT.)
- A sound started by a trigger whose `[volcurve]` the script moves right after (a door or
  window clap, a door hit) keeps the loudest volume its curves gave it since the trigger fired,
  as OMSI 2 holds it, instead of falling silent the frame after it starts (#131).
- A bus coming to the player no longer plays the triggers its `{init}` fired before its sounds
  existed (the stock scripts fire `ev_engineshutdown` while setting the engine up); the
  announcements' file triggers are kept.
- A content file can no longer make the game look at another computer or another drive: names
  with a drive or stream (`C:`, `x.bmp:y`) or a NUL resolve to nothing, and a texture named by a
  full path (or a network path) is followed only when it lies inside the content folders.
- A `[mouseevent]` mesh behind another mesh of the bus (a panel, a wall, a window, the steering
  wheel) can no longer be clicked, hovered or turned with the wheel, from any camera: the ray
  through the cursor has to reach it first. Only the outside camera checked this before, and only
  against the mesh's bounding sphere. Meshes a camera does not draw (`[viewpoint]`) neither block
  nor take clicks from it.
- City map (Shift+M) terrain view polish: the contour heights are now computed on a worker thread when Terrain is turned on (the frame never waits; lines show without heights until they are ready, and a failed worker leaves them off), with `nav_surface` off the Terrain button and all relief are gone, a height label is skipped where it would overlap a district name, stop label or icon already placed, and the compass moves left of the lines panel instead of sitting under it. Behavior specified by the project from public discussion and reimplemented cleanly; not validated against real OMSI 2 content, only build and automated tests.

### Changed
- `omsi-sim`'s `vehicle.rs` is split by subject into `vehicle/` submodules (the type and its
  loading, the scripts' variables and triggers, motion, the AI's frame, trailers, pose, visuals,
  mesh properties, skinning, radio text, weather, built-in variables and the exporter's winding),
  with its tests in `vehicle/tests.rs`. The code moved unchanged; `vehicle.rs` keeps the
  `VehicleInstance` struct, its construction and update, and re-exports the public items at their
  old paths. The submodules, `articulation` and `physics_bridge` included, import what they use
  instead of `use super::*`.
- `omsi-app`'s crate root no longer gathers its modules' names with `use module::*`: it imports
  what it uses itself, and the modules that reached names through it (`crate::App`, `crate::Args`,
  `crate::start_clock`, the launcher's showroom with `use super::super::*`, …) name the module that
  defines them.
- The last `omsi-app` modules that took the crate root's names with `use super::*` (`app_events`,
  `offscreen`, `startup`, `android`, `input/` and its `mouse`) import them from where they are
  defined, and the crate root keeps only the imports and globs something still uses.
- `actions`, `app`, `player` and `humans_pax` in `omsi-app` import what they use from where it is
  defined instead of `use super::*`; the crate root drops the imports nothing uses any more.
- Nine more modules of `omsi-app` (`camera_util`, `input/keyboard`, `input/menus`, `server`,
  `situation`, `spawn`, `touch`, `weather_setup`, `weather_time`) import what they use from where it
  is defined instead of `use super::*`.
- Ten modules of `omsi-app` (`cli`, `duty_start`, `input_script`, `input/editor_input`,
  `launcher_link`, `memory`, `online`, `services`, `traffic_link`, `world_load`) import what they
  use from where it is defined instead of taking everything from the crate root with `use super::*`.
- `omsi-render` reads its `OMSI_*` variables through `omsi_cfg::flags`, where its 45 remaining
  settings and diagnostics are now declared, so `docs/ENVIRONMENT.md` lists every variable the game
  reads.
- `omsi-app`'s LAN, launcher and plugin modules read their `OMSI_*` variables through
  `omsi_cfg::flags`. What a phone's `env.txt` may set now comes from the register (the variables
  marked `env_txt`, `omsi-render`'s switches among them) instead of a list of its own, and a test
  pins that set. `OMSI_NO_PRESENCE` and `OMSI_TILE_DIST`, which nothing reads any more, are no
  longer taken from `env.txt` (the log lists them as skipped). `docs/ENVIRONMENT.md` lists the
  declared variables and is made from the register; a test fails when it is out of date.
- The rest of `omsi-app` outside LAN and the launcher (the offscreen run, the window's events,
  start-up, settings, cameras, VR, input scripts) reads its `OMSI_*` variables through
  `omsi_cfg::flags`, where 73 more are now declared; the variables the game sets for itself
  (`OMSI_SAFE_GPU`, `OMSI_BACKEND`, `OMSI_CONTENT` on phones) are named through it too. No
  behaviour change.
- `omsi-app`'s traffic, people, timetable and driver modules (`traffic`, `humans`, `schedule`,
  `navigator`, `driver`, `bus_service`, `spawn`, `player`, `on_foot`, `route_arrows`, `career`,
  `services`) read their `OMSI_*` variables through `omsi_cfg::flags`, where 44 more are now
  declared. No behaviour change.
- `omsi-app`'s world modules (`scene`, `tiles`, `lights`, `rain`, `puddles`, `ground_gap`,
  `memory`, `weather_setup`, `world_load`) read their 66 `OMSI_*` variables through
  `omsi_cfg::flags`, where they are now declared. No behaviour change.
- `omsi-cfg` gets `flags`, the register of the `OMSI_*` environment variables: each is declared
  once with the kind of value it takes, who it is for (user, path, diagnostic, test, internal),
  the module that reads it, whether a phone's `env.txt` may set it and what it does. The crates
  outside `omsi-app` and `omsi-render` (`omsi-sim`, `omsi-net`, `omsi-plugin`,
  `omsi-launcher-core`, `omsi-audio`, `omsi-map`, `omsi-script`, `omsi-geometry`, `omsi-cfg`) read
  their 33 variables through it. Tests check that no path or program can come from `env.txt`.
  The reads that went straight to the process environment now go through `omsi_cfg::env`'s
  cache, like the rest. No behaviour change.
- `omsi-app`'s `lan` feature is now off by default, so the game's build no longer carries LAN play,
  the dedicated server or `omsi-net` (as with `plugins` and `omsi-plugin`). The LAN helpers left in
  shared modules are marked as used only with `lan`, and the off twin of `ask_server` goes.
  `cargo build -p omsi-app --features lan,server,plugins` builds everything again
  (`docs/removed/MULTIPLAYER_DISABLED.md`). Multiplayer was already switched off at run time, so
  nothing changes in the game.
- CI (`quality` job): the `omsi-app` tests run with every feature, and a check fails if
  `omsi-net` or `omsi-plugin` reach the game's default build.
- The offscreen run, the game's start-up and the window's graphics set-up in `omsi-app` keep their
  LAN code behind the `lan` feature, and the dedicated server's code (`server.rs`, the server admin
  commands, the server's status page and clock) moves behind the `server` feature. Without `server`,
  `ServerCfg` is an empty type, so the server paths are compiled out; `--server` already stopped
  with a message. `lan` is still on by default. No behaviour change.
- The game's frame and menus in `omsi-app` keep their LAN code behind the `lan` feature: the chat,
  the other players' buses, walkers, name tags and map arrows, the host's clock and weather, the
  server admin list and the session's goodbye. Without `lan`, `lan_off.rs` answers "no session"
  (as `plugins_off.rs` does for plugins). `lan` is still on by default. No behaviour change.
- `omsi-app` gets the features `lan` and `server` (`server` implies `lan`). `lan` is on by default
  for now, so nothing changes yet. The launcher's server browser (the Multiplayer page, the phone's
  Online tab and saved servers, the joined server's banner and clock) is the first part behind `lan`.
  The game's own LAN code follows in the next steps, and then `lan` turns off by default.
- `omsi-launcher-core` gets a `lan` feature, off by default, for the one thing it takes from
  `omsi-net`: checking a LAN join code or address. Without it, `check_join` answers that LAN play is
  not in this build and a duty's `join:` target goes to the game unchecked (the game ignores it).
  `omsi-app` still asks for the feature until its own `lan` feature exists. No behaviour change.
- Helpers the game uses outside LAN play move out of the LAN modules of `omsi-app`: the teleport and
  the fall-through guard to `teleport.rs`, clearing the AI traffic, the vehicle's box
  (`vehicle_box`, which `lan::footprint_of` now wraps) and its indicator state to `traffic_link.rs`,
  and the `~/.depot` folder (`data_dir`) to `settings.rs`. No behaviour change.
- The seven LAN fields of `omsi-app`'s `App` (`lan`, `remotes`, `voice`, `remote_walkers`,
  `inside_remote`, `is_admin`, `remote_added`) move into one `Online` struct, `App::online`, so the
  `lan` feature can switch them off together. No behaviour change.
- `omsi-app` asks `is_lan_client()` and `is_lan_host()` instead of comparing the LAN session's
  `omsi_net::Role` in 22 places (weather, clock, menus, the editor, lists). Without a session both say
  no, as the comparisons did. This prepares the `lan` feature. No behaviour change.
- The plugin code (`plugins.rs`, `plugin_ui.rs` and the `omsi-plugin` crate) moves behind a new
  `plugins` feature of `omsi-app`, off by default, so the game no longer builds it. Plugins were already
  switched off since the first Depot release; `--features plugins` builds them again. Nothing is
  deleted, and the game behaves as before.
- `omsi-app/src/input_script.rs` held 4,900 lines, most of them the game's key, mouse and menu handling; it now
  keeps only the `OMSI_INPUT` script (330 lines). The rest moves to files named for what they do:
  `input/keyboard.rs`, `input/mouse.rs`, `input/menus.rs` and `input/editor_input.rs`, `actions.rs` (game
  actions, vehicles, saving), `weather_time.rs` (weather, METAR, the clock) and `online.rs` (LAN and voice
  each frame). Code moved as is; no behaviour change.
- `vehicle_path` and `human_path`, which check a content path that came from another machine, move from
  `omsi-net` to `omsi_content::remote_path`; `omsi-net` re-exports them, and the game's driver and walker
  figures no longer need the network crate for it. The first step of building the game without
  multiplayer and plugins. Code moved as is; no behaviour change.
- `cargo fmt` (rustfmt defaults, 100 columns) on `omsi-cfg`, `omsi-o3d`, `omsi-model`, `omsi-map`, `omsi-vehicle`,
  `omsi-scenery`, `omsi-texture`, `omsi-timetable`, `omsi-content`, `omsi-script`, `omsi-audio`, `omsi-ui`,
  `omsi-geometry`, `tools/omsi-check` and `omsi-render`; CI's new `rustfmt (formatted crates)` step checks them
  and blocks. Formatting only; no behaviour change.
- The clippy warnings of the ten format crates are fixed (manual `div_ceil` and `is_multiple_of`, index loops,
  needless conversions and borrows, complex types named, doc lists). No behaviour change.
- `omsi-render`: `lib.rs` is split into modules, with the code moved as is and no behaviour change:
  `camera.rs`, `targets.rs` and `material.rs` for the types without logic; `gpu.rs` (adapter, video
  memory, the OpenGL paths) and `lighting.rs`; `pipelines.rs` for the pipelines `Renderer::build` makes
  apart from the main pass's (shadows, prepass, ambient occlusion, fog lamps, mip maps, post passes,
  overlays, upscaling), each in a function of its own; and `renderer.rs` (the `Renderer`, its options
  and its start), `resources.rs` (meshes, textures, materials), `scene.rs`, `frame.rs`, `draw.rs`,
  `shaders.rs` and `surface.rs`. `render_inner` moves to `render.rs` with its passes (shadows, prepass and
  ambient occlusion, sky cube, probe, main pass, reflections, fog lamps, glass, post, present) as
  methods in `passes.rs`, and the tests to `tests.rs` and `tests_units.rs`: no file of the crate is
  over 3,000 lines.
- The HTML engine's interface with the game (`ApiValue`, `HtmlRenderer`, `HtmlRequest`, `PageApi`, `PointerKind`,
  `rebase_css_urls`) moves into `omsi-sim/src/htmlengine/page.rs`, re-exported where it was
  (`vehicle_api`, `htmltex`): the engine no longer reaches into the rest of `omsi-sim`, the first step of the
  `omsi-html` crate. Code moved as is; no behaviour change.
- The HTML engine's tests that need the rest of the game (the vehicle snapshot, the clock, pages loaded from
  disk) move to `omsi-sim/tests/htmlengine_game.rs`; `EngineRenderer::text_of` is public (hidden from the docs)
  for them.
- The HTML engine leaves `omsi-sim` for its own crate, `omsi-html` (`crates/omsi-html`), which depends only on
  `omsi-cfg` and `omsi-texture`; `omsi-sim` re-exports it as `omsi_sim::htmlengine`, so every path stays the same.
  Code moved as is; no behaviour change.

### Added
- `OMSI_DEBUG_AUDIO` (seconds, 10 by default) logs how the sound device's callbacks come and
  how long a block takes to mix (late callbacks, blocks over their length, waits for the voice
  list, voices alive), and which loops were started again within a second of being stopped,
  each from a random place of its clip (#35). It changes nothing it measures.
- "Muffle outside sounds" (Options and launcher, Sound; `muffle_outside` in the settings, off
  by default): from a cab, other vehicles' sounds and the bus's own outside sounds go through a
  low-pass, 450 Hz with the bus shut up to 7.2 kHz with a door or window open.
- `sound_levels <sound.cfg> lint` lists what is likely to sound wrong whatever the state (`[3d]`
  without a range, a loop without a reference value, variable names with spaces, odd
  `[viewpoint]` values) and notes the cab/outside pairs; `OMSI_DEBUG_SOUND` shows
  `Snd_OutsideVol` and the viewpoint of what is heard and warns of a `[3d]` with range 0.
- Lines panel timetables: double-clicking a line on the city map shows its departures by direction and day
  (Monday to Friday, Saturday, Sunday) and, for a chosen departure, when the bus is at each stop, also written
  on the map at the stops. The times come from the map's own timetable (`TTData`), the same figures the AI
  buses run by, and nothing is added to the package. Lines carry a kind of vehicle (bus, tram, metro, rail)
  with a tab for each, and a form of a line can have dates (eras), so only the forms in force on the session's
  date are shown. Select all / Clear, one click ticks both directions, trams, metro trains and trains have
  their own icons on the map, and the manifest can name which rail vehicles are which. Documented in
  `docs/MAP_PACKAGE.md`. Source: maintainer (Leo, `lmonteirotech`), fork lmonteirotech/openOMSI branch
  `local/lines-timetable` (commits `5327426`, `0999942`, `d287e0c`, `161fab8`), MIT. Not validated against
  real OMSI 2 content; only build and automated tests were run. One test of the train icon was corrected
  (it compared the train's triangle count with the bus's, which is lower).
  `docs/MAP_PACKAGE.md` now names `tools/mapgen` of lmonteirotech/openomsi-map-packages as the supported way
  to build packages, and `tools/synthetic-package/make_synthetic_package.py` writes a tiny invented package
  (no OpenStreetMap or OMSI 2 data) for trying the Lines panel.
- City map orientation: a compass on the city map shows where north lies and a click turns the map between
  **Map** (as the package draws it), **North** up and **Heading** up. New setting `nav_orientation` (`map`,
  `north`, `heading`; a row in the game settings) and new manifest key `north` (degrees from the map's up to real
  north, `docs/MAP_PACKAGE.md` sections 2 and 8.3). Dragging, zoom, the street under the pointer and the vehicle
  arrows follow the turn. Packages without `north` behave as before. Source: maintainer (Leo, `lmonteirotech`), fork
  lmonteirotech/openOMSI branch `local/lines-timetable` (the orientation part of `b826977` and `cfb9e86`, without
  its calibration tools and map-specific parts), MIT. Not validated against real OMSI 2 content; only build and
  automated tests were run.
- City map terrain view: a **Terrain** button (and the setting `nav_contours`) shows the contour lines of the terrain
  from a map package that carries them (new line classes 5 and 6 of `docs/MAP_PACKAGE.md` section 4), the height
  written on the index lines, and the height of the street the player is on and of the street under the pointer.
  The view shows only water, rivers, rails, the game's roads and the contour lines. New manifest keys
  `terrain_offset` and `contour` (colour). Packages without contour lines behave as before. Source: maintainer
  (Leo, `lmonteirotech`), fork lmonteirotech/openOMSI branch `local/lines-timetable` (the contour part of
  `d09111f` and `ae77b95`), MIT. Not validated against real OMSI 2 content; only build and automated tests were run.
- City map: the street under the pointer (any street the map's package names, also where the game has no road)
  in a small box, and the speed limit of the street the player is on as a road sign (and the limit of heavy
  vehicles when it is another), from a new line class of the package, "street with limits" (class 4 of
  `docs/MAP_PACKAGE.md` section 4). Packages without that class behave as before. Source: maintainer (Leo,
  `lmonteirotech`), fork lmonteirotech/openOMSI branch `local/lines-timetable` (commits `cae2286` and the limits
  part of `d09111f`), MIT. Not validated against real OMSI 2 content; only build and automated tests were run.
- Offscreen state trace (B0 of the refactoring plan): `OMSI_STATE_TRACE=<file>` makes `depot --offscreen` write,
  for every frame, the player's vehicle, the AI cars, the lights, the rail signals and the duty. A new test runs it
  on a synthetic street (`crates/omsi-app/tests/offscreen_state_trace.rs`), `scripts/offscreen-trace-diff.sh`
  compares a commit with its base, and the CI job `offscreen-traces` (informational for now) runs it on pull
  requests on the software Vulkan adapter. Test and CI only; no behaviour change.
- Round-trip test of `settings.cfg` between the game and the launcher
  (`crates/omsi-app/src/settings/round_trip.rs`): writes the file from one side and reads it
  from the other, in both directions, for the defaults, one value per key and values at the
  edges of the ranges. The differences found at the edges are listed in the test, not fixed;
  see docs/QUALITY.md. The game's test-only `Settings::to_text` now writes every field.
  Test only; no behaviour change.
- CI job `offscreen-golden` (informational for now): the offscreen golden runs on every push and pull
  request on the software Vulkan adapter (lavapipe), with `OMSI_GOLDEN_REQUIRE=1` so a missing adapter or a
  missing golden for it fails instead of skipping. Golden files are now kept per adapter in
  `crates/omsi-app/tests/golden/offscreen_synthetic/`. Test and CI only; no behaviour change.

### Fixed
- A graphics device the driver gives up in the middle of a frame (a driver reset, the card
  removed, the card out of memory) no longer closes the game: the session ends in order (or
  starts again with safer graphics) as it did when the loss was seen between two frames. wgpu
  panics instead of failing in a few places - waiting for the device, reading a buffer back,
  letting the device go at the end - and those panics are now caught and taken as the device
  lost (#48). The card's memory budget is enforced when textures and buffers are made (95 %),
  so running short is an "out of memory" the game gets over, and an "out of memory" that
  switched the multisampling off now also cuts the textures (it was forgotten in the switch).
  `OMSI_FAKE_GPU_ERROR=lost-poll` and `=oom` test both ways.
- AI buses of a depot group whose depot file exists nowhere (Mapa Transcol's garages, whose buses carry
  only `Transcol.hof`) drove with a blank destination sign. They now run with the first depot file of their
  own folder, as a plain group's buses do, and a bus with no depot file at all is logged as a warning (#58).
- The offscreen golden's synthetic root kept the box next to the map, where the game does not look for
  scenery objects, so the picture showed only ground and sky and the "box is in the picture" check failed
  on any adapter. The box now sits in `Sceneryobjects/Golden/` with its mesh in `model/` and its texture in
  `texture/`, as in OMSI 2. Test only.

## 0.2.52 - 2026-10-10

Release notes: [docs/releases/0.2.52.md](docs/releases/0.2.52.md).

### Added
- Golden test for the offscreen run (`crates/omsi-app/tests/offscreen_golden.rs`): runs `depot --offscreen` on a
  synthetic root the test writes itself and checks that the picture is repeatable, shows the scene and matches a
  stored thumbnail within a tolerance. It skips cleanly when the machine has no graphics adapter. It also records
  that the window gives the player's rail to `signal_aspects` while the offscreen run gives none. Test only; no
  behaviour change.
- Navigator map packages: the navigator and the city map can draw a surface (water, green, rivers, names),
  icons, regions, street names and bus lines supplied by an optional per-map package
  (`docs/MAP_PACKAGE.md`), also looked up in the content folder and used for offscreen pictures. New
  settings `nav_surface` and `nav_light`. Source: maintainer (Leo, `lmonteirotech`), fork
  lmonteirotech/openOMSI branch `feat/map-package` (commits `80fb1482`, `dbcb93f9`, `4c7a284d`,
  `d898c8fb`), MIT. Example packages (OpenStreetMap/ODbL data) left out pending licence review. Not
  validated against real OMSI 2 content; only build and automated tests were run.
- Outside camera (F3) gets an independent `outside_fov` setting, 20-120 vertical degrees or Auto. Auto
  preserves the 16:9 horizontal extent on ultrawide windows when no general override is set. Source:
  openOMSI `ea47e092ee6e0a2c472d2c7261ea7a7913e6ca9f` (MIT), selective backport authored by zane_thruston;
  no later upstream architecture imported. See `docs/integrations/OUTSIDE-FOV-ULTRAWIDE.md`.
- Articulated buses can opt into a pusher joint with `[coupling_pitch_offset]` in the rear section's
  `.bus`. Without the key, or with an invalid/zero offset, the baseline puller behaviour is unchanged.
  Source: openOMSI `a97595b1b254cf94abc27bb67844a8a1dfcee2ab` (MIT), authored by zane_thruston
  (contribution confirmed MIT by the maintainer). See `docs/integrations/PUSHER-ARTICULATION.md`.

### Changed
- omsi-sim: the rest of the game reads a vehicle's motion through `VehicleInstance::motion()` (speed, steering,
  accelerations, wheels: `VehicleMotion`), `controls()`, `mass_kg()` and `rigid()`/`rigid_mut()` instead of the
  `physics` and `rigid` fields, so that the physics can change how it keeps them. No behaviour change: the
  physics traces are identical bit for bit.
- omsi-sim: the rigid body (`rigid.rs`) split into a module per stage under `rigid/` (`ground`, `suspension`,
  `wheel`, `tyre`, `body`, `collide`), with its calibration values named in `rigid/params.rs`, each marked with
  where it comes from, and the A/B switches `OMSI_TYRE_SUSPENSION`, `OMSI_NO_WHEEL_SLIP` and
  `OMSI_KINEMATIC_TRAILERS` read once a process in `rigid/toggles.rs`. No behaviour change: the physics traces
  are identical bit for bit.
- omsi-sim: what the physics reads from the scripts (`M_Wheel`, the brake forces, the spring factors) and writes
  back (`Velocity`, `n_Wheel`, `A_Trans_*`, `Wheel_*`, `Axle_*`) goes through one place, `vehicle/physics_bridge.rs`,
  for the rigid body, the kinematic model, the AI and the coupled parts alike; it was written out four times. The
  physics traces gain an AI vehicle with a coupled section and a kinematic crash. No behaviour change: the
  traces are identical bit for bit.
- omsi-sim: a vehicle's `physics` and `rigid` fields become `enum Dynamics` (its rigid body, or the simple model
  renamed `KinematicModel`) next to the `VehicleMotion` the physics writes each frame; `VehicleInstance` gains
  `physics_state()`/`set_physics_state()` (the offscreen run's save and restore) and `set_wheel_travel()` (the LAN's
  copies). No behaviour change: the physics traces are identical bit for bit.
- the smaller crates (geometry, map, vehicle, model, texture, content, cfg, scenery, script, launcher-core, o3d, audio example): comments cleaned of addresses, offsets and mentions of the reference
- omsi-app: comments cleaned of addresses, offsets and mentions of the reference
- omsi-sim: comments cleaned of addresses, offsets and mentions of the reference
  executable; they describe observable behaviour only. Comment text only, no code change.
- Mixer, vehicle sound flags and player input/camera: comments cleaned of addresses and mentions of the reference
  executable; they describe observable behaviour only. Comment text only, no code change.
- `omsi-audio` sound set: comments cleaned of references to addresses of the reference executable; they
  describe observable behaviour only. Comment text only, no code change.
- Articulated buses and trailers: every section is a body of its own. Behind the player's vehicle the rear
  section of an articulated bus and a lorry's trailer run on the same wheel physics as the front, each with
  its own `.bus` mass, inertia, centre of gravity, axles, springs and brakes, held by a joint that follows
  `[coupling_front_character]` (a bus turntable keeps the sections from rolling against each other, a lorry's
  hitch does not). The AI's vehicles, LAN players and trains on rails follow as before, and so does every part
  with `OMSI_KINEMATIC_TRAILERS=1` (A/B). Source: maintainer's fork lmonteirotech/openOMSI branch
  `feat/multibody-articulation`, code authored by zane_thruston, MIT (confirmed by the maintainer); comments
  and docs cleaned of references to addresses of the reference executable. High-risk physics change, not
  validated against real articulated buses. See `docs/FORMATS.md`, `docs/MODDING.md`.

### Removed (reversible records in [docs/removed](docs/removed))
- The vehicle physics' A/B switches `OMSI_TYRE_SUSPENSION`, `OMSI_NO_WHEEL_SLIP` and `OMSI_KINEMATIC_TRAILERS`,
  with the older models they brought back for comparison: the suspension with a wheel mass, a tyre and bump
  stops (and the kerb lift across the tread only it read), every wheel gripping, and the coupled parts
  following kinematically behind the player's vehicle. What the game does by default is unchanged: the
  physics traces are identical bit for bit. `docs/removed/PHYSICS_AB_SWITCHES_REMOVED.md`.
- Wheel seating (`seat_wheels`): the drawn tyres of the rigid-body vehicle are no longer moved up or down
  onto the hub the physics has when the model's `Axle_Suspension` animation does not match the spring's
  travel. A model's 3D animation is shown as configured; the correction only ever applied to the front
  section of an articulated bus. `docs/removed/SEAT_WHEELS_REMOVED.md`.
- In-app updater (check, download, install, `update_*` settings, flags, UI, Android install flow). The
  Settings "About" section shows the version and a link to the releases page. `docs/removed/UPDATER_DISABLED.md`.
- Discord Rich Presence and its borrowed application ID, until Depot registers its own. The community
  Discord links stay. `docs/removed/DISCORD_DISABLED.md`.
- Worker "playing now" presence counter. `docs/removed/PRESENCE_WORKER.md`.
- Multiplayer, dedicated server and Lua plugins are disabled in the client (code kept, unreachable).
  `docs/removed/MULTIPLAYER_DISABLED.md`.
- RAR and 7z installation and automatic ZIP extraction: the installer takes a folder or a `.zip`, which
  is stored under `Archives/` unextracted. Drops `unrar-rs`, `sevenz-rust2`, `reedsolomon-rs` (GPL-3.0
  dependency removed). `docs/removed/RAR_7Z_ZIP_EXTRACTION_REMOVED.md`.
- Donation links and button on the website.

### Security
- ZIP64: capacity and allocation limits in the archive reader (`docs/security/ZIP64_FIX.md`).
- ZIP entry names: length/depth caps and no panic on non-ASCII case folding.
- `.x` models: no panic on non-ASCII bytes, nesting limit; terrain size limit and clamped sampling.
- Patchwork respects the station cap; outline self-crossing test no longer quadratic.
- One shared ceiling for texture sizes and list counts in content parsers (`omsi_cfg::limits`),
  mirrors and dynamic textures bounded.
- HTML/JS engine: parse and DOM depth limits, bounded arrays/strings, no deadlock on self-referencing
  arrays, bounded inline-block layout, `font-size` clamp; script textures and `STDrawRect` bounded;
  script string builtins capped.
- The terrain editor no longer overwrites an unreadable `.terrain`, and writes atomically with a `.bak`.
- Radio and METAR URLs: https only, public hosts only, ICAO code validated and encoded.
- Head tracking listens on 127.0.0.1 by default (`head_tracking_bind` to change).
- Android: `env.txt` is limited to an allow-list, no default signing password, unused permissions dropped.
- Website: Markdown sanitised with DOMPurify, Content-Security-Policy, marked and DOMPurify vendored.
- Release builds use `overflow-checks`.

### Fixed
- Vehicle physics: driving and braking pitch the body as OMSI 2 does. The extra lever of the tyres' pull
  (the wheel radius, taken at the hubs on top of the centre of gravity's height) is now the radius of the
  first axle's wheels, where it was the mean radius of all of them, and the pitch from it acts whenever the
  bus moves or its drive beats the brakes, where it waited for 0.2 m/s: a bus pulling away pitches from the
  first frame the drive overcomes the brakes, and one braking to a stop until it stands. It still stays off
  while the bus stands with its brakes holding. The tuned constant `PITCH_SPEED` is gone. New trace
  scenario T16 (a bus on smaller wheels in front than behind). The physics traces differ in 15 of the 26
  scenarios, each from the frame where the old and the new pitch lever first differ; the largest change of
  pitch is 0.2°, and the handling report is unchanged.
- Vehicle physics: the tyres take the road's grip of the frame they drive in. The grip (`road_grip` of
  `StreetCond` and the air temperature) was set after the physics' step, so a road that froze over or dried
  took hold one frame late, and an articulated bus's rear sections copied the late value. It is now set at
  the start of the rigid body's step. The physics traces differ only there: the scenario whose road freezes
  over and thaws is bit for bit the old code with the weather changed a frame earlier.
- Vehicle viewer in the launcher: the rear sections of an articulated bus no longer stand with their wheels
  above the floor. The viewer gave the bus no ground and ran its rear sections kinematically, hung level with
  their coupling while their wheels went up into the arches by the springs' sag. The viewer now puts the bus
  on a flat floor at z = 0 and on the same wheel physics as the game (the bus and each rear section a rigid
  body on its own springs) with the springs held at full air (`Axle_Springfactor` 1, new
  `VehicleInstance::springs_full`: the scripts' kneeling and levelling left the bus knelt and leaning in the
  picture), and steps it until every body has been still for 0.5 s (at most 10 s) before showing it
  (`crates/omsi-app/src/launcher/showroom.rs`).
- Vehicle physics: the rear section of an articulated bus loses its tyres' hold to a bend too fast for their
  grip, as the front section does. The test (the bend's pull against the grip of all of a body's tyres) worked
  the bend out again from the steering angle of the body's front axle; a rear section with its one axle on its
  own turning line has no angle there, so the test never saw its bend, and on ice its tyres went on passing up
  to several times their grip. It now takes the curvature the body turns on in the slice (the steering's for
  the front section, its joint's for a section behind); the constant `KAPPA_ARM_MIN`, which only the old
  reckoning used, is gone. New trace scenario B7 (an articulated bus into a bend on ice at 13 km/h): the
  physics traces differ only there, from the first frame the rear section's bend outgrows its grip.
- The game opens on the monitor the launcher is on (upstream #2025: after 0.2.2 it always opened on the primary monitor). The launcher passes a point of its window as `--monitor-at X,Y`; without it, or when no monitor holds the point, the primary monitor is used as before. Independent implementation; the start-up window code is otherwise unchanged.
- Timetable: the delay shown on the way between two stops follows where the bus is (the last stop's
  departure and the next arrival shared out by the distance covered) instead of standing still at the delay
  it left the last stop with (upstream #1898). Behaviour taken from openOMSI commit `3c75499` (MIT,
  turbo-devv), reimplemented; the existing test now expects the new value. Parity with the reference still
  to be confirmed by observation.
- Sound: an entry that is about to start playing starts at full volume. DirectSound refuses a request
  over 0 dB and keeps the buffer's last volume, so a `[volume]` over 1 that a curve or condition held at 0
  while silent started "audible" at gain 0 (hand brake, door-closing and indicator sounds; upstream #1851,
  #1852; may contribute to #1418, which still needs a reproduction with the reporter's `sound.cfg`).
  Reimplemented from the behaviour described in openOMSI commit `ea26060` (MIT, turbo-devv); regression
  tests added.
- `omsi-plugin` test `panels_clicks_and_focus_through_omsi_ui` failed on Windows CI because the reload is detected by
  modification time and the rewritten file could keep the old timestamp; the test now sets a newer one. The Lua
  feature itself is unchanged (it stays in the crate; it is only unreachable from the client).

### Project
- Identity and links point to OmsiModsBR/depot, omsimods.com.br and omsimods.com.br/discord; new logos and icons.
- LICENSE keeps the upstream copyright and adds the Project Depot line.
- CI: informational rustfmt, clippy, cargo audit and cargo deny (`docs/QUALITY.md`, `deny.toml`);
  blocking check of the committed Steam binaries' hashes; provenance notes in
  `docs/DEPENDENCY-PROVENANCE.md`.
