use std::{fs, io::Read};

use clap::{Arg, ArgAction, Command};

fn main() {
	let matches = Command::new("cat")
		.version("0.1.0")
		.author("Colin Douch <colin@quirl.co.nz>")
		.about("Concatenate FILE(s) to standard output")
		.arg(
			Arg::new("FILE")
				.help("The file to concatenate")
				.num_args(0..)
				.default_value("-"),
		)
		.arg(
			Arg::new("number")
				.short('n')
				.long("number")
				.help("Number all output lines")
				.action(ArgAction::SetTrue),
		)
		.get_matches();

	let number = matches.get_flag("number");
	let files: Vec<&String> = matches.get_many("FILE").unwrap().collect();

	let mut i = 0;
	for file in files {
		let file_contents = match file.as_str() {
			"-" => {
				// This technically isn't the same support as the real cat, but it's close enough.
				// The real cat streams stdin rather than reading it all at once.
				let mut buffer = String::new();
				std::io::stdin().read_to_string(&mut buffer).unwrap();
				buffer
			}
			_ => match fs::read_to_string(file) {
				Ok(file) => file,
				Err(e) => {
					eprintln!("cat: {}: {}", file, e);
					continue;
				}
			},
		};

		for line in file_contents.lines() {
			if number {
				i += 1;
				print!("{:6}  ", i);
			}
			println!("{}", line);
		}
	}
}
