import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {loadSemanticApi} from './semantic-oracle.mjs';

const fixture = fileURLToPath(new URL('../fixtures/program/native-entry-paths.json', import.meta.url));

test('native file discovery appends directory entries without interpreting drive roots',
  {skip: process.platform === 'win32' && 'colon directory entries require a Unix filesystem'}, async () => {
    const semantic = await loadSemanticApi();
    const rows = [];
    for (const directory of ['A:', 'z:', '1:', 'AA:', 'A:child', 'é:']) {
      for (const access of ['real', 'link']) {
        for (const selection of ['wildcard', 'literal']) {
          const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'native-entry-paths-')));
          const source = 'export const x = 1;\n';
          let api, snapshot;
          try {
            fs.mkdirSync(path.join(root, 'real', directory), {recursive: true});
            fs.writeFileSync(path.join(root, 'real', directory, 'x.ts'), source);
            const config = {compilerOptions: {noLib: true, skipLibCheck: true}};
            if (selection === 'literal') config.files = [`./${directory}/x.ts`];
            fs.writeFileSync(path.join(root, 'real/tsconfig.json'), JSON.stringify(config));
            fs.symlinkSync('real', path.join(root, 'link'), 'dir');
            const configPath = path.join(root, access, 'tsconfig.json');
            api = new semantic.API({tsserverPath: semantic.binary, cwd: root});
            snapshot = api.updateSnapshot({openProject: configPath});
            const {program, checker} = snapshot.getProject(configPath);
            const files = program.getSourceFileNames();
            const input = program.getSourceFile(path.join(root, access, directory, 'x.ts'));
            const token = input && semantic.getTokenAtPosition(input, source.indexOf('x ='));
            rows.push({directory, access, selection,
              program: files.map(file => path.relative(root, file).split(path.sep).join('/')),
              diagnostics: [program.getConfigFileParsingDiagnostics(), program.getProgramDiagnostics(),
                program.getSyntacticDiagnostics(), program.getSemanticDiagnostics()].flat()
                .filter(diagnostic => diagnostic.code !== 2318).map(diagnostic => diagnostic.code),
              selected: token ? checker.typeToString(checker.getTypeAtLocation(token)) : null});
          } finally {
            try { snapshot?.dispose(); } finally { api?.close(); }
            fs.rmSync(root, {recursive: true, force: true});
          }
        }
      }
    }
    fs.mkdirSync('.cc/temp', {recursive: true});
    fs.writeFileSync('.cc/temp/native-entry-paths.json', JSON.stringify(rows, null, 2) + '\n');
    assert.deepEqual(rows, JSON.parse(fs.readFileSync(fixture, 'utf8')));
  });
