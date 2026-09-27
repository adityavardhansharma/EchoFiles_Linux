//! `cargo run -p echofiles-disks --example agent_probe` — asks to mount the first unmounted
//! Windows volume and cancels at the password prompt: proves prompts reach EchoFiles.

fn main() {
    ef_disks::polkit::ensure(|p| {
        println!("prompt for {}: {} (retry: {})", p.user, p.message, p.retry);
        p.answer(None);
    })
    .expect("register agent");
    let vols = ef_disks::windows_volumes().expect("volumes");
    let Some(v) = vols.iter().find(|v| !v.is_mounted() && !v.locked) else { return println!("nothing to mount") };
    println!("mounting {} …", v.device);
    println!("{:?}", ef_disks::mount(v, false));
}
