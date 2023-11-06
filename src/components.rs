// components.rs
// July 12 2023

use std::fmt;
use std::hash::Hash;
use bevy::ecs::entity::*;
use bevy::utils::hashbrown::{HashMap, HashSet};
use bevy::prelude::{
	Component,
	FromWorld,
	Reflect,
	ReflectComponent,
	ReflectResource,
	Resource,
	World,
};
use bracket_pathfinding::prelude::*;
use ratatui::layout::Rect;
use strum_macros::AsRefStr;
use crate::engine::event::ActionType;
use crate::camera::ScreenCell;
use simplelog::*;

// Full-length derive macros
//#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
//#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]

/// Identifies the Entity that represents the player character
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Player { }
/// Identifies the LMR in the ECS
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct LMR { }
/// Allows an entity to identify the set of ActionTypes that it supports.
/// The presence of an ActionType in actions indicates it is compatible;
/// finding the intersection between two ActionSets results in the set of actions
/// that one entity may execute on another
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct ActionSet {
	#[reflect(ignore)]
	pub actions: HashSet<ActionType>,
	#[reflect(ignore)]
	pub outdated: bool,
}
impl ActionSet {
	pub fn new() -> Self {
		ActionSet::default()
	}
}
impl Default for ActionSet {
	fn default() -> ActionSet {
		ActionSet {
			actions: HashSet::new(),
			outdated: true,
		}
	}
}
/// Represents a point on a 2D grid as an XY pair, plus a Z-coordinate to indicate what floor the entity is on
#[derive(Component, Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(Component, Resource)]
pub struct Position {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}
impl Position {
	/// A handy constant for checking if a map coordinate is invalid
	pub const INVALID: Position = Position{x: -1, y: -1, z: -1};
	/// Creates a new Position from the given values
	pub fn new(new_x: i32, new_y: i32, new_z: i32) -> Position {
		Position{ x: new_x, y: new_y, z: new_z }
	}
	/// This is just a naive calculator for when all the variables can be obtained easily
	/// Thus it runs very quickly by virtue of not needing to call into the ECS
	/// Returns true if distance == range (ie is inclusive)
	pub fn in_range_of(&self, target: &Position, range: i32) -> bool {
		debug!("* Testing range {} between positions {} to {}", range, self, target); // DEBUG: announce range check
		if self.z != target.z { return false; } // z-levels must match (ie on same floor)
		if range == 0 {
			// This case is provided against errors; it's often faster/easier to just compare
			// positions directly in the situation where this method would be called
			if self == target { return true; }
		} else {
			let mut d_x = f32::powi((target.y - self.y) as f32, 2);
			let mut d_y = f32::powi((target.x - self.x) as f32, 2);
			debug!("dx: {}, dy: {}", d_x, d_y); // DEBUG: print the raw values for dx, dy
			if d_x.signum() != 1.0 { d_x *= -1.0; }
			if d_y.signum() != 1.0 { d_y *= -1.0; }
			debug!("dx: {}, dy: {}", d_x, d_y); // DEBUG: print the normalized values for dx, dy
			let distance = f32::sqrt(d_x + d_y).round();
			debug!("* in_range_of(): calc dist = {self:?} to {target:?}: {} in range {} -> {}", distance, range, (distance as i32 <= range)); // DEBUG: print the result of the calculation
			if distance as i32 <= range { return true; }
		}
		false
	}
	/// Checks if two Positions are next to each other; shorthand for calling `self.in_range_of(target, 1)`
	pub fn is_adjacent_to(&self, target: &Position) -> bool {
		self.in_range_of(target, 1)
	}
	/// Converts map coordinates to screen coordinates
	/// WARN: this method does NOT guarantee or validate the coordinates it generates; if a given Position
	/// would fall offscreen, then that is what will be returned!
	/// The player's position is required as the second parameter in order to provide a reference point between the two maps
	pub fn to_camera_coords(&self, screen: Rect, p_map: Position) -> Position {
		// We can discard the z coordinate, since we can only see one level at a time anyway
		// We can also assume the following relation/analogy: centerpoint : screen :: p_map : worldmap
		let c_x = screen.width / 2;
		let c_y = screen.height / 2;
		let d_x = p_map.x - self.x;
		let d_y = p_map.y - self.y;
		Position::new(c_x as i32 - d_x, c_y as i32 - d_y, 0)
	}
}
impl From<(i32, i32, i32)> for Position {
	fn from(value: (i32, i32, i32)) -> Self {
		Position {
			x: value.0,
			y: value.1,
			z: value.2,
		}
	}
}
impl From<(usize, usize, usize)> for Position {
	fn from(value: (usize, usize, usize)) -> Self {
		Position{
			x: value.0 as i32,
			y: value.1 as i32,
			z: value.2 as i32,
		}
	}
}
impl PartialEq<(i32, i32, i32)> for Position {
	fn eq(&self, rhs: &(i32, i32, i32)) -> bool {
		if self.x == rhs.0 && self.y == rhs.1 && self.z == rhs.2 { return true; }
		false
	}
}
impl fmt::Display for Position {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}, {}, {}", self.x, self.y, self.z)
	}
}
impl std::ops::Add<(i32, i32, i32)> for Position {
	type Output = Position;
	fn add(self, rhs: (i32, i32, i32)) -> Position {
		Position {
			x: self.x + rhs.0,
			y: self.y + rhs.1,
			z: self.z + rhs.2,
		}
	}
}
impl std::ops::AddAssign<(i32, i32, i32)> for Position {
	fn add_assign(&mut self, rhs: (i32, i32, i32)) {
		self.x += rhs.0;
		self.y += rhs.1;
		self.z += rhs.2;
	}
}
// ***
/// Provides some ergonomics around Rust's type handling so that there's less "x as usize" casting everywhere;
/// used for small adjustments on a grid map in the SAME z-level; if a z-level transition is required look elsewhere
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PosnOffset {
	pub x_diff: i32,
	pub y_diff: i32,
	pub z_diff: i32,
}
impl PosnOffset {
	pub fn new(echs: i32, whye: i32, zhee: i32) -> PosnOffset {
		PosnOffset {
			x_diff: echs,
			y_diff: whye,
			z_diff: zhee,
		}
	}
}
impl std::ops::Add<PosnOffset> for Position {
	type Output = Position;
	fn add(self, rhs: PosnOffset) -> Position {
		Position {
			x: self.x + rhs.x_diff,
			y: self.y + rhs.y_diff,
			z: self.z + rhs.z_diff,
		}
	}
}
impl std::ops::AddAssign<PosnOffset> for Position {
	fn add_assign(&mut self, rhs: PosnOffset) {
		*self = *self + rhs;
	}
}
/* NOTE: Defn for "Position - PosnOffset = Position" is disabled due to uncertainty; subtraction on a PosnOffset
 *       that contains negative values will almost definitely produce unexpected behavior...
 *	impl std::ops::Sub<PosnOffset> for Position {
 *	type Output = Position;
 *	fn sub(self, rhs: PosnOffset) -> Position {
 *		Position {
 *			x: self.x - rhs.x_diff,
 *			y: self.y - rhs.y_diff,
 *			z: self.z - rhs.z_diff,
 *		}
 *	}
 *}
 *impl std::ops::SubAssign<PosnOffset> for Position {
 *	fn sub_assign(&mut self, rhs: PosnOffset) {
 *		*self = *self - rhs;
 *	}
 *}
*/
/* NOTE: Defn for "Position + Position = Position" is disabled due to uncertainty:
 * vector sums are useful when trying to calculate the amount of force applied to a body,
 * but that isn't useful right now since I have no physics to worry about
*/
impl std::ops::Sub<Position> for Position {
	type Output = PosnOffset;
	fn sub(self, rhs: Position) -> PosnOffset {
		PosnOffset {
			x_diff: self.x - rhs.x,
			y_diff: self.y - rhs.y,
			z_diff: self.z - rhs.z,
		}
	}
}
/// Defines the shape/form of an Entity's physical body within the gameworld, defined on absolute game Positions
/// Allows Entities to track all of their physical shape, not just their canonical Position
/// NOTE: if an Entity's 'extended' Body is supposed to use different glyphs, then the Renderable.glyph
/// property should be set to the _entire_ string, in order, that the game should render
/// ie the Positions listed in Body.extent need to correspond with the chars in the Entity's Renderable.glyph
/// If there aren't enough chars to cover all the given Positions, then the last-used char will be repeated
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Body { // aka Exterior, Veneer, Mass, Body, Visage, Shape, Bulk, Whole
	pub ref_posn: Position,
	pub extent: Vec<Position>
}
impl Body {
	pub fn single(posn: Position) -> Body {
		Body {
			ref_posn: posn,
			extent: vec![posn],
		}
	}
	pub fn multitile(posns: Vec<Position>) -> Body {
		Body {
			ref_posn: posns[0],
			extent: posns.clone(),
		}
	}
	/// WARN: this does not check for duplicates! It is meant to extend an entity that already has a body...
	pub fn extend(mut self, mut new_posns: Vec<Position>) -> Self {
		self.extent.append(&mut new_posns);
		self
	}
	pub fn contains(&self, target: &Position) -> bool {
		self.extent.contains(target)
	}
	pub fn is_adjacent_to(&self, target: &Position) -> bool {
		for point in self.extent.iter() {
			if point.in_range_of(target, 1) {
				return true;
			}
		}
		false
	}
	pub fn in_range_of(&self, target: &Position, range: i32) -> bool {
		for point in self.extent.iter() {
			if point.in_range_of(target, range) {
				return true;
			}
		}
		false
	}
	pub fn move_to(&mut self, target: Position) {
		//let posn_diff = self.ref_posn - target;
		let posn_diff = target - self.ref_posn;
		for posn in self.extent.iter_mut() {
			*posn += posn_diff;
		}
		//debug!("move_to: {}({:?}) to {} => {:?}", self.ref_posn, self.extent, target, posn_diff);
		self.ref_posn = target;
	}
	pub fn project_to(&self, target: Position) -> Vec<Position> {
		let mut posn_list = Vec::new();
		let posn_diff = target - self.ref_posn;
		for posn in self.extent.iter() {
			let new_posn = *posn + posn_diff;
			posn_list.push(new_posn);
		}
		posn_list
	}
}

// ***
/// Holds the narrative description of an object. If this component is used as an input for text formatting, it will produce
/// the name of the entity that owns it. See also the name() and desc() methods
#[derive(Component, Clone, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Description {
	pub name: String,
	pub desc: String,
	pub locn: String,
}
impl Description {
	/// Creates a new Description with the given name and description
	pub fn new() -> Description {
		Description::default()
	}
	pub fn name(mut self, new_name: &str) -> Self {
		self.name = new_name.to_string();
		self
	}
	pub fn desc(mut self, new_desc: &str) -> Self {
		self.desc = new_desc.to_string();
		self
	}
	pub fn locn(mut self, new_locn: &str) -> Self {
		self.locn = new_locn.to_string();
		self
	}
	pub fn get_name(&self) -> String {
		self.name.clone()
	}
	pub fn get_desc(&self) -> String {
		self.desc.clone()
	}
	pub fn get_locn(&self) -> String {
		self.locn.clone()
	}
}
impl Default for Description {
	fn default() -> Description {
		Description {
			name: "default_name".to_string(),
			desc: "default_desc".to_string(),
			locn: "default_locn".to_string(),
		}
	}
}
impl fmt::Display for Description {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.name)
	}
}
/// Holds the information needed to display an Entity on the worldmap
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Renderable {
	// Field types selected for compatibility with ratatui::buffer::Cell
	pub glyph: String,  // stdlib
	pub fg: u8,         // ratatui as a Color::Indexed
	pub bg: u8,         // ratatui
	pub mods: u16,      // ratatui
	pub width: u32,
	pub height: u32,
	// The above fields will be superceded by the ScreenCell object list
	pub glyphs: HashMap<Position, ScreenCell>,
}
impl Renderable {
	pub fn new() -> Renderable {
		Renderable::default()
	}
	pub fn glyph(mut self, new_glyph: &str) -> Renderable {
		self.glyph = new_glyph.to_string();
		self
	}
	pub fn dims(mut self, new_width: u32, new_height: u32) -> Renderable {
		self.width = new_width;
		self.height = new_height;
		self
	}
	pub fn fg(mut self, fg_value: u8) -> Renderable {
		self.fg = fg_value;
		self
	}
	pub fn bg(mut self, bg_value: u8) -> Renderable {
		self.bg = bg_value;
		self
	}
}

/// Provides an object abstraction for the sensory range of a given entity
//  INFO: This Viewshed type is NOT eligible for bevy_save because bracket_lib::Point doesn't impl Reflect/FromReflect
#[derive(Component, Clone, Debug)]
pub struct Viewshed {
	pub visible_tiles: Vec<Point>, // for bracket_lib::pathfinding::field_of_view
	pub range: i32,
	pub dirty: bool, // indicates whether this viewshed needs to be updated from world data
	// Adding an Entity type to the enty_memory ought to allow for retrieving that information later, so that the
	// player's own memory can be queried, something like the Nethack dungeon feature notes tracker
}
impl Viewshed {
	pub fn new(new_range: i32) -> Self {
		Self {
			visible_tiles: Vec::new(),
			range: new_range,
			dirty: true,
		}
	}
}
/// Provides a memory of seen entities and other things to an entity with sentience
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Memory {
	//pub visual: HashMap<Entity, Position>,
	pub visual: HashMap<Position, Vec<Entity>>,
}
impl Memory {
	pub fn new() -> Self {
		Memory::default()
	}
	fn remove_from_memory(&mut self, target: Entity) {
		// This line will find the first key-value pair where the value is 'target' and return the key
		//self.visual.iter().find_map(|(key, &val)| if val.contains(&target) { Some(key) } else { None });
		// Find all Positions in the actor's memory that contain this Entity
		let all_points: Vec<Position> = self.visual.iter()
			.filter_map(|(key, val)| if val.contains(&target) { Some(*key) } else { None }).collect();
		//debug!("remove_from_memory: {:?}", all_points);
		// Remove the Entity from those Positions in the actor's memory
		for posn in all_points.iter() {
			if let Some(enty_list) = self.visual.get_mut(posn) {
				enty_list.remove(enty_list.iter().position(|x| *x == target).unwrap());
			}
		}
	}
	/// Updates the memorized positions for the specified entity; adds to memory if not already present
	pub fn update(&mut self, target: Entity, posn: Position) {
		// Find any previous references to this entity in the visual memory and remove them
		self.remove_from_memory(target); // DEBUG: this method seems to work fine without this call...?
		// Update the memory with the new position
		if let Some(enty_list) = self.visual.get_mut(&posn) {
			enty_list.push(target);
			//debug!("Memory::update: {:?}", enty_list);
		} else {
			self.visual.insert(posn, vec![target]);
			//debug!("Memory::insert: {:?} @{:?}", target, posn);
		}
	}
}
/// Defines a set of mechanisms that allow an entity to maintain some internal state and memory of game context
/// Describes an Entity that can move around under its own power
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Mobile { }
/// Describes an entity that obstructs movement by other entities
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Obstructive { }
/// Describes an entity that can be picked up and carried around
//#[derive(Component, Clone, Copy, Debug, Default)]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Portable {
	pub carrier: Entity
}
impl Portable {
	pub fn new(target: Entity) -> Portable { Portable { carrier: target } }
	pub fn empty() -> Portable { Portable { carrier: Entity::PLACEHOLDER } }
}
impl MapEntities for Portable {
	fn map_entities(&mut self, entity_mapper: &mut EntityMapper) {
		self.carrier = entity_mapper.get_or_reserve(self.carrier);
	}
}
impl FromWorld for Portable {
	// This is intentional (lmao) to prevent issues when loading from save game
	fn from_world(_world: &mut World) -> Self {
		Self {
			carrier: Entity::PLACEHOLDER,
		}
	}
}
/// Describes an Entity that is currently located within a Container
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct IsCarried { }
/// Describes an entity which may contain entities tagged with the Portable Component
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Container { }
/// Describes an entity that blocks line of sight; comes with an internal state for temp use
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Opaque {
	pub opaque: bool
}
impl Opaque {
	pub fn new(setting: bool) -> Self {
		Opaque {
			opaque: setting,
		}
	}
}
/// Describes an entity with an operable barrier of some kind: a container's lid, or a door, &c
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Openable {
	pub is_open: bool,
	pub is_stuck: bool,
	pub open_glyph: String,
	pub closed_glyph: String,
}
impl Openable {
	pub fn new(state: bool, opened: &str, closed: &str) -> Openable {
		Openable {
			is_open: state,
			is_stuck: false,
			open_glyph: opened.to_string(),
			closed_glyph: closed.to_string(),
		}
	}
}
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Lockable {
	pub is_locked: bool,
	pub key: i32
}
impl Lockable {
	// Unlocks, given the correct key value as input
	pub fn unlock(&mut self, test_key: i32) -> bool {
		if test_key == self.key {
			self.is_locked = false;
			return true;
		}
		false
	}
	// Locks when called; if a key is given, it will overwrite the previous key-value
	// Specify a value of 0 to obtain the existing key-value instead
	pub fn lock(&mut self, new_key: i32) -> i32 {
		self.is_locked = true;
		if new_key != 0 { self.key = new_key; }
		self.key
	}
}
/// Describes an entity that can lock or unlock a Lockable object
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Key { pub key_id: i32 }
/// Describes an entity with behavior that can be applied/used/manipulated by another entity
#[derive(Component, Clone, Copy, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Device {
	pub pw_switch: bool,
	pub batt_voltage: i32,
	pub batt_discharge: i32,
	pub state: DeviceState,
}
impl Device {
	/// Creates a new Device; set the batt_discharge param to 0 to disable battery use
	pub fn new(discharge_rate: i32) -> Device {
		Device {
			pw_switch: false,
			batt_voltage: 0, // BATTERIES NOT INCLUDED LMAOOO
			batt_discharge: discharge_rate,
			state: DeviceState::Offline,
		}
	}
	/// Turns on the device, if there's any power remaining. Returns false if no power left.
	pub fn power_on(&mut self) -> bool {
		if self.batt_voltage > 0
		|| self.batt_discharge == 0 {
			self.pw_switch = true;
			self.state = DeviceState::Idle;
		}
		self.pw_switch
	}
	/// Turns off the device.
	pub fn power_off(&mut self) {
		self.pw_switch = false;
		self.state = DeviceState::Offline;
	}
	/// Discharges battery power according to the specified duration, returns current power level
	pub fn discharge(&mut self, duration: i32) -> i32 {
		if self.batt_discharge < 0 {
			// This item does not need a battery/has infinite power, so no discharge can occur
			return self.batt_voltage;
		}
		self.batt_voltage -= self.batt_discharge * duration;
		if self.batt_voltage < 0 { self.batt_voltage = 0; }
		self.batt_voltage
	}
	/// Recharges the battery to the given percentage
	pub fn recharge(&mut self, charge_level: i32) -> i32 {
		self.batt_voltage += charge_level;
		self.batt_voltage
	}
	/// power toggle
	pub fn power_toggle(&mut self) -> bool {
		// NOTE: trying to invoke these methods doesn't seem to work here; not sure why
		//if !self.pw_switch { self.power_on(); }
		//else { self.power_off(); }
		self.pw_switch = !self.pw_switch;
		self.pw_switch
	}
}
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum DeviceState {
	#[default]
	Offline,
	Idle,
	Working,
	Error(u32) // Takes an error code as a specifier
}
/// Describes an entity with a PLANQ-compatible maintenance system
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct AccessPort { }
/// Describes an entity that can connect to and communicate with the shipnet
#[derive(Component, Copy, Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct Networkable { }

//  *** PRIMITIVES AND COMPUTED VALUES (ie no save/load)
/// A small type that lets us specify friendly names for colors instead of using ints everywhere
/// Because none of these carry any data, they can be cast to numeric types directly
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum Color {
	// These are arranged in order of their ANSI index
	Black,    // 00
	Red,      // 01
	Green,    // 02
	Yellow,   // 03
	Blue,     // 04
	Pink,     // 05
	Cyan,     // 06
	White,    // 07
	#[default]
	LtBlack,  // 08
	LtRed,    // 09
	LtGreen,  // 10
	LtYellow, // 11
	LtBlue,   // 12
	LtPink,   // 13
	LtCyan,   // 14
	LtWhite   // 15
}
/// A convenient type that makes it clear whether we mean the Player entity or some other
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Creature {
	Player,     // The player(s)
	Zilch,      // Any non-player entity or character
}
/// The compass rose - note this is not a component...
/// These are mapped to cardinals just for ease of comprehension
#[derive(AsRefStr, Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum Direction {
	#[default]
	X,
	N,
	NW,
	W,
	SW,
	S,
	SE,
	E,
	NE,
	UP,
	DOWN
}
impl fmt::Display for Direction {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let text: String = match self {
			Direction::X    => { "null_dir".to_string() }
			Direction::N    => { "North".to_string() }
			Direction::NW   => { "Northwest".to_string() }
			Direction::W    => { "West".to_string() }
			Direction::SW   => { "Southwest".to_string() }
			Direction::S    => { "South".to_string() }
			Direction::SE   => { "Southeast".to_string() }
			Direction::E    => { "East".to_string() }
			Direction::NE   => { "Northeast".to_string() }
			Direction::UP   => { "Up".to_string() }
			Direction::DOWN => { "Down".to_string() }
		};
		write!(f, "{}", text)
	}
}

// EOF
