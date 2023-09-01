// engine/handler.rs
// Provides the keyboard parser

use bevy::ecs::event::Events;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
// crossterm::KeyEvent: https://docs.rs/crossterm/latest/crossterm/event/struct.KeyEvent.html
// bevy::KeyboardInput: https://docs.rs/bevy/latest/bevy/input/keyboard/struct.KeyboardInput.html
use tui_textarea::{Key, Input};

use crate::components::*;
use crate::components::Direction;
use crate::engine::*;
use crate::engine::handler::ActionType::*;
use crate::engine::event::*;
use crate::engine::event::GameEventType::*;
use crate::engine::planq::*;
//use crate::engine::planq::PlanqEventType::*;

/// Parses the player inputs coming from ratatui and turns them into game logic
pub fn key_parser(key_event: KeyEvent, eng: &mut GameEngine) -> AppResult<()> {
	// WARN: STOP TRYING TO USE BEVY QUERIES IN THIS METHOD, it WILL cause ownership issues!
	// Either you meant to send a control command somewhere else,
	//  you forgot to defer/delegate the data query to a Bevy system,
	//  or if you're trying to control the GameEngine, consider abstracting up to the GameEngine
	/* Because it is implemented in crossterm via ratatui, making it into a Bevy system
	 * has so far been too difficult to finish, if not outright impossible
	 * The game_events object below will monopolize the mutable ref to the game world
	 * Therefore, do not try to extract and send info from here; defer to Bevy's event handling
	 */
	// *** DEBUG KEY HANDLING
	if (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('C'))
	&& key_event.modifiers == KeyModifiers::CONTROL {
		// Always allow the program to be closed via Ctrl-C
		eng.quit();
	}
	// Extract entity ids for the player and the player's planq
	let mut player_query = eng.bevy.world.query_filtered::<Entity, With<Player>>();
	let player_ref = player_query.get_single(&eng.bevy.world);
	let player = player_ref.unwrap_or(Entity::PLACEHOLDER);
	// *** GAME CONTROL HANDLING
	if eng.mode == EngineMode::Running {
		let mut new_game_event = GameEvent::new(GameEventType::NullEvent, Some(player), None);
		let mut new_planq_event = PlanqEvent::new(PlanqEventType::NullEvent);
		let planq = &mut eng.bevy.world.get_resource_mut::<PlanqData>().unwrap();
		// *** PLANQ CLI INPUT MODE
		if planq.show_cli_input {
			match key_event.code {
				// close the CLI, do not run anything
				KeyCode::Esc => { // Close and clear the input buffer
					planq.show_cli_input = false; // Need to force it closed immediately, the system updates don't seem to work for this
					new_planq_event.etype = PlanqEventType::CliClose; // Still going to generate the event in case I use it for a hook later
				}
				KeyCode::Enter => { // Dispatch the input buffer to the parser
					planq.show_cli_input = false;
					eng.planq_stdin.input.move_cursor(tui_textarea::CursorMove::Head);
					eng.planq_stdin.input.delete_line_by_end();
					let input_text = "> ".to_string() + eng.planq_stdin.input.yank_text();
					// We must finish working with the PLANQ reference before we can get the msglog
					if planq.cpu_mode == PlanqCPUMode::Idle {
						let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap(); // Must keep these here to satisfy borrow checker
						msglog.replace(input_text.clone(), "planq".to_string(), 0, 0);
					} else {
						let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap(); // See above ^^^
						msglog.tell_planq(input_text.clone());
					}
					eng.exec(planq_parser(input_text));
				}
				// TODO: set up the cursor dirs to allow movement? or reserve for planq menus?
				the_input => {
					// pass everything else to the CLI parser
					//eng.planq_stdin.input.input(key_event.clone().into());
					eprintln!("* attempting a translation of {:?} (todo)", the_input);
					let flag = eng.planq_stdin.input.input(
						Input {
							key: keycode_to_input_key(the_input),
							ctrl: false, // FIXME: probably want to detect this
							alt: false, // FIXME: probably want to detect this
						}
					);
					eprintln!("{}", eng.planq_stdin.input.lines()[0]);
					if flag { eprintln!("succeeded"); }
				}
			}
			return Ok(()) // WARN: do not disable this, lest key inputs be parsed twice (ie again below) by mistake!
		}
		// *** STANDARD GAME INPUTS
		match key_event.code {
			// Meta/menu controls
			KeyCode::Char('p') => { // Pause key toggle
				// Dispatch immediately, do not defer
				eng.pause_game();
				return Ok(())
			}
			KeyCode::Esc | KeyCode::Char('Q') => { // Close any open menus, or if none are open, open the main menu
				eng.menu_context.reset();
				if eng.visible_menu != MenuType::None {
					eng.visible_menu = MenuType::None;
				} else {
					eng.set_menu(MenuType::Main, (15, 15));
					eng.pause_game();
					return Ok(())
				}
			}
			KeyCode::Enter => {
				if eng.visible_menu == MenuType::Context {
					eng.menu_context.select();
					eng.visible_menu = MenuType::None;
					eng.menu_context.reset();
				}
			}
			// The cursor controls will be directed to any open menu before fallthru to player movement
			KeyCode::Left => {
				if eng.visible_menu == MenuType::Context {
					eng.menu_context.left();
				} else {
					new_game_event.etype = PlayerAction(MoveTo(Direction::W));
				}
			}
			KeyCode::Down => {
				if eng.visible_menu == MenuType::Context {
					eng.menu_context.down();
				} else {
					new_game_event.etype = PlayerAction(MoveTo(Direction::S));
				}
			}
			KeyCode::Up => {
				if eng.visible_menu == MenuType::Context {
					eng.menu_context.up();
				} else {
					new_game_event.etype = PlayerAction(MoveTo(Direction::N));
				}
			}
			KeyCode::Right => {
				if eng.visible_menu == MenuType::Context {
					eng.menu_context.right();
				} else {
					new_game_event.etype = PlayerAction(MoveTo(Direction::E));
				}
			}
			// Simple actions, no context required
			// The player movement controls will only operate menus if the game is Paused
			KeyCode::Char('h') => { new_game_event.etype = PlayerAction(MoveTo(Direction::W));}
			KeyCode::Char('j') => { new_game_event.etype = PlayerAction(MoveTo(Direction::S));}
			KeyCode::Char('k') => { new_game_event.etype = PlayerAction(MoveTo(Direction::N));}
			KeyCode::Char('l') => { new_game_event.etype = PlayerAction(MoveTo(Direction::E));}
			KeyCode::Char('y') => { new_game_event.etype = PlayerAction(MoveTo(Direction::NW));}
			KeyCode::Char('u') => { new_game_event.etype = PlayerAction(MoveTo(Direction::NE));}
			KeyCode::Char('b') => { new_game_event.etype = PlayerAction(MoveTo(Direction::SW));}
			KeyCode::Char('n') => { new_game_event.etype = PlayerAction(MoveTo(Direction::SE));}
			KeyCode::Char('>') => { new_game_event.etype = PlayerAction(MoveTo(Direction::DOWN));}
			KeyCode::Char('<') => { new_game_event.etype = PlayerAction(MoveTo(Direction::UP));}
			// Compound actions, context required: may require secondary inputs from player
			KeyCode::Char('i') => { // INVENTORY the player's possessions and allow selection
				let mut item_names = Vec::new();
				//eprintln!("* item_query: {:?}", item_query); // DEBUG: report size of item_query
				let mut backpack_query = eng.bevy.world.query_filtered::<(Entity, &Description, &Portable, &ActionSet), Without<Position>>();
				for item in backpack_query.iter(&eng.bevy.world) {
					//eprintln!("* found item {}", item.1.name.clone()); // DEBUG: report the item being worked on
					if item.2.carrier == player {
						let mut menu_entries = Vec::new();
						for action in item.3.actions.iter() {
							menu_entries.push(GameEvent::new(PlayerAction(*action), Some(player), Some(item.0)));
						}
						let submenu = make_new_submenu(menu_entries);
						//eprintln!("* Made submenu of size {} from {} actions", submenu.len(), item.3.actions.len()); // DEBUG: report submenu creation
						item_names.push(MenuItem::group(item.1.name.clone(), submenu));
					}
				}
				if item_names.is_empty() {
					//eprintln!("* Nothing in inventory to display"); // DEBUG: announce feedback
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("You are not carrying anything.".to_string());
					return Ok(())
				} else {
					//eprintln!("* Attempting to show_chooser()"); // DEBUG: announce attempt to show the context menu
					eng.menu_context = MenuState::new(item_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('d') => { // DROP an item from player's inventory
				let mut item_names = Vec::new();
				let mut backpack_query = eng.bevy.world.query_filtered::<(Entity, &Description, &Portable), Without<Position>>();
				for item in backpack_query.iter(&eng.bevy.world) {
					if item.2.carrier == player {
						item_names.push(MenuItem::item(
							item.1.name.clone(),
							GameEvent::new(PlayerAction(DropItem), Some(player), Some(item.0)),
							None,
							)
						);
					}
				}
				if item_names.is_empty() {
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("You have nothing to drop.".to_string());
					return Ok(())
				} else {
					eng.menu_context = MenuState::new(item_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('g') => { // GET an item from the ground
				let mut item_names = Vec::new();
				let mut item_query = eng.bevy.world.query::<(Entity, &Description, &Position, &Portable)>();
				let p_posn = eng.bevy.world.get_resource::<Position>().unwrap();
				for target in item_query.iter(&eng.bevy.world) {
					//eprintln!("* found item {}", target.1.name.clone()); // DEBUG: announce found targets for GET
					if target.2 == p_posn {
						item_names.push(MenuItem::item(
							target.1.name.clone(),
							GameEvent::new(PlayerAction(MoveItem), Some(player), Some(target.0)),
							None,
						));
					}
				}
				if item_names.is_empty() {
					//eprintln!("* Nothing to pick up at player's position"); // DEBUG: announce feedback
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing here to pick up.".to_string());
					return Ok(())
				} else {
					//eprintln!("* Attempting to set the entity menu"); // DEBUG: announce entity menu use
					eng.menu_context = MenuState::new(item_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('o') => { // OPEN an Openable item
				let mut item_names = Vec::new();
				let mut item_query = eng.bevy.world.query::<(Entity, &Description, &Position, &Openable)>();
				let p_posn = eng.bevy.world.get_resource::<Position>().unwrap();
				for target in item_query.iter(&eng.bevy.world) {
					//eprintln!("* found item {}", target.1.name.clone()); // DEBUG: report found OPENABLE items
					if target.2.is_adjacent_to(*p_posn) && !target.3.is_open {
						item_names.push(MenuItem::item(
								target.1.name.clone(),
								GameEvent::new(PlayerAction(OpenItem), Some(player), Some(target.0)),
								Some(*target.2)
							)
						);
					}
				}
				if item_names.is_empty() {
					//eprintln!("* Nothing to open nearby"); // DEBUG: announce feedback
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing nearby to open.".to_string());
					return Ok(())
				} else {
					//eprintln!("* Attempting to set the entity menu"); // DEBUG: announce entity menu use
					eng.menu_context = MenuState::new(item_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('c') => { // CLOSE an Openable nearby
				let mut item_names = Vec::new();
				let mut item_query = eng.bevy.world.query::<(Entity, &Description, &Position, &Openable)>();
				let p_posn = eng.bevy.world.get_resource::<Position>().unwrap();
				for target in item_query.iter(&eng.bevy.world) {
					//eprintln!("* found item {}", target.1.name.clone()); // DEBUG: report found closed OPENABLE items
					if target.2.is_adjacent_to(*p_posn) && target.3.is_open {
						item_names.push(MenuItem::item(
								target.1.name.clone(),
								GameEvent::new(PlayerAction(CloseItem), Some(player), Some(target.0)),
								Some(*target.2)
							)
						);
					}
				}
				if item_names.is_empty() {
					//eprintln!("* Nothing to close nearby"); // DEBUG: announce feedback
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing nearby to close.".to_string());
					return Ok(())
				} else {
					//eprintln!("* Attempting to set the entity menu"); // DEBUG: announce entity menu use
					eng.menu_context = MenuState::new(item_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('x') => { // EXAMINE a nearby Entity
				let mut enty_names = Vec::new();
				let mut enty_query = eng.bevy.world.query::<(Entity, &Description, &Position)>();
				let p_posn = eng.bevy.world.get_resource::<Position>().unwrap();
				for target in enty_query.iter(&eng.bevy.world) {
					//eprintln!("* Found target {}", target.1.name.clone()); // DEBUG: announce EXAMINE target
					if target.2.in_range_of(*p_posn, 2) {
						enty_names.push(MenuItem::item(
							target.1.name.clone(),
							GameEvent::new(PlayerAction(Examine), Some(player), Some(target.0)),
							Some(*target.2),
						));
					}
				}
				if enty_names.is_empty() {
					//eprintln!("* Nothing close enough to examine"); // DEBUG: report EXAMINE failure
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing nearby to examine.".to_string());
					return Ok(());
				} else {
					//eprintln!("* Attempting to set the entity menu with targets");// DEBUG: announce examine menu use
					eng.menu_context = MenuState::new(enty_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('a') => { // APPLY (use) an Operable item
				// Get a list of all Operable items in the player's vicinity
				let mut device_names = Vec::new();
				let mut device_query = eng.bevy.world.query::<(Entity, Option<&Position>, &Description, Option<&Portable>, &Device)>();
				let p_posn = *eng.bevy.world.get_resource::<Position>().unwrap();
				//eng.item_chooser.list.clear();
				// Drop them into one of the choosers
				for device in device_query.iter(&eng.bevy.world) {
					if device.3.is_some() { // Is the player carrying it?
						if device.3.unwrap().carrier == player {
							device_names.push(MenuItem::item(
								device.2.name.clone(),
								GameEvent::new(PlayerAction(UseItem), Some(player), Some(device.0)),
								None,
							));
						}
					} else if device.1.is_some() { // Is the player near it?
						if p_posn.in_range_of(*device.1.unwrap(), 1) {
							device_names.push(MenuItem::item(
								device.2.name.clone(),
								GameEvent::new(PlayerAction(UseItem), Some(player), Some(device.0)),
								None,
							));
						}
					}
				}
				if device_names.is_empty() {
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing nearby to use.".to_string());
					return Ok(())
				} else {
					eng.menu_context = MenuState::new(device_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('L') => { // LOCK a Lockable item
				let mut lock_names = Vec::new();
				let mut lock_query = eng.bevy.world.query::<(Entity, Option<&Position>, &Description, &Lockable)>();
				let p_posn = *eng.bevy.world.get_resource::<Position>().unwrap();
				for lock in lock_query.iter(&eng.bevy.world) {
					if let Some(l_posn) = lock.1 {
						if l_posn.in_range_of(p_posn, 1)
						&& lock.3.is_locked {
							lock_names.push(MenuItem::item(
								lock.2.name.clone(),
								GameEvent::new(PlayerAction(LockItem), Some(player), Some(lock.0)),
								None,
							));
						}
					}
				}
				if lock_names.is_empty() {
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing to lock nearby.".to_string());
					return Ok(())
				} else {
					eng.menu_context = MenuState::new(lock_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('U') => { // UNLOCK a Lockable item
				let mut lock_names = Vec::new();
				let mut lock_query = eng.bevy.world.query::<(Entity, Option<&Position>, &Description, &Lockable)>();
				let p_posn = *eng.bevy.world.get_resource::<Position>().unwrap();
				for lock in lock_query.iter(&eng.bevy.world) {
					if let Some(l_posn) = lock.1 {
						if !lock.3.is_locked
						&& l_posn.in_range_of(p_posn, 1) {
							lock_names.push(MenuItem::item(
								lock.2.name.clone(),
								GameEvent::new(PlayerAction(UnlockItem), Some(player), Some(lock.0)),
								None,
							));
						}
					}
				}
				if lock_names.is_empty() {
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing to unlock nearby.".to_string());
					return Ok(())
				} else {
					eng.menu_context = MenuState::new(lock_names);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('C') => { // CONNECT the PLANQ to a nearby AccessPort
				let mut access_ports = Vec::new();
				let mut port_query = eng.bevy.world.query_filtered::<(Entity, &Position, &Description), With<AccessPort>>();
				let p_posn = *eng.bevy.world.get_resource::<Position>().unwrap();
				for port in port_query.iter(&eng.bevy.world) {
					if *port.1 == p_posn {
						access_ports.push(MenuItem::item(
							port.2.name.clone(),
							GameEvent::new(PlanqConnect(port.0), Some(player), Some(port.0)), // NOTE: might want to swap player for planq here?
							None,
						));
					}
				}
				if access_ports.is_empty() {
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There are no access ports nearby.".to_string());
					return Ok(())
				} else {
					eng.menu_context = MenuState::new(access_ports);
					eng.set_menu(MenuType::Context, (15, 5));
				}
			}
			KeyCode::Char('D') => { // DISCONNECT the PLANQ from a connected AccessPort, if set
				if planq.jack_cnxn == Entity::PLACEHOLDER {
					// report "no connection" and abort the action
					let mut msglog = eng.bevy.world.get_resource_mut::<MessageLog>().unwrap();
					msglog.tell_player("There's nothing connected to your PLANQ.".to_string());
				} else {
					// disconnect the PLANQ
					new_game_event.etype = PlanqConnect(Entity::PLACEHOLDER);
					new_game_event.context = Some(GameEventContext{ subject: player, object: planq.jack_cnxn });
				}
			}
			// PLANQ 'sidebar'/ambient controls
			KeyCode::Char('P') | KeyCode::Char(':') => {
				if planq.cpu_mode == PlanqCPUMode::Idle || planq.cpu_mode == PlanqCPUMode::Working {
					new_planq_event.etype = PlanqEventType::CliOpen;
				}
			}
			// Debug keys and other tools
			KeyCode::Char('s') => { // DEBUG: Drop a generic snack item for testing
				eprintln!("* Dropping snack at 5, 5, 0"); // DEBUG: announce arrival of debug snack
				eng.make_item(ItemType::Snack, Position::create(5, 5, 0));
			}
			KeyCode::Char('S') => { // DEBUG: Give a snack to the player for testing
				eprintln!("* Giving snack to player"); // DEBUG: announce arrival of debug snack
				eng.give_item(ItemType::Snack, player);
			}
			_ => {
				eprintln!("* Unhandled key: {:?}", key_event.code); // DEBUG: report an unhandled key from this method
			}
		}
		// If an event was generated, send it off for processing
		if new_game_event.etype != GameEventType::NullEvent {
			// Get a linkage to the game event distribution system
			let game_events: &mut Events<GameEvent> = &mut eng.bevy.world.get_resource_mut::<Events<GameEvent>>().unwrap();
			game_events.send(new_game_event);
		}
		if new_planq_event.etype != PlanqEventType::NullEvent {
			let planq_events: &mut Events<PlanqEvent> = &mut eng.bevy.world.get_resource_mut::<Events<PlanqEvent>>().unwrap();
			planq_events.send(new_planq_event);
		}
	} else { // ALL OTHER SITUATIONS: Paused, Standby, etc
		match key_event.code {
			// Only handle these keys if the game's actually in-progress
			// Close open menus/unpause on Esc or Q
			KeyCode::Esc | KeyCode::Char('Q') => {
				//eng.menu_context.target = None; // Reset the targeting reticle
				eng.visible_menu = MenuType::None;
				eng.menu_main.reset();
				eng.menu_context.reset();
				eng.unpause_game();
				// Dispatch immediately
				return Ok(())
			}
			// Scroll the menu
			KeyCode::Char('h') | KeyCode::Left  => { eng.menu_main.left(); }
			KeyCode::Char('j') | KeyCode::Down  => { eng.menu_main.down(); }
			KeyCode::Char('k') | KeyCode::Up    => { eng.menu_main.up(); }
			KeyCode::Char('l') | KeyCode::Right => { eng.menu_main.right(); }
			// Confirm selection
			KeyCode::Enter => {
				eng.visible_menu = MenuType::None;
				eng.menu_main.select();
				if !eng.standby { eng.unpause_game(); }
				eng.menu_context.reset();
				return Ok(())
			}
			// Else, do nothing
			_ => { }
		}
	}
	Ok(())
}
/// Creates a new submenu given a Vec of the entries to put in it; note that only strings, Actions, and Entities are supported
pub fn make_new_submenu<T: std::fmt::Display>(entries: Vec<T>) -> Vec<MenuItem<T>> {
	let mut submenu = Vec::new();
	for item in entries {
		submenu.push(MenuItem::item(item.to_string(), item, None));
	}
	submenu.sort_by(|a, b| a.partial_cmp(b).unwrap());
	submenu
}
/// Converts my Event keycodes into tui_textarea::Input::Keys
pub fn keycode_to_input_key(key_code: KeyCode) -> Key {
	match key_code {
		KeyCode::Char(val)   => { Key::Char(val) }
		KeyCode::F(num)      => { Key::F(num) }
		KeyCode::Modifier(_) => { Key::Null } // NOTE: is this the ctrl/alt/whatever detection?
		KeyCode::Up          => { Key::Up }
		KeyCode::Down        => { Key::Down }
		KeyCode::Left        => { Key::Left }
		KeyCode::Right       => { Key::Right }
		KeyCode::Home        => { Key::Home }
		KeyCode::End         => { Key::End }
		KeyCode::PageUp      => { Key::PageUp }
		KeyCode::PageDown    => { Key::PageDown }
		KeyCode::Delete      => { Key::Delete }
		KeyCode::Backspace   => { Key::Backspace }
		KeyCode::Enter       => { Key::Enter }
		KeyCode::Esc         => { Key::Esc }
		KeyCode::Tab         => { Key::Tab }
		KeyCode::Insert      => { Key::Null } // Not supported by textarea
		KeyCode::BackTab     => { Key::Null } // Not supported by textarea
		KeyCode::CapsLock    => { Key::Null } // Not supported by textarea
		KeyCode::ScrollLock  => { Key::Null } // Not supported by textarea
		KeyCode::NumLock     => { Key::Null } // Not supported by textarea
		KeyCode::PrintScreen => { Key::Null } // Not supported by textarea
		KeyCode::Pause       => { Key::Null } // Not supported by textarea
		KeyCode::Menu        => { Key::Null } // Not supported by textarea
		KeyCode::KeypadBegin => { Key::Null } // Not supported by textarea
		KeyCode::Media(_)    => { Key::Null } // Not supported by textarea
		KeyCode::Null        => { Key::Null }
	}
}
/// Translates an input string from the player into a PLANQ command and context
pub fn planq_parser(input: String) -> PlanqCmd {
	let input_vec: Vec<&str> = input.trim_matches(|c| c == '>' || c == '¶').trim_start().split(' ').collect();
	//eprintln!("> {:?}", input_vec); // DEBUG:
	match input_vec[0] {
		"help" => { PlanqCmd::Help }
		"shutdown" => { PlanqCmd::Shutdown }
		"reboot" => { PlanqCmd::Reboot }
		"connect" => { PlanqCmd::Connect(input_vec[1].to_string()) }
		"disconnect" => { PlanqCmd::Disconnect }
		input => { PlanqCmd::Error(format!("Unknown command: {}", input)) } // No matching command was found!
	}
}

// EOF
