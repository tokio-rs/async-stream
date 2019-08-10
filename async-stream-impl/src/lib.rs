extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_hack::proc_macro_hack;
use quote::quote;
use syn::Token;
use syn::parse::{Parse, ParseStream, Result};
use syn::visit_mut::VisitMut;

struct AsyncStreamImpl {
    stmts: Vec<syn::Stmt>,
}

struct Scrub {
    yielder: syn::Ident,
    unit: Box<syn::Expr>,
}

impl Parse for AsyncStreamImpl {
    fn parse(input: ParseStream) -> Result<Self> {
        let yielder: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut stmts = vec![];
        let mut scrub = Scrub {
            yielder,
            unit: syn::parse_quote!(()),
        };

        while !input.is_empty() {
            let mut stmt = input.parse()?;
            scrub.visit_stmt_mut(&mut stmt);
            stmts.push(stmt);
        }

        Ok(AsyncStreamImpl { stmts })
    }
}

impl VisitMut for Scrub {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        match i {
            syn::Expr::Yield(expr) => {
                let value_expr = if let Some(ref e) = expr.expr {
                    e
                } else {
                    &self.unit
                };

                let ident = &self.yielder;
                *i = syn::parse_quote! { #ident.send(#value_expr).await };
            }
            expr => syn::visit_mut::visit_expr_mut(self, expr),
        }
    }
}

#[proc_macro_hack]
pub fn async_stream_impl(input: TokenStream) -> TokenStream {
    let AsyncStreamImpl {
        stmts,
    } = syn::parse_macro_input!(input as AsyncStreamImpl);

    quote!(#(#stmts)*).into()
    // quote!(#input).into()
}
