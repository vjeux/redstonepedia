//! `mctas web single`: a generated page with its viewer as one self-contained HTML file (the viewer's
//! script, its WebAssembly, its textures, the design and terrain files it fetches, the page's style),
//! to share where only a single file goes (an artifact): every file the viewer fetches is served
//! from inside the page by a `fetch` shim, the viewer's modules loaded from blob URLs.

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The value of the first `key"..."` after `from` in `text` (`import init from "` -> the path).
fn quoted_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let i = text.find(key)? + key.len();
    let rest = &text[i..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// A page-relative URL without its query (`../viewer/x/mcview.js?v=1` -> `../viewer/x/mcview.js`).
fn no_query(u: &str) -> &str {
    u.split(['?', '#']).next().unwrap_or(u)
}

fn mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => "image/png",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "js" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// Every file under `dir` (recursively), as `(path relative to dir with '/', path)`.
fn files_under(dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                out.push((rel, p));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// A JavaScript string literal of `s` safe inside a `<script>` (JSON-escaped, `</` broken up).
fn js_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 16);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{2028}' => o.push_str("\\u2028"),
            '\u{2029}' => o.push_str("\\u2029"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o.replace("</", "<\\/")
}

/// The page `page` (a generated site page with a live viewer) as one file `out`.
pub fn single_page(page: &Path, out: &Path) -> Result<()> {
    let html = std::fs::read_to_string(page).with_context(|| format!("reading {}", page.display()))?;
    let dir = page.parent().unwrap_or(Path::new("."));
    // the viewer's module script: `import init from "<js>"; init({module_or_path: "<wasm>"});`
    let Some(s0) = html.find("<script type=\"module\">") else { bail!("{}: no viewer module script", page.display()) };
    let s1 = s0 + html[s0..].find("</script>").context("an unclosed module script")? + "</script>".len();
    let script = &html[s0..s1];
    let js_rel = no_query(quoted_after(script, "import init from \"").context("no `import init from` in the module script")?).to_string();
    let wasm_rel = no_query(quoted_after(script, "module_or_path: \"").context("no `module_or_path` in the module script")?).to_string();
    let js_path = dir.join(&js_rel);
    let js = std::fs::read_to_string(&js_path).with_context(|| format!("reading {}", js_path.display()))?;
    // its own modules (`from "./mcview-inline0.js"`), each loaded from a blob URL
    let mut modules: Vec<(String, String)> = Vec::new();
    let mut rest = js.as_str();
    while let Some(i) = rest.find("from \"./") {
        let after = &rest[i + "from \"".len()..];
        let end = after.find('"').context("an unclosed import")?;
        let spec = &after[..end];
        if !modules.iter().any(|(s, _)| s == spec) {
            let p = js_path.parent().unwrap().join(spec);
            modules.push((spec.to_string(), std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?));
        }
        rest = &after[end..];
    }
    // the files the viewer fetches: the wasm, and every page-relative file or directory a
    // `data-*` attribute names (the design, the terrain, the textures' directory)
    let mut files: BTreeMap<String, PathBuf> = BTreeMap::new();
    files.insert(wasm_rel.clone(), dir.join(&wasm_rel));
    let mut rest = html.as_str();
    while let Some(i) = rest.find(" data-") {
        let after = &rest[i + 1..];
        let Some(eq) = after.find("=\"") else { break };
        let name_ok = after[..eq].chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let val_start = eq + 2;
        let Some(val_len) = after[val_start..].find('"') else { break };
        let val = no_query(&after[val_start..val_start + val_len]);
        if name_ok && (val.starts_with("../") || val.starts_with("./")) {
            let p = dir.join(val);
            if p.is_dir() {
                for (r, fp) in files_under(&p)? {
                    files.insert(format!("{}/{}", val.trim_end_matches('/'), r), fp);
                }
            } else if p.is_file() {
                files.insert(val.to_string(), p);
            }
        }
        rest = &after[val_start + val_len..];
    }
    // the page's style sheets, inlined
    let mut body = html.clone();
    while let Some(i) = body.find("<link rel=\"stylesheet\" href=\"") {
        let end = i + body[i..].find('>').context("an unclosed link")? + 1;
        let href = no_query(quoted_after(&body[i..end], "href=\"").unwrap_or("")).to_string();
        let css = std::fs::read_to_string(dir.join(&href)).unwrap_or_default();
        body.replace_range(i..end, &format!("<style>\n{}\n</style>", css.replace("</", "<\\/")));
    }
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut table = String::from("{");
    let mut total = 0usize;
    for (rel, p) in &files {
        let bytes = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
        total += bytes.len();
        table.push_str(&format!("{}:[{},\"{}\"],\n", js_string(rel), js_string(mime(p)), b64.encode(&bytes)));
    }
    table.push('}');
    let shim = format!(
        "<script>\n(function(){{\nconst F={table};\nconst map=new Map();\nfor(const k in F){{const u=new URL(k,document.baseURI);u.search='';map.set(u.href,F[k]);}}\nconst bytes=b=>{{const s=atob(b);const a=new Uint8Array(s.length);for(let i=0;i<s.length;i++)a[i]=s.charCodeAt(i);return a;}};\nconst real=window.fetch.bind(window);\nwindow.fetch=function(input,init){{const url=typeof input==='string'?input:(input&&input.url)||String(input);let u;try{{u=new URL(url,document.baseURI);u.search='';u.hash='';}}catch(e){{return real(input,init);}}const hit=map.get(u.href);if(hit)return Promise.resolve(new Response(bytes(hit[1]),{{status:200,headers:{{'Content-Type':hit[0]}}}}));return real(input,init);}};\n}})();\n</script>\n"
    );
    let mut boot = String::from("<script type=\"module\">\n");
    let mut main_src = js.clone();
    for (k, (spec, src)) in modules.iter().enumerate() {
        boot.push_str(&format!("const m{k}=URL.createObjectURL(new Blob([{}],{{type:'text/javascript'}}));\n", js_string(src)));
        main_src = main_src.replace(&format!("\"{spec}\""), &format!("\"__MODULE_{k}__\""));
    }
    boot.push_str(&format!("let main={};\n", js_string(&main_src)));
    for k in 0..modules.len() {
        boot.push_str(&format!("main=main.split('__MODULE_{k}__').join(m{k});\n"));
    }
    boot.push_str(&format!(
        "const v=await import(URL.createObjectURL(new Blob([main],{{type:'text/javascript'}})));\nawait v.default({{module_or_path:{}}});\n</script>",
        js_string(&wasm_rel)
    ));
    let s0 = body.find("<script type=\"module\">").unwrap();
    let s1 = s0 + body[s0..].find("</script>").unwrap() + "</script>".len();
    body.replace_range(s0..s1, &boot);
    let head = body.find("<head>").map(|i| i + "<head>".len()).unwrap_or(0);
    body.insert_str(head, &shim);
    std::fs::write(out, &body).with_context(|| format!("writing {}", out.display()))?;
    println!("{} -> {} ({} files inlined, {:.1} MB of them; the page {:.1} MB)", page.display(), out.display(), files.len() + modules.len() + 1, total as f64 / 1e6, body.len() as f64 / 1e6);
    Ok(())
}
