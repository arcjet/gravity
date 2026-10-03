use gravity::export_memo::host;

wit_bindgen::generate!({
    world: "export-memo",
});

struct ExportMemo;

export!(ExportMemo);

impl Guest for ExportMemo {
    fn join(parts: Vec<String>, sep: String) -> String {
        parts.join(&sep)
    }

    fn host_words(n: u32) -> u32 {
        host::words(n).iter().map(|w| w.len() as u32).sum()
    }
}
