#![crate_name = "foo"]

pub struct Struct<A, B, C>(A, B, C);

pub trait Trait<Other> {}

impl<B, A, C> Trait<Struct<B, A, C>> for Struct<B, A, C>
where
    B: Trait<B>,
    A: Trait<A>,
    C: Trait<C>,
{
}
