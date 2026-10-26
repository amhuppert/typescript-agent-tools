use std::{
    env, fs,
    io::{self, Read},
    process::ExitCode,
};
use typescript_parser::{ParseOptions, parse_with_options};

mod cli;
mod process_limits;
mod service;

use cli::Failure;
use cli::delivery::{self, Delivered, Format, Route, write_output};
use cli::failure::Code;
use cli::registry;
use cli::request::{self, Request};

fn run() -> Result<(), Delivered> {
    let request = request::parse(env::args_os().skip(1))
        .map_err(|invalid| delivery::fail(invalid.route, invalid.failure))?;
    match request {
        Request::Help(help) => cli::help::run(&help),
        Request::Version => {
            write_output(None, |writer| writer.write_all(cli::version().as_bytes()))
        }
        Request::Parse(request) => parse(request),
        Request::Service(request) => service_action(request),
        Request::Doctor(request) => cli::doctor::run(&request),
        Request::Query(query) => cli::query::run(query),
        Request::Edit(command) => cli::editing::run(command),
    }
}

/// Parse mode, which has no document: its failures are stderr's. With
/// `--check` its result is one verdict line on stdout, naming the input as
/// its diagnostics do and whether JSX was parsed; it says what was checked,
/// syntax, and never implies type checking (design, step 5, "Clean
/// verdicts"). It writes no AST, so `--output` and `--pretty` change
/// nothing then.
fn parse(request: request::Parse) -> Result<(), Delivered> {
    let name = request.file.as_deref().unwrap_or("-");
    let file = ParseOptions::for_file(if name == "-" { "source.ts" } else { name });
    let unread = |message: String| delivery::fail(Route::TEXT, Failure::new(Code::Read, message));
    let source = if name == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| unread(e.to_string()))?;
        source
    } else {
        fs::read_to_string(name).map_err(|e| unread(format!("{name}: {e}")))?
    };
    let jsx = request.jsx || file.jsx;
    let ast = parse_with_options(&source, ParseOptions { jsx, ..file }).map_err(|e| {
        let (line, column) = e.line_column(&source);
        delivery::diagnostic(
            delivery::SYNTAX_ERROR,
            &format!("{name}:{line}:{column}: {}", e.message),
        )
    })?;
    if request.check {
        let jsx = if jsx { "on" } else { "off" };
        return write_output(None, |writer| {
            write!(writer, "ok: {name} parses as TypeScript 7 (jsx: {jsx})")
        });
    }
    write_output(request.output.as_deref(), |writer| {
        ast.write_json(writer, request.pretty)
    })
}

/// A service action. Status is text, or with `--json` a document, as are
/// its failures; `--pretty` alone keeps its indented document. Stop prints
/// its verdict on stdout. Stop and serve have no document: their failures
/// are stderr's, and `--pretty` and `--output` change nothing.
fn service_action(request: request::Service) -> Result<(), Delivered> {
    let failed = |failure| delivery::fail(Route::TEXT, failure);
    match request.action {
        request::ServiceAction::Status => {
            let route = Route {
                format: request.format,
                pretty: request.pretty,
                output: request.output.as_deref(),
                new_file: false,
                operation: Some(registry::OperationId::ReferencesStatus.operation().name),
                fields: &[],
            };
            let status = match service::status(&request.project) {
                Ok(status) => status,
                Err(failure) => return Err(delivery::fail(route, failure)),
            };
            let status = &status;
            let document = request.format == Format::Json || request.pretty;
            delivery::deliver(
                route,
                Ok(delivery::Delivery {
                    code: 0,
                    document: Box::new(|writer, pretty| {
                        delivery::write_document(writer, &status.document(), pretty)
                    }),
                    text: (!document).then(|| {
                        Box::new(|| status.text(&request.project)) as Box<dyn FnOnce() -> String>
                    }),
                    text_is_result: true,
                }),
            )
        }
        request::ServiceAction::Stop => {
            let stopped = service::stop(&request.project).map_err(failed)?;
            write_output(None, |writer| {
                writer.write_all(stopped.text(&request.project).as_bytes())
            })
        }
        request::ServiceAction::Serve => {
            let environment = process_limits::Environment::read().map_err(failed)?;
            service::serve(&request.project, &environment).map_err(failed)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(delivered) => ExitCode::from(delivered.code()),
    }
}
