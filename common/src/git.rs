use std::collections::BTreeSet;

use crate::util::Id;

pub type Oid = Id;

pub trait HasType {
    fn otype(&self) -> Type;
}

#[derive(Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum Type {
    // 160000
    Commit,
    // 040000
    Tree,
    // file: 100755 or 100644
    // symlink: 120000
    Blob(i32),
    Tag,
}

impl Type {
    pub fn mode(&self) -> i32 {
        match self {
            Type::Commit  => 0o160000,
            Type::Tree    => 0o040000,
            Type::Blob(m) => *m,
            Type::Tag     => 0,
        }
    }

    pub fn from_mode(m: i32) -> Type {
        match m {
            0o160000 => Type::Commit,
            0o040000 => Type::Tree,
            _ => Type::Blob(m),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub parent: Vec<Oid>,
    pub tree: Oid,
    pub comment: String,
    // TODO
}

impl HasType for Commit { fn otype(&self) -> Type { Type::Commit }}

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Clone)]
pub struct TreeEntry {
    pub name: String,
    pub oid: Oid,
    pub mode: Type,
}

// NOTE: Tree should be sorted by name
pub type Tree = BTreeSet<TreeEntry>;

impl HasType for Tree { fn otype(&self) -> Type { Type::Tree }}

#[derive(Debug, Clone)]
pub struct Blob {
    pub mode: i32, // if unknown, use 644
    pub data: Option<Vec<u8>>,
}

impl HasType for Blob { fn otype(&self) -> Type { Type::Blob(self.mode) }}

pub type Tag = Option<Vec<u8>>;

impl HasType for Tag { fn otype(&self) -> Type { Type::Tag }}

#[derive(Debug, Clone)]
pub enum Object {
    Commit(Commit),
    Blob(Blob),
    Tree(Tree),
    Tag(Tag),
}


impl HasType for Object {
    fn otype(&self) -> Type {
        match self {
            Object::Commit(x) => x.otype(),
            Object::Blob(x)   => x.otype(),
            Object::Tree(x)   => x.otype(),
            Object::Tag(x)    => x.otype(),
        }
    }
}
