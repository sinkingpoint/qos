mod service;

use std::{
	collections::{HashMap, VecDeque},
	error::Error,
	fmt::{self, Display, Formatter},
	fs,
	path::{Path, PathBuf},
};

use service::SphereDefinition;
pub use service::{Permissions, ServiceConfig, StartMode};

const SERVICE_FILE_EXTENSION: &str = "service";
const SPHERE_FILE_EXTENSION: &str = "sphere";

/// An error that occurred while validating a service definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
	/// The error message.
	message: String,

	/// Whether the error prevents the service from being used.
	fatal: bool,
}

impl ValidationError {
	/// Creates a new non-fatal validation error with the given message.
	fn new(message: &str) -> ValidationError {
		ValidationError {
			message: message.to_string(),
			fatal: false,
		}
	}

	/// Creates a new fatal validation error with the given message.
	fn new_fatal(message: &str) -> ValidationError {
		ValidationError {
			message: message.to_string(),
			fatal: true,
		}
	}

	fn with_context(&mut self, message: &str) -> &Self {
		self.message = format!("{}: {}", message, self.message);
		self
	}
}

impl Display for ValidationError {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "{}", self.message)
	}
}

impl Error for ValidationError {}

/// The result of validating a service definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationResult {
	errors: Vec<ValidationError>,
}

impl ValidationResult {
	/// Creates a new empty validation result.
	fn new() -> ValidationResult {
		ValidationResult { errors: Vec::new() }
	}

	/// Merges the given validation result into this one.
	fn merge(&mut self, other: ValidationResult) {
		self.errors.extend(other.errors);
	}

	/// Adds an error to the validation result.
	fn add_error(&mut self, error: ValidationError) {
		self.errors.push(error);
	}

	/// Returns true if the validation result contains any errors.
	pub fn is_error(&self) -> bool {
		!self.errors.is_empty()
	}

	/// Returns true if the validation result contains any fatal errors.
	pub fn is_fatal(&self) -> bool {
		self.errors.iter().any(|e| e.fatal)
	}

	/// Adds a context to all errors in the result.
	fn with_context(mut self, message: &str) -> Self {
		for error in self.errors.iter_mut() {
			error.with_context(message);
		}

		self
	}
}

impl Display for ValidationResult {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		for error in &self.errors {
			writeln!(f, "{}", error)?;
		}

		Ok(())
	}
}

impl Error for ValidationResult {}

/// The configuration for qinit.
pub struct Config {
	services: HashMap<String, ServiceConfig>,

	spheres: HashMap<String, SphereDefinition>,
}

impl Config {
	/// Creates a new empty configuration.
	fn empty() -> Config {
		Config {
			services: HashMap::new(),
			spheres: HashMap::new(),
		}
	}

	pub fn get_service_config(&self, name: &str) -> Option<&ServiceConfig> {
		self.services.get(name)
	}

	pub fn get_sphere(&self, name: &str) -> Option<&SphereDefinition> {
		self.spheres.get(name)
	}

	/// Loads all the services from .service files in the given directory
	/// and adds them to the configuration.
	fn load_services_from_directory(&mut self, path: &Path) -> ValidationResult {
		if !path.exists() {
			return ValidationResult::new();
		}

		let mut errors = ValidationResult::new();
		let iter = match fs::read_dir(path) {
			Ok(iter) => iter,
			Err(e) => {
				errors.add_error(ValidationError::new_fatal(&format!(
					"Failed to read directory {}: {}",
					path.display(),
					e
				)));
				return errors;
			}
		};

		for entry in iter {
			let entry = match entry {
				Ok(entry) => entry,
				Err(e) => {
					errors.add_error(ValidationError::new_fatal(&format!(
						"Failed to read directory entry: {}",
						e
					)));
					continue;
				}
			};

			let path = entry.path();

			if !path.is_file() {
				continue;
			}

			if let Some(ext) = path.extension() {
				if ext == SERVICE_FILE_EXTENSION {
					errors.merge(self.load_service_from_file(&path));
				} else if ext == SPHERE_FILE_EXTENSION {
					errors.merge(self.load_sphere_from_file(&path));
				} else {
					errors.add_error(ValidationError::new(&format!(
						"Unknown file extension: {}",
						path.display()
					)));
				}
			}
		}

		errors
	}

	/// Tries to add a service to the configuration, returning any errors or warnings
	/// that we encountered while doing so. If the returned result `.is_fatal()`, then
	/// the service was not added to the configuration.
	fn add_service(&mut self, mut service: ServiceConfig) -> ValidationResult {
		if self.services.contains_key(&service.name) {
			let mut errors = ValidationResult::new();
			errors.add_error(ValidationError::new_fatal(&format!(
				"Service with name {} already exists",
				service.name
			)));
			return errors;
		}

		let result = service.validate();
		if !result.is_fatal() {
			self.services.insert(service.name.clone(), service);
		}

		result
	}

	/// Loads a service from a file and adds it to the configuration.
	fn load_service_from_file(&mut self, path: &Path) -> ValidationResult {
		let mut errors = ValidationResult::new();
		let definition = match fs::read_to_string(path) {
			Ok(definition) => definition,
			Err(e) => {
				errors.add_error(ValidationError::new_fatal(&format!(
					"Failed to read service definition from {}: {}",
					path.display(),
					e
				)));
				return errors;
			}
		};

		let service: ServiceConfig = match toml::from_str(&definition) {
			Ok(service) => service,
			Err(e) => {
				errors.add_error(ValidationError::new_fatal(&format!(
					"Failed to parse service definition from {}: {}",
					path.display(),
					e
				)));
				return errors;
			}
		};

		self.add_service(service)
	}

	/// Loads a sphere from a file and adds it to the configuration.
	fn load_sphere_from_file(&mut self, path: &Path) -> ValidationResult {
		let mut errors = ValidationResult::new();
		let definition = match fs::read_to_string(path) {
			Ok(definition) => definition,
			Err(e) => {
				errors.add_error(ValidationError::new_fatal(&format!(
					"Failed to read sphere definition from {}: {}",
					path.display(),
					e
				)));
				return errors;
			}
		};

		let sphere: SphereDefinition = match toml::from_str(&definition) {
			Ok(sphere) => sphere,
			Err(e) => {
				errors.add_error(ValidationError::new_fatal(&format!(
					"Failed to parse sphere definition from {}: {}",
					path.display(),
					e
				)));
				return errors;
			}
		};

		if self.spheres.contains_key(&sphere.name) {
			errors.add_error(ValidationError::new_fatal(&format!(
				"Sphere with name {} already exists",
				sphere.name
			)));
			return errors;
		}

		self.spheres.insert(sphere.name.clone(), sphere);
		errors
	}

	/// Validates the configuration.
	pub fn validate(&self) -> ValidationResult {
		// We assume that the _individual_ services are already validated.
		// We just need to check for any dependencies between services.
		let mut errors = ValidationResult::new();

		for service in self.services.values() {
			for dependency in service.wants.iter() {
				// Make sure the wanted service exists.
				if !self.services.contains_key(&dependency.name) {
					errors.add_error(ValidationError::new_fatal(&format!(
						"Service {} wants non-existent service {}",
						service.name, dependency.name
					)));

					continue;
				}

				// Make sure the wanted service has the required arguments.
				let wanted_service = self.services.get(&dependency.name).unwrap();
				for name in dependency.arguments.keys() {
					if !wanted_service.service.has_argument(name) {
						errors.add_error(ValidationError::new_fatal(&format!(
							"Service {} wants service {} with non-existent argument {}",
							service.name, wanted_service.name, name
						)));
					}
				}
			}

			for dependency in service.needs.iter() {
				// Make sure the needed service exists.
				if !self.services.contains_key(&dependency.name) {
					errors.add_error(ValidationError::new_fatal(&format!(
						"Service {} needs non-existent service {}",
						service.name, dependency.name
					)));

					continue;
				}

				let needed_service = self.services.get(&dependency.name).unwrap();
				let mut missing_arguments = needed_service.service.arguments.clone();
				for name in dependency.arguments.keys() {
					if !needed_service.service.has_argument(name) {
						errors.add_error(ValidationError::new_fatal(&format!(
							"Service {} needs service {} with non-existent argument {}",
							service.name, needed_service.name, name
						)));
					}

					missing_arguments.retain(|a| &a.name != name);
				}

				if !missing_arguments.is_empty() {
					errors.add_error(ValidationError::new_fatal(&format!(
						"Service {} needs service {} with missing arguments: {}",
						service.name,
						needed_service.name,
						missing_arguments
							.iter()
							.map(|a| a.name.as_str())
							.collect::<Vec<_>>()
							.join(", ")
					)));
				}
			}
		}

		for sphere in self.spheres.values() {
			let mut seen = HashMap::new();
			let mut to_see = VecDeque::new();
			to_see.push_back((sphere.name.clone(), Vec::new()));
			while let Some((sphere_name, path)) = to_see.pop_back() {
				if seen.contains_key(&sphere_name) {
					errors.add_error(ValidationError::new_fatal(&format!(
						"Found circular dependencies in spheres. Sphere {} is depended on through flow: {:?}",
						sphere.name, path,
					)));

					continue;
				}

				let dep = self.get_sphere(&sphere_name).expect("already found sphere");
				let mut new_deps = path.clone();
				new_deps.push(sphere_name.clone());

				for sphere_name in dep.needs.iter() {
					if self.get_sphere(sphere_name).is_none() {
						errors.add_error(ValidationError::new_fatal(&format!(
							"Sphere {} needs sphere {}, but it doesn't exist",
							sphere.name, sphere_name
						)));

						continue;
					};

					to_see.push_back((sphere_name.to_owned(), new_deps.clone()));
				}

				seen.insert(sphere_name, path);
			}

			for service in sphere.services.iter() {
				if let Some(c) = self.get_service_config(&service.name) {
					for arg in service.arguments.keys() {
						if !c.service.has_argument(arg) {
							errors.add_error(ValidationError::new_fatal(&format!(
								"Sphere {} needs argument {} on service {}, but it doesn't exist",
								sphere.name, arg, service.name
							)))
						}
					}
				} else {
					errors.add_error(ValidationError::new(&format!(
						"Sphere {} needs service {}, but it doesn't exist",
						sphere.name, service.name
					)))
				}
			}
		}

		errors
	}
}

/// Loads all the service definitions from the given directories and returns the configuration.
pub fn load_config<T: IntoIterator<Item = PathBuf>>(config_directories: T) -> (Config, ValidationResult) {
	let mut config = Config::empty();

	let mut errors = ValidationResult::new();

	for path in config_directories {
		errors.merge(config.load_services_from_directory(&path));
	}

	(config, errors)
}

#[cfg(test)]
mod test {
	use super::*;
	use service::{Argument, ServiceDefinition};

	#[test]
	fn test_config() {
		let mut config = Config::empty();
		let definition = r#"
      name = "test"
      description = "Test service"
      service = { command = "echo" }
    "#;
		let service: ServiceConfig = toml::from_str(definition).unwrap();
		let errors = config.add_service(service);
		assert!(!errors.is_error());
		assert!(!errors.is_fatal());
		assert_eq!(config.services.len(), 1);
		assert_eq!(config.services.get("test").unwrap().name, "test");
	}

	#[test]
	fn test_config_missing_name() {
		let mut config = Config::empty();
		let definition = r#"
      name = ""
      description = "Test service"
      service = { command = "echo" }
    "#;
		let service: ServiceConfig = toml::from_str(definition).unwrap();
		let errors = config.add_service(service);
		assert!(errors.is_error());
		assert!(errors.is_fatal());
		assert_eq!(config.services.len(), 0);
	}

	#[test]
	fn test_config_duplicate_name() {
		let mut config = Config::empty();
		let definition = r#"
      name = "test"
      description = "Test service"
      service = { command = "echo" }
    "#;
		let service: ServiceConfig = toml::from_str(definition).unwrap();
		let errors = config.add_service(service.clone());
		assert!(!errors.is_error());
		assert!(!errors.is_fatal());
		assert_eq!(config.services.len(), 1);
		assert_eq!(config.services.get("test").unwrap().name, "test");

		let errors = config.add_service(service);
		assert!(errors.is_error());
		assert!(errors.is_fatal());
		assert_eq!(config.services.len(), 1);
	}

	#[test]
	fn test_config_argument_required_with_default() {
		let argument = r#"
      name = "test"
      description = "Test argument"
      required = true
      default = "default"
    "#;
		let argument: Argument = toml::from_str(argument).unwrap();
		let errors = argument.validate();
		assert!(errors.is_error());
		assert!(!errors.is_fatal());
	}

	#[test]
	fn test_config_argument_duplicate() {
		let service = ServiceDefinition {
			command: "echo".to_string(),
			arguments: vec![
				Argument {
					name: "test".to_string(),
					description: None,
					required: false,
					default: None,
				},
				Argument {
					name: "test".to_string(),
					description: None,
					required: false,
					default: None,
				},
			],
		};

		let errors = service.validate();
		assert!(errors.is_error());
		assert!(errors.is_fatal());
	}

	#[test]
	fn test_config_wants() {
		let mut config = Config::empty();
		let definition = r#"
			name = "test"
			description = "Test service"
			service = { command = "echo" }
			wants = [
				{ name = "other" }
			]
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());
		assert!(config.validate().is_error());

		let definition = r#"
			name = "other"
			description = "Other service"
			service = { command = "echo" }
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());
		assert!(!config.validate().is_error());
	}

	#[test]
	fn test_config_wants_invalid_arg() {
		let mut config = Config::empty();
		let definition = r#"
			name = "test"
			description = "Test service"
			service = { command = "echo" }
			wants = [
				{ name = "other", arguments = { "test" = "value" } }
			]
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		let definition = r#"
			name = "other"
			description = "Other service"
			service = { command = "echo" }
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		// Validation fails because the `test` wants an `other` with test=value, but `other` does not have a `test` argument.
		assert!(config.validate().is_error());
	}

	#[test]
	fn test_config_needs() {
		let mut config = Config::empty();
		let definition = r#"
			name = "test"
			description = "Test service"
			service = { command = "echo" }
			needs = [
				{ name = "other" }
			]
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());
		assert!(config.validate().is_error());

		let definition = r#"
			name = "other"
			description = "Other service"
			service = { command = "echo" }
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());
		assert!(!config.validate().is_error());
	}

	#[test]
	fn test_config_needs_invalid_arg() {
		let mut config = Config::empty();
		let definition = r#"
			name = "test"
			description = "Test service"
			service = { command = "echo" }
			needs = [
				{ name = "other", arguments = { "test" = "value" } }
			]
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		let definition = r#"
			name = "other"
			description = "Other service"
			service = { command = "echo" }
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		// Validation fails because the `test` needs an `other` with test=value, but `other` does not have a `test` argument.
		assert!(config.validate().is_error());
	}

	#[test]
	fn test_config_needs_missing_arg() {
		let mut config = Config::empty();
		let definition = r#"
			name = "test"
			description = "Test service"
			service = { command = "echo" }
			needs = [
				{ name = "other" }
			]
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		let definition = r#"
			name = "other"
			description = "Other service"
			service = { command = "echo", arguments = [ { name = "test" } ] }
		"#;
		let errors = config.add_service(toml::from_str(definition).unwrap());
		assert!(!errors.is_error());

		// Validation fails because `other` needs a `test` argument, but `test` does not provide it.
		assert!(config.validate().is_error());
	}

	#[test]
	fn test_load_basic_service() {
		let mut config = Config::empty();
		let errors = config.load_services_from_directory(&PathBuf::from("./testdata/basic-service"));
		assert!(!errors.is_error());
		assert!(config.services.contains_key("getty-${TTY}"));
		assert_eq!(config.services.len(), 1);
		assert_eq!(
			config.services.get("getty-${TTY}").unwrap().service.command,
			"/sbin/getty ${TTY}"
		);
		assert_eq!(
			config.services.get("getty-${TTY}").unwrap().service.arguments,
			vec![
				Argument {
					name: "TTY".to_string(),
					description: Some("The tty to run getty on".to_string()),
					required: true,
					default: None,
				},
				Argument {
					name: "Baud".to_string(),
					description: Some("The baud rate to set on the terminal".to_string()),
					required: false,
					default: Some("9600".to_string()),
				}
			]
		);
	}

	#[test]
	fn test_load_invalid_service() {
		let mut config = Config::empty();
		let errors = config.load_services_from_directory(&PathBuf::from("./testdata/invalid-service"));
		assert!(errors.is_error());
		assert!(errors.is_fatal());
		assert_eq!(config.services.len(), 0);
	}

	#[test]
	fn test_dependant_service() {
		let mut config = Config::empty();
		let errors =
			config.load_service_from_file(PathBuf::from("./testdata/dependant-service/getty.service").as_path());
		assert!(!errors.is_error());
		assert!(config.validate().is_error());

		let errors =
			config.load_service_from_file(PathBuf::from("./testdata/dependant-service/udev.service").as_path());
		assert!(!errors.is_error());
		assert!(!config.validate().is_error());
	}

	#[test]
	fn test_sphere() {
		let mut config = Config::empty();
		let errors = config.load_sphere_from_file(PathBuf::from("./testdata/sphere/base.sphere").as_path());
		assert!(!errors.is_error(), "{:}", errors);
		assert!(config.validate().is_error());

		let errors =
			config.load_service_from_file(PathBuf::from("./testdata/dependant-service/udev.service").as_path());
		assert!(!errors.is_error());
		assert!(!config.validate().is_error());
	}

	#[test]
	fn test_dependant_sphere() {
		let mut config = Config::empty();
		let errors = config.load_sphere_from_file(PathBuf::from("./testdata/dependant-sphere/base.sphere").as_path());
		assert!(!errors.is_error(), "{:}", errors);
		assert!(config.validate().is_error());

		let errors = config.load_sphere_from_file(PathBuf::from("./testdata/dependant-sphere/base1.sphere").as_path());
		assert!(!errors.is_error());
		assert!(!config.validate().is_error());
	}

	#[test]
	fn test_circular_dependant_sphere() {
		let mut config = Config::empty();
		let errors = config.load_sphere_from_file(PathBuf::from("./testdata/dependant-sphere/base2.sphere").as_path());
		assert!(!errors.is_error(), "{:}", errors);
		assert!(config.validate().is_error());

		let errors = config.load_sphere_from_file(PathBuf::from("./testdata/dependant-sphere/base3.sphere").as_path());
		assert!(!errors.is_error());
		assert!(config.validate().is_error(), "{:?}", config.validate());
	}
}
