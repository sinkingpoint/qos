use std::collections::{HashMap, HashSet};

use super::{ValidationError, ValidationResult};
use serde::Deserialize;

/// An argument to a service.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Argument {
	/// The name of the argument which is used for templating.
	pub name: String,

	/// A description of the argument.
	pub description: Option<String>,

	/// Whether the argument is required.
	#[serde(default)]
	pub required: bool,

	/// The default value of the argument.
	pub default: Option<String>,
}

impl Argument {
	/// Validates the argument.
	pub fn validate(&self) -> ValidationResult {
		let mut result = ValidationResult::new();
		if self.name.is_empty() {
			result.add_error(ValidationError::new_fatal("Argument name cannot be empty"));
		}

		if self.required && self.default.is_some() {
			result.add_error(ValidationError::new("Required argument cannot have a default value"));
		}

		result
	}
}

/// The definition of a service.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ServiceDefinition {
	/// The command to run.
	pub command: String,

	/// The arguments to the command.
	#[serde(default)]
	pub arguments: Vec<Argument>,
}

impl ServiceDefinition {
	pub fn validate(&self) -> ValidationResult {
		let mut result = ValidationResult::new();
		if self.command.is_empty() {
			result.add_error(ValidationError::new_fatal("Command cannot be empty"));
		}

		let mut existing_args = HashSet::new();
		for argument in self.arguments.iter() {
			result.merge(argument.validate().with_context(&format!("Argument {}", argument.name)));
			if existing_args.contains(&argument.name) {
				result.add_error(ValidationError::new_fatal(&format!(
					"Duplicate argument name: {}",
					argument.name
				)));
			}

			existing_args.insert(&argument.name);
		}

		result
	}

	pub fn has_argument(&self, name: &str) -> bool {
		self.arguments.iter().any(|a| a.name == name)
	}
}

/// A service dependency.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Dependency {
	/// The name of the service that this service depends on.
	pub name: String,

	/// The arguments to the service.
	#[serde(default)]
	pub args: HashMap<String, String>,
}

/// The default user/group to run a service as.
fn default_root() -> String {
	"root".to_string()
}

/// The users and group to start the service with.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Permissions {
	/// The user to start the service as.
	#[serde(default = "default_root")]
	pub user: String,

	/// The group to start the service as.
	#[serde(default = "default_root")]
	pub group: String,

	/// Whether or not to _create_ the service / group if it exists. If false,
	/// and the user / group doesn't exist, the service fails.
	#[serde(default)]
	pub create: bool,
}

impl Permissions {
	fn validate(&self) -> ValidationResult {
		let mut result = ValidationResult::new();
		if self.user.is_empty() {
			result.add_error(ValidationError::new_fatal("User cannot be empty"));
		}

		if self.group.is_empty() {
			result.add_error(ValidationError::new_fatal("Group cannot be empty"));
		}

		result
	}
}

impl Default for Permissions {
	fn default() -> Self {
		Permissions {
			user: default_root(),
			group: default_root(),
			create: false,
		}
	}
}

/// A service definition.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ServiceConfig {
	/// The name of the service.
	pub name: String,

	/// A description of the service.
	pub description: Option<String>,

	/// The service definition.
	pub service: ServiceDefinition,

	/// The "wants" of the service. Wants are services that this service depends on, but is
	/// not explicitly responsible for starting.
	/// For example if a service is running: {name="foo" foo="bar" bar="baz"}
	/// Then a wants of {name="foo" foo="bar"} would pass (note that the wants does not need to include all arguments).
	/// In the same example, a wants of {name="foo" foo="baz"} would fail, and the service would not start.
	/// If {name="foo" foo="bar" bar="baz"} is in the same start group, then the wants implies a dependency so that
	/// the wanted dependency would start before service that wants it.
	#[serde(default)]
	pub wants: Vec<Dependency>,

	/// The "needs" of the service. Needs are services that this service depends on, and is
	/// explicitly responsible for starting. As such, the arguments in the dependency must be a complete
	/// set of the required arguments for the service. If the arguments are not a complete set, the service
	/// will not start.
	/// For example if a service is running: {name="foo" foo="bar" bar="baz"} and a needs of {name="foo" foo="bar"}
	/// is in the same start group, then the needs implies a dependency so that the needed dependency would start before
	/// the service that needs it (if it not already).
	#[serde(default)]
	pub needs: Vec<Dependency>,

	/// The permissions that the service will get when it is started.
	#[serde(default)]
	pub permissions: Permissions,

	/// The runtime directory for the service. This is the directory that the service will be started in.
	pub runtime_directory: Option<String>,

	/// The result of validating this service.
	#[serde(skip)]
	pub errors: ValidationResult,
}

impl ServiceConfig {
	pub fn validate(&mut self) -> ValidationResult {
		let mut result = ValidationResult::new();
		if self.name.is_empty() {
			result.add_error(ValidationError::new_fatal("Service name cannot be empty"));
		}

		result.merge(self.service.validate());
		result.merge(self.permissions.validate());

		self.errors = result.clone();

		result.with_context(&format!("Service {}", self.name))
	}
}
