//! Builds `wit_parser` fixtures for unit tests from WIT text, so the tests do
//! not construct wit-parser's structs field by field.

use wit_bindgen_core::wit_parser::{
    Function, Resolve, SizeAlign, TypeId, World, WorldId, WorldItem, WorldKey,
};

pub struct Fixture {
    pub resolve: Resolve,
    pub world_id: WorldId,
    pub sizes: SizeAlign,
}

impl Fixture {
    /// Parses `wit`, which must declare a package and exactly one world.
    pub fn parse(wit: &str) -> Self {
        let mut resolve = Resolve::default();
        let package = resolve
            .push_source("fixture.wit", wit)
            .expect("fixture WIT should parse");
        let world_id = resolve
            .select_world(&[package], None)
            .expect("fixture should declare one world");
        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve).expect("sizes should fill");
        Self {
            resolve,
            world_id,
            sizes,
        }
    }

    pub fn world(&self) -> &World {
        &self.resolve.worlds[self.world_id]
    }

    /// The function the world exports as `name`.
    pub fn export(&self, name: &str) -> &Function {
        match self.world().exports.get(&WorldKey::Name(name.to_string())) {
            Some(WorldItem::Function(func)) => func,
            other => panic!("world should export function {name}, found {other:?}"),
        }
    }

    /// The function `name` declared in interface `interface`.
    pub fn function(&self, interface: &str, name: &str) -> &Function {
        let (_, iface) = self
            .resolve
            .interfaces
            .iter()
            .find(|(_, iface)| iface.name.as_deref() == Some(interface))
            .unwrap_or_else(|| panic!("fixture should declare interface {interface}"));
        iface
            .functions
            .get(name)
            .unwrap_or_else(|| panic!("interface {interface} should declare function {name}"))
    }

    /// The type named `name`. Panics unless exactly one type in the fixture
    /// has that name, so a fixture that declares it in two interfaces fails
    /// instead of returning whichever comes first.
    pub fn type_id(&self, name: &str) -> TypeId {
        let matches: Vec<TypeId> = self
            .resolve
            .types
            .iter()
            .filter(|(_, typ)| typ.name.as_deref() == Some(name))
            .map(|(id, _)| id)
            .collect();
        match matches.as_slice() {
            [id] => *id,
            [] => panic!("fixture should declare type {name}"),
            _ => panic!("fixture declares {} types named {name}", matches.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Fixture;

    #[test]
    #[should_panic(expected = "fixture declares 2 types named point")]
    fn test_type_id_rejects_ambiguous_name() {
        let fixture = Fixture::parse(
            "package test:fixture;
            interface a {
                record point { x: u32 }
            }
            interface b {
                record point { y: u32 }
            }
            world test-world {
                import a;
                import b;
            }",
        );
        fixture.type_id("point");
    }
}
