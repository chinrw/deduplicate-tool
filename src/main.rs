use clap::Parser;
use mime_guess::from_path;
use std::collections::HashMap;
// use std::sync::Arc;
use tokio::fs::remove_file;
// use tokio::runtime;
use walkdir::{DirEntry, WalkDir};

use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    // The path to read
    path: std::path::PathBuf,

    #[clap(short, long, default_value = "false")]
    dry_run: bool,
}

fn is_video_file(entry: &DirEntry) -> bool {
    let mime_type = from_path(entry.path()).first_or_octet_stream();
    let mime_type_str = mime_type.to_string();
    mime_type_str.starts_with("video/")
}

fn is_valid_entry(entry: &DirEntry) -> bool {
    entry.file_type().is_file() && is_video_file(entry)
}

async fn delete_file(path: &PathBuf) -> std::io::Result<()> {
    let args = Args::parse();
    if args.dry_run {
        println!("Dry run mode, will not delete file: {:?}", path);
        Ok(())
    } else {
        remove_file(path).await
    }
}

fn get_file_name(filename: &str) -> Vec<String> {
    let extensions = ["-thumb.jpg", "-fanart.jpg", "-poster.jpg", ".nfo"];
    let filenames = extensions
        .iter()
        .map(|ext| format!("{}{}", filename, ext))
        .collect::<Vec<_>>();
    filenames
}

async fn process_entry(
    entry: (String, PathBuf),
    entries_map: &HashMap<String, PathBuf>,
) -> Result<(), std::io::Error> {
    let (key, value) = entry;
    let parts: Vec<&str> = key.rsplitn(2, '.').collect();
    let file_name = parts.last().unwrap();
    let extension = if parts.len() > 1 { parts[0] } else { "" };

    // Add the suffix to the filename and reconstruct the full file name
    let subtitle_version = format!("{}-C.{}", file_name, extension);
    let subtitle_version_uc = format!("{}-UC.{}", file_name, extension);

    // delete the original file if the subtitle version exist
    if entries_map.contains_key(&subtitle_version) || entries_map.contains_key(&subtitle_version_uc)
    {
        let exist_file = if entries_map.contains_key(&subtitle_version) {
            &subtitle_version
        } else {
            &subtitle_version_uc
        };

        println!("file will delete {:?} since exist {}", value, exist_file);

        delete_file(&value).await?;

        // delete the images and nfo
        let extensions = get_file_name(file_name);
        let path_parent = value.parent().unwrap();

        for ext in &extensions {
            // let file = value.with_extension(ext);
            let mut path = PathBuf::from(path_parent);
            path.push(ext);

            let file = PathBuf::from(path);
            println!("file will delete {:?}", file);
            match delete_file(&file).await {
                Ok(_) => println!("File deleted successfully: {:?}", file),
                Err(e) => println!("Failed to delete file: {:?}, error: {}", file, e),
            }
        }
    }

    if entries_map.contains_key(&subtitle_version) && entries_map.contains_key(&subtitle_version_uc)
    {
        let path_parent = value.parent().unwrap();
        let mut path = PathBuf::from(path_parent);
        path.push(subtitle_version);

        println!(
            "subtitled file will delete {:?} since exist {}",
            path, subtitle_version_uc
        );
        delete_file(&path).await?;
        // delete the images and nfo
        let extensions = get_file_name(file_name);
        for ext in &extensions {
            let mut path = value.file_stem().unwrap().to_os_string();
            path.push(ext);
            let file = PathBuf::from(path);
            println!("file will delete {:?}", file);
            match delete_file(&file).await {
                Ok(_) => println!("File deleted successfully: {:?}", file),
                Err(e) => println!("Failed to delete file: {:?}, error: {}", file, e),
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Parse the command line arguments
    let args = Args::parse();
    println!("The path is: {:?}", args.path);

    if args.dry_run {
        println!("Dry run mode");
    }

    let iter = WalkDir::new(&args.path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(is_valid_entry);

    // let rt = runtime::Builder::new_multi_thread()
    //     .worker_threads(1)
    //     .build()?;

    println!("Start collect entries_map");

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

    println!("Collect entries_map done!");

    // let entries_map_arc = Arc::new(entries_map.clone());
    // let entries_map_arc_iter = Arc::clone(&entries_map_arc);
    // let mut handles: Vec<_> = Vec::new();
    let entry_map_clone = entries_map.clone();
    for entry in entries_map.clone() {
        let entry_clone = entry.clone();
        // let entries_map_arc_clone = Arc::clone(&entries_map_arc);
        // let _ = tokio::task::spawn_blocking(move || process_entry(entry, entries_map_arc_clone))
        //     .await?;
        process_entry(entry_clone, &entry_map_clone).await?;
        // handles.push(handle);
    }

    println!("All done!");

    // Wait for all spawned tasks to complete
    // for handle in handles {
    //     let _ = handle.await?;
    // }

    Ok(())
}
