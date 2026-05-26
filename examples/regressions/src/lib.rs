use gravity::regressions::{
    bot_verifier, checker, email_checker, ip_source, pinger, processor,
};

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

    fn check_email_allowed(email: String) -> u32 {
        match email_checker::is_allowed(&email) {
            email_checker::ValidatorResponse::Yes => 0,
            email_checker::ValidatorResponse::No => 1,
            email_checker::ValidatorResponse::Maybe => 2,
        }
    }

    fn check_bot_verified(bot_id: String) -> u32 {
        match bot_verifier::verify(&bot_id) {
            bot_verifier::ValidatorResponse::Verified => 0,
            bot_verifier::ValidatorResponse::Spoofed => 1,
            bot_verifier::ValidatorResponse::Unverifiable => 2,
        }
    }

    fn run_ip_lookup(ip: String) -> String {
        ip_source::lookup(&ip).unwrap_or_else(|| "absent".to_string())
    }
}
