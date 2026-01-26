#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use std::collections::HashSet;

use proc_macro2::{Ident, Span};

use quote::quote;
use syn::{AttributeArgs, ItemFn, Lit, Meta, MetaList, MetaNameValue, NestedMeta, parse_macro_input};

/// The `counter` attribute macro instruments a function with a metrics counter,
/// allowing you to measure how many times a function is called. It requires to specify
/// `measurement` name under which the count will be recorded. It also accepts optional `tags`
/// represented as comma-separated list of key-value tuples such as `tags(key1 = "value1", key2 = "value2")`.
/// The function name (`fn_name`) is automatically added as a tag, so there is no need to include it manually.
/// All keys must be unique.
///
/// ## Examples
///
/// Instrument function with a counter with tags.
///
/// ```ignore
/// use metricus_macros::counter;
///
/// #[counter(measurement = "counters", tags(key1 = "value1", key2 = "value2"))]
/// fn my_function_with_tags() {
///     // function body
/// }
/// ```
/// In the above example, each call to `my_function_with_tags` increments a counter with the measurement name
/// "counters" and tagged with the environment. The function name is automatically tagged.
///
/// Instrument function wut h a counter without tags.
///
/// ```ignore
/// use metricus_macros::counter;
///
/// #[counter(measurement = "counters")]
/// fn my_function_without_tags() {
///     // function body
/// }
/// ```
/// Here, each call to `my_function_without_tags` increments a counter with the measurement name
/// "counters". Only the function name is tagged automatically, since no additional tags were provided.
#[proc_macro_attribute]
pub fn counter(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // initialize variables to hold parsed values
    let mut measurement = None;
    let mut tags = Vec::new();

    // auto include method name
    let method_name = fn_name.to_string();
    tags.push(("fn_name".to_string(), method_name));

    // keys must be unique
    let keys: HashSet<String> = tags.iter().map(|(k, _)| k).cloned().collect();
    assert_eq!(keys.len(), tags.len(), "must include unique tag keys");

    // Parse attributes for measurement and tags
    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                ref path,
                lit: Lit::Str(ref value),
                ..
            })) if path.is_ident("measurement") => {
                measurement = Some(value.value());
            }
            NestedMeta::Meta(Meta::List(MetaList {
                ref path, ref nested, ..
            })) if path.is_ident("tags") => {
                for meta in nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(value),
                        ..
                    })) = meta
                    {
                        tags.push((path.get_ident().unwrap().to_string(), value.value()));
                    } else {
                        return TokenStream::from(
                            syn::Error::new_spanned(meta, "Expected a name-value pair for tags").to_compile_error(),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // Ensure consistent ordering of tags
    tags.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

    let tags: Vec<(&str, &str)> = tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let tags = tags.into_iter().map(|(k, v)| {
        // Directly quote each tuple
        quote! { (#k, #v) }
    });

    // Ensure measurement field is provided
    let measurement = match measurement {
        Some(measurement) => measurement,
        None => {
            return TokenStream::from(
                syn::Error::new_spanned(&input_fn, "Missing required 'measurement' field").to_compile_error(),
            );
        }
    };

    let measurement = measurement.as_str();

    // Reconstruct the original function and inject the counter

    let fn_body = &input_fn.block.stmts;
    let fn_vis = &input_fn.vis;
    let fn_unsafe = &input_fn.sig.unsafety;
    let fn_async = &input_fn.sig.asyncness;
    let fn_args = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_generics = &input_fn.sig.generics;
    let fn_where_clause = &input_fn.sig.generics.where_clause;
    let attrs = &input_fn.attrs;

    let generated = quote! {
        #(#attrs)*
        #fn_vis #fn_async #fn_unsafe fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where_clause {

            static mut COUNTER: core::cell::LazyCell<metricus::Counter> = core::cell::LazyCell::new(|| metricus::Counter::new(#measurement, &[ #(#tags),* ]));
            #[allow(static_mut_refs)]
            unsafe { metricus::CounterOps::increment(&COUNTER); }

            #( #fn_body )*
        }
    };

    generated.into()
}

/// The `counter_with_id` attribute macro instruments a function with a metrics counter with a user
/// supplied id. This can be useful to provide instrumentation for memory allocators where we need to 'defer' metric
/// registration until the backend has been registered.
///
/// This macro accepts either `u64` value that represents counter `id` or the name of a const function that returns the id
/// of the counter to be created.
///
/// ## Examples
///
/// Using integer literal as id.
///
/// ```ignore
/// use metricus_macros::counter_with_id;
///
/// #[counter_with_id(id = 100)]
/// fn my_function() {
///     // function body
/// }
/// ```
///
/// Using const expression as id.
///
/// ```ignore
/// use metricus_macros::counter_with_id;
///
/// const fn get_counter_id() -> CounterId {
///     100
/// }
///
/// #[counter_with_id(id = "get_counter_id")]
/// fn my_function() {
///     // function body
/// }
/// ```
#[proc_macro_attribute]
pub fn counter_with_id(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // Initialize variables to hold parsed values
    let mut counter_id_fn = None;
    let mut counter_id_value = None;

    // Parse attributes for measurement and tags
    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                ref path,
                lit: Lit::Str(ref value),
                ..
            })) if path.is_ident("id") => {
                counter_id_fn = Some(value.value());
            }
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                ref path,
                lit: Lit::Int(value),
                ..
            })) if path.is_ident("id") => {
                counter_id_value = Some(value);
            }
            _ => {}
        }
    }

    // Ensure counter_id field is provided
    let counter_id = match counter_id_value {
        Some(id_int) => quote! { #id_int },
        None => match counter_id_fn {
            Some(f) => {
                let getter_fn = Ident::new(f.as_str(), Span::call_site());
                quote! { #getter_fn() }
            }
            None => {
                return TokenStream::from(
                    syn::Error::new_spanned(&input_fn, "Missing required 'id' field").to_compile_error(),
                );
            }
        },
    };

    let fn_body = &input_fn.block.stmts;
    let fn_vis = &input_fn.vis;
    let fn_unsafe = &input_fn.sig.unsafety;
    let fn_async = &input_fn.sig.asyncness;
    let fn_args = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_generics = &input_fn.sig.generics;
    let fn_where_clause = &input_fn.sig.generics.where_clause;
    let attrs = &input_fn.attrs;

    let generated = quote! {
        #(#attrs)*
        #fn_vis #fn_async #fn_unsafe fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where_clause {

            static mut COUNTER: core::cell::LazyCell<metricus::Counter> = core::cell::LazyCell::new(|| metricus::Counter::new_with_id(#counter_id));
            #[allow(static_mut_refs)]
            unsafe { metricus::CounterOps::increment(&COUNTER); }

            #( #fn_body )*
        }
    };

    generated.into()
}

/// The `span` attribute macro instruments a function with a metrics span that will be recorded
/// using a histogram, allowing you to measure how long a given function took to execute
/// in nanoseconds. It requires to specify `measurement` name under which the count will be recorded.
/// It also accepts optional `tags` represented as comma-separated list of key-value tuples such as
/// `tags(key1 = "value1", key2 = "value2")`. The function name (`fn_name`) is automatically added
/// as a tag, so there is no need to include it manually. All keys must be unique.
///
/// ## Examples
///
/// Instrument function with a span with tags.
///
/// ```ignore
/// use metrics_macros::span;
///
/// #[span(measurement = "latencies", tags(key1 = "value1", key2 = "value2"))]
/// fn my_function_with_tags() {
///     // function body
/// }
/// ```
///
/// Instrument function with a span without tags.
///
/// ```ignore
/// use metrics_macros::span;
///
/// #[span(measurement = "latencies")]
/// fn my_function_without_tags() {
///     // function body
/// }
/// ```
#[proc_macro_attribute]
pub fn span(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // Initialize variables to hold parsed values
    let mut measurement = None;
    let mut tags = Vec::new();

    // auto include method name
    let method_name = fn_name.to_string();
    tags.push(("fn_name".to_string(), method_name));

    // keys must be unique
    let keys: HashSet<String> = tags.iter().map(|(k, _)| k).cloned().collect();
    assert_eq!(keys.len(), tags.len(), "must include unique tag keys");

    // Parse attributes for measurement and tags
    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                ref path,
                lit: Lit::Str(ref value),
                ..
            })) if path.is_ident("measurement") => {
                measurement = Some(value.value());
            }
            NestedMeta::Meta(Meta::List(MetaList {
                ref path, ref nested, ..
            })) if path.is_ident("tags") => {
                for meta in nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(value),
                        ..
                    })) = meta
                    {
                        tags.push((path.get_ident().unwrap().to_string(), value.value()));
                    } else {
                        return TokenStream::from(
                            syn::Error::new_spanned(meta, "Expected a name-value pair for tags").to_compile_error(),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // Ensure consistent ordering of tags
    tags.sort_unstable_by(|(k1, _), (k2, _)| k1.cmp(k2));

    let tags: Vec<(&str, &str)> = tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let tags = tags.into_iter().map(|(k, v)| {
        // Directly quote each tuple
        quote! { (#k, #v) }
    });

    // Ensure measurement field is provided
    let measurement = match measurement {
        Some(measurement) => measurement,
        None => {
            return TokenStream::from(
                syn::Error::new_spanned(&input_fn, "Missing required 'measurement' field").to_compile_error(),
            );
        }
    };

    let measurement = measurement.as_str();

    // Reconstruct the original function and inject the histogram span
    let fn_body = &input_fn.block.stmts;
    let fn_vis = &input_fn.vis;
    let fn_unsafe = &input_fn.sig.unsafety;
    let fn_args = &input_fn.sig.inputs;
    let fn_async = &input_fn.sig.asyncness;
    let fn_output = &input_fn.sig.output;
    let fn_generics = &input_fn.sig.generics;
    let fn_where_clause = &input_fn.sig.generics.where_clause;
    let attrs = &input_fn.attrs;

    let generated = quote! {
        #(#attrs)*
        #fn_vis #fn_async #fn_unsafe fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where_clause {

            static mut HISTOGRAM: core::cell::LazyCell<metricus::Histogram> = core::cell::LazyCell::new(|| metricus::Histogram::new(#measurement, &[ #(#tags),* ]));
            #[allow(static_mut_refs)]
            let _span = unsafe { metricus::HistogramOps::span(&HISTOGRAM) };

            #( #fn_body )*
        }
    };

    generated.into()
}
