mod foo {
    pub trait Foo {
        fn foo();
    }

    pub struct Bar;

    pub use Foo as _;
    pub use Bar as _;
}

pub use foo::*;
