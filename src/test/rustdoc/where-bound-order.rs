// aux-build:where-bound-order.rs
// build-aux-docs

extern crate foo;

pub use foo::*;

// @has 'foo/trait.Trait.html'
// @has - '//*[@id="implementors-list"]//span[@class="where fmt-newline"]/text()[1]' 'B:'
