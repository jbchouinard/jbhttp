//! Handler for serving static files.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use log::warn;

use crate::handler::{Handler, Res};
use crate::request::Request;
use crate::response::Response;

/// Handler which serves files under the given root directory.
pub struct DirectoryHandler {
    pub root: PathBuf,
}

impl DirectoryHandler {
    /// Create a new DirectoryHandler.
    ///
    /// # Arguments
    /// * `root`: serve files under this path
    pub fn new(root: &Path) -> Result<Self, io::Error> {
        Ok(Self {
            root: root.canonicalize()?,
        })
    }
}

/// Check if root is parent of target. Make sure both are canonical
/// by calling `canonicalize()` first if you want it to work reliably.
fn is_parent(root: &Path, target: &Path) -> bool {
    let mut curr = target;
    loop {
        if curr == root {
            return true;
        }
        curr = match curr.parent() {
            Some(parent) => parent,
            None => return false,
        };
    }
}

impl Handler<Vec<u8>, Vec<u8>, Vec<u8>, ()> for DirectoryHandler {
    fn handle(&self, request: Request<Vec<u8>>, _context: &mut ()) -> Res<Vec<u8>, Vec<u8>> {
        let filepath = match self.root.join(&request.path[1..]).canonicalize() {
            Ok(p) => p,
            Err(_) => return Err(Response::new(400)),
        };

        // Prevent serving files above root from path traversals like
        // ../../../etc/passwd
        if !is_parent(&self.root, &filepath) {
            warn!("path traversal attempted: {:?}", &filepath);
            return Err(Response::new(404));
        }

        let (contents, content_type) = if filepath.is_file() {
            match fs::read(&filepath) {
                Ok(contents) => (contents, "application/octet-stream"),
                Err(_) => return Err(Response::new(404)),
            }
        } else if filepath.is_dir() {
            match fs::read_dir(&filepath) {
                Ok(dirs) => {
                    let mut dirs_vec = vec![];
                    for dir in dirs {
                        if let Ok(dir) = dir {
                            let path = dir.path();
                            if let Some(file_name) = path.file_name() {
                                dirs_vec.push(file_name.to_str().unwrap().to_string());
                            }
                        }
                    }
                    dirs_vec.push("".to_string());
                    (dirs_vec.join("\n").into_bytes(), "text/plain")
                }
                Err(_) => return Err(Response::new(404)),
            }
        } else {
            return Err(Response::new(404));
        };

        Ok(Response::new(200)
            .with_payload(contents)
            .with_header("Content-Type", content_type))
    }
}
