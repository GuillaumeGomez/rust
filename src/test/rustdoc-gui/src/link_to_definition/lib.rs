pub struct Bar {
    pub a: String,
    pub b: u32,
}

pub fn foo(_b: &Bar, _c: tadam::Foo) {}

pub mod tadam;
