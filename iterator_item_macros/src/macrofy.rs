use proc_macro::{Group, Punct, Spacing, TokenStream, TokenTree};
use std::mem::replace;

struct Macrofy<I: Iterator<Item = TokenTree>> {
    inner: I,
    needs_bang: bool,
}

impl<I: Iterator<Item = TokenTree>> Iterator for Macrofy<I> {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        let needs_bang = replace(&mut self.needs_bang, false);
        Some(if needs_bang {
            TokenTree::Punct(Punct::new('!', Spacing::Alone))
        } else {
            match self.inner.next()? {
                TokenTree::Ident(i) if i.to_string() == "gen" => {
                    self.needs_bang = true;
                    TokenTree::Ident(i)
                }
                TokenTree::Group(g) => {
                    TokenTree::Group(Group::new(g.delimiter(), macrofy(g.stream())))
                }
                other => other,
            }
        })
    }
}

pub fn macrofy(input: TokenStream) -> TokenStream {
    TokenStream::from_iter(Macrofy {
        inner: input.into_iter(),
        needs_bang: false,
    })
}
