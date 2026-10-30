//! `--mouse-test X,Y`: move the pointer and report where it actually is over
//! the following second.  Used to see whether the client warps the cursor.

#[cfg(not(windows))]
pub fn run(_args: &[String]) -> i32 {
    eprintln!("--mouse-test needs the Windows backend");
    2
}

#[cfg(windows)]
pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform::traits::Mouse;
    use dyetrace_platform_win::WinMouse;
    use std::thread::sleep;
    use std::time::Duration;

    let target = match args.first().and_then(|a| a.split_once(',')) {
        Some((x, y)) => match (x.trim().parse::<i32>(), y.trim().parse::<i32>()) {
            (Ok(x), Ok(y)) => (x, y),
            _ => {
                eprintln!("usage: dyetrace --mouse-test X,Y");
                return 2;
            }
        },
        None => {
            eprintln!("usage: dyetrace --mouse-test X,Y");
            return 2;
        }
    };

    let mouse = WinMouse::new();
    println!("before      {:?}", mouse.position());
    let arrived = mouse.move_to(target.0, target.1);
    println!("move_to     {arrived} -> {:?}", mouse.position());

    for ms in [100u64, 300, 600, 1000] {
        sleep(Duration::from_millis(if ms == 100 { 100 } else { 200 }));
        println!("after {ms:>4} ms  {:?}", mouse.position());
    }

    0
}
