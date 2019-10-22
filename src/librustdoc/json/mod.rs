use std::cell::RefCell;
use std::io::Write;
use std::fs::{create_dir_all, File, OpenOptions};
use std::path::PathBuf;
use std::rc::Rc;

use crate::clean::{self, GetDefId};
use crate::config::{RenderInfo, RenderOptions};
use crate::docfs::PathError;
use crate::doctree;
use crate::error::Error;
use crate::formats::{AssocItemRender, RenderMode, FormatRenderer, Impl, get_attributes};
use crate::formats::cache::Cache;
use crate::formats::item_type::ItemType;
// TODO: move this type somewhere else

use rustc::util::nodemap::FxHashMap;
use rustc::hir::def_id::DefId;
use serialize::json::{ToJson/*, Json, as_json*/};
//use syntax::symbol::{Symbol, sym};
//use syntax::ast;
use syntax::edition::Edition;

#[derive(Clone)]
pub struct Context {
    pub current: Vec<String>,
    file: Rc<RefCell<File>>,
    mods: Rc<RefCell<FxHashMap<String, usize>>>,
    path: Rc<PathBuf>,
}

impl FormatRenderer for Context {
    type Output = Self;

    fn init(
        krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
        _ : &mut Cache,
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

    fn after_krate(&mut self, krate: &clean::Crate, _: &Cache) -> Result<(), Error> {
        Ok(())
    }

    fn mod_item_in(
        &mut self,
        item: &clean::Item,
        item_name: &str,
        full_parent_item_path: &str,
        _: &Cache,
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

    fn item(
        &mut self,
        item: clean::Item,
        full_item_path: &str,
        cache: &Cache,
    ) -> Result<(), Error> {
        println!("item");
        let nb: usize = *self.mods.borrow().get(full_item_path).unwrap_or_else(|| &0);
        self.render_item(item, nb, cache)?;
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

    fn render_item(
        &mut self,
        item: clean::Item,
        nb_items: usize,
        cache: &Cache,
    ) -> Result<(), Error> {
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
            clean::StructItem(ref s) => self.render_struct(&mut buf, &item, s, cache),
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

    fn render_struct(&self, w: &mut String, it: &clean::Item, s: &clean::Struct, cache: &Cache) {
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

    fn render_assoc_items(
        &self,
        w: &mut String,
        item: &clean::Item,
        it: DefId,
        what: AssocItemRender<'_>,
        cache: &Cache,
    ) {
        let v = match cache.impls.get(&it) {
            Some(v) => v,
            None => return,
        };
        let (non_trait, traits): (Vec<_>, _) = v.iter().partition(|i| {
            i.inner_impl().trait_.is_none()
        });
        if !non_trait.is_empty() {
            let render_mode = match what {
                AssocItemRender::All => RenderMode::Normal,
                AssocItemRender::DerefFor { trait_, type_, deref_mut_ } => {
                    RenderMode::ForDeref { mut_: deref_mut_ }
                }
            };
            for i in &non_trait {
                self.render_impl(w, i, render_mode,
                                 item.stable_since(), true, None, false, true, cache);
            }
        }
        if let AssocItemRender::DerefFor { .. } = what {
            return;
        }
        if !traits.is_empty() {
            let deref_impl = traits.iter().find(|t| {
                t.inner_impl().trait_.def_id() == cache.deref_trait_did
            });
            if let Some(impl_) = deref_impl {
                let has_deref_mut = traits.iter().find(|t| {
                    t.inner_impl().trait_.def_id() == cache.deref_mut_trait_did
                }).is_some();
                render_deref_methods(w, impl_, containing_item, has_deref_mut, cache);
            }

            let (synthetic, concrete): (Vec<&&Impl>, Vec<&&Impl>) = traits
                .iter()
                .partition(|t| t.inner_impl().synthetic);
            let (blanket_impl, concrete): (Vec<&&Impl>, _) = concrete
                .into_iter()
                .partition(|t| t.inner_impl().blanket_impl.is_some());

            let mut impls = Buffer::empty_from(&w);
            render_impls(&mut impls, &concrete, containing_item, cache);
            let impls = impls.into_inner();
            if !impls.is_empty() {
                w.push_str(&format!("\
                    <h2 id='implementations' class='small-section-header'>\
                      Trait Implementations<a href='#implementations' class='anchor'></a>\
                    </h2>\
                    <div id='implementations-list'>{}</div>", impls));
            }

            if !synthetic.is_empty() {
                w.push_str("\
                    <h2 id='synthetic-implementations' class='small-section-header'>\
                      Auto Trait Implementations\
                      <a href='#synthetic-implementations' class='anchor'></a>\
                    </h2>\
                    <div id='synthetic-implementations-list'>\
                ");
                render_impls(w, &synthetic, containing_item, cache);
                w.push_str("</div>");
            }

            if !blanket_impl.is_empty() {
                w.push_str("\
                    <h2 id='blanket-implementations' class='small-section-header'>\
                      Blanket Implementations\
                      <a href='#blanket-implementations' class='anchor'></a>\
                    </h2>\
                    <div id='blanket-implementations-list'>\
                ");
                render_impls(w, &blanket_impl, containing_item, cache);
                w.push_str("</div>");
            }
        }
    }

    fn write(&mut self, s: &str) -> Result<(), Error> {
        write!(*self.file.borrow_mut(), "{}", s).map_err(|e| Error::new(e, &*self.path))
    }
}

fn render_impls(
    w: &mut String,
    traits: &[&&Impl],
    containing_item: &clean::Item,
    cache: &Cache,
) {
    for i in traits {
        let did = i.trait_did().unwrap();
        render_impl(w, i,
                    RenderMode::Normal, containing_item.stable_since(), true, None, false, true,
                    cache);
    }
}

fn render_impl(
    w: &mut String,
    i: &Impl,
    render_mode: RenderMode,
    outer_version: Option<&str>,
    show_def_docs: bool,
    use_absolute: Option<bool>,
    is_on_foreign_type: bool,
    show_default_items: bool,
    cache: &Cache,
) {
    if render_mode == RenderMode::Normal {
        if let Some(use_absolute) = use_absolute {
            fmt_impl_for_trait_page(&i.inner_impl(), w, use_absolute);
            if show_def_docs {
                for it in &i.inner_impl().items {
                    if let clean::TypedefItem(ref tydef, _) = it.inner {
                        assoc_type(w, it, &vec![], Some(&tydef.type_), "");
                    }
                }
            }
        }
        let since = i.impl_item.stability.as_ref().map(|s| &s.since[..]);
        render_stability_since_raw(w, since, outer_version);
        if let Some(ref dox) = cx.shared.maybe_collapsed_doc_value(&i.impl_item) {
            let mut ids = cx.id_map.borrow_mut();
            w.push_str(&format!("<div class='docblock'>{}</div>",
                                Markdown(&*dox, &i.impl_item.links(), &mut ids,
                                         cx.shared.codes.shared.edition,
                                         &cx.shared.playground).to_string()));
        }
    }

    fn doc_impl_item(w: &mut String, item: &clean::Item,
                     render_mode: RenderMode,
                     is_default_item: bool, outer_version: Option<&str>,
                     trait_: Option<&clean::Trait>, show_def_docs: bool,
                     cache: &Cache) {
        let item_type = item.type_();
        let name = item.name.as_ref().unwrap();

        let render_method_item = match render_mode {
            RenderMode::Normal => true,
            RenderMode::ForDeref { mut_: deref_mut_ } => should_render_item(&item, deref_mut_),
        };

        let (is_hidden, extra_class) = if (trait_.is_none() ||
                                           item.doc_value().is_some() ||
                                           item.inner.is_associated()) &&
                                          !is_default_item {
            (false, "")
        } else {
            (true, " hidden")
        };
        match item.inner {
            clean::MethodItem(clean::Method { ref decl, .. }) |
            clean::TyMethodItem(clean::TyMethod { ref decl, .. }) => {
                // Only render when the method is not static or we allow static methods
                if render_method_item {
                    w.push_str(&format!("{}", spotlight_decl(decl)));
                    render_assoc_item(w, item, ItemType::Impl);
                    render_stability_since_raw(w, item.stable_since(), outer_version);
                }
            }
            clean::TypedefItem(ref tydef, _) => {
                assoc_type(w, item, &Vec::new(), Some(&tydef.type_), "");
            }
            clean::AssocConstItem(ref ty, ref default) => {
                assoc_const(w, item, ty, default.as_ref(), "");
                render_stability_since_raw(w, item.stable_since(), outer_version);
            }
            clean::AssocTypeItem(ref bounds, ref default) => {
                assoc_type(w, item, bounds, default.as_ref(), "");
            }
            clean::StrippedItem(..) => return,
            _ => panic!("can't make docs for trait item with name {:?}", item.name)
        }

        if render_method_item {
            if !is_default_item {
                if let Some(t) = trait_ {
                    // The trait item may have been stripped so we might not
                    // find any documentation or stability for it.
                    if let Some(it) = t.items.iter().find(|i| i.name == item.name) {
                        // We need the stability of the item from the trait
                        // because impls can't have a stability.
                        document_stability(w, it, is_hidden);
                        if item.doc_value().is_some() {
                            document_full(w, item, "", is_hidden);
                        } else if show_def_docs {
                            // In case the item isn't documented,
                            // provide short documentation from the trait.
                            document_short(w, it, "", is_hidden);
                        }
                    }
                } else {
                    document_stability(w, item, is_hidden);
                    if show_def_docs {
                        document_full(w, item, "", is_hidden);
                    }
                }
            } else {
                document_stability(w, item, is_hidden);
                if show_def_docs {
                    document_short(w, item, "", is_hidden);
                }
            }
        }
    }

    let traits = &cache.traits;
    let trait_ = i.trait_did().map(|did| &traits[&did]);

    w.push_str("<div class='impl-items'>");
    for trait_item in &i.inner_impl().items {
        doc_impl_item(w, trait_item, render_mode,
                      false, outer_version, trait_, show_def_docs, cache);
    }

    fn render_default_items(
        w: &mut String,
        t: &clean::Trait,
        i: &clean::Impl,
        render_mode: RenderMode,
        outer_version: Option<&str>,
        show_def_docs: bool,
        cache: &Cache,
    ) {
        for trait_item in &t.items {
            let n = trait_item.name.clone();
            if i.items.iter().find(|m| m.name == n).is_some() {
                continue;
            }
            let did = i.trait_.as_ref().unwrap().def_id().unwrap();

            doc_impl_item(w, trait_item, render_mode, true,
                          outer_version, None, show_def_docs, cache);
        }
    }

    // If we've implemented a trait, then also emit documentation for all
    // default items which weren't overridden in the implementation block.
    // We don't emit documentation for default items if they appear in the
    // Implementations on Foreign Types or Implementors sections.
    if show_default_items {
        if let Some(t) = trait_ {
            render_default_items(w, t, &i.inner_impl(),
                                render_mode, outer_version, show_def_docs, cache);
        }
    }
    w.push_str("</div>");
}

// Render md_text as markdown.
fn render_markdown(
    w: &mut String,
    md_text: &str,
    links: Vec<(String, String)>,
    prefix: &str,
    is_hidden: bool,
) {
    // let mut ids = cx.id_map.borrow_mut();
    // write!(w, "<div class='docblock{}'>{}{}</div>",
    //        if is_hidden { " hidden" } else { "" },
    //        prefix,
    //        Markdown(md_text, &links, &mut ids,
    //        cx.shared.codes, cx.shared.edition, &cx.shared.playground).to_string())
}

fn document_short(
    w: &mut String,
    item: &clean::Item,
    prefix: &str,
    is_hidden: bool,
) {
    if let Some(s) = item.doc_value() {
        render_markdown(w, s, item.links(), prefix, is_hidden);
    } else if !prefix.is_empty() {
        w.push_str(&format!("<div class='docblock{}'>{}</div>",
                            if is_hidden { " hidden" } else { "" },
                            prefix);
    }
}

fn document_full(w: &mut String, item: &clean::Item, prefix: &str, is_hidden: bool) {
    if !prefix.is_empty() {
        w.push_str(&format!("<div class='docblock{}'>{}</div>",
                            if is_hidden { " hidden" } else { "" },
                            prefix);
    }
}

fn document_stability(w: &mut String, item: &clean::Item, is_hidden: bool) {
    let stabilities = short_stability(item);
    if !stabilities.is_empty() {
        write!(w, "<div class='stability{}'>", if is_hidden { " hidden" } else { "" });
        for stability in stabilities {
            write!(w, "{}", stability);
        }
        write!(w, "</div>");
    }
}

struct UnstabilityInfo {
    issue_number: Option<u32>,
    reason: Option<String>,
    feature: Option<String>
}

struct StabilityInfo {
    state: StabilityState,
    cfg: Option<Arc<Cfg>>,
}

enum StabilityState {
    /// Contains the associated note if any.
    Deprecated(Option<String>),
    /// Contains the version number and the associated note if any.
    DeprecatedSince(String, Option<String>),
    /// Contains the version number and the associated note if any.
    DeprecatingIn(String, Option<String>),
    Internal(UnstabilityInfo),
    NightlyOnly(UnstabilityInfo),
}

/// Render the stability and/or deprecation warning that is displayed at the top of the item's
/// documentation.
fn short_stability(item: &clean::Item) -> Vec<StabilityInfo> {
    let mut stability = vec![];

    if let Some(Deprecation { note, since }) = &item.deprecation() {
        if let Some(ref stab) = item.stability {
            if let Some(ref depr) = stab.deprecation {
                if let Some(ref since) = depr.since {
                    if !stability::deprecation_in_effect(&since) {
                        stability.push(StabilityInfo {
                            state: StabilityState::DeprecatingIn(since.clone(), note.clone()),
                            cfg: item.attrs.cfg.clone(),
                        });
                    }
                }
            }
        }
        if stability.is_empty() {
            stability.push(StabilityInfo {
                state: if let Some(since) = since {
                           StabilityState::DeprecatedSince(since.clone(), note.clone())
                       } else {
                           StabilityState::Deprecated(note.clone())
                       },
                cfg: item.attrs.cfg.clone(),
            });
        }
    }

    if let Some(stab) = item
        .stability
        .as_ref()
        .filter(|stab| stab.level == stability::Unstable)
    {
        let is_rustc_private = stab.feature.as_ref().map(|s| &**s) == Some("rustc_private");
        let reason = if let Some(unstable_reason) = &stab.unstable_reason {
            // Provide a more informative message than the compiler help.
            Some(if is_rustc_private {
                "This crate is being loaded from the sysroot, a permanently unstable location \
                for private compiler dependencies. It is not intended for general use. Prefer \
                using a public version of this crate from \
                [crates.io](https://crates.io) via [`Cargo.toml`]\
                (https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)."
            } else {
                unstable_reason
            }.to_owned())
        } else {
            None
        };

        let unstability_info = UnstabilityInfo {
            issue_number: stab.issue,
            reason,
            feature: stab.feature.clone(),
        };

        stability.push(if is_rustc_private {
            StabilityInfo {
                state: StabilityState::Internal(unstability_info),
                cfg: item.attrs.cfg.clone(),
            }
        } else {
            StabilityInfo {
                state: StabilityState::NightlyOnly(unstability_info),
                cfg: item.attrs.cfg.clone(),
            }
        });
    }

    stability
}
