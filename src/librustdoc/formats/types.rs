use crate::clean;
use crate::doctree;

use std::fmt;

pub enum StructKind<'a> {
    Normal {
        fields: Vec<(&'a clean::Item, &'a clean::Type)>,
        has_hidden_fields: bool,
    },
    Tuple(Vec<(Option<&'a clean::Item>, &'a clean::Type)>),
    Unit,
}

impl clean::Struct {
    pub fn get_fields<'a>(&'a self) -> StructKind<'a> {
        let (struct_kind, has_hidden_fields) = match self.struct_type {
            doctree::Plain => {
                let fields = self.fields.iter()
                    .filter_map(|field| {
                        if let clean::StructFieldItem(ref ty) = field.inner {
                            Some((field, ty))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                StructKind::Normal {
                    fields,
                    has_hidden_fields: self.has_stripped_fields().unwrap(),
                }
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
                                Some((field, ty))
                            }
                            _ => unreachable!()
                        }
                    })
                    .collect::<Vec<_>>();
                StructKind::Tuple {
                    fields,
                    has_hidden_fields,
                }
            }
            doctree::Unit => StructKind::Unit,
        }
    }
}

impl clean::Union {
    pub fn get_fields<'a>(&'a self) -> StructKind<'a> {
        let fields = self.fields.iter()
            .filter_map(|field| {
                if let clean::StructFieldItem(ref ty) = field.inner {
                    Some((field, ty))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        StructKind::Normal {
            fields,
            has_hidden_fields: self.has_stripped_fields().unwrap(),
        }
    }
}

impl fmt::Display for doctree::StructType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match *self {
            doctree::StructType::Plain => "plain",
            doctree::StructType::Tuple => "tuple",
            doctree::StructType::Unit => "unit",
        })
    }
}
