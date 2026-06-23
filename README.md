# carve (Python binding)

Native Python bindings for the [Carve](https://markup-carve.github.io/carve/)
markup language. This package is a thin [PyO3](https://pyo3.rs) binding over the
Rust reference implementation [carve-rs](https://github.com/markup-carve/carve-rs),
so the parser is not reimplemented in Python: every conversion delegates to the
same engine the Rust CLI and WASM builds use. Output is byte-identical to
carve-rs for the same input.

This unlocks the Python docs / data ecosystem (MkDocs, Sphinx, Pelican,
Jupyter/nbconvert) for Carve.

## Install

Wheels are abi3 (`abi3-py38`), so a single wheel covers CPython 3.8+.

From a built wheel:

```bash
pip install carve-*.whl
```

From source (needs a Rust toolchain, 1.75+):

```bash
pip install maturin
maturin develop --release      # build + install into the active venv
# or
maturin build --release        # produce a wheel under target/wheels/
```

## Usage

```python
import carve

print(carve.__version__)

# Core conversion (no extensions)
html = carve.to_html("# Hello *world*")
# -> '<h1 id="Hello">Hello <strong>world</strong></h1>\n'

# Inline emphasis: /italic/ and *bold*
carve.to_html("/italic/ and *bold*")

# Enable opt-in extensions by name
html = carve.to_html(source, extensions=["math_block", "list_table"])

# Dedicated explicit-list variant
html = carve.to_html_with_extensions(source, ["autolink"])

# Other renderers
carve.to_markdown(source)
carve.to_plain_text(source)
carve.to_ansi(source)

# Discover supported extension names
carve.extensions()
```

Passing an unknown extension name raises `ValueError`.

## Supported extensions

The string passed in `extensions=[...]` maps to a carve-rs extension:

| name                 | effect                                              |
|----------------------|-----------------------------------------------------|
| `autolink`           | turn bare URLs into links                            |
| `details`            | collapsible `<details>` blocks                       |
| `external_links`     | mark external links (rel/target)                     |
| `fenced_render`      | render fenced blocks of a target language (mermaid)  |
| `heading_permalinks` | add permalink anchors to headings                    |
| `list_table`         | build tables from nested lists                        |
| `math_block`         | fenced math blocks                                   |
| `spoiler`            | spoiler / hidden-content inline                       |
| `tab_normalize`      | normalize tab indentation                            |
| `wikilinks`          | `[[wiki style]]` links                               |
| `citations`          | citation references                                  |

## Switching the engine dependency for publishing

`Cargo.toml` uses a local `path` dependency on carve-rs so it builds in this
workspace. To build a publishable wheel, comment out the `path` line and
uncomment the `git` line (the public API is identical, so no Rust changes are
needed).
