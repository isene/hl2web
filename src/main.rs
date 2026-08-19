//! hl2web — render a HyperList (.hl) as one self-contained HTML page.
//!
//! Output: a single file, no external assets, no frameworks. Native
//! <details>/<summary> nesting gives folding without JavaScript; a small
//! inline script adds fold-to-level buttons and live search. Colors
//! follow hyperlist.vim, the authoritative syntax.
//!
//!     hl2web file.hl > file.html
//!     hl2web --title "My List" file.hl > file.html

use std::fmt::Write as _;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// hyperlist.vim palette, terminal colors transcribed to hex.
const CSS: &str = "
:root {
  --bg: #0d0d14; --fg: #d8d8d8; --dim: #777;
  --op: #4a8dff; --prop: #ff4444; --qual: #33cc55;
  --start: #d75fff; --ref: #d75fff; --cmt: #33bbbb;
  --tag: #e0d020; --todo-bg: #e0d020; --todo-fg: #000;
  --hit: #2a2a40;
}
* { box-sizing: border-box; }
body { background: var(--bg); color: var(--fg);
  font-family: 'JetBrains Mono', 'DejaVu Sans Mono', monospace;
  font-size: 15px; line-height: 1.55; margin: 0; padding: 1.2em 1.6em 4em; }
header { position: sticky; top: 0; background: var(--bg);
  padding: .6em 0; border-bottom: 1px solid #222; z-index: 2; }
header input { background: #16161f; color: var(--fg); border: 1px solid #333;
  border-radius: 4px; padding: .3em .6em; font: inherit; width: 16em; }
header button { background: #16161f; color: var(--dim); border: 1px solid #333;
  border-radius: 4px; padding: .25em .55em; font: inherit; cursor: pointer; }
header button:hover { color: var(--fg); }
h1 { font-size: 1.15em; margin: 0 0 .4em; }
details { margin: 0; }
details > summary { list-style: none; cursor: pointer; }
details > summary::before { content: '▸ '; color: var(--dim); }
details[open] > summary::before { content: '▾ '; }
summary::-webkit-details-marker { display: none; }
.item { white-space: pre-wrap; }
.leaf::before { content: '  '; color: var(--dim); }
.kids { margin-left: 1.6em; border-left: 1px solid #1e1e2a; padding-left: .5em; }
.op { color: var(--op); font-weight: bold; }
.prop { color: var(--prop); }
.qual { color: var(--qual); }
.start { color: var(--start); }
.ref { color: var(--ref); }
a.ref { text-decoration: underline; }
.cmt { color: var(--cmt); }
.tag { color: var(--tag); }
.todo { background: var(--todo-bg); color: var(--todo-fg); }
.lit { color: var(--fg); opacity: .85; }
.hidden { display: none; }
.hit > summary, .hit.item { background: var(--hit); }
footer { color: var(--dim); margin-top: 2em; font-size: .8em; }
b { font-weight: bold; } i { font-style: italic; } u { text-decoration: underline; }
.cb { cursor: pointer; font-size: 1.55em; font-weight: bold; line-height: 1; vertical-align: -0.1em; }
body.light {
  --bg: #fbfaf6; --fg: #222; --dim: #999;
  --op: #0044cc; --prop: #c00000; --qual: #0a7a33;
  --start: #a000a0; --ref: #a000a0; --cmt: #007a7a;
  --tag: #7a6000; --hit: #ece8cf;
}
body.light header { border-bottom-color: #ddd; }
body.light header input, body.light header button {
  background: #f0eee6; border-color: #ccc; }
body.light .kids { border-left-color: #e2ded2; }
";

const JS: &str = "
function level(n){document.querySelectorAll('details').forEach(d=>{
  d.open = (n<0)||(parseInt(d.dataset.l)<n);});}
function cb(ev,el){ev.preventDefault();ev.stopPropagation();
  var t=el.textContent;
  el.textContent = t.indexOf('\\u2610')>=0 ? '\\u2611 ' : '\\u2610 ';}
function go(id){var e=document.getElementById(id);if(!e)return;
  var p=e.parentElement;
  while(p&&p.id!=='root'){if(p.tagName==='DETAILS')p.open=true;p=p.parentElement;}
  var t=(e.tagName==='DETAILS')?e.querySelector('summary'):e;
  t.scrollIntoView({block:'center'});
  e.classList.add('hit');setTimeout(function(){e.classList.remove('hit');},1500);}
function theme(){document.body.classList.toggle('light');
  try{localStorage.setItem('hl2web-theme',
    document.body.classList.contains('light')?'light':'dark');}catch(e){}}
try{if(localStorage.getItem('hl2web-theme')==='light')
  document.body.classList.add('light');}catch(e){}
function search(q){
  q=q.toLowerCase();
  document.querySelectorAll('.node').forEach(e=>{
    e.classList.remove('hidden','hit');});
  if(!q)return;
  document.querySelectorAll('.node').forEach(e=>{e.classList.add('hidden');});
  document.querySelectorAll('.node').forEach(e=>{
    var t=(e.dataset.t||'').toLowerCase();
    if(t.indexOf(q)>=0){
      e.classList.remove('hidden');e.classList.add('hit');
      var p=e.parentElement;
      while(p&&p.id!=='root'){
        if(p.classList&&p.classList.contains('node')){
          p.classList.remove('hidden');
          if(p.tagName==='DETAILS')p.open=true;}
        p=p.parentElement;}
      e.querySelectorAll('.node').forEach(k=>{k.classList.remove('hidden');});
    }});}
";

struct Item {
    level: usize,
    text: String,
    literal: bool,
}

fn parse(src: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let mut lit = false;
    let mut lit_level = 0;
    for raw in src.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let mut level = 0;
        let mut rest = line;
        loop {
            if let Some(r) = rest.strip_prefix('\t') {
                level += 1;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("   ") {
                level += 1; // the spec's examples indent with 3 spaces
                rest = r;
            } else {
                break;
            }
        }
        // A lone backslash toggles a literal block: verbatim until the
        // closing backslash at the same level. The delimiter lines stay
        // visible, one at the top and one at the bottom of the block.
        if rest == "\\" {
            if lit && level == lit_level {
                lit = false;
            } else if !lit {
                lit = true;
                lit_level = level;
            }
            out.push(Item { level, text: "\\".into(), literal: true });
            continue;
        }
        out.push(Item {
            level,
            text: rest.to_string(),
            literal: lit,
        });
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn upperish(c: char) -> bool {
    c.is_uppercase() || !c.is_ascii()
}

/// Head classification, ported from the hyperlist display hook, which in
/// turn follows hyperlist.vim: Starter/Identifier, then Operator (CAPS +
/// colon), then Property (word run + colon). Returns (html-so-far, rest).
fn head(text: &str) -> (String, String) {
    let mut out = String::new();
    let mut rest = text;

    // Starter: "- ", "+ ", "* " or a numbering like "1.2.3. "
    if let Some(r) = rest.strip_prefix("- ").or_else(|| rest.strip_prefix("+ "))
        .or_else(|| rest.strip_prefix("* "))
    {
        let _ = write!(out, "<span class=start>{} </span>", esc(&rest[..1]));
        rest = r;
    } else {
        // Identifier: digits and dots then a space — "1.2.3 " AND the
        // bare "2 " both count, per hyperlist.vim's HLident.
        let ident: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if !ident.is_empty()
            && ident.chars().any(|c| c.is_ascii_digit())
            && rest[ident.len()..].starts_with(' ')
        {
            let _ = write!(out, "<span class=start>{}</span>", esc(&ident));
            rest = &rest[ident.len()..];
        }
    }

    // State / Transition markers are Operators per the spec: blue.
    for m in ["S: ", "T: ", "| ", "/ "] {
        if let Some(r) = rest.strip_prefix(m) {
            let _ = write!(out, "<span class=op>{}</span>", esc(m));
            rest = r;
            break;
        }
    }

    // Checkbox qualifier at the head; clicking toggles it on the page.
    for (m, sym) in [("[_] ", "☐ "), ("[ ] ", "☐ "), ("[x] ", "☑ "),
                     ("[X] ", "☑ "), ("[O] ", "◔ ")] {
        if let Some(r) = rest.strip_prefix(m) {
            let _ = write!(out,
                "<span class=\"qual cb\" onclick=\"cb(event,this)\">{}</span>",
                sym);
            rest = r;
            break;
        }
    }

    // Operator: two or more of [A-Z_-() /] then ':' — else Property.
    let mut i = 0;
    let mut seen = 0;
    for c in rest.chars() {
        if upperish(c) && c.is_alphabetic() || "_-() /".contains(c) {
            if c.is_alphabetic() && !c.is_uppercase() {
                break;
            }
            seen += 1;
            i += c.len_utf8();
        } else {
            break;
        }
    }
    if seen >= 2 && rest[i..].starts_with(':') {
        out.push_str(&colon_head_html(&format!("{}:", &rest[..i]), "op"));
        return (out, rest[i + 1..].to_string());
    }
    let mut i = 0;
    let mut seen = 0;
    for c in rest.chars() {
        if c.is_alphanumeric() || ",._&?!%= -/+<>#'\"()*".contains(c) {
            if c == '<' || c == '"' {
                break; // a ref or quote is not a Property head
            }
            seen += 1;
            i += c.len_utf8();
        } else {
            break;
        }
    }
    if seen >= 2 && rest[i..].starts_with(':')
        && (rest[i + 1..].is_empty() || rest[i + 1..].starts_with(' '))
    {
        out.push_str(&colon_head_html(&format!("{}:", &rest[..i]), "prop"));
        return (out, rest[i + 1..].to_string());
    }
    (out, rest.to_string())
}

/// An Operator/Property head with any parenthesized comment inside kept
/// teal, per hyperlist.vim's `contains=HLcomment` on both rules.
fn colon_head_html(text: &str, cls: &str) -> String {
    let mut out = format!("<span class={}>", cls);
    let mut rest = text;
    while let Some(a) = rest.find('(') {
        let Some(b) = rest[a..].find(')') else { break };
        out.push_str(&esc(&rest[..a]));
        let _ = write!(out, "<span class=cmt>{}</span>",
                       esc(&rest[a..a + b + 1]));
        rest = &rest[a + b + 1..];
    }
    out.push_str(&esc(rest));
    out.push_str("</span>");
    out
}

/// An item's text reduced to its reference-matchable key: leading
/// Starters, State/Transition markers, checkboxes and Identifiers
/// stripped, lowercased.
fn match_key(s: &str) -> String {
    let mut t = s.trim_start();
    loop {
        let before = t;
        for p in ["- ", "+ ", "* ", "S: ", "T: ", "| ", "/ ",
                  "[_] ", "[ ] ", "[x] ", "[X] ", "[O] "] {
            if let Some(r) = t.strip_prefix(p) {
                t = r;
            }
        }
        let id_len = t.chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .count();
        if id_len > 0
            && t[..id_len].chars().any(|c| c.is_ascii_digit())
            && t[id_len..].starts_with(' ')
        {
            t = &t[id_len + 1..];
        }
        if t == before {
            break;
        }
    }
    t.to_lowercase()
}

/// A `<reference>` as HTML: URLs and file: refs open externally; an
/// item-path ref links to the first item containing its last segment
/// (never the referencing item itself); unresolved refs stay a span.
fn ref_html(inner: &str, self_ix: usize, texts: &[String]) -> String {
    if inner.starts_with("http://") || inner.starts_with("https://")
        || inner.starts_with("file:")
    {
        let href = inner.strip_prefix("file:").unwrap_or(inner);
        return format!("<a class=ref href=\"{}\">&lt;{}&gt;</a>",
                       esc(href), esc(inner));
    }
    let seg = inner.rsplit('/').next().unwrap_or(inner).trim().to_lowercase();
    if !seg.is_empty() {
        if let Some(ix) = texts.iter().enumerate().position(|(i, t)| {
            i != self_ix && t.starts_with(&seg)
        }) {
            return format!(
                "<a class=ref href=\"#i{ix}\" \
                 onclick=\"go('i{ix}');return false\">&lt;{}&gt;</a>",
                esc(inner));
        }
    }
    format!("<span class=ref>&lt;{}&gt;</span>", esc(inner))
}

/// Inline elements over the remainder: qualifiers, refs, comments,
/// quotes, hashtags, keywords, markup.
fn inline(s: &str, self_ix: usize, texts: &[String]) -> String {
    let mut out = String::new();
    let b: Vec<char> = s.chars().collect();
    let n = b.len();
    let mut i = 0;
    while i < n {
        let c = b[i];
        let pair = |open: char, close: char| -> Option<usize> {
            if c != open {
                return None;
            }
            b[i + 1..].iter().position(|x| *x == close).map(|j| i + 1 + j)
        };
        if let Some(j) = pair('[', ']') {
            let body: String = b[i..=j].iter().collect();
            let _ = write!(out, "<span class=qual>{}</span>", esc(&body));
            i = j + 1;
            continue;
        }
        if let Some(j) = pair('<', '>') {
            let body: String = b[i + 1..j].iter().collect();
            out.push_str(&ref_html(&body, self_ix, texts));
            i = j + 1;
            continue;
        }
        if let Some(j) = pair('(', ')') {
            let body: String = b[i..=j].iter().collect();
            let _ = write!(out, "<span class=cmt>{}</span>",
                           inline_min(&body, self_ix, texts));
            i = j + 1;
            continue;
        }
        if let Some(j) = pair('"', '"') {
            let body: String = b[i..=j].iter().collect();
            let _ = write!(out, "<span class=cmt>{}</span>",
                           inline_min(&body, self_ix, texts));
            i = j + 1;
            continue;
        }
        if c == '#' && i + 1 < n && (b[i + 1].is_alphanumeric() || b[i + 1] == '_') {
            let mut j = i + 1;
            while j < n && (b[j].is_alphanumeric() || "._-".contains(b[j])) {
                j += 1;
            }
            let body: String = b[i..j].iter().collect();
            let _ = write!(out, "<span class=tag>{}</span>", esc(&body));
            i = j;
            continue;
        }
        if c == ';' {
            let _ = write!(out, "<span class=qual>;</span>");
            i += 1;
            continue;
        }
        // Keywords and markup at a word boundary.
        let boundary = i == 0 || !b[i - 1].is_alphanumeric();
        if boundary {
            let restfrom: String = b[i..].iter().collect();
            let mut hit = false;
            for (kw, cls) in [("SKIP", "start"), ("END", "start"),
                              ("FIXME", "todo"), ("TODO", "todo")] {
                if restfrom.starts_with(kw)
                    && b.get(i + kw.len()).map_or(true, |c| !c.is_alphanumeric())
                {
                    let _ = write!(out, "<span class={}>{}</span>", cls, kw);
                    i += kw.len();
                    hit = true;
                    break;
                }
            }
            if hit {
                continue;
            }
            for (m, tag) in [('*', "b"), ('/', "i"), ('_', "u")] {
                if c == m && i + 1 < n && !b[i + 1].is_whitespace() {
                    if let Some(j) = b[i + 1..].iter().position(|x| *x == m) {
                        let j = i + 1 + j;
                        if !b[j - 1].is_whitespace() && j > i + 1 {
                            let body: String = b[i + 1..j].iter().collect();
                            let _ = write!(out, "<{}>{}</{}>", tag, esc(&body), tag);
                            i = j + 1;
                            hit = true;
                            break;
                        }
                    }
                }
            }
            if hit {
                continue;
            }
        }
        out.push_str(&esc(&c.to_string()));
        i += 1;
    }
    out
}

/// Comments and quotes stay one color; only refs light up inside them.
fn inline_min(s: &str, self_ix: usize, texts: &[String]) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('<') {
        if let Some(b) = rest[a..].find('>') {
            out.push_str(&esc(&rest[..a]));
            out.push_str(&ref_html(&rest[a + 1..a + b], self_ix, texts));
            rest = &rest[a + b + 1..];
        } else {
            break;
        }
    }
    out.push_str(&esc(rest));
    out
}

fn render_line(it: &Item, self_ix: usize, texts: &[String]) -> String {
    if it.literal {
        return format!("<span class=lit>{}</span>", esc(&it.text));
    }
    let (h, rest) = head(&it.text);
    format!("{}{}", h, inline(&rest, self_ix, texts))
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut title = String::new();
    if args.first().map(|a| a == "-h" || a == "--help").unwrap_or(false) {
        println!("hl2web — HyperList to one self-contained interactive HTML page");
        println!();
        println!("Usage: hl2web [--title T] file.hl > file.html");
        return;
    }
    if args.first().map(|a| a == "-v" || a == "--version").unwrap_or(false) {
        println!("hl2web {}", VERSION);
        return;
    }
    if args.first().map(|a| a == "--title").unwrap_or(false) {
        args.remove(0);
        if !args.is_empty() {
            title = args.remove(0);
        }
    }
    let Some(path) = args.first() else {
        eprintln!("usage: hl2web [--title T] file.hl > file.html");
        std::process::exit(2);
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hl2web: {}: {}", path, e);
            std::process::exit(1);
        }
    };
    let items = parse(&src);
    if title.is_empty() {
        title = std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "HyperList".into());
    }

    // Reference-resolution targets: item texts with Starters, checkboxes
    // and Identifiers stripped, lowercased. A reference matches the START
    // of an item, never text buried inside one.
    let texts: Vec<String> = items.iter().map(|it| match_key(&it.text)).collect();
    let mut body = String::new();
    let mut stack: Vec<usize> = Vec::new(); // open <details> levels
    for (i, it) in items.iter().enumerate() {
        let has_kids = items
            .get(i + 1)
            .map(|n| n.level > it.level)
            .unwrap_or(false);
        while let Some(&top) = stack.last() {
            if it.level <= top {
                body.push_str("</div></details>");
                stack.pop();
            } else {
                break;
            }
        }
        let line = render_line(it, i, &texts);
        let plain = it.text.replace('"', "&quot;");
        if has_kids {
            let _ = write!(
                body,
                "<details class=node open data-l={} data-t=\"{}\" id=i{}>\
                 <summary>{}</summary><div class=kids>",
                it.level, esc(&plain), i, line
            );
            stack.push(it.level);
        } else {
            let _ = write!(
                body,
                "<div class=\"item leaf node\" data-t=\"{}\" id=i{}>{}</div>",
                esc(&plain), i, line
            );
        }
    }
    for _ in stack {
        body.push_str("</div></details>");
    }

    println!(
        "<!doctype html><html><head><meta charset=utf-8>\
         <meta name=viewport content=\"width=device-width, initial-scale=1\">\
         <title>{t}</title><style>{css}</style></head><body>\
         <header><h1>{t}</h1>\
         <input id=q placeholder=search oninput=search(this.value)>\
         <button onclick=level(1)>1</button><button onclick=level(2)>2</button>\
         <button onclick=level(3)>3</button><button onclick=level(4)>4</button>\
         <button onclick=level(-1)>all</button><button onclick=level(0)>none</button>\
         <button onclick=theme() title=\"light / dark\">☾☀</button>\
         </header><div id=root>{body}</div>\
         <footer>rendered by hl2web {v} — <a class=ref \
         href=\"https://isene.org/hyperlist/\">HyperList</a></footer>\
         <script>{js}</script></body></html>",
        t = esc(&title), css = CSS, body = body, js = JS, v = VERSION
    );
}
