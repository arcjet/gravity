use gravity::name_collisions::mod_;

wit_bindgen::generate!({
    world: "name-collisions",
});

struct NameCollisions;

export!(NameCollisions);

impl Guest for NameCollisions {
    fn pick(v: R) -> u64 {
        v.ptr1
    }

    fn keywords(r#type: String, r#func: u32, string: u32, range: u32, nil: bool) -> String {
        format!(
            "{type} {func} {string} {range} {nil}",
            r#type = r#type,
            func = r#func
        )
    }

    fn fixed_names(len: u32, errors: String, ctx: u32, i: u32) -> String {
        format!("{len} {errors} {ctx} {i}")
    }

    fn numbered(arg1: u32, ptr0: u32, write_string: String) -> String {
        format!("{arg1} {ptr0} {write_string}")
    }

    fn call_host(s: String) -> String {
        mod_::echo(&s, 7, s.len() as u32)
    }
}
