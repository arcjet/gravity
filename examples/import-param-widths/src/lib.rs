use gravity::import_param_widths::host;

wit_bindgen::generate!({
    world: "import-param-widths",
});

struct ImportParamWidths;

export!(ImportParamWidths);

impl Guest for ImportParamWidths {
    fn call_wide(a: u64, b: i64, c: f32, d: f64, e: u32, f: i8) -> f64 {
        host::wide(a, b, c, d, e, f)
    }

    fn call_opt(x: Option<i64>) -> i64 {
        host::opt(x)
    }
}
