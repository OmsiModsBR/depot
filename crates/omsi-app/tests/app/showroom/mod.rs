use openomsi_game::testing::showroom::*;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

#[test]
fn messages_go_through_the_pipe_as_they_were_sent() {
    let msgs = [
        Msg::Frame(2, 1, 77, vec![1, 2, 3, 255, 4, 5, 6, 255]),
        Msg::Status {
            busy: true,
            error: None,
        },
        Msg::Status {
            busy: false,
            error: Some("no bus".into()),
        },
        Msg::Kept(77, Shape::of(1280, 720, 0.5)),
    ];
    let mut buf = Vec::new();
    for m in &msgs {
        write_msg(&mut buf, m).unwrap();
    }
    let mut r = std::io::Cursor::new(buf);
    for m in &msgs {
        assert_eq!(&read_msg(&mut r).unwrap(), m);
    }
    assert!(read_msg(&mut r).is_err(), "nothing more");
}

#[test]
fn a_stray_line_is_passed_over_and_a_wrong_size_refused() {
    let mut buf = b"hello world, a log line\n".to_vec();
    write_msg(
        &mut buf,
        &Msg::Status {
            busy: true,
            error: None,
        },
    )
    .unwrap();
    let mut r = std::io::Cursor::new(buf);
    assert_eq!(
        read_msg(&mut r).unwrap(),
        Msg::Status {
            busy: true,
            error: None
        }
    );
    assert!(read_msg(&mut std::io::Cursor::new(b"only a line\n".to_vec())).is_err());
    let mut buf = Vec::new();
    write_msg(&mut buf, &Msg::Frame(2, 2, 0, vec![0; 4])).unwrap();
    assert!(
        read_msg(&mut std::io::Cursor::new(buf)).is_err(),
        "a 2x2 picture is 16 bytes"
    );
}

#[test]
fn commands_are_one_line_each() {
    let cmds = [
        Cmd::Look(Look {
            bus: "Vehicles/A/a.bus".into(),
            time: 480,
            ..Default::default()
        }),
        Cmd::Size(800, 450),
        Cmd::Orbit(1.5, -2.0),
        Cmd::Zoom(0.9),
        Cmd::Focus(0.6),
    ];
    for c in cmds {
        let line = serde_json::to_string(&c).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<Cmd>(&line).unwrap(), c);
    }
}

/// A picture of the bus shown before is not taken for the one wanted, and when the helper
/// stops nothing of its last picture stays (no picture kept: "No preview").
#[test]
fn only_a_picture_of_the_bus_wanted_is_shown() {
    let a = Look {
        root: "/nowhere".into(),
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    let b = Look {
        bus: "Vehicles/B/b.bus".into(),
        ..a.clone()
    };
    let mut p = Preview::new();
    let (cmds, _cmd_rx) = channel();
    let (msg_tx, msgs) = channel();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    p.want(a.clone());
    msg_tx.send(Msg::Frame(2, 1, tag(&a), vec![0; 8])).unwrap();
    p.update(0.016, Some((2, 1)));
    assert!(p.has_picture() && p.live);
    assert!(p.take_frame().is_some());
    // another bus: the helper still draws the first until the second is read
    p.want(b.clone());
    msg_tx.send(Msg::Frame(2, 1, tag(&a), vec![0; 8])).unwrap();
    p.update(0.016, Some((2, 1)));
    assert!(
        !p.has_picture() && !p.live,
        "the first bus is not shown as the second"
    );
    assert!(p.take_frame().is_none());
    msg_tx.send(Msg::Frame(2, 1, tag(&b), vec![0; 8])).unwrap();
    p.update(0.016, Some((2, 1)));
    assert!(p.has_picture() && p.live);
    // the helper stops
    drop(msg_tx);
    p.update(0.016, Some((2, 1)));
    assert!(!p.has_picture() && !p.live && p.error.is_some());
}

/// A helper with no picture that says nothing for `FIRST_PICTURE_LIMIT` (one that crashed
/// and is held open, its output never closing) is taken for one that ended: started again
/// on the next graphics interface. One still within the limit is waited for.
#[test]
fn a_helper_with_no_picture_for_too_long_is_tried_on_the_next_interface() {
    let a = Look {
        root: "/nowhere".into(),
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    let mut p = Preview::new();
    p.backends = vec![Some("vulkan"), Some("dx12"), Some("gl")];
    let (cmds, _cmd_rx) = channel();
    let (_msg_tx, msgs) = channel::<Msg>();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    p.want(a.clone());
    p.update(0.016, Some((2, 1)));
    assert!(p.link.is_some() && p.backend == 0, "still waited for");
    p.heard = Instant::now() - FIRST_PICTURE_LIMIT - Duration::from_secs(1);
    p.update(0.016, Some((2, 1)));
    assert!(p.link.is_none());
    assert_eq!(p.backend, 1);
    assert!(
        p.broken.is_none() && p.error.is_none(),
        "tried again, not given up"
    );
}

/// A helper that ends before its first picture is started again on the next graphics
/// interface (where there is one); one that had drawn is not.
#[test]
fn a_helper_gone_before_its_first_picture_is_tried_on_the_next_interface() {
    let a = Look {
        root: "/nowhere".into(),
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    let mut p = Preview::new();
    p.backends = vec![Some("vulkan"), Some("dx12"), Some("gl")];
    let (cmds, _cmd_rx) = channel();
    let (msg_tx, msgs) = channel::<Msg>();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    p.want(a.clone());
    drop(msg_tx);
    p.update(0.016, Some((2, 1)));
    assert!(p.link.is_none());
    assert_eq!(p.backend, 1);
    assert!(
        p.broken.is_none() && p.error.is_none(),
        "tried again, not given up"
    );
    // one that drew pins its interface: a later helper gone early does not move it on
    let (cmds, _cmd_rx) = channel();
    let (msg_tx, msgs) = channel::<Msg>();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    msg_tx.send(Msg::Frame(2, 1, tag(&a), vec![0; 8])).unwrap();
    p.update(0.016, Some((2, 1)));
    drop(msg_tx);
    p.update(0.016, Some((2, 1)));
    let (cmds, _cmd_rx) = channel();
    let (msg_tx, msgs) = channel::<Msg>();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    p.drew = false;
    drop(msg_tx);
    p.update(0.016, Some((2, 1)));
    assert_eq!(p.backend, 1);
    assert!(p.broken.is_some());
}

/// The player's own graphics interface comes first, then the others; Automatic starts on
/// Vulkan (Windows only: elsewhere the helper uses the settings' one as the game does).
#[test]
fn the_helper_starts_on_the_interface_the_player_chose() {
    if !cfg!(windows) || omsi_cfg::flags::BACKEND.is_set() {
        assert_eq!(helper_backends("dx12"), vec![None]);
        return;
    }
    // (Vulkan first even when the game's setting names DirectX 12 or OpenGL)
    assert_eq!(
        helper_backends("auto"),
        vec![Some("vulkan"), Some("dx12"), Some("gl")]
    );
    assert_eq!(
        helper_backends("vulkan"),
        vec![Some("vulkan"), Some("dx12"), Some("gl")]
    );
    assert_eq!(
        helper_backends("dx12"),
        vec![Some("vulkan"), Some("dx12"), Some("gl")]
    );
    assert_eq!(
        helper_backends("gl"),
        vec![Some("vulkan"), Some("gl"), Some("dx12")]
    );
    assert_eq!(
        helper_backends(" D3D12 "),
        vec![Some("vulkan"), Some("dx12"), Some("gl")]
    );
    assert_eq!(
        helper_backends("OpenGL"),
        vec![Some("vulkan"), Some("gl"), Some("dx12")]
    );
}

/// A page that shows a picture (Drive) keeps the helper only until the picture is kept, and
/// the picture stays; turning it asks for the live view.
#[test]
fn a_picture_page_lets_the_helper_go_once_the_picture_is_kept() {
    let a = Look {
        root: "/nowhere".into(),
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    let mut p = Preview::new();
    let (cmds, _cmd_rx) = channel();
    let (msg_tx, msgs) = channel();
    p.link = Some(Link {
        cmds,
        msgs,
        child: None,
        mark: None,
    });
    p.want_picture(a.clone());
    msg_tx.send(Msg::Frame(2, 1, tag(&a), vec![0; 8])).unwrap();
    p.update(0.016, Some((2, 1)));
    assert!(p.has_picture() && p.live && p.link.is_some());
    msg_tx
        .send(Msg::Kept(tag(&a), Shape::of(2, 1, p.focus_x)))
        .unwrap();
    for _ in 0..4 {
        p.update(1.0, Some((2, 1)));
    }
    assert!(p.link.is_none(), "the helper ends once the picture is kept");
    assert!(p.has_picture() && !p.live, "and the picture stays");
    // dragging it: the live view, asked for
    p.orbit(3.0, 0.0);
    assert!(!p.picture_done());
}

#[test]
fn only_the_newest_pictures_are_kept() {
    let dir = std::env::temp_dir().join(format!("depot-previews-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..5 {
        let f = dir.join(format!("{i}.png"));
        std::fs::write(&f, b"x").unwrap();
        let t = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000 + i * 60);
        std::fs::File::options()
            .write(true)
            .open(&f)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    prune(&dir, 2);
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    left.sort();
    assert_eq!(left, ["3.png", "4.png", "notes.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_kept_picture_is_per_bus_and_paint_not_per_light() {
    let a = Look {
        root: "/omsi".into(),
        bus: "Vehicles/A/a.bus".into(),
        paint: "Red".into(),
        time: 480,
        ..Default::default()
    };
    let b = Look {
        time: 1200,
        weather: "rain".into(),
        ..a.clone()
    };
    let c = Look {
        paint: "Blue".into(),
        ..a.clone()
    };
    let shape = Shape::of(1280, 720, 0.5);
    assert_eq!(cache_file(&a, shape), cache_file(&b, shape));
    assert_ne!(cache_file(&a, shape), cache_file(&c, shape));
    assert_eq!(tag(&a), tag(&b));
    assert_ne!(tag(&a), tag(&c));
    // (another page's preview, wider and with the bus further left, keeps its own)
    assert_ne!(
        cache_file(&a, shape),
        cache_file(&a, Shape::of(1600, 720, 0.35))
    );
    assert_eq!(
        shape,
        Shape::of(1284, 722, 0.51),
        "a few pixels more is the same shape"
    );
}

#[test]
fn only_the_paints_with_no_kept_picture_are_missing() {
    let root = std::env::temp_dir().join(format!("depot-missing-kept-{}", std::process::id()));
    let look = |paint: &str| Look {
        root: root.clone(),
        bus: "Vehicles/A/a.bus".into(),
        paint: paint.into(),
        ..Default::default()
    };
    let shape = Shape::of(336, 208, 0.5);
    let all = vec![look(""), look("Red"), look("Blue")];
    // nothing kept: all three; with the Red one kept: the other two
    assert_eq!(missing_kept(all.clone(), shape).len(), 3);
    let file = cache_file(&look("Red"), shape);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"png").unwrap();
    let left = missing_kept(all.clone(), shape);
    let _ = std::fs::remove_file(&file);
    assert_eq!(
        left.iter().map(|l| l.paint.as_str()).collect::<Vec<_>>(),
        vec!["", "Blue"]
    );
    assert!(missing_kept(Vec::new(), shape).is_empty());
    // (the small pictures of the page are named by bus, paint and shape)
    assert_ne!(
        kept_key(&look("Red"), shape),
        kept_key(&look("Blue"), shape)
    );
    assert_ne!(
        kept_key(&look("Red"), shape),
        kept_key(&look("Red"), Shape::of(1600, 720, 0.5))
    );
}

#[test]
fn the_small_pictures_have_a_folder_of_their_own_and_do_not_change_with_the_build() {
    let look = Look {
        root: "/omsi".into(),
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    let small = cache_file_with(&look, thumb_shape(), "1:2:3");
    let page = cache_file_with(&look, Shape::of(1280, 720, 0.5), "1:2:3");
    assert_eq!(small.parent().unwrap().file_name().unwrap(), "small");
    assert_ne!(page.parent().unwrap().file_name().unwrap(), "small");
    assert_ne!(small, page);
    // (a bus that changed is drawn anew)
    assert_ne!(small, cache_file_with(&look, thumb_shape(), "1:2:4"));
    // the run is smaller than what is kept, so it never prunes its own pictures
    assert!(RUN_MAX < KEEP_SMALL);
}

#[test]
fn a_prerender_preview_has_a_mark_of_its_own() {
    assert_eq!(Preview::new().mark, None);
    let own = Preview::for_prerender().mark.unwrap();
    assert!(own.starts_with("showroom-placing") && own != "showroom-placing.txt");
}
