mod ty;

use crate::{ItemSpec, resolve::Resolved};
use rustdoc_types::{
    Crate, Enum, Function, Item, ItemEnum, ItemKind, Module, Struct, StructKind, Trait, Union,
    Variant, VariantKind,
};
use std::fmt::Write;

/// Entry point: render a resolved item to Markdown.
#[must_use]
pub fn render(krate: &Crate, resolved: &Resolved<'_>, spec: &ItemSpec) -> String {
    let mut out = String::new();
    header(&mut out, spec, resolved);

    let Some(item) = resolved.item else {
        // Item known from `paths` but not in `index` - typically a re-export to
        // a foreign crate. Show a minimal stub.
        let _ = writeln!(
            out,
            "_This item is re-exported from another crate; full docs unavailable here._\n"
        );
        return out;
    };

    if let Some(docs) = &item.docs {
        out.push_str(docs);
        out.push_str("\n\n");
    }

    match &item.inner {
        ItemEnum::Module(m) => render_module(&mut out, krate, m),
        ItemEnum::Struct(s) => render_struct(&mut out, krate, item, s),
        ItemEnum::Enum(e) => render_enum(&mut out, krate, item, e),
        ItemEnum::Trait(t) => render_trait(&mut out, krate, item, t),
        ItemEnum::Function(f) => render_function(&mut out, item, f),
        ItemEnum::Union(u) => render_union(&mut out, krate, item, u),
        ItemEnum::TypeAlias(ta) => render_type_alias(&mut out, item, ta),
        ItemEnum::Constant { type_, const_ } => {
            let name = item.name.as_deref().unwrap_or("_");
            let _ = writeln!(out, "```rust");
            let _ = writeln!(
                out,
                "pub const {}: {} = {};",
                name,
                ty::ty(type_),
                const_.expr
            );
            let _ = writeln!(out, "```\n");
        }
        ItemEnum::Static(s) => {
            let name = item.name.as_deref().unwrap_or("_");
            let mut_kw = if s.is_mutable { "mut " } else { "" };
            let _ = writeln!(out, "```rust");
            let _ = writeln!(
                out,
                "pub static {mut_kw}{name}: {} = {};",
                ty::ty(&s.type_),
                s.expr
            );
            let _ = writeln!(out, "```\n");
        }
        ItemEnum::Macro(def) => {
            let _ = writeln!(out, "```rust");
            out.push_str(def);
            out.push_str("\n```\n\n");
        }
        ItemEnum::TraitAlias(_) => {
            let _ = writeln!(out, "_(trait alias - signature not yet rendered)_\n");
        }
        ItemEnum::ProcMacro(_) => {
            let _ = writeln!(out, "_Procedural macro._\n");
        }
        ItemEnum::Primitive(_) => {
            let _ = writeln!(out, "_Primitive type._\n");
        }
        _ => {
            let _ = writeln!(out, "_(no renderer for this item kind in v0)_\n");
        }
    }

    out
}

fn header(out: &mut String, spec: &ItemSpec, resolved: &Resolved<'_>) {
    let kind_label = resolved.kind.map_or("item", kind_label);
    let title = resolved.path.join("::");
    let _ = writeln!(out, "# {kind_label} `{title}`\n");
    let krate = &spec.crate_name;
    let version = &spec.version;
    let _ = writeln!(out, "docs.rs: https://docs.rs/{krate}/{version}/{krate}/\n");
}

fn kind_label(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Module => "Module",
        ItemKind::Struct => "Struct",
        ItemKind::Enum => "Enum",
        ItemKind::Trait => "Trait",
        ItemKind::Function => "Function",
        ItemKind::TypeAlias => "Type Alias",
        ItemKind::Constant => "Constant",
        ItemKind::Static => "Static",
        ItemKind::Macro => "Macro",
        ItemKind::Union => "Union",
        ItemKind::Primitive => "Primitive",
        ItemKind::TraitAlias => "Trait Alias",
        ItemKind::ProcAttribute | ItemKind::ProcDerive => "Proc Macro",
        ItemKind::AssocConst => "Associated Constant",
        ItemKind::AssocType => "Associated Type",
        ItemKind::Variant => "Variant",
        ItemKind::StructField => "Field",
        ItemKind::Impl => "Impl",
        ItemKind::ExternCrate => "Extern Crate",
        ItemKind::Use => "Use",
        ItemKind::ExternType => "Extern Type",
        ItemKind::Keyword => "Keyword",
        ItemKind::Attribute => "Attribute",
    }
}

// --- Module -----------------------------------------------------------------

fn render_module(out: &mut String, krate: &Crate, m: &Module) {
    let mut buckets = ItemBuckets::default();
    for id in &m.items {
        let Some(child) = krate.index.get(id) else {
            continue;
        };
        // Skip non-public (rustdoc usually strips these but be defensive).
        let Some(name) = child.name.clone() else {
            continue;
        };
        let summary = first_doc_line(child.docs.as_deref());
        match &child.inner {
            ItemEnum::Module(_) => buckets.modules.push((name, summary)),
            ItemEnum::Struct(_) => buckets.structs.push((name, summary)),
            ItemEnum::Enum(_) => buckets.enums.push((name, summary)),
            ItemEnum::Trait(_) => buckets.traits.push((name, summary)),
            ItemEnum::Function(_) => buckets.functions.push((name, summary)),
            ItemEnum::TypeAlias(_) => buckets.type_aliases.push((name, summary)),
            ItemEnum::Constant { .. } => buckets.constants.push((name, summary)),
            ItemEnum::Static(_) => buckets.statics.push((name, summary)),
            ItemEnum::Macro(_) | ItemEnum::ProcMacro(_) => buckets.macros.push((name, summary)),
            ItemEnum::Union(_) => buckets.unions.push((name, summary)),
            ItemEnum::Use(u) => {
                let target = u.name.clone();
                let src = u.source.clone();
                buckets.reexports.push((target, Some(src)));
            }
            _ => {}
        }
    }

    write_section(out, "Modules", &buckets.modules);
    write_section(out, "Structs", &buckets.structs);
    write_section(out, "Enums", &buckets.enums);
    write_section(out, "Unions", &buckets.unions);
    write_section(out, "Traits", &buckets.traits);
    write_section(out, "Functions", &buckets.functions);
    write_section(out, "Type Aliases", &buckets.type_aliases);
    write_section(out, "Constants", &buckets.constants);
    write_section(out, "Statics", &buckets.statics);
    write_section(out, "Macros", &buckets.macros);
    write_section(out, "Re-exports", &buckets.reexports);
}

#[derive(Default)]
struct ItemBuckets {
    modules: Vec<(String, Option<String>)>,
    structs: Vec<(String, Option<String>)>,
    enums: Vec<(String, Option<String>)>,
    unions: Vec<(String, Option<String>)>,
    traits: Vec<(String, Option<String>)>,
    functions: Vec<(String, Option<String>)>,
    type_aliases: Vec<(String, Option<String>)>,
    constants: Vec<(String, Option<String>)>,
    statics: Vec<(String, Option<String>)>,
    macros: Vec<(String, Option<String>)>,
    reexports: Vec<(String, Option<String>)>,
}

fn write_section(out: &mut String, title: &str, entries: &[(String, Option<String>)]) {
    if entries.is_empty() {
        return;
    }
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_by_key(|e| &e.0);
    let _ = writeln!(out, "## {title}\n");
    for (name, desc) in entries {
        match desc {
            Some(d) if !d.is_empty() => {
                let _ = writeln!(out, "- `{name}` - {d}");
            }
            _ => {
                let _ = writeln!(out, "- `{name}`");
            }
        }
    }
    out.push('\n');
}

fn first_doc_line(docs: Option<&str>) -> Option<String> {
    let raw = docs?.trim();
    if raw.is_empty() {
        return None;
    }
    // First paragraph (up to blank line), compressed to one line.
    let para: String = raw
        .split("\n\n")
        .next()
        .unwrap_or(raw)
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    if para.is_empty() { None } else { Some(para) }
}

// --- Struct / Union ---------------------------------------------------------

fn render_struct(out: &mut String, krate: &Crate, item: &Item, s: &Struct) {
    let name = item.name.as_deref().unwrap_or("_");
    out.push_str("```rust\n");
    match &s.kind {
        StructKind::Unit => {
            let _ = writeln!(out, "pub struct {name};");
        }
        StructKind::Tuple(fields) => {
            let _ = write!(out, "pub struct {name}(");
            let parts: Vec<String> = fields
                .iter()
                .map(|id| match id.as_ref().and_then(|i| krate.index.get(i)) {
                    Some(it) => match &it.inner {
                        ItemEnum::StructField(t) => format!("pub {}", ty::ty(t)),
                        _ => "_".into(),
                    },
                    None => "_".into(),
                })
                .collect();
            out.push_str(&parts.join(", "));
            out.push_str(");\n");
        }
        StructKind::Plain { fields, .. } => {
            let _ = writeln!(out, "pub struct {name} {{");
            for fid in fields {
                if let Some(f) = krate.index.get(fid)
                    && let ItemEnum::StructField(t) = &f.inner
                {
                    let fname = f.name.as_deref().unwrap_or("_");
                    let _ = writeln!(out, "    pub {fname}: {},", ty::ty(t));
                }
            }
            out.push_str("}\n");
        }
    }
    out.push_str("```\n\n");

    // Fields section with docs
    if let StructKind::Plain { fields, .. } = &s.kind {
        let with_docs: Vec<_> = fields
            .iter()
            .filter_map(|id| krate.index.get(id))
            .filter(|f| f.docs.as_deref().is_some_and(|d| !d.trim().is_empty()))
            .collect();
        if !with_docs.is_empty() {
            out.push_str("## Fields\n\n");
            for f in with_docs {
                let fname = f.name.as_deref().unwrap_or("_");
                let type_str = if let ItemEnum::StructField(t) = &f.inner {
                    ty::ty(t)
                } else {
                    "_".into()
                };
                let _ = writeln!(out, "### `{fname}: {type_str}`\n");
                if let Some(d) = &f.docs {
                    out.push_str(d.trim());
                    out.push_str("\n\n");
                }
            }
        }
    }
}

fn render_union(out: &mut String, krate: &Crate, item: &Item, u: &Union) {
    let name = item.name.as_deref().unwrap_or("_");
    out.push_str("```rust\n");
    let _ = writeln!(out, "pub union {name} {{");
    for fid in &u.fields {
        if let Some(f) = krate.index.get(fid)
            && let ItemEnum::StructField(t) = &f.inner
        {
            let fname = f.name.as_deref().unwrap_or("_");
            let _ = writeln!(out, "    pub {fname}: {},", ty::ty(t));
        }
    }
    out.push_str("}\n```\n\n");
}

// --- Enum -------------------------------------------------------------------

fn render_enum(out: &mut String, krate: &Crate, item: &Item, e: &Enum) {
    let name = item.name.as_deref().unwrap_or("_");
    out.push_str("```rust\n");
    let _ = writeln!(out, "pub enum {name} {{");
    for vid in &e.variants {
        if let Some(v) = krate.index.get(vid) {
            write_variant_signature(out, krate, v, 1);
        }
    }
    out.push_str("}\n```\n\n");

    // Variants section
    let variants: Vec<_> = e
        .variants
        .iter()
        .filter_map(|id| krate.index.get(id))
        .collect();
    if !variants.is_empty() {
        out.push_str("## Variants\n\n");
        for v in variants {
            let vname = v.name.as_deref().unwrap_or("_");
            let _ = writeln!(out, "### `{vname}`\n");
            if let Some(d) = &v.docs {
                out.push_str(d.trim());
                out.push_str("\n\n");
            }
            if let ItemEnum::Variant(var) = &v.inner {
                write_variant_fields_detail(out, krate, var);
            }
        }
    }
}

fn write_variant_signature(out: &mut String, krate: &Crate, v: &Item, indent: usize) {
    let pad = "    ".repeat(indent);
    let vname = v.name.as_deref().unwrap_or("_");
    let ItemEnum::Variant(var) = &v.inner else {
        let _ = writeln!(out, "{pad}{vname},");
        return;
    };
    match &var.kind {
        VariantKind::Plain => {
            let _ = writeln!(out, "{pad}{vname},");
        }
        VariantKind::Tuple(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|id| match id.as_ref().and_then(|i| krate.index.get(i)) {
                    Some(it) => match &it.inner {
                        ItemEnum::StructField(t) => ty::ty(t),
                        _ => "_".into(),
                    },
                    None => "_".into(),
                })
                .collect();
            let _ = writeln!(out, "{pad}{vname}({}),", parts.join(", "));
        }
        VariantKind::Struct { fields, .. } => {
            let _ = writeln!(out, "{pad}{vname} {{");
            for fid in fields {
                if let Some(f) = krate.index.get(fid)
                    && let ItemEnum::StructField(t) = &f.inner
                {
                    let fname = f.name.as_deref().unwrap_or("_");
                    let _ = writeln!(out, "{pad}    {fname}: {},", ty::ty(t));
                }
            }
            let _ = writeln!(out, "{pad}}},");
        }
    }
}

fn write_variant_fields_detail(out: &mut String, krate: &Crate, v: &Variant) {
    if let VariantKind::Struct { fields, .. } = &v.kind {
        let documented: Vec<_> = fields
            .iter()
            .filter_map(|id| krate.index.get(id))
            .filter(|f| f.docs.as_deref().is_some_and(|d| !d.trim().is_empty()))
            .collect();
        if documented.is_empty() {
            return;
        }
        for f in documented {
            let fname = f.name.as_deref().unwrap_or("_");
            let type_str = if let ItemEnum::StructField(t) = &f.inner {
                ty::ty(t)
            } else {
                "_".into()
            };
            let _ = writeln!(out, "- `{fname}: {type_str}`");
            if let Some(d) = &f.docs {
                let _ = writeln!(out, "  - {}", d.trim().lines().next().unwrap_or(""));
            }
        }
        out.push('\n');
    }
}

// --- Trait ------------------------------------------------------------------

fn render_trait(out: &mut String, krate: &Crate, item: &Item, t: &Trait) {
    let name = item.name.as_deref().unwrap_or("_");
    let unsafe_kw = if t.is_unsafe { "unsafe " } else { "" };
    let auto_kw = if t.is_auto { "auto " } else { "" };
    out.push_str("```rust\n");
    let _ = writeln!(out, "pub {unsafe_kw}{auto_kw}trait {name} {{ /* ... */ }}");
    out.push_str("```\n\n");

    let mut required_methods: Vec<&Item> = Vec::new();
    let mut provided_methods: Vec<&Item> = Vec::new();
    let mut assoc_types: Vec<&Item> = Vec::new();
    let mut assoc_consts: Vec<&Item> = Vec::new();

    for id in &t.items {
        let Some(child) = krate.index.get(id) else {
            continue;
        };
        match &child.inner {
            ItemEnum::Function(f) => {
                if f.has_body {
                    provided_methods.push(child);
                } else {
                    required_methods.push(child);
                }
            }
            ItemEnum::AssocType { .. } => assoc_types.push(child),
            ItemEnum::AssocConst { .. } => assoc_consts.push(child),
            _ => {}
        }
    }

    write_assoc_types(out, &assoc_types);
    write_assoc_consts(out, &assoc_consts);
    write_methods(out, "Required Methods", &required_methods);
    write_methods(out, "Provided Methods", &provided_methods);
}

fn write_assoc_types(out: &mut String, items: &[&Item]) {
    if items.is_empty() {
        return;
    }
    out.push_str("## Associated Types\n\n");
    for it in items {
        let name = it.name.as_deref().unwrap_or("_");
        let _ = writeln!(out, "### `type {name}`\n");
        if let Some(d) = &it.docs {
            out.push_str(d.trim());
            out.push_str("\n\n");
        }
    }
}

fn write_assoc_consts(out: &mut String, items: &[&Item]) {
    if items.is_empty() {
        return;
    }
    out.push_str("## Associated Constants\n\n");
    for it in items {
        let name = it.name.as_deref().unwrap_or("_");
        let tstr = if let ItemEnum::AssocConst { type_, .. } = &it.inner {
            ty::ty(type_)
        } else {
            "_".into()
        };
        let _ = writeln!(out, "### `const {name}: {tstr}`\n");
        if let Some(d) = &it.docs {
            out.push_str(d.trim());
            out.push_str("\n\n");
        }
    }
}

fn write_methods(out: &mut String, title: &str, items: &[&Item]) {
    if items.is_empty() {
        return;
    }
    let _ = writeln!(out, "## {title}\n");
    for it in items {
        let ItemEnum::Function(f) = &it.inner else {
            continue;
        };
        let name = it.name.as_deref().unwrap_or("_");
        let sig = format_fn_signature(name, f);
        let _ = writeln!(out, "### `{name}`\n");
        out.push_str("```rust\n");
        out.push_str(&sig);
        out.push_str("\n```\n\n");
        if let Some(d) = &it.docs {
            out.push_str(d.trim());
            out.push_str("\n\n");
        }
    }
}

// --- Function ---------------------------------------------------------------

fn render_function(out: &mut String, item: &Item, f: &Function) {
    let name = item.name.as_deref().unwrap_or("_");
    out.push_str("```rust\n");
    out.push_str(&format_fn_signature(name, f));
    out.push_str("\n```\n\n");
}

fn format_fn_signature(name: &str, f: &Function) -> String {
    let mut s = String::new();
    if f.header.is_const {
        s.push_str("const ");
    }
    if f.header.is_async {
        s.push_str("async ");
    }
    if f.header.is_unsafe {
        s.push_str("unsafe ");
    }
    let _ = write!(s, "fn {name}(");
    for (i, (arg_name, arg_ty)) in f.sig.inputs.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        if arg_name == "self" {
            // Render receiver as just the type (&self, &mut self, self, etc.).
            s.push_str(&format_self_receiver(arg_ty));
        } else {
            let _ = write!(s, "{arg_name}: {}", ty::ty(arg_ty));
        }
    }
    if f.sig.is_c_variadic {
        if !f.sig.inputs.is_empty() {
            s.push_str(", ");
        }
        s.push_str("...");
    }
    s.push(')');
    if let Some(o) = &f.sig.output {
        let _ = write!(s, " -> {}", ty::ty(o));
    }
    s.push(';');
    s
}

fn format_self_receiver(t: &rustdoc_types::Type) -> String {
    // Special-case common self receivers for readability.
    match t {
        rustdoc_types::Type::Generic(g) if g == "Self" => "self".into(),
        rustdoc_types::Type::BorrowedRef {
            is_mutable,
            lifetime,
            type_,
        } => {
            if let rustdoc_types::Type::Generic(g) = &**type_
                && g == "Self"
            {
                let mut s = String::from("&");
                if let Some(lt) = lifetime {
                    s.push_str(lt);
                    s.push(' ');
                }
                if *is_mutable {
                    s.push_str("mut ");
                }
                s.push_str("self");
                return s;
            }
            format!("self: {}", ty::ty(t))
        }
        other => format!("self: {}", ty::ty(other)),
    }
}

// --- TypeAlias --------------------------------------------------------------

fn render_type_alias(out: &mut String, item: &Item, ta: &rustdoc_types::TypeAlias) {
    let name = item.name.as_deref().unwrap_or("_");
    out.push_str("```rust\n");
    let _ = writeln!(out, "pub type {name} = {};", ty::ty(&ta.type_));
    out.push_str("```\n\n");
}
