use std::{
	collections::{HashMap, HashSet},
	io::{self, Read},
	os::fd::OwnedFd,
};

use bitflags::bitflags;

use crate::xkb::{CompatSelector, XKBType, XkbKeyMap};

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct BuiltinModifiers: u32 {
		const SHIFT   = 0x01;
		const LOCK    = 0x02;
		const CONTROL = 0x04;
		const MOD1    = 0x08;
		const MOD2    = 0x10;
		const MOD3    = 0x20;
		const MOD4    = 0x40;
		const MOD5    = 0x80;
	}
}

#[derive(Debug, Clone)]
pub struct Modifiers {
	pub modifiers: BuiltinModifiers,
	pub virtual_modifiers: Vec<String>,
}

impl Modifiers {
	pub fn new() -> Self {
		Self {
			modifiers: BuiltinModifiers::empty(),
			virtual_modifiers: Vec::new(),
		}
	}

	pub fn is_active(&self, modifier: &str) -> bool {
		match modifier {
			"all" => self.modifiers.bits() != 0 || !self.virtual_modifiers.is_empty(),
			"Shift" => self.modifiers.contains(BuiltinModifiers::SHIFT),
			"Lock" => self.modifiers.contains(BuiltinModifiers::LOCK),
			"Control" => self.modifiers.contains(BuiltinModifiers::CONTROL),
			"Mod1" => self.modifiers.contains(BuiltinModifiers::MOD1),
			"Mod2" => self.modifiers.contains(BuiltinModifiers::MOD2),
			"Mod3" => self.modifiers.contains(BuiltinModifiers::MOD3),
			"Mod4" => self.modifiers.contains(BuiltinModifiers::MOD4),
			"Mod5" => self.modifiers.contains(BuiltinModifiers::MOD5),
			v => self.virtual_modifiers.contains(&v.to_string()),
		}
	}

	pub fn set_modifier(&mut self, modifier: &str, active: bool) {
		match modifier {
			"Shift" => self.modifiers.set(BuiltinModifiers::SHIFT, active),
			"Lock" => self.modifiers.set(BuiltinModifiers::LOCK, active),
			"Control" => self.modifiers.set(BuiltinModifiers::CONTROL, active),
			"Mod1" => self.modifiers.set(BuiltinModifiers::MOD1, active),
			"Mod2" => self.modifiers.set(BuiltinModifiers::MOD2, active),
			"Mod3" => self.modifiers.set(BuiltinModifiers::MOD3, active),
			"Mod4" => self.modifiers.set(BuiltinModifiers::MOD4, active),
			"Mod5" => self.modifiers.set(BuiltinModifiers::MOD5, active),
			v => {
				if active && !self.virtual_modifiers.contains(&v.to_string()) {
					self.virtual_modifiers.push(v.to_string());
				} else if !active {
					self.virtual_modifiers.retain(|m| m != v);
				}
			}
		}
	}

	fn active_names(&self) -> HashSet<String> {
		let mut out = HashSet::new();

		if self.modifiers.contains(BuiltinModifiers::SHIFT) {
			out.insert("Shift".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::LOCK) {
			out.insert("Lock".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::CONTROL) {
			out.insert("Control".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::MOD1) {
			out.insert("Mod1".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::MOD2) {
			out.insert("Mod2".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::MOD3) {
			out.insert("Mod3".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::MOD4) {
			out.insert("Mod4".to_string());
		}
		if self.modifiers.contains(BuiltinModifiers::MOD5) {
			out.insert("Mod5".to_string());
		}

		for name in &self.virtual_modifiers {
			out.insert(name.clone());
		}

		out
	}
}

struct State {
	modifiers: Modifiers,
	latched_modifiers: Modifiers,
	locked_modifiers: Modifiers,
	group: u32,
	latched_group: Option<GroupTransform>,
	locked_group: Option<GroupTransform>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupTransform {
	Absolute(u32),
	Relative(i32),
}

impl State {
	fn compute_level_for_type_def(&self, xkb_type: &XKBType) -> usize {
		let active = self.effective_modifier_set();

		let relevant: std::collections::HashSet<String> = active
			.into_iter()
			.filter(|m| xkb_type.modifiers.iter().any(|tm| tm == m))
			.collect();

		let mut best_level_1_based: u32 = 1;
		let mut best_specificity = 0usize;

		for (map_key, level) in &xkb_type.map {
			let required: std::collections::HashSet<String> = map_key
				.split('+')
				.map(|s| s.trim().to_string())
				.filter(|s| !s.is_empty())
				.collect();

			if required.iter().all(|m| relevant.contains(m)) && required.len() >= best_specificity {
				best_specificity = required.len();
				best_level_1_based = *level;
			}
		}

		best_level_1_based.saturating_sub(1) as usize
	}

	fn is_alphabetic_pair(map: &[crate::xkb::KeySym]) -> bool {
		if map.len() < 2 {
			return false;
		}

		let Some(base) = map[0].to_char() else {
			return false;
		};
		let Some(shifted) = map[1].to_char() else {
			return false;
		};

		base.is_ascii_lowercase() && shifted == base.to_ascii_uppercase()
	}

	// For keys without an explicit type, we have to work out what level they should use based on the active modifiers
	// and the available levels in the keymap. This mostly just means trying to guess whether Shift is active, but
	// some keys have more complex behavior so we try to handle those too.
	fn infer_level_without_explicit_type(&self, config: &XkbKeyMap, map: &[crate::xkb::KeySym]) -> usize {
		if map.is_empty() {
			return 0;
		}

		let is_alpha = Self::is_alphabetic_pair(map);
		let mut best: Option<(usize, usize)> = None;

		for xkb_type in config.types.types.values() {
			let level = self.compute_level_for_type_def(xkb_type);
			if level >= map.len() {
				continue;
			}

			let max_level = xkb_type.map.iter().map(|(_, l)| *l as usize).max().unwrap_or(1);

			let has_shift = xkb_type.modifiers.iter().any(|m| m == "Shift");
			let lock_maps_to_level2 = xkb_type
				.map
				.iter()
				.any(|(mods, l)| *l == 2 && mods.split('+').any(|m| m.trim() == "Lock"));

			let mut score = 0usize;
			if max_level >= map.len() {
				score += 4;
			}
			if map.len() >= 2 && has_shift {
				score += 2;
			}
			if is_alpha && lock_maps_to_level2 {
				score += 3;
			}
			if xkb_type.modifiers.iter().any(|m| m == "LevelThree") && map.len() >= 3 {
				score += 1;
			}

			if best.is_none_or(|(best_score, _)| score > best_score) {
				best = Some((score, level));
			}
		}

		best.map(|(_, level)| level).unwrap_or(0)
	}

	fn set_raw_modifiers(&mut self, modifiers: u32, latched: u32, locked: u32) {
		self.modifiers.modifiers = BuiltinModifiers::from_bits_truncate(modifiers);
		self.latched_modifiers.modifiers = BuiltinModifiers::from_bits_truncate(latched);
		self.locked_modifiers.modifiers = BuiltinModifiers::from_bits_truncate(locked);
	}

	fn parse_group_transform(group_str: &str) -> Option<GroupTransform> {
		if let Ok(group) = group_str.parse::<u32>() {
			return Some(GroupTransform::Absolute(group));
		}

		if let Some(plus) = group_str.strip_prefix('+')
			&& let Ok(delta) = plus.parse::<i32>()
		{
			return Some(GroupTransform::Relative(delta));
		}

		if let Some(minus) = group_str.strip_prefix('-')
			&& let Ok(delta) = minus.parse::<i32>()
		{
			return Some(GroupTransform::Relative(-delta));
		}

		None
	}

	fn apply_group_transform(group: u32, transform: GroupTransform) -> u32 {
		match transform {
			GroupTransform::Absolute(value) => value,
			GroupTransform::Relative(delta) if delta >= 0 => group.saturating_add(delta as u32),
			GroupTransform::Relative(delta) => group.saturating_sub((-delta) as u32),
		}
	}

	fn consume_latched(&mut self) {
		self.latched_modifiers = Modifiers::new();
		self.latched_group = None;
	}

	fn compute_level_for_type_name(&self, config: &XkbKeyMap, type_name: &str) -> usize {
		let Some(xkb_type) = config.types.types.get(type_name) else {
			return 0;
		};
		self.compute_level_for_type_def(xkb_type)
	}

	fn handle_set_mods(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if let Some(mods) = args.get("modifiers") {
			for mod_name in mods.split('+') {
				self.modifiers.set_modifier(mod_name.trim(), pressed);
			}
		}

		if args.get("clearLocks").is_some() {
			self.locked_modifiers = Modifiers::new();
		}
	}

	fn handle_latch_mods(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if !pressed {
			return;
		}

		if let Some(mods) = args.get("modifiers") {
			for mod_name in mods.split('+') {
				if self.latched_modifiers.is_active(mod_name) && args.get("latchToLock").is_some() {
					self.latched_modifiers.set_modifier(mod_name.trim(), false);
					self.locked_modifiers.set_modifier(mod_name.trim(), true);
				} else {
					self.latched_modifiers.set_modifier(mod_name.trim(), true);
				}
			}
		}

		if args.get("clearLocks").is_some() {
			self.locked_modifiers = Modifiers::new();
		}
	}

	fn handle_lock_mods(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if !pressed {
			return;
		}

		if let Some(mods) = args.get("modifiers") {
			for mod_name in mods.split('+') {
				let mod_name = mod_name.trim();
				let currently_locked = self.locked_modifiers.is_active(mod_name);
				self.locked_modifiers.set_modifier(mod_name, !currently_locked);
			}
		}
	}

	fn handle_set_group(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if !pressed {
			return;
		}

		if let Some(group_str) = args.get("group")
			&& let Some(transform) = Self::parse_group_transform(group_str)
		{
			self.group = Self::apply_group_transform(self.group, transform);
		}
	}

	fn handle_latch_group(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if !pressed {
			return;
		}

		if let Some(group_str) = args.get("group") {
			self.latched_group = Self::parse_group_transform(group_str);
		}
	}

	fn handle_lock_group(&mut self, args: &HashMap<String, String>, pressed: bool) {
		if !pressed {
			return;
		}

		if let Some(group_str) = args.get("group")
			&& let Some(transform) = Self::parse_group_transform(group_str)
		{
			if self.locked_group == Some(transform) {
				self.locked_group = None;
			} else {
				self.locked_group = Some(transform);
			}
		}
	}

	fn effective_group(&self) -> u32 {
		let mut group = self.group;

		if let Some(locked) = self.locked_group {
			group = Self::apply_group_transform(group, locked);
		}

		if let Some(latched) = self.latched_group {
			group = Self::apply_group_transform(group, latched);
		}

		group
	}

	fn effective_modifier_set(&self) -> HashSet<String> {
		let mut out = self.modifiers.active_names();
		out.extend(self.latched_modifiers.active_names());
		out.extend(self.locked_modifiers.active_names());
		out
	}

	fn selector_arg_set(args: Option<&Vec<String>>) -> HashSet<String> {
		args.map(|v| v.iter().map(|s| s.trim().to_string()).collect())
			.unwrap_or_default()
	}

	pub fn matches_selector(&self, selector: &CompatSelector) -> bool {
		let active = self.effective_modifier_set();
		let required = Self::selector_arg_set(selector.selector_args.as_ref());
		let has_all = required.contains("all");

		match selector.selector_func.as_deref() {
			Some("AnyOf") => {
				if has_all {
					!active.is_empty()
				} else {
					required.iter().any(|m| active.contains(m))
				}
			}
			Some("AnyOfOrNone") => {
				if has_all {
					true
				} else {
					let any_of = required.iter().any(|m| active.contains(m));
					let none = required.iter().all(|m| !active.contains(m));
					any_of || none
				}
			}
			Some("Exactly") => {
				if has_all {
					false
				} else {
					active == required
				}
			}
			None => true,
			_ => false,
		}
	}
}

pub struct Keyboard {
	state: State,
	configuration: Option<XkbKeyMap>,
}

impl Keyboard {
	pub fn new() -> Self {
		Self {
			state: State {
				modifiers: Modifiers::new(),
				latched_modifiers: Modifiers::new(),
				locked_modifiers: Modifiers::new(),
				group: 1,
				latched_group: None,
				locked_group: None,
			},
			configuration: None,
		}
	}

	pub fn set_raw_modifiers(&mut self, modifiers: u32, latched: u32, locked: u32) {
		self.state.set_raw_modifiers(modifiers, latched, locked);
	}

	pub fn load_configuration(&mut self, config: OwnedFd) -> io::Result<()> {
		let mut raw_config_file = std::fs::File::from(config);
		let mut raw_config = String::new();
		raw_config_file.read_to_string(&mut raw_config)?;
		let config = XkbKeyMap::from_str(&raw_config).map_err(io::Error::other)?;
		self.configuration = Some(config);
		Ok(())
	}

	pub fn handle_event(&mut self, keycode: u32, pressed: bool) -> Option<crate::xkb::KeySym> {
		if let Some(config) = &self.configuration {
			let mut saw_latch_action = false;

			let xkb_code = keycode + 8; // XKB keycodes are offset by 8
			let xkb_key_name = config
				.key_codes
				.keys
				.iter()
				.find(|(_, code)| *code == xkb_code)
				.map(|(name, _)| name.clone())?;

			let alias = config.key_codes.aliases.get(&xkb_key_name);
			let key_sym = match config.symbols.keys.get(&xkb_key_name) {
				Some(symbols) => symbols,
				None => alias.and_then(|alias| config.symbols.keys.get(alias))?,
			};

			let group_string = format!("Group{}", self.state.effective_group());
			let map = if !key_sym.symbols.is_empty() {
				&key_sym.symbols
			} else if let Some(map) = key_sym.grouped_symbols.get(&group_string) {
				map
			} else {
				key_sym.grouped_symbols.get("Group1")?
			};

			let explicit_type = key_sym
				.grouped_type_names
				.get(&group_string)
				.or(key_sym.type_name.as_ref())
				.or_else(|| config.symbols.grouped_key_type_names.get(&group_string))
				.or(config.symbols.key_type_name.as_ref())
				.cloned();

			let level = if let Some(type_name) = explicit_type {
				self.state.compute_level_for_type_name(config, &type_name)
			} else {
				self.state.infer_level_without_explicit_type(config, map)
			};
			let selected_keysym = if level >= map.len() {
				map.last().unwrap()
			} else {
				&map[level]
			};
			let key_name = selected_keysym.name();

			if let Some(compat_entries) = config.compat.entries.get(key_name) {
				for compat_entry in compat_entries {
					let selector_matches = compat_entry
						.selector
						.as_ref()
						.map(|s| self.state.matches_selector(s))
						.unwrap_or(true);

					if !selector_matches {
						continue;
					}

					match compat_entry.action.as_str() {
						"SetMods" => self.state.handle_set_mods(&compat_entry.args, pressed),
						"LatchMods" => {
							saw_latch_action = true;
							self.state.handle_latch_mods(&compat_entry.args, pressed)
						}
						"LockMods" => self.state.handle_lock_mods(&compat_entry.args, pressed),
						"SetGroup" => self.state.handle_set_group(&compat_entry.args, pressed),
						"LatchGroup" => {
							saw_latch_action = true;
							self.state.handle_latch_group(&compat_entry.args, pressed)
						}
						"LockGroup" => self.state.handle_lock_group(&compat_entry.args, pressed),
						_ => {}
					}

					break;
				}
			}

			if pressed && !saw_latch_action {
				self.state.consume_latched();
			}

			return Some(*selected_keysym);
		}

		None
	}
}
