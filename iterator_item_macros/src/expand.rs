use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned,
    token::Brace,
    visit_mut::VisitMut,
    Attribute, Block, Expr, Item, Macro, Result, Stmt,
};

pub struct GenMacro {
    pub body: Block,
    pub is_async: bool,
    pub is_try_yield: bool,
    pub attributes: Vec<Attribute>,
}

impl GenMacro {
    pub fn build(self) -> Expr {
        let GenMacro {
            mut body,
            is_async,
            is_try_yield,
            attributes,
        } = self;

        let mut visitor = BodyVisitor::new(is_async, is_try_yield);
        visitor.visit_block_mut(&mut body);

        let expansion = if is_async {
            quote!(::iterator_item::__internal::AsyncIteratorItem { gen, size_hint })
        } else {
            quote!(::iterator_item::__internal::IteratorItem { gen, size_hint })
        };
        let head = if is_async {
            quote!(static move |mut __stream_ctx|)
        } else {
            quote!(move ||)
        };

        let mut size_hint = quote!((0, None));
        for attr in attributes {
            // An annotation of the type `#[size_hint((0, None))] fn* foo() { ... }` lets the end
            // user provide code to override the default return of `Iterator::size_hint`.
            // FIXME: verify if an alternative name should be considered.
            // Once we do this is in the compiler, we can observe the materialized types of all the
            // arguments, *and* thier uses, so that for simpler cases where iterators are being
            // consumed once and without nesting, we can come up with an accurate `size_hint` (or
            // at least as accurate as the `size_hint()` call is for the inputs).
            // FIXME: we can do some of the above by modifying `Visitor` to keep track of renames
            // and reassigns of the input bindings and of them being iterated on in for loops, but
            // this will be tricky to get right.
            if attr.path.is_ident("size_hint") {
                size_hint = attr.tokens.clone();
            }
        }

        // The `yield panic!()` in the desugaring is to allow an empty body in the input to still
        // expand to a generator. `rustc` relies on the presence of a `yield` statement in a
        // closure body to turn it into a generator.
        let tail = quote! {
            #[allow(unreachable_code)]
            {
                return;
                yield panic!();
            }
        };

        parse_quote! {
                #[allow(unused_parens, unused_braces)]
                {
                    let size_hint = #size_hint;
                    let gen = #head {
                        #body
                        #tail
                    };
                    #expansion
            }
        }
    }

    fn convert_macro(mac: &Macro, attributes: &[Attribute]) -> Option<Expr> {
        let is_gen = mac.path.is_ident("gen");
        let is_async_gen = mac.path.is_ident("async_gen");
        if is_gen || is_async_gen {
            let gen = mac.parse_body::<GenMacro>();
            Some(match gen {
                Ok(mut gen) => {
                    gen.is_async = is_async_gen;
                    gen.attributes.extend_from_slice(attributes);
                    gen.build()
                }
                Err(e) => {
                    let e = e.into_compile_error();
                    parse_quote! { { #e } }
                }
            })
        } else {
            None
        }
    }
}

impl Parse for GenMacro {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(GenMacro {
            body: Block {
                brace_token: Brace { span: input.span() },
                stmts: Block::parse_within(input)?,
            },
            is_async: false,
            is_try_yield: false,
            attributes: Vec::new(),
        })
    }
}

pub struct GenMacroExpander;

impl VisitMut for GenMacroExpander {
    fn visit_stmt_mut(&mut self, i: &mut Stmt) {
        if let Stmt::Item(Item::Macro(m)) = i {
            if let Some(e) = GenMacro::convert_macro(&m.mac, &m.attrs) {
                *i = Stmt::Expr(e);
            }
        } else {
            syn::visit_mut::visit_stmt_mut(self, i);
        }
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        if let Expr::Macro(m) = i {
            if let Some(e) = GenMacro::convert_macro(&m.mac, &m.attrs) {
                *i = e;
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
