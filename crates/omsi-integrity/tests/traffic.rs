//! A small map's AI traffic built from scratch in a temporary folder: nothing here is game
//! content.

use omsi_integrity::traffic::{clock, day};
use omsi_integrity::AiTraffic;
use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omsi-traffic-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn map(root: &Path) {
    write(root, "Maps/Test/global.cfg", "[name]\nTest\n");
    write(
        root,
        "Vehicles/Car/a.ovh",
        "[friendlyname]\nMaker\nSedan\nRed\n",
    );
    write(
        root,
        "Vehicles/Car/b.ovh",
        "[friendlyname]\nMaker\nVan\nWhite\n",
    );
    write(
        root,
        "Vehicles/Bus/bus.bus",
        "[friendlyname]\nMaker\nCity bus\nYellow\n",
    );
    write(
        root,
        "Maps/Test/ailists.cfg",
        "[aigroup_2]\nCars\n\nVehicles\\Car\\a.ovh\t3\nVehicles\\Car\\b.ovh\t1\n\
         Vehicles\\Car\\gone.ovh\t0\n[end]\n\n\
         [aigroup_depot]\nBuses\nHome\n\n\
         [aigroup_depot_typgroup_2]\nVehicles\\Bus\\bus.bus\n\
         1\tB-1\tYellow\t20000101\t\n2\tB-2\tCream\t\t20051231\n3\tB-3\tBlue\t20100101\t\n[end]\n",
    );
    write(
        root,
        "Maps/Test/unsched_vehgroups.txt",
        "[group]\nCars\n1\n",
    );
    write(
        root,
        "Maps/Test/unsched_trafficdens.txt",
        "[group]\nCars\n0.5\n\n[set_day_of_week]\n1\n[trafficdensity]\n0\n0\n\
         [trafficdensity]\n12\n2\n\n[set_day_of_week]\n6\n[trafficdensity]\n0\n1\n",
    );
    write(
        root,
        "Maps/Test/TTData/36.ttl",
        "[newtour]\n1\nBuses\n1023\n\n[addtrip]\nA\n0\n300\n\n[addtrip]\nB\n0\n1290\n\n\
         [newtour]\n2\nGhosts\n1023\n\n[newtour]\n3\nBuses\n864\n\n[addtrip]\nC\n0\n600\n",
    );
    write(
        root,
        "Maps/Test/car_use/36.ocu",
        "[valid]\n19000101\n29991231\n\n[line]\n36\n\n[onlytypes]\nVehicles\\Bus\\bus.bus\n[end]\n",
    );
    write(root, "Sceneryobjects/Cars/p.sco", "");
    write(
        root,
        "Maps/Test/parklist_p.txt",
        "Sceneryobjects\\Cars\\p.sco\nSceneryobjects\\Cars\\p.sco\nSceneryobjects\\Cars\\gone.sco\nnote\n",
    );
    write(
        root,
        "Maps/Test/parklist_p_2.txt",
        "Sceneryobjects\\Cars\\p.sco\n",
    );
    write(root, "Humans/a/m.hum", "");
    write(
        root,
        "Maps/Test/humans.txt",
        "Humans\\a\\m.hum\nHumans\\a\\m.hum\nHumans\\a\\x.hum\n",
    );
}

#[test]
fn groups_fleet_lines_parking_and_people_as_on_the_day() {
    let root = temp_root("day");
    map(&root);
    let t = AiTraffic::load(&root, &root.join("Maps/Test"), 20040614);
    assert_eq!(t.lists, ["Maps/Test/ailists.cfg"]);
    assert!(t.ailists_size > 0);

    let cars = &t.groups[0];
    assert_eq!(
        (cars.name.as_str(), cars.is_depot, cars.random),
        ("Cars", false, Some(1))
    );
    assert_eq!(cars.vehicles.len(), 3);
    assert_eq!(cars.vehicles[0].name, "Maker Sedan");
    assert!(!cars.vehicles[2].exists);
    assert_eq!(cars.total_weight(), 4.0);
    assert_eq!(cars.chance(0), 0.75);
    assert_eq!(cars.chance(2), 0.0);

    // a depot vehicle is in service from its first day to its last
    let buses = &t.groups[1];
    assert!(buses.is_depot && buses.hof.as_deref() == Some("Home"));
    let on: Vec<bool> = buses.fleet[0].entries.iter().map(|e| e.1).collect();
    assert_eq!(on, [true, true, false]);
    assert_eq!(buses.in_service(), 2);
    assert_eq!(t.tours_of("buses"), ["36/1"]);

    let line = &t.lines[0];
    assert_eq!(line.name, "36");
    assert_eq!(line.types.as_ref().unwrap().0, 1.0);
    assert_eq!(line.tours[0].trips, 2);
    assert_eq!(
        (
            clock(line.tours[0].first.unwrap()),
            clock(line.tours[0].last.unwrap())
        ),
        ("05:00".into(), "21:30".into())
    );
    assert!(line.tours[0].group_known && !line.tours[1].group_known);
    // tour 3 runs on Saturdays and Sundays only: not on this Monday
    assert_eq!(line.tours.len(), 2);
    let saturday = AiTraffic::load(&root, &root.join("Maps/Test"), 20040619);
    let tours: Vec<&str> = saturday.lines[0]
        .tours
        .iter()
        .map(|x| x.number.as_str())
        .collect();
    assert_eq!(tours, ["1", "2", "3"]);

    assert_eq!(t.parklists.len(), 2);
    assert_eq!((t.parklists[0].index, t.parklists[1].index), (0, 2));
    assert_eq!(t.parklists[0].total(), 3);
    assert_eq!(t.parklists[0].cars[0].count, 2);
    assert!(!t.parklists[0].cars[1].exists);
    assert_eq!(t.humans.len(), 2);
    assert_eq!(t.humans[0].count, 2);
    assert!(t.drivers.is_empty());

    // the density: factor times the day's curve (6 = Saturday and Sunday)
    let d = &t.density[0];
    assert_eq!(d.at(0, 6.0), 0.5);
    assert_eq!(d.at(1, 6.0), 0.5);
    assert_eq!(d.at(2, 12.0), 0.5);
    assert_eq!(d.at(0, 12.0), 1.0);

    let text = t.to_text(&[("Map".into(), "Test".into())]);
    assert!(!text.replace("\r\n", "").contains('\n'));
    for want in [
        "Project Depot - AI traffic",
        "Day      : 2004-06-14",
        "== Car groups (1) ==",
        "Cars (default, random traffic, class 1): 3 vehicles",
        "75.0 %  Vehicles\\Car\\a.ovh (Maker Sedan)",
        "Vehicles\\Car\\gone.ovh - missing",
        "Buses (depot file Home): 2 of 3 in service",
        "3        B-3            Blue  2010-01-01 to ... - not in service",
        "36 (Maps/Test/TTData/36.ttl): 2 tours, priority 0, only Vehicles\\Bus\\bus.bus",
        "05:00-21:30",
        "Ghosts",
        "- no AI list has this group",
        "Maps/Test/parklist_p.txt: 3 cars",
        "  2x Sceneryobjects\\Cars\\p.sco",
        "Cars: factor 0.5",
        "Saturday, Sunday: 00:00=1",
    ] {
        assert!(text.contains(want), "{want:?} not in\n{text}");
    }
    assert_eq!(day(20040614), "2004-06-14");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_chrono_scenario_in_force_adds_to_its_groups() {
    let root = temp_root("chrono");
    map(&root);
    write(
        &root,
        "Maps/Test/Chrono/2010/Chrono.cfg",
        "[startdate]\n20100101\n\n[enddate]\n20201231\n",
    );
    write(
        &root,
        "Maps/Test/Chrono/2010/ailists_#upd.cfg",
        "[aigroup_2]\nCars\n\nVehicles\\Car\\b.ovh\t4\n[end]\n",
    );
    let before = AiTraffic::load(&root, &root.join("Maps/Test"), 20040614);
    assert!(before.chrono.is_empty());
    assert_eq!(before.groups[0].vehicles.len(), 3);
    let after = AiTraffic::load(&root, &root.join("Maps/Test"), 20150614);
    assert_eq!(after.chrono, ["Chrono/2010"]);
    assert_eq!(after.lists.len(), 2);
    let added = after.groups[0].vehicles.last().unwrap();
    assert_eq!(added.weight, 4.0);
    assert_eq!(added.listed_in, "Maps/Test/Chrono/2010/ailists_#upd.cfg");
    let _ = fs::remove_dir_all(&root);
}
