use dirs::data_dir;
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{prelude::*, ErrorKind},
    path::{Path, PathBuf},
};

pub fn copy_to_dir(
    copy_to: &Path,
    profile: &PathBuf,
    start_point: &Path,
    overwrite: bool,
    log_path: &PathBuf,
    symlink: bool,
) -> Result<(), Box<dyn Error>> {
    let mut walk_dir = PathBuf::new();
    match data_dir() {
        Some(path) => {
            walk_dir = path.join(profile).join("extracted").join(start_point);
        }
        None => {
            eprintln!("Failed to get home directory");
        }
    }
    println!(
        "Copying files from {:?} to {:?}",
        walk_dir.display(),
        copy_to
    );
    for entry_result in fs::read_dir(&*walk_dir).unwrap() {
        println!("HOLAA");
        let entry = entry_result?;
        let src_path = entry.path();
        let filename = entry.file_name();
        let dst_path = copy_to.join(&filename);
        let metadata = entry.metadata()?;

        println!(
            "Entry is directory: {} ({})",
            metadata.is_dir(),
            src_path.display()
        );
        if metadata.is_dir() {
            println!(
                "Creating directory and copying contents: '{}' -> '{}'",
                src_path.display(),
                dst_path.display()
            );

            let new_walk_dir = walk_dir.join(&filename);
            match fs::create_dir(&dst_path) {
                Ok(_) => {
                    println!("Created directory: {}", dst_path.display());
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::AlreadyExists => {
                        println!("Directory already exists: {}", dst_path.display());
                    }
                    _ => {
                        println!("Failed to create directory: {}", e);
                        return Err(e.into());
                    }
                },
            }
            copy_to_dir(
                &dst_path,
                profile,
                &new_walk_dir,
                overwrite,
                log_path,
                symlink,
            )?;
        } else if metadata.is_file() {
            if dst_path.exists() {
                let copy_bak_to = match split_path(&dst_path, profile) {
                    Some(x) => x,
                    None => {
                        println!("Failed to split path: {}", dst_path.display());
                        continue;
                    }
                };

                println!(
                    "Backing up file: '{}' -> '{}'",
                    dst_path.display(),
                    copy_bak_to.display()
                );
                create_bak(&dst_path, &copy_bak_to)?;
                println!(
                    "File already exists, writing to log: {}",
                    dst_path.display()
                );

                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_path)?;

                let path_str = format!("{}\n", &dst_path.to_string_lossy());

                file.write_all(path_str.as_bytes())?;
                if !overwrite {
                    continue;
                }
            }

            if !symlink {
                println!(
                    "Copying file: '{}' -> '{}'",
                    src_path.display(),
                    dst_path.display()
                );

                println!("Copying file");
                fs::copy(&src_path, &dst_path)?;
            } else {
                println!(
                    "Creating symlink to file: '{}' -> '{}'",
                    src_path.display(),
                    dst_path.display()
                );

                if dst_path.exists() {
                    println!("Removing old copy/symlink");
                    fs::remove_file(&dst_path)?;
                }
                create_symlink(&src_path, &dst_path)?;
            }
        }
    }

    Ok(())
}

#[cfg(target_family = "unix")]
fn create_symlink(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    std::os::unix::fs::symlink(src, dst)?;
    Ok(())
}

#[cfg(target_family = "windows")]
fn create_symlink(src: &Path, dst: &Path) -> Result<(), Box<dyn Error>> {
    if !src.is_dir() {
        std::os::windows::fs::symlink_file(src, dst)?;
    }
    Ok(())
}

pub fn restore(profile: &Path) -> Result<(), Box<dyn Error>> {
    println!(
        "Restoring profile ({}) to original state",
        profile.display()
    );
    let existing_file_path = profile.join("existing.txt");
    let file_content = fs::read_to_string(&existing_file_path)?;
    let profile_existing = file_content.lines();
    let profile_skip = profile_existing.skip(1);
    let profile_filename = match profile.file_name() {
        Some(x) => x,
        None => {
            println!("Failed to get profile name");
            return Err("Failed to get profile name".into());
        }
    };

    println!("Restoring files from backup");
    for path in profile_skip {
        let copy_bak_to = match path.split_once(profile_filename.to_str().unwrap_or("")) {
            Some((_, copy_bak_to)) => PathBuf::from(copy_bak_to),
            None => continue,
        };
        let temp_dir = PathBuf::from(format!("{}bak{}", profile.display(), copy_bak_to.display()));
        let path = PathBuf::from(path);
        println!(
            "Restoring file: '{}' -> '{}'",
            temp_dir.display(),
            path.display()
        );

        match fs::rename(&temp_dir, &path) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::CrossesDevices => {
                fs::copy(&temp_dir, path)?;
                fs::remove_file(temp_dir)?;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    println!("Removing symlinks");
    let dst = match file_content.lines().next().map(|line| line.to_string()) {
        Some(line) => line,
        None => return Ok(()),
    };
    println!("Restoring directory: '{}'", dst);

    match remove_symlinks(&PathBuf::from(dst)) {
        Ok(_) => {
            println!("Removed symlinks correctly");
        }
        Err(e) => {
            println!("Failed to remove symlinks: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

fn split_path(path: &Path, profile: &Path) -> Option<PathBuf> {
    let path_str = path.to_string_lossy();
    let profile_filename = profile.file_name()?;
    let where_to = match path_str.split_once(profile_filename.to_str()?) {
        Some((_, where_to)) => where_to,
        None => return None,
    };

    let complete_path = PathBuf::from(format!("{}bak{}", profile.display(), where_to));

    Some(complete_path)
}

fn create_bak(src: &PathBuf, dst: &PathBuf) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = dst.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    match fs::rename(src, dst) {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            fs::copy(src, dst)?;
            fs::remove_file(src)?;
        }
        Err(e) => return Err(Box::new(e)),
    }

    println!("src: {}, dst: {}", src.display(), dst.display());

    Ok(())
}

fn remove_symlinks(path: &Path) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            remove_symlinks(&entry.path())?;
        } else if metadata.file_type().is_symlink() {
            fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}
