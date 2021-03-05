use crate::util::Id;

type Oid = Id;

trait HasOid {
    fn oid(&self) -> Oid;
}

trait HasType {
    fn otype(&self) -> Type;
}

#[derive(Debug, Copy, Clone)]
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
    pub oid: Oid,
    pub parent: Vec<Oid>,
    pub tree: Oid,
    pub comment: String,
    // TODO
}

impl HasOid for Commit { fn oid(&self) -> Oid { self.oid } }
impl HasType for Commit { fn otype(&self) -> Type { Type::Commit }}

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub oid: Oid,
    pub mode: Type,
    pub name: String,
}

impl HasOid for TreeEntry { fn oid(&self) -> Oid { self.oid } }

#[derive(Debug, Clone)]
pub struct Tree {
    pub oid: Oid,
    pub entry: Vec<TreeEntry>,
}

impl HasOid for Tree { fn oid(&self) -> Oid { self.oid } }
impl HasType for Tree { fn otype(&self) -> Type { Type::Tree }}

#[derive(Debug, Clone)]
pub struct Blob {
    pub oid: Oid,
    pub mode: i32, // if unknown, use 644
    pub data: Option<Vec<u8>>,
}

impl HasOid for Blob { fn oid(&self) -> Oid { self.oid } }
impl HasType for Blob { fn otype(&self) -> Type { Type::Blob(self.mode) }}

#[derive(Debug, Clone)]
pub struct Tag {
    pub oid: Oid,
    pub data: Option<Vec<u8>>,
}

impl HasOid for Tag { fn oid(&self) -> Oid { self.oid } }
impl HasType for Tag { fn otype(&self) -> Type { Type::Tag }}

#[derive(Debug, Clone)]
pub enum Object {
    Commit(Commit),
    Blob(Blob),
    Tree(Tree),
    Tag(Tag),
}

impl HasOid for Object {
    fn oid(&self) -> Oid {
        match self {
            Object::Commit(x) => x.oid(),
            Object::Blob(x)   => x.oid(),
            Object::Tree(x)   => x.oid(),
            Object::Tag(x)    => x.oid(),
        }
    }
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
