extern crate proc_macro;

use minilz4::{BlockMode, BlockSize, EncoderBuilder};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::{
    env::var,
    fs::File,
    io::{copy, BufReader, BufWriter},
    path::Path,
};
use syn::{parse_macro_input, DeriveInput, Lit, Meta, MetaNameValue};

#[proc_macro_derive(StaticFileMap, attributes(parse, names, files, compression))]
pub fn file_map(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);
    let get_attr = |name| {
        input.attrs.iter().find(|attr| match attr.path.get_ident() {
            Some(ident) if ident.to_string() == name => true,
            _ => false,
        })
    };
    let get_attr_str = |name, default: Option<String>| {
        match get_attr(name) {
            None => default,
            Some(attr) => match attr.parse_meta() {
                Ok(Meta::NameValue(MetaNameValue {
                    lit: Lit::Str(value),
                    ..
                })) => Some(value.value()),
                _ => None,
            },
        }
        .expect(&format!(
            "#[derive(StaticFileMap)] invalid or missing attribute: #[{} = ..], must be a string",
            name
        ))
    };
    let get_attr_num = |name, default: Option<u32>| {
        match get_attr(name) {
            None => default,
            Some(attr) => match attr.parse_meta() {
                Ok(Meta::NameValue(MetaNameValue {
                    lit: Lit::Int(value),
                    ..
                })) => value.base10_parse::<u32>().ok(),
                _ => None,
            },
        }
        .expect(&format!(
            "#[derive(StaticFileMap)] invalid or missing attribute: #[{} = ..], must be a positive number",
            name
        ))
    };
    let parse = get_attr_str("parse", Some("string".to_string()));
    let mut names = get_attr_str("names", None);
    let mut files = get_attr_str("files", None);
    let compression = get_attr_num("compression", Some(0));
    if ["env"].contains(&parse.as_str()) {
        names = var(&names).expect(&format!(
            "#[derive(StaticFileMap)] #[names = ..] environment variable {} not found",
            names
        ));
        files = var(&files).expect(&format!(
            "#[derive(StaticFileMap)] #[files = ..] environment variable {} not found",
            files,
        ));
    } else if ["string"].contains(&parse.as_str()) {
        names = names;
        files = files;
    } else {
        panic!(
            "#[derive(StaticFileMap)] #[parse = ..] supports the following values: \"env\", \"string\""
        )
    }
    let names = names
        .split(";")
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let files = files
        .split(";")
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if names.len() != files.len() {
        panic!(
            "#[derive(StaticFileMap)] #[names = ..] must contain the same number of items as #[files = ..]"
        )
    }
    let len = names.len();
    let data = files
        .iter()
        .map(|file| {
            let source = Path::new(&var("CARGO_MANIFEST_DIR").unwrap()).join(file);
            let file = File::open(&source).expect(&format!(
                "#[derive(StaticFileMap)] file does not exist: {}",
                source.display()
            ));
            let buffer = if compression > 0 {
                let mut encoder = EncoderBuilder::new()
                    .auto_flush(false)
                    .level(compression)
                    .block_mode(BlockMode::Linked)
                    .block_size(BlockSize::Max64KB)
                    .build(Vec::new())
                    .unwrap();
                {
                    let mut reader = BufReader::new(&file);
                    let mut writer = BufWriter::new(&mut encoder);
                    copy(&mut reader, &mut writer).expect(&format!(
                        "#[derive(StaticFileMap)] error reading/compressing file: {}",
                        source.display()
                    ));
                }
                let buffer = encoder.finish().expect(&format!(
                    "#[derive(StaticFileMap)] error compressing file: {}",
                    source.display()
                ));
                buffer
            } else {
                let mut reader = BufReader::new(&file);
                let mut writer = BufWriter::new(Vec::new());
                copy(&mut reader, &mut writer).expect(&format!(
                    "#[derive(StaticFileMap)] error reading file: {}",
                    source.display()
                ));
                Vec::from(writer.buffer())
            };
            buffer
        })
        .collect::<Vec<Vec<u8>>>();
    let ident = &input.ident;
    let data_ident = format_ident!("{}Data", ident);
    let trait_ident = format_ident!("{}Trait", ident);
    let map_data = data
        .iter()
        .map(|data| {
            quote! { &[ #( #data ),* ] }
        })
        .collect::<Vec<_>>();
    let ids1 = 0..len;
    let ids2 = 0..len;
    let result = quote! {

        #[allow(non_upper_case_globals)]
        static #data_ident : &'static [ &'static [u8] ; #len ] = &[ #( #map_data ),* ];

        trait #trait_ident {
            fn keys() -> &'static [&'static str; #len ];
            fn get<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]>;
            fn get_match<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]>;
        }

        impl #trait_ident for #ident {
            #[inline]
            fn keys() -> &'static [&'static str; #len ] {
                static _k: &'static [&'static str; #len ] = &[ #( #names ),* ];
                _k
            }
            #[inline]
            fn get<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]> {
                let name = name.as_ref();
                #(
                    if name == #names {
                        return Some( #data_ident [ #ids1 ] );
                    }
                )*
                None
            }
            fn get_match<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]> {
                let name = name.as_ref();
                let mut matches = 0;
                let mut matching = 0;
                #(
                    if #names.contains(name) {
                        if matches == 1 {
                            return None;
                        }
                        matches += 1;
                        matching = #ids2 ;
                    }
                )*
                if matches == 1 {
                    return Some( #data_ident [matching] );
                }
                None
            }
        }
    };
    result.into()
}
