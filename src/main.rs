use clap::Parser;
use mime_guess::from_path;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
struct Args {
    // The path to read
    path: std::path::PathBuf,
}

fn is_video_file(entry: &DirEntry) -> bool {
    let mime_type = from_path(entry.path()).first_or_octet_stream();
    let mime_type_str = mime_type.to_string();
    mime_type_str.starts_with("video/")
}

fn is_valid_entry(entry: &DirEntry) -> bool {
    entry.file_type().is_file() && is_video_file(entry)
}

fn delete_file(path: &std::path::PathBuf) -> std::io::Result<()> {
    fs::remove_file(path)
}

fn main() {
    // Parse the command line arguments
    let args = Args::parse();
    println!("The path is: {:?}", args.path);

    let iter = WalkDir::new(&args.path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(is_valid_entry);

    let entries_map: HashMap<String, std::path::PathBuf> = iter
        .map(|entry| {
            let file_name = entry
                .file_name()
                .to_os_string()
                .into_string()
                .unwrap_or_else(|_| String::new());
            let file_path = entry.path().to_owned();
            (file_name, file_path)
        })
        .collect();

    // let entries: Vec<_> = iter.collect();

    // Process the entries in parallel using rayon
    entries_map.par_iter().for_each(|(key, value)| {
        // check if has the subtitle version
        let parts: Vec<&str> = key.rsplitn(2, '.').collect();
        let file_name = parts.last().unwrap();
        let extension = if parts.len() > 1 { parts[0] } else { "" };

        // Add the suffix to the filename and reconstruct the full file name
        let subtitle_version = format!("{}-C.{}", file_name, extension);
        if entries_map.contains_key(&subtitle_version) {
            println!("file will delete {:?}", value);
            match delete_file(value) {
                Ok(()) => println!("File deleted successfully: {:?}", value),
                Err(error) => eprintln!("Error deleting file: {}", error),
            }

            // also delete the related nfo file
            let nfo_file = value.with_extension("nfo");
            println!("file will delete {:?}", nfo_file);
            match delete_file(&nfo_file) {
                Ok(()) => println!("File deleted successfully: {:?}", nfo_file),
                Err(error) => eprintln!("Error deleting file: {}", error),
            }
        }
    });
}
