use std::{
	fs,
	os::unix::fs::MetadataExt,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use auth::{Group, User};
use clap::{Arg, ArgAction, Command};
use tables::{RowTable, Table};

struct LsArgs {
	all: bool,
	recursive: bool,
}

struct LsFile {
	name: PathBuf,
	mode: u32,
	nlink: u64,
	uid: u32,
	gid: u32,
	size: u64,
	mtime: i64,
}

fn ls_dir(file: &Path, args: &LsArgs) -> Result<Vec<LsFile>> {
	let mut result = Vec::new();
	let entries = fs::read_dir(file).with_context(|| format!("failed to read directory {}", file.display()))?;
	for entry in entries {
		let entry = entry?;
		let name = entry.file_name();
		if args.all || !name.to_str().unwrap().starts_with('.') {
			let metadata = entry
				.metadata()
				.with_context(|| format!("failed to get metadata for {}", name.to_string_lossy()))?;
			let ls_file = LsFile {
				name: entry.path().strip_prefix(file)?.to_path_buf(),
				mode: metadata.mode(),
				nlink: metadata.nlink(),
				uid: metadata.uid(),
				gid: metadata.gid(),
				size: metadata.size(),
				mtime: metadata.mtime(),
			};

			result.push(ls_file);

			if args.recursive
				&& entry
					.file_type()
					.with_context(|| format!("failed to get file type for {}", name.to_string_lossy()))?
					.is_dir()
			{
				result.append(&mut ls_dir(&entry.path(), args)?);
			}
		}
	}

	Ok(result)
}

fn ls_file(file: &Path) -> Result<LsFile> {
	let stat = fs::metadata(file).with_context(|| format!("failed to get metadata for {}", file.display()))?;
	Ok(LsFile {
		name: file
			.strip_prefix("./")
			.with_context(|| "failed to strip `./` prefix")?
			.to_path_buf(),
		mode: stat.mode(),
		nlink: stat.nlink(),
		uid: stat.uid(),
		gid: stat.gid(),
		size: stat.size(),
		mtime: stat.mtime(),
	})
}

fn ls(file: &Path, args: &LsArgs) -> Result<Vec<LsFile>> {
	let mut result = Vec::new();
	if file.is_dir() {
		result.append(&mut ls_dir(file, args)?);
	} else {
		result.push(ls_file(file)?);
	}

	Ok(result)
}

fn main() {
	let matches = Command::new("ls")
		.about("List files in a directory")
		.author("Colin Douch")
		.version("1.0")
		.arg(Arg::new("file").num_args(0..).default_value("."))
		.arg(
			Arg::new("long")
				.short('l')
				.long("long")
				.help("use long listing format")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("recursive")
				.short('R')
				.long("recursive")
				.help("list subdirectories recursively")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("all")
				.short('a')
				.long("all")
				.help("do not ignore entries starting with .")
				.action(ArgAction::SetTrue),
		)
		.get_matches();

	let args = LsArgs {
		all: *matches.get_one("all").expect("all is missing"),
		recursive: *matches.get_one("recursive").expect("recursive is missing"),
	};

	let paths: Vec<String> = matches.get_many("file").expect("file is missing").cloned().collect();
	let long = *matches.get_one("long").expect("long is missing");
	for path in paths {
		let files = match ls(&PathBuf::from(&path), &args) {
			Ok(files) => files,
			Err(e) => {
				eprintln!("ls: cannot access {}: {}", path, e.source().unwrap());
				continue;
			}
		};

		if long {
			let mut table = Table::new();
			for file in files {
				let username = match User::from_uid(file.uid) {
					Ok(Some(user)) => user.username,
					_ => file.uid.to_string(),
				};

				let group = match Group::from_gid(file.gid) {
					Ok(Some(group)) => group.name,
					_ => file.gid.to_string(),
				};

				table
					.add_row([
						file.mode.to_string(),
						file.nlink.to_string(),
						username,
						group,
						file.size.to_string(),
						file.mtime.to_string(),
						file.name.to_string_lossy().to_string(),
					])
					.unwrap();
			}

			println!("{}", table);
		} else {
			let mut table = RowTable::new(238);
			for file in files {
				table.add_value(file.name.to_string_lossy().to_string()).unwrap();
			}

			println!("{}", table);
		}
	}
}
