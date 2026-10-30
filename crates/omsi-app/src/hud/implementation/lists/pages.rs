//! The pages of the settings windows and their titles.

use super::chooser::items;
use super::dropdown::weather_name;
use super::keyboard::keyboard_pages;
use super::rows::{button, limit_or, opens, row, slider_row, switch_row};
use super::select::{preset_row, select_row, settings_file};
use super::values::cloud_index;
use crate::core::app::App;
use crate::hud::domain::lists::{ListKind, Page, TitlesCache, CLOUD_TYPES, PRECIP_KINDS};

pub(in crate::hud) fn options_pages(app: &App) -> Vec<Page> {
    let s = &app.settings;
    let file = settings_file();
    let pick = |key: &str, name: &str, desc: &str| select_row(&file, key, name, desc);
    let pct = |v: f32| format!("{:.0} %", v * 100.0);
    let cm = |v: f32| format!("{:+.0} cm", v * 100.0);
    let later = "Takes effect when the game starts the next time";
    // (the switches of the custom layout only where that layout is chosen)
    let custom_bar = s.info_bar_mode == "custom";
    let tiers = matches!(s.info_bar_mode.as_str(), "twotier" | "split");
    let game: Vec<(String, String)> = vec![
        switch_row(app, "navigator", "Navigator", "Enables/Disables the Minimap"),
        switch_row(app, "nav_ai", "AI vehicles on the map", "Shows/hides the other (AI) vehicles on the Minimap and the city map"),
        switch_row(app, "nav_follow_view", "Minimap on foot and in the free camera", "On foot the minimap shows a walker icon; in the free camera (F4) it centres on a camera icon and moves with the camera instead of staying on the bus"),
        switch_row(app, "nav_contours", "Terrain contours", "Shows the terrain's contour lines on the city map, when the map ships them (also a button on the map)"),
        switch_row(app, "nav_light", "Light map", "The map in a light look: light ground and white roads instead of the dark one"),
        switch_row(app, "nav_surface", "Map surface", "Shows the map's own surface (water, green, rivers, place names) under the roads, when the map ships one"),
        switch_row(app, "nav_arrows", "Route arrows (as in OMSI 2)", "Shows OMSI 2's route arrows over the road"),
        pick("navigator_corner", "Corner", later),
        pick("nav_view", "Minimap view", "How the minimap looks at the map: in perspective, tilted ahead of the vehicle, or from straight above (top view)"),
        pick("nav_orientation", "City map orientation", "How the city map is turned: as the map is drawn, real north up (where the map says where it lies) or the vehicle's heading up; the compass on the map changes it for the session"),
        switch_row(app, "get_up", "Ability to get up (Ctrl+Shift+G)", "Allows you to get out of the car and explore the world"),
        switch_row(app, "coll_objects", "Collisions with objects", "Enables/disables collisions with objects such as buildings, streetlights, etc."),
        switch_row(app, "coll_vehicles", "Collisions with vehicles", "Enables/Disables Collisions with Other Vehicles"),
        switch_row(app, "collision_pedestrians", "Collisions with people", "Enables/disables knocking down people"),
        switch_row(app, "timetable_win", "Timetable window (Insert)", "Displays a list of all stops (only when a tour is active)"),
        switch_row(app, "info_bar", "Information bar (Shift+Y)", "Displays information such as the time, speed, and other details along the screen"),
        pick("info_bar_mode", "Information bar layout", "Full bar shows every item; Compact bar the driving essentials and the next stop; Essentials only the speed, its limit and the delay; Custom bar the items chosen below"),
        pick("info_bar_pos", "Information bar position", "Along the top or the bottom of the screen"),
        switch_row(app, "info_bar_fps", "Frame rate in the information bar", "Adds the frame rate and its 1 % low as the bar's last item"),
        switch_row(app, "info_bar_collapsed", "Second tier starts folded away", "In the Two-tier and Split layouts the lower readouts start hidden; the arrow beside the bar shows them").filter(|_| tiers),
        switch_row(app, "info_bar_labels", "Names in the second tier", "Writes the name of each readout (Trip, Tank, ...) above its value in the Two-tier and Split layouts").filter(|_| tiers),
        switch_row(app, "info_bar_lowfuel", "Highlight a low tank", "Turns the tank meter red and marks its readout when under 15 % in the Two-tier and Split layouts").filter(|_| tiers),
        switch_row(app, "info_item_time", "Time", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_speed", "Speed and limit", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_trip", "Trip distance", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_odometer", "Odometer", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_temps", "Temperatures", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_tank", "Tank", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_pax", "Passengers", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_line", "Line and destination", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "info_item_next", "Next stop and delay", "Shown in the Custom bar layout").filter(|_| custom_bar),
        switch_row(app, "exact_fare", "Passengers pay the exact fare", "No change is given at the cash desk"),
        pick("boarding", "Boarding", "How passengers get their tickets"),
        pick("maintenance", "Maintenance", later),
        pick("ai_unsched_factor", "Road traffic factor", later),
        pick("ai_max_random", "Vehicles without a timetable", later),
        pick("ai_max_scheduled", "Vehicles with a timetable", later),
        pick("ai_priority_scheduled", "Highest line priority", later),
        pick("ai_max_parked", "Parked cars", later),
        pick("autosave_minutes", "Autosave", "How often the last situation is saved automatically while you play"),
        pick("ai_max_humans", "People", "The most people in the world at once: waiting at stops, walking or riding. Takes effect when the game starts the next time"),
    ]
        .into_iter()
        .flatten()
        .collect();
    let graphics: Vec<(String, String)> = vec![
        limit_or(app, "preset", "Quality preset", || preset_row(&file, "Quality preset", "Sets most of the graphics options at once")),
        limit_or(app, "graphics", "Graphics", || pick("graphics", "Graphics", later)),
        limit_or(app, "msaa", "Anti-aliasing", || pick("msaa", "Anti-aliasing", later)),
        limit_or(app, "render_scale", "Render scale", || pick("render_scale", "Render scale", later)),
        limit_or(app, "anisotropy", "Anisotropic", || pick("anisotropy", "Anisotropic", later)),
        // (Enhanced+ traces its shadows, occlusion and reflections: always on there)
        // (under "DirectX9 (D3D9Ex)" the `api_limits` table hides or locks these: `limit_or`)
        limit_or(app, "shadows", "Sun shadows", || switch_row(app, "shadows", "Sun shadows", "Enables/Disabled shadows").filter(|_| !app.settings.ray_tracing())),
        limit_or(app, "shadow_size", "Shadow map", || pick("shadow_size", "Shadow map", later)),
        limit_or(app, "ssao", "Ambient occlusion", || switch_row(app, "ssao", "Ambient occlusion", later).filter(|_| !app.settings.ray_tracing())),
        limit_or(app, "shadow_casters", "Shadows cast by", || pick("shadow_casters", "Shadows cast by", later)),
        limit_or(app, "detail_textures", "Detail texturing up close", || switch_row(app, "detail_textures", "Detail texturing up close", "The ground and large walls get fine grain when close")),
        limit_or(app, "night_brightness", "Night brightness", || slider_row(app, "night_brightness", "Night brightness", "Enhanced graphics: how much brighter the night is shown", &|v| if v < 0.01 { "Off".to_string() } else { format!("+{v:.2}") })),
        limit_or(app, "led_glow", "LED glow", || slider_row(app, "led_glow", "LED glow", "How strongly the dots of LED destination displays glow", &|v| format!("{}/15", v as i64))),
        limit_or(app, "led_mips", "LED mask mipmaps", || slider_row(app, "led_mips", "LED mask mipmaps", "Keep the mip chain of the LED masks (smoother from a distance).", &|v| format!("{v:.2}"))),
        switch_row(app, "reflections", "Reflection maps (paint, chrome, glass)", later).filter(|_| !app.settings.ray_tracing()),
        limit_or(app, "clouds", "Clouds", || switch_row(app, "clouds", "Clouds", later)),
        // (only "DirectX9 (D3D9Ex)" draws a sun glow)
        switch_row(app, "sun_glow", "Sun glow", "The bright glow around the sun")
            .filter(|_| app.output.backend() == crate::output::Backend::D3d9),
        limit_or(app, "windy_trees", "Windy trees", || switch_row(app, "windy_trees", "Windy trees", "The trees' leaves bend and sway in the wind and its gusts; with no wind they stand still")),
        // (DirectX9 only: the other APIs draw their own snowfall)
        (app.output.backend() == crate::output::Backend::D3d9).then(|| switch_row(app, "dx9_snow", "Snow", "Snowflakes fall in snow weather")).flatten(),
    ]
        .into_iter()
        .flatten()
        .collect();
    let mut display = vec![
        switch_row(
            app,
            "fullscreen",
            "Fullscreen",
            "Switches the window between windowed and fullscreen",
        ),
        pick("resolution", "Window size", later),
        switch_row(
            app,
            "triple_screen",
            "Triple screen",
            "Three physical screen projections; OpenXR takes priority",
        ),
    ];
    // (the rig's own settings only while it is on)
    let triple = vec![
        switch_row(
            app,
            "triple_hud_center",
            "HUD on centre screen",
            "Keep the navigator, menus and information on the centre screen",
        ),
        switch_row(
            app,
            "triple_span",
            "Span three monitors at startup",
            "Borderless across three equal monitors in one horizontal row; restart required",
        ),
        slider_row(
            app,
            "triple_width_mm",
            "Visible panel width",
            "Width of one screen without its frame",
            &|v| format!("{v:.0} mm"),
        ),
        slider_row(
            app,
            "triple_distance_mm",
            "Eye distance",
            "Eye to the centre screen",
            &|v| format!("{v:.0} mm"),
        ),
        slider_row(
            app,
            "triple_bezel_mm",
            "Frame width at each join",
            "Combined width of both adjacent frames",
            &|v| format!("{v:.0} mm"),
        ),
        slider_row(
            app,
            "triple_left_angle_deg",
            "Left screen angle",
            "Inward angle from a flat row",
            &|v| format!("{v:.0}°"),
        ),
        slider_row(
            app,
            "triple_right_angle_deg",
            "Right screen angle",
            "Inward angle from a flat row",
            &|v| format!("{v:.0}°"),
        ),
        slider_row(
            app,
            "triple_eye_height_mm",
            "Eye above screen centre",
            "Vertical eye offset",
            &|v| format!("{v:.0} mm"),
        ),
    ];
    if app.settings.triple.enabled {
        display.extend(triple);
    }
    let display: Vec<(String, String)> = display
        .into_iter()
        .chain([
            switch_row(app, "vsync", "V-sync", "Waits for the screen's refresh"),
            pick("max_fps", "Frame limit", "Frames a second at most"),
            switch_row(
                app,
                "fps",
                "Frame rate",
                "Show the frames per second in the top right corner",
            ),
            pick("view_distance", "View distance", later),
            pick("max_obj_dist", "Object distance", later),
            pick("min_obj_size", "Small objects", later),
            limit_or(app, "mirror_size", "Mirrors", || {
                pick("mirror_size", "Mirrors", later)
            }),
            pick("texture_memory", "Texture memory", later),
            switch_row(
                app,
                "texture_compression",
                "Compress textures on loading",
                later,
            ),
            (!omsi_launcher_lib::graphics_profiles().is_empty()).then(|| {
                opens(
                    "Load graphics profile",
                    "Applies a graphics profile saved in the launcher",
                    "gfxprofile",
                )
            }),
        ])
        .flatten()
        .collect();
    let sound: Vec<(String, String)> = vec![
        slider_row(app, "volume", "Volume", "Set how loud the game should be", &pct),
        slider_row(app, "vol_ai", "Traffic", "How loud the other vehicles are", &pct),
        slider_row(app, "vol_scenery", "Surroundings", "How loud the sounds of the scenery are", &pct),
        switch_row(app, "doppler", "Doppler effect", "Approaching sounds higher, receding ones lower"),
        switch_row(app, "muffle_outside", "Muffle outside sounds", "From the cab, the sounds of outside the bus are dulled, less so with a door or window open"),
        pick("pax_voices", "Passenger voices", "What passengers say"),
    ]
        .into_iter()
        .flatten()
        .collect();
    let mut camera: Vec<(String, String)> = vec![
        switch_row(app, "head", "Head movement", "The view moves with the vehicle's acceleration"),
        switch_row(app, "cam_smooth", "Smooth viewpoint changes", "Enables a smooth transition between camera perspectives"),
        switch_row(app, "camcoll", "Camera collisions", "The outside camera cannot pass through objects"),
        switch_row(app, "steer_look", "View turns with steering", "Camera turns with the steering wheel (cockpit only)"),
        slider_row(app, "steer_look_angle", "Steering view angle", "How far the view turns at full steering lock", &|v| format!("{v:.0}°")),
        slider_row(app, "steer_look_response", "Steering view response", "How quickly the view follows the steering", &|v| format!("{:.0} ms", v * 1000.0)),
        slider_row(app, "head_idle", "Head sway at a standstill", "How much the view sways on its own when nothing is done to it - a head at rest is never quite still, most of it seen while the bus waits at a stop", &|v| if v <= 0.0 { "Off".to_string() } else { format!("{:.0}%", v * 100.0) }),
        slider_row(app, "head_idle_pace", "Sway pace", "How fast that sway moves (100% is the pace it is designed at)", &|v| format!("{:.0}%", v * 100.0)),
        switch_row(app, "hands_in_cab", "Driver's hands in the cab view", "Shows the driver's hand on the steering wheel (Cockpit only)"),
        switch_row(app, "driver", "Driver at the wheel (outside views)", "Shows the driver in the outside views and in the mirrors"),
        switch_row(app, "headtrack", "Head tracking (native TrackIR / OpenTrack)", &format!("Use native TrackIR on Windows, or OpenTrack on UDP port {}", s.head_tracking_port)),
        slider_row(app, "head_tracking_yaw_sens", "TrackIR yaw sensitivity", "Left / right head movement. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        slider_row(app, "head_tracking_pitch_sens", "TrackIR pitch sensitivity", "Up / down head movement. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        slider_row(app, "head_tracking_roll_sens", "TrackIR roll sensitivity", "Head tilt. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        slider_row(app, "head_tracking_x_sens", "TrackIR X sensitivity", "Move your head left / right. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        slider_row(app, "head_tracking_y_sens", "TrackIR Y sensitivity", "Move your head up / down. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        slider_row(app, "head_tracking_z_sens", "TrackIR Z sensitivity", "Move your head forward / back. 100% is 1:1; the low end is deliberately finer.", &|v| format!("{v:.0}%")),
        switch_row(app, "head_tracking_invert_yaw", "Invert TrackIR yaw", "Reverse left / right head rotation"),
        switch_row(app, "head_tracking_invert_pitch", "Invert TrackIR pitch", "Reverse up / down head rotation"),
        switch_row(app, "head_tracking_invert_roll", "Invert TrackIR roll", "Reverse head tilt"),
        switch_row(app, "head_tracking_invert_x", "Invert TrackIR X", "Reverse left / right head movement"),
        switch_row(app, "head_tracking_invert_y", "Invert TrackIR Y", "Reverse up / down head movement"),
        switch_row(app, "head_tracking_invert_z", "Invert TrackIR Z", "Reverse forward / back head movement"),
        slider_row(app, "look_sens", "Mouse look sensitivity", "How fast the view turns when looking round with the mouse (100% is OMSI's)", &pct),
        slider_row(app, "look_smoothing_ms", "Smooth the mouse look", "How long the view takes to come round to where the mouse or the stick turned it (off: at once, as OMSI)", &|v| if v <= 0.0 { "Off".to_string() } else { format!("{v:.0} ms") }),
        switch_row(app, "alt_view", "Right mouse button turns the view", "Shift+right zooms; off: right zooms as in OMSI, the wheel button turns"),
        switch_row(app, "precision_zoom", "Precision mouse zoom", "The mouse zoom follows the FOV curve instead of OMSI's linear way"),
        slider_row(app, "fov", "Field of view", "Vertical field of view; in triple screen Default uses physical measurements, an override moves the virtual eye", &|v| if v < 20.0 { "Default".to_string() } else { format!("{v:.0}°") }),
        slider_row(app, "outside_fov", "Outside view field of view (F3)", "Vertical field of view of the outside camera; Auto fits the shape of your monitor (ultrawide keeps the sides undistorted)", &|v| if v < 20.0 { "Auto".to_string() } else { format!("{v:.0}°") }),
        slider_row(app, "seat 1", "Seat forward and back", "Adjust the driver's seat position forward or backward", &cm),
        slider_row(app, "seat 2", "Seat height", "Adjust the driver's seat height", &cm),
        slider_row(app, "seat 0", "Seat left and right", "Adjust the driver's seat position from side to side", &cm),
        slider_row(app, "seat_pitch", "Head pitch", "Set the driver's neutral head tilt up or down, independent of the display setup", &|v| format!("{v:+.0}°")),
    ]
        .into_iter()
        .flatten()
        .collect();
    camera.push(button(
        "Reset the seat position",
        "Reset",
        "Put the seat back where the vehicle has it.",
        "seat_reset",
    ));
    if cfg!(windows) {
        camera.extend(
            vec![
                switch_row(app, "vr", "Use OpenXR headset", later),
                if s.vr {
                    pick("vr_scale", "Eye resolution", later)
                } else {
                    None
                },
                if s.vr {
                    pick("vr_head_smoothing_ms", "Head tracking smoothing", later)
                } else {
                    None
                },
                if s.vr {
                    pick("vr_mirror_rate", "Bus mirror refresh", later)
                } else {
                    None
                },
                if s.vr {
                    switch_row(
                        app,
                        "vr_desktop_mirror",
                        "Show headset picture on monitor",
                        later,
                    )
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten(),
        );
    }
    let controls: Vec<(String, String)> = vec![
        pick("drive_keys", "Driving keys", "Which keys drive the vehicle"),
        switch_row(app, "mouse", "Steering with the mouse", "Steer and control the pedals using the mouse"),
        switch_row(app, "mouse_right", "A right click ends the mouse steering", "As in OMSI; off: the right button only looks round"),
        slider_row(app, "mouse_sens", "Mouse steering sensitivity", "Adjust how much the steering wheel turns based on mouse movement", &pct),
        switch_row(app, "mouse_smooth", "Smooth mouse steering", "The wheel eases after the cursor; off: it follows at once, as in OMSI"),
        switch_row(app, "steering_linear", "Steering linearity (keys at OMSI's steady pace)", "Keyboard steering at OMSI's steady pace"),
        switch_row(app, "old_steering", "Old Steering (the wheel stays, turn it back yourself)", "The wheel stays where the keys left it"),
        switch_row(app, "red_steer_spd", "Dynamic steering (slower keys at speed, OMSI's redSteerSpd)", "The steering keys act slower at speed"),
        switch_row(app, "ff", "Force feedback and vibration", "Enable force feedback for the steering wheel and vibration for controllers"),
        switch_row(app, "ff_invert", "Invert force feedback by default", "For wheels without a saved direction"),
        slider_row(app, "wheel_range", "Wheel rotation", "The steering wheel's own rotation, lock to lock", &|v| format!("{v:.0}°")),
        slider_row(app, "wheel_lock", "Full lock at", "How far the wheel turns for the vehicle's full lock", &|v| if v < 45.0 { "OMSI".to_string() } else { format!("{v:.0}°") }),
        slider_row(app, "pedal_t", "Throttle pedal strength", "Adjust how strongly pedal input affects the throttle", &|v| format!("x{v}")),
        slider_row(app, "pedal_b", "Brake pedal strength", "Adjust how strongly pedal input affects the brake", &|v| format!("x{v}")),
        switch_row(app, "blinker_cancel", "Indicators cancel themselves", "The bus's script turns the indicator off after a turn; off: it stays on until you turn it off"),
        switch_row(app, "brake_hold", "Keyboard brake stays on", "Keep the brake applied until the throttle is pressed"),
        switch_row(app, "auto_clutch", "Automatic clutch", "Automatically operate the clutch for you"),
        switch_row(app, "momentary_gears", "H-pattern shifter: return to neutral when the gear is released", "For a manual shifter without a neutral button; releasing a gear selects neutral immediately"),
        switch_row(app, "auto_shift", "Automated manual gearbox", "Shift a manual gearbox's gears for you by the engine speed"),
    ]
        .into_iter()
        .flatten()
        .collect();
    let interface: Vec<(String, String)> = vec![
        pick("language", "Language", "The language of the game's interface"),
        slider_row(app, "ui_scale", "Game interface size", "The size of the texts, the menu, the timetable and the navigator", &pct),
        switch_row(app, "ui_scale_window", "Interface grows with the window", "On a window taller than 1080p the interface grows with it"),
        slider_row(app, "ui_opacity", "Interface opacity", "How much of the interface's backgrounds shows", &pct),
        switch_row(app, "tooltips", "Name of the button under the mouse", "Shows the name of what the cursor points at"),
        switch_row(app, "notes", "Notes in the top-left corner", "Why the vehicle does not move, the change due, what a service did"),
        switch_row(app, "chat", "Chat in online games", "Shows the chat of a LAN session"),
        slider_row(app, "chat_size", "Chat size", "The chat's texts on top of the interface size (also Ctrl + the mouse wheel over the chat)", &pct),
        switch_row(app, "name_tags", "Other players' names above their buses", "Shows the names of the other players"),
        Some(opens("Reset all settings...", "Everything but the language, the key bindings and the game folder goes back to how it came", "reset")),
    ]
        .into_iter()
        .flatten()
        .collect();
    let mut pages = vec![
        ("Gameplay", game),
        ("Graphics", graphics),
        ("Display and memory", display),
        ("Sound", sound),
        ("Camera", camera),
        ("Controls", controls),
        ("Interface", interface),
    ];
    if app.vr_active() && app.player.is_some() {
        let desc = "Navigator position (this bus)";
        let mut rows = vec![
            switch_row(app, "navigator", "Navigator", desc).unwrap(),
            button(
                "Move and rotate with the mouse...",
                "Open",
                desc,
                "vr_nav_edit",
            ),
        ];
        for (id, label) in [
            ("x", "Position right / left"),
            ("y", "Position forward / back"),
            ("z", "Position up / down"),
            ("width", "Display width"),
        ] {
            rows.extend(slider_row(app, &format!("vr_nav_{id}"), label, desc, &cm));
        }
        for (id, label) in [
            ("yaw", "Display rotation"),
            ("tilt", "Display tilt"),
            ("roll", "Display roll"),
        ] {
            rows.extend(slider_row(
                app,
                &format!("vr_nav_{id}"),
                label,
                desc,
                &|v| format!("{v:.0}°"),
            ));
        }
        rows.extend(slider_row(
            app,
            "vr_nav_opacity",
            "Interface opacity",
            desc,
            &pct,
        ));
        rows.push(button(
            "Reset navigator position",
            "Reset",
            desc,
            "vr_nav_reset",
        ));
        pages.push(("VR", rows));
    }
    pages
}

pub(in crate::hud) fn vehicle_pages(app: &App) -> Vec<Page> {
    let has = app.player.is_some();
    let server = crate::input::on_server(&app.args);
    let mut display: Vec<(String, String)> = Vec::new();
    if has {
        display.push(opens(
            "Destination display",
            "Change the current destination",
            "dest",
        ));
        display.push(opens(
            "Depot file (HOF)",
            "Change the current depot file (used for the timetable)",
            "hof",
        ));
        display.push(opens(
            "Fleet number",
            "Change the vehicle's current fleet number",
            "number",
        ));
    }
    if !server {
        display.push(opens(
            "Driver",
            "Change the current driver profile",
            "driver",
        ));
    }
    let mut fleet: Vec<(String, String)> = Vec::new();
    if has || !app.placed.is_empty() {
        fleet.push(button(
            "Drive the next vehicle",
            "Switch",
            "Take the wheel of another vehicle standing in the world",
            "switch",
        ));
    }
    fleet.push(opens(
        "Place a vehicle",
        "Place a vehicle of your choice",
        "place",
    ));
    if has {
        fleet.push(button(
            "Couple",
            "Couple",
            "Couple the vehicle to the one in front of or behind it",
            "couple",
        ));
        fleet.push(button(
            "Uncouple",
            "Uncouple",
            "Separate the coupled vehicles",
            "uncouple",
        ));
        if app.on_foot.is_none() {
            fleet.push(button(
                "Get up and out",
                "Get out",
                "Step out of your car and explore the world",
                "getout",
            ));
        }
        fleet.push(button(
            "Remove this vehicle",
            "Remove",
            "Removes the current vehicle",
            "remove",
        ));
        // (#728: another bus in this one's place, or this one again with its files read
        // anew - a script or a .bus changed - without starting the game again)
        fleet.push(button(
            "Swap for another vehicle",
            "Swap",
            "Put another vehicle in this one's place and drive it",
            "swap",
        ));
        fleet.push(button("Reload this vehicle", "Reload", "Read the vehicle's files again (.bus, model and sound configuration, scripts) and drive it from here", "reload"));
    }
    if !app.placed.is_empty() {
        fleet.push(button(
            "Remove the placed vehicles",
            "Remove",
            "Removes all vehicles you've placed from the world",
            "clearplaced",
        ));
    }
    let mut service: Vec<(String, String)> = Vec::new();
    if has {
        service.push(button(
            "Refuel",
            "Refuel",
            "Fills the tank of the current vehicle",
            "refuel",
        ));
        service.push(button("Wash", "Wash", "Cleans the current vehicle", "wash"));
        service.push(button(
            "Repair",
            "Repair",
            "Repairs the current vehicle",
            "repair",
        ));
        service.push(button(
            "Put back on its wheels",
            "Reset",
            "Return the vehicle to an upright position",
            "reset",
        ));
        if !server && app.navigator.is_some() {
            service.push(button(
                "Move on the map",
                "Pick",
                "Teleports you to any location on the map",
                "teleport",
            ));
            service.push(opens(
                "Teleport to a start point",
                "Teleport to a starting point on the map",
                "tplist",
            ));
        }
    }
    vec![
        ("Display and driver", display),
        ("Vehicles", fleet),
        ("Service", service),
    ]
}

pub(in crate::hud) fn world_pages(app: &App) -> Vec<Page> {
    let client = app.is_lan_client();
    let pct = |v: f32| format!("{:.0} %", v * 100.0);
    let mut time: Vec<(String, String)> = Vec::new();
    let mut weather: Vec<(String, String)> = Vec::new();
    let mut climate: Vec<(String, String)> = Vec::new();
    let mut tools: Vec<(String, String)> = Vec::new();
    if !client {
        let t = app.clock.time;
        let now = format!(
            "{:02}:{:02}",
            ((t / 3600.0) as i64).rem_euclid(24),
            ((t / 60.0) as i64) % 60
        );
        time.extend(switch_row(
            app,
            "time_sync",
            "Real-time sync",
            "The game follows your device's date and time",
        ));
        if app.real_time_locked() {
            let (d, m) = app.clock.day_month();
            let text = format!(
                "{:04}-{m:02}-{d:02}  {}:{:02}",
                app.clock.year,
                now,
                (t as i64) % 60
            );
            time.push((
                row(
                    "Date and time",
                    'i',
                    &text,
                    "Synchronized with the real time",
                    None,
                ),
                "noop".to_string(),
            ));
        } else {
            // the exact time: typed as hours, minutes and seconds
            match app.menu_edit.as_ref() {
                Some(d) => {
                    let mut c: Vec<char> = d.chars().collect();
                    c.resize(6, '_');
                    let typed = format!("{}{}:{}{}:{}{}", c[0], c[1], c[2], c[3], c[4], c[5]);
                    time.push((
                        row(
                            "Exact time",
                            'E',
                            &typed,
                            "Press Enter to change, Esc to cancel",
                            None,
                        ),
                        "time_edit".to_string(),
                    ));
                }
                None => {
                    let secs = format!("{}:{:02}", now, (t as i64) % 60);
                    time.push((
                        row(
                            "Exact time",
                            'e',
                            &secs,
                            "Change the current time (Press Enter to change)",
                            None,
                        ),
                        "time_edit".to_string(),
                    ));
                }
            }
            time.extend(slider_row(
                app,
                "hour",
                "Hour",
                "Set the hour of the day directly",
                &|v| format!("{:02}", v as i64),
            ));
            time.extend(slider_row(
                app,
                "minute",
                "Minute",
                "Set the minute directly",
                &|v| format!("{:02}", v as i64),
            ));
            for (name, hm, secs) in [
                ("Morning", "06:00", 6 * 3600),
                ("Noon", "12:00", 12 * 3600),
                ("Evening", "18:00", 18 * 3600),
                ("Night", "23:00", 23 * 3600),
            ] {
                time.push(button(
                    name,
                    hm,
                    "Jump to this time of day.",
                    &format!("clock_set {secs}"),
                ));
            }
            if let (Some(_), Some(p)) = (app.duty.as_ref(), app.player.as_ref()) {
                let d = p.vehicle.host.tt_delay as f64;
                if d.abs() >= 1.0 {
                    let text = format!(
                        "{}{}:{:02}",
                        if d < 0.0 { "−" } else { "+" },
                        (d.abs() / 60.0) as i64,
                        d.abs() as i64 % 60
                    );
                    time.push(button(
                        "On time with the timetable",
                        &text,
                        "Move the clock so that the vehicle is on time",
                        "clock_ontime",
                    ));
                }
            }
            if !app.in_lan_session() {
                time.extend(slider_row(
                    app,
                    "speed",
                    "Time speed",
                    "How fast the world's clock runs",
                    &|v| format!("x{v}"),
                ));
            }
        }
        weather.extend(switch_row(
            app,
            "metar_sync",
            "METAR sync",
            "The weather follows the real METAR report",
        ));
        let src = if app.settings.metar_station.is_empty() {
            format!("{} ({})", app.metar_station(), omsi_ui::tr("automatic"))
        } else {
            app.metar_station()
        };
        weather.push((
            row(
                "METAR source",
                'o',
                &src,
                "The airport used for real weather.",
                None,
            ),
            "metar_src".to_string(),
        ));
        let typed = if app.menu_edit_icao {
            let mut s = app.menu_edit.clone().unwrap_or_default();
            while s.len() < 4 {
                s.push('_');
            }
            omsi_ui::tr("%{text}  (typing)").replace("%{text}", &s)
        } else {
            app.metar_station()
        };
        weather.push((
            row(
                "ICAO",
                if app.menu_edit_icao { 'E' } else { 'e' },
                &typed,
                "Enter any 4-letter ICAO station.",
                None,
            ),
            "metar_icao_edit".to_string(),
        ));
        if app.metar_locked() {
            weather.push(button(
                "METAR report",
                "Refresh now",
                "Fetch the selected station again without waiting for the next automatic update.",
                "metar_refresh",
            ));
        } else {
            weather.push(button(
                "METAR report",
                "Load once",
                "Load the selected station once without enabling continuous METAR sync.",
                "metar_once",
            ));
        }
        weather.push((
            row(
                "Preset",
                'o',
                &weather_name(app),
                "A ready-made weather",
                None,
            ),
            "weather".to_string(),
        ));
        if !app.metar_locked() {
            weather.push(button(
                "Custom weather",
                "Edit current",
                "Freeze the weather currently in force and edit it as a custom weather.",
                "weather_custom",
            ));
        }
        let cloud = app
            .weather
            .as_ref()
            .and_then(|w| cloud_index(&w.clouds.0))
            .map(|i| CLOUD_TYPES[i].1.to_string())
            .or_else(|| app.weather.as_ref().map(|w| w.clouds.0.trim().to_string()))
            .unwrap_or_default();
        weather.push((
            row(
                "Clouds",
                'o',
                &cloud,
                "The kind of clouds in the sky.",
                None,
            ),
            "cloudkind".to_string(),
        ));
        weather.extend(slider_row(
            app,
            "visibility",
            "Visibility",
            "How far one can see; less is fog.",
            &|v| {
                if v >= 1000.0 {
                    format!("{:.1} km", v / 1000.0)
                } else {
                    format!("{} m", v as i64)
                }
            },
        ));
        weather.extend(slider_row(
            app,
            "brightness",
            "Brightness",
            "Brightness of the custom weather lighting.",
            &|v| format!("{:.0} %", v * 100.0),
        ));
        let kind = app
            .weather
            .as_ref()
            .map(|w| {
                (w.precip.first().copied().unwrap_or(0.0).max(0.0) as usize)
                    .min(PRECIP_KINDS.len() - 1)
            })
            .unwrap_or(0);
        weather.push((
            row(
                "Precipitation",
                'o',
                PRECIP_KINDS[kind],
                "Rain or snow.",
                None,
            ),
            "precipkind".to_string(),
        ));
        weather.extend(slider_row(
            app,
            "rain_amt",
            "Precipitation strength",
            "How hard it rains or snows.",
            &pct,
        ));
        weather.extend(slider_row(
            app,
            "wet",
            "Wet roads",
            "How wet the roads are now (they dry in the sun, wet in the rain).",
            &pct,
        ));
        weather.extend(switch_row(
            app,
            "snow_cover",
            "Snow cover",
            "Snow lying on the world and ground.",
        ));
        weather.extend(switch_row(
            app,
            "snow_road",
            "Snow on road",
            "Treat the road surface as snow-covered.",
        ));
        climate.extend(slider_row(
            app,
            "temp",
            "Temperature",
            "The air temperature.",
            &|v| format!("{} °C", v as i64),
        ));
        let dew_temp = app.weather.as_ref().map(|w| w.temp.0).unwrap_or(15.0);
        climate.extend(slider_row(
            app,
            "humidity",
            "Humidity",
            "Relative humidity of the air.",
            &|v| {
                omsi_ui::tr("%{humidity} % · dew %{dew} °C")
                    .replace("%{humidity}", &format!("{:.0}", v))
                    .replace(
                        "%{dew}",
                        &format!("{:.0}", crate::weather_setup::dew_point_c(dew_temp, v)),
                    )
            },
        ));
        climate.extend(slider_row(
            app,
            "wind_speed",
            "Wind speed",
            "How fast the wind blows; it drives the clouds.",
            &|v| format!("{} m/s", v as i64),
        ));
        climate.extend(slider_row(
            app,
            "wind_dir",
            "Wind direction",
            "The direction of the wind in degrees (0 is north).",
            &|v| format!("{}°", v as i64),
        ));
        // the METAR sync on: only its own rows stay (the weather is the report's)
        if app.metar_locked() {
            weather.retain(|r| {
                matches!(
                    r.1.as_str(),
                    "metar_sync" | "metar_src" | "metar_icao_edit" | "metar_refresh"
                )
            });
            climate.clear();
        }
        tools.push(button(
            "Object editor",
            "Open",
            "Place and move objects in the world.",
            "editor",
        ));
    }
    let mut people: Vec<(String, String)> = Vec::new();
    people.extend(slider_row(
        app,
        "traffic",
        "Traffic",
        "How many vehicles drive around the map.",
        &|v| format!("{} vehicles", v as i64),
    ));
    if !client && app.traffic.is_some() {
        people.push(button(
            "Clear AI traffic",
            "Clear",
            "Remove the current AI cars from the road; random traffic will return automatically.",
            "traffic_clear",
        ));
    }
    people.extend(slider_row(
        app,
        "pax",
        "Passengers",
        "How many passengers wait at the stops and ride.",
        &pct,
    ));
    vec![
        ("Time", time),
        ("Weather", weather),
        ("Temperature and wind", climate),
        ("Traffic and people", people),
        ("Tools", tools),
    ]
}

/// The pages of the settings window `kind` (empty ones left out) and the one shown.
pub(in crate::hud) fn pages_of(app: &App, kind: &ListKind) -> Option<(Vec<Page>, usize)> {
    let (pages, tab) = match kind {
        ListKind::Options(t) => (options_pages(app), *t),
        ListKind::Vehicle(t) => (vehicle_pages(app), *t),
        ListKind::World(t) => (world_pages(app), *t),
        ListKind::Controls => (vec![("Controls", items(app, kind))], 0),
        ListKind::Keyboard(tab) => (keyboard_pages(app), *tab),
        ListKind::ControllerDevices(_)
        | ListKind::Controller(..)
        | ListKind::ControllerAxis(..)
        | ListKind::ControllerButtonSettings(..) => crate::game_controller_menu::pages(app, kind)?,
        _ => return None,
    };
    let pages: Vec<Page> = pages.into_iter().filter(|p| !p.1.is_empty()).collect();
    let tab = tab.min(pages.len().saturating_sub(1));
    Some((pages, tab))
}

/// Which tab of the Options window is the one titled `title` (the first if none is).
pub(crate) fn options_tab(app: &App, title: &str) -> usize {
    pages_of(app, &ListKind::Options(0))
        .and_then(|(pages, _)| pages.iter().position(|p| p.0 == title))
        .unwrap_or(0)
}

thread_local! {
    static TITLES: std::cell::RefCell<TitlesCache> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn forget_page_titles() {
    TITLES.with(|c| *c.borrow_mut() = None);
}

/// The titles of the pages of an open settings window and the one shown.
///
/// Asked every frame while a window is open, and building the pages is the work of
/// all their rows: the answer is kept for a moment.
pub(crate) fn page_titles(app: &App, kind: &ListKind) -> Option<(Vec<String>, usize)> {
    let vr_nav_available = app.vr_active() && app.player.is_some();
    if let Some(hit) = TITLES.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(k, vr, t, _)| {
                k == kind && *vr == vr_nav_available && t.elapsed().as_millis() < 5000
            })
            .map(|(_, _, _, r)| r.clone())
    }) {
        return Some(hit);
    }
    let (pages, tab) = pages_of(app, kind)?;
    let r = (
        pages.iter().map(|p| p.0.to_string()).collect::<Vec<_>>(),
        tab,
    );
    TITLES.with(|c| {
        *c.borrow_mut() = Some((
            kind.clone(),
            vr_nav_available,
            std::time::Instant::now(),
            r.clone(),
        ))
    });
    Some(r)
}
