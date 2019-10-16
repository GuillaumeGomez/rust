use crate::clean::{self, /*GetDefId,*/ AttributesExt};
//use crate::fold::DocFolder;
//use rustc::hir::def_id::{CrateNum, CRATE_DEF_INDEX, DefId};
//use rustc::middle::privacy::AccessLevels;
use rustc_data_structures::fx::{FxHashMap/*, FxHashSet*/};
//use std::mem;
use std::path::{Path/*, PathBuf*/};
use std::collections::BTreeMap;
//use syntax::source_map::FileName;
use syntax::symbol::sym;
use serialize::json::{ToJson, Json, as_json};

use crate::formats::cache::Cache;
use crate::html::render::{/*ItemType,*/IndexItem, IndexItemFunctionType, shorten, plain_summary_line};
use crate::html::render::{/*Impl,*/Type};
//use crate::config::RenderInfo;

/// Indicates where an external crate can be found.
pub enum ExternalLocation {
    /// Remote URL root of the external crate
    Remote(String),
    /// This external crate can be found in the local doc/ folder
    Local,
    /// The external crate could not be found.
    Unknown,
}

/// Attempts to find where an external crate is located, given that we're
/// rendering in to the specified source destination.
pub fn extern_location(e: &clean::ExternalCrate, extern_url: Option<&str>, dst: &Path)
    -> ExternalLocation
{
    use ExternalLocation::*;
    // See if there's documentation generated into the local directory
    let local_location = dst.join(&e.name);
    if local_location.is_dir() {
        return Local;
    }

    if let Some(url) = extern_url {
        let mut url = url.to_string();
        if !url.ends_with("/") {
            url.push('/');
        }
        return Remote(url);
    }

    // Failing that, see if there's an attribute specifying where to find this
    // external crate
    e.attrs.lists(sym::doc)
     .filter(|a| a.check_name(sym::html_root_url))
     .filter_map(|a| a.value_str())
     .map(|url| {
        let mut url = url.to_string();
        if !url.ends_with("/") {
            url.push('/')
        }
        Remote(url)
    }).next().unwrap_or(Unknown) // Well, at least we tried.
}

/// Builds the search index from the collected metadata
pub fn build_index(krate: &clean::Crate, cache: &mut Cache) -> String {
    let mut nodeid_to_pathid = FxHashMap::default();
    let mut crate_items = Vec::with_capacity(cache.search_index.len());
    let mut crate_paths = Vec::<Json>::new();

    let Cache { ref mut search_index,
                ref orphan_impl_items,
                ref paths, .. } = *cache;

    // Attach all orphan items to the type's definition if the type
    // has since been learned.
    for &(did, ref item) in orphan_impl_items {
        if let Some(&(ref fqp, _)) = paths.get(&did) {
            search_index.push(IndexItem {
                ty: item.type_(),
                name: item.name.clone().unwrap(),
                path: fqp[..fqp.len() - 1].join("::"),
                desc: shorten(plain_summary_line(item.doc_value())),
                parent: Some(did),
                parent_idx: None,
                search_type: get_index_search_type(&item),
            });
        }
    }

    // Reduce `NodeId` in paths into smaller sequential numbers,
    // and prune the paths that do not appear in the index.
    let mut lastpath = String::new();
    let mut lastpathid = 0usize;

    for item in search_index {
        item.parent_idx = item.parent.map(|nodeid| {
            if nodeid_to_pathid.contains_key(&nodeid) {
                *nodeid_to_pathid.get(&nodeid).unwrap()
            } else {
                let pathid = lastpathid;
                nodeid_to_pathid.insert(nodeid, pathid);
                lastpathid += 1;

                let &(ref fqp, short) = paths.get(&nodeid).unwrap();
                crate_paths.push(((short as usize), fqp.last().unwrap().clone()).to_json());
                pathid
            }
        });

        // Omit the parent path if it is same to that of the prior item.
        if lastpath == item.path {
            item.path.clear();
        } else {
            lastpath = item.path.clone();
        }
        crate_items.push(item.to_json());
    }

    let crate_doc = krate.module.as_ref().map(|module| {
        shorten(plain_summary_line(module.doc_value()))
    }).unwrap_or(String::new());

    let mut crate_data = BTreeMap::new();
    crate_data.insert("doc".to_owned(), Json::String(crate_doc));
    crate_data.insert("i".to_owned(), Json::Array(crate_items));
    crate_data.insert("p".to_owned(), Json::Array(crate_paths));

    // Collect the index into a string
    format!("searchIndex[{}] = {};",
            as_json(&krate.name),
            Json::Object(crate_data))
}

fn get_index_search_type(item: &clean::Item) -> Option<IndexItemFunctionType> {
    let (all_types, ret_types) = match item.inner {
        clean::FunctionItem(ref f) => (&f.all_types, &f.ret_types),
        clean::MethodItem(ref m) => (&m.all_types, &m.ret_types),
        clean::TyMethodItem(ref m) => (&m.all_types, &m.ret_types),
        _ => return None,
    };

    let inputs = all_types.iter().map(|arg| {
        get_index_type(&arg)
    }).filter(|a| a.name.is_some()).collect();
    let output = ret_types.iter().map(|arg| {
        get_index_type(&arg)
    }).filter(|a| a.name.is_some()).collect::<Vec<_>>();
    let output = if output.is_empty() {
        None
    } else {
        Some(output)
    };

    Some(IndexItemFunctionType { inputs, output })
}

fn get_index_type(clean_type: &clean::Type) -> Type {
    let t = Type {
        name: get_index_type_name(clean_type, true).map(|s| s.to_ascii_lowercase()),
        generics: get_generics(clean_type),
    };
    t
}

fn get_index_type_name(clean_type: &clean::Type, accept_generic: bool) -> Option<String> {
    match *clean_type {
        clean::ResolvedPath { ref path, .. } => {
            let segments = &path.segments;
            let path_segment = segments.into_iter().last().unwrap_or_else(|| panic!(
                "get_index_type_name(clean_type: {:?}, accept_generic: {:?}) had length zero path",
                clean_type, accept_generic
            ));
            Some(path_segment.name.clone())
        }
        clean::Generic(ref s) if accept_generic => Some(s.clone()),
        clean::Primitive(ref p) => Some(format!("{:?}", p)),
        clean::BorrowedRef { ref type_, .. } => get_index_type_name(type_, accept_generic),
        // FIXME: add all from clean::Type.
        _ => None
    }
}

fn get_generics(clean_type: &clean::Type) -> Option<Vec<String>> {
    clean_type.generics()
              .and_then(|types| {
                  let r = types.iter()
                               .filter_map(|t| get_index_type_name(t, false))
                               .map(|s| s.to_ascii_lowercase())
                               .collect::<Vec<_>>();
                  if r.is_empty() {
                      None
                  } else {
                      Some(r)
                  }
              })
}
