use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

lazy_static! {
    static ref DATA_FILE_PATH: PathBuf = {
        let mut data_file_path = directories::ProjectDirs::from("", "kerstop", "git_subscribe")
            .expect("user home directory should be available")
            .data_local_dir()
            .to_owned();
        data_file_path.set_file_name("data.toml");
        data_file_path
    };
}

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// list out the tracked repositories
    List,

    /// start tracking a repository
    Add { repo: Option<PathBuf> },

    /// stop tracking a repository
    Remove { repo: Option<PathBuf> },
}

#[derive(Debug, Serialize, Deserialize)]
struct TrackedRepo {
    path: PathBuf,
    last_fetch: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApplicationData {
    tracked_repos: Vec<TrackedRepo>,
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::List => command_list(),
        Commands::Add { repo } => command_add(repo),
        Commands::Remove { repo } => command_remove(repo),
    }
}

fn command_list() {
    let data = load_app_data();
    println!("{data:?}");
    println!();
    for entry in data.tracked_repos {
        let path = &entry.path;
        let time = humantime::format_duration(entry.last_fetch.elapsed().unwrap());
        println!("{:<30} | {}", path.to_string_lossy(), time)
    }
}

fn command_add(path: Option<PathBuf>) {
    let mut data = load_app_data();

    let repo_result = match path {
        Some(p) => git2::Repository::open(p),
        None => git2::Repository::open(
            std::env::current_dir().expect("should be able to access current working dir"),
        ),
    };

    let repo = match repo_result {
        Ok(r) => r,
        Err(e) => panic!("unexpected error: {e}"),
    };

    let mut path = repo.path().to_owned();

    if path.ends_with(".git") {
        path.pop();
    }

    let new_entry = TrackedRepo {
        path: path,
        last_fetch: SystemTime::from(std::time::UNIX_EPOCH),
    };

    data.tracked_repos.push(new_entry);

    write_app_data(data);
}

fn command_remove(path: Option<PathBuf>) {
    let mut data = load_app_data();

    let resolv_path = match path {
        Some(p) => p,
        None => std::env::current_dir().expect("should be able to access the cwd"),
    };

    let r = data
        .tracked_repos
        .iter()
        .enumerate()
        .find(|&(_, r)| same_file::is_same_file(&resolv_path, &r.path).unwrap_or(false));

    match r {
        Some((i, _)) => {
            data.tracked_repos.remove(i);
        }
        None => println!(
            "there were no tracked repositories at {}",
            resolv_path.to_string_lossy()
        ),
    }

    write_app_data(data);
}

fn load_app_data() -> ApplicationData {
    dbg!(DATA_FILE_PATH.as_path());
    let file: Option<File> = match OpenOptions::new()
        .read(true)
        .write(false)
        .open(DATA_FILE_PATH.as_path())
    {
        Ok(f) => Some(f),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            panic!("unable to open data directory");
        }
        Err(_) => panic!("unexpected error opening {}", DATA_FILE_PATH.display()),
    };

    match file {
        Some(mut f) => {
            let mut buf: Vec<u8> = Vec::new();
            let i = f.read_to_end(&mut buf);
            if i.is_err() {
                panic!("error reading app database {}", i.unwrap_err().to_string())
            }
            match toml::from_slice(buf.as_ref()) {
                Ok(data) => data,
                Err(e) => panic!("error reading app database {}", e.to_string()),
            }
        }
        None => ApplicationData {
            tracked_repos: Vec::new(),
        },
    }
}

fn write_app_data(data: ApplicationData) {
    let mut file: File = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(DATA_FILE_PATH.as_path())
    {
        Ok(f) => f,
        Err(e) => panic!("unable to open file due to following error: {e}"),
    };

    let s = toml::to_string(&data).expect("serialization shouldn't fail ");

    let x = file.write_all(s.as_bytes());

    match x {
        Ok(_) => {}
        Err(e) => panic!("unexpected error writing to file {e}"),
    }
}
