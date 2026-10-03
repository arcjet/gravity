use gravity::enum_discriminants::host;

wit_bindgen::generate!({
    world: "enum-discriminants",
});

struct EnumDiscriminants;

export!(EnumDiscriminants);

impl Guest for EnumDiscriminants {
    fn echo(l: Level) -> Level {
        l
    }

    fn echo_list(ls: Vec<Level>) -> Vec<Level> {
        ls
    }

    fn echo_entry(e: Entry) -> Entry {
        e
    }

    fn echo_option(l: Option<Level>) -> Option<Level> {
        l
    }

    fn via_host(l: Level) -> Level {
        host::pass(l)
    }

    fn host_entries() -> Vec<Entry> {
        host::entries()
    }

    fn via_host_option(l: Option<Level>) -> Option<Level> {
        host::maybe(l)
    }
}
