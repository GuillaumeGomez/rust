use std::cell::RefCell;
use std::io::Write;
use std::fs::{create_dir_all, File, OpenOptions};
use std::path::PathBuf;
use std::rc::Rc;

use crate::clean;
use crate::config::{RenderInfo, RenderOptions};
use crate::docfs::PathError;
use crate::error::Error;
use crate::formats::{FormatRenderer, get_attributes};

use rustc::util::nodemap::FxHashMap;
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
        self.write(&format!("{comma}{{\"name\":\"{name}\",\"doc\":\"{doc}\",\"items\":[",
                            comma=if nb_items > 0 { "," } else { "" },
                            name=item.name.as_ref().unwrap(),
                            doc=item.doc_value()
                                    .unwrap_or_else(|| "")
                                    .replace("\\", "\\\\")
                                    .replace("\"", "\\\""))) // replace with to json
    }

    fn render_item(&mut self, item: clean::Item, nb_items: usize) -> Result<(), Error> {
        println!("render_item");
        let mut s = format!("{comma}{{\"name\":\"{name}\",\"doc\":\"{doc}\"",
                            comma=if nb_items > 0 { "," } else { "" },
                            name=item.name.as_ref().unwrap(),
                            doc=item.doc_value()
                                .unwrap_or_else(|| "")
                                .replace("\\", "\\\\")
                                .replace("\"", "\\\"")); // replace with to json
        let attrs = get_attributes(&item);
        if !attrs.is_empty() {
            s.push_str(&format!(",\"attributes\":[{}]",
                                attrs
                                    .iter()
                                    .map(|a| format!("\"{}\"", a))
                                    .collect::<Vec<_>>()
                                    .join(","))); // replace with to json
        }
        s.push_str("}");
        self.write(&s)
    }


    fn write(&mut self, s: &str) -> Result<(), Error> {
        write!(*self.file.borrow_mut(), "{}", s).map_err(|e| Error::new(e, &*self.path))
    }
}
