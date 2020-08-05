use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::visit_mut::VisitMut;

struct Scrub {
    is_xforming: bool,
    is_try: bool,
    unit: Box<syn::Expr>,
    num_yield: u32,
}

fn parse_input(input: TokenStream) -> syn::Result<Vec<syn::Stmt>> {
    let input = replace_for_await(input.into());
    // syn does not provide a way to parse `Vec<Stmt>` directly from `TokenStream`,
    // so wrap input in a brace and then parse it as a block.
    let input = TokenStream2::from(TokenTree::Group(Group::new(Delimiter::Brace, input)));
    let syn::Block { stmts, .. } = syn::parse2(input)?;

    Ok(stmts)
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
                syn::visit_mut::visit_expr_try_mut(self, try_expr);
                // let ident = &self.yielder;
                let e = &try_expr.expr;

                *i = syn::parse_quote! {
                    match #e {
                        Ok(v) => v,
                        Err(e) => {
                            __yield_tx.send(Err(e.into())).await;
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
            syn::Expr::ForLoop(expr) => {
                syn::visit_mut::visit_expr_for_loop_mut(self, expr);
                // TODO: Should we allow other attributes?
                if expr.attrs.len() != 1 || !expr.attrs[0].path.is_ident("await") {
                    return;
                }
                let syn::ExprForLoop {
                    attrs,
                    label,
                    pat,
                    expr,
                    body,
                    ..
                } = expr;

                let attr = attrs.pop().unwrap();
                if let Err(e) = syn::parse2::<syn::parse::Nothing>(attr.tokens) {
                    *i = syn::parse2(e.to_compile_error()).unwrap();
                    return;
                }

                *i = syn::parse_quote! {{
                    let mut __pinned = #expr;
                    let mut __pinned = unsafe {
                        ::core::pin::Pin::new_unchecked(&mut __pinned)
                    };
                    #label
                    loop {
                        let #pat = match ::async_stream::reexport::next(&mut __pinned).await {
                            ::core::option::Option::Some(e) => e,
                            ::core::option::Option::None => break,
                        };
                        #body
                    }
                }}
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

/// Asynchronous stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```rust
/// use async_stream::stream;
///
/// use futures_util::pin_mut;
/// use futures_util::stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let s = stream! {
///         for i in 0..3 {
///             yield i;
///         }
///     };
///
///     pin_mut!(s); // needed for iteration
///
///     while let Some(value) = s.next().await {
///         println!("got {}", value);
///     }
/// }
/// ```
#[proc_macro]
pub fn stream(input: TokenStream) -> TokenStream {
    let mut stmts = match parse_input(input) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut scrub = Scrub {
        is_xforming: true,
        is_try: false,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts[..] {
        scrub.visit_stmt_mut(&mut stmt);
    }

    let dummy_yield = if scrub.num_yield == 0 {
        Some(quote!(if false {
            __yield_tx.send(()).await;
        }))
    } else {
        None
    };

    quote!({
        let (mut __yield_tx, __yield_rx) = ::async_stream::yielder::pair();
        ::async_stream::AsyncStream::new(__yield_rx, async move {
            #dummy_yield
            #(#stmts)*
        })
    })
    .into()
}

/// Asynchronous fallible stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```rust
/// use tokio::net::{TcpListener, TcpStream};
///
/// use async_stream::try_stream;
/// use futures_core::stream::Stream;
///
/// use std::io;
/// use std::net::SocketAddr;
///
/// fn bind_and_accept(addr: SocketAddr)
///     -> impl Stream<Item = io::Result<TcpStream>>
/// {
///     try_stream! {
///         let mut listener = TcpListener::bind(addr).await?;
///
///         loop {
///             let (stream, addr) = listener.accept().await?;
///             println!("received on {:?}", addr);
///             yield stream;
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn try_stream(input: TokenStream) -> TokenStream {
    let mut stmts = match parse_input(input) {
        Ok(x) => x,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut scrub = Scrub {
        is_xforming: true,
        is_try: true,
        unit: syn::parse_quote!(()),
        num_yield: 0,
    };

    for mut stmt in &mut stmts[..] {
        scrub.visit_stmt_mut(&mut stmt);
    }

    let dummy_yield = if scrub.num_yield == 0 {
        Some(quote!(if false {
            __yield_tx.send(()).await;
        }))
    } else {
        None
    };

    quote!({
        let (mut __yield_tx, __yield_rx) = ::async_stream::yielder::pair();
        ::async_stream::AsyncStream::new(__yield_rx, async move {
            #dummy_yield
            #(#stmts)*
        })
    })
    .into()
}

fn replace_for_await(input: TokenStream2) -> TokenStream2 {
    let mut input = input.into_iter().peekable();
    let mut tokens = Vec::new();

    while let Some(token) = input.next() {
        match token {
            TokenTree::Ident(ident) => {
                match input.peek() {
                    Some(TokenTree::Ident(next)) if ident == "for" && next == "await" => {
                        tokens.extend(quote!(#[#next]));
                        let _ = input.next();
                    }
                    _ => {}
                }
                tokens.push(ident.into());
            }
            TokenTree::Group(group) => {
                let stream = replace_for_await(group.stream());
                tokens.push(Group::new(group.delimiter(), stream).into());
            }
            _ => tokens.push(token),
        }
    }

    tokens.into_iter().collect()
}
