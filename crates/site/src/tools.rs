//! The Tools section (docs/site.md#tools): calculators for the parts of the game that only look
//! random, each a page whose text and empty forms are written here and whose buttons the
//! browser bundle answers (web/tools, Rust compiled to WebAssembly on crates/rng): the enchanting
//! table calculator and the librarian calculator. `tools/index.html` lists them; the header's
//! Tools tab opens it.

use crate::pages::{self, SITE_NAME, Scripts};
use site_model::Site;
use site_model::html::esc;

/// The Tools section's pages: (path, title, one line for its card).
pub const PAGES: [(&str, &str, &str); 2] = [
    ("tools/enchanting.html", "Enchanting table calculator", "Choose the enchantments you get: read the table a few times, drop the number of items it says, then enchant."),
    ("tools/librarian.html", "Librarian calculator", "Find how many times to break and place the lectern before a librarian sells the book you want."),
];

/// The tools' overview, tools/index.html.
pub fn tools_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let tiles: Vec<String> = PAGES
        .iter()
        .map(|(path, title, line)| format!("<a class=\"card tool\" href=\"{prefix}{path}\"><div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{}</div></div></a>", esc(title), esc(line)))
        .collect();
    let body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Tools</p>\n<h1>Tools</h1>\n<p class=\"lede\">Calculators for the parts of the game that only look random. For Minecraft Java 26.3; they run in your browser.</p>\n{}",
        pages::grid(&tiles)
    );
    pages::shell_full("Tools", "Minecraft calculators: choose your enchantments at the enchanting table, and find the librarian book you want.", prefix, &body, Scripts { tools: false, ..scripts }, site, None, Some("tools"))
}

fn crumbs(prefix: &str, here: &str) -> String {
    format!("<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}tools/index.html\">Tools</a> › {here}</p>\n")
}

/// tools/enchanting.html. `worker`: the URL of the seed search's worker script, from the page
/// (`pkg/worker.js?v=<hash>`).
pub fn enchanting_page(site: &Site, scripts: Scripts, worker: &str) -> String {
    let prefix = "../";
    let data = rng::Data::load();
    let sword = data.item("diamond_sword");
    let items: String = (0..data.items.len())
        .filter(|&i| data.items[i].is_enchantable)
        .map(|i| format!("<option value=\"{}\"{}>{}</option>", esc(&data.items[i].id), if Some(i) == sword { " selected" } else { "" }, esc(&data.items[i].name.replace('_', " "))))
        .collect();
    let body = format!(
        "{crumbs}<h1>Enchanting table calculator</h1>\n\
<p class=\"lede\">The enchanting table is not random. What it offers comes from a hidden number of yours, the enchanting seed, which changes only when you enchant. Read the table a few times and the calculator finds that number; then it tells you how many items to drop so that your next enchant gives exactly what you picked.</p>\n\
<div id=\"calc-enchant\" class=\"calc\" data-worker=\"{worker}\">\n\
<section class=\"step\"><h2>Before you start</h2><p>Enchant any cheap item once (a book in the top slot costs 1 level and 1 lapis). From then until you are done, do not sprint, take damage, eat or drink, swim, or leave the world, and keep parrots off your shoulder: those use the same hidden numbers (so does experience mending a damaged item). Walking, jumping, opening the table and moving items are fine.</p></section>\n\
<section class=\"step\"><h2>1. Find your enchanting seed</h2><p>Put an item you can enchant in the table and write down the level each of the three slots asks for. Then change how many bookshelves the table sees (a torch or a carpet between a shelf and the table hides it) and write them down again. Low bookshelf counts tell the most; with 15 the bottom slot always asks for 30. It usually takes about seven readings; with the hint you see when you point at a slot, three or four.</p>\n\
<div id=\"obs1\" class=\"readings\"></div>\n\
<div class=\"row\"><button type=\"button\" class=\"small\" data-act=\"add-obs\" data-target=\"obs1\">+ add a reading</button> <button type=\"button\" class=\"act\" id=\"crack1\" data-act=\"crack1\">Find the seed</button> <progress id=\"prog1\" max=\"1\" value=\"0\" hidden></progress></div>\n\
<div class=\"row\"><label>or type it: <input class=\"hex\" id=\"xp1\" placeholder=\"5F94CCE0\" spellcheck=\"false\" autocomplete=\"off\"></label></div>\n\
<div class=\"out\" id=\"out1\" hidden></div></section>\n\
<section class=\"step\"><h2>2. Enchant something cheap, then find the next seed</h2><p>Enchant any cheap item in the top slot, then read the table again the same way. Only 65,536 seeds can come next, so three or four readings are usually enough (one or two with the hints).</p>\n\
<div id=\"obs2\" class=\"readings\"></div>\n\
<div class=\"row\"><button type=\"button\" class=\"small\" data-act=\"add-obs\" data-target=\"obs2\">+ add a reading</button> <button type=\"button\" class=\"act\" id=\"crack2\" data-act=\"crack2\">Find the next seed and the player seed</button> <progress id=\"prog2\" max=\"1\" value=\"0\" hidden></progress></div>\n\
<div class=\"row\"><label>or type it: <input class=\"hex\" id=\"xp2\" placeholder=\"BE8A2899\" spellcheck=\"false\" autocomplete=\"off\"></label></div>\n\
<div class=\"row\"><label>Player seed: <input class=\"hex wide\" id=\"pseed\" placeholder=\"12 hex digits\" spellcheck=\"false\" autocomplete=\"off\"></label></div>\n\
<div class=\"out\" id=\"out2\" hidden></div></section>\n\
<section class=\"step\"><h2>3. Pick the enchantments</h2>\n\
<div class=\"row\"><label>Item <select id=\"titem\">{items}</select></label> <label>Most bookshelves <input class=\"num\" id=\"tmaxsh\" inputmode=\"numeric\" value=\"15\"></label> <label>Your level <input class=\"num\" id=\"tlevel\" inputmode=\"numeric\" value=\"30\"></label> <label>Most drops <input class=\"num wide\" id=\"tmaxdrops\" inputmode=\"numeric\" value=\"2048\"></label></div>\n\
<p class=\"hint\">For each enchantment: × it must not have it, – either way, or the lowest level you want.</p>\n\
<div class=\"levels\" id=\"tlevels\"></div>\n\
<div class=\"row\"><button type=\"button\" class=\"act\" data-act=\"calc\">Calculate</button></div>\n\
<div class=\"out\" id=\"out3\" hidden></div>\n\
<div class=\"row\" id=\"donerow\" hidden><button type=\"button\" class=\"small\" data-act=\"done\">I did it: update the player seed for the next item</button></div></section>\n\
</div>\n\
<section class=\"how\"><h2>How it works</h2><p>The table seeds its own random numbers with your enchanting seed, so the same seed, item and bookshelf count always show the same offers. The seed changes only when you enchant: the game then draws the next number from your player's own random generator. Two seeds in a row give that generator away, so every later seed is known. Each press of Q draws four numbers from the same generator, so the number of items you drop picks which seed your next enchant draws: drop them, enchant anything cheap (that draws the seed the calculator picked), set the bookshelves, and enchant.</p><p>Checked against the real 26.3 game: every offer of the table for 200 seeds, every enchantable item and every bookshelf count, and the whole method, item in hand, from reading the table to the final enchant. The trick was first found by Earthcomputer.</p></section>\n",
        crumbs = crumbs(prefix, "Enchanting table")
    );
    pages::shell_full(
        "Enchanting table calculator",
        "Choose the enchantments you get from the enchanting table in Minecraft Java 26.3: find your enchanting seed from what the table shows, then drop the number of items the calculator says.",
        prefix,
        &body,
        scripts,
        site,
        None,
        Some("tools"),
    )
}

/// tools/librarian.html.
pub fn librarian_page(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let data = rng::Data::load();
    let mending = data.enchant("mending");
    let books: String = data
        .tags
        .get("minecraft:tradeable")
        .into_iter()
        .flatten()
        .map(|&e| format!("<option value=\"{}\"{}>{}</option>", esc(&data.enchants[e as usize].id), if Some(e) == mending { " selected" } else { "" }, esc(&data.enchants[e as usize].name.replace('_', " "))))
        .collect();
    let body = format!(
        "{crumbs}<h1>Librarian calculator</h1>\n\
<p class=\"lede\">The books librarians sell are not up to each villager. The game draws every librarian's trades from one list per villager level, set by the world seed and shared by the whole world: the 10th time any librarian gets its first trades, it gets the 10th entry of the list. Breaking and placing the lectern again just moves on to the next entry, so once you know where your world is in the list, you know how many times to reroll for the book you want.</p>\n\
<div id=\"calc-librarian\" class=\"calc\">\n\
<section class=\"step\"><h2>Your world</h2><div class=\"row\"><label>World seed <input id=\"wseed\" class=\"seed\" inputmode=\"numeric\" placeholder=\"type /seed in the chat\" spellcheck=\"false\" autocomplete=\"off\"></label> <label>Villager level <select id=\"vlevel\"><option value=\"1\">1 (novice)</option><option value=\"2\">2 (apprentice)</option><option value=\"3\">3 (journeyman)</option><option value=\"4\">4 (expert)</option><option value=\"5\">5 (master)</option></select></label></div><p class=\"hint\">In single player /seed works even without cheats; on a server, ask an operator. A villager's first trades are level 1 (novice).</p></section>\n\
<section class=\"step\"><h2>1. Find where your world is</h2><p>Reroll your librarian a few times: break the lectern, place it back and open the trade screen. Write down each screen in order: its enchanted book, the book's level and its price in emeralds, or \"no book\". Two or three screens with a book are usually enough (a screen without one tells less). In a new world, the first librarian that ever gets its trades shows entry 0.</p>\n\
<div id=\"seen\" class=\"readings\"></div>\n\
<div class=\"row\"><button type=\"button\" class=\"small\" data-act=\"add-seen\">+ add a screen</button> <button type=\"button\" class=\"act\" data-act=\"vsync\">Find my place</button> <button type=\"button\" class=\"small\" data-act=\"vlist\">Show the first 20 entries</button></div>\n\
<div class=\"out\" id=\"vout1\" hidden></div></section>\n\
<section class=\"step\"><h2>2. How many rerolls to the book you want</h2>\n\
<div class=\"row\"><label>The last screen you saw is entry <input class=\"num wide\" id=\"vafter\" inputmode=\"numeric\"></label></div>\n\
<div class=\"row\"><label>Book <select id=\"vwant\">{books}</select></label> <label>level at least <input class=\"num\" id=\"vminlvl\" inputmode=\"numeric\" value=\"1\"></label> <label>price at most <input class=\"num\" id=\"vmaxprice\" inputmode=\"numeric\" placeholder=\"any\"></label> <button type=\"button\" class=\"act\" data-act=\"vplan\">Calculate</button></div>\n\
<div class=\"out\" id=\"vout2\" hidden></div></section>\n\
</div>\n\
<section class=\"how\"><h2>How it works</h2><p>Every list of villager trades in the game names its own random sequence (a librarian's first trades: <code>minecraft:trade_set/librarian/level_1</code>). The game keeps one random generator per sequence for the whole world, seeded from the world seed and the sequence's name, and draws those trades from it; the villager's own random numbers never come into it. So the calculator lists the entries from the world seed alone.</p><p>Any librarian that gets new trades uses up an entry: a librarian working at its lectern gets its trades even if nobody looks, and so do the villagers of a village nearby. Open the trade screen after every reroll so each one counts once, and if a screen stops matching, find your place again from two screens. Prices change with your reputation (curing a zombie villager or the Hero of the Village effect lowers them): if yours differ, leave the price empty.</p><p>Checked against the real 26.3 game: 300 rerolls in a row in two worlds and a villager rerolling by itself with a lectern, every trade, book and price the same.</p></section>\n",
        crumbs = crumbs(prefix, "Librarian")
    );
    pages::shell_full(
        "Librarian calculator",
        "Find how many lectern rerolls until a librarian sells the enchanted book you want, from your world seed, in Minecraft Java 26.3.",
        prefix,
        &body,
        scripts,
        site,
        None,
        Some("tools"),
    )
}
