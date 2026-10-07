wit_bindgen::generate!({
    world: "option-float-params",
});

struct OptionFloatParams;

export!(OptionFloatParams);

impl Guest for OptionFloatParams {
    fn scale(value: f64, factor: Option<f64>) -> f64 {
        value * factor.unwrap_or(1.0)
    }

    fn scale32(value: f32, factor: Option<f32>) -> f32 {
        value * factor.unwrap_or(1.0)
    }

    fn to_meters(l: Length) -> f64 {
        match l {
            Length::Meters(m) => m,
            Length::Feet(f) => f * 0.3048,
            Length::Unknown => -1.0,
        }
    }
}
