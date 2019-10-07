use crate::clean;
use crate::config::{RenderInfo, RenderOptions};
use crate::error::Error;

use rustc::util::nodemap::FxHashMap;
use syntax::symbol::{Symbol, sym};
use syntax::ast;
use syntax::edition::Edition;

pub trait FormatRenderer: Clone {
    type Output: FormatRenderer;

    fn init(
        krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
    ) -> Result<(Self::Output, clean::Crate), Error>;

    fn after_run(&mut self, diag: &errors::Handler) -> Result<(), Error>;

    fn before_krate(
        &mut self,
        krate: &clean::Crate,
    ) -> Result<(), Error>;
    fn after_krate(
        &mut self,
        krate: &clean::Crate,
    ) -> Result<(), Error>;

    fn item(
        &mut self,
        item: clean::Item,
        full_item_path: &str,
    ) -> Result<(), Error>;

    fn mod_item_in(
        &mut self,
        item: &clean::Item,
        item_name: &str,
        full_parent_item_path: &str,
    ) -> Result<(), Error>;
    fn mod_item_out(
        &mut self,
        name: &str,
        full_item_path: &str,
    ) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct Renderer;

impl Renderer {
    pub fn new() -> Renderer {
        Renderer
    }

    pub fn run<T: FormatRenderer + Clone>(
        self,
        krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
    ) -> Result<(), Error> {
        let (mut renderer, mut krate) = T::init(
            krate, options, renderinfo, diag, edition,
        )?;
        let mut item = match krate.module.take() {
            Some(i) => i,
            None => return Ok(()),
        };

        item.name = Some(krate.name.clone());

        // Render the crate documentation
        let mut work = vec![(renderer.clone(), item, String::new())];

        renderer.before_krate(&krate)?;

        let mut mods = FxHashMap::default();

        while let Some((mut cx, item, mut parent)) = work.pop() {
            if item.is_mod() {
                // modules are special because they add a namespace. We also need to
                // recurse into the items of the module as well.
                let name = item.name.as_ref().unwrap().to_string();
                if name.is_empty() {
                    panic!("Unexpected module with empty name");
                }

                cx.mod_item_in(&item, &name, &parent)?;
                parent.push_str(&format!("{}{}", if parent.is_empty() { "" } else { "::" }, name));
                let module = match item.inner {
                    clean::StrippedItem(box clean::ModuleItem(m)) |
                    clean::ModuleItem(m) => m,
                    _ => unreachable!()
                };
                let mut counts = 0;
                for it in module.items {
                    if it.name.is_some() {
                        counts += 1;
                    }
                    // push front for items
                    work.push((cx.clone(), it, parent.clone()));
                }
                let mut parts = parent.split("::")
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>();
                if counts > 0 {
                    mods.insert(parent.clone(), 0);
                    while !parts.is_empty() {
                        if let Some(c) = mods.get_mut(&parts.join("::")) {
                            *c += counts;
                        }
                        parts.pop();
                    }
                } else {
                    remove_modules(parts, &mut mods, &mut cx)?;
                }
            } else if item.name.is_some() {
                cx.item(item, &parent)?;
                remove_modules(
                    parent.split("::").filter(|p| !p.is_empty()).collect::<Vec<_>>(),
                    &mut mods,
                    &mut cx,
                )?;
            }
        }

        renderer.after_krate(&krate)?;
        renderer.after_run(diag)
    }
}

fn remove_modules<T: FormatRenderer>(
    mut parts: Vec<&str>,
    mods: &mut FxHashMap<String, usize>,
    cx: &mut T,
) -> Result<(), Error> {
    let mut removals = 1;
    while !parts.is_empty() {
        let name = parts.join("::");
        if let Some(c) = mods.get_mut(&name) {
            *c -= removals;
        }
        if mods.get(&name) == Some(&0) {
            removals += 1; // for parents, one more child has been removed.
            cx.mod_item_out(&parts[parts.len() - 1], &parts.join("::"))?;
            mods.remove(&name);
        }
        parts.pop();
    }
    Ok(())
}

const ATTRIBUTE_WHITELIST: &'static [Symbol] = &[
    sym::export_name,
    sym::lang,
    sym::link_section,
    sym::must_use,
    sym::no_mangle,
    sym::repr,
    sym::non_exhaustive
];

pub fn render_attribute(attr: &ast::MetaItem) -> Option<String> {
    let path = attr.path.to_string();

    if attr.is_word() {
        Some(path)
    } else if let Some(v) = attr.value_str() {
        Some(format!("{} = {:?}", path, v.as_str()))
    } else if let Some(values) = attr.meta_item_list() {
        let display: Vec<_> = values.iter().filter_map(|attr| {
            attr.meta_item().and_then(|mi| render_attribute(mi))
        }).collect();

        if display.len() > 0 {
            Some(format!("{}({})", path, display.join(", ")))
        } else {
            None
        }
    } else {
        None
    }
}

pub fn get_attributes(it: &clean::Item) -> Vec<String> {
    let mut attrs = Vec::new();

    for attr in &it.attrs.other_attrs {
        if !ATTRIBUTE_WHITELIST.contains(&attr.name_or_empty()) {
            continue;
        }
        if let Some(s) = render_attribute(&attr.meta().unwrap()) {
            attrs.push(s);
        }
    }
    attrs
}
