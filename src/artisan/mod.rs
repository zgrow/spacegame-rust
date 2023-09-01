// artisan/mod.rs
// Describes the class of inanimate objects throughout the game, both Props and Furniture

// *** EXTERNAL LIBRARIES
use bevy::prelude::*;
use bevy::ecs::world::EntityMut;

// *** INTERNAL LIBRARIES
use crate::components::*;
use crate::engine::planq::*;

/// Defines the set of item types, which allow requests to be made for specific types of items at runtime
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ItemType {
	#[default]
	Simple,  /// aka Item, name changed for better disambiguation
	Thing,
	Snack,
	Fixture,
	Door,
	Planq,
}
/// Defines a baseline 'inanimate object' component bundle
/// This is only useful on its own for defining pieces of scenery/backdrop, ie
/// things that will not move, do not have interactions, and do not block movement or sight
#[derive(Bundle)]
pub struct Item {
	pub desc:    Description,
	pub render:  Renderable,
	pub actions: ActionSet,
}
/// Defines the class of objects that are generally smaller than the player/assumed to be Portable
#[derive(Bundle)]
pub struct Thing {
	pub item:       Item,
	pub portable:   Portable,
}
/// just a demo thing for now, might change later
#[derive(Bundle)]
pub struct Snack {
	pub item:       Thing,
//	pub consume:    Consumable,
}
/// Defines the class of objects that are generally larger than the player/assumed to Obstruct movement
#[derive(Bundle)]
pub struct Fixture {
	pub item:       Item,
	pub obstructs:  Obstructive,
	pub opaque:     Opaque,
}
/// Defines the class of objects that allow/obstruct entity movement across a threshold
#[derive(Bundle)]
pub struct Door {
	pub item:       Fixture,
	pub door:       Openable,
	pub lock:       Lockable,
}

/// Provides a facility for creating items during gameplay
#[derive(Resource, Clone, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct ItemBuilder {
	pub spawn_count: i32,
	desc:     Option<Description>,
	render:   Option<Renderable>,
	posn:     Option<Position>,
	actions:  Option<ActionSet>,
	obstruct: Option<Obstructive>,
	opaque:   Option<Opaque>,
	open:     Option<Openable>,
	portable: Option<Portable>,
	device:   Option<Device>,
	mobile:   Option<Mobile>,
	contain:  Option<Container>,
	lock:     Option<Lockable>,
	key:      Option<Key>,
	planq:    Option<Planq>,
}
impl<'a, 'b> ItemBuilder where 'a: 'b {
	/// ItemBuilder constructor
	pub fn new() -> ItemBuilder {
		ItemBuilder::default()
	}
	/// Generates the Item itself; note that the Portable component will always be generated with a placeholder!
	/// Therefore, to actually spawn the item into the world, either the at() or within() builder chains MUST be used
	pub fn create(&mut self, new_type: ItemType) -> &mut ItemBuilder {
		match new_type {
			ItemType::Simple    => {
				self.desc = Some(Description::new(format!("_simpleItem_{}", self.spawn_count), "A simple Item.".to_string()));
				self.render = Some(Renderable::new("i".to_string(), 4, 0));
				self.actions = Some(ActionSet::new());
			}
			ItemType::Thing     => {
				self.desc = Some(Description::new(format!("_thing_{}", self.spawn_count), "A new Thing.".to_string()));
				self.render = Some(Renderable::new("t".to_string(), 4, 0));
				self.actions = Some(ActionSet::new());
				self.portable = Some(Portable::empty());
			}
			ItemType::Fixture   => {
				self.desc = Some(Description::new(format!("_fixture_{}", self.spawn_count), "A plain Fixture.".to_string()));
				self.render = Some(Renderable::new("#".to_string(), 4, 0));
				self.actions = Some(ActionSet::new());
				self.obstruct = Some(Obstructive::default());
				self.opaque = Some(Opaque::new(true));
			}
			ItemType::Door      => {
				self.desc = Some(Description::new(format!("_door_{}", self.spawn_count), "A regular Door.".to_string()));
				self.render = Some(Renderable::new("█".to_string(), 4, 0));
				self.actions = Some(ActionSet::new());
				self.obstruct = Some(Obstructive::default());
				self.opaque = Some(Opaque::new(true));
				self.open = Some(Openable::new(false, "▔".to_string(), "█".to_string(),));
			}
			ItemType::Snack     => {
				self.desc = Some(Description::new(format!("_snack_{}", self.spawn_count), "A tasty Snack.".to_string()));
				self.render = Some(Renderable::new("%".to_string(), 5, 0));
				self.actions = Some(ActionSet::new());
				self.portable = Some(Portable::empty());
			}
			ItemType::Planq     => {
				self.desc = Some(Description::new("PLANQ".to_string(), "It's your PLANQ.".to_string()));
				self.render = Some(Renderable::new("¶".to_string(), 3, 0));
				self.actions = Some(ActionSet::new());
				self.portable = Some(Portable::empty());
				self.device = Some(Device::new(-1));
				self.planq = Some(Planq::new());
			}
		}
		self
	}
	pub fn at(&mut self, posn: Position) -> &mut ItemBuilder {
		self.posn = Some(posn);
		self
	}
	pub fn within(&mut self, target: Entity) -> &mut ItemBuilder {
		self.portable = Some(Portable::new(target));
		self
	}
	pub fn build(&'b mut self, world: &'a mut World) -> EntityMut<'b> {
		self.spawn_count += 1;
		let mut new_item = world.spawn_empty();
		if let Some(desc)     = &self.desc { new_item.insert(desc.clone()); self.desc = None; }
		if let Some(render)   = &self.render { new_item.insert(render.clone()); self.render = None; }
		if let Some(posn)     = self.posn { new_item.insert(posn); self.posn = None; }
		if let Some(actions)  = &self.actions { new_item.insert(actions.clone()); self.actions = None; }
		if let Some(obstruct) = self.obstruct { new_item.insert(obstruct); self.obstruct = None; }
		if let Some(opaque)   = self.opaque { new_item.insert(opaque); self.opaque = None; }
		if let Some(open)     = &self.open { new_item.insert(open.clone()); self.open = None; }
		if let Some(portable) = self.portable { new_item.insert(portable); self.portable = None; }
		if let Some(device)   = self.device { new_item.insert(device); self.device = None; }
		if let Some(mobile)   = self.mobile { new_item.insert(mobile); self.mobile = None; }
		if let Some(contain)  = &self.contain { new_item.insert(contain.clone()); self.contain = None; }
		if let Some(lock)     = self.lock { new_item.insert(lock); self.lock = None; }
		if let Some(key)      = self.key { new_item.insert(key); self.key = None; }
		if let Some(planq)    = self.planq { new_item.insert(planq); self.planq = None; }
		new_item
	}
}

// EOF