

impl clean::Struct {
    /// Returns `None` if called on a unit struct.
    ///
    /// Returns the fields as first (they can be `None` if stripped) and the
    /// second parameter is `true` if there are hidden fields.
    pub fn get_fields(&self) -> Option<(Vec<Option<StructKind>>, bool)> {
        let (struct_kind, has_hidden_fields) = match self.struct_type {
            doctree::Plain => {
                let fields = item.fields.iter()
                    .filter_map(|field| {
                        if let clean::StructFieldItem(_) = field.inner {
                            Some(Some(field.inner.clone()))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                Some((StructKind::Normal(fields), item.has_stripped_fields().unwrap()))
            }
            doctree::Tuple => {
                let mut has_hidden_fields = false;
                let fields = fields.iter()
                    .map(|field| {
                        match field.inner {
                            clean::StrippedItem(box clean::StructFieldItem(ref ty)) => {
                                has_hidden_fields = true;
                                None
                            }
                            clean::StructFieldItem(ref ty) => {
                                Some(clean::StructFieldItem(ty.clone()))
                            }
                            _ => unreachable!()
                        }
                    })
                    .collect::<Vec<_>>();
                Some((StructKind::Tuple(fields), has_hidden_fields))
            }
            doctree::Unit => {
                None
            }
        }
    }
}

pub trait GetImpls {
    fn get_blanket_impls(&self) -> Vec<Item> {
        // code in clean/blanket_impl.rs
    }

    fn get_auto_impls(&self) -> Vec<Item> {
        ;
    }

    fn get_impls(&self) {
        ;
    }
}
