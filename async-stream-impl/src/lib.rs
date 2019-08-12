extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_hack::proc_macro_hack;
use quote::quote;
use syn::Token;
use syn::parse::{Parse, ParseStream, Result};
use syn::visit_mut::VisitMut;

struct AsyncStreamImpl {
    yielder: syn::Ident,
    stmts: Vec<syn::Stmt>,
}

struct Scrub {
    is_try: bool,
    yielder: syn::Ident,
    unit: Box<syn::Expr>,
    num_yield: u32,
}

impl Parse for AsyncStreamImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let yielder: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut stmts = vec![];

        while !input.is_empty() {
            stmts.push(input.parse()?);
        }

        Ok(AsyncStreamImpl {
            yielder,
            stmts,
        })
    }
}

impl VisitMut for Scrub {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        match i {
            syn::Expr::Yield(yield_expr) => {
                self.num_yield += 1;

                let value_expr = if let Some(ref e) = yield_expr.expr {
                    e
                } else {
                    &self.unit
                };

                let ident = &self.yielder;

                *i = if self.is_try {
                    syn::parse_quote! { #ident.send(Ok(#value_expr)).await }
                } else {
                    syn::parse_quote! { #ident.send(#value_expr).await }
                };
            }
            syn::Expr::Try(try_expr) => {
                let ident = &self.yielder;
                let e = &try_expr.expr;

                *i = syn::parse_quote! {
                    match #e {
                        Ok(v) => v,
                        Err(e) => {
                            #ident.send(Err(e)).await;
                            return;
                        }
                    }
                };
            }
            expr => syn::visit_mut::visit_expr_mut(self, expr),
        }
    }
}

#[proc_macro_hack]
pub fn async_stream_impl(input: TokenStream) -> TokenStream {
    let AsyncStreamImpl {
        yielder,
        mut stmts,
    } = syn::parse_macro_input!(input as AsyncStreamImpl);

    let mut scrub = Scrub {
        is_try: false,
        yielder,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts {
        scrub.visit_stmt_mut(&mut stmt);
    }

    if scrub.num_yield == 0 {
        let yielder = &scrub.yielder;

        quote!({
            if false {
                #yielder.send(()).await;
            }

            #(#stmts)*
        }).into()
    } else {
        quote!({
            #(#stmts)*
        }).into()
    }
}

#[proc_macro_hack]
pub fn async_try_stream_impl(input: TokenStream) -> TokenStream {
    let AsyncStreamImpl {
        yielder,
        mut stmts,
    } = syn::parse_macro_input!(input as AsyncStreamImpl);

    let mut scrub = Scrub {
        is_try: true,
        yielder,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts {
        scrub.visit_stmt_mut(&mut stmt);
    }

    if scrub.num_yield == 0 {
        let yielder = &scrub.yielder;

        quote!({
            if false {
                #yielder.send(()).await;
            }

            #(#stmts)*
        }).into()
    } else {
        quote!({
            #(#stmts)*
        }).into()
    }
}
