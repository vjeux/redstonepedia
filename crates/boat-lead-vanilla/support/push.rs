use std::process::Command;

fn base64(bytes:&[u8])->String {
    let table=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out=String::new();
    for chunk in bytes.chunks(3) {
        let a=chunk[0] as usize;
        let b=chunk.get(1).copied().unwrap_or(0) as usize;
        let c=chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(table[a>>2] as char);
        out.push(table[((a&3)<<4)|(b>>4)] as char);
        out.push(if chunk.len()>1 {table[((b&15)<<2)|(c>>6)] as char}else{'='});
        out.push(if chunk.len()>2 {table[c&63] as char}else{'='});
    }
    out
}
fn main()->Result<(),Box<dyn std::error::Error>> {
    assert_eq!(base64(b"user:token"),"dXNlcjp0b2tlbg==");
    let head=std::env::args().nth(1).ok_or("explicit coordinator-approved hash required")?;
    if head.len()!=40 || !head.chars().all(|c|c.is_ascii_hexdigit()){return Err("full hash required".into());}
    let token=Command::new("gh").args(["auth","token"]).output()?;
    if !token.status.success(){return Err("gh auth token failed".into());}
    let auth=base64(format!("vjeux:{}",std::str::from_utf8(&token.stdout)?.trim()).as_bytes());
    let status=Command::new("git").env("GIT_CONFIG_COUNT","1")
        .env("GIT_CONFIG_KEY_0","http.extraHeader")
        .env("GIT_CONFIG_VALUE_0",format!("Authorization: Basic {auth}"))
        .args(["push","origin",&format!("{head}:refs/heads/explore/boat-lead-vanilla")]).status()?;
    if !status.success(){return Err(format!("git push failed {status}").into());}
    Ok(())
}
