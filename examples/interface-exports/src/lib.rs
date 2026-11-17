use exports::gravity::interface_exports::{geometry, greeter};
use gravity::interface_exports::{log, types::Point};

wit_bindgen::generate!({
    world: "interface-exports",
});

struct InterfaceExports;

export!(InterfaceExports);

impl Guest for InterfaceExports {
    fn version() -> String {
        "0.1.0".into()
    }
}

impl geometry::Guest for InterfaceExports {
    fn quadrant_of(p: Point) -> geometry::Quadrant {
        use geometry::Quadrant::*;
        match (p.x.signum(), p.y.signum()) {
            (0, 0) => Origin,
            (1, 1) => First,
            (-1, 1) => Second,
            (-1, -1) => Third,
            (1, -1) => Fourth,
            _ => Axis,
        }
    }

    fn midpoint(s: geometry::Segment) -> Point {
        Point {
            x: (s.start.x + s.end.x) / 2,
            y: (s.start.y + s.end.y) / 2,
        }
    }

    fn length_squared(s: geometry::Segment) -> u64 {
        let dx = i64::from(s.end.x - s.start.x);
        let dy = i64::from(s.end.y - s.start.y);
        (dx * dx + dy * dy) as u64
    }
}

impl greeter::Guest for InterfaceExports {
    fn greet(name: String) -> String {
        log::info(&format!("greeting {name}"));
        format!("hello, {name}")
    }
}
