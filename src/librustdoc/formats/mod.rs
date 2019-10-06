use crate::clean;
use crate::config::{RenderInfo, RenderOptions};
use crate::error::Error;

use std::cell::RefCell;
use std::rc::Rc;

use syntax::edition::Edition;

pub trait FormatRenderer: Clone {
    type Output: FormatRenderer;

    fn init(
        krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
        parent: Rc<RefCell<Renderer>>,
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
    ) -> Result<(), Error>;

    fn mod_item_in(
        &mut self,
        item: &clean::Item,
        item_name: &str,
        module: &clean::Module,
    ) -> Result<(), Error>;
    fn mod_item_out(&mut self) -> Result<(), Error>;
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
        let rself = Rc::new(RefCell::new(self));
        let (mut renderer, mut krate) = T::init(
            krate, options, renderinfo, diag, edition, rself.clone(),
        )?;
        let mut item = match krate.module.take() {
            Some(i) => i,
            None => return Ok(()),
        };

        item.name = Some(krate.name.clone());

        // Render the crate documentation
        let mut work = vec![(renderer.clone(), item)];

        renderer.before_krate(&krate)?;

        while let Some((mut cx, item)) = work.pop() {
            if item.is_mod() {
                // modules are special because they add a namespace. We also need to
                // recurse into the items of the module as well.
                let name = item.name.as_ref().unwrap().to_string();
                if name.is_empty() {
                    panic!("Unexpected module with empty name");
                }

                let module = match item.inner {
                    clean::StrippedItem(box clean::ModuleItem(ref m)) |
                    clean::ModuleItem(ref m) => m,
                    _ => unreachable!()
                };
                cx.mod_item_in(&item, &name, module)?;
                let module = match item.inner {
                    clean::StrippedItem(box clean::ModuleItem(m)) |
                    clean::ModuleItem(m) => m,
                    _ => unreachable!()
                };
                for it in module.items {
                    work.push((renderer.clone(), it));
                }

                cx.mod_item_out()?;
            } else if item.name.is_some() {
                cx.item(item)?;
            }
        }

        renderer.after_krate(&krate)
    }
}
