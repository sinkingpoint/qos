mod sha;
use chrono::DateTime;
use sha::Sha2Mode;
use std::{
	fmt::{self, Display, Formatter, Write},
	fs::read_to_string,
	io,
	ops::Range,
	path::PathBuf,
};
use thiserror::Error;

/// The path to the passwd file.
const PASSWD_PATH: &str = "/etc/passwd";

/// The path to the shadow file.
const SHADOW_PATH: &str = "/etc/shadow";

/// The path to the group file.
const GROUP_PATH: &str = "/etc/group";

/// The placeholder for a non-existent password (i.e an account that cannot be logged in).
const NON_EXISTANT_PASSWORD: &str = "x";

// Passwd file lines are in the format `<username>:<password>:<uid>:<gid>:<group>:<home>:<shell>`
// Here we define the indices of each field in the colon separated passwd file line for easy access.

/// The index in the colon separated passwd file line for the username.
const USERNAME_INDEX: usize = 0;

/// The index in the colon separated passwd file line for the UID.
const UID_INDEX: usize = 2;

/// The index in the colon separated passwd file line for the GID.
const GID_INDEX: usize = 3;

/// The index in the colon separated passwd file line for the home directory.
const HOME_INDEX: usize = 5;

/// The index in the colon separated passwd file line for the shell.
const SHELL_INDEX: usize = 6;

/// The index in the colon separated shadow file line for the username.
const SHADOW_PASSWORD_INDEX: usize = 1;

/// The index in the colon separated shadow file line for the last changed field.
const SHADOW_LAST_CHANGED_INDEX: usize = 2;

/// A user on the system, that exists in the passwd file.
pub struct User {
	/// The username of the user.
	pub username: String,

	/// The user's UID.
	pub uid: u32,

	/// The user's primary group ID.
	pub gid: u32,

	/// The user's home directory.
	pub home: PathBuf,

	/// The user's shell that gets exec'd when they log in.
	pub shell: PathBuf,
}

impl User {
	pub fn create(
		username: &str,
		uid: Option<u32>,
		gid: Option<u32>,
		home: &str,
		shell: &str,
		password: Option<&str>,
	) -> Result<Self, AuthError> {
		let uid = match uid {
			Some(uid) => uid,
			None => next_uid(1000..65535)?.ok_or(AuthError::NoMoreIDs)?,
		};

		let gid = match gid {
			Some(gid) => gid,
			None => next_gid(1000..65535)?.ok_or(AuthError::NoMoreIDs)?,
		};

		ShadowEntry::create(username, password)?;
		Group::create(username, Some(gid))?;
		Ok(Self {
			username: username.to_owned(),
			uid,
			gid,
			home: PathBuf::from(home),
			shell: PathBuf::from(shell),
		})
	}

	pub fn get(selector: Selector) -> Result<Option<Self>, AuthError> {
		match selector {
			Selector::Name(name) => Self::from_username(&name),
			Selector::ID(id) => Self::from_uid(id),
		}
	}

	/// Returns the user with the given UID, if it exists.
	pub fn from_uid(uid: u32) -> Result<Option<Self>, AuthError> {
		let passwd = read_to_string(PASSWD_PATH)?;
		for line in passwd.lines() {
			let user = Self::from_passwd_line(line)?;
			if user.uid == uid {
				return Ok(Some(user));
			}
		}

		Ok(None)
	}

	/// Returns the user with the given username, if it exists.
	pub fn from_username(username: &str) -> Result<Option<Self>, AuthError> {
		let passwd = read_to_string(PASSWD_PATH)?;
		for line in passwd.lines() {
			let user = Self::from_passwd_line(line)?;
			if user.username == username {
				return Ok(Some(user));
			}
		}

		Ok(None)
	}

	/// Parses a line from the passwd file into a `User`.
	fn from_passwd_line(line: &str) -> Result<Self, AuthError> {
		let parts: Vec<&str> = line.split(':').collect();
		if parts.len() != 7 {
			return Err(AuthError::Malformed("malformed passwd entry".to_owned()));
		}

		let username = parts[USERNAME_INDEX].to_string();
		let uid = parts[UID_INDEX]
			.parse()
			.map_err(|_| AuthError::Malformed(format!("malformed uid: {}", parts[UID_INDEX])))?;
		let gid = parts[GID_INDEX]
			.parse()
			.map_err(|_| AuthError::Malformed(format!("malformed gid: {}", parts[GID_INDEX])))?;
		let home = PathBuf::from(parts[HOME_INDEX]);
		let shell = PathBuf::from(parts[SHELL_INDEX]);

		Ok(Self {
			username,
			uid,
			gid,
			home,
			shell,
		})
	}

	pub fn shadow(&self) -> Result<Option<ShadowEntry>, AuthError> {
		ShadowEntry::from_username(&self.username)
	}
}

pub struct ShadowEntry {
	/// The username of the user.
	pub username: String,

	/// The encrypted password of the user.
	password_hash: Option<HashedPassword>,

	/// The last time the password was changed.
	last_change: u32,
}

impl ShadowEntry {
	fn create(username: &str, password: Option<&str>) -> Result<Self, AuthError> {
		let password_hash = if let Some(password) = password {
			Some(HashedPassword::from_crypt_password(password)?)
		} else {
			None
		};

		let new = Self {
			username: username.to_owned(),
			password_hash,
			last_change: days_since_epoch(),
		};

		new.write()?;
		Ok(new)
	}

	pub fn write(&self) -> Result<(), AuthError> {
		let shadow = read_to_string(SHADOW_PATH)?;
		let mut lines_to_write = Vec::new();
		let mut exists = false;
		for line in shadow.lines() {
			match Self::from_shadow_line(line) {
				Ok(entry) if entry.username == self.username => {
					lines_to_write.push(self.to_string());
					exists = true;
				}
				_ => lines_to_write.push(line.to_owned()),
			}
		}

		if !exists {
			lines_to_write.push(self.to_string());
		}

		Ok(())
	}

	fn from_shadow_line(line: &str) -> Result<Self, AuthError> {
		let parts: Vec<&str> = line.split(':').collect();
		if parts.len() != 9 {
			return Err(AuthError::Malformed(format!(
				"malformed shadow entry. Expected {} parts, but got {}",
				8,
				parts.len()
			)));
		}

		let password = HashedPassword::from_crypt_password(parts[SHADOW_PASSWORD_INDEX])
			.map(Some)
			.unwrap_or(None);

		let last_change = parts[SHADOW_LAST_CHANGED_INDEX].parse().map_err(|_| {
			AuthError::Malformed(format!("malformed last changed: {}", parts[SHADOW_LAST_CHANGED_INDEX]))
		})?;

		Ok(Self {
			username: parts[USERNAME_INDEX].to_string(),
			password_hash: password,
			last_change,
		})
	}

	/// Returns the shadow entry for the given username, if it exists.
	pub fn from_username(username: &str) -> Result<Option<Self>, AuthError> {
		let shadow = read_to_string(SHADOW_PATH)?;
		for line in shadow.lines() {
			match Self::from_shadow_line(line) {
				Ok(entry) if entry.username == username => return Ok(Some(entry)),
				_ => continue,
			}
		}

		Ok(None)
	}

	/// Verifies the given password against the stored hash.
	pub fn verify_password(&self, password: &str) -> Result<bool, AuthError> {
		match &self.password_hash {
			Some(hashed) => hashed.verify(password),
			None => Ok(false),
		}
	}
}

impl Display for ShadowEntry {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let password = self
			.password_hash
			.as_ref()
			.map(|p| p.to_string())
			.unwrap_or(NON_EXISTANT_PASSWORD.to_string());

		write!(f, "{}:{}:{}::::::", self.username, password, self.last_change)
	}
}

enum PasswordAlgorithm {
	Sha(Sha2Mode),
}

impl Display for PasswordAlgorithm {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		match self {
			PasswordAlgorithm::Sha(Sha2Mode::Sha256) => f.write_char('5'),
			PasswordAlgorithm::Sha(Sha2Mode::Sha512) => f.write_char('6'),
		}
	}
}

struct HashedPassword {
	salt: String,
	hash: String,
	rounds: Option<u32>,
	algorithm: PasswordAlgorithm,
}

impl HashedPassword {
	fn from_crypt_password(crypt_password: &str) -> Result<Self, AuthError> {
		let parts: Vec<&str> = crypt_password.split('$').collect();
		let (salt, hash, rounds) = if parts.len() == 4 {
			(parts[2], parts[3], None)
		} else if parts.len() == 5 {
			if !parts[2].starts_with("rounds=") {
				return Err(AuthError::Malformed("malformed rounds".to_owned()));
			}

			let rounds = parts[2].trim_start_matches("rounds=").parse().map_err(|_| {
				AuthError::Malformed(format!("malformed rounds: {}", parts[2].trim_start_matches("rounds=")))
			})?;

			(parts[3], parts[4], Some(rounds))
		} else {
			// If the hashed password isn't valid crypt(3), it's always incorrect.
			return Err(AuthError::AlwaysBad);
		};

		let algorithm = match parts[1] {
			"5" => PasswordAlgorithm::Sha(Sha2Mode::Sha256),
			"6" => PasswordAlgorithm::Sha(Sha2Mode::Sha512),
			_ => return Err(AuthError::Unsupported(parts[1].to_owned())),
		};

		Ok(Self {
			salt: salt.to_owned(),
			hash: hash.to_owned(),
			rounds,
			algorithm,
		})
	}

	/// Verifies the given password against the stored hash.
	fn verify(&self, password: &str) -> Result<bool, AuthError> {
		match &self.algorithm {
			PasswordAlgorithm::Sha(mode) => {
				let hash: String = mode
					.crypt_sha2(self.salt.as_bytes(), password.as_bytes(), self.rounds)
					.map_err(|e| AuthError::InvalidPassword(e.to_string()))?;

				Ok(self.hash == hash)
			}
		}
	}
}

impl Display for HashedPassword {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "${}${}${}", self.algorithm, self.salt, self.hash)
	}
}

/// A group on the system, that exists in the group file.
pub struct Group {
	/// The GID of the group.
	pub gid: u32,

	/// The name of the group.
	pub name: String,
}

impl Group {
	pub fn create(name: &str, gid: Option<u32>) -> Result<Self, AuthError> {
		let gid = gid.unwrap_or_else(|| next_gid(1000..65535).unwrap().unwrap());
		let new = Self {
			gid,
			name: name.to_owned(),
		};

		new.write()?;
		Ok(new)
	}

	fn write(&self) -> Result<(), AuthError> {
		let group = read_to_string(GROUP_PATH)?;
		let mut lines_to_write = Vec::new();
		let mut exists = false;
		for line in group.lines() {
			let group = Self::from_group_line(line)?;
			if group.gid == self.gid {
				lines_to_write.push(format!("{}:{}:{}:", self.name, "x", self.gid));
				exists = true;
			} else {
				lines_to_write.push(line.to_owned());
			}
		}

		if !exists {
			lines_to_write.push(format!("{}:{}:{}:", self.name, "x", self.gid));
		}

		Ok(())
	}

	pub fn get(selector: Selector) -> Result<Option<Self>, AuthError> {
		match selector {
			Selector::Name(name) => Self::from_groupname(&name),
			Selector::ID(id) => Self::from_gid(id),
		}
	}

	/// Returns the group with the given GID, if it exists.
	pub fn from_gid(gid: u32) -> Result<Option<Self>, AuthError> {
		let group = read_to_string(GROUP_PATH)?;
		for line in group.lines() {
			let group = Self::from_group_line(line)?;
			if group.gid == gid {
				return Ok(Some(group));
			}
		}

		Ok(None)
	}

	/// Returns the group with the given name, if it exists.
	pub fn from_groupname(name: &str) -> Result<Option<Self>, AuthError> {
		let group = read_to_string(GROUP_PATH)?;
		for line in group.lines() {
			let group = Self::from_group_line(line)?;
			if group.name == name {
				return Ok(Some(group));
			}
		}

		Ok(None)
	}

	/// Parses a line from the group file into a `Group`.
	fn from_group_line(line: &str) -> Result<Self, AuthError> {
		let parts: Vec<&str> = line.split(':').collect();
		if parts.len() != 4 {
			return Err(AuthError::Malformed("malformed group entry".to_owned()));
		}

		let name = parts[0].to_string();
		let gid = parts[2]
			.parse()
			.map_err(|_| AuthError::Malformed(format!("malformed gid: {}", parts[2])))?;

		Ok(Self { gid, name })
	}
}

/// The target of an authentication request.
pub enum Selector {
	/// Selects a user/group by its name.
	Name(String),

	/// Selects a user/group by its UID/GID.
	ID(u32),
}

#[derive(Error, Debug)]
pub enum AuthError {
	#[error("I/O error: {0}")]
	IO(#[from] io::Error),

	#[error("Malformed passwd file: {0}")]
	Malformed(String),

	#[error("Unsupported algorithm: {0}")]
	Unsupported(String),

	#[error("Invalid password: {0}")]
	InvalidPassword(String),

	#[error("Invalid password hash")]
	AlwaysBad,

	#[error("No more UIDs or GIDs available")]
	NoMoreIDs,
}

fn days_since_epoch() -> u32 {
	let now = chrono::Utc::now();
	let then = DateTime::UNIX_EPOCH;
	let duration = now.signed_duration_since(then);
	duration.num_days() as u32
}

/// Returns the next available UID, or `None` if there are no more UIDs available.
fn next_uid(acceptable_range: Range<u32>) -> Result<Option<u32>, AuthError> {
	let mut uids = Vec::new();
	let passwd = read_to_string(PASSWD_PATH)?;
	for line in passwd.lines() {
		let user = User::from_passwd_line(line)?;

		if acceptable_range.contains(&user.uid) {
			uids.push(user.uid);
		}
	}

	uids.sort();

	Ok(find_non_overlapping_value(acceptable_range, &uids))
}

/// Returns the next available GID, or `None` if there are no more GIDs available.
fn next_gid(acceptable_range: Range<u32>) -> Result<Option<u32>, AuthError> {
	let mut gids = Vec::new();
	let group = read_to_string(GROUP_PATH)?;
	for line in group.lines() {
		let group = Group::from_group_line(line)?;

		if acceptable_range.contains(&group.gid) {
			gids.push(group.gid);
		}
	}

	gids.sort();

	Ok(find_non_overlapping_value(acceptable_range, &gids))
}

/// Returns the first value in the range that doesn't exist in the given values.
fn find_non_overlapping_value(range: Range<u32>, values: &[u32]) -> Option<u32> {
	for (i, value) in range.enumerate() {
		if values[i] != value {
			return Some(value);
		}
	}

	None
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_user_from_passwd_line() {
		let line = "root:x:0:0:root:/root:/bin/bash";
		let user = User::from_passwd_line(line).unwrap();
		assert_eq!(user.username, "root");
		assert_eq!(user.uid, 0);
		assert_eq!(user.gid, 0);
		assert_eq!(user.home, PathBuf::from("/root"));
		assert_eq!(user.shell, PathBuf::from("/bin/bash"));
	}

	#[test]
	fn test_shadow_entry_from_username() {
		let entry = ShadowEntry::from_shadow_line(
			"test:$6$GkbfJlFNcqp8VGNn$9uWgXkCpoCCdoER/1yc1on8Rus0.eQHfLWkGth30liq9rL.joqL1hP/KfBXUHNT8fbwB44Txr1A01WoozxokQ/:19788:0:99999:7:::",
		)
		.unwrap();
		let password = entry.password_hash.as_ref().unwrap();
		assert_eq!(entry.username, "test");
		assert_eq!(password.salt, "GkbfJlFNcqp8VGNn".to_owned());
		assert_eq!(
			password.hash,
			"9uWgXkCpoCCdoER/1yc1on8Rus0.eQHfLWkGth30liq9rL.joqL1hP/KfBXUHNT8fbwB44Txr1A01WoozxokQ/".to_owned()
		);

		assert!(entry.verify_password("test").unwrap());
	}

	#[test]
	fn test_hashed_password() {
		assert!(HashedPassword::from_crypt_password("$6$GkbfJlFNcqp8VGNn$9uWgXkCpoCCdoER/1yc1on8Rus0.eQHfLWkGth30liq9rL.joqL1hP/KfBXUHNT8fbwB44Txr1A01WoozxokQ/").is_ok());
		assert!(HashedPassword::from_crypt_password("$6$rounds=5000$GkbfJlFNcqp8VGNn$9uWgXkCpoCCdoER/1yc1on8Rus0.eQHfLWkGth30liq9rL.joqL1hP/KfBXUHNT8fbwB44Txr1A01WoozxokQ/").is_ok());

		let hash = HashedPassword::from_crypt_password("$6$6K5C/5JmLlz2u620$zdVIE6PI0EpEtinzxU8eo7NIncxRnMCTZgIltb9voa8.YktocGmjUQp2RdENvWj0LV/sGt1NnGMj9Xpjvga4e/").unwrap();
		assert!(hash.verify("test").unwrap())
	}

	#[test]
	fn test_group() {
		let group = Group::from_group_line("root:x:0:").unwrap();
		assert_eq!(group.gid, 0);
		assert_eq!(group.name, "root");

		assert!(Group::from_group_line("YY").is_err());
	}

	#[test]
	fn test_find_non_overlapping_value() {
		let values = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10];
		assert_eq!(find_non_overlapping_value(0..10, &values), Some(9));

		let values = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
		assert_eq!(find_non_overlapping_value(0..10, &values), None);
	}
}
