//! `helper\stars.dat` and the star map the sky shaders read.

use omsi_content::stars::{face_of, star_size, StarCatalog, StarClass, StarMap};

const LINES: &str = "101.29\t-16.72\t3.837072455\tA\r\n\
                     95.99\t-52.7\t1.940885878\tF\r\n\
                     \r\n\
                     279.23\t38.78\t0.972747224\tA\r\n\
                     358.89\t57.41\t0.003981072\tB\r\n\
                     10.0\t20.0\t0.5\tC\r\n";

#[test]
fn reads_each_star_and_passes_over_blank_lines() {
    let c = StarCatalog::parse(LINES);
    assert_eq!(c.stars.len(), 5);
    assert!(c.skipped.is_empty());
    let sirius = c.stars[0];
    assert_eq!((sirius.ra, sirius.dec), (101.29, -16.72));
    assert!((sirius.flux - 3.837).abs() < 1e-3);
    assert_eq!(sirius.class, StarClass::A);
    // a carbon star is drawn as the red giant it is
    assert_eq!(c.stars[4].class, StarClass::Other);
    assert_eq!(StarClass::Other.rgb(), StarClass::M.rgb());
}

#[test]
fn a_line_that_holds_no_star_is_left_out_and_named() {
    let c = StarCatalog::parse(
        "10\t20\t0.5\tG\n\
         ten\t20\t0.5\tG\n\
         10\t95\t0.5\tG\n\
         10\t20\t-1\tG\n\
         10\t20\t0.5\n\
         10\t20\n",
    );
    assert_eq!(c.stars.len(), 1);
    let lines: Vec<usize> = c.skipped.iter().map(|(l, _)| *l).collect();
    assert_eq!(lines, [2, 3, 4, 5, 6]);
}

#[test]
fn the_classes_run_from_blue_white_to_orange() {
    let blue = |c: StarClass| c.rgb()[2] as i32 - c.rgb()[0] as i32;
    let order = [
        StarClass::O,
        StarClass::B,
        StarClass::A,
        StarClass::F,
        StarClass::G,
        StarClass::K,
        StarClass::M,
    ];
    // (K and M stop at the same blue; M is the redder by its green)
    assert!(StarClass::M.rgb()[1] < StarClass::K.rgb()[1]);
    for w in order.windows(2) {
        assert!(blue(w[0]) >= blue(w[1]), "{:?} {:?}", w[0], w[1]);
    }
}

#[test]
fn a_star_lies_where_its_right_ascension_and_declination_put_it() {
    let c = StarCatalog::parse("90\t0\t1\tG\n0\t90\t1\tG\n");
    let east = c.stars[0].direction();
    assert!((east[1] - 1.0).abs() < 1e-12, "{east:?}");
    let pole = c.stars[1].direction();
    assert!((pole[2] - 1.0).abs() < 1e-12, "{pole:?}");
}

#[test]
fn the_size_grows_with_the_square_root_of_the_brightness() {
    let c = StarCatalog::parse(LINES);
    // magnitude 0 is size 4, magnitude 6 a quarter, Sirius at the cap
    assert!((star_size(&c.stars[2]) - 4.0 * 0.9727f32.sqrt()).abs() < 1e-3);
    assert!((star_size(&c.stars[3]) - 0.2524).abs() < 1e-3);
    assert!(star_size(&c.stars[0]) <= 8.0);
}

#[test]
fn each_star_lands_in_its_cell_with_its_place_size_and_class() {
    let c = StarCatalog::parse(LINES);
    let map = StarMap::new(&c, 64);
    assert_eq!((map.width, map.height), (384, 128));
    assert_eq!(map.crowded, 0);
    let filled = map.rgba.chunks(4).filter(|t| t[2] != 0).count();
    assert_eq!(filled, c.stars.len());
    for s in &c.stars {
        let (face, u, v) = face_of(s.direction());
        let i = ((u + 1.0) * 32.0).floor() as usize;
        let j = ((v + 1.0) * 32.0).floor() as usize;
        let t = &map.rgba[(j * 384 + face * 64 + i) * 4..][..4];
        // the place inside the cell, to 1/256 of it
        let back_u = (i as f64 + (t[0] as f64 + 0.5) / 256.0) / 32.0 - 1.0;
        let back_v = (j as f64 + (t[1] as f64 + 0.5) / 256.0) / 32.0 - 1.0;
        assert!((back_u - u).abs() < 1.0 / 64.0 / 256.0 + 1e-9, "{s:?}");
        assert!((back_v - v).abs() < 1.0 / 64.0 / 256.0 + 1e-9, "{s:?}");
        let size = (t[2] as f32 - 1.0) / 254.0 * 8.0;
        assert!((size - star_size(s)).abs() < 0.02, "{s:?}");
        assert_eq!(t[3], s.class.code());
    }
}

#[test]
fn a_cell_holds_two_stars_the_brighter_on_top_and_drops_a_third() {
    let c = StarCatalog::parse("40\t10\t0.01\tM\n40.01\t10.01\t2\tB\n40.02\t10.0\t0.5\tG\n");
    let map = StarMap::new(&c, 16);
    assert_eq!(map.crowded, 1);
    let held: Vec<(usize, u8)> = map
        .rgba
        .chunks(4)
        .enumerate()
        .filter(|(_, t)| t[2] != 0)
        .map(|(k, t)| (k / map.width as usize, t[3]))
        .collect();
    assert_eq!(held.len(), 2);
    let (top, bottom) = (held[0], held[1]);
    assert!(top.0 < 16 && bottom.0 >= 16 && bottom.0 - top.0 == 16);
    assert_eq!(top.1, StarClass::B.code());
    assert_eq!(bottom.1, StarClass::G.code());
}

#[test]
fn every_face_of_the_cube_is_found() {
    let dirs = [
        [1.0, 0.2, 0.1],
        [-1.0, 0.2, 0.1],
        [0.2, 1.0, 0.1],
        [0.2, -1.0, 0.1],
        [0.2, 0.1, 1.0],
        [0.2, 0.1, -1.0],
    ];
    for (k, d) in dirs.iter().enumerate() {
        let (face, u, v) = face_of(*d);
        assert_eq!(face, k);
        assert!(u.abs() <= 1.0 && v.abs() <= 1.0);
    }
}
