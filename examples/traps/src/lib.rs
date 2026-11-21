wit_bindgen::generate!({
    world: "traps",
});

struct Traps;

export!(Traps);

/// Panics, which traps the guest, when `fail` is true.
fn trap_if(fail: bool) {
    if fail {
        panic!("trap requested");
    }
}

impl Guest for Traps {
    fn count(text: String, fail: bool) -> u32 {
        trap_if(fail);
        text.len() as u32
    }

    fn detect(text: String, fail: bool) -> Vec<Finding> {
        trap_if(fail);
        vec![Finding {
            start: 0,
            end: text.len() as u32,
            kind: "all".into(),
        }]
    }

    fn reset(fail: bool) {
        trap_if(fail);
    }
}
