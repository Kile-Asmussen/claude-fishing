use std::path::Path;

pub fn rotate(log: &Path) {
    if !log.exists() || !log.is_file() {
        return;
    }

    let mut backup = log.to_path_buf();
    let mut backup_name = log.file_name().unwrap().to_os_string();
    backup_name.push("~");
    backup.set_file_name(&backup_name);

    if let Err(e) = std::fs::rename(log, &backup) {
        eprintln!("failed to rotate log {log:?} -> {backup:?}: {e}");
    }
}
