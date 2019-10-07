pub struct Context {
    pub current: Vec<String>,
    pub dst: Rc<PathBuf>,
    pub elems: Rc<Elem>,
}

struct Elem {
    name: String,
    doc: String,
    type_: String,
    // variants on enums, arguments on functions, fields on structs
    fields: Vec<Elem>,
    functions: Vec<Elem>,
    trait_impls: Vec<Elem>,
    blanket_impls: Vec<Elem>,
    auto_impls: Vec<Elem>,
}

impl FormatRenderer for Context {
    type Output = Self;

    fn init(
        mut krate: clean::Crate,
        options: RenderOptions,
        renderinfo: RenderInfo,
        diag: &errors::Handler,
        edition: Edition,
        parent: Rc<RefCell<Renderer>>,
    ) -> Result<(Context, clean::Crate), Error> {
        let RenderOptions {
            output,
            ..
        } = options;

        let mut cx = Context {
            current: Vec::new(),
            dst,
            
        };

        // Freeze the cache now that the index has been built. Put an Arc into TLS
        // for future parallelization opportunities
        CACHE_KEY.with(|v| *v.borrow_mut() = cache.clone());
        CURRENT_DEPTH.with(|s| s.set(0));

        // Write shared runs within a flock; disable thread dispatching of IO temporarily.
        Arc::get_mut(&mut cx.shared).unwrap().fs.set_sync_only(true);
        write_shared(&cx, &krate, index, &md_opts, diag)?;
        Arc::get_mut(&mut cx.shared).unwrap().fs.set_sync_only(false);
        Ok((cx, krate))
    }

    fn after_run(&mut self, diag: &errors::Handler) -> Result<(), Error> {
        let nb_errors = Arc::get_mut(&mut self.errors)
            .map_or_else(|| 0, |errors| errors.write_errors(diag));
        if nb_errors > 0 {
            Err(Error::new(io::Error::new(io::ErrorKind::Other, "I/O error"), ""))
        } else {
            Ok(())
        }
    }

    /// Main method for rendering a crate.
    ///
    /// This currently isn't parallelized, but it'd be pretty easy to add
    /// parallelization to this function.
    fn before_krate(&mut self, _krate: &clean::Crate) -> Result<(), Error> {
        /*let mut item = match krate.module.take() {
            Some(i) => i,
            None => return Ok(()),
        };

        item.name = Some(krate.name);*/
        Ok(())
    }

    fn after_krate(&mut self, krate: &clean::Crate) -> Result<(), Error> {
        let final_file = self.dst.join(&krate.name)
                                 .join("all.html");
        let settings_file = self.dst.join("settings.html");
        let crate_name = krate.name.clone();

        let mut root_path = self.dst.to_str().expect("invalid path").to_owned();
        if !root_path.ends_with('/') {
            root_path.push('/');
        }
        let mut page = layout::Page {
            title: "List of all items in this crate",
            css_class: "mod",
            root_path: "../",
            static_root_path: self.shared.static_root_path.as_deref(),
            description: "List of all items in this crate",
            keywords: BASIC_KEYWORDS,
            resource_suffix: &self.shared.resource_suffix,
            extra_scripts: &[],
            static_extra_scripts: &[],
        };
        let sidebar = if let Some(ref version) = self.cache.crate_version {
            format!("<p class='location'>Crate {}</p>\
                     <div class='block version'>\
                         <p>Version {}</p>\
                     </div>\
                     <a id='all-types' href='index.html'><p>Back to index</p></a>",
                    crate_name, version)
        } else {
            String::new()
        };
        let all = self.all.replace(AllTypes::new());
        let v = layout::render(&self.shared.layout,
                       &page, sidebar, |buf: &mut Buffer| all.print(buf),
                       &self.shared.themes);
        self.shared.fs.write(&final_file, v.as_bytes())?;

        // Generating settings page.
        page.title = "Rustdoc settings";
        page.description = "Settings of Rustdoc";
        page.root_path = "./";

        let mut themes = self.shared.themes.clone();
        let sidebar = "<p class='location'>Settings</p><div class='sidebar-elems'></div>";
        themes.push(PathBuf::from("settings.css"));
        let v = layout::render(
            &self.shared.layout,
            &page, sidebar, settings(
                self.shared.static_root_path.as_deref().unwrap_or("./"),
                &self.shared.resource_suffix
            ),
            &themes);
        self.shared.fs.write(&settings_file, v.as_bytes())?;

        Ok(())
    }

    fn mod_item_in(
        &mut self,
        item: &clean::Item,
        item_name: &str,
        module: &clean::Module,
    ) -> Result<(), Error> {
        // Stripped modules survive the rustdoc passes (i.e., `strip-private`)
        // if they contain impls for public types. These modules can also
        // contain items such as publicly re-exported structures.
        //
        // External crates will provide links to these structures, so
        // these modules are recursed into, but not rendered normally
        // (a flag on the context).
        if !self.render_redirect_pages {
            self.render_redirect_pages = item.is_stripped();
        }
        let scx = &self.shared;
        //let prev = self.dst.clone();
        self.dst.push(item_name);

        info!("Recursing into {}", self.dst.display());

        let buf = self.render_item(item, false);
        // buf will be empty if the module is stripped and there is no redirect for it
        if !buf.is_empty() {
            self.shared.ensure_dir(&self.dst)?;
            let joint_dst = self.dst.join("index.html");
            scx.fs.write(&joint_dst, buf.as_bytes())?;
        }

        ;

        // Render sidebar-items.js used throughout this module.
        if !self.render_redirect_pages {
            let items = self.build_sidebar_items(module);
            let js_dst = self.dst.join("sidebar-items.js");
            let v = format!("initSidebarItems({});", as_json(&items));
            scx.fs.write(&js_dst, &v)?;
        }

        info!("Recursed; leaving {}", self.dst.display());
        Ok(())
    }

    fn mod_item_out(&mut self) -> Result<(), Error> {
        // Go back to where we were at
        self.dst.pop();
        self.current.pop();
        Ok(())
    }

    /// Non-parallelized version of rendering an item. This will take the input
    /// item, render its contents, and then invoke the specified closure with
    /// all sub-items which need to be rendered.
    ///
    /// The rendering driver uses this closure to queue up more work.
    fn item(&mut self, item: clean::Item) -> Result<(), Error> {
        // Stripped modules survive the rustdoc passes (i.e., `strip-private`)
        // if they contain impls for public types. These modules can also
        // contain items such as publicly re-exported structures.
        //
        // External crates will provide links to these structures, so
        // these modules are recursed into, but not rendered normally
        // (a flag on the context).
        if !self.render_redirect_pages {
            self.render_redirect_pages = item.is_stripped();
        }

        let buf = self.render_item(&item, true);
        // buf will be empty if the item is stripped and there is no redirect for it
        if !buf.is_empty() {
            let name = item.name.as_ref().unwrap();
            let item_type = item.type_();
            let file_name = &item_path(item_type, name);
            self.shared.ensure_dir(&self.dst)?;
            let joint_dst = self.dst.join(file_name);
            self.shared.fs.write(&joint_dst, buf.as_bytes())?;

            if !self.render_redirect_pages {
                self.all.borrow_mut().append(full_path(self, &item), &item_type);
            }
            if self.shared.generate_redirect_pages {
                // Redirect from a sane URL using the namespace to Rustdoc's
                // URL for the page.
                let redir_name = format!("{}.{}.html", name, item_type.name_space());
                let redir_dst = self.dst.join(redir_name);
                let v = layout::redirect(file_name);
                self.shared.fs.write(&redir_dst, v.as_bytes())?;
            }
            // If the item is a macro, redirect from the old macro URL (with !)
            // to the new one (without).
            if item_type == ItemType::Macro {
                let redir_name = format!("{}.{}!.html", item_type, name);
                let redir_dst = self.dst.join(redir_name);
                let v = layout::redirect(file_name);
                self.shared.fs.write(&redir_dst, v.as_bytes())?;
            }
        }
        Ok(())
    }
}
