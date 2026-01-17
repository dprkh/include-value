use macro_string::MacroString;

use proc_macro::TokenStream;

use proc_macro2::{Span, TokenStream as TokenStream2};

use quote::{ToTokens, quote};

use serde_json::Value;

use std::fs;

use std::path::{Path, PathBuf};

use syn::{Error, Result, parse_macro_input};

#[proc_macro]
pub fn include_value(input: TokenStream) -> TokenStream {
    let MacroString(path) = parse_macro_input!(input);

    do_include_value(&path)
        //
        .unwrap_or_else(syn::Error::into_compile_error)
        //
        .into()
}

#[proc_macro]
pub fn include_raw_value(input: TokenStream) -> TokenStream {
    let MacroString(path) = parse_macro_input!(input);

    do_include_raw_value(&path)
        //
        .unwrap_or_else(syn::Error::into_compile_error)
        //
        .into()
}

fn resolve_path(path_str: &str) -> Result<PathBuf> {
    let path = Path::new(path_str);

    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let caller_span = proc_macro::Span::call_site();

    caller_span
        //
        .local_file()
        //
        .and_then(|local_file| {
            local_file
                //
                .canonicalize()
                //
                .ok()
                //
                .and_then(|canonical| {
                    canonical
                        //
                        .parent()
                        //
                        .map(|parent| parent.join(path))
                })
        })
        //
        .ok_or_else(|| {
            Error::new(
                //
                Span::call_site(),
                //
                "Could not determine parent directory of source file",
            )
        })
}

fn do_include_value(path_str: &str) -> Result<TokenStream2> {
    let resolved_path = resolve_path(path_str)?;

    let resolved_path_str = resolved_path
        //
        .to_str()
        //
        .ok_or_else(|| Error::new(Span::call_site(), "Path contains invalid UTF-8"))?
        //
        .to_string();

    let content = match fs::read(&resolved_path) {
        Ok(content) => content,

        Err(err) => {
            return Err(Error::new(
                //
                Span::call_site(),
                //
                format!("{} {}", err, resolved_path.display()),
            ));
        }
    };

    let json: Value = match serde_json::from_slice(&content) {
        Ok(json) => json,

        Err(err) => {
            return Err(Error::new(
                //
                Span::call_site(),
                //
                format!("{} {}", err, resolved_path.display()),
            ));
        }
    };

    let value_tokens = PrintValue(&json);

    Ok(quote! {
        {
            const _: &[u8] = include_bytes!(#resolved_path_str);

            #value_tokens
        }
    })
}

fn do_include_raw_value(path_str: &str) -> Result<TokenStream2> {
    let resolved_path = resolve_path(path_str)?;

    let resolved_path_str = resolved_path
        //
        .to_str()
        //
        .ok_or_else(|| Error::new(Span::call_site(), "Path contains invalid UTF-8"))?
        //
        .to_string();

    let content = match fs::read(&resolved_path) {
        Ok(content) => content,

        Err(err) => {
            return Err(Error::new(
                //
                Span::call_site(),
                //
                format!("{} {}", err, resolved_path.display()),
            ));
        }
    };

    let json: Value = match serde_json::from_slice(&content) {
        Ok(json) => json,

        Err(err) => {
            return Err(Error::new(
                //
                Span::call_site(),
                //
                format!("{} {}", err, resolved_path.display()),
            ));
        }
    };

    let minified_json_str = match serde_json::to_string(&json) {
        Ok(s) => s,

        Err(err) => {
            return Err(Error::new(
                //
                Span::call_site(),
                //
                format!("{} {}", err, resolved_path.display()),
            ));
        }
    };

    Ok(quote! {
        {
            const _: &[u8] = include_bytes!(#resolved_path_str);

            unsafe {
                ::core::mem::transmute::<
                    &'static str,
                    &'static ::serde_json::value::RawValue,
                >(#minified_json_str)
            }
        }
    })
}

struct PrintValue<'a>(&'a Value);

impl ToTokens for PrintValue<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self.0 {
            Value::Null => tokens.extend(quote!(::serde_json::Value::Null)),

            Value::Bool(b) => tokens.extend(quote!(::serde_json::Value::Bool(#b))),

            Value::Number(n) => {
                let repr = n.to_string();

                tokens.extend(quote! {
                    ::serde_json::Value::Number(
                        ::core::str::FromStr::from_str(#repr).unwrap()
                    )
                });
            }

            Value::String(s) => {
                tokens.extend(quote! {
                    ::serde_json::Value::String(::core::convert::From::from(#s))
                });
            }

            Value::Array(vec) => {
                if vec.is_empty() {
                    tokens.extend(quote! {
                        ::serde_json::Value::Array(::core::default::Default::default())
                    });
                } else {
                    let elements = vec.iter().map(PrintValue);
                    tokens.extend(quote! {
                        ::serde_json::Value::Array(vec![#(#elements),*])
                    });
                }
            }

            Value::Object(map) => {
                if map.is_empty() {
                    tokens.extend(quote! {
                        ::serde_json::Value::Object(::serde_json::Map::new())
                    });
                } else {
                    let len = map.len();

                    let keys = map.keys();

                    let values = map.values().map(PrintValue);

                    tokens.extend(quote! {
                        ::serde_json::Value::Object({
                            let mut object = ::serde_json::Map::with_capacity(#len);

                            #(
                                let _ = object.insert(::core::convert::From::from(#keys), #values);
                            )*

                            object
                        })
                    });
                }
            }
        }
    }
}
