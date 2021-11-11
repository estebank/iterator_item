use proc_macro::{Group, Ident, Punct, Spacing, TokenStream, TokenTree};
use std::{collections::VecDeque, mem::replace};

enum MacrofyState {
    Passthrough,
    SawAsync(Ident),
}

fn bang() -> TokenTree {
    TokenTree::Punct(Punct::new('!', Spacing::Alone))
}

impl MacrofyState {
    fn step(&mut self, tree: TokenTree, out: &mut VecDeque<TokenTree>) {
        use MacrofyState::*;

        *self = match (replace(self, Passthrough), tree) {
            (Passthrough, TokenTree::Group(g)) => {
                out.push_back(TokenTree::Group(Group::new(
                    g.delimiter(),
                    macrofy(g.stream()),
                )));
                Passthrough
            }
            (Passthrough, TokenTree::Ident(i)) if i.to_string() == "gen" => {
                out.push_back(TokenTree::Ident(i));
                out.push_back(bang());
                Passthrough
            }
            (SawAsync(_), TokenTree::Ident(i)) if i.to_string() == "gen" => {
                out.push_back(TokenTree::Ident(Ident::new("async_gen", i.span())));
                out.push_back(bang());
                Passthrough
            }
            (SawAsync(a), tok) => {
                out.push_back(TokenTree::Ident(a));
                out.push_back(tok);
                Passthrough
            }
            (Passthrough, TokenTree::Ident(i)) if i.to_string() == "async" => SawAsync(i),
            (_, tok) => {
                out.push_back(tok);
                Passthrough
            }
        };
    }
}

struct Macrofy<I: Iterator<Item = TokenTree>> {
    inner: I,
    queue: VecDeque<TokenTree>,
    state: MacrofyState,
}

impl<I: Iterator<Item = TokenTree>> Iterator for Macrofy<I> {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(tok) = self.queue.pop_front() {
                return Some(tok);
            }
            self.state.step(self.inner.next()?, &mut self.queue);
        }
    }
}

pub fn macrofy(input: TokenStream) -> TokenStream {
    TokenStream::from_iter(Macrofy {
        inner: input.into_iter(),
        queue: VecDeque::with_capacity(2),
        state: MacrofyState::Passthrough,
    })
}
