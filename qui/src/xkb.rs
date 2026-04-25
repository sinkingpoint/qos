use std::collections::HashMap;

use thiserror::Error;

pub struct XkbKeyCodes {
	minimum: u32,
	maximum: u32,
	keys: Vec<(String, u32)>,
	aliases: HashMap<String, String>,
	indicator: HashMap<u32, String>,
}

impl XkbKeyCodes {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut minimum = None;
		let mut maximum = None;
		let mut keys = Vec::new();
		let mut aliases = HashMap::new();
		let mut indicator = HashMap::new();
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			if lexer.consume_literal("minimum") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes minimum".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let minimum_str = lexer.consume_int();
				minimum = Some(minimum_str.parse().map_err(|_| {
					XkbError::ExpectedInt(
						lexer.position(),
						"key codes minimum".to_string(),
						minimum_str.to_string(),
					)
				})?);
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes minimum".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("maximum") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes maximum".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let maximum_str = lexer.consume_int();
				maximum = Some(maximum_str.parse().map_err(|_| {
					XkbError::ExpectedInt(
						lexer.position(),
						"key codes maximum".to_string(),
						maximum_str.to_string(),
					)
				})?);
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes maximum".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("alias") {
				let name = lexer.consume_until(|c| c == '=').trim().to_string();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes alias".to_string(),
						"=".to_string(),
					));
				}

				let value = lexer.consume_until(|c| c == ';').trim().to_string();
				aliases.insert(name, value);
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes maximum".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("indicator") {
				// 6= "foo"
				lexer.skip_whitespace();
				let indicator_index_str = lexer.consume_int();
				let indicator_index = indicator_index_str.parse().map_err(|_| {
					XkbError::ExpectedInt(
						lexer.position(),
						"key codes indicator index".to_string(),
						indicator_index_str.to_string(),
					)
				})?;

				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						format!("key codes indicator for index {}", indicator_index),
						"=".to_string(),
					));
				}

				lexer.skip_whitespace();
				let indicator_name = lexer.consume_until(|c| c == ';').trim_matches('"').to_string();
				indicator.insert(indicator_index, indicator_name);
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes maximum".to_string(),
						";".to_string(),
					));
				}
			} else {
				let key_name = lexer.consume_until(|c| c == '=').trim().to_string();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let key_code_str = lexer.consume_int();
				let key_code = key_code_str.parse().map_err(|_| {
					XkbError::ExpectedInt(
						lexer.position(),
						format!("key codes: {}", key_code_str),
						key_code_str.to_string(),
					)
				})?;
				keys.push((key_name, key_code));
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"key codes".to_string(),
						";".to_string(),
					));
				}
			}
		}

		Ok(Self {
			minimum: minimum.ok_or_else(|| {
				XkbError::ExpectedInt(lexer.position(), "key codes minimum".to_string(), "minimum".to_string())
			})?,
			maximum: maximum.ok_or_else(|| {
				XkbError::ExpectedInt(lexer.position(), "key codes maximum".to_string(), "maximum".to_string())
			})?,
			aliases,
			indicator,
			keys,
		})
	}
}

// modifiers = none; level_name[1] = "Any";
// modifiers = Shift+Lock; map[Shift] = 2; map[Lock] = 2; preserve[Lock] = Lock; level_name[1] = "Base"; level_name[2] = "Caps";
pub struct XKBType {
	modifiers: Vec<String>,
	map: Vec<(String, u32)>,
	preserve: Vec<(String, String)>,
	level_names: Vec<String>,
}

impl XKBType {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut modifiers = Vec::new();
		let mut map = Vec::new();
		let mut preserve = Vec::new();
		let mut level_names = Vec::new();

		lexer.skip_whitespace();
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			if lexer.consume_literal("modifiers") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"modifiers".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				modifiers = lexer
					.consume_until(|c| c == ';')
					.split('+')
					.map(|s| s.trim().to_string())
					.collect();
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type modifiers".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("map") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("[") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type map".to_string(),
						"[".to_string(),
					));
				}
				lexer.skip_whitespace();
				let modifier_name = lexer.consume_until(|c| c == ']').trim().to_string();
				if !lexer.consume_literal("]") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type map".to_string(),
						"]".to_string(),
					));
				}

				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type map".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let level_str = lexer.consume_int();
				let level = level_str.parse().map_err(|_| {
					XkbError::ExpectedInt(lexer.position(), "type map level".to_string(), level_str.to_string())
				})?;
				map.push((modifier_name, level));
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type map".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("preserve") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("[") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type preserve".to_string(),
						"[".to_string(),
					));
				}
				lexer.skip_whitespace();
				let modifier_name = lexer.consume_until(|c| c == ']').trim().to_string();
				if !lexer.consume_literal("]") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type preserve".to_string(),
						"]".to_string(),
					));
				}

				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type preserve".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let preserve_type = lexer.consume_until(|c| c == ';').trim().to_string();
				preserve.push((modifier_name, preserve_type));
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type preserve".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("level_name") {
				if !lexer.consume_literal("[") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type level name".to_string(),
						"[".to_string(),
					));
				}
				let level_index_str = lexer.consume_int();
				let level_index = level_index_str.parse().map_err(|_| {
					XkbError::ExpectedInt(
						lexer.position(),
						"type level name index".to_string(),
						level_index_str.to_string(),
					)
				})?;
				if !lexer.consume_literal("]") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type level name".to_string(),
						"]".to_string(),
					));
				}
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type level name".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let level_name = lexer.consume_until(|c| c == ';').trim_matches('"').to_string();
				if level_names.len() < level_index {
					level_names.resize(level_index, String::new());
				}
				level_names[level_index - 1] = level_name;
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"type level name".to_string(),
						";".to_string(),
					));
				}
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"type".to_string(),
					format!("Unknown field: {}", lexer.consume_while(|_| true)),
				));
			}
			lexer.skip_whitespace();
		}

		Ok(Self {
			modifiers,
			map,
			preserve,
			level_names,
		})
	}
}

// action = SetMods(modifiers=Shift);
pub struct XKBCompatEntry {
	action: String,
	args: HashMap<String, String>,
}

impl XKBCompatEntry {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut action = None;
		let mut args = HashMap::new();
		let mut virtual_modifier = None;
		let mut use_mod_maps = None;
		let mut repeat = None;
		lexer.skip_whitespace();
		while lexer.has_more() {
			if lexer.consume_literal("useModMapMods") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry useModMapMods".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				use_mod_maps = Some(lexer.consume_until(|c| c == ';').trim().to_string());
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry useModMapMods".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("repeat") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry repeat".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				repeat = Some(lexer.consume_until(|c| c == ';').trim().to_string());
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry repeat".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("virtualModifier") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry virtualModifier".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				virtual_modifier = Some(lexer.consume_until(|c| c == ';').trim().to_string());
				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry virtualModifier".to_string(),
						";".to_string(),
					));
				}
			} else if lexer.consume_literal("action") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry action".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				let action_str = lexer.consume_until(|c| c == '(' || c == ';').trim().to_string();
				let action_args = if lexer.consume_literal("(") {
					let args_str = lexer.consume_until(|c| c == ')');
					let mut args = HashMap::new();
					for arg in args_str.split(',') {
						let parts: Vec<&str> = arg.split('=').map(|s| s.trim()).collect();
						if parts.len() == 1 {
							args.insert(parts[0].to_string(), String::new());
							continue;
						} else if parts.len() != 2 {
							return Err(XkbError::ExpectedLiteral(
								lexer.position(),
								"compat entry action args".to_string(),
								"key=value".to_string(),
							));
						}
						args.insert(parts[0].to_string(), parts[1].to_string());
					}
					args
				} else {
					HashMap::new()
				};

				lexer.skip_whitespace();
				if !lexer.consume_literal(")") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry action args".to_string(),
						")".to_string(),
					));
				}

				lexer.skip_whitespace();
				if !lexer.consume_literal(";") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat entry action args".to_string(),
						";".to_string(),
					));
				}
				action = Some(action_str);
				args = action_args;
			} else {
				let next = lexer.consume_until(|c| c == ';').trim().to_string();
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"compat entry field".to_string(),
					format!("virtualModifier, useModMaps, repeat, or action, got '{}'", next),
				));
			}
			lexer.skip_whitespace();
		}

		Ok(Self {
			action: action.ok_or_else(|| {
				XkbError::ExpectedLiteral(
					lexer.position(),
					"compat entry action".to_string(),
					"action".to_string(),
				)
			})?,
			args,
		})
	}
}

pub struct XKBSymbolsKey {
	type_name: Option<String>,
	symbols: Vec<String>,
}

//  type="TWO_LEVEL", [ 1, exclam ]
//  [ Escape ]
impl XKBSymbolsKey {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut type_name = None;
		let mut symbols = Vec::new();
		lexer.skip_whitespace();
		if lexer.consume_literal("type") {
			lexer.skip_whitespace();
			if !lexer.consume_literal("=") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols key type".to_string(),
					"=".to_string(),
				));
			}

			lexer.skip_whitespace();
			if !lexer.consume_literal("\"") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols key type".to_string(),
					"\"".to_string(),
				));
			}

			let type_name_str = lexer.consume_until(|c| c == '"');
			type_name = Some(type_name_str.to_string());
			if !lexer.consume_literal("\"") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols key type".to_string(),
					"\"".to_string(),
				));
			}
			lexer.skip_whitespace();
			if !lexer.consume_literal(",") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols key type".to_string(),
					",".to_string(),
				));
			}
		}

		lexer.skip_whitespace();
		if !lexer.consume_literal("[") {
			return Err(XkbError::ExpectedLiteral(
				lexer.position(),
				"symbols key type".to_string(),
				"[".to_string(),
			));
		}

		while lexer.has_more() {
			lexer.skip_whitespace();
			let symbol = lexer.consume_until(|c| c == ',' || c == ']').trim().to_string();
			symbols.push(symbol);
			lexer.skip_whitespace();
			if lexer.consume_literal(",") {
				continue;
			} else if lexer.consume_literal("]") {
				break;
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols key".to_string(),
					", or ]".to_string(),
				));
			}
		}

		Ok(Self { type_name, symbols })
	}
}

pub struct XkbSymbolsModifierMap {
	keys: Vec<String>,
}

impl XkbSymbolsModifierMap {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut keys = Vec::new();
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			let key = lexer.consume_until(|c| c == ',' || c == ';').trim().to_string();
			keys.push(key);
			lexer.skip_whitespace();
			if !lexer.consume_literal(",") {
				break;
			}
		}

		Ok(Self { keys })
	}
}

pub struct XkbSymbols {
	keys: HashMap<String, XKBSymbolsKey>,
	modifier_maps: HashMap<String, XkbSymbolsModifierMap>,
}

// key <CAPS> { [ Caps_Lock ] };
// modifier_map Shift   { <LFSH>, <RTSH> };
impl XkbSymbols {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut keys = HashMap::new();
		let mut modifier_maps = HashMap::new();
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			if lexer.consume_literal("key") {
				let name = lexer.consume_until(|c| c == '{').trim().to_string();
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(lexer.position(), "key block".to_string(), "key block".to_string())
				})?;
				let symbols_key = XKBSymbolsKey::parse(block)?;
				keys.insert(name, symbols_key);
			} else if lexer.consume_literal("modifier_map") {
				let name = lexer.consume_until(|c| c == '{').trim().to_string();
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"modifier_map block".to_string(),
						"modifier_map block".to_string(),
					)
				})?;
				let modifier_map = XkbSymbolsModifierMap::parse(block)?;
				modifier_maps.insert(name, modifier_map);
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols".to_string(),
					"key or modifier_map".to_string(),
				));
			}

			if !lexer.consume_literal(";") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"symbols".to_string(),
					";".to_string(),
				));
			}
			lexer.skip_whitespace();
		}
		Ok(Self { keys, modifier_maps })
	}
}

pub struct XKBCompatIndicatorEntry {
	which_mod_state: Option<String>,
	modifiers: Vec<String>,
	groups: Option<u32>,
	controls: Vec<String>,
}

impl XKBCompatIndicatorEntry {
	//  indicator "Shift Lock" {
	//               whichModState= locked;
	//               modifiers= Shift;
	//       };
	//       indicator "Group 2" {
	//               groups= 0xfe;
	//       };
	//       indicator "Mouse Keys" {
	//               controls= MouseKeys;
	//       };
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut which_mod_state = None;
		let mut modifiers = Vec::new();
		let mut groups = None;
		let mut controls = Vec::new();
		lexer.skip_whitespace();
		while lexer.has_more() {
			if lexer.consume_literal("whichModState") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat indicator whichModState".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				which_mod_state = Some(lexer.consume_until(|c| c == ';').trim().to_string());
			} else if lexer.consume_literal("modifiers") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat indicator modifiers".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				modifiers = lexer
					.consume_until(|c| c == ';')
					.split('+')
					.map(|s| s.trim().to_string())
					.collect();
			} else if lexer.consume_literal("groups") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat indicator groups".to_string(),
						"=".to_string(),
					));
				}
				let groups_str = lexer.consume_until(|c| c == ';').trim().to_string();
				let groups_int = if let Some(hex_str) = groups_str.strip_prefix("0x") {
					u32::from_str_radix(hex_str, 16)
				} else {
					groups_str.parse()
				};
				groups = Some(groups_int.map_err(|_| {
					XkbError::ExpectedInt(lexer.position(), "compat indicator groups".to_string(), groups_str)
				})?);
			} else if lexer.consume_literal("controls") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat indicator controls".to_string(),
						"=".to_string(),
					));
				}
				lexer.skip_whitespace();
				controls = lexer
					.consume_until(|c| c == ';')
					.split('+')
					.map(|s| s.trim().to_string())
					.collect();
			} else {
				let next = lexer.consume_until(|c| c == ';').trim().to_string();
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"compat indicator field".to_string(),
					format!("whichModState, modifiers, groups, or controls, found '{}'", next),
				));
			}
			lexer.skip_whitespace();
			if !lexer.consume_literal(";") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"compat indicator controls".to_string(),
					";".to_string(),
				));
			}
			lexer.skip_whitespace();
		}

		Ok(Self {
			which_mod_state,
			modifiers,
			groups,
			controls,
		})
	}
}

pub struct XkbCompat {
	entries: HashMap<String, XKBCompatEntry>,
	virtual_modifiers: Option<Vec<String>>,
}

impl XkbCompat {
	// interpret Shift_L   { action = SetMods(modifiers=Shift); }
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut entries = HashMap::new();
		let mut virtual_modifiers = None;
		let mut interpret_args = HashMap::new();
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			if lexer.consume_literal("virtual_modifiers") {
				lexer.skip_whitespace();
				virtual_modifiers = Some(
					lexer
						.consume_until(|c| c == ';')
						.split(',')
						.map(|s| s.trim().to_string())
						.collect(),
				);
			} else if lexer.consume_literal("interpret.") {
				// interpret.<argName> = <value>;
				let arg_name = lexer.consume_until(|c| c == '=').trim().to_string();
				if !lexer.consume_literal("=") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"compat interpret arg".to_string(),
						"=".to_string(),
					));
				}

				let arg_value = lexer.consume_until(|c| c == ';').trim().to_string();
				interpret_args.insert(arg_name, arg_value);
			} else if lexer.consume_literal("interpret") {
				let name = lexer.consume_until(|c| c == '{').trim().to_string();
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"interpret block".to_string(),
						"interpret block".to_string(),
					)
				})?;
				let compat_entry = XKBCompatEntry::parse(block)?;
				entries.insert(name, compat_entry);
			} else if lexer.consume_literal("indicator") {
				let name = lexer.consume_until(|c| c == '{').trim().to_string();
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"indicator block".to_string(),
						"indicator block".to_string(),
					)
				})?;
				let compat_entry = XKBCompatIndicatorEntry::parse(block)?;
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"compat".to_string(),
					"interpret".to_string(),
				));
			}

			if !lexer.consume_literal(";") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"compat".to_string(),
					";".to_string(),
				));
			}
			lexer.skip_whitespace();
		}

		Ok(Self {
			entries,
			virtual_modifiers,
		})
	}
}

// type "ONE_LEVEL"  { modifiers = none; level_name[1] = "Any"; };
pub struct XkbTypes {
	types: HashMap<String, XKBType>,
	virtual_modifiers: Vec<String>,
}

impl XkbTypes {
	pub fn parse(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		let mut types = HashMap::new();
		let mut virtual_modifiers = None;
		while lexer.has_more() {
			lexer.skip_whitespace();
			if !lexer.has_more() {
				break;
			}
			if lexer.consume_literal("virtual_modifiers") {
				virtual_modifiers = Some(
					lexer
						.consume_until(|c| c == ';')
						.split(",")
						.map(|s| s.to_owned())
						.collect(),
				);
				lexer.skip_whitespace();
			} else if lexer.consume_literal("type") {
				lexer.skip_whitespace();
				if !lexer.consume_literal("\"") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"types type".to_string(),
						"\"".to_string(),
					));
				}
				let name = lexer.consume_until(|c| c == '"').to_string();
				if !lexer.consume_literal("\"") {
					return Err(XkbError::ExpectedLiteral(
						lexer.position(),
						"types type".to_string(),
						"\"".to_string(),
					));
				}

				lexer.skip_whitespace();
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(lexer.position(), "type block".to_string(), "type block".to_string())
				})?;
				let xkb_type = XKBType::parse(block)?;
				types.insert(name, xkb_type);
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"type".to_string(),
					"type".to_string(),
				));
			}

			if !lexer.consume_literal(";") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"type".to_string(),
					";".to_string(),
				));
			}
			lexer.skip_whitespace();
		}
		Ok(Self {
			types,
			virtual_modifiers: virtual_modifiers
				.ok_or_else(|| XkbError::ExpectedLiteral(lexer.position(), "type".to_string(), ";".to_string()))?,
		})
	}
}

pub struct XkbKeyMap {
	key_codes: XkbKeyCodes,
	types: XkbTypes,
	compat: XkbCompat,
	symbols: XkbSymbols,
}

impl XkbKeyMap {
	pub fn from_str(input: &str) -> Result<Self, XkbError> {
		let mut lexer = Lexer::new(input);
		lexer.skip_whitespace();
		if !lexer.consume_literal("xkb_keymap") {
			return Err(XkbError::ExpectedLiteral(
				lexer.position(),
				"xkb_keymap".to_string(),
				"xkb_keymap".to_string(),
			));
		}
		lexer.skip_whitespace();
		let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
			XkbError::ExpectedLiteral(
				lexer.position(),
				"xkb_keymap".to_string(),
				"xkb_keymap block".to_string(),
			)
		})?;
		Self::parse(block)
	}

	fn parse(input: &str) -> Result<Self, XkbError> {
		let mut key_codes = None;
		let mut types = None;
		let mut compat = None;
		let mut symbols = None;
		let mut lexer = Lexer::new(input);
		lexer.skip_whitespace();
		while lexer.has_more() {
			if lexer.consume_literal("xkb_keycodes") {
				lexer.skip_whitespace();
				if lexer.consume_literal("\"") {
					// Parse the optional name
					let _name = lexer.consume_until(|c| c == '"').to_string();
					if !lexer.consume_literal("\"") {
						return Err(XkbError::ExpectedLiteral(
							lexer.position(),
							"xkb_keycodes".to_string(),
							"\"".to_string(),
						));
					}
					lexer.skip_whitespace();
				}
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"xkb_keycodes block".to_string(),
						"xkb_keycodes block".to_string(),
					)
				})?;
				key_codes = Some(XkbKeyCodes::parse(block)?);
			} else if lexer.consume_literal("xkb_types") {
				lexer.skip_whitespace();
				if lexer.consume_literal("\"") {
					// Parse the optional name
					let _name = lexer.consume_until(|c| c == '"').to_string();
					if !lexer.consume_literal("\"") {
						return Err(XkbError::ExpectedLiteral(
							lexer.position(),
							"keycodes types".to_string(),
							"\"".to_string(),
						));
					}
					lexer.skip_whitespace();
				}
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"xkb_types block".to_string(),
						"xkb_types block".to_string(),
					)
				})?;
				types = Some(XkbTypes::parse(block)?);
			} else if lexer.consume_literal("xkb_compatibility") || lexer.consume_literal("xkb_compat") {
				lexer.skip_whitespace();
				if lexer.consume_literal("\"") {
					// Parse the optional name
					let _name = lexer.consume_until(|c| c == '"').to_string();
					if !lexer.consume_literal("\"") {
						return Err(XkbError::ExpectedLiteral(
							lexer.position(),
							"keymap compat".to_string(),
							"\"".to_string(),
						));
					}
					lexer.skip_whitespace();
				}
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"xkb_compat block".to_string(),
						"xkb_compat block".to_string(),
					)
				})?;
				compat = Some(XkbCompat::parse(block)?);
			} else if lexer.consume_literal("xkb_symbols") {
				lexer.skip_whitespace();
				if lexer.consume_literal("\"") {
					// Parse the optional name
					let _name = lexer.consume_until(|c| c == '"').to_string();
					if !lexer.consume_literal("\"") {
						return Err(XkbError::ExpectedLiteral(
							lexer.position(),
							"keymap symbols".to_string(),
							"\"".to_string(),
						));
					}
					lexer.skip_whitespace();
				}
				let block = lexer.consume_until_matching('{', '}').ok_or_else(|| {
					XkbError::ExpectedLiteral(
						lexer.position(),
						"xkb_symbols block".to_string(),
						"xkb_symbols block".to_string(),
					)
				})?;
				symbols = Some(XkbSymbols::parse(block)?);
			} else {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"keymap".to_string(),
					"xkb_keycodes, xkb_types, xkb_compat, or xkb_symbols".to_string(),
				));
			}

			lexer.skip_whitespace();
			if !lexer.consume_literal(";") {
				return Err(XkbError::ExpectedLiteral(
					lexer.position(),
					"keymap".to_string(),
					";".to_string(),
				));
			}
			lexer.skip_whitespace();
		}

		Ok(Self {
			key_codes: key_codes.ok_or_else(|| {
				XkbError::ExpectedLiteral(
					lexer.position(),
					"xkb_keycodes block".to_string(),
					"xkb_keycodes block".to_string(),
				)
			})?,
			types: types.ok_or_else(|| {
				XkbError::ExpectedLiteral(
					lexer.position(),
					"xkb_types block".to_string(),
					"xkb_types block".to_string(),
				)
			})?,
			compat: compat.ok_or_else(|| {
				XkbError::ExpectedLiteral(
					lexer.position(),
					"xkb_compat block".to_string(),
					"xkb_compat block".to_string(),
				)
			})?,
			symbols: symbols.ok_or_else(|| {
				XkbError::ExpectedLiteral(
					lexer.position(),
					"xkb_symbols block".to_string(),
					"xkb_symbols block".to_string(),
				)
			})?,
		})
	}
}

struct Lexer<'a> {
	input: &'a str,
	position: usize,
}

impl<'a> Lexer<'a> {
	fn new(input: &'a str) -> Lexer<'a> {
		Lexer { input, position: 0 }
	}

	fn peek(&self) -> Option<char> {
		self.input[self.position..].chars().next()
	}

	fn consume_literal(&mut self, literal: &str) -> bool {
		if self.input[self.position..].starts_with(literal) {
			self.position += literal.len();
			true
		} else {
			false
		}
	}

	fn consume_while<F: Fn(char) -> bool>(&mut self, condition: F) -> &'a str {
		let start = self.position;
		while let Some(c) = self.peek() {
			if !condition(c) {
				break;
			}
			self.position += c.len_utf8();
		}
		&self.input[start..self.position]
	}

	fn consume_until_matching(&mut self, open: char, close: char) -> Option<&'a str> {
		if !self.consume_literal(&open.to_string()) {
			return None;
		}
		let mut depth = 0;
		let start = self.position;
		while let Some(c) = self.peek() {
			if c == open {
				depth += 1;
			} else if c == close {
				if depth == 0 {
					let result = &self.input[start..self.position];
					self.position += close.len_utf8();
					return Some(result);
				} else {
					depth -= 1;
				}
			}
			self.position += c.len_utf8();
		}

		None
	}

	fn consume_until<F: Fn(char) -> bool>(&mut self, condition: F) -> &'a str {
		let start = self.position;
		while let Some(c) = self.peek() {
			if condition(c) {
				break;
			}
			self.position += c.len_utf8();
		}
		&self.input[start..self.position]
	}

	fn consume_int(&mut self) -> &'a str {
		self.consume_while(|c| c.is_digit(10))
	}

	fn skip_whitespace(&mut self) {
		while self.position < self.input.len() && self.input[self.position..].starts_with(char::is_whitespace) {
			self.position += 1;
		}
	}

	fn has_more(&self) -> bool {
		self.position < self.input.len()
	}

	fn position(&self) -> usize {
		self.position
	}
}

#[derive(Debug, Error)]
pub enum XkbError {
	#[error("Expected integer at position {0} while parsing {1} but found invalid value: {2}")]
	ExpectedInt(usize, String, String),

	#[error("Expected literal '{2}' at position {0} while parsing {1} but not found")]
	ExpectedLiteral(usize, String, String),
}

mod tests {
	#[test]
	fn test_xkb_key_codes() {
		let input = "minimum = 8; maximum = 255; <ESC>  = 9;   <AE01> = 10;  <AE02> = 11;";
		let key_codes = super::XkbKeyCodes::parse(input).unwrap();
		assert_eq!(key_codes.minimum, 8);
		assert_eq!(key_codes.maximum, 255);
		assert_eq!(key_codes.keys.len(), 3);
		assert_eq!(key_codes.keys[0], ("<ESC>".to_string(), 9));
		assert_eq!(key_codes.keys[1], ("<AE01>".to_string(), 10));
		assert_eq!(key_codes.keys[2], ("<AE02>".to_string(), 11));
	}

	#[test]
	fn test_xkb_type() {
		let input = "modifiers = Shift+Lock; map[Shift] = 2; map[Lock] = 2; preserve[Lock] = Lock; level_name[1] = \"Base\"; level_name[2] = \"Caps\";";
		let xkb_type = super::XKBType::parse(input).unwrap();
		assert_eq!(xkb_type.modifiers, vec!["Shift".to_string(), "Lock".to_string()]);
		assert_eq!(xkb_type.map.len(), 2);
		assert_eq!(xkb_type.map[0], ("Shift".to_string(), 2));
		assert_eq!(xkb_type.map[1], ("Lock".to_string(), 2));
		assert_eq!(xkb_type.preserve.len(), 1);
		assert_eq!(xkb_type.preserve[0], ("Lock".to_string(), "Lock".to_string()));
		assert_eq!(xkb_type.level_names.len(), 2);
		assert_eq!(xkb_type.level_names[0], "Base".to_string());
		assert_eq!(xkb_type.level_names[1], "Caps".to_string());
	}

	#[test]
	fn test_xkb_compat_entry() {
		let input = "action = SetMods(modifiers=Shift)";
		let compat_entry = super::XKBCompatEntry::parse(input).unwrap();
		assert_eq!(compat_entry.action, "SetMods".to_string());
		assert_eq!(compat_entry.args.len(), 1);
		assert_eq!(compat_entry.args.get("modifiers").unwrap(), "Shift");
	}

	#[test]
	fn test_xkb_symbols_key() {
		let input = "type=\"TWO_LEVEL\", [ 1, exclam ]";
		let symbols_entry = super::XKBSymbolsKey::parse(input).unwrap();
		assert_eq!(symbols_entry.type_name.unwrap(), "TWO_LEVEL".to_string());
		assert_eq!(symbols_entry.symbols.len(), 2);
		assert_eq!(symbols_entry.symbols[0], "1".to_string());
		assert_eq!(symbols_entry.symbols[1], "exclam".to_string());

		let input2 = "[ Escape ]";
		let symbols_entry2 = super::XKBSymbolsKey::parse(input2).unwrap();
		assert!(symbols_entry2.type_name.is_none());
		assert_eq!(symbols_entry2.symbols.len(), 1);
		assert_eq!(symbols_entry2.symbols[0], "Escape".to_string());
	}

	#[test]
	fn test_xkb_symbols_modifier_map() {
		let input = "Shift, Lock";
		let modifier_map = super::XkbSymbolsModifierMap::parse(input).unwrap();
		assert_eq!(modifier_map.keys.len(), 2);
		assert_eq!(modifier_map.keys[0], "Shift".to_string());
		assert_eq!(modifier_map.keys[1], "Lock".to_string());
	}

	#[test]
	fn test_xkb_symbols() {
		let input = r#"
      key <CAPS> { [ Caps_Lock ] };
      modifier_map Shift { <LFSH>, <RTSH> };
    "#;
		let symbols = super::XkbSymbols::parse(input).unwrap();
		assert_eq!(symbols.keys.len(), 1);
		assert!(symbols.keys.contains_key("<CAPS>"));
		let caps_key = symbols.keys.get("<CAPS>").unwrap();
		assert!(caps_key.type_name.is_none());
		assert_eq!(caps_key.symbols.len(), 1);
		assert_eq!(caps_key.symbols[0], "Caps_Lock".to_string());

		assert_eq!(symbols.modifier_maps.len(), 1);
		assert!(symbols.modifier_maps.contains_key("Shift"));
		let shift_map = symbols.modifier_maps.get("Shift").unwrap();
		assert_eq!(shift_map.keys.len(), 2);
		assert_eq!(shift_map.keys[0], "<LFSH>".to_string());
		assert_eq!(shift_map.keys[1], "<RTSH>".to_string());
	}

	#[test]
	fn test_xkb_compat() {
		let input = r#"
    interpret Shift_L   { action = SetMods(modifiers=Shift); };
    interpret Shift_R   { action = SetMods(modifiers=Shift); };
    interpret Caps_Lock { action = LockMods(modifiers=Lock); };
    "#;
		let compat = super::XkbCompat::parse(input).unwrap();
		assert_eq!(compat.entries.len(), 3);
		assert!(compat.entries.contains_key("Shift_L"));
		let entry = compat.entries.get("Shift_L").unwrap();
		assert_eq!(entry.action, "SetMods".to_string());
		assert_eq!(entry.args.len(), 1);
		assert_eq!(entry.args.get("modifiers").unwrap(), "Shift");
	}

	#[test]
	fn test_xkb_types() {
		let input = r#"
    type "ONE_LEVEL"  { modifiers = none; level_name[1] = "Any"; };
    type "TWO_LEVEL"  { modifiers = Shift; map[Shift] = 2; level_name[1] = "Base"; level_name[2] = "Shift"; };
    type "ALPHABETIC" { modifiers = Shift+Lock; map[Shift] = 2; map[Lock] = 2; preserve[Lock] = Lock; level_name[1] = "Base"; level_name[2] = "Caps"; };
    "#;
		let types = super::XkbTypes::parse(input).unwrap();
		assert_eq!(types.types.len(), 3);
		assert!(types.types.contains_key("ONE_LEVEL"));
		let one_level = types.types.get("ONE_LEVEL").unwrap();
		assert_eq!(one_level.modifiers, vec!["none".to_string()]);
		assert_eq!(one_level.map.len(), 0);
		assert_eq!(one_level.preserve.len(), 0);
		assert_eq!(one_level.level_names.len(), 1);
		assert_eq!(one_level.level_names[0], "Any".to_string());

		assert!(types.types.contains_key("TWO_LEVEL"));
		let two_level = types.types.get("TWO_LEVEL").unwrap();
		assert_eq!(two_level.modifiers, vec!["Shift".to_string()]);
		assert_eq!(two_level.map.len(), 1);
		assert_eq!(two_level.map[0], ("Shift".to_string(), 2));
		assert_eq!(two_level.preserve.len(), 0);
		assert_eq!(two_level.level_names.len(), 2);
		assert_eq!(two_level.level_names[0], "Base".to_string());
		assert_eq!(two_level.level_names[1], "Shift".to_string());
	}

	#[test]
	fn test_xkb_keymap_qwerty() {
		let input = include_str!("../testdata/xkb/qwerty.txt");

		let keymap = super::XkbKeyMap::from_str(input).unwrap();
		assert_eq!(keymap.key_codes.minimum, 8);
		assert_eq!(keymap.key_codes.maximum, 255);
		assert_eq!(keymap.key_codes.keys.len(), 57);
		assert_eq!(keymap.key_codes.keys[0], ("<ESC>".to_string(), 9));
		assert_eq!(keymap.key_codes.keys[1], ("<AE01>".to_string(), 10));
		assert_eq!(keymap.key_codes.keys[2], ("<AE02>".to_string(), 11));

		assert_eq!(keymap.types.types.len(), 3);
		assert!(keymap.types.types.contains_key("ONE_LEVEL"));
		let one_level = keymap.types.types.get("ONE_LEVEL").unwrap();
		assert_eq!(one_level.modifiers, vec!["none".to_string()]);
		assert_eq!(one_level.map.len(), 0);
		assert_eq!(one_level.preserve.len(), 0);
		assert_eq!(one_level.level_names.len(), 1);
		assert_eq!(one_level.level_names[0], "Any".to_string());

		assert!(keymap.types.types.contains_key("TWO_LEVEL"));
	}

	#[test]
	fn test_xkb_keymap_weston() {
		let input = include_str!("../testdata/xkb/weston.txt");
		let keymap = super::XkbKeyMap::from_str(input).unwrap();
	}
}
