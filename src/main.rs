use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use clap::Parser;
use std::env;
use std::collections::HashSet;

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

    //let path: std::path::PathBuf = env::current_dir()?;
    let path: PathBuf = PathBuf::from("/home/ven/.steam");

    let mut visited: HashSet<std::path::PathBuf> = HashSet::new();

    println!("{:?}", path);

    let size: u64 = get_folder_size(&path, &args, &mut 0, &mut visited)?;

    println!("Total size: {} ", human_readable_size(size));

    Ok(())
    
}


fn check_sym_link(file_path: &Path) -> io::Result<bool> {
    let meta = fs::symlink_metadata(file_path)?;
    Ok(meta.file_type().is_symlink())
}



fn get_file_size(file_path: &Path,args: &Args, depth: &mut u8, visited: &mut HashSet<std::path::PathBuf>) -> io::Result<u64> {
    let meta = fs::symlink_metadata(file_path)?;

    if check_sym_link(file_path)? && args.symlink {
        let link_target = fs::read_link(file_path)?;
        let resolved_path = if link_target.is_absolute() {
            link_target
        } else {
            file_path.parent().unwrap_or(Path::new("/")).join(link_target)
        };

        //for the symlinks that using relative path
        let target_path = resolved_path.canonicalize()?;

        if target_path.is_dir() && *depth <= args.depth_limit {
            *depth += 1;
            visited.insert(target_path.clone());
            let size = get_folder_size(&target_path,&args, depth, visited)?;
            *depth -= 1;
            return Ok(size)
        } else {
            visited.insert(target_path.clone());
            return get_file_size(&target_path,&args, depth,visited);
        }
    } else {
        Ok(meta.len())
    }
}

fn get_folder_size(path: &Path, args: &Args, depth: &mut u8, visited: &mut HashSet<std::path::PathBuf>) -> io::Result<u64> {
    let mut total: u64 = 0;
    let entries = match fs::read_dir(path) {
        Ok(read_dir) => read_dir,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Permission denied accessing {:?}, skipping...", path);
            return Ok(0); // skip this directory and continue
        }
        Err(e) => return Err(e),
    };
    for entry_result in entries {
        let entry: fs::DirEntry = entry_result?;
        let file_path: std::path::PathBuf = entry.path();
        let cannonical = match file_path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: could not canonicalize (maybe broken symlink)  {:?}: {}", file_path, e);
                return Ok(0); // Skip this file/folder gracefully
            }
        };




        //symlink infinite recursion fix
        if visited.contains(&cannonical) {
            println!("Already visited: {:?}", file_path);
            return Ok(0)
        }

        if file_path.is_dir() && *depth <= args.depth_limit {
            if check_sym_link(&file_path)? && !args.symlink {
                println!("skipping simlink");
                println!("to enable add --symlink to args");
                continue;
            }
            *depth += 1;
            visited.insert(file_path.clone());
            let size = get_folder_size(&file_path,&args, depth, visited)?;
            if args.directory_b {
                println!("├── {:?}    ({})", file_path, human_readable_size(size));  
            }
            total += size;
            *depth -= 1;

            
        } else if file_path.is_file() {
            visited.insert(file_path.clone());
            let size = get_file_size(&file_path,&args, depth, visited)?;
            if args.directory_b {
                println!("├── {:?}    ({})", file_path, human_readable_size(size));  
            }
            total += size;

        }
    }

    Ok(total)
}

fn human_readable_size(bytes: u64) -> String {
    let kb = 1024.0;
    let mb = kb * 1024.0;
    let gb = mb * 1024.0;

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