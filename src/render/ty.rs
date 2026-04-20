use rustdoc_types::{
    DynTrait, FunctionPointer, GenericArg, GenericArgs, GenericBound, Path, PolyTrait, Type,
};
use std::fmt::Write;

/// Format a rustdoc `Type` into a Rust-syntax string, best-effort.
pub fn ty(t: &Type) -> String {
    let mut out = String::new();
    write_ty(&mut out, t);
    out
}

fn write_ty(out: &mut String, t: &Type) {
    match t {
        Type::ResolvedPath(p) => write_path(out, p),
        Type::DynTrait(d) => write_dyn_trait(out, d),
        Type::Generic(s) | Type::Primitive(s) => out.push_str(s),
        Type::FunctionPointer(fp) => write_fn_pointer(out, fp),
        Type::Tuple(types) => {
            out.push('(');
            for (i, sub) in types.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_ty(out, sub);
            }
            if types.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        Type::Slice(inner) => {
            out.push('[');
            write_ty(out, inner);
            out.push(']');
        }
        Type::Array { type_, len } => {
            out.push('[');
            write_ty(out, type_);
            let _ = write!(out, "; {len}]");
        }
        Type::Pat { type_, .. } => write_ty(out, type_),
        Type::ImplTrait(bounds) => {
            out.push_str("impl ");
            write_bounds(out, bounds);
        }
        Type::Infer => out.push('_'),
        Type::RawPointer { is_mutable, type_ } => {
            out.push_str(if *is_mutable { "*mut " } else { "*const " });
            write_ty(out, type_);
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            out.push('&');
            if let Some(lt) = lifetime {
                out.push_str(lt);
                out.push(' ');
            }
            if *is_mutable {
                out.push_str("mut ");
            }
            write_ty(out, type_);
        }
        Type::QualifiedPath {
            name,
            args,
            self_type,
            trait_,
        } => {
            if let Some(tr) = trait_ {
                out.push('<');
                write_ty(out, self_type);
                out.push_str(" as ");
                write_path(out, tr);
                out.push('>');
            } else {
                write_ty(out, self_type);
            }
            out.push_str("::");
            out.push_str(name);
            if let Some(a) = args {
                write_generic_args(out, a);
            }
        }
    }
}

fn write_path(out: &mut String, p: &Path) {
    out.push_str(&p.path);
    if let Some(args) = &p.args {
        write_generic_args(out, args);
    }
}

fn write_generic_args(out: &mut String, args: &GenericArgs) {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            if args.is_empty() && constraints.is_empty() {
                return;
            }
            out.push('<');
            let mut first = true;
            for a in args {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                match a {
                    GenericArg::Lifetime(s) => out.push_str(s),
                    GenericArg::Type(t) => write_ty(out, t),
                    GenericArg::Const(c) => out.push_str(&c.expr),
                    GenericArg::Infer => out.push('_'),
                }
            }
            for c in constraints {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&c.name);
                // Skipping constraint args/bounds detail for v0 - just show name.
            }
            out.push('>');
        }
        GenericArgs::Parenthesized { inputs, output } => {
            out.push('(');
            for (i, t) in inputs.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_ty(out, t);
            }
            out.push(')');
            if let Some(o) = output {
                out.push_str(" -> ");
                write_ty(out, o);
            }
        }
        GenericArgs::ReturnTypeNotation => {
            out.push_str("(..)");
        }
    }
}

fn write_dyn_trait(out: &mut String, d: &DynTrait) {
    out.push_str("dyn ");
    for (i, pt) in d.traits.iter().enumerate() {
        if i > 0 {
            out.push_str(" + ");
        }
        write_poly_trait(out, pt);
    }
    if let Some(lt) = &d.lifetime {
        out.push_str(" + ");
        out.push_str(lt);
    }
}

fn write_poly_trait(out: &mut String, pt: &PolyTrait) {
    // HRTBs (for<'a>) omitted for brevity in v0.
    write_path(out, &pt.trait_);
}

fn write_bounds(out: &mut String, bounds: &[GenericBound]) {
    for (i, b) in bounds.iter().enumerate() {
        if i > 0 {
            out.push_str(" + ");
        }
        match b {
            GenericBound::TraitBound { trait_, .. } => write_path(out, trait_),
            GenericBound::Outlives(lt) => out.push_str(lt),
            GenericBound::Use(_) => out.push_str("use<..>"),
        }
    }
}

fn write_fn_pointer(out: &mut String, fp: &FunctionPointer) {
    out.push_str("fn(");
    for (i, (_, t)) in fp.sig.inputs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_ty(out, t);
    }
    out.push(')');
    if let Some(o) = &fp.sig.output {
        out.push_str(" -> ");
        write_ty(out, o);
    }
}
