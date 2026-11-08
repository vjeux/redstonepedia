use anyhow::{Result, bail};
use mctas::vanilla::VanillaServer;

fn uuid(server: &mut VanillaServer, selector: &str, path: &str) -> Result<String> {
    let result = server.cmd(&format!("data get entity {selector} {path}"))?;
    let Some(array) = result.split("[I;").nth(1).and_then(|s| s.split(']').next()) else {
        bail!("missing UUID for {selector} {path}: {result}");
    };
    Ok(array.chars().filter(|c| !c.is_whitespace()).collect())
}

pub fn check(server: &mut VanillaServer, mixed: bool, leashed: bool) -> Result<String> {
    let a = "@e[tag=boat_a,limit=1]";
    let b = "@e[tag=boat_b,limit=1]";
    let ha = "@e[tag=holder_a,limit=1]";
    let hb = "@e[tag=holder_b,limit=1]";
    for (boat, passenger) in [(a, ha), (b, hb)] {
        if uuid(server, boat, "Passengers[0].UUID")? != uuid(server, passenger, "UUID")? {
            bail!("wrong passenger aboard {boat}");
        }
    }
    if leashed {
        for (child, holder) in [(a, if mixed { "@a[limit=1]" } else { hb }), (b, ha)] {
            if uuid(server, child, "leash.UUID")? != uuid(server, holder, "UUID")? {
                bail!("wrong leash edge {child} -> {holder}");
            }
        }
    } else {
        for boat in [a, b] {
            let result = server.cmd(&format!("data get entity {boat} leash"))?;
            if !result.contains("Found no elements") {
                bail!("expected absent lead: {result}");
            }
        }
    }
    Ok(format!(
        "correct passengers and graph; mixed={mixed} leashed={leashed}"
    ))
}
