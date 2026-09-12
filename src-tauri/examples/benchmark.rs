use std::{path::Path, sync::atomic::AtomicBool};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("사용법: benchmark ZIP파일 출력폴더");
        std::process::exit(2);
    }
    match dalzip_lib::archive::extract(
        Path::new(&args[1]),
        Path::new(&args[2]),
        None,
        20 * 1024 * 1024 * 1024,
        &AtomicBool::new(false),
        &|_| {},
    ) {
        Ok(out) => println!(
            "{}\t{}\t{}\t{}",
            out.path, out.bytes, out.seconds, out.files
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
