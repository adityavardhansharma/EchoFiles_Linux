//! `cargo run -p echofiles-net --example netprobe -- <address> [user] [password]`
//! Connects like EchoFiles does, answering questions from the arguments, then lists the
//! folder and every network mount. `--unmount` disconnects afterwards. With `scan`, lists
//! servers nearby instead.

use ef_net::{gvfs, Address, Ask, Login, Protocol, Remember};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("scan") {
        println!("{:#?}", ef_net::discover::scan(std::time::Duration::from_secs(4)));
        return;
    }
    let unmount = args.iter().any(|a| a == "--unmount");
    let args: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let addr = Address::parse(args[0], Protocol::Smb).expect("address");
    let user = args.get(1).map(|s| s.to_string()).unwrap_or_default();
    let password = args.get(2).map(|s| s.to_string()).unwrap_or_default();
    println!("uri: {}  name: {}", addr.uri(), addr.display_name());
    let t = std::time::Instant::now();
    let answer = move |ask| match ask {
        Ask::Login(l) => {
            println!("login asked: {:?} user? {} domain? {} anon? {} save? {} retry {}", l.message, l.need_user, l.need_domain, l.can_anonymous, l.can_remember, l.retry);
            if l.retry {
                l.answer(None);
            } else {
                l.answer(Some(Login { user: user.clone(), domain: l.default_domain.clone(), password: password.clone(), anonymous: user.is_empty(), remember: Remember::Never }));
            }
        }
        Ask::Question(q) => {
            println!("question: {:?} {:?} → 0", q.message, q.choices);
            q.answer(Some(0));
        }
        Ask::Withdrawn(op) => println!("withdrawn {op}"),
    };
    if addr.protocol == Protocol::Smb && addr.share.is_none() {
        println!("shares: {:?} in {:?}", gvfs::shares(&addr, gvfs::next_op(), answer), t.elapsed());
        return;
    }
    let r = gvfs::mount(&addr, gvfs::next_op(), answer);
    println!("mount took {:?}: {:?}", t.elapsed(), r);
    match r {
        Ok(m) => {
            let dir = m.path_for(&addr);
            let t = std::time::Instant::now();
            let n = std::fs::read_dir(&dir).map(|d| d.count());
            println!("{} → {:?} entries in {:?}", dir.display(), n, t.elapsed());
            for m in gvfs::mounts().unwrap() {
                println!("  mounted: {} {} {:?} landing {:?}", m.kind, m.name, m.address.as_ref().map(|a| a.uri()), m.landing);
            }
            if unmount {
                println!("unmount: {:?}", gvfs::unmount(&m, false));
            }
        }
        Err(e) => println!("explained: {}", gvfs::explain(&e, &addr)),
    }
}
