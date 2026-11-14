use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use mkxpz::{Archive, Config, Demo, Error};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("mkxpz: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> mkxpz::Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        return Ok(());
    }
    let command = args.remove(0);
    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "demo" => demo(args)?,
        "ls" => list_archive(args)?,
        "extract" => extract_archive(args)?,
        "config" => show_config(args)?,
        other => return Err(Error::new(format!("unknown command '{other}'"))),
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
mkxpz {version} — Rust RGSS runtime

Usage:
  mkxpz demo [--frames N] [--ppm FILE]
  mkxpz ls ARCHIVE
  mkxpz extract ARCHIVE DEST
  mkxpz config [DIR]

The demo composites a sprite, a window, and text with the software renderer.
`ls` and `extract` read RGSSAD version 1, 2, and 3 archives.",
        version = env!("CARGO_PKG_VERSION")
    );
}

fn demo(args: Vec<String>) -> mkxpz::Result<()> {
    let mut frames = 1i32;
    let mut ppm: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--frames" => {
                index += 1;
                frames = args
                    .get(index)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| Error::new("--frames needs an integer"))?;
            }
            "--ppm" => {
                index += 1;
                ppm = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| Error::new("--ppm needs a path"))?,
                ));
            }
            other => return Err(Error::new(format!("unknown demo option '{other}'"))),
        }
        index += 1;
    }
    let mut demo = Demo::new()?;
    for _ in 1..frames.max(1) {
        demo.step()?;
    }
    let frame = demo.graphics.frame();
    println!(
        "frame {}  {}x{}  actor-pixel #{:02x}{:02x}{:02x}",
        demo.graphics.frame_count,
        frame.width(),
        frame.height(),
        frame.get_pixel(12, 12).to_rgba8()[0],
        frame.get_pixel(12, 12).to_rgba8()[1],
        frame.get_pixel(12, 12).to_rgba8()[2]
    );
    if let Some(path) = ppm {
        fs::write(&path, frame.to_ppm()).map_err(|err| Error::new(err.to_string()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn list_archive(args: Vec<String>) -> mkxpz::Result<()> {
    let path = args
        .first()
        .ok_or_else(|| Error::new("usage: mkxpz ls ARCHIVE"))?;
    let bytes = fs::read(path).map_err(|err| Error::new(err.to_string()))?;
    let archive = Archive::open(bytes)?;
    println!("RGSSAD version {}", archive.version());
    for name in archive.files() {
        let size = archive.size_of(name).unwrap_or(0);
        println!("{size:>10}  {name}");
    }
    Ok(())
}

fn extract_archive(args: Vec<String>) -> mkxpz::Result<()> {
    let path = args
        .first()
        .ok_or_else(|| Error::new("usage: mkxpz extract ARCHIVE DEST"))?;
    let dest = args
        .get(1)
        .ok_or_else(|| Error::new("usage: mkxpz extract ARCHIVE DEST"))?;
    let bytes = fs::read(path).map_err(|err| Error::new(err.to_string()))?;
    let archive = Archive::open(bytes)?;
    let root = PathBuf::from(dest);
    archive.extract_to(&root)?;
    for name in archive.files() {
        println!("{name}");
    }
    Ok(())
}

fn show_config(args: Vec<String>) -> mkxpz::Result<()> {
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let config = Config::load_dir(&dir, &[]);
    println!("title: {}", config.game_title);
    println!("rgss: {}", config.rgss_version);
    println!("screen: {}x{}", config.def_screen_w, config.def_screen_h);
    println!("scripts: {}", config.game_scripts);
    for warning in &config.warnings {
        println!("warning: {warning}");
    }
    Ok(())
}
