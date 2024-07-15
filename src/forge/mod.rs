#![allow(dead_code)]
// Jackson Coxson

// The forge functions as a cached CDN to serve files dynamically
// The file structure exists as follows (this isn't how it works anymore, I changed it)
// .
// | -- file.txt             // Files in a folder without a config will be served like normal
// | -- folder/
// |  | -- file.txt          // Still no config, so the file will be served at /folder/file.txt
// | -- folder2.txt/
// |  | -- forge.toml        // The config file will be parsed and used to serve the file
// |  | -- folder2.txt       // Served at /folder2.txt
// | -- folder3.txt/
// |  | -- v0.1.0/           // Symantic versioning will be used
// |  |  | -- folder3.txt    // The file will be served at /folder3.txt?v=0.1.0
// |  | -- v0.1.1/           // Symantic versioning will be used
// |  |  | -- folder3.txt    // The file will be served at /folder3.txt?v=0.1.1 or /folder3.txt since it's the latest

use std::{collections::HashMap, io::Read, path::PathBuf};

use tree::Node;

mod config;
mod converters;
mod tree;

/// Serves as a cache for the files
pub struct Forge {
    inner: Node,
    cache: HashMap<String, Vec<u8>>,
    cache_size: usize,
    cache_limit: usize,
}

#[derive(Clone)]
pub struct ForgeEntry {
    versions: ForgeVersioned,
    converters: Vec<ForgeConverter>,
    content_type: String,
    hidden: bool,
}

pub type ForgeConverter = fn(Vec<u8>) -> Vec<u8>;

pub enum ForgeReturnType {
    File((Vec<u8>, String)),
    Dir,
}

#[derive(Clone)]
enum ForgeVersioned {
    Versioned((String, HashMap<String, PathBuf>)),
    Unversioned(PathBuf),
}

enum LoadReturn {
    Entry((String, ForgeEntry)),
    Node((String, Node)),
}

/// https://stackoverflow.com/questions/23714383/what-are-all-the-possible-values-for-http-content-type-header
enum DefaultContentType {
    // Type application
    Ogg,
    Pdf,
    Json,
    ApplicationXml,
    // Audio
    Mpeg,
    // Image
    Gif,
    Jpeg,
    Png,
    Tiff,
    SvgXml,
    // Text
    Css,
    Csv,
    Html,
    Javascript,
    Plain,
    Xml,
    // Video
    VideoMpeg,
    Mp4,
    Quicktime,
    Flv,
    Webm,
}

impl Forge {
    pub fn get(
        &self,
        request: Vec<&str>,
        _version: Option<String>,
    ) -> Result<ForgeReturnType, std::io::Error> {
        if let Some(r) = self.inner.traverse(request) {
            // Did we get a file or dir?
            match r {
                tree::NodeTraverseReturn::File(entry) => {
                    // TODO respect versioning
                    let path = match entry.versions {
                        ForgeVersioned::Versioned(_) => unimplemented!(),
                        ForgeVersioned::Unversioned(p) => p,
                    };

                    let mut buf = Vec::new();
                    std::fs::File::open(path)?.read_to_end(&mut buf)?;

                    for converter in entry.converters.iter() {
                        buf = converter(buf);
                    }

                    Ok(ForgeReturnType::File((buf, entry.content_type.clone())))
                }
                tree::NodeTraverseReturn::Dir(_) => Ok(ForgeReturnType::Dir),
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
        }
    }

    /// Gets a view of a folder, dirs and files inside said node
    pub fn view(&self, request: Vec<&str>) -> Result<(Vec<String>, Vec<String>), std::io::Error> {
        if let Some(r) = self.inner.traverse(request) {
            match r {
                tree::NodeTraverseReturn::Dir(node) => {
                    let mut dirs = Vec::new();
                    let mut files = Vec::new();
                    for (name, child) in node.children.iter() {
                        if !child.hidden {
                            dirs.push(name.to_owned())
                        }
                    }
                    for (name, child) in node.files.iter() {
                        if !child.hidden {
                            files.push(name.to_owned())
                        }
                    }

                    Ok((dirs, files))
                }
                tree::NodeTraverseReturn::File(_) => {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "File"))
                }
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ))
        }
    }

    pub fn new(path: PathBuf, cache_limit: usize) -> Result<Self, std::io::Error> {
        let head = Self::load(path, 0)?;
        let node: Node = head.into();
        let node = node.take_first_child().unwrap();
        println!("Loaded tree");
        node.print();
        Ok(Forge {
            inner: node,
            cache: HashMap::new(),
            cache_size: 0,
            cache_limit,
        })
    }

    /// Loads the current Node and recursively loads nodes below
    /// Takes a node and the path for the node to load
    /// Returns an optional string if the node needs a different hashed name than the folder
    fn load(path: PathBuf, depth: usize) -> Result<Vec<LoadReturn>, std::io::Error> {
        // First, determine if this folder requires special loading (aka there's a forge.toml)
        let config = config::load(&path.join("forge.toml")).unwrap_or_default();

        // Get the list of files in the current directory
        let dir = std::fs::read_dir(&path)?;

        let mut nodes = Vec::new();
        let mut files = Vec::new();
        for file in dir {
            let file = file?;
            let path = file.path();

            // If the file is a directory, load it recursively
            if path.is_dir() {
                let rets = Self::load(path, if config.parented { depth } else { depth + 1 })?;
                for r in rets {
                    match r {
                        LoadReturn::Node((name, mut node)) => {
                            if config.parented && config.hidden {
                                node.hidden = true;
                            }
                            nodes.push((name, node));
                        }
                        LoadReturn::Entry((name, entry)) => {
                            files.push((name, entry));
                        }
                    }
                }
                continue; // explicit continue to re-init path
            }

            // For the files, load them up
            if path.is_file() {
                if path.file_name().unwrap_or_default() == "forge.toml" {
                    continue;
                }
                files.push((
                    path.file_name()
                        .ok_or::<std::io::Error>(std::io::ErrorKind::InvalidInput.into())?
                        .to_string_lossy()
                        .to_string(),
                    ForgeEntry {
                        versions: ForgeVersioned::Unversioned(path.as_path().to_owned()),
                        converters: Vec::new(), // todo
                        content_type: config.content_type.clone().unwrap_or_else(|| {
                            // Get the extension of the file to make a guess

                            match path.extension() {
                                Some(p) => {
                                    let p = p.to_string_lossy();
                                    let ext = p.as_ref();
                                    DefaultContentType::from_extension(ext).to_content_string()
                                }
                                None => DefaultContentType::default().to_content_string(),
                            }
                        }),
                        hidden: config.hidden && config.parented,
                    },
                ));
            }
        }

        if config.parented {
            // The node is parented, return everything as we have it now
            let mut results = Vec::new();
            for n in nodes {
                results.push(LoadReturn::Node(n));
            }
            for f in files {
                results.push(LoadReturn::Entry(f))
            }
            Ok(results)
        } else {
            // Get the name of the folder we're currently in

            let name = path
                .components()
                .next_back()
                .ok_or(std::io::ErrorKind::InvalidInput)?
                .as_os_str()
                .to_string_lossy()
                .to_string();

            // Return the node and the files
            Ok(vec![LoadReturn::Node((
                name,
                (nodes, files, depth, config.hidden).into(),
            ))])
        }
    }

    pub fn print_tree(&self) {
        self.inner.print()
    }
}

impl DefaultContentType {
    fn to_content_string(&self) -> String {
        match self {
            DefaultContentType::Ogg => "application/ogg",
            DefaultContentType::Pdf => "application/pdf",
            DefaultContentType::Json => "application/json",
            DefaultContentType::ApplicationXml => "application/xml",
            DefaultContentType::Mpeg => "audio/mpeg",
            DefaultContentType::Gif => "image/gif",
            DefaultContentType::Jpeg => "image/jpeg",
            DefaultContentType::Png => "image/png",
            DefaultContentType::Tiff => "image/tiff",
            DefaultContentType::SvgXml => "image/svg+xml",
            DefaultContentType::Css => "text/css",
            DefaultContentType::Csv => "text/csv",
            DefaultContentType::Html => "text/html",
            DefaultContentType::Javascript => "text/javascript",
            DefaultContentType::Plain => "text/plain",
            DefaultContentType::Xml => "text/xml",
            DefaultContentType::VideoMpeg => "video/mpeg",
            DefaultContentType::Mp4 => "video/mp4",
            DefaultContentType::Quicktime => "video/quicktime",
            DefaultContentType::Flv => "video/x-flv",
            DefaultContentType::Webm => "video/webm",
        }
        .to_string()
    }

    fn from_extension(ext: &str) -> Self {
        match ext {
            "ogg" => DefaultContentType::Ogg,
            "pdf" => DefaultContentType::Pdf,
            "json" => DefaultContentType::Json,
            "xml" => DefaultContentType::Xml,
            "mpeg" => DefaultContentType::Mpeg,
            "gif" => DefaultContentType::Gif,
            "jpeg" => DefaultContentType::Jpeg,
            "png" => DefaultContentType::Png,
            "tiff" => DefaultContentType::Tiff,
            "svg" => DefaultContentType::SvgXml,
            "css" => DefaultContentType::Css,
            "csv" => DefaultContentType::Csv,
            "html" => DefaultContentType::Html,
            "js" => DefaultContentType::Javascript,
            "txt" => DefaultContentType::Plain,
            "mp4" => DefaultContentType::Mp4,
            "mov" => DefaultContentType::Quicktime,
            "flv" => DefaultContentType::Flv,
            "webm" => DefaultContentType::Webm,
            _ => DefaultContentType::default(),
        }
    }
}

impl Default for DefaultContentType {
    fn default() -> Self {
        Self::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load() -> Result<(), std::io::Error> {
        let path = PathBuf::from("forge");
        let forge = Forge::new(path, 0)?;
        forge.inner.print();
        Ok(())
    }
}
