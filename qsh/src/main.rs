mod parser;
use nix::{
    libc,
    sys::signal::{
        killpg,
        signal,
        SigHandler,
        Signal::{
            SIGINT,
            SIGQUIT,
            SIGTSTP,
            SIGTTIN,
            SIGTTOU,
        },
    },
    unistd::{
        getpgrp,
        getpid,
        isatty,
        setpgid,
        tcgetpgrp,
        tcsetpgrp,
    },
};
use slog::{
    o,
    Drain,
};
use slog_json::Json;

fn main() {
    let logger = assemble_logger();
    let fd = libc::STDIN_FILENO;
    let is_interactive = match isatty(fd) {
        Ok(s) => s,
        Err(errno) => {
            slog::warn!(logger, "failed to run isatty. Err: {}", errno);
            false
        }
    };

    let mut job_control_enabled = is_interactive;

    if is_interactive {
        loop {
            let shell_pgid = getpgrp();

            let foreground_pgid = match tcgetpgrp(fd) {
                Ok(s) => s,
                Err(e) => {
                    println!("failed to run tcgetpgrp. Err: {}", e);
                    job_control_enabled = false;
                    break;
                }
            };

            if foreground_pgid == shell_pgid {
                break;
            }

            if let Err(e) = killpg(shell_pgid, SIGTTIN) {
                slog::error!(logger, "failed to run killpg. Err: {}", e);
                return;
            }
        }

        if !job_control_enabled {
            println!("no job control for this shell")
        } else {
            // Ignore interactive and job-control signals.
            unsafe {
                signal(SIGINT, SigHandler::SigIgn).unwrap();
                signal(SIGQUIT, SigHandler::SigIgn).unwrap();
                signal(SIGTSTP, SigHandler::SigIgn).unwrap();
                signal(SIGTTIN, SigHandler::SigIgn).unwrap();
                signal(SIGTTOU, SigHandler::SigIgn).unwrap();
            }

            let my_pid = getpid();
            setpgid(my_pid, my_pid).expect("Failed to set PGID for shell");
            tcsetpgrp(fd, my_pid).expect("Failed to become the foreground process");
        }
    }

    println!("loaded!");
}

fn assemble_logger() -> slog::Logger {
    let drain = slog_async::Async::new(
        Json::new(std::io::stderr())
            .add_default_keys()
            .build()
            .fuse(),
    )
    .build()
    .fuse();
    slog::Logger::root(drain, o!())
}
