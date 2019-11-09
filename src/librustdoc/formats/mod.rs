use crate::clean;
use crate::clean::GetDefId;
use rustc::hir::def_id::DefId;

pub use renderer::{
    FormatRenderer,
    Renderer,
    remove_modules,
    ATTRIBUTE_WHITELIST,
    render_attribute,
    get_attributes,
};
pub mod cache;
pub mod item_type;
pub mod renderer;
pub mod stability;
pub mod types;

pub enum AssocItemRender<'a> {
    All,
    DerefFor { trait_: &'a clean::Type, type_: &'a clean::Type, deref_mut_: bool }
}

#[derive(Copy, Clone, PartialEq)]
pub enum RenderMode {
    Normal,
    ForDeref { mut_: bool },
}

impl RenderMode {
    pub fn is_normal(&self) -> bool {
        match *self {
            RenderMode::Normal => true,
            _ => false,
        }
    }
}

/// Metadata about implementations for a type or trait.
#[derive(Clone, Debug)]
pub struct Impl {
    pub impl_item: clean::Item,
}

impl Impl {
    pub fn inner_impl(&self) -> &clean::Impl {
        match self.impl_item.inner {
            clean::ImplItem(ref impl_) => impl_,
            _ => panic!("non-impl item found in impl")
        }
    }

    pub fn trait_did(&self) -> Option<DefId> {
        self.inner_impl().trait_.def_id()
    }
}
