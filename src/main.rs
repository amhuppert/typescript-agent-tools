use std::{
    env, fs,
    io::{self, BufWriter, Read, Write},
    process::ExitCode,
};
use typescript_parser::{ParseOptions, parse_with_options};

fn run() -> Result<(), (u8, String)> {
    let mut pretty = false;
    let mut jsx = false;
    let mut check = false;
    let mut file = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                let mut stdout = io::stdout().lock();
                return writeln!(
                    stdout,
                    "tsparse [--pretty] [--tsx] [--check] [-o OUTPUT] [FILE|-]\n\nParse TypeScript into a compiler-style JSON AST. Reads stdin when FILE is omitted.\n--check validates syntax without producing JSON.\nExit codes: 0 success, 1 syntax error, 2 usage or I/O error."
                )
                .and_then(|_| stdout.flush())
                .map_err(|e| (2, e.to_string()));
            }
            "-V" | "--version" => {
                let mut stdout = io::stdout().lock();
                return writeln!(stdout, "tsparse {}", env!("CARGO_PKG_VERSION"))
                    .and_then(|_| stdout.flush())
                    .map_err(|e| (2, e.to_string()));
            }
            "--pretty" => pretty = true,
            "--tsx" => jsx = true,
            "--check" => check = true,
            "-o" | "--output" => {
                output = Some(args.next().ok_or((2, "Missing output path".into()))?)
            }
            "-" => {
                if file.replace(arg).is_some() {
                    return Err((2, "Expected one input file".into()));
                }
            }
            _ if arg.starts_with('-') => return Err((2, format!("Unknown option: {arg}"))),
            _ => {
                if file.replace(arg).is_some() {
                    return Err((2, "Expected one input file".into()));
                }
            }
        }
    }
    let name = file.as_deref().unwrap_or("-");
    jsx |= name.ends_with(".tsx") || name.ends_with(".jsx");
    let source = if name == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| (2, e.to_string()))?;
        source
    } else {
        fs::read_to_string(name).map_err(|e| (2, format!("{name}: {e}")))?
    };
    let ast = parse_with_options(
        &source,
        ParseOptions {
            jsx,
            file_name: if name == "-" { "source.ts" } else { name }.into(),
            ..Default::default()
        },
    )
    .map_err(|e| {
        let (line, column) = e.line_column(&source);
        (1, format!("{name}:{line}:{column}: {}", e.message))
    })?;
    if check {
        return Ok(());
    }
    let writer: Box<dyn Write> = match output {
        Some(path) => Box::new(fs::File::create(&path).map_err(|e| (2, format!("{path}: {e}")))?),
        None => Box::new(io::stdout().lock()),
    };
    let mut writer = BufWriter::new(writer);
    ast.write_json(&mut writer, pretty)
        .map_err(|e| (2, e.to_string()))?;
    writeln!(writer)
        .and_then(|_| writer.flush())
        .map_err(|e| (2, e.to_string()))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
