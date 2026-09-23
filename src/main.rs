use std::{env, fs, path::PathBuf, process};

fn main() {
    process::exit(run(env::args().skip(1).collect()));
}

fn run(args: Vec<String>) -> i32 {
    let [command, entry, flag, output] = args.as_slice() else {
        eprintln!("usage: wira build <entry-file> --output <file.pdf>");
        return 2;
    };
    if command != "build" || flag != "--output" || !output.ends_with(".pdf") {
        eprintln!("usage: wira build <entry-file> --output <file.pdf>");
        return 2;
    }
    let entry = PathBuf::from(entry);
    let output = PathBuf::from(output);
    if let (Ok(a), Ok(b)) = (fs::canonicalize(&entry), fs::canonicalize(&output)) {
        if a == b {
            eprintln!("error: output must not overwrite source file");
            return 2;
        }
    }
    let (project, bytes) = match wira::build(&entry) {
        Ok(v) => v,
        Err(error) => {
            eprintln!("error: {error}");
            return 1;
        }
    };
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("error: {}: {error}", parent.display());
            return 1;
        }
    }
    if let Err(error) = fs::write(&output, bytes) {
        eprintln!("error: {}: {error}", output.display());
        return 1;
    }
    println!(
        "{}: {} pages -> {}",
        project.name,
        project.pages.len(),
        output.display()
    );
    0
}
