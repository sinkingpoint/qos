use std::path::PathBuf;

use clap::Parser;
use superblocks::Device;

use nix::mount::{mount, MsFlags};

#[derive(Parser)]
#[command(about="mount a filesystem")]
struct Cli {
    device: PathBuf,
    mount_point: PathBuf,

    #[arg(short, long, help="limit the set of filesystem types", default_value_t = String::from("auto"))]
    types: String,
}

fn main() {
    let cli = Cli::parse();

    let filesystem_type = if cli.types == "auto" {
        let device = Device::new(&cli.device);
        match device.probe() {
            Ok(Some(filesystem)) => filesystem.filesystem_type,
            Ok(None) => {
                eprintln!("mount: Error: Unknown filesystem type");
                return;
            }
            Err(errno) => {
                eprintln!("mount: Error: {}", errno);
                return;
            }
        }
    } else {
        cli.types
    };

    let device = cli.device.to_str();
    let mount_point = match cli.mount_point.to_str() {
        Some(mount_point) => mount_point,
        None => {
            eprintln!("mount: Error: Invalid mount point");
            return;
        }
    };

    match mount::<_, _, str, str>(device, mount_point, Some(&filesystem_type), MsFlags::empty(), None){
        Ok(()) => {}
        Err(errno) => {
            eprintln!("mount: Error: {}", errno);
        }
    };
}
