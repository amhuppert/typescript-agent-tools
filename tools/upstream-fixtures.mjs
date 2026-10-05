import fs from 'node:fs';
import path from 'node:path';
export function* walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) yield* walk(file);
    else if (entry.isFile() && /\.(?:tsx?|mts|cts)$/.test(entry.name)) yield file;
  }
}
export function units(file, text) {
  const sections = [...text.matchAll(/^\s*\/\/\s*@filename\s*:\s*(.*?)\s*\r?$/gmi)];
  if (!sections.length) return [{ name: path.basename(file), source: text }];
  return sections.map((section, index) => ({ name: section[1], source: text.slice(section.index + section[0].length, sections[index + 1]?.index ?? text.length) }))
    .filter(unit => /\.(?:tsx?|mts|cts)$/.test(unit.name));
}
export function* allUnits(root) {
  for (const folder of corpusFolders(root)) {
    for (const file of walk(folder)) {
      for (const unit of units(file, fs.readFileSync(file, 'utf8'))) yield { fixture: path.relative(root, file), ...unit };
    }
  }
}

export function corpusFolders(root) {
  const folders = ['conformance', 'compiler'].map(category => path.join(root, 'tests/cases', category))
    .filter(folder => fs.existsSync(folder) && fs.statSync(folder).isDirectory());
  if (!folders.length) throw new Error(`No upstream corpus directories found under ${root}/tests/cases`);
  return folders;
}
