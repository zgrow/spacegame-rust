// sys.rs
// Defines the various subsystems we'll be running on the GameEngine
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::single_match)]

// NOTE: see bevy/examples/games/alien_cake_addict.rs for example on handling the Player entity

use crate::components::*;
use crate::camera_system::CameraView;
use crate::map::*;
use crate::components::{Name, Position, Renderable, Player, Mobile};
use crate::sys::event::*;
use crate::sys::{GameEventType::*, PlanqEventType::*};
use crate::app::messagelog::MessageLog;
use crate::app::planq::*;
use crate::app::*;
use crate::item_builders::*;
use bevy::ecs::system::{Commands, Res, Query, ResMut};
use bevy::ecs::event::EventReader;
use bevy::ecs::query::{With, Without, QueryEntityError};
use bevy::ecs::entity::Entity;
use bevy::utils::Duration;
use bevy::time::Time;
use bracket_pathfinding::prelude::*;
use bevy_turborand::prelude::*;

// TODO: Need to implement change detection on the following:
// map_indexing_system
// visibility_system

//  UTILITIES
/// Converts a spacegame::Position into a bracket_pathfinding::Point
pub fn posn_to_point(input: &Position) -> Point { Point { x: input.x, y: input.y } }

//  SINGLETON SYSTEMS (run once)
/// Spawns a new CameraView on the game world (ie the default/main view)
pub fn new_camera_system(mut commands: Commands) {
	commands.insert_resource(CameraView {
		map: Vec::new(),
		width: 0,
		height: 0,
	});
}
/// Spawns a new player, including their subsystems and default values
pub fn new_player_spawn(mut commands: Commands,
	                      mut msglog:   ResMut<MessageLog>,
	                      spawnpoint:   Res<Position>,
) {
	commands.spawn((
		// this is the player's collection of components and their initial values
		Player      { },
		Name        {name: "Pleyeur".to_string()},
		Position    {x: spawnpoint.x, y: spawnpoint.y, z: spawnpoint.z},
		Renderable  {glyph: "@".to_string(), fg: 2, bg: 0},
		Viewshed    {visible_tiles: Vec::new(), range: 8, dirty: true},
		Mobile      { },
		Obstructive { },
		Container   { contents: Vec::new() },
		Opaque      { opaque: true },
		CanOpen     { },
		CanOperate  { },
	));
	msglog.add("WELCOME TO SPACEGAME".to_string(), "world".to_string(), 1, 1);
}
/// Spawns a new LMR at the specified Position, using default values
pub fn new_lmr_spawn(mut commands:  Commands,
	                   mut msglog:    ResMut<MessageLog>,
) {
	commands.spawn((
		Name        {name: "LMR".to_string()},
		Position    {x: 12, y: 12, z: 0}, // TODO: remove magic numbers
		Renderable  {glyph: "l".to_string(), fg: 14, bg: 0},
		Viewshed    {visible_tiles: Vec::new(), range: 5, dirty: true},
		Mobile      { },
		Obstructive { },
		Opaque      { opaque: true },
		CanOpen     { },
		CanOperate  { },
	));
	msglog.add(format!("LMR spawned at {}, {}, {}", 12, 12, 0), "debug".to_string(), 1, 1);
}
/// Spawns the player's PLANQ [TODO: in the starting locker]
pub fn new_planq_spawn(mut commands:    Commands,
	                     mut msglog:      ResMut<MessageLog>,
	                     mut global_rng:  ResMut<GlobalRng>,
) {
	commands.spawn((
		Planq { },
		Thing {
			item: Item {
				name: Name { name: "PLANQ".to_string() },
				posn: Position::new(25, 30, 0),
				render: Renderable { glyph: "¶".to_string(), fg: 3, bg: 0 },
			},
			portable: Portable { carrier: Entity::PLACEHOLDER },
		},
		Device {
			pw_switch: false,
			batt_voltage: 100,
			batt_discharge: -1, // TODO: implement battery charge loss
			state: DeviceState::Offline, // TODO: sync this to the PLANQ's mode, don't try to use it!
		},
		RngComponent::from(&mut global_rng),
	));
	msglog.add(format!("planq spawned at {}, {}, {}", 25, 30, 0), "debug".to_string(), 1, 1);
}

//  CONTINUOUS SYSTEMS (run frequently)
/// Runs assessment of the game state for things like victory/defeat conditions, &c
pub fn engine_system(mut state:         ResMut<GameSettings>,
	                   mut ereader:       EventReader<GameEvent>,
	                   p_query:           Query<(Entity, &Position), With<Player>>,
	                   p_items_query:     Query<(Entity, &Portable), Without<Position>>,
	                   q_query:           Query<(Entity, &Portable), With<Planq>>,
) {
	for event in ereader.iter() {
		match event.etype {
			ModeSwitch(mode) => {// Immediately switch to the specified mode
				//eprintln!("Switching engine mode: {mode:?}"); // DEBUG:
				state.mode = mode;
			}
			PauseToggle => {
				//eprintln!("Pause toggled"); // DEBUG:
				if state.mode == EngineMode::Running { state.mode = EngineMode::Paused; }
				else if state.mode == EngineMode::Paused { state.mode = EngineMode::Running; }
			}
			_ => { } // Throw out all other event types
		}
	}
	// Check for the victory state
	let player = p_query.get_single().unwrap();
	let planq = q_query.get_single().unwrap();
	let mut p_inventory = Vec::new();
	for item in p_items_query.iter() {
		if item.1.carrier == player.0 { p_inventory.push(item.0); }
	}
	// version 0.1: Player must be standing in the specified Position
	//if *player.1 == Position::new(28, 1, 1) { state.mode = EngineMode::GoodEnd; }
	// version 0.2: v0.1 AND Player.has == planq
	// version 0.3: constraint: the Door to the Elevator is stuck shut, the Planq can reboot it
	if *player.1 == Position::new(28, 1, 1) && p_inventory.contains(&planq.0) {
		eprintln!("VICTORY condition achieved!"); // DEBUG:
		state.mode = EngineMode::GoodEnd;
		state.mode_changed = true;
	}
}
/// Handles entities that can move around the map
pub fn movement_system(mut ereader:     EventReader<GameEvent>,
	                     mut msglog:      ResMut<MessageLog>,
	                     mut p_posn_res:  ResMut<Position>,
	                     mut p_query:     Query<(&mut Position, &mut Viewshed), With<Player>>,
	                     model:           Res<Model>,
	                     enty_query:      Query<(&Position, &Name, Option<&mut Viewshed>), Without<Player>>,
) {
	// NOTE: the enty_query doesn't need to include Obstructive component because the map's
	// blocked_tiles sub-map already includes that information in an indexed vector
	// This allows us to only worry about consulting the query when we know we need it, as it is
	// much more expensive to iterate a query than to generate it
	for event in ereader.iter() {
		//eprintln!("player attempting to move"); // DEBUG:
		match event.etype {
			PlayerMove(dir) => {
				let mut feedback;
				let (mut p_pos, mut p_view) = p_query.single_mut();
				let mut xdiff = 0;
				let mut ydiff = 0;
				let mut zdiff = 0; // NOTE: not a typical component: z-level indexes to map stack
				match dir {
					Direction::N    =>             { ydiff -= 1 }
					Direction::NW   => { xdiff -= 1; ydiff -= 1 }
					Direction::W    => { xdiff -= 1 }
					Direction::SW   => { xdiff -= 1; ydiff += 1 }
					Direction::S    =>             { ydiff += 1 }
					Direction::SE   => { xdiff += 1; ydiff += 1 }
					Direction::E    => { xdiff += 1 }
					Direction::NE   => { xdiff += 1; ydiff -= 1 }
					Direction::UP   =>      { zdiff += 1 }
					Direction::DOWN =>      { zdiff -= 1 }
				}
				// Calculate the desired position's components
				let mut target = Position{x: p_pos.x + xdiff, y: p_pos.y + ydiff, z: p_pos.z + zdiff};
				let t_index = model.levels[target.z as usize].to_index(target.x, target.y);
				// NOTE: IF the actor is changing z-levels, some extra logic is required:
				if dir == Direction::UP || dir == Direction::DOWN {
					// Prevent movement if an invalid z-level was obtained
					if target.z < 0 || target.z as usize >= model.levels.len() { continue; }
					// Prevent movement if the entity is not on a stairway
					let p_index = model.levels[p_pos.z as usize].to_index(p_pos.x, p_pos.y);
					if model.levels[p_pos.z as usize].tiles[p_index].ttype != TileType::Stairway {
						feedback = "There is nothing here to ".to_string();
						if zdiff == 1 { feedback.push_str("ascend.") }
						else { feedback.push_str("descend.") }
						msglog.tell_player(feedback);
						continue;
					}
					// If we arrived here, then all's good; get the destination coords
					let possible = model.portals.get(&(p_pos.x, p_pos.y, p_pos.z));
					//eprintln!("poss: {possible:?}"); // DEBUG:
					if let Some(actual) = possible {
						target.x = actual.0;
						target.y = actual.1;
						target.z = actual.2;
					}
				}
				assert!(model.levels[target.z as usize].tiles.len() > 1, "Destination map is empty!");
				if model.levels[target.z as usize].blocked_tiles[t_index] {
					// Find out who's in the way and tell the player about it
					// CASE 1: there's an entity at that location
					for guy in enty_query.iter() {
						if guy.0 == &target {
							msglog.tell_player(format!("The way {} is blocked by a {}.", dir, guy.1));
							return;
						}
					}
					// CASE 2: it's a wall or similar
					msglog.tell_player(format!("The way {} is blocked by the {}.",
						              dir, &model.levels[target.z as usize].tiles[t_index].ttype.to_string()));
					return;
				}
				// If we arrived here, there's nothing in that space blocking the movement
				// Therefore, update the player's position
				(p_pos.x, p_pos.y, p_pos.z) = (target.x, target.y, target.z);
				// Don't forget to update the player position Resource too
				(p_posn_res.x, p_posn_res.y, p_posn_res.z) = (target.x, target.y, target.z);
				// Make sure the player's viewshed will be updated on the next pass
				p_view.dirty = true;
				// A tile's contents are implicitly defined as those non-blocking entities at a given Posn
				// If we use the player's position, then we may conclude that any entities at that
				// position that are not the player must be non-blocking, since the player's
				// movement rules prevent them from entering a tile with any other Obstructive enty
				let mut contents = Vec::new();
				for enty in enty_query.iter() {
					if *enty.0 == *p_pos {
						contents.push(&enty.1.name);
					}
				}
				if !contents.is_empty() {
					if contents.len() <= 3 {
						// Use a short summary
						let mut text = "There's a ".to_string();
						loop {
							text.push_str(contents.pop().unwrap());
							if contents.is_empty() { break; }
							else { text.push_str(", and a "); }
						}
					text.push_str(" here.");
					msglog.tell_player(text);
					} else {
						// Use a long summary
						msglog.tell_player("There's some stuff here on the ground.".to_string());
					}
				}
			}
			// TODO: this is where we'd handle an NPCMove action
			_ => { } // Throw out anything we're not specifically interested in
		}
	}
}
/// Handles updates to the 'meta' maps, ie the blocked and opaque tilemaps
pub fn map_indexing_system(mut model:         ResMut<Model>,
	                         mut blocker_query: Query<&Position, With<Obstructive>>,
	                         mut opaque_query:  Query<(&Position, &Opaque)>,
) {
	// TODO: consider possible optimization for not updating levels that the player is not on?
	// might require some extra smartness to allow updates if the LMR does something out of sight
	// First, rebuild the blocking map by the map tiles
	let mut f_index = 0;
	let mut index;
	for floor in model.levels.iter_mut() {
		floor.update_tilemaps(); // Update tilemaps based on their tiletypes
		// Then, step through all blocking entities and flag their locations on the map as well
		for guy in blocker_query.iter_mut() {
			if guy.z != f_index { continue; }
			index = floor.to_index(guy.x, guy.y);
			floor.blocked_tiles[index] = true;
		}
		// Do the same for the opaque entities
		for guy in opaque_query.iter_mut() {
			if guy.0.z != f_index { continue; }
			index = floor.to_index(guy.0.x, guy.0.y);
			floor.opaque_tiles[index] = guy.1.opaque;
		}
		f_index += 1;
	}
}
/// Handles CanOpen component action via ActorOpen/Close events
pub fn openable_system(mut commands:    Commands,
	                     mut ereader:     EventReader<GameEvent>,
	                     mut msglog:      ResMut<MessageLog>,
	                     mut door_query:  Query<(Entity, &Position, &mut Openable, &mut Renderable, &mut Opaque, Option<&Obstructive>)>,
	                     mut e_query:     Query<(Entity, &Position, &Name, Option<&Player>, Option<&mut Viewshed>), With<CanOpen>>,
) {
	for event in ereader.iter() {
		if event.etype != ActorOpen
		&& event.etype != ActorClose { continue; }
		if event.context.is_none() { continue; }
		let econtext = event.context.as_ref().unwrap();
		//eprintln!("actor opening door {0:?}", econtext.object); // DEBUG:
		let actor = e_query.get_mut(econtext.subject).unwrap();
		let player_action = actor.3.is_some();
		let mut message: String = "".to_string();
		match event.etype {
			GameEventType::ActorOpen => {
				//eprintln!("Trying to open a door"); // DEBUG:
				for mut door in door_query.iter_mut() {
					if door.0 == econtext.object {
						door.2.is_open = true;
						door.3.glyph = door.2.open_glyph.clone();
						door.4.opaque = false;
						commands.entity(door.0).remove::<Obstructive>();
					}
				}
				if player_action {
					message = "The door slides open at your touch.".to_string();
				} else {
					message = format!("The {} opens a door.", actor.2.name.clone());
				}
				if actor.4.is_some() { actor.4.unwrap().dirty = true; }
			}
			GameEventType::ActorClose => {
				//eprintln!("Trying to close a door"); // DEBUG:
				for mut door in door_query.iter_mut() {
					if door.0 == econtext.object {
						door.2.is_open = false;
						door.3.glyph = door.2.closed_glyph.clone();
						door.4.opaque = true;
						commands.entity(door.0).insert(Obstructive {});
					}
				}
				if player_action {
					message = "The door slides shut.".to_string();
				} else {
					message = format!("The {} closes a door.", actor.2.name.clone());
				}
				if actor.4.is_some() { actor.4.unwrap().dirty = true; }
			}
			_ => { }
		}
		if !message.is_empty() {
			msglog.tell_player(message);
		}
	}
}
/// Handles ActorLock/Unlock events
pub fn lock_system(mut _commands:    Commands,
                   mut ereader:     EventReader<GameEvent>,
                   mut msglog:      ResMut<MessageLog>,
                   mut lock_query:  Query<(Entity, &Position, &Name, &mut Lockable)>,
                   mut e_query:     Query<(Entity, &Position, &Name, Option<&Player>), With<CanOpen>>,
                   key_query:       Query<(Entity, &Portable, &Name, &Key), Without<Position>>,
) {
	for event in ereader.iter() {
		if event.etype != ActorLock
		&& event.etype != ActorUnlock { continue; }
		if event.context.is_none() { continue; }
		let econtext = event.context.as_ref().unwrap();
		let actor = e_query.get_mut(econtext.subject).unwrap();
		let player_action = actor.3.is_some();
		let mut target = lock_query.get_mut(econtext.object).unwrap();
		let mut message: String = "".to_string();
		match event.etype {
			ActorLock => {
				// TODO: obtain the new key value and apply it to the lock
				target.3.is_locked = true;
				if player_action {
					message = format!("You tap the LOCK button on the {}.", target.2.name.clone());
				} else {
					message = format!("The {} locks the {}.", actor.2.name.clone(), target.2.name.clone());
				}
			}
			ActorUnlock => {
				// Obtain the set of keys that the actor is carrying
				let mut carried_keys: Vec<(Entity, i32, String)> = Vec::new();
				for key in key_query.iter() {
					if key.1.carrier == actor.0 { carried_keys.push((key.0, key.3.key_id, key.2.name.clone())); }
				}
				if carried_keys.is_empty() { continue; } // no keys to try!
				// The actor has at least one key to try in the lock
				for key in carried_keys.iter() {
					if key.1 == target.3.key {
						// the subject has the right key, unlock the lock
						target.3.is_locked = false;
						if player_action {
							message = format!("Your {} unlocks the {}.", key.2, target.2.name.clone());
						} else {
							message = format!("The {} unlocks the {}.", actor.2.name.clone(), target.2.name.clone());
						}
					} else {
						// none of the keys worked, report a failure
						if player_action {
							message = "You don't seem to have the right key.".to_string();
						}
					}
				}
			}
			_ => { }
		}
		if !message.is_empty() {
			msglog.tell_player(message);
		}
	}
}
/// Handles anything related to the CanOperate component: ActorUse, ToggleSwitch, &c
pub fn operable_system(mut ereader: EventReader<GameEvent>,
                       //mut o_query: Query<(Entity, &Position, &Name), With<CanOperate>>,
                       mut d_query: Query<(Entity, &Name, &mut Device)>,
) {
	for event in ereader.iter() {
		if event.etype != ItemUse { continue; }
		let econtext = event.context.as_ref().unwrap();
		if econtext.is_invalid() { continue; }
		//let operator = o_query.get(econtext.subject).unwrap();
		let mut device = d_query.get_mut(econtext.object).unwrap();
		if !device.2.pw_switch { // If it's not powered on, assume that function first
			device.2.power_toggle();
		}
	}
}
/// Handles entities that can see physical light
pub fn visibility_system(mut model: ResMut<Model>,
	                     mut seers: Query<(&mut Viewshed, &Position, Option<&Player>)>
) {
	for (mut viewshed, posn, player) in &mut seers {
		//eprintln!("posn: {posn:?}"); // DEBUG:
		if viewshed.dirty {
			assert!(posn.z != -1);
			let map = &mut model.levels[posn.z as usize];
			viewshed.visible_tiles.clear();
			viewshed.visible_tiles = field_of_view(posn_to_point(posn), viewshed.range, map);
			viewshed.visible_tiles.retain(|p| p.x >= 0 && p.x < map.width
				                             && p.y >= 0 && p.y < map.height
			);
			if let Some(_player) = player { // if this is the player...
				for posn in &viewshed.visible_tiles { // For all the player's visible tiles...
					// ... set the corresponding tile in the map.revealed_tiles to TRUE
					let map_index = map.to_index(posn.x, posn.y);
					map.revealed_tiles[map_index] = true;
				}
			}
			viewshed.dirty = false;
		}
	}
}
/// Handles pickup/drop/destroy requests for Items
pub fn item_collection_system(mut commands: Commands,
	                            mut ereader:  EventReader<GameEvent>,
	                            mut msglog:   ResMut<MessageLog>,
	                            // The list of Entities that also have Containers
	                            e_query:      Query<(Entity, &Name, &Position, &Container, Option<&Player>)>,
	                            // The list of every Item that may or may not be in a container
	                            i_query:      Query<(Entity, &Name, &Portable, Option<&Position>)>,
) {
	for event in ereader.iter() {
		if event.etype != ItemMove
		&& event.etype != ItemDrop
		&& event.etype != ItemKILL { continue; }
		if event.context.is_none() { continue; }
		let econtext = event.context.as_ref().unwrap();
		if econtext.is_invalid() { continue; } // TODO: consider renaming this function...
		let mut message: String = "".to_string();
		let subject = e_query.get(econtext.subject).unwrap();
		let subject_name = subject.1.name.clone();
		let player_action = subject.4.is_some();
		let object = i_query.get(econtext.object).unwrap();
		let item_name = object.1.name.clone();
		match event.etype {
			ItemMove => { // Move an Item into an Entity's possession
				commands.entity(object.0)
				.insert(Portable{carrier: subject.0}) // put the container's ID to the target's Portable component
				.remove::<Position>(); // remove the Position component from the target
				// note that the above simply does nothing if it doesn't exist,
				// and inserting a Component that already exists overwrites the previous one,
				// so it's safe to call even on enty -> enty transfers
				if player_action {
					message = format!("Obtained a {}.", item_name);
				} else {
					message = format!("The {} takes a {}.", subject_name, item_name);
				}
			}
			ItemDrop => { // Remove an Item and place it into the World
				let location = subject.2;
				commands.entity(object.0)
				.insert(Portable{carrier: Entity::PLACEHOLDER}) // still portable but not carried
				.insert(Position{x: location.x, y: location.y, z: location.z});
				if player_action {
					message = format!("Dropped a {}.", item_name);
				} else {
					message = format!("The {} drops a {}.", subject_name, item_name);
				}
			}
			ItemKILL => { // DESTROY an Item entirely, ie remove it from the game
				commands.entity(econtext.object).despawn();
			}
			_ => { /* do nothing */ }
		}
		if !message.is_empty() {
			msglog.tell_player(message);
		}
	}
}
/// Allows us to run PLANQ updates and methods in their own thread, just like a real computer~
pub fn planq_system(mut commands:   Commands,
	                  mut ereader:    EventReader<GameEvent>,
	                  mut preader:    EventReader<PlanqEvent>,
	                  time:           Res<Time>,
	                  mut msglog:     ResMut<MessageLog>,
	                  mut planq:      ResMut<PlanqData>, // contains the PLANQ's settings and data storage
	                  mut monitor:    ResMut<PlanqMonitor>, // contains the PLANQ's status bar info
	                  p_query:        Query<(Entity, &Position), With<Player>>, // provides interface to player data
	                  i_query:        Query<(Entity, &Portable), Without<Position>>,
	                  mut q_query:    Query<(Entity, &Planq, &Device, &mut RngComponent)>, // contains the PLANQ's component data
	                  mut t_query:    Query<(Entity, &mut PlanqProcess)>, // contains the set of all PlanqTimers
) {
	/* TODO: Implement level generation such that the whole layout can be created at startup from a
	 * tree of rooms, rather than by directly loading a REXPaint map; by retaining this tree-list
	 * of rooms in the layout, the PLANQ can then show the player's location as a room name
	 */
	// Update the planq's settings if there are any changes queued up
	let player = p_query.get_single().unwrap();
	let planq_enty = q_query.get_single_mut().unwrap();
	//let planq_enty = q_query.get_single().unwrap();
	let mut refresh_inventory = false;
	// Handle any new comms
	for event in ereader.iter() { // First, handle any GameEvents
		match event.etype {
			// Player interaction events that need to be monitored
			ItemMove => { // The player (g)ot the PLANQ from somewhere external
				let econtext = event.context.as_ref().unwrap();
				if econtext.subject == player.0 {
					refresh_inventory = true;
					if econtext.object == planq_enty.0 {
						planq.is_carried = true;
						commands.spawn(DataSampleTimer::new().source("planq_mode".to_string()));
						commands.spawn(DataSampleTimer::new().source("current_time".to_string()));
						commands.spawn(DataSampleTimer::new().source("planq_battery".to_string()));
					}
				}
			}
			ItemDrop => { // The player (d)ropped the PLANQ
				let econtext = event.context.as_ref().unwrap();
				if econtext.subject == player.0 { refresh_inventory = true; }
				if econtext.object == planq_enty.0 { planq.is_carried = false; }
			}
			ItemUse => { // The player (a)pplied the PLANQ
				let econtext = event.context.as_ref().unwrap();
				if econtext.subject == player.0
				&& econtext.object == planq_enty.0 {
					// Note that the Operable system already handles the ItemUse action for the
					// PLANQ: it allows the player to operate the power switch
					// This seems likely to change in the future to allow some better service
					// commands, like battery swaps or peripheral attachment
					msglog.tell_player("There is a faint 'click' as you press the PLANQ's power button.".to_string());
				}
			}
			_ => { }
		}
	}
	for event in preader.iter() { // Then, handle any PlanqEvents
		match event.etype {
			// PLANQ system commands
			PlanqEventType::NullEvent => { /* do nothing */ }
			Startup => { planq.cpu_mode = PlanqCPUMode::Startup; } // covers the entire boot stage
			BootStage(lvl) => { planq.boot_stage = lvl; }
			Shutdown => { planq.cpu_mode = PlanqCPUMode::Shutdown; }
			Reboot => { /* do a Shutdown, then a Startup */ }
			GoIdle => { planq.cpu_mode = PlanqCPUMode::Idle; }
			CliOpen => {
				if planq.cpu_mode != PlanqCPUMode::Startup
				&& planq.cpu_mode != PlanqCPUMode::Shutdown
				&& planq.cpu_mode != PlanqCPUMode::Offline {
					planq.show_cli_input = true;
					planq.action_mode = PlanqActionMode::CliInput;
				}
			}
			CliClose => {
				// FIXME: need to clear the CLI's input buffer! might need to do this at the time of key input?
				planq.show_cli_input = false;
				planq.action_mode = PlanqActionMode::Default; // FIXME: this might be a bad choice
			}
			InventoryUse => {
				planq.inventory_toggle(); // display the inventory menu
				planq.action_mode = PlanqActionMode::UseItem;
			}
			InventoryDrop => {
				planq.inventory_toggle(); // display the inventory menu
				planq.action_mode = PlanqActionMode::DropItem;
			}
		}
	}
	// Update the PLANQData resources:
	// - Get the device hardware info
	if !planq.power_is_on && planq_enty.2.pw_switch { // If the power's off but the switch is set to ON...
		planq.power_is_on = planq_enty.2.pw_switch; // Update the power switch setting
		//planq.output_1_enabled = true; // DEBUG:
		planq.show_terminal = true;
		planq.cpu_mode = PlanqCPUMode::Startup; // Begin booting the PLANQ's OS
	}
	if planq.power_is_on && !planq_enty.2.pw_switch { // If the power's on but the switch is set to OFF...
		planq.power_is_on = planq_enty.2.pw_switch; // Update the power switch setting
		planq.cpu_mode = PlanqCPUMode::Shutdown; // Initiate a shutdown
	}
	// HINT: Get the current battery voltage with planq_enty.2.batt_voltage
	// - Handle the Planq's CPU mode logic
	// TODO: catch the edge case where the proc_table.len() == 0 but the CPUMode != Offline/Startup/Error/&c
	match planq.cpu_mode {
		PlanqCPUMode::Error(_) => { /* TODO: implement Error modes */ }
		PlanqCPUMode::Offline => { /* do nothing */ }
		PlanqCPUMode::Startup => {
			// do the boot process: send outputs, progress bars, the works
			// then kick over to PAM::Idle
			if !planq.proc_table.is_empty() {
				// if there are any running processes, check to see if they're done
				for id in planq.proc_table.clone() {
					let enty = t_query.get(id).unwrap();
					if enty.1.timer.just_finished() {
						match enty.1.outcome.etype {
							BootStage(lvl) => {
								planq.boot_stage = lvl;
							}
							PlanqEventType::GoIdle => { planq.cpu_mode = PlanqCPUMode::Idle; }
							_ => { }
						}
					}
				}
			}
			// Get proc 0, aka the boot process
			let proc_ref = if !planq.proc_table.is_empty() {
				t_query.get_mut(planq.proc_table[0])
			} else {
				Err(QueryEntityError::NoSuchEntity(Entity::PLACEHOLDER))
			};
			// NOTE: might be able to deal with some formatting if i rewrote the boot msgs in a Table
			match planq.boot_stage {
				0 => {
					if planq.proc_table.is_empty() {
						eprintln!("¶ running boot stage {}", planq.boot_stage);
						msglog.boot_message(planq.boot_stage);
						// kick off boot stage 1
						planq.proc_table.push(commands.spawn(
								PlanqProcess::new()
								.time(3)
								.event(PlanqEvent::new(PlanqEventType::BootStage(1))))
							.id()
						);
					}
				}
				1 => {
					if let Ok(mut proc) = proc_ref {
						if proc.1.timer.just_finished() {
							eprintln!("¶ running boot stage {}", planq.boot_stage);
							msglog.boot_message(planq.boot_stage);
							// set its duration, if needed
							//proc.1.timer.set_duration(Duration::from_secs(5));
							// reset it
							proc.1.timer.reset(); // will be iterated on at next system run
							proc.1.outcome = PlanqEvent::new(PlanqEventType::BootStage(2));
						}
					}
				}
				2 => {
					if let Ok(mut proc) = proc_ref {
						if proc.1.timer.just_finished() {
							eprintln!("¶ running boot stage {}", planq.boot_stage);
							msglog.boot_message(planq.boot_stage);
							// set its duration, if needed
							//proc.1.timer.set_duration(Duration::from_secs(5));
							// reset it and start it
							proc.1.timer.reset(); // will be iterated on at next system run
							proc.1.outcome = PlanqEvent::new(PlanqEventType::BootStage(3));
						}
					}
				}
				3 => {
					if let Ok(mut proc) = proc_ref {
						if proc.1.timer.just_finished() {
							eprintln!("¶ running boot stage {}", planq.boot_stage);
							msglog.boot_message(planq.boot_stage);
							// set its duration, if needed
							//proc.1.timer.set_duration(Duration::from_secs(5));
							// reset it and start it
							proc.1.timer.reset(); // will be iterated on at next system run
							proc.1.outcome = PlanqEvent::new(PlanqEventType::BootStage(4));
						}
					}
				}
				4 => {
					if let Ok(mut proc) = proc_ref {
						if proc.1.timer.just_finished() {
							eprintln!("¶ running boot stage {}", planq.boot_stage);
							msglog.boot_message(planq.boot_stage);
							// TODO: implement an add() method on the monitor: monitor.add("player_location");
							// DEBUG: these status bars are for testing/debugging
							monitor.status_bars.push("player_location".to_string());
							monitor.raw_data.insert("player_location".to_string(), PlanqDataType::Text("".to_string()));
							commands.spawn(DataSampleTimer::new().source("player_location".to_string()));
							//monitor.status_bars.push("test_line".to_string());
							//monitor.raw_data.insert("test_line".to_string(), PlanqDataType::Decimal {numer: 1, denom: 10});
							//commands.spawn(DataSampleTimer::new().duration(2).source("test_line".to_string()));
							//monitor.status_bars.push("test_sparkline".to_string());
							//monitor.raw_data.insert("test_sparkline".to_string(), PlanqDataType::Series(VecDeque::new()));
							//commands.spawn(DataSampleTimer::new().duration(1).source("test_sparkline".to_string()));
							//monitor.status_bars.push("test_gauge".to_string());
							//monitor.raw_data.insert("test_gauge".to_string(), PlanqDataType::Percent(0));
							//commands.spawn(DataSampleTimer::new().duration(3).source("test_gauge".to_string()));
							// END DEBUG:
							proc.1.outcome = PlanqEvent::new(PlanqEventType::NullEvent);
							planq.cpu_mode = PlanqCPUMode::Idle;
							eprintln!("¶ Adding status bars to PLANQ");
							// TODO: ensure that the status bar stack is cleaned up on PLANQ shutdown
						}
					}
				}
				_ => { }
			}
		}
		PlanqCPUMode::Shutdown => {
			// TODO: Make sure the proc_table is clear
			// Set the CPU's mode
			// When finished, set the power_is_on AND planq_enty.2.pw_switch to false
		}
		PlanqCPUMode::Idle => {
			// given a sequence of integers 0-9,
			// produce a smoothly scaled integer 1-21
			let smooth_input = (time.elapsed().as_secs() % 10) as f64;
			//let angle: f64 = 0.6282 * smooth_input - 1.571;
			//let output = (10.5 * angle.sin() + 10.5) as usize;
			let output = (4.4 * smooth_input - 23.0).abs() as usize;
			let idle_message = format!("{:width$}", "", width=output) + "-=[ ]=-";
			if planq.proc_table.len() == 1 { // Is there anything besides the boot process running?
				// update the idle graphic if we're still idling, or send a new one if not
				msglog.replace(idle_message, "planq".to_string(), 0, 0);
			}
			// FIXME: what to do if CPUMode = Idle but proc_table.len() > 1 (there are tasks to finish)?
		}
		PlanqCPUMode::Working => {
			// TODO: Display the outputs from the workloads
			// ...
			// Finally, if all the workloads are done, shift back to Idle mode
			if planq.proc_table.len() == 1 { planq.cpu_mode = PlanqCPUMode::Idle; }
		}
	}
	// - Iterate any active PlanqProcesses
	for mut pq_timer in t_query.iter_mut() {
		if !pq_timer.1.timer.finished() {
			pq_timer.1.timer.tick(time.delta());
		}
	}
	// - Refill the planq's inventory list
	if refresh_inventory {
		planq.inventory_list = Vec::new();
		for item in i_query.iter().enumerate() {
			if item.1.1.carrier == player.0 {
				planq.inventory_list.push(item.1.0);
			}
		}
	}
}
/// Handles the 'backend' automated stuff for the PLANQ, such as the status bars
pub fn planq_monitor_system(time:           Res<Time>,
	                          msglog:         Res<MessageLog>,
	                          mut planq:      ResMut<PlanqData>, // contains the PLANQ's settings and data storage
	                          mut monitor:    ResMut<PlanqMonitor>, // contains the PLANQ's status bar info
	                          p_query:        Query<(Entity, &Position), With<Player>>, // provides interface to player data
	                          mut q_query:    Query<(Entity, &Planq, &Device, &mut RngComponent)>, // contains the PLANQ's component data
	                          mut s_query:    Query<(Entity, &mut DataSampleTimer)>, // the set of datasources that need updates
) {
	let player = p_query.get_single().unwrap();
	let mut planq_enty = q_query.get_single_mut().unwrap();
	// - Refresh the planq's scrollback
	// TODO: optimize this to avoid doing a full copy of the log every single time
	planq.stdout = msglog.get_log_as_messages("planq".to_string(), 0);
	// - Get the player's location
	planq.player_loc = *player.1;
	// - Update the status bar data from externals
	// METHOD
	// 1 Get the list of active status bars from the PLANQ monitor
	// 2 Incr all timers in the list
	// 3 If any timers have finished, call the data source's update method, then reset the timer
	for mut data_timer in s_query.iter_mut() {
		if data_timer.1.timer.finished() {
			let source_name = data_timer.1.source.clone();
			match source_name.as_str() {
				// START HERE: all of the other cases below need to be revised to match this method call pattern in "planq_mode"
				"planq_mode" => {
					monitor.raw_data.entry(source_name).and_modify(|x| *x = PlanqDataType::Text(planq.cpu_mode.to_string()));
				}
				"player_location" => {
					monitor.raw_data.entry(source_name).and_modify(|x| *x = PlanqDataType::Text(planq.player_loc.to_string()));
				}
				"current_time" => { // TODO: this needs to be rewritten into a 24h clock, not a stopwatch
					let start_time_offset = Duration::new(2096, 789); // 12:34:56.789
					let current_time = time.elapsed() + start_time_offset;
					monitor.raw_data.entry(source_name).and_modify(|x| *x = PlanqDataType::Text(current_time.get_as_string()));
				}
				"planq_battery" => {
					monitor.raw_data.entry(source_name)
						.and_modify(|x| *x = PlanqDataType::Percent(planq_enty.2.batt_voltage as u32));
				}
				"test_line" => {
					monitor.raw_data.entry(source_name)
						.and_modify(|x| *x = PlanqDataType::Decimal {numer: planq_enty.3.i32(0..100), denom: 100});
				}
				"test_sparkline" => {
					// This update method is a little 'backwards': instead of passing a new value to raw_data via entry(),
					// we modify the raw_data's values directly using the mutable reference we obtain with get_mut()
					let entry = monitor.raw_data.get_mut(&source_name).unwrap();
					if let PlanqDataType::Series(ref mut arr) = entry {
						arr.push_back(planq_enty.3.u64(0..10));
						loop {
							if arr.len() >= 31 {
								arr.pop_front();
							} else {
								break;
							}
						}
					}
				}
				"test_gauge" => {
					monitor.raw_data.entry(source_name)
					.and_modify(|x| *x = PlanqDataType::Percent(planq_enty.3.u32(0..=100)));
				}
				_ => {  }
			}
			data_timer.1.timer.reset();
		} else {
			data_timer.1.timer.tick(time.delta());
		}
	}
}

/// This is a lil reverse-trait/extension trait that provides some shorthand for the Duration type provided by Bevy
/// Defining a trait on an external type like this allows the trait methods to be called on instances of the type as self
/// Note that this does not change any of the scope hierarchy; the only methods callable here are the public methods defined
/// by the Display type
/// The concept has two parts:
/// 1) Define a new trait with the signatures of the desired methods
/// 2) Implement the new trait T on the external type Y: 'impl T for Y { ... }'
/// source: http://xion.io/post/code/rust-extension-traits.html
pub trait DurationFmtExt {
	fn get_as_string(self) -> String;
	fn get_as_msecs(self) -> u128;
}
impl DurationFmtExt for Duration {
	/// Provides the time as a preformatted string, suitable for display.
	fn get_as_string(self) -> String {
		let mut secs = self.as_secs();
		let mils = self.subsec_millis();
		let hours: u64 = secs / 3600;
		secs -= hours * 3600;
		let mins: u64 = secs / 60;
		secs -= mins * 60;
		format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, mils)
	}
	/// Provides the current ship time as a raw quantity of milliseconds, suitable for doing maths to.
	fn get_as_msecs(self) -> u128 {
		self.as_millis()
	}
}

/* TODO: "memory_system":
 * Maintains an enhanced Map of Tiles where the Tile glyphs are painted to include the locations of
 * existing Renderables in addition to the terrain
 * When this system is initialized (after the initial level setup, before the disaster design
 * phase), it provides a 'prior memory' of the ship layout
 * When this system is updated, it provides the player with a visual mapping of where to find
 * complex machines and other gameplay objectives
 */

// EOF
