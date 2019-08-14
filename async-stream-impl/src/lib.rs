extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree};
use proc_macro2::Span;
use quote::quote;
use syn::visit_mut::VisitMut;

struct Scrub {
    is_xforming: bool,
    is_try: bool,
    unit: Box<syn::Expr>,
    num_yield: u32,
}

#[derive(Debug)]
struct AsyncStreamEnumHack {
    macro_ident: syn::Ident,
    stmts: Vec<syn::Stmt>,
}

impl AsyncStreamEnumHack {
    fn parse(input: TokenStream) -> Self {
        macro_rules! n {
            ($i:ident) => {
                $i.next().unwrap()
            };
        }

        let mut input = input.into_iter();
        n!(input); // enum
        n!(input); // ident

        let mut braces = match n!(input) {
            TokenTree::Group(group) => group.stream().into_iter(),
            _ => unreachable!(),
        };

        n!(braces); // Dummy
        n!(braces); // =
        n!(braces); // $crate
        n!(braces); // :
        n!(braces); // :
        n!(braces); // scrub
        n!(braces); // !

        let inner = n!(braces);
        let syn::Block { stmts, .. } = syn::parse(inner.clone().into()).unwrap();

        let macro_ident = syn::Ident::new(
            &format!("stream_{}", count_bangs(inner.into())),
            Span::call_site(),
        );

        AsyncStreamEnumHack { stmts, macro_ident }
    }
}

impl VisitMut for Scrub {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        if !self.is_xforming {
            syn::visit_mut::visit_expr_mut(self, i);
            return;
        }

        match i {
            syn::Expr::Yield(yield_expr) => {
                self.num_yield += 1;

                let value_expr = if let Some(ref e) = yield_expr.expr {
                    e
                } else {
                    &self.unit
                };

                // let ident = &self.yielder;

                *i = if self.is_try {
                    syn::parse_quote! { __yield_tx.send(Ok(#value_expr)).await }
                } else {
                    syn::parse_quote! { __yield_tx.send(#value_expr).await }
                };
            }
            syn::Expr::Try(try_expr) => {
                // let ident = &self.yielder;
                let e = &try_expr.expr;

                *i = syn::parse_quote! {
                    match #e {
                        Ok(v) => v,
                        Err(e) => {
                            __yield_tx.send(Err(e)).await;
                            return;
                        }
                    }
                };
            }
            syn::Expr::Closure(_) | syn::Expr::Async(_) => {
                let prev = self.is_xforming;
                self.is_xforming = false;
                syn::visit_mut::visit_expr_mut(self, i);
                self.is_xforming = prev;
            }
            _ => syn::visit_mut::visit_expr_mut(self, i),
        }
    }

    fn visit_item_mut(&mut self, i: &mut syn::Item) {
        let prev = self.is_xforming;
        self.is_xforming = false;
        syn::visit_mut::visit_item_mut(self, i);
        self.is_xforming = prev;
    }
}

#[proc_macro_derive(AsyncStreamHack)]
pub fn async_stream_impl(input: TokenStream) -> TokenStream {
    let AsyncStreamEnumHack {
        macro_ident,
        mut stmts,
    } = AsyncStreamEnumHack::parse(input);

    let mut scrub = Scrub {
        is_xforming: true,
        is_try: false,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts[..] {
        scrub.visit_stmt_mut(&mut stmt);
    }

    if scrub.num_yield == 0 {
        quote!(macro_rules! #macro_ident {
            () => {{
                if false {
                    __yield_tx.send(()).await;
                }

                #(#stmts)*
            }};
        })
        .into()
    } else {
        quote!(macro_rules! #macro_ident {
            () => {{
                #(#stmts)*
            }};
        })
        .into()
    }
}

#[proc_macro_derive(AsyncTryStreamHack)]
pub fn async_try_stream_impl(input: TokenStream) -> TokenStream {
    let AsyncStreamEnumHack {
        macro_ident,
        mut stmts,
    } = AsyncStreamEnumHack::parse(input);

    let mut scrub = Scrub {
        is_xforming: true,
        is_try: true,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts[..] {
        scrub.visit_stmt_mut(&mut stmt);
    }

    if scrub.num_yield == 0 {
        quote!(macro_rules! #macro_ident {
            () => {{
                if false {
                    __yield_tx.send(()).await;
                }

                #(#stmts)*
            }};
        })
        .into()
    } else {
        quote!(macro_rules! #macro_ident {
            () => {{
                #(#stmts)*
            }};
        })
        .into()
    }
}

fn count_bangs(input: TokenStream) -> usize {
    let mut count = 0;

    for token in input {
        match token {
            TokenTree::Punct(punct) => {
                if punct.as_char() == '!' {
                    count += 1;
                }
            }
            TokenTree::Group(group) => {
                count += count_bangs(group.stream());
            }
            _ => {}
        }
    }

    count
}
