use std::{
    fs::{self, OpenOptions},
    io::BufReader,
    os::unix,
    path::Path,
};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use libc::{unshare, CLONE_NEWPID};
use tar::Archive;

fn pull_image(repository: &str, _tag: &str) -> Result<()> {
    let sandbox_dir = Path::new("./sandbox");
    if sandbox_dir.exists() {
        fs::remove_dir_all(sandbox_dir)?;
    }

    let resp = reqwest::blocking::get(format!(
        "https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/{}:pull",
        repository
    ))
    .unwrap()
    .json::<serde_json::Value>()
    .unwrap();
    let token = resp.get("token").unwrap().as_str().unwrap();

    let resp = reqwest::blocking::Client::new()
        .get(format!(
            "https://registry-1.docker.io/v2/library/{}/manifests/latest",
            repository
        ))
        .header("Authorization", format!("Bearer {}", token))
        .header(
            "Accept",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()
        .unwrap()
        .json::<serde_json::Value>()
        .unwrap();
    println!("{resp:#?}");

    // let amd64_linux_manifests: Vec<&serde_json::Value> = resp
    //     .get("manifests")
    //     .unwrap()
    //     .as_array()
    //     .unwrap()
    //     .iter()
    //     .filter(|x| {
    //         let platform = x.get("platform").unwrap();
    //         let architecture = platform.get("architecture").unwrap().as_str().unwrap();
    //         let os = platform.get("os").unwrap().as_str().unwrap();
    //         architecture == "amd64" && os == "linux"
    //     })
    //     .collect();
    // let manifest = amd64_linux_manifests[0];
    // println!("{manifest:#?}");

    // let media_type = manifest.get("mediaType").unwrap().as_str().unwrap();
    // let digest = manifest.get("digest").unwrap().as_str().unwrap();
    //
    // let resp = reqwest::blocking::Client::new()
    //     .get(format!(
    //         "https://registry-1.docker.io/v2/library/{}/manifests/{}",
    //         repository, digest
    //     ))
    //     .header("Authorization", format!("Bearer {}", token))
    //     .header("Accept", media_type)
    //     .send()
    //     .unwrap()
    //     .json::<serde_json::Value>()
    //     .unwrap();
    // println!("{resp:#?}");

    let layer_digests: Vec<&str> = resp
        .get("layers")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.get("digest").unwrap().as_str().unwrap())
        .collect();

    for layer_digest in layer_digests.into_iter() {
        let target = format!(
            "https://registry-1.docker.io/v2/library/{}/blobs/{}",
            repository, layer_digest
        );
        println!("{target:#?}");

        let resp = reqwest::blocking::Client::new()
            .get(target)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .unwrap();

        let gz_decoder = GzDecoder::new(BufReader::new(resp));
        let mut archive = Archive::new(gz_decoder);
        archive.unpack("./sandbox")?;
    }

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

    Ok(())
}

fn start_sandbox() -> Result<()> {
    unix::fs::chroot("./sandbox")?;
    std::env::set_current_dir("/")?;

    unsafe {
        unshare(CLONE_NEWPID);
    }

    Ok(())
}

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();

    println!("{args:#?}");
    let image_args = &args[2].split(':').collect::<Vec<&str>>();
    let repo = image_args[0];
    let tag = image_args[1];

    pull_image(repo, tag)?;
    start_sandbox()?;

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
