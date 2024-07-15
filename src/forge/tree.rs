// Jackson Coxson

use std::collections::HashMap;

use super::{ForgeEntry, LoadReturn};

#[derive(Default)]
pub struct Node {
    pub children: HashMap<String, Node>,
    pub files: HashMap<String, ForgeEntry>,
    depth: usize,
    pub hidden: bool,
}

pub enum NodeTraverseReturn<'a> {
    File(ForgeEntry),
    Dir(&'a Node),
}

impl Node {
    /// Prints the tree structure to the console
    pub fn print(&self) {
        // Start with the children
        for (name, node) in &self.children {
            println!(
                "{}{}{}",
                "-".repeat(self.depth),
                name,
                if node.hidden { " (hidden)" } else { "" }
            );
            node.print();
        }
        for (name, file) in self.files.iter() {
            println!(
                "{}{}{}",
                "-".repeat(self.depth),
                name,
                if file.hidden { " (hidden)" } else { "" }
            );
        }
    }

    /// Recursively gets a file given a path
    pub fn traverse(&self, mut path: Vec<&str>) -> Option<NodeTraverseReturn> {
        if path.is_empty() {
            Some(NodeTraverseReturn::Dir(self))
        } else {
            let name = path.remove(0);
            if let Some(node) = self.children.get(name) {
                if path.is_empty() {
                    Some(NodeTraverseReturn::Dir(node))
                } else {
                    node.traverse(path)
                }
            } else if let Some(file) = self.files.get(name) {
                let file = file.to_owned();
                Some(NodeTraverseReturn::File(file))
            } else {
                None
            }
        }
    }

    pub fn add_file(&mut self, name: &str, entry: ForgeEntry) {
        self.files.insert(name.to_string(), entry);
    }

    pub fn add_child(&mut self, name: &str, node: Node) {
        self.children.insert(name.to_string(), node);
    }

    pub fn take_first_child(self) -> Option<Node> {
        if let Some(child) = self.children.into_iter().next() {
            return Some(child.1);
        }
        None
    }
}

impl From<(Vec<(String, Node)>, Vec<(String, ForgeEntry)>, usize, bool)> for Node {
    fn from(val: (Vec<(String, Node)>, Vec<(String, ForgeEntry)>, usize, bool)) -> Self {
        Node {
            children: val.0.into_iter().collect(),
            files: val.1.into_iter().collect(),
            depth: val.2,
            hidden: val.3,
        }
    }
}

impl From<Vec<LoadReturn>> for Node {
    fn from(val: Vec<LoadReturn>) -> Self {
        let mut node = Node::default();
        for load in val {
            match load {
                super::LoadReturn::Entry((name, entry)) => node.add_file(&name, entry),
                super::LoadReturn::Node((name, n)) => node.add_child(&name, n),
            }
        }
        node
    }
}
