#![feature(proc_macro_diagnostic)]

use crate::expand::GenMacro;

use self::macrofy::macrofy;
use expand::{BodyVisitor, GenMacroExpander};
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::*;
mod elision;
mod expand;
mod macrofy;

/// AST of an iterator item. Similar to an `Item::Fn`
///
/// We *could* use an `Fn` directly here, and get parsing from it, but given the objective of this
/// crate is to explore the syntactic space, doing all of the parsing ourselves seems like a better
/// approach.
enum IteratorItemParse {
    Ordinary {
        function: ItemFn,
    },
    Custom {
        attributes: Vec<Attribute>,
        visibility: Visibility,
        is_async: bool,
        name: Ident,
        generics: Generics,
        args: Punctuated<FnArg, Token![,]>,
        yields: Option<Type>,
        body: Block,
    },
}

fn check_fn_star(input: ParseStream) -> bool {
    let lookahead = input.lookahead1();
    if lookahead.peek(Token![*]) {
        input.parse::<Token![*]>().unwrap();
        true
    } else {
        false
    }
}

fn parse_fn_star(input: ParseStream) -> Result<IteratorItemParse> {
    // This will parse the following:
    // `#[attr(..)] #[attr2] pub async fn* foo(<args>) yields Ty { ... }`
    let attributes: Vec<Attribute> = input.call(Attribute::parse_outer)?;
    let visibility: Visibility = input.parse()?;
    let r#async: Option<Token![async]> = input.parse()?;
    input.parse::<Token![fn]>()?;
    input.parse::<Token![*]>()?;
    let name: Ident = input.parse()?;
    let generics: Generics = input.parse()?;
    let fn_args;
    parenthesized!(fn_args in input);
    let args = parse_fn_args(&fn_args)?;
    let yields: Option<Ident> = input.parse()?;
    let yields: Option<Type> = if let Some(yields) = yields {
        if yields != "yields" {
            return Err(Error::new(
                yields.span().unwrap().into(),
                "expected contextual keyword `yields` or the start of an iterator body",
            ));
            // FIXME: potentially deal better with this and try to recover the parse in a way
            // that doesn't spam an user that forgot to write yields or tried to write `->`.
        }
        Some(input.parse()?)
    } else {
        None
    };
    let body: Block = input.parse()?;
    Ok(IteratorItemParse::Custom {
        attributes,
        visibility,
        is_async: r#async.is_some(),
        name,
        generics,
        args,
        yields,
        body,
    })
}

fn check_gen_2996(input: ParseStream) -> bool {
    let lookahead = input.lookahead1();
    if lookahead.peek(Token![!]) {
        input.parse::<Token![!]>().unwrap();
        true
    } else {
        false
    }
}

fn parse_gen_2996(input: ParseStream) -> Result<IteratorItemParse> {
    // This will parse the following:
    // `#[attr(..)] #[attr2] pub async gen fn foo(<args>) -> Ty { ... }`
    let attributes: Vec<Attribute> = input.call(Attribute::parse_outer)?;
    let visibility: Visibility = input.parse()?;

    // Parse expected `gen` keyword. That's not currently a token, so hack it up.
    let gen: Option<Ident> = input.parse()?;
    let is_async;
    if let Some(gen) = gen {
        is_async = gen == "async_gen";
        if gen != "gen" && gen != "async_gen" {
            return Err(Error::new(
                gen.span().unwrap().into(),
                "expected keyword `gen` marking an iterator",
            ));
        }
    } else {
        return Err(Error::new(
            input.span().unwrap().into(),
            "expected keyword `gen` marking an iterator",
        ));
    }
    input.parse::<Token![!]>()?;
    input.parse::<Token![fn]>()?;
    let name: Ident = input.parse()?;
    let generics: Generics = input.parse()?;
    let fn_args;
    parenthesized!(fn_args in input);
    let args = parse_fn_args(&fn_args)?;
    // Parse optional right arrow token `->`, marking the beginning of the return type
    let lookahead = input.lookahead1();
    let yields: Option<Type> = if lookahead.peek(Token![->]) {
        input.parse::<Token![->]>()?;
        Some(input.parse()?)
    } else {
        None
    };
    let body: Block = input.parse()?;
    Ok(IteratorItemParse::Custom {
        attributes,
        visibility,
        is_async,
        name,
        generics,
        args,
        yields,
        body,
    })
}

fn check_gen_blocks(input: ParseStream) -> bool {
    let lookahead = input.lookahead1();
    if lookahead.peek(Token![#]) {
        input.parse::<Token![#]>().unwrap();
        true
    } else {
        false
    }
}

fn parse_gen_blocks(input: ParseStream) -> Result<IteratorItemParse> {
    Ok(IteratorItemParse::Ordinary {
        function: input.parse()?,
    })
}

impl Parse for IteratorItemParse {
    /// Hi! If you are looking to hack on this crate to come up with your own syntax, **look here**!
    fn parse(input: ParseStream) -> Result<Self> {
        if check_fn_star(input) {
            parse_fn_star(input)
        } else if check_gen_2996(input) {
            parse_gen_2996(input)
        } else if check_gen_blocks(input) {
            parse_gen_blocks(input)
        } else {
            Err(Error::new(
                input.span().unwrap().into(),
                "expected an iterator item syntax token: `*`, `!`, `#`",
            ))
        }
    }
}

impl IteratorItemParse {
    fn build(self) -> TokenStream {
        match self {
            IteratorItemParse::Custom {
                mut attributes,
                visibility,
                is_async,
                name,
                mut generics,
                args,
                yields,
                mut body,
            } => {
                let yields = match yields {
                    Some(ty) => ty,
                    None => Type::Tuple(TypeTuple {
                        paren_token: syn::token::Paren::default(),
                        elems: Punctuated::new(),
                    }),
                };
                let args = elision::unelide_lifetimes(&mut generics.params, args);
                let lifetimes: Vec<syn::Lifetime> =
                    generics.lifetimes().map(|l| l.lifetime.clone()).collect();

                let is_try_yield = match yields {
                    // This would be much nicer in `rustc` desugaring because we'd have access to name resolution.
                    Type::Path(TypePath {
                        qself: None,
                        ref path,
                    }) => {
                        let is_try = path
                            .segments
                            .first()
                            .map_or(false, |s| s.ident == "Result" || s.ident == "Option");
                        path.segments.len() == 1 && is_try
                    }
                    _ => false,
                };
                let mut visitor = BodyVisitor::new(is_async, is_try_yield);
                visitor.visit_block_mut(&mut body);

                let return_type = if is_async {
                    // Whey don't we use `std`'s `Stream` here?
                    // `Stream` is currently on the process of being reworked into `AsyncIterator`[1],
                    // leveraging associated `async fn` support that isn't yet in nightly. For now, we
                    // just rely on the library that people are actually using, the futures' crate Stream.
                    // [1]: https://rust-lang.github.io/wg-async-foundations/vision/roadmap/async_iter/traits.html
                    // quote! { impl ::core::stream::Stream<Item = #yields> #(+ #lifetimes)* }
                    quote!(impl ::futures::stream::Stream<Item = #yields> #(+ #lifetimes)*)
                } else {
                    quote!(impl ::core::iter::Iterator<Item = #yields> #(+ #lifetimes)*)
                };
                let args: Vec<_> = args.into_iter().collect();

                let expansion = GenMacro {
                    body,
                    is_async,
                    is_try_yield,
                    attributes: attributes.clone(),
                }
                .build();
                attributes.retain(|attr| !attr.path.is_ident("size_hint"));

                // Consider modifying this so that `gen` is `let gen = Box::pin(gen);`
                let expanded = quote! {
                    #(#attributes)* #visibility fn #name #generics(#(#args),*) -> #return_type {
                        #expansion
                    }
                };

                TokenStream::from(expanded)
            }
            IteratorItemParse::Ordinary { mut function } => {
                GenMacroExpander.visit_item_fn_mut(&mut function);
                function.to_token_stream().into()
            }
        }
    }
}

#[proc_macro]
pub fn iterator_item(input: TokenStream) -> TokenStream {
    // change `gen => gen!` and `async gen => async_gen!` so we get a second
    // shot at parsing wherever they appear in an expression
    let input = macrofy(input.into());
    // actually parse the macro input
    let item: IteratorItemParse = parse_macro_input!(input as IteratorItemParse);
    item.build()
}

/// Copied from `syn` because it exists but it is private 🤷
fn parse_fn_args(input: ParseStream) -> Result<Punctuated<FnArg, Token![,]>> {
    let mut args = Punctuated::new();
    let mut has_receiver = false;

    while !input.is_empty() {
        let attrs = input.call(Attribute::parse_outer)?;

        let arg = if let Some(dots) = input.parse::<Option<Token![...]>>()? {
            dots.span()
                .unwrap()
                .error("variadic arguments are not allowed in iterator items")
                .emit();
            continue;
        } else {
            let mut arg: FnArg = input.parse()?;
            match &mut arg {
                FnArg::Receiver(receiver) if has_receiver => {
                    return Err(Error::new(
                        receiver.self_token.span,
                        "unexpected second method receiver",
                    ));
                }
                FnArg::Receiver(receiver) if !args.is_empty() => {
                    return Err(Error::new(
                        receiver.self_token.span,
                        "unexpected method receiver",
                    ));
                }
                FnArg::Receiver(receiver) => {
                    has_receiver = true;
                    receiver.attrs = attrs;
                }
                FnArg::Typed(arg) => arg.attrs = attrs,
            }
            arg
        };
        args.push_value(arg);

        if input.is_empty() {
            break;
        }

        let comma: Token![,] = input.parse()?;
        args.push_punct(comma);
    }

    Ok(args)
}
