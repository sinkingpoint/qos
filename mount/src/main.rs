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

    let device = Device::new(&cli.device);
    let filesystem = match device.probe() {
        Ok(Some(filesystem)) => filesystem,
        Ok(None) => {
            eprintln!("mount: Error: Unknown filesystem type");
            return;
        }
        Err(errno) => {
            eprintln!("mount: Error: {}", errno);
            return;
        }
    };

    let filesystem_type = Some(match cli.types.as_str() {
        "auto" => filesystem.filesystem_type.as_str(),
        _ => cli.types.as_str(),
    });

    let device = cli.device.to_str();
    let mount_point = match cli.mount_point.to_str() {
        Some(mount_point) => mount_point,
        None => {
            eprintln!("mount: Error: Invalid mount point");
            return;
        }
    };
    
    let flags = MsFlags::empty();

    match mount::<_, _, _, str>(device, mount_point, filesystem_type, flags, None){
        Ok(()) => {}
        Err(errno) => {
            eprintln!("mount: Error: {}", errno);
        }
    };
}
