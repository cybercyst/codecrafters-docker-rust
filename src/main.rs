use std::{
    fs::{self, OpenOptions},
    os::unix,
    path::Path,
};

use anyhow::{Context, Result};

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    fs::create_dir_all("./sandbox/usr/local/bin")?;
    fs::create_dir_all("./sandbox/dev")?;
    fs::copy(
        "/usr/local/bin/docker-explorer",
        "./sandbox/usr/local/bin/docker-explorer",
    )?;
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(Path::new("./sandbox/dev/null"))?;
    unix::fs::chroot("./sandbox")?;
    std::env::set_current_dir("/")?;

    let args: Vec<_> = std::env::args().collect();
    let command = &args[3];
    let command_args = &args[4..];
    let output = std::process::Command::new(command)
        .args(command_args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                command, command_args
            )
        })?;

    let std_out = std::str::from_utf8(&output.stdout)?;
    print!("{}", std_out);
    let std_err = std::str::from_utf8(&output.stderr)?;
    eprint!("{}", std_err);

    std::process::exit(output.status.code().unwrap_or(-1));
}
