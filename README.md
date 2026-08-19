# hl2web

![Rust](https://img.shields.io/badge/language-Rust-orange) ![Unlicense](https://img.shields.io/badge/license-Unlicense-green)

Render a [HyperList](https://isene.org/hyperlist/) as one self-contained
interactive HTML page. No frameworks, no external assets, nothing to
host but the file itself.

<img src="img/hl2web.svg" align="left" width="150" height="150">

A HyperList can describe anything: plans, arguments, processes, systems.
Sharing one used to mean a PDF or a screenshot. hl2web gives the web the
real thing: folding, search, and the colors you know from hyperlist.vim.

<br clear="left"/>

## Usage

```sh
hl2web file.hl > file.html
hl2web --title "My List" file.hl > file.html
```

Open the HTML anywhere, phone included. Folding works even with
JavaScript disabled (native details/summary).

## What the page gives you

- Native folding: click any parent, or the 1/2/3/4/all/none buttons
- Live search: matching items highlight, ancestors unfold, rest hides
- hyperlist.vim colors: Operators blue, Properties red, Qualifiers
  green, Starters and references magenta, comments and quotes teal,
  hashtags yellow, FIXME/TODO black on yellow
- Checkboxes as symbols, `*bold* /italic/ _underline_` markup,
  literal blocks (`\`) rendered verbatim

## Install

```sh
git clone https://github.com/isene/hl2web
cd hl2web
cargo build --release
ln -s "$PWD/target/release/hl2web" ~/bin/hl2web
```

## License

Public domain (Unlicense). Do what you want with it.
