extern crate proc_macro;

use std::{
    env::var,
    fs::File,
    io::{copy, BufReader, BufWriter},
    path::{Path, PathBuf},
};

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Lit};

#[cfg(feature = "lz4")]
use minilz4::{BlockMode, BlockSize, EncoderBuilder};
#[cfg(feature = "zstd")]
use zstd::stream::copy_encode;


#[proc_macro_derive(StaticFileMap, attributes(parse, names, files, compression, algorithm))]
pub fn file_map(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);

    let get_attr = |name| {
        input
            .attrs
            .iter()
            .find(|attr| matches!(attr.path().get_ident(), Some(ident) if *ident == name))
    };
    let get_attr_str = |name, default: Option<String>| {
        match get_attr(name) {
            None => default,
            Some(attr) =>
                attr.parse_args()
                    .ok()
                    .and_then(|lit: Lit| match lit {
                        Lit::Str(value) => Some(value.value()),
                        _ => None
                    })
        }
        .unwrap_or_else(|| panic!("#[derive(StaticFileMap)] invalid or missing attribute: #[{}(..)], must be a string", name))
    };
    let get_attr_num = |name, default: Option<u32>| {
        match get_attr(name) {
            None => default,
            Some(attr) =>
                attr.parse_args()
                .ok()
                .and_then(|lit: Lit| match lit {
                    Lit::Int(value) => value.base10_parse::<u32>().ok(),
                    _ => None
                })

        }
        .unwrap_or_else(|| panic!("#[derive(StaticFileMap)] invalid or missing attribute: #[{}(..)], must be a positive number", name))
    };

    let parse = get_attr_str("parse", Some("string".to_string())).to_lowercase();
    let mut names = get_attr_str("names", Some("".to_string()));
    let mut files = get_attr_str("files", None);
    let compression = get_attr_num("compression", Some(0));
    let algorithm = get_attr_str("algorithm", Some("zstd".to_string())).to_lowercase();

    if !["lz4", "zstd"].contains(&algorithm.as_str()) {
        panic!(
            "#[derive(StaticFileMap)] #[algorithm(..)] supports the following values: \"lz4\", \"zstd\", got \"{}\"",
            &parse
        )
    }

    if ["env"].contains(&parse.as_str()) {
        files = var(&files).unwrap_or_else(|_| {
            panic!(
                "#[derive(StaticFileMap)] #[files(..)] environment variable {} not found",
                files
            )
        });
        if !names.is_empty() {
            names = var(&names).unwrap_or_else(|_| {
                panic!(
                    "#[derive(StaticFileMap)] #[names(..)] environment variable {} not found",
                    names
                )
            });
        }
    } else if ["string"].contains(&parse.as_str()) {
        // no change
    } else {
        panic!(
            "#[derive(StaticFileMap)] #[parse(..)] supports the following values: \"env\", \"string\", got \"{}\"",
            &parse
        )
    }

    let mut names = names
        .split(';')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let files = files
        .split(';')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let file_names = files
        .iter()
        .map(|file| {
            PathBuf::from(file)
                .file_name()
                .map(|n| n.to_os_string().to_string_lossy().to_string())
                .unwrap()
        })
        .collect::<Vec<_>>();
    if names.is_empty() {
        names = file_names.iter().map(|s| s.as_ref()).collect();
    }

    if names.len() != files.len() {
        panic!(
            "#[derive(StaticFileMap)] #[names(..)] must contain the same number of items as #[files(..)], got {} names and {} files",
            names.len(),
            files.len()
        )
    }
    let len = names.len();

    let data = files
        .iter()
        .map(|file| {
            let source = Path::new(&var("CARGO_MANIFEST_DIR").unwrap()).join(file);
            let file = File::open(&source).unwrap_or_else(|_| {
                panic!(
                    "#[derive(StaticFileMap)] file does not exist: {}",
                    source.display()
                )
            });

            if compression > 0 {
                if algorithm == "lz4" {
                    #[cfg(feature = "lz4")]
                    {
                        let mut encoder = EncoderBuilder::new()
                            .auto_flush(false)
                            .level(compression)
                            .block_mode(BlockMode::Linked)
                            .block_size(BlockSize::Max64KB)
                            .build(Vec::new())
                            .unwrap();
                        {
                            let mut reader = BufReader::new(&file);
                            copy(&mut reader, &mut encoder).unwrap_or_else(|_| {
                                panic!(
                                    "#[derive(StaticFileMap)] error reading/compressing file: {}",
                                    source.display()
                                )
                            });
                        }

                        encoder.finish().unwrap_or_else(|_| {
                            panic!(
                                "#[derive(StaticFileMap)] error compressing file: {}",
                                source.display()
                            )
                        })
                    }
                    #[cfg(not(feature = "lz4"))]
                    panic!("#[derive(StaticFileMap)] lz4 compression requested but lz4 feature not enabled")
                } else {
                    #[cfg(feature = "zstd")]
                    {
                        let mut data = Vec::new();
                        let mut reader = BufReader::new(&file);
                        copy_encode(&mut reader, &mut data, compression as i32).unwrap_or_else(|_| {
                            panic!(
                                "#[derive(StaticFileMap)] error reading/compressing file: {}",
                                source.display()
                            )
                        });
                        data
                    }
                    #[cfg(not(feature = "zstd"))]
                    panic!("#[derive(StaticFileMap)] zstd compression requested but zstd feature not enabled")
                }
            } else {
                let mut buffer = Vec::new();
                {
                    let mut reader = BufReader::new(&file);
                    let mut writer = BufWriter::new(&mut buffer);
                    copy(&mut reader, &mut writer).unwrap_or_else(|_| {
                        panic!(
                            "#[derive(StaticFileMap)] error reading file: {}",
                            source.display()
                        )
                    });
                }
                buffer
            }
        })
        .collect::<Vec<Vec<u8>>>();

    let ident = &input.ident;
    let map_data = data
        .iter()
        .map(|data| {
            quote! { &[ #( #data ),* ] }
        })
        .collect::<Vec<_>>();

    let iter = format_ident!("{}Iterator", ident);

    let result = quote! {
        impl #ident {
            const _k: &'static [&'static str; #len ] = &[ #( #names ),* ];
            #[inline]
            pub const fn keys() -> &'static [&'static str; #len ] {
                Self::_k
            }
            const _d : &'static [ &'static [u8] ; #len ] = &[ #( #map_data ),* ];
            #[inline]
            pub const fn data() -> &'static [ &'static [u8] ; #len ] {
                Self::_d
            }
            #[inline]
            pub fn get<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]> {
                let name = name.as_ref();
                for (i, key) in Self::keys().iter().enumerate() {
                    if key == &name {
                        return Some(Self::get_index(i));
                    }
                }
                None
            }
            #[inline]
            pub const fn get_index(index: usize) -> &'static [u8] {
                Self::data()[index]
            }
            #[inline]
            pub fn get_match_index<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<usize> {
                let mut matches = 0;
                let mut matching = 0;
                let name = name.as_ref();
                for (i, key) in Self::keys().iter().enumerate() {
                    if key.contains(name) {
                        if matches == 1 {
                            return None;
                        }
                        matches += 1;
                        matching = i;
                    }
                }
                if matches == 1 {
                    Some(matching)
                }
                else {
                    None
                }
            }
            #[inline]
            pub fn get_match<S: ::core::convert::AsRef<str>>(name: S) -> ::core::option::Option<&'static [u8]> {
                if let Some(index) = Self::get_match_index(name) {
                    Some(Self::get_index(index))
                }
                else {
                    None
                }
            }
            #[inline]
            pub const fn iter() -> #iter {
                #iter { position: 0 }
            }
        }

        struct #iter {
            position: usize
        }
        impl ::core::iter::Iterator for #iter {
            type Item = (&'static str, &'static [u8]);
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                if self.position >= #len {
                    return None;
                }
                let ret = (#ident::keys()[self.position], #ident::data()[self.position]);
                self.position += 1;
                Some(ret)
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                (#len - self.position, Some(#len - self.position))
            }
        }
        impl ::core::iter::ExactSizeIterator for #iter {}
    };
    result.into()
}
