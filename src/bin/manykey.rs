use manykey::{BackendKind, Event, EventOrigin, KeyState, Listener, Options};
use std::time::{Duration, Instant};

fn usage() -> ! {
    eprintln!(
        "usage: manykey <list|watch> [--include-injected] [--backend NAME]\n       manykey --version\n  list   show connected keyboards\n  watch  print keyboards and key events as they happen\n\n  --include-injected  also report software-injected input (Windows)\n  --backend NAME      auto (default), raw-input (Windows) or evdev (Linux)"
    );
    std::process::exit(2)
}

fn id_str(vid: Option<u16>, pid: Option<u16>) -> String {
    match (vid, pid) {
        (Some(v), Some(p)) => format!("{v:04x}:{p:04x}"),
        _ => "----:----".into(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.first().map(String::as_str), Some("--version" | "-V")) {
        println!("manykey {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // Check the command before opening any devices, so a typo or --help gets
    // the usage text rather than (on Linux without permission) an access error.
    let cmd = args.first().cloned().filter(|c| c == "list" || c == "watch").unwrap_or_else(|| usage());
    let mut options = Options::default();
    let mut rest = args[1..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--include-injected" => options.include_injected = true,
            "--backend" => {
                options.backend = rest.next().and_then(|n| BackendKind::from_name(n)).unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
    }
    let listener = match Listener::with_options(options) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("manykey: {e}");
            std::process::exit(1);
        }
    };
    match cmd.as_str() {
        "list" => {
            // Backends announce existing keyboards at startup; collect briefly.
            let deadline = Instant::now() + Duration::from_millis(750);
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                if let Some(Event::KeyboardAdded(k)) = listener.recv_timeout(left) {
                    println!("{:>3}  {}  {}  ({})", k.id.0, id_str(k.vendor_id, k.product_id), k.name, k.path);
                }
            }
        }
        "watch" => {
            for event in listener.iter() {
                match event {
                    Event::KeyboardAdded(k) => {
                        println!("+ keyboard {} {} {}", k.id.0, id_str(k.vendor_id, k.product_id), k.name)
                    }
                    Event::KeyboardRemoved(id) => println!("- keyboard {}", id.0),
                    Event::Overflow { dropped } => println!("! overflow ({dropped} events dropped so far)"),
                    Event::Key(k) => {
                        let s = match k.state {
                            KeyState::Pressed => "down",
                            KeyState::Released => "up",
                            KeyState::Repeat => "repeat",
                            _ => "?", // a state added after this tool was written
                        };
                        let note = if k.origin == EventOrigin::Synthesized { "  [synthesized]" } else { "" };
                        println!(
                            "keyboard {}  {:<12} {:<6} (native {:#06x}){note}",
                            k.keyboard.0,
                            k.key.name(),
                            s,
                            k.native_code
                        );
                    }
                    _ => {} // an event kind added after this tool was written
                }
            }
        }
        _ => usage(),
    }
}
