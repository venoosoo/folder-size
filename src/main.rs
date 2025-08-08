use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Follow symlinks
    #[arg(long)]
    symlink: bool,

    /// Show directory breakdown
    #[arg(long)]
    directory_b: bool,

    /// Limit the recursion depth
    #[arg(long, default_value_t = 10)]
    depth_limit: u8,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let path = env::current_dir()?;



    let mut visited: HashSet<PathBuf> = HashSet::new();

    println!("Starting scan at: {:?}", path);

    let total_size = get_folder_size(&path, &args, 0, &mut visited)?;

    println!("Total size: {}", human_readable_size(total_size));

    Ok(())
}

fn check_symlink(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

fn get_file_size(
    file_path: &Path,
    args: &Args,
    depth: u8,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<u64> {
    let meta = match fs::symlink_metadata(file_path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("Permission denied reading metadata for {:?}, skipping...", file_path);
            return Ok(0);
        }
        Err(e) => return Err(e),
    };

    // Skip special files like sockets, pipes, devices (not files or dirs)
    if !meta.file_type().is_file() && !meta.file_type().is_dir() {
        eprintln!("Skipping special file (not regular file or dir): {:?}", file_path);
        return Ok(0);
    }

    if check_symlink(&meta) && args.symlink {
        let link_target = match fs::read_link(file_path) {
            Ok(target) => target,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied reading symlink target for {:?}, skipping...", file_path);
                return Ok(0);
            }
            Err(e) => return Err(e),
        };

        let resolved_path = if link_target.is_absolute() {
            link_target
        } else {
            file_path.parent().unwrap_or(Path::new("/")).join(link_target)
        };

        let target_path = match resolved_path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied canonicalizing symlink target {:?}, skipping...", resolved_path);
                return Ok(0);
            }
            Err(e) => return Err(e),
        };

        if visited.contains(&target_path) {
            println!("Already visited symlink target: {:?}", target_path);
            return Ok(0);
        }

        if target_path.is_dir() && depth < args.depth_limit {
            visited.insert(target_path.clone());
            let size = get_folder_size(&target_path, args, depth + 1, visited)?;
            Ok(size)
        } else {
            visited.insert(target_path.clone());
            let size = get_file_size(&target_path, args, depth, visited)?;
            Ok(size)
        }
    } else {
        // Regular file or directory (should be file here)
        Ok(meta.len())
    }
}

fn get_folder_size(
    path: &Path,
    args: &Args,
    depth: u8,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<u64> {
    if depth > args.depth_limit {
        println!("Depth limit reached at {:?}", path);
        return Ok(0);
    }

    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("Permission denied canonicalizing {:?}, skipping...", path);
            return Ok(0);
        }
        Err(e) => {
            eprintln!("Warning: could not canonicalize {:?}: {}", path, e);
            return Ok(0);
        }
    };

    if visited.contains(&canonical_path) {
        println!("Already visited: {:?}", canonical_path);
        return Ok(0);
    }

    visited.insert(canonical_path.clone());

    let entries = match fs::read_dir(path) {
        Ok(read_dir) => read_dir,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("Permission denied accessing directory {:?}, skipping...", path);
            return Ok(0);
        }
        Err(e) => return Err(e),
    };

    let mut total = 0u64;

    for entry_result in entries {
        let entry = match entry_result {
            Ok(en) => en,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied reading directory entry in {:?}, skipping...", path);
                continue;
            }
            Err(e) => return Err(e),
        };

        let file_path = entry.path();

        // Try canonicalize for visited check, skip on permission denied
        let canonical = match file_path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied when canonicalizing {:?}, skipping...", file_path);
                continue;
            }
            Err(e) => {
                eprintln!("Warning: could not canonicalize {:?}: {}", file_path, e);
                continue;
            }
        };

        if visited.contains(&canonical) {
            println!("Already visited: {:?}", canonical);
            continue;
        }

        let meta = match fs::symlink_metadata(&file_path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied reading metadata for {:?}, skipping...", file_path);
                continue;
            }
            Err(e) => return Err(e),
        };

        if meta.is_dir() {
            if check_symlink(&meta) && !args.symlink {
                println!("Skipping symlink directory: {:?}", file_path);
                println!("To enable, add --symlink");
                continue;
            }

            let size = get_folder_size(&file_path, args, depth + 1, visited)?;

            if args.directory_b {
                println!("├── {:?}    ({})", file_path, human_readable_size(size));
            }
            total += size;
        } else if meta.is_file() {
            let size = get_file_size(&file_path, args, depth, visited)?;

            if args.directory_b {
                println!("├── {:?}    ({})", file_path, human_readable_size(size));
            }
            total += size;
        } else {
            eprintln!("Skipping special file type: {:?}", file_path);
        }
    }

    Ok(total)
}

fn human_readable_size(bytes: u64) -> String {
    let kb = 1024f64;
    let mb = kb * 1024f64;
    let gb = mb * 1024f64;

    let b = bytes as f64;

    if b >= gb {
        format!("{:.2} GB", b / gb)
    } else if b >= mb {
        format!("{:.2} MB", b / mb)
    } else if b >= kb {
        format!("{:.2} KB", b / kb)
    } else {
        format!("{} B", bytes)
    }
}
