use gravity::regressions::{checker, pinger, processor};

wit_bindgen::generate!({
    world: "regressions",
});

struct RegressionsWorld;

export!(RegressionsWorld);

impl Guest for RegressionsWorld {
    fn check_enabled(key: String) -> bool {
        checker::is_enabled(&key)
    }

    fn check_status(key: String) -> u32 {
        match checker::get_status(&key) {
            checker::Status::Active => 0,
            checker::Status::Inactive => 1,
            checker::Status::Unknown => 2,
        }
    }

    fn double_value(value: u32) -> u32 {
        processor::double(value)
    }

    fn run_ping() -> bool {
        pinger::ping()
    }
}
