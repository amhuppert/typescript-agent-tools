import hashlib
import json
import pathlib
import subprocess

base = pathlib.Path(__file__).resolve().parent
repo = pathlib.Path('/Users/alex/github/command-center')
baseline = json.loads((base.parent / 'types-baseline/command-center-manifest.json').read_text())
revision = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
assert revision == baseline['revision'] == '92fe6ab06fbea2c4b6a305689f8cf336173eef81'
records = {entry['file']: entry for entry in baseline['files']}
names = [query['at']['file'] for query in baseline['queries']]
tracked = [entry for entry in baseline['files'] if entry['tracked'] and entry['file'].endswith(('.ts', '.tsx', '.mts', '.cts'))]
names.extend(entry['file'] for entry in sorted(tracked, key=lambda entry: -entry['bytes'])[:3])
names.append(min((entry for entry in tracked if entry['bytes'] >= 2048), key=lambda entry: entry['bytes'])['file'])
for suffix in ['lib.dom.d.ts', 'lib.es5.d.ts']:
    names.append(next(entry['file'] for entry in baseline['files'] if entry['defaultLibrary'] and entry['file'].endswith('/' + suffix)))
external = [entry for entry in baseline['files'] if entry['externalLibrary'] and not entry['defaultLibrary'] and entry['file'].endswith(('.ts', '.tsx', '.mts', '.cts'))]
names.append(max(external, key=lambda entry: entry['bytes'])['file'])
generated = [entry for entry in baseline['generatedInputs'] if entry['file'].endswith(('.ts', '.tsx', '.mts', '.cts'))]
if generated:
    names.append(max(generated, key=lambda entry: entry['bytes'])['file'])

manifest = []
lines = []
for ordinal, name in enumerate(dict.fromkeys(names)):
    original = (repo / name).resolve()
    raw = original.read_bytes()
    raw.decode('utf-8')
    expected = records[name]
    digest = hashlib.sha256(raw).hexdigest()
    assert digest == expected['sha256'] and len(raw) == expected['bytes'], name
    capture = base / 'sources' / str(ordinal)
    capture.write_bytes(raw)
    assert '\t' not in name and '\t' not in str(original)
    manifest.append({**expected, 'captured': str(capture), 'original': str(original)})
    lines.append(f'{name}\t{original}\t{capture}')

controls = {
    'synthetic-annotated.ts': ''.join(f'export const n{i}: number = {i};\nexport const s{i}: string = "x";\nexport const b{i}: boolean = true;\n' for i in range(5000)),
    'synthetic-shallow.ts': 'export const values: number[] = [' + ','.join(str(i) for i in range(100000)) + '];\nexport const size: number = 100000;\n',
    'synthetic-deep.ts': 'export type Deep = ' + '[' * 128 + 'number' + ']' * 128 + ';\nexport const depth: number = 128;\n',
    'synthetic-roles.ts': '\ufeff#! prototype\r\nexport const n: number = 1;\u2028export const s: string = "x";\u2029export const b: boolean = true;\rconst f = (x: number): boolean => x > 0;\n',
    'synthetic-reparse.ts': 'export {};\nawait(foo);\nexport const n: number = 1;\n',
}
for name, text in controls.items():
    capture = base / 'sources' / name
    raw = text.encode('utf-8')
    capture.write_bytes(raw)
    manifest.append({'file': name, 'captured': str(capture), 'original': str(capture), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(), 'control': True})
    lines.append(f'{name}\t{capture}\t{capture}')
(base / 'inputs.tsv').write_text('\n'.join(lines) + '\n')
over_budget = base / 'sources' / 'over-budget.ts'
over_budget.write_text('export type Deep = ' + '[' * 300 + 'number' + ']' * 300 + ';\n')
(base / 'capture-manifest.json').write_text(json.dumps({'revision': revision, 'baselineManifest': str(base.parent / 'types-baseline/command-center-manifest.json'), 'sources': manifest, 'excludedControl': {'file': str(over_budget), 'sha256': hashlib.sha256(over_budget.read_bytes()).hexdigest(), 'reason': 'Maximum syntax nesting depth exceeded under analysis budget; valid syntax, not a grammar rejection'}}, indent=2) + '\n')
print(json.dumps({'revision': revision, 'capturedSources': len(manifest), 'bytes': sum(entry['bytes'] for entry in manifest), 'scope': 'hashed sample, not whole project'}))
