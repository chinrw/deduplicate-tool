use clap::Parser;
use mime_guess::from_path;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::remove_file;
use tokio::runtime;
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

async fn process_entry(
    entry: (String, PathBuf),
    entries_map: Arc<HashMap<String, PathBuf>>,
) -> Result<(), std::io::Error> {
    let (key, value) = entry;
    let parts: Vec<&str> = key.rsplitn(2, '.').collect();
    let file_name = parts.last().unwrap();
    let extension = if parts.len() > 1 { parts[0] } else { "" };

    // Add the suffix to the filename and reconstruct the full file name
    let subtitle_version = format!("{}-C.{}", file_name, extension);

    if entries_map.contains_key(&subtitle_version) {
        println!(
            "file will delete {:?} since exist {}",
            value, subtitle_version
        );
        delete_file(&value).await?;
        println!("File deleted successfully: {:?}", value);

        // also delete the related nfo file
        let nfo_file = value.with_extension("nfo");
        println!("file will delete {:?}", nfo_file);
        delete_file(&nfo_file).await?;
        println!("File deleted successfully: {:?}", nfo_file);

        // TODO: delete the images
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
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
    //     .worker_threads(2)
    //     .build()?;

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

    let entries_map_arc = Arc::new(entries_map.clone());
    // let entries_map_arc_iter = Arc::clone(&entries_map_arc);
    // let mut handles: Vec<_> = Vec::new();
    for entry in entries_map.clone() {
        // let entry_clone = Arc::new(entry);
        let entries_map_arc_clone = Arc::clone(&entries_map_arc);
        // let handle = rt.block_on(async move { process_entry(entry, entries_map_arc_clone).await });
        process_entry(entry, entries_map_arc_clone);
        // handles.push(handle);
    }

    // Wait for all spawned tasks to complete
    // for handle in handles {
    //     let _ = handle.await?;
    // }

    Ok(())
}
