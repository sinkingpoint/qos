use std::{fs, io, path::PathBuf};

use nix::{
	mount::{mount, MsFlags},
	sys::stat::Mode,
	unistd::{chroot, mkdir},
};
use superblocks::Device;
use thiserror::Error;

/// A command to switch the root filesystem.
pub struct SwitchrootCommand {
	/// The new root filesystem that will be mounted.
	new_root: PathBuf,

	/// The path where the new root filesystem will be mounted.
	mount_path: PathBuf,
}

impl SwitchrootCommand {
	pub fn new(new_root: Option<PathBuf>) -> io::Result<Self> {
		match new_root.or(default_new_root()?) {
			Some(new_root) => Ok(Self {
				new_root,
				mount_path: PathBuf::from("/.root"),
			}),
			None => Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"No root= parameter found, and no new root specified.",
			)),
		}
	}

	/// Mount the new root filesystem.
	fn mount(&self) -> Result<(), SwitchRootError> {
		let device = Device::new(&self.new_root);
		let probe = match device.probe()? {
			Some(fstype) => fstype,
			None => return Err(SwitchRootError::UnsupportedFilesystem),
		};

		println!("Mounting {} to {}", self.new_root.display(), self.mount_path.display());
		mount::<_, _, _, str>(
			Some(&self.new_root),
			&self.mount_path,
			Some(probe.filesystem_type.as_str()),
			MsFlags::empty(),
			None,
		)
		.map_err(|e| {
			SwitchRootError::Nix(
				format!(
					"failed to mount {} at {}",
					&self.new_root.display(),
					&self.mount_path.display()
				),
				e,
			)
		})?;
		Ok(())
	}

	/// Move the device filesystems (/dev, /proc, /sys, /run) into the new root filesystem.
	fn move_devices(&self) -> Result<(), SwitchRootError> {
		for mount_dev in ["/dev", "/proc", "/sys", "/run"] {
			let mount_dev = PathBuf::from(mount_dev);
			let target = self.mount_path.join(mount_dev.file_name().unwrap());
			mkdir(&target, Mode::from_bits(0o755).unwrap())
				.map_err(|e| SwitchRootError::Nix(format!("failed to create {}", &target.display()), e))?;
			mount::<_, _, str, str>(Some(&mount_dev), &target, None, MsFlags::MS_MOVE, None).map_err(|e| {
				SwitchRootError::Nix(
					format!("failed to move {} to {}", &mount_dev.display(), &target.display()),
					e,
				)
			})?;
		}

		Ok(())
	}

	/// Run the switchroot command.
	pub fn run(&self) -> Result<(), SwitchRootError> {
		println!("Switching root to {}", self.new_root.display());
		fs::create_dir_all(&self.mount_path).unwrap();

		self.mount()?;
		self.move_devices()?;
		chroot(&self.mount_path).map_err(|e| SwitchRootError::Nix("Failed to chroot".to_string(), e))?;

		Ok(())
	}
}

/// Get the new root filesystem from the kernel command line.
fn default_new_root() -> io::Result<Option<PathBuf>> {
	let cmdline = fs::read_to_string("/proc/cmdline")?;
	for var in cmdline.split_ascii_whitespace() {
		if let Some(root) = var.strip_prefix("root=") {
			return Ok(Some(PathBuf::from(root)));
		}
	}

	Ok(None)
}

#[derive(Error, Debug)]
pub enum SwitchRootError {
	#[error("Failed to mount new root: {0}")]
	Nix(String, nix::Error),

	#[error("Failed to mount new root: {0}")]
	IO(#[from] io::Error),

	#[error("Unsupported Filesystem to mount")]
	UnsupportedFilesystem,
}
