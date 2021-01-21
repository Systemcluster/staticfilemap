# staticfilemap

[![Crates.io](https://img.shields.io/crates/v/staticfilemap)](https://crates.io/crates/staticfilemap)
[![Docs.rs](https://docs.rs/staticfilemap/badge.svg)](https://docs.rs/staticfilemap)

Procedural macro to create a static map of optionally compressed files during compilation.

Similar to `include_file!` or [`include_dir!`](https://crates.io/crates/include_dir), but accepts a list of files that can be specified through environment variables and supports compression with [LZ4](https://github.com/lz4/lz4) or [zstd](https://github.com/facebook/zstd).

## Usage

Derive from `StaticFileMap` to create a map.

Specify the files to be included and the names they should be accessed by with the `files` and `names` attributes.
These can either be strings containing values separated with `;`, or environment variables containing them when the `parse` attribute is set to `env`.
Relative paths are resolved relative to `CARGO_MANIFEST_DIR`. If the `names` attribute is not specified, the names are inferred from the filenames.

The compression level is controlled by the `compression` attribute. With a compression level above `0` the included files are compresed with LZ4. LZ4 accepts a compression level up to `12`.
Alternatively, zstd can be used for compression by setting the `algorithm` attribute to `zstd`. Zstd supports a compression level up to `21`.

The cargo features `lz4` and `zstd` are available to select compression support. `lz4` is enabled by default.

Files can be accessed as `&'static [u8]` slices by the `get(name)` and `get_match(name)` functions at runtime.
`get_match(name)` accepts partial names if only one name matches. `keys()` returns a list of all keys.

See the examples, [the tests](tests/tests.rs) or [the implementation](src/lib.rs) for details.

### Dependency

```toml
[dependencies]
staticfilemap = "^0.1"
```

### Examples

```rust
use staticfilemap::StaticFileMap;

#[derive(StaticFileMap)]
#[names = "a;b"]
#[files = "README.md;LICENSE"]
struct StaticMap;

fn main() {
    let content: &[u8] = StaticMap::get("b").unwrap();
    let keys: &[&str] = StaticMap::keys();
}
```

```rust
use staticfilemap::StaticFileMap;
use minilz4::Decode;
use std::io::Read;

#[derive(StaticFileMap)]
#[parse = "env"]
#[names = "FILENAMES"]
#[files = "FILEPATHS"]
#[compression = 8]
struct StaticMap;

fn main() {
    let compressed = StaticMap::get_match("diogenes")
        .expect("file matching diogenes was not included");
    let content = compressed.decode();
}
```

```rust
use staticfilemap::StaticFileMap;
use zstd::decode_all;

#[derive(StaticFileMap)]
#[parse = "env"]
#[names = "FILENAMES"]
#[files = "FILEPATHS"]
#[compression = 8]
#[algorithm = "zstd"]
struct StaticMap;

fn main() {
    let mut compressed = StaticMap::get("diogenes.txt")
        .expect("file diogenes.txt was not included");
    let content = decode_all(&mut compressed);
}
```

```rust
use staticfilemap::StaticFileMap;

#[derive(StaticFileMap)]
#[files = "README.md;LICENSE"]
struct StaticMap;

fn main() {
    for (key, data) in StaticMap::iter() {
        println!("{}: {}", key, String::from_utf8_lossy(data));
    }
}
```
