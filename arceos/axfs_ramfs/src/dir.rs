use crate::alloc::string::ToString;
use crate::dir;
use crate::file::FileNode;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};
use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType, VfsOps};
use axfs_vfs::{VfsError, VfsResult};
use log::debug;
use spin::RwLock;

/// The directory node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].

pub struct DirNode {
    this: Weak<DirNode>,
    parent: RwLock<Weak<dyn VfsNodeOps>>,
    children: RwLock<BTreeMap<String, VfsNodeRef>>,
}
impl DirNode {
    pub(super) fn new(parent: Option<Weak<dyn VfsNodeOps>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Returns a string list of all entries in this directory.
    pub fn get_entries(&self) -> Vec<String> {
        self.children.read().keys().cloned().collect()
    }

    /// Checks whether a node with the given name exists in this directory.
    pub fn exist(&self, name: &str) -> bool {
        self.children.read().contains_key(name)
    }

    /// Creates a new node with the given name and type in this directory.
    pub fn create_node(&self, name: &str, ty: VfsNodeType) -> VfsResult {
        if self.exist(name) {
            log::error!("AlreadyExists {}", name);
            return Err(VfsError::AlreadyExists);
        }
        let node: VfsNodeRef = match ty {
            VfsNodeType::File => Arc::new(FileNode::new()),
            VfsNodeType::Dir => Self::new(Some(self.this.clone())),
            _ => return Err(VfsError::Unsupported),
        };
        debug!("create_node: name = '{}', type = {:?}", name, ty);
        debug!("create_node: created node ptr = {:p}", Arc::as_ptr(&node));
        self.children.write().insert(name.into(), node);

        Ok(())
    }

    /// Removes a node by the given name in this directory.
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let node = children.get(name).ok_or(VfsError::NotFound)?;
        if let Some(dir) = node.as_any().downcast_ref::<DirNode>() {
            if !dir.children.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }
        children.remove(name);
        Ok(())
    }
    // find root
    // pub fn find_root(self: &Arc<DirNode>) -> Arc<DirNode> {
    //     let mut current: Arc<DirNode> = self.clone();

    //     loop {
    //         // 限定 parent_weak 的作用域，避免借用跨 current 赋值
    //         let parent_dir_arc_opt = {
    //             let parent_weak = current.parent.read();
    //             match parent_weak.upgrade() {
    //                 Some(parent_arc) => {
    //                     // parent 是 dyn VfsNodeOps，尝试转换为 DirNode
    //                     if let Some(parent_dir) = parent_arc.as_any().downcast_ref::<DirNode>() {
    //                         // 注意 downcast_ref 返回 &DirNode，不是 Arc
    //                         // 需要从 Weak 升级成 Arc，故先升级 Weak
    //                         parent_dir.this.upgrade()
    //                     } else {
    //                         // 父节点不是 DirNode（比如文件节点），无法继续向上找根，返回当前
    //                         None
    //                     }
    //                 }
    //                 None => {
    //                     // 没有父节点，当前就是根节点
    //                     None
    //                 }
    //             }
    //         };
    //         if let Some(parent_dir_arc) = parent_dir_arc_opt {
    //             current = parent_dir_arc;
    //             continue; // 继续往上找
    //         } else {
    //             break;
    //         }
    //     }
    //     current
    // }
}

impl VfsNodeOps for DirNode {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_dir(4096, 0))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let (name, rest) = split_path(path);
        debug!(
            "lookup: path = '{}', current node = {:p}, name = '{}', rest = {:?}",
            path,
            Arc::as_ptr(&self),
            name,
            rest
        );

        let node = match name {
            "" | "." => {
                debug!("-> current directory");
                Ok(self.clone() as VfsNodeRef)
            }
            ".." => {
                debug!("-> parent directory");
                self.parent().ok_or(VfsError::NotFound)
            }
            _ => {
                let children = self.children.read();
                if let Some(child) = children.get(name) {
                    debug!(
                        "-> found child '{}': {:p}",
                        name,
                        Arc::as_ptr(&child.clone())
                    );
                    Ok(child.clone())
                } else {
                    debug!("-> child '{}' not found in current node", name);
                    Err(VfsError::NotFound)
                }
            }
        }?;

        if let Some(rest) = rest {
            debug!("-> descending into '{}'", rest);
            node.lookup(rest)
        } else {
            debug!("-> final node reached: {:p}", Arc::as_ptr(&node));
            Ok(node)
        }
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        let children = self.children.read();
        let mut children = children.iter().skip(start_idx.max(2) - 2);
        for (i, ent) in dirents.iter_mut().enumerate() {
            match i + start_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, node)) = children.next() {
                        *ent = VfsDirEntry::new(name, node.get_attr().unwrap().file_type());
                    } else {
                        return Ok(i);
                    }
                }
            }
        }
        Ok(dirents.len())
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        log::debug!("create {:?} at ramfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.create(rest, ty),
                ".." => self.parent().ok_or(VfsError::NotFound)?.create(rest, ty),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.create(rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Ok(()) // already exists
        } else {
            self.create_node(name, ty)
        }
    }

    fn remove(&self, path: &str) -> VfsResult {
        log::debug!("remove at ramfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.remove(rest),
                ".." => self.parent().ok_or(VfsError::NotFound)?.remove(rest),
                _ => {
                    let subdir = self
                        .children
                        .read()
                        .get(name)
                        .ok_or(VfsError::NotFound)?
                        .clone();
                    subdir.remove(rest)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Err(VfsError::InvalidInput) // remove '.' or '..
        } else {
            self.remove_node(name)
        }
    }

    fn rename(&self, old_path: &str, new_path: &str) -> VfsResult<()> {
        debug!("rename: {} -> {}", old_path, new_path);

        // 解析 old_path，获得 old_dir_path 和 old_name
        let (_, old_name) = split_parent(old_path)?;
        let (_, new_name) = split_parent(new_path)?;
        // 从 root 开始查找 old_dir
        let old_dir = self.this.upgrade().ok_or(VfsError::NotFound)?;

        // 移除 old_node
        let old_node = {
            let mut old_children = old_dir.children.write();
            old_children.remove(old_name).ok_or(VfsError::NotFound)?
        };
        old_dir.children.write().insert(new_name.to_string(), old_node);
        Ok(())
    }
    // fn rename(&self, old_path: &str, new_path: &str) -> VfsResult<()> {
    //     debug!("rename: {} -> {}", old_path, new_path);

    //     // 先获得 root 节点
    //     let current_dir = self.this.upgrade().ok_or(VfsError::NotFound)?;
    //     let root = current_dir.find_root();
    //     let parent_node = self.parent();
    //     debug!("Root children:");
    //     for (k, v) in root.children.read().iter() {
    //         debug!("  {} => {:p}", k, Arc::as_ptr(v));
    //     }
    //     debug!(
    //         "DEBUG Node Info:
    //         self ptr: {:p}
    //         root ptr: {:p}
    //         parent ptr: {}
    //         ",
    //         Arc::as_ptr(&current_dir),
    //         Arc::as_ptr(&root),
    //         parent_node
    //             .as_ref()
    //             .map(|p| format!("{:p}", Arc::as_ptr(p)))
    //             .unwrap_or_else(|| "None".into())
    //     );
    //     // 解析 old_path，获得 old_dir_path 和 old_name
    //     let (old_dir_path, old_name) = split_parent(old_path)?;
    //     // 从 root 开始查找 old_dir
    //     let old_dir_node = current_dir.clone().lookup(old_dir_path)?;
    //     let old_dir_ref = old_dir_node
    //         .as_any()
    //         .downcast_ref::<DirNode>()
    //         .ok_or(VfsError::NotADirectory)?;
    //     let old_dir = old_dir_ref.this.upgrade().ok_or(VfsError::NotFound)?;
    //     // let self_arc = self.this.upgrade().unwrap();
    //     // debug!("old_dir ptr: {:p}", Arc::as_ptr(&old_dir));
    //     // debug!("self ptr: {:p}", Arc::as_ptr(&self_arc));
    //     for name in old_dir.children.read().keys() {
    //         debug!("child in old_dir: {}", name);
    //     }

    //     // 移除 old_node
    //     let old_node = {
    //         let mut old_children = old_dir.children.write();
    //         old_children.remove(old_name).ok_or(VfsError::NotFound)?
    //     };
    //     // 解析 new_path，获得 new_dir_path 和 new_name
    //     let (new_dir_path, new_name) = split_parent(new_path)?;
    //     // 从 root 开始查找 new_dir
    //     debug!("new_dir_path is {}, new_name is {}", new_dir_path, new_name);
    //     let new_dir_node = root.clone().lookup(new_dir_path)?;
    //     let new_dir_ref = new_dir_node
    //         .as_any()
    //         .downcast_ref::<DirNode>()
    //         .ok_or(VfsError::NotADirectory)?;
    //     let new_dir = new_dir_ref.this.upgrade().ok_or(VfsError::NotFound)?;
    //     // 插入新节点

    //     let mut new_children = new_dir.children.write();
    //     if new_children.contains_key(new_name) {
    //         return Err(VfsError::AlreadyExists);
    //     }
    //     new_children.insert(new_name.to_string(), old_node);

    //     Ok(())
    // }

    axfs_vfs::impl_vfs_dir_default! {}
}

fn split_path(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}

/// 拆分出父目录路径 + 文件名，例如 "/tmp/f1" => ("/tmp", "f1")
fn split_parent(path: &str) -> VfsResult<(&str, &str)> {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(pos) if pos > 0 => Ok((&trimmed[..pos], &trimmed[pos + 1..])),
        Some(0) => Ok(("/", &trimmed[1..])),
        None => Err(VfsError::InvalidInput),
        _ => Err(VfsError::InvalidInput),
    }
}
