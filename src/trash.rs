use std::path::Path;

/// Moves a file or folder to the Trash, so a wrong click can be undone from the Finder.
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    platform::move_to_trash(path)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use objc2_foundation::{NSFileManager, NSString, NSURL};

    pub fn move_to_trash(path: &Path) -> Result<(), String> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|err| err.localizedDescription().to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::Path;

    pub fn move_to_trash(path: &Path) -> Result<(), String> {
        let result = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
        result.map_err(|err| err.to_string())
    }
}
