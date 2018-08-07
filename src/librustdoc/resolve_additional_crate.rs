// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::hir::itemlikevisit::ItemLikeVisitor;
use rustc::hir::{Item, ImplItem, TraitItem};
use rustc_metadata::creader::CrateLoader;
use syntax::ast::Attribute;
use syntax::symbol::Symbol;
use syntax_pos::DUMMY_SP;

use html::markdown::markdown_links;

pub struct UnusedExternCrate<'a, 'b: 'a> {
    crate_loader: &'a mut CrateLoader<'b>,
}

impl<'a, 'b> UnusedExternCrate<'a, 'b> {
    pub fn new(crate_loader: &'a mut CrateLoader<'b>) -> UnusedExternCrate<'a, 'b> {
        UnusedExternCrate {
            crate_loader,
        }
    }

    fn check_for_links(&mut self, attrs: &[Attribute]) {
        for attr in attrs.iter() {
            if !attr.is_sugared_doc {
                continue
            }
            println!("====> {:?}", attr.value_str());
            if let Some(v) = attr.value_str() {
                for (ori_link, _) in markdown_links(&v.to_string()) {
                    // bail early for real links
                    if ori_link.contains('/') {
                        continue;
                    }
                    let link = ori_link.replace("`", "");
                    println!("before: {:?}", ori_link);
                    let link = link.split('@')
                                   .last()
                                   .unwrap_or_else(|| "")
                                   .split("::")
                                   .filter(|s| !s.trim().is_empty())
                                   .next()
                                   .unwrap_or_else(|| "");
                    println!("after: {:?}", link);

                    if link.is_empty() ||
                       link.contains(|ch: char| !(ch.is_alphanumeric() ||
                                                  ch == ':' || ch == '_')) {
                        continue;
                    }

                    self.crate_loader.process_path_extern_no_fail(Symbol::intern(&link), DUMMY_SP);
                }
            }
        }
    }
}

impl<'hir, 'a, 'b> ItemLikeVisitor<'hir> for UnusedExternCrate<'a, 'b> {
    fn visit_item(&mut self, item: &'hir Item) {
        self.check_for_links(&item.attrs);
    }

    fn visit_trait_item(&mut self, trait_item: &'hir TraitItem) {
        self.check_for_links(&trait_item.attrs);
    }

    fn visit_impl_item(&mut self, impl_item: &'hir ImplItem) {
        self.check_for_links(&impl_item.attrs);
    }
}
