use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned,
    token::Brace,
    visit_mut::VisitMut,
    Block, Expr, Item, Result, Stmt, Token,
};

pub struct GenMacro {
    body: Block,
}

impl GenMacro {
    fn build(self) -> Expr {
        parse_quote! { todo!() }
    }
}

impl Parse for GenMacro {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(GenMacro {
            body: Block {
                brace_token: Brace { span: input.span() },
                stmts: Block::parse_within(input)?,
            },
        })
    }
}

pub struct GenMacroExpander;

impl VisitMut for GenMacroExpander {
    fn visit_stmt_mut(&mut self, i: &mut Stmt) {
        if let Stmt::Item(Item::Macro(m)) = i {
            if m.mac.path.is_ident("gen") {
                let gen = m.mac.parse_body::<GenMacro>();
                *i = match gen {
                    Ok(gen) => Stmt::Expr(gen.build()),
                    Err(e) => {
                        let e = e.into_compile_error();
                        parse_quote! { { #e } }
                    }
                };
            }
        } else {
            syn::visit_mut::visit_stmt_mut(self, i);
        }
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        eprintln!("EXPR: {}", quote! { #i });
        if let Expr::Macro(m) = i {
            if m.mac.path.is_ident("gen") {
                let gen = m.mac.parse_body::<GenMacro>();
                *i = match gen {
                    Ok(gen) => gen.build(),
                    Err(e) => {
                        let e = e.into_compile_error();
                        parse_quote! { { #e } }
                    }
                };
            }
        } else {
            syn::visit_mut::visit_expr_mut(self, i);
        }
    }
}

/// This `Visitor` allows us to modify the body (block) of the parsed item to make changes to it
/// before passing it back to `rustc`. This allows us to construct our own desugaring for `await`
/// and `yield`.
pub struct BodyVisitor {
    is_async: bool,
    is_try_yield: bool,
}

impl BodyVisitor {
    pub fn new(is_async: bool, is_try_yield: bool) -> Self {
        BodyVisitor {
            is_async,
            is_try_yield,
        }
    }
}

impl VisitMut for BodyVisitor {
    /// Desugar the iterator item's body into an underlying unstable `Generator`.
    ///
    /// This takes care of turning `async` iterators into a sync `Generator` body that is
    /// equivalent to the `rustc` desugared `async` code for `async`/`await`.
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        // We traverse all the child nodes first.
        syn::visit_mut::visit_expr_mut(self, i);
        match i {
            // FIXME: consider implementing `for await i in foo {}` syntax here by handling
            // `syn::Expr::ForLoop`.
            // FIXME: attempt to calculate `size_hint` proactively in loops by calling `size_hint`
            // in the expression being iterated *before* building the generator. This can only work
            // in very specific circumstances, so we need to be very clear that we are in one of
            // the valid cases. If we do this, we need to also increment a counter for every
            // `yield` statement outside of loops.
            syn::Expr::Return(syn::ExprReturn { expr, .. }) => {
                // To avoid further type errors down the line, explicitly handle this case and
                // remove it from the resulting item body.
                if let Some(expr) = expr {
                    expr.span()
                        .unwrap()
                        .error("iterator items can't return a non-`()` value")
                        .help("returning in an iterator is only meant for stopping the iterator")
                        .emit();
                }
                *expr = None;
            }
            syn::Expr::Yield(syn::ExprYield {
                expr: Some(expr), ..
            }) if self.is_async => {
                // Turn `yield #expr` in an `async` iterator item into `yield Poll::Ready(#expr)`
                *i = parse_quote!(iterator_item::async_gen_yield!(#expr));
            }
            syn::Expr::Yield(syn::ExprYield { expr: None, .. }) if self.is_async => {
                // Turn `yield;` in an `async` iterator item into `yield Poll::Ready(())`
                *i = parse_quote!(iterator_item::async_gen_yield!(()));
            }
            syn::Expr::Await(syn::ExprAwait { base: expr, .. }) if self.is_async => {
                // Turn `#expr.await` in an `async` iterator item into a `poll(#expr, cxt)` call
                // (with more details, look at the macro for more)
                *i = parse_quote!(iterator_item::async_gen_await!(#expr, __stream_ctx));
            }
            syn::Expr::Try(syn::ExprTry { expr, .. }) => {
                *i = match (self.is_async, self.is_try_yield) {
                    // Turn `#expr?` into one last `yield #expr`
                    (true, true) => parse_quote!(iterator_item::async_gen_try!(#expr)),
                    (false, true) => parse_quote!(iterator_item::gen_try!(#expr)),
                    // Turn `#expr?` into an early return. This would operate better in `rustc`
                    // with trait selection because then we can check whether the yielded value is
                    // try. This might not be what we do, instead guide people towards `let else`.
                    (true, false) => parse_quote!(iterator_item::async_gen_try_bare!(#expr)),
                    (false, false) => parse_quote!(iterator_item::gen_try_bare!(#expr)),
                };
            }
            _ => {}
        }
    }
}
