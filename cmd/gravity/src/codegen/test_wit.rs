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

    /// The type named `name`, wherever it is declared.
    pub fn type_id(&self, name: &str) -> TypeId {
        self.resolve
            .types
            .iter()
            .find(|(_, typ)| typ.name.as_deref() == Some(name))
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("fixture should declare type {name}"))
    }
}
