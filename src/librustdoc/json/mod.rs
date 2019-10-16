use std::cell::RefCell;
use std::io::Write;
use std::fs::{create_dir_all, File, OpenOptions};
use std::path::PathBuf;
use std::rc::Rc;

use crate::clean;
use crate::config::{RenderInfo, RenderOptions};
use crate::docfs::PathError;
use crate::doctree;
use crate::error::Error;
use crate::formats::{FormatRenderer, get_attributes};

use rustc::util::nodemap::FxHashMap;
use serialize::json::{ToJson, Json, as_json};
use syntax::symbol::{Symbol, sym};
use syntax::ast;
use syntax::edition::Edition;

#[derive(Clone)]
pub struct Context {
    pub current: Vec<String>,
    file: Rc<RefCell<File>>,
    mods: Rc<RefCell<FxHashMap<String, usize>>>,
    path: Rc<PathBuf>,
}

struct Elem {
    name: String,
    doc: String,
    type_: String,
    // variants on enums, arguments on functions, fields on structs
    /*fields: Vec<Elem>,
    functions: Vec<Elem>,
    trait_impls: Vec<Elem>,
    blanket_impls: Vec<Elem>,
    auto_impls: Vec<Elem>,*/
}

impl FormatRenderer for Context {
    type Output = Self;

    fn init(
        krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
    ) -> Result<(Context, clean::Crate), Error> {
        let RenderOptions {
            output,
            ..
        } = options;

        let path = output.join(&format!("{}.json", krate.name));
        create_dir_all(output).map_err(|e| Error::new(e, &path))?;
        let mut cx = Context {
            current: Vec::new(),
            file: Rc::new(RefCell::new(OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| Error::new(e, &path))?)),
            mods: Rc::new(RefCell::new(FxHashMap::default())),
            path: Rc::new(path),
        };
        Ok((cx, krate))
    }

    fn after_run(&mut self, diag: &errors::Handler) -> Result<(), Error> {
        Ok(())
    }

    /// Main method for rendering a crate.
    ///
    /// This currently isn't parallelized, but it'd be pretty easy to add
    /// parallelization to this function.
    fn before_krate(&mut self, _krate: &clean::Crate) -> Result<(), Error> {
        Ok(())
    }

    fn after_krate(&mut self, krate: &clean::Crate) -> Result<(), Error> {
        Ok(())
    }

    fn mod_item_in(
        &mut self,
        item: &clean::Item,
        item_name: &str,
        full_parent_item_path: &str,
    ) -> Result<(), Error> {
        let nb: usize = *self.mods.borrow().get(full_parent_item_path).unwrap_or_else(|| &0);
        self.render_mod(item, nb)?;
        *(self.mods.borrow_mut().entry(full_parent_item_path.to_owned()).or_insert_with(|| 0)) += 1;
        Ok(())
    }

    fn mod_item_out(&mut self, name: &str, full_item_path: &str) -> Result<(), Error> {
        // Go back to where we were at
        // self.current.pop();
        if self.mods.borrow().contains_key(&full_item_path.to_owned()) {
            self.mods.borrow_mut().remove(full_item_path);
        }
        self.write("]}")
    }

    fn item(&mut self, item: clean::Item, full_item_path: &str) -> Result<(), Error> {
        println!("item");
        let nb: usize = *self.mods.borrow().get(full_item_path).unwrap_or_else(|| &0);
        self.render_item(item, nb)?;
        *(self.mods.borrow_mut().entry(full_item_path.to_owned()).or_insert_with(|| 0)) += 1;
        Ok(())
    }
}

impl Context {
    fn render_mod(&mut self, item: &clean::Item, nb_items: usize) -> Result<(), Error> {
        println!("render_mod");
        self.write(
            &format!("{comma}{{\"name\":{name},\"doc\":{doc},\"kind\":\"module\",\"items\":[",
                     comma=if nb_items > 0 { "," } else { "" },
                     name=item.name.as_ref().unwrap().to_json(),
                     doc=item.doc_value().unwrap_or_else(|| "").to_json()))
    }

    fn render_item(&mut self, item: clean::Item, nb_items: usize) -> Result<(), Error> {
        println!("render_item");
        let mut buf = format!("{comma}{{\"name\":{name},\"doc\":{doc},\"type\":{kind}",
                              comma=if nb_items > 0 { "," } else { "" },
                              name=item.name.as_ref().unwrap().to_json(),
                              doc=item.doc_value().unwrap_or_else(|| "").to_json(),
                              kind=item.type_().to_string().to_json());
        let attrs = get_attributes(&item);
        if !attrs.is_empty() {
            buf.push_str(&format!(",\"attributes\":[{}]",
                                  attrs
                                      .iter()
                                      .map(|a| a.to_json().to_string())
                                      .collect::<Vec<_>>()
                                      .join(","))); // replace with to json
        }
        match item.inner {
            clean::ModuleItem(ref m) => {
                // we do nothing
            }
            // clean::FunctionItem(ref f) | clean::ForeignFunctionItem(ref f) =>
            //     item_function(buf, cx, item, f),
            // clean::TraitItem(ref t) => item_trait(buf, cx, item, t),
            clean::StructItem(ref s) => self.render_struct(&mut buf, &item, s),
            // clean::UnionItem(ref s) => item_union(buf, cx, item, s),
            // clean::EnumItem(ref e) => item_enum(buf, cx, item, e),
            // clean::TypedefItem(ref t, _) => item_typedef(buf, cx, item, t),
            // clean::MacroItem(ref m) => item_macro(buf, cx, item, m),
            // clean::ProcMacroItem(ref m) => item_proc_macro(buf, cx, item, m),
            // clean::PrimitiveItem(_) => item_primitive(buf, cx, item),
            // clean::StaticItem(ref i) | clean::ForeignStaticItem(ref i) =>
            //     item_static(buf, cx, item, i),
            // clean::ConstantItem(ref c) => item_constant(buf, cx, item, c),
            // clean::ForeignTypeItem => item_foreign_type(buf, cx, item),
            // clean::KeywordItem(_) => item_keyword(buf, cx, item),
            // clean::OpaqueTyItem(ref e, _) => item_opaque_ty(buf, cx, item, e),
            // clean::TraitAliasItem(ref ta) => item_trait_alias(buf, cx, item, ta),
            _ => {
                // We don't generate pages for any other type.
            }
        }
        buf.push_str("}");
        self.write(&buf)
    }

    fn render_struct(&self, w: &mut String, it: &clean::Item, s: &clean::Struct) {
        self.render_generics(w, s);
        match s.struct_type {
            doctree::Plain => {
                let f = s.fields.iter()
                    .filter_map(|f| {
                        if let clean::StructFieldItem(ref ty) = f.inner {
                            Some(format!("{{\"name\":{},\"type-name\":{}}}",
                                f.name.as_ref().unwrap(),
                                ty.as_str(),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                w.push_str(
                    &format!(",\"kind\":\"plain\",\"fields\":{},\"has-hidden-fields\":{}",
                        f.to_json(),
                        (!s.fields.is_empty() && f.is_empty()) || it.has_stripped_fields().unwrap(),
                    ));
            }
            doctree::Tuple => {
                let f = s.fields.iter()
                    .filter_map(|f| {
                        match f.inner {
                            clean::StrippedItem(box clean::StructFieldItem(..)) => {
                                Some("_".to_owned())
                            }
                            clean::StructFieldItem(ref ty) => {
                                Some(ty.as_str())
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                w.push_str(&format!(",\"kind\":\"tuple\",\"fields\":{}", f.to_json()));
            }
            doctree::Unit => {
                w.push_str(",\"kind\":\"unit\"");
            }
        }
    }

    fn render_generics<T: clean::GetGenerics>(&self, w: &mut String, it: &T) {
        let real_params = it.generics().get_params();
        if real_params.is_empty() {
            return;
        }
        let real_params = real_params.into_iter()
            .map(|p| p.as_str())
            .collect::<Vec<String>>();
        w.push_str(&format!(",\"generics\":{}", real_params.to_json()));
    }

    fn write(&mut self, s: &str) -> Result<(), Error> {
        write!(*self.file.borrow_mut(), "{}", s).map_err(|e| Error::new(e, &*self.path))
    }
}
