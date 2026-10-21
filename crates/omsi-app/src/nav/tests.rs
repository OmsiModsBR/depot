#[test]
fn centre_panel_map_click_and_drag_use_window_coordinates() {
    let mut n = Navigator::new(true, 0.85, "bottom-left");
    n.origin_x = 1920.0;
    n.panel_rect = [1930.0, 600.0, 2290.0, 890.0];
    n.panel_room = [1240.0, 610.0];
    assert!(!n.over_panel(100.0, 700.0));
    assert!(n.over_panel(2020.0, 700.0));
    n.panel_press(2020.0, 700.0);
    n.panel_move(2630.0, 405.0);
    assert_eq!(n.at, Some([0.5, 0.5]));
    n.city.open = true;
    n.city.rect = [2100.0, 100.0, 3700.0, 900.0];
    n.city.center = DVec2::new(100.0, 200.0);
    n.city.mpp = 2.0;
    assert_eq!(n.map_point(2900.0, 500.0), Some(n.city.center));
    assert_eq!(n.map_point(1000.0, 500.0), None);
}
/// A turned city map: a screen point maps back to the map through the turn, a drag moves the centre the way the
/// map is turned, and a click on the compass goes on to the next way of turning.
#[test]
fn a_turned_city_map_reads_points_and_drags_through_the_turn() {
    let mut n = Navigator::new(true, 0.85, "bottom-left");
    n.city.open = true;
    n.city.rect = [0.0, 0.0, 1000.0, 800.0];
    n.city.center = DVec2::new(100.0, 200.0);
    n.city.mpp = 2.0;
    // (a quarter turn: the map's east points up on the screen)
    n.city.phi = std::f64::consts::FRAC_PI_2;
    // 10 px above the middle of the view is 20 m east of the centre
    let p = n.map_point(500.0, 390.0).unwrap();
    assert!((p - DVec2::new(120.0, 200.0)).length() < 1e-6, "{p:?}");
    assert_eq!(n.map_point(500.0, 400.0), Some(n.city.center));
    // dragging the picture up by 10 px moves the centre 20 m to the west
    n.city.drag = Some((500.0, 400.0));
    n.map_move(500.0, 390.0);
    assert!((n.city.center - DVec2::new(80.0, 200.0)).length() < 1e-6, "{:?}", n.city.center);
    // the compass cycles map, north, heading
    n.city.buttons.push((Rect::new(10.0, 10.0, 20.0, 20.0), 6));
    for want in [1, 2, 0] {
        n.map_press(15.0, 15.0);
        assert_eq!(n.orient, want);
    }
}

/// A press and a small wobble is a click (the city map), a longer move drags the
/// navigator, and the place reads back from the setting it is saved as (#940).
#[test]
fn the_navigator_is_dragged_by_the_mouse() {
    let mut n = Navigator::new(true, 0.85, "bottom-left");
    assert!(n.at.is_none());
    n.panel_rect = [10.0, 600.0, 370.0, 890.0];
    n.panel_room = [1240.0, 610.0];
    n.panel_press(100.0, 700.0);
    assert!(!n.panel_move(103.0, 702.0));
    assert_eq!(n.panel_release(), Some(false));
    n.panel_press(100.0, 700.0);
    assert!(n.panel_move(100.0 + 610.0, 700.0 - 295.0));
    assert_eq!(n.panel_release(), Some(true));
    let at = n.at.unwrap();
    assert!((at[0] - 0.5).abs() < 1e-3 && (at[1] - 0.5).abs() < 1e-3, "{at:?}");
    let back = placed_at(&n.placement()).unwrap();
    assert!((back[0] - 0.5).abs() < 1e-3 && (back[1] - 0.5).abs() < 1e-3);
    // (dragged past the window's edge: held inside it)
    n.panel_press(100.0, 700.0);
    n.panel_move(5000.0, -5000.0);
    assert_eq!(n.at, Some([1.0, 0.0]));
    assert_eq!(placed_at("bottom-right"), None);
}

/// With the on-screen controls the navigator stands in the top middle, under the
/// information bar when that is on - and where a finger dragged it, once it has been
/// (#1138), as the mouse places it on a computer; the bar does not move a navigator in a
/// corner it does not reach (#1164).
#[test]
fn the_navigator_keeps_where_it_was_dragged_on_a_phone_and_clear_of_the_information_bar() {
    let mut n = Navigator::new(true, 0.85, "bottom-left");
    let (screen, size) = ((1280.0, 720.0), (300.0, 250.0));
    assert_eq!(n.panel_origin(screen, size, true, None), (490.0, 13.0));
    assert_eq!(n.panel_origin(screen, size, true, Some([311.0, 20.0, 898.0, 69.0])), (490.0, 76.0));
    assert_eq!(n.panel_origin(screen, size, false, Some([311.0, 20.0, 898.0, 69.0])), (13.0, 457.0));
    n.corner = "top-left".into();
    assert_eq!(n.panel_origin(screen, size, false, Some([330.0, 20.0, 950.0, 69.0])), (13.0, 13.0));
    assert_eq!(n.panel_origin(screen, size, false, Some([300.0, 20.0, 980.0, 69.0])), (13.0, 76.0));
    // a finger's drag: there, on the phone as on the computer
    n.panel_rect = [490.0, 13.0, 790.0, 263.0];
    n.panel_press(600.0, 100.0);
    assert!(n.panel_move(600.0 - 490.0, 100.0 + 457.0));
    assert_eq!(n.panel_release(), Some(true));
    assert_eq!(n.panel_origin(screen, size, true, None), (0.0, 470.0));
    assert_eq!(n.panel_origin(screen, size, false, None), (0.0, 470.0));
}

use super::draw::{rects_overlap, spaced_markers, stop_label_rect, stop_request_icon, vehicle_icon, Icon, STOP_REQUEST};
use super::geometry::{confirm_road_surfaces, road_geometry, visible_road_lanes};
use super::*;
use omsi_sim::traffic::{Lane, LaneKey, LaneKind};
use omsi_ui::{Layer, Painter};

#[test]
fn stop_request_icon_reserves_space_only_while_active() {
    let mut atlas = Atlas::new(256);
    for scale in [0.75, 1.0, 2.0] {
        // Both the next-stop row and the free-drive row leave room for the symbol.
        for height in [20.0, 46.0] {
            let row = Rect::new(11.0 * scale, 300.0 * scale, 300.0 * scale, height * scale);
            for requested in [false, true, false] {
                let mut ui = Painter::new();
                let text = stop_request_icon(&mut ui, &mut atlas, requested, row, scale);
                assert_eq!((text.x, text.y, text.h), (row.x, row.y, row.h));
                if requested {
                    assert!(!ui.verts.is_empty(), "the stop sign must render");
                    assert!(ui.verts.iter().all(|v| v.color == STOP_REQUEST.0));
                    assert!(ui.verts.iter().all(|v| v.pos[0] > text.right() && v.pos[0] <= row.right()));
                    assert!((text.w - (row.w - 38.0 * scale)).abs() < 0.01);
                } else {
                    assert!(ui.verts.is_empty());
                    assert_eq!(text.w, row.w);
                }
            }
        }
    }
}

fn straight(a: (f64, f64), b: (f64, f64)) -> Lane {
    omsi_sim::traffic::LaneBuilder::polyline(vec![DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0)], LaneKind::Street, 3.0)
}

#[test]
fn a_heading_of_90_points_east() {
    assert!(heading_vec(90.0).x > 0.99);
}

#[test]
fn projection_puts_the_look_at_point_in_the_middle() {
    let view = Mat4::look_at_rh(Vec3::new(0.0, -50.0, 80.0), Vec3::ZERO, Vec3::Z);
    let l = Layer::world(view, 0.7, [0.0, 0.0, 400.0, 300.0], [0.0; 4], 0.0, 1.0);
    let p = project(l.view_proj, [0.0, 0.0, 400.0, 300.0], Vec3::ZERO).unwrap();
    assert!((p - Vec2::new(200.0, 150.0)).length() < 0.5, "{p}");
    let ahead = project(l.view_proj, [0.0, 0.0, 400.0, 300.0], Vec3::new(0.0, 30.0, 0.0)).unwrap();
    assert!(ahead.y < 150.0, "ahead is up the picture: {ahead}");
}

#[test]
fn editor_only_lanes_do_not_become_gps_roads() {
    let visible = straight((0.0, 0.0), (100.0, 0.0));
    let mut hidden = straight((0.0, 30.0), (100.0, 30.0));
    hidden.invisible = true;
    let net = Network { lanes: vec![visible.clone(), hidden], ..Default::default() };
    let drawn = visible_road_lanes(&net);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].1.points, visible.points);
}

#[test]
fn separate_asphalt_meshes_corroborate_invisible_traffic_splines() {
    let mut on_road = straight((0.0, 0.0), (100.0, 0.0));
    on_road.invisible = true;
    let mut helper = straight((0.0, 25.0), (100.0, 25.0));
    helper.invisible = true;
    let mut bridge = straight((0.0, 0.0), (100.0, 0.0));
    bridge.points.iter_mut().for_each(|p| p.z = 8.0);
    bridge.invisible = true;
    let mut net = Network { lanes: vec![on_road, helper, bridge], ..Default::default() };
    let surface = (vec![DVec3::ZERO, DVec3::new(100.0, 0.0, 0.0)], 8.0);
    confirm_road_surfaces(&mut net, &[surface.clone()]);
    assert!(!net.lanes[0].invisible);
    assert!(net.lanes[1].invisible);
    assert!(net.lanes[2].invisible, "asphalt below a bridge must not corroborate its paths");
    assert_eq!(road_geometry(&net).len(), 1, "surface evidence must not draw a second road");
}

#[test]
fn adjacent_spline_lanes_form_carriageways_without_filling_the_median() {
    let mut lanes = Vec::new();
    for (path, offset) in [-10.0f32, -7.0, 7.0, 10.0].into_iter().enumerate() {
        let mut lane = straight((offset as f64, 0.0), (offset as f64, 100.0));
        lane.source = 1;
        lane.key = Some(LaneKey { tile: (0, 0), id: 42, path: path as u16 });
        lane.offset = offset;
        if offset < 0.0 { lane.points.reverse(); lane.reversed = true; }
        lanes.push(lane);
    }
    let net = Network { lanes, ..Default::default() };
    let roads = road_geometry(&net);
    assert_eq!(roads.len(), 2);
    assert_eq!(roads[0].width, 6.0);
    assert_eq!(roads[1].width, 6.0);
    assert_eq!(roads[0].points, vec![DVec3::new(-8.5, 0.0, 0.0), DVec3::new(-8.5, 100.0, 0.0)]);
    assert_eq!(roads[1].points[0].x, 8.5);
    // Display geometry must not replace or move the graph used by routing.
    assert_eq!(net.lanes.len(), 4);
    assert_eq!(net.lanes[0].points[0].y, 100.0);
}

#[test]
fn opposite_directions_draw_once_even_when_reverse_is_first() {
    let mut a = straight((0.0, 0.0), (0.0, 100.0));
    a.key = Some(LaneKey { tile: (0, 0), id: 42, path: 0 });
    let mut b = a.clone();
    b.points.reverse();
    b.reversed = true;
    let net = Network { lanes: vec![b, a], ..Default::default() };
    assert_eq!(visible_road_lanes(&net).len(), 1);
}

#[test]
fn junction_helpers_between_roads_remain_connected() {
    let mut lanes = vec![straight((0.0, 0.0), (0.0, 40.0)), straight((0.0, 40.0), (0.0, 50.0)), straight((0.0, 50.0), (0.0, 60.0)), straight((0.0, 60.0), (0.0, 100.0)), straight((30.0, 0.0), (30.0, 100.0))];
    lanes[0].next = vec![1]; lanes[1].next = vec![2]; lanes[2].next = vec![3];
    for i in [1, 2, 4] { lanes[i].invisible = true; }
    let mut net = Network { lanes, ..Default::default() };
    confirm_road_surfaces(&mut net, &[]);
    assert!(!net.lanes[1].invisible && !net.lanes[2].invisible);
    assert!(net.lanes[4].invisible);
    assert_eq!(road_geometry(&net).len(), 4);
}

#[test]
fn placement_gaps_are_bridged_only_where_the_graph_connects() {
    let mut lanes = vec![straight((0.0, 0.0), (0.0, 40.0)), straight((0.0, 41.0), (0.0, 80.0)), straight((2.0, 41.0), (2.0, 80.0))];
    lanes[0].next = vec![1];
    let roads = road_geometry(&Network { lanes, ..Default::default() });
    assert_eq!(roads.len(), 4);
    assert_eq!(roads[3].points, vec![DVec3::new(0.0, 40.0, 0.0), DVec3::new(0.0, 41.0, 0.0)]);
}

#[test]
fn paved_areas_without_driving_paths_do_not_draw_streets() {
    let mut net = Network::default();
    confirm_road_surfaces(&mut net, &[(vec![DVec3::ZERO, DVec3::new(100.0, 0.0, 0.0)], 20.0)]);
    assert!(road_geometry(&net).is_empty());
}

#[test]
fn stacked_paths_on_one_spline_are_not_merged() {
    let mut a = straight((0.0, 0.0), (0.0, 100.0));
    a.source = 1;
    a.key = Some(LaneKey { tile: (0, 0), id: 42, path: 0 });
    let mut b = a.clone();
    b.key.as_mut().unwrap().path = 1;
    b.points.iter_mut().for_each(|p| p.z = 8.0);
    let roads = road_geometry(&Network { lanes: vec![a, b], ..Default::default() });
    assert_eq!(roads.len(), 2);
    assert_eq!(roads[0].points[0].z, 0.0);
    assert_eq!(roads[1].points[0].z, 8.0);
}

#[test]
fn crowded_stop_markers_and_labels_do_not_overlap() {
    let p = Vec2::new(200.0, 200.0);
    let markers = spaced_markers([(0, p), (1, p + Vec2::X * 5.0), (2, p + Vec2::X * 35.0)], 20.0);
    assert_eq!(markers.iter().map(|m| m.0).collect::<Vec<_>>(), vec![0, 2]);
    let win = Rect::new(0.0, 0.0, 600.0, 400.0);
    let first = stop_label_rect(p, 150.0, 1.0, win, &[]).unwrap();
    let second = stop_label_rect(p + Vec2::X * 35.0, 150.0, 1.0, win, &[first]).unwrap();
    assert!(!rects_overlap(&first, &second));
    let edge = stop_label_rect(Vec2::new(590.0, 200.0), 150.0, 1.0, win, &[]).unwrap();
    assert!(edge.right() < 600.0);
    assert!(stop_label_rect(p, 150.0, 1.0, win, &[win]).is_none());
}

#[test]
fn a_vehicle_icon_is_whole_triangles_of_edge_body_and_windscreen() {
    let mut bus = Painter::new();
    vehicle_icon(&mut bus, Vec3::ZERO, 90.0, Color::WHITE, Icon::Road, 7.0);
    // an edge and a body of six corners (four triangles each), a windscreen of four (two)
    assert_eq!(bus.verts.len(), (4 + 4 + 2) * 3);
    // every kind is made of whole triangles, and none is empty
    for icon in [Icon::Tram, Icon::Metro, Icon::Train] {
        let mut p = Painter::new();
        vehicle_icon(&mut p, Vec3::ZERO, 0.0, Color::WHITE, icon, 7.0);
        assert!(!p.verts.is_empty() && p.verts.len() % 3 == 0, "{icon:?}");
    }
}

/// The city map of a made-up grid town with a trip on it (a route, its stops, street names),
/// drawn by the navigator into its texture and laid over a plain picture by the renderer, into
/// a PNG (`DEPOT_CITY_MAP_PNG`: the file; else it is only drawn), for looking at it next to
/// the design. Also checks the map's buttons are where its header is drawn.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_city_map_draws_over_the_picture() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(mut renderer) = pollster::block_on(omsi_render::Renderer::new_with(&instance, None, Some(wgpu::TextureFormat::Rgba8UnormSrgb), omsi_render::RenderOptions { msaa: 1, ssao: false, shadow_size: 1024, fxaa: false, render_scale: 1.0, ..Default::default() })) else {
        eprintln!("no graphics adapter: skipped");
        return;
    };
    let mut scene = renderer.new_scene();
    // streets every 150 m both ways, each block a lane of its own each way
    let mut lanes = Vec::new();
    let mut route = Vec::new();
    for k in -6..=6 {
        let c = k as f64 * 150.0;
        for j in -6..6 {
            let (a, b) = (j as f64 * 150.0, (j + 1) as f64 * 150.0);
            // (the trip: east along y = 0 to x = 600, then north)
            if k == 0 && (-4..4).contains(&j) {
                route.push(lanes.len());
            }
            lanes.push(straight((a, c), (b, c)));
            lanes.push(straight((b, c + 4.0), (a, c + 4.0)));
            if k == 4 && (0..3).contains(&j) {
                route.push(lanes.len());
            }
            lanes.push(straight((c, a), (c, b)));
            lanes.push(straight((c - 4.0, b), (c - 4.0, a)));
        }
    }
    let signs = vec![(DVec3::new(-375.0, 6.0, 0.0), 0.0, "Main Street".to_string()), (DVec3::new(606.0, 225.0, 0.0), 90.0, "Station Road".to_string())];
    let mut nav = Navigator::new(false, 0.85, "bottom-left");
    nav.set_map(crate::scene::NavigationMap { lanes, road_surfaces: Vec::new(), places: HashMap::new(), signs });
    nav.set_route("50/1", route, true, nav.global_version + (1 << 40));
    nav.toggle_map();
    let stop = |id: i64, x: f64, y: f64, name: &str, min: f64| NavStop { object_id: id, position: DVec3::new(x, y, 0.0), name: name.into(), arrival: 8.0 * 3600.0 + min * 60.0 };
    let frame = NavFrame {
        traffic: None,
        players: Vec::new(),
        bus: DVec3::new(-420.0, -2.0, 0.0),
        heading: 90.0,
        speed_kmh: 32.0,
        outside_temp: 18.0,
        inside_temp: 21.0,
        line: Some("50".into()),
        terminus: Some("Central Station".into()),
        stops: vec![stop(1, -300.0, -2.0, "Depot", 44.0), stop(2, 0.0, -2.0, "Market", 47.0), stop(3, 450.0, -2.0, "Main Street", 50.0), stop(4, 602.0, 300.0, "Central Station", 54.0)],
        delay: Some(30.0),
        passengers: Some(12),
        stop_requested: false,
        time: 8.0 * 3600.0 + 42.0 * 60.0,
        weekday: 1,
        date: 20261020,
        car_line: None,
        language: "ENG",
        screen: (1672.0, 941.0),
        ui_scale: 1.0,
        follow_window: true,
        dt: 1.0 / 60.0,
        info_rect: None,
    };
    // (a few frames: the bus is found on the route, the views settle)
    for _ in 0..4 {
        scene.overlays.clear();
        nav.frame(&renderer, &mut scene, &frame);
    }
    assert!(nav.map_open() && !scene.overlays.is_empty());
    let r = nav.city.rect;
    // the close button is in the header, at its right end
    let close = nav.city.buttons.iter().find(|b| b.1 == 3).map(|b| b.0).expect("the close button");
    assert!(close.right() <= r[2] - r[0] && close.y < 60.0, "{close:?}");
    let camera = omsi_render::Camera { position: DVec3::new(0.0, 0.0, 2.0), yaw: 0.0, pitch: 0.0, roll: 0.0, fov_deg: 60.0, near: 0.1, far: 2000.0 };
    let lighting = omsi_render::Lighting { shadows: false, ..Default::default() };
    let pixels = renderer.render_to_image(&mut scene, 1672, 941, &camera, &lighting).unwrap();
    if let Ok(path) = std::env::var("DEPOT_CITY_MAP_PNG") {
        image::save_buffer(&path, &pixels, 1672, 941, image::ExtendedColorType::Rgba8).unwrap();
    }
    // the map covers the middle of the picture: dark ground, not the sky
    let at = |x: u32, y: u32| { let i = ((y * 1672 + x) * 4) as usize; [pixels[i], pixels[i + 1], pixels[i + 2]] };
    let mid = at(((r[0] + r[2]) * 0.5) as u32, (r[3] - 40.0) as u32);
    assert!(mid.iter().all(|&c| c < 90), "{mid:?}");
}
