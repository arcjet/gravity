wit_bindgen::generate!({
    world: "variants",
});

struct VariantsWorld;

export!(VariantsWorld);

impl Guest for VariantsWorld {
    fn classify(input: String) -> Entity {
        match input.as_str() {
            "email" => Entity::Email,
            "phone" => Entity::PhoneNumber,
            "ip" => Entity::IpAddress,
            "cc" => Entity::CreditCardNumber,
            other => Entity::Custom(other.to_string()),
        }
    }

    fn tag_all(inputs: Vec<String>) -> Vec<Detected> {
        inputs
            .into_iter()
            .enumerate()
            .map(|(i, input)| Detected {
                kind: Self::classify(input),
                start: i as u32,
                end: (i + 1) as u32,
            })
            .collect()
    }

    fn choose(input: Config) -> String {
        match input {
            Config::Allow(allow) => format!(
                "allow:{}:ctx={:?}",
                allow.entities.len(),
                allow.context_window_size
            ),
            Config::Deny(deny) => format!("deny:{}", deny.entities.len()),
        }
    }

    fn choose_many(input: Entities) -> String {
        match input {
            Entities::AllowAll(list) => format!("allow-all:{}", list.len()),
            Entities::DenyAll(list) => format!("deny-all:{}", list.len()),
        }
    }
}
