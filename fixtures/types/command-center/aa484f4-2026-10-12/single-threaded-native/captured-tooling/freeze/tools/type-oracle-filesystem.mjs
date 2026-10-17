import fs from 'node:fs';
import path from 'node:path';
import {compilerText} from './navigation-oracle.mjs';

// Capture the five callbackFS operations, not an inferred directory tree: a
// positive FileExists can affect resolution without ever causing a ReadFile.
// One capture survives fresh checker restarts when the hook demands more facts.
// Reobservations must agree with frozen answers; only absence is a normal miss.
export function captureFileSystem() {
  const files = Object.create(null);
  const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
  const absent = error => error.code === 'ENOENT' || error.code === 'ENOTDIR';
  const stat = file => {
    try { return fs.statSync(file); }
    catch (error) { if (absent(error)) return undefined; throw error; }
  };
  // These are the OS observations behind vfs.Common and osvfs.Realpath. The
  // API's Entries callback deliberately does not carry symlink metadata.
  const observe = {
    readFile: file => {
      try { return decoder.decode(fs.readFileSync(file)); }
      catch (error) { if (absent(error)) return null; throw error; }
    },
    fileExists: file => { const info = stat(file); return info !== undefined && !info.isDirectory(); },
    directoryExists: file => stat(file)?.isDirectory() ?? false,
    getAccessibleEntries: directory => {
      const result = { files: [], directories: [] };
      let entries;
      try { entries = fs.readdirSync(directory, { withFileTypes: true, encoding: 'buffer' }); }
      catch (error) { if (absent(error)) return result; throw error; }
      for (const entry of entries) {
        const name = decoder.decode(entry.name);
        const info = entry.isSymbolicLink() ? stat(path.join(directory, name)) : entry;
        if (info?.isDirectory()) result.directories.push(name);
        else if (info?.isFile()) result.files.push(name);
      }
      const compareBytes = (a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b));
      result.files.sort(compareBytes); result.directories.sort(compareBytes);
      return result;
    },
    realpath: file => {
      try { return decoder.decode(fs.realpathSync(file, { encoding: 'buffer' })); }
      catch (error) { if (absent(error)) return file; throw error; }
    },
  };
  const answers = Object.fromEntries(Object.keys(observe).map(operation => [operation, Object.create(null)]));
  const check = (operation, file, value) => {
    if (JSON.stringify(answers[operation][file]) !== JSON.stringify(value))
      throw new Error(`Oracle filesystem changed during capture: ${operation} ${file}`);
  };
  const verifyPath = file => {
    for (const operation of Object.keys(observe)) if (Object.hasOwn(answers[operation], file))
      check(operation, file, observe[operation](file));
  };
  const capture = (operation, file) => {
    if (!Object.hasOwn(observe, operation) || typeof file !== 'string')
      throw new Error(`Compiler hook filesystem protocol: invalid operation ${operation}`);
    verifyPath(file);
    const value = observe[operation](file);
    if (Object.hasOwn(answers[operation], file)) check(operation, file, value);
    else answers[operation][file] = value;
    if (operation === 'readFile' && value !== null) files[file] = value;
    return operation === 'readFile' && value !== null ? compilerText(value) : value;
  };
  return { files, answers,
    callbacks: Object.fromEntries(Object.keys(observe).map(operation => [operation, file => capture(operation, file)])),
    complete({ operation, path: file }) {
      if (!Object.hasOwn(answers, operation) || typeof file !== 'string')
        throw new Error('Compiler hook filesystem protocol: invalid missing-operation request');
      if (Object.hasOwn(answers[operation], file))
        throw new Error(`Compiler hook filesystem protocol: requested an already captured answer: ${operation} ${file}`);
      capture(operation, file);
    },
    verify() {
      for (const [operation, entries] of Object.entries(answers))
        for (const file of Object.keys(entries)) check(operation, file, observe[operation](file));
    },
  };
}

