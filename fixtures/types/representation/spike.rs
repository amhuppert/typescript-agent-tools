//! Disposable structural-description comparison. No binder or production API.
//! AnnotationPrimitive asks GetTypeFromTypeNode only for primitive keywords;
//! it does not infer value types, follow aliases, or resolve lexical names.
use std::{alloc::{GlobalAlloc, Layout, System}, collections::HashMap, fs, hint::black_box,
    sync::atomic::{AtomicUsize, Ordering}, time::Instant};
use typescript_parser::{Ast, NodeId, ParseOptions, Value};

struct Allocator;
const ACCOUNT: bool = cfg!(memory_accounting);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
fn allocated(bytes: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) }; if ACCOUNT && !p.is_null() { allocated(layout.size()); } p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) }; if ACCOUNT && !p.is_null() { allocated(layout.size()); } p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        if ACCOUNT { LIVE.fetch_sub(layout.size(), Ordering::Relaxed); } unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(p, layout, size) };
        if ACCOUNT && !next.is_null() { LIVE.fetch_sub(layout.size(), Ordering::Relaxed); allocated(size); } next
    }
}
#[global_allocator] static ALLOCATOR: Allocator = Allocator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)] struct Site(u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Primitive { Number, String, Boolean, Any, Unknown, Never, BigInt, Symbol, Void, Object, Undefined }
impl Primitive { fn label(self) -> &'static str { match self {
    Self::Number => "number", Self::String => "string", Self::Boolean => "boolean", Self::Any => "any", Self::Unknown => "unknown",
    Self::Never => "never", Self::BigInt => "bigint", Self::Symbol => "symbol", Self::Void => "void", Self::Object => "object", Self::Undefined => "undefined",
} } }
fn primitive(kind: &str) -> Option<Primitive> { Some(match kind {
    "NumberKeyword" => Primitive::Number, "StringKeyword" => Primitive::String, "BooleanKeyword" => Primitive::Boolean,
    "AnyKeyword" => Primitive::Any, "UnknownKeyword" => Primitive::Unknown, "NeverKeyword" => Primitive::Never,
    "BigIntKeyword" => Primitive::BigInt, "SymbolKeyword" => Primitive::Symbol, "VoidKeyword" => Primitive::Void,
    "ObjectKeyword" => Primitive::Object, "UndefinedKeyword" => Primitive::Undefined, _ => return None,
}) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] struct Term {
    kind: &'static str, start: u32, end: u32, flags: u32, first_child: u32, children: u32, annotation: u32,
}
#[derive(Debug, PartialEq, Eq)] struct Descriptions { terms: Box<[Term]>, children: Box<[Site]> }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] struct Seed { declaration: Site, annotation: Site, start: u32, end: u32, expected: Primitive }
struct Source { name: String, original: String, text: String }
struct Inventory { nodes: Vec<NodeId>, ordinals: Vec<u32>, seeds: Vec<Seed> }
const ABSENT: u32 = u32::MAX;
fn node_field(ast: &Ast<'_>, node: NodeId, name: &str) -> Option<NodeId> {
    match ast.node(node).field(name) { Some(Value::Node(child)) => Some(*child), _ => None }
}
fn inventory(ast: &Ast<'_>) -> Inventory {
    // Ast.walk owns final-tree traversal. Arena ordinals are used transiently
    // only to attach the stable final-preorder Site to fields/child links.
    let nodes: Vec<_> = ast.walk().collect();
    let mut ordinals = vec![ABSENT; ast.nodes().len()];
    for (i, id) in nodes.iter().enumerate() { assert_eq!(ordinals[id.index()], ABSENT); ordinals[id.index()] = u32::try_from(i).unwrap(); }
    let mut seeds = Vec::new();
    for (i, id) in nodes.iter().enumerate() {
        if !matches!(ast.node(*id).kind, "VariableDeclaration" | "Parameter" | "PropertyDeclaration" | "PropertySignature" | "FunctionDeclaration" | "MethodDeclaration" | "MethodSignature" | "FunctionExpression" | "ArrowFunction" | "CallSignature") { continue; }
        let Some(annotation) = node_field(ast, *id, "type") else { continue; };
        let Some(expected) = primitive(ast.node(annotation).kind) else { continue; };
        let span = ast.token_span(annotation);
        seeds.push(Seed { declaration: Site(i as u32), annotation: Site(ordinals[annotation.index()]), start: span.start.try_into().unwrap(), end: span.end.try_into().unwrap(), expected });
    }
    Inventory { nodes, ordinals, seeds }
}
fn project(ast: &Ast<'_>, inventory: &Inventory) -> Descriptions {
    let mut terms = Vec::with_capacity(inventory.nodes.len());
    let mut children = Vec::new();
    for id in &inventory.nodes {
        let node = ast.node(*id); let span = ast.token_span(*id); let first_child = u32::try_from(children.len()).unwrap();
        // Ast.children owns logical fields (including assertClause alias exclusion).
        children.extend(ast.children(*id).map(|id| { let site = inventory.ordinals[id.index()]; assert_ne!(site, ABSENT); Site(site) }));
        terms.push(Term { kind: node.kind, start: span.start.try_into().unwrap(), end: span.end.try_into().unwrap(), flags: node.flags,
            first_child, children: u32::try_from(children.len()).unwrap() - first_child,
            annotation: node_field(ast, *id, "type").map_or(ABSENT, |id| inventory.ordinals[id.index()]) });
    }
    Descriptions { terms: terms.into_boxed_slice(), children: children.into_boxed_slice() }
}
fn parse(source: &Source) -> Ast<'_> {
    typescript_parser::parse_with_options(&source.text, ParseOptions { max_depth: ParseOptions::DEEP_MAX_DEPTH, ..ParseOptions::for_file(&source.original) })
        .unwrap_or_else(|e| panic!("{}: {e}", source.name))
}
fn acquire(source: &Source) -> (Descriptions, Vec<Seed>, usize) {
    let ast = parse(source); let inventory = inventory(&ast); let descriptions = project(&ast, &inventory);
    let arena = ast.nodes().len(); (descriptions, inventory.seeds, arena)
}
fn annotation_primitive(descriptions: &Descriptions, seed: Seed) -> Primitive {
    let declaration = &descriptions.terms[seed.declaration.0 as usize];
    assert_eq!(declaration.annotation, seed.annotation.0);
    let annotation = &descriptions.terms[seed.annotation.0 as usize];
    assert_eq!((annotation.start, annotation.end), (seed.start, seed.end));
    primitive(annotation.kind).expect("admitted primitive annotation")
}
fn answers(descriptions: &Descriptions, seeds: &[Seed], limit: usize) -> u64 {
    seeds.iter().take(limit).fold(0, |sum, seed| {
        let result = annotation_primitive(descriptions, *seed); assert_eq!(result, seed.expected);
        sum.wrapping_mul(31).wrapping_add(result as u64 + 1)
    })
}
#[derive(Clone, Copy)] struct Metrics { nanos: u128, peak: usize, retained: isize, calls: usize }
fn measured<T>(action: impl FnOnce() -> T) -> (T, Metrics) {
    // The global live counter never resets: pre-existing deallocation during
    // replacement is accounted, instead of wrapping a reset-zero usize.
    let base = LIVE.load(Ordering::Relaxed); let calls = CALLS.load(Ordering::Relaxed); PEAK.store(base, Ordering::Relaxed);
    let start = Instant::now(); let result = action(); let nanos = start.elapsed().as_nanos();
    let m = Metrics { nanos, peak: PEAK.load(Ordering::Relaxed).saturating_sub(base), retained: LIVE.load(Ordering::Relaxed) as isize - base as isize, calls: CALLS.load(Ordering::Relaxed) - calls };
    (result, m)
}
fn emit(source: &str, operation: &str, round: usize, m: Metrics, parses: usize, sites: usize, queries: usize) {
    println!("metric\t{source}\t{operation}\t{round}\t{}\t{}\t{}\t{}\t{parses}\t{sites}\t{queries}", m.nanos, m.peak, m.retained, m.calls);
}
fn run(input: &str, rounds: usize) {
    println!("build\trequested_heap_accounting\t{ACCOUNT}");
    let sources: Vec<_> = fs::read_to_string(input).unwrap().lines().map(|line| {
        let fields: Vec<_> = line.split('\t').collect(); assert_eq!(fields.len(), 3);
        Source { name: fields[0].into(), original: fields[1].into(), text: fs::read_to_string(fields[2]).unwrap() }
    }).collect();
    let mut whole_eager = Vec::new(); let mut whole_seeds = Vec::new();
    for source in &sources {
        let (expected, seeds, arena) = acquire(source);
        println!("inventory\t{}\t{}\t{}\t{}\t{}\t{}\t{}", source.name, source.text.len(), expected.terms.len(), expected.children.len(), seeds.len(), arena, std::mem::size_of::<Term>());
        for seed in seeds.iter().take(3) { println!("oracle\t{}\t{}\t{}\t{}\t{}", source.name, seed.declaration.0, seed.annotation.0, seed.start, seed.expected.label()); }
        // Every term and child link, not just counts, round-trips from captured text.
        let (second, second_seeds, _) = acquire(source); assert_eq!(expected, second); assert_eq!(seeds, second_seeds); drop(second);
        for round in 0..rounds {
            let (_, m) = measured(|| { let ast = parse(source); black_box(ast.root_id()); });
            emit(&source.name, "parse-only", round, m, 1, expected.terms.len(), 0);
            for eager in if round % 2 == 0 { [true, false] } else { [false, true] } {
                if eager {
                    let (capture, m) = measured(|| acquire(source));
                    assert_eq!(expected, capture.0); emit(&source.name, "eager-ingest", round, m, 1, expected.terms.len(), 0); drop(capture);
                } else {
                    let (retained_seeds, m) = measured(|| { let ast = parse(source); let i = inventory(&ast); black_box(i.seeds) });
                    assert_eq!(seeds, retained_seeds); emit(&source.name, "reprojection-ingest", round, m, 1, expected.terms.len(), 0); drop(retained_seeds);
                }
            }
            let ast = parse(source); let i = inventory(&ast);
            let (projection, m) = measured(|| project(&ast, &i)); assert_eq!(expected, projection);
            emit(&source.name, "projection-only", round, m, 0, expected.terms.len(), 0); drop(projection); drop(i); drop(ast);
            for limit in [1, 16, usize::MAX] {
                let count = limit.min(seeds.len()); if count == 0 { continue; }
                let operation = if limit == 1 { "single" } else if limit == 16 { "batch16" } else { "all-annotations" };
                let checksum = answers(&expected, &seeds, limit);
                let (answer, m) = measured(|| black_box(answers(black_box(&expected), black_box(&seeds), limit)));
                assert_eq!(checksum, answer); emit(&source.name, &format!("eager-{operation}"), round, m, 0, expected.terms.len(), count);
                let (answer, m) = measured(|| { let (descriptions, replay_seeds, _) = acquire(source); assert_eq!(seeds, replay_seeds); black_box(answers(&descriptions, &seeds, limit)) });
                assert_eq!(checksum, answer); emit(&source.name, &format!("reproject-{operation}"), round, m, 1, expected.terms.len(), count);
            }
        }
        whole_eager.push(expected); whole_seeds.push(seeds);
    }
    for round in 0..rounds {
        let (answer, m) = measured(|| (0..2).fold(0u64, |sum, _| whole_eager.iter().zip(&whole_seeds).fold(sum, |sum, (d, s)| sum.wrapping_add(black_box(answers(d, s, 16))))));
        emit("ALL", "eager-cross-file-double", round, m, 0, whole_eager.iter().map(|d| d.terms.len()).sum(), 2 * whole_seeds.iter().map(|s| s.len().min(16)).sum::<usize>());
        let (replayed, m) = measured(|| {
            let mut cache = HashMap::new(); let mut count = 0; let mut checksum = 0u64;
            // Two demands per file; both share one request-local description.
            for _ in 0..2 { for (file, source) in sources.iter().enumerate() {
                if whole_seeds[file].is_empty() { continue; }
                let descriptions = cache.entry(file).or_insert_with(|| { count += 1; let (d, s, _) = acquire(source); assert_eq!(whole_seeds[file], s); d });
                checksum = checksum.wrapping_add(answers(descriptions, &whole_seeds[file], 16));
            } }
            black_box((checksum, count))
        });
        assert_eq!(answer, replayed.0); emit("ALL", "reproject-cross-file-double", round, m, replayed.1, whole_eager.iter().map(|d| d.terms.len()).sum(), 2 * whole_seeds.iter().map(|s| s.len().min(16)).sum::<usize>());
    }
    // Snapshot shape teardown: source strings predate this measured lifetime.
    let before = LIVE.load(Ordering::Relaxed); let bytes: usize = whole_eager.iter().map(|d| d.terms.len() * std::mem::size_of::<Term>() + d.children.len() * std::mem::size_of::<Site>()).sum();
    drop(whole_eager); let released = before - LIVE.load(Ordering::Relaxed); if ACCOUNT { assert!(released >= bytes); }
    println!("summary\t{}\t{bytes}\t{released}", sources.len());
}
fn main() {
    let args: Vec<_> = std::env::args().collect(); assert_eq!(args.len(), 3, "spike inputs.tsv rounds");
    let input = args[1].clone(); let rounds = args[2].parse().unwrap();
    std::thread::Builder::new().name("representation-spike".into()).stack_size(typescript_parser::DEEP_PARSE_STACK_BYTES)
        .spawn(move || run(&input, rounds)).unwrap().join().unwrap();
}
