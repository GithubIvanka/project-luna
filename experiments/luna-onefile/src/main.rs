use std::process::Command;
use std::time::Duration;

fn main() -> ! {
    println!("LUNA-INIT: PID 1");
    println!("LUNA-INIT: Stage 0 one-file experiment");
    println!("LUNA-INIT: Linux kernel + embedded initramfs");

    let result = Command::new("/bin/sh")
        .arg("-c")
        .arg("echo LUNA-READY; while true; do sleep 3600; done")
        .status();

    match result {
        Ok(status) => println!("LUNA-INIT: bootstrap shell exited: {status}"),
        Err(error) => println!("LUNA-INIT: failed to start bootstrap shell: {error}"),
    }

    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
