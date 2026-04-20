use crate::{Error, ItemSpec, Result};
use rustdoc_types::{Crate, Id, Item, ItemEnum, ItemKind};
use std::collections::{HashMap, HashSet};

/// An item resolved from an `ItemSpec`: the `Id`, a reference into `crate.index`
/// if available, and the full path the user asked for.
pub struct Resolved<'a> {
    pub id: Id,
    pub item: Option<&'a Item>,
    pub path: Vec<String>,
    pub kind: Option<ItemKind>,
}

/// # Errors
/// Returns `Error::NotFound` when the spec's path does not match any item in
/// the crate's public module tree or its `paths` index.
pub fn resolve<'a>(krate: &'a Crate, spec: &ItemSpec) -> Result<Resolved<'a>> {
    let target_path = spec.full_path();

    if spec.is_root() {
        let id = krate.root;
        return Ok(Resolved {
            id,
            item: krate.index.get(&id),
            path: vec![spec.crate_name.clone()],
            kind: krate.paths.get(&id).map(|s| s.kind),
        });
    }

    // Build (once per call) a public-path index by walking the module tree
    // starting from `crate.root`. This follows `pub use` re-exports so items
    // show up at their canonical public location (e.g. tokio::sync::Mutex)
    // even when rustdoc stored the defining path (tokio::sync::mutex::Mutex).
    let public = build_public_index(krate, &spec.crate_name);

    if let Some(&id) = public.get(&target_path) {
        let item = krate.index.get(&id);
        let kind = krate.paths.get(&id).map(|s| s.kind);
        return Ok(Resolved {
            id,
            item,
            path: target_path,
            kind,
        });
    }

    // Fallback: match the exact stored path in `crate.paths` (works for items
    // at their defining location even if no pub use reaches them).
    let mut candidates: Vec<(&Id, &rustdoc_types::ItemSummary)> = krate
        .paths
        .iter()
        .filter(|(_, s)| s.path == target_path)
        .collect();
    if !candidates.is_empty() {
        candidates.sort_by_key(|(id, _)| !krate.index.contains_key(id));
        let (id, summary) = candidates[0];
        return Ok(Resolved {
            id: *id,
            item: krate.index.get(id),
            path: summary.path.clone(),
            kind: Some(summary.kind),
        });
    }

    Err(Error::NotFound(target_path.join("::")))
}

/// Walk the crate module tree and produce `public_path -> Id`, following
/// `pub use` re-exports. Each key is a Vec starting with the crate name.
fn build_public_index(krate: &Crate, crate_name: &str) -> HashMap<Vec<String>, Id> {
    let mut out: HashMap<Vec<String>, Id> = HashMap::new();
    let mut visited: HashSet<Id> = HashSet::new();
    let root_path = vec![crate_name.to_string()];
    out.insert(root_path.clone(), krate.root);
    walk_module(krate, krate.root, &root_path, &mut out, &mut visited);
    out
}

fn walk_module(
    krate: &Crate,
    module_id: Id,
    path: &[String],
    out: &mut HashMap<Vec<String>, Id>,
    visited: &mut HashSet<Id>,
) {
    if !visited.insert(module_id) {
        return;
    }
    let Some(module_item) = krate.index.get(&module_id) else {
        return;
    };
    let ItemEnum::Module(m) = &module_item.inner else {
        return;
    };

    for child_id in &m.items {
        let Some(child) = krate.index.get(child_id) else {
            continue;
        };
        if let ItemEnum::Use(u) = &child.inner {
            walk_use(krate, u, path, out, visited);
        } else {
            let Some(name) = child.name.clone() else {
                continue;
            };
            let mut new_path = path.to_vec();
            new_path.push(name);
            out.entry(new_path.clone()).or_insert(*child_id);
            if matches!(child.inner, ItemEnum::Module(_)) {
                walk_module(krate, *child_id, &new_path, out, visited);
            }
        }
    }
}

/// Handle a `pub use source as name;` - expose the target under `name` at the
/// current module level. If `u.is_glob`, inline the target module's children;
/// otherwise record a single mapping to the target `Id`.
fn walk_use(
    krate: &Crate,
    u: &rustdoc_types::Use,
    path: &[String],
    out: &mut HashMap<Vec<String>, Id>,
    visited: &mut HashSet<Id>,
) {
    let Some(target_id) = u.id else { return };
    let alias = if u.name.is_empty() {
        // Fall back to the last segment of the source path.
        u.source.rsplit("::").next().unwrap_or("").to_string()
    } else {
        u.name.clone()
    };
    if u.is_glob {
        if let Some(target) = krate.index.get(&target_id)
            && let ItemEnum::Module(_) = target.inner
        {
            // Walk the target module but attribute items to the *current* path.
            walk_module(krate, target_id, path, out, visited);
        }
    } else {
        let mut new_path = path.to_vec();
        new_path.push(alias);
        out.entry(new_path.clone()).or_insert(target_id);
        if let Some(target) = krate.index.get(&target_id)
            && matches!(target.inner, ItemEnum::Module(_))
        {
            walk_module(krate, target_id, &new_path, out, visited);
        }
    }
}
