wit_bindgen::generate!({
    world: "enum-collisions",
});

struct EnumCollisions;

export!(EnumCollisions);

impl Guest for EnumCollisions {
    fn via_for(k: CredentialKind) -> Via {
        match k {
            CredentialKind::PeerSvid => Via::PeerSvid,
            CredentialKind::SignedToken => Via::SignedToken,
            CredentialKind::Bearer => Via::Anonymous,
        }
    }

    fn describe(s: Shape, p: Point) -> String {
        match s {
            Shape::Point => format!("point({}, {})", p.x, p.y),
            Shape::Line => format!("line to ({}, {})", p.x, p.y),
        }
    }
}
