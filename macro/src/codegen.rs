use derive_more::Display;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

static FILE_LOCKS: Lazy<Mutex<HashMap<PathBuf, Arc<Mutex<File>>>>> = Lazy::new(Default::default);

pub trait TargetCodeWriter {
    fn cursor() -> &'static str;
    fn encloser(module: &[String], name: &str) -> (String, String);
    fn module_name(&self) -> &[String];
    fn module_template(&self) -> String;
    fn target_name() -> &'static str;
    fn target_file_path(&self) -> PathBuf;

    fn insert(&self, method: &str, name: &str) -> Result<(), Box<dyn Error>> {
        let (file, mut content) = self.open_module()?;

        let (opening, closing) = Self::encloser(self.module_name(), name);
        if let Some(opening_index) = content.find(&opening) {
            let closing_index = content.find(&closing).ok_or(TargetCodeError {
                message: "Found an opening tag but not the closing.",
            })?;
            let range = opening_index..(closing_index + closing.len());
            content.replace_range(range, method);
        } else {
            let cursor = content.find(Self::cursor()).ok_or(TargetCodeError {
                message: "No cursor to insert code.",
            })?;
            content.insert_str(cursor, &method);
        }

        let mut file_guard = file.lock().unwrap();
        file_guard.seek(SeekFrom::Start(0)).unwrap();
        file_guard.write_all(content.as_bytes())?;
        file_guard.sync_all()?;
        Ok(())
    }

    /// Creates the target code root directory and returns its path.
    fn target_root(&self) -> PathBuf {
        let path = crate::config::current().output.join(Self::target_name());
        std::fs::create_dir_all(&path).expect(&format!(
            "Failed to create target code directory: {:?}",
            &path
        ));
        path
    }

    /// Opens the target code source file representing the a Rust module.
    ///
    /// If the source file does not exist, it will be created first and a module template will be
    /// written to it.
    fn open_module(&self) -> std::io::Result<(Arc<Mutex<File>>, String)> {
        let path = self.target_file_path();
        let mut files = FILE_LOCKS.lock().unwrap();
        if !files.contains_key(&path) {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&path)?;
            files.insert(path.clone(), Arc::new(Mutex::new(file)));
        }

        let file = files[&path].clone();
        let mut file_guard = file.lock().unwrap();
        file_guard.seek(SeekFrom::Start(0)).unwrap();

        let mut content = String::new();
        file_guard.read_to_string(&mut content)?;
        if content.trim().is_empty() {
            content += &self.module_template();
        }
        Ok((file.clone(), content))
    }
}

#[derive(Debug, Display)]
#[display(fmt = "{}", message)]
pub struct TargetCodeError {
    pub message: &'static str,
}

impl Error for TargetCodeError {}

pub fn normalize_source_code(code: &str) -> String {
    let regex = r"\s+".parse::<Regex>().unwrap();
    code.replace(&regex, " ")
}
