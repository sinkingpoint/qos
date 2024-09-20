use std::{fs, path::Path};

use netlink::rtnetlink::{IPAddress, Interface, MacAddress};
use serde::{de, Deserialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	interface: Vec<InterfaceActions>,
}

impl Config {
	pub fn read_from<T: AsRef<Path>>(path: T) -> anyhow::Result<Self> {
		let config_file_bytes = fs::read(path)?;
		let config_file_str = String::from_utf8(config_file_bytes)?;
		let config = toml::from_str(&config_file_str)?;
		Ok(config)
	}

	pub fn get_actions_for_interface(&self, i: &Interface) -> Option<&Actions> {
		self.interface
			.iter()
			.find(|actions| actions.matcher.matches(i))
			.map(|actions| &actions.actions)
	}
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum Matcher {
	#[serde(deserialize_with = "deserialize_mac_address", rename = "mac")]
	Mac(MacAddress),

	#[serde(rename = "name")]
	Name(String),
}

impl Matcher {
	pub fn matches(&self, i: &Interface) -> bool {
		match &self {
			Self::Mac(m) => i.attributes.mac_address.as_ref() == Some(m),
			Self::Name(n) => i.attributes.name.as_ref() == Some(n),
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceActions {
	#[serde(flatten)]
	pub matcher: Matcher,
	pub actions: Actions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actions {
	pub rename_to: Option<String>,
	#[serde(deserialize_with = "deserialize_scoped_address", default)]
	pub add_ips: Option<Vec<ScopedAddress>>,
}

type ScopedAddress = (IPAddress, u8);

fn deserialize_mac_address<'de, D>(d: D) -> Result<MacAddress, D::Error>
where
	D: de::Deserializer<'de>,
{
	let value = String::deserialize(d)?;

	MacAddress::try_from(value.as_str()).map_err(|e| de::Error::invalid_value(de::Unexpected::Str(&value), &e.as_str()))
}

fn deserialize_scoped_address<'de, D>(d: D) -> Result<Option<Vec<ScopedAddress>>, D::Error>
where
	D: de::Deserializer<'de>,
{
	match Option::<Vec<String>>::deserialize(d)? {
		Some(ips) => {
			let ips: Result<Vec<ScopedAddress>, D::Error> = ips
				.into_iter()
				.map(|s| {
					let parts = s.split_once("/");
					if let Some((address_str, scope_str)) = parts {
						let addr = IPAddress::try_from(address_str)
							.map_err(|_| de::Error::invalid_value(de::Unexpected::Str(address_str), &"ip address"))?;

						let scope = scope_str
							.parse()
							.map_err(|_| de::Error::invalid_value(de::Unexpected::Str(scope_str), &"scope"))?;

						Ok((addr, scope))
					} else {
						Err(de::Error::invalid_value(de::Unexpected::Str(&s), &"ip address"))
					}
				})
				.collect();
			Ok(Some(ips?))
		}
		None => Ok(None),
	}
}

#[cfg(test)]
mod test {
	use super::*;
	#[test]
	fn test_config_parse_rename() {
		let config = "
[[interface]]
mac = \"8c:16:45:1f:64:1b\"
[interface.actions]
rename_to = \"foo\"
    ";

		let config = toml::from_str::<Config>(config).unwrap();
		assert_eq!(config.interface.len(), 1);
		assert_eq!(
			config.interface[0].matcher,
			Matcher::Mac(MacAddress::try_from("8c:16:45:1f:64:1b").unwrap())
		);
		assert_eq!(config.interface[0].actions.rename_to.as_deref(), Some("foo"));
	}

	#[test]
	fn test_config_parse_add_ips() {
		let config = "
[[interface]]
mac = \"8c:16:45:1f:64:1b\"
[interface.actions]
add_ips = [\"127.0.0.2/128\", \"::1/128\"]
    ";

		let config = toml::from_str::<Config>(config).unwrap();
		assert_eq!(config.interface.len(), 1);
		assert_eq!(
			config.interface[0].matcher,
			Matcher::Mac(MacAddress::try_from("8c:16:45:1f:64:1b").unwrap())
		);
	}
}
