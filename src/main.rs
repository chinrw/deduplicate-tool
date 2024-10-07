use clap::Parser;
use mime_guess::from_path;
use serde::Serialize;
use tokio::fs::rename;
use walkdir::{DirEntry, WalkDir};

use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    // The path to read
    path: std::path::PathBuf,

    #[clap(short, long, default_value = "false")]
    dry_run: bool,

    #[arg(short = 'f', long, default_value = "data_dict.yml.zst")]
    dict_path: String,
}

fn is_video_file(entry: &DirEntry) -> bool {
    let mime_type = from_path(entry.path()).first_or_octet_stream();
    let mime_type_str = mime_type.to_string();
    mime_type_str.starts_with("video/")
}

fn is_valid_entry(entry: &DirEntry) -> bool {
    entry.file_type().is_file() && is_video_file(entry)
}

fn load_test_dict(
    test_dict_path: &std::path::Path,
) -> Result<HashMap<String, PathBuf>, Box<dyn Error>> {
    // load dict from file if dict_path is not empty
    println!("load dict from path {:?}", test_dict_path);
    let file = File::open(test_dict_path)?;
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let reader = BufReader::new(decoder);

    // Deserialize the string into a HashMap
    let mut test_dict = HashMap::new();

    reader.lines().for_each(|line| {
        let line = line.unwrap();
        // Assuming each line in your file is a valid YAML representing a
        // key-value pair
        let deserialized_map: HashMap<String, PathBuf> = serde_yaml::from_str(&line).unwrap();
        test_dict.extend(deserialized_map);
    });

    println!("test dict len: {}", test_dict.len());
    Ok(test_dict)
}

// Make the function generic over `T` where `T: Serialize`
fn write_hashmap_to_file<T: Serialize>(hashmap: &T, file_path: &str) -> std::io::Result<()> {
    // Serialize the hashmap to a yaml string
    let serialized = serde_yaml::to_string(hashmap).expect("Failed to serialize");

    // Create or open the file
    let file = File::create(file_path)?;

    // Create a zstd encoder with default compression level
    let mut encoder = zstd::stream::write::Encoder::new(file, 7)?;

    // Write the JSON string to the file
    encoder.write_all(serialized.as_bytes())?;
    encoder.finish()?;

    Ok(())
}

async fn delete_file(path: &PathBuf) -> std::io::Result<()> {
    let args = Args::parse();
    let backup_path = args.path.join("backup").join(path.file_name().unwrap());

    // create folder if not exist
    if !backup_path.exists() {
        fs::create_dir(&backup_path)?;
    }
    println!(
        "File {} will be rename to: {}",
        path.to_string_lossy(),
        backup_path.to_string_lossy()
    );
    if !args.dry_run {
        rename(path, backup_path.as_path()).await?
    }
    Ok(())
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

        println!("file will rename {:?} since exist {}", value, exist_file);

        delete_file(&value).await?;

        // delete the images and nfo
        let extensions = get_file_name(file_name);
        let path_parent = value.parent().unwrap();

        for ext in &extensions {
            // let file = value.with_extension(ext);
            let mut path = PathBuf::from(path_parent);
            path.push(ext);

            println!("file will rename {:?}", path);
            match delete_file(&path).await {
                Ok(_) => println!("File deleted successfully: {:?}", path),
                Err(e) => println!("Failed to delete file: {:?}, error: {}", path, e),
            }
        }
    }

    if entries_map.contains_key(&subtitle_version) && entries_map.contains_key(&subtitle_version_uc)
    {
        let path_parent = value.parent().unwrap();
        let mut path = PathBuf::from(path_parent);
        path.push(subtitle_version);

        println!(
            "subtitled file will rename {:?} since exist {}",
            path, subtitle_version_uc
        );
        delete_file(&path).await?;
        // delete the images and nfo
        let extensions = get_file_name(file_name);
        for ext in &extensions {
            let mut path = PathBuf::from(value.parent().unwrap());
            println!("{} value: {:?}", ext, value);
            path.push(ext);
            println!("file will rename {:?}", path);
            match delete_file(&path).await {
                Ok(_) => println!("File deleted successfully: {:?}", path),
                Err(e) => println!("Failed to delete file: {:?}, error: {}", path, e),
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    let data_dict_path = std::path::Path::new(&args.dict_path);
    let entries_map: HashMap<String, PathBuf>;
    if !data_dict_path.exists() {
        println!("Data dict not found, load file rclone");
        entries_map = iter
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
        write_hashmap_to_file(&entries_map, &args.dict_path)?;
    } else {
        println!("Data dict found, load file from disk");
        entries_map = load_test_dict(data_dict_path)?;
    };

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
