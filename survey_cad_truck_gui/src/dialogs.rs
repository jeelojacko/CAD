use std::path::{Path, PathBuf};

pub fn open_project_file(dir: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(d) = dir {
        dialog = dialog.set_directory(d);
    }
    dialog.add_filter("Project", &["json"]).pick_file()
}
