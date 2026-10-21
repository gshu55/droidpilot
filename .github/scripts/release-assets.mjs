import { readFileSync, writeFileSync, readdirSync, mkdirSync, copyFileSync, unlinkSync } from 'node:fs';
import { join, basename } from 'node:path';
import { createHash } from 'node:crypto';

const [mode, tag, platform, root] = process.argv.slice(2);
if (!/^v\d+\.\d+\.\d+$/.test(tag)) throw new Error('Invalid stable release tag');
const version = tag.slice(1);
const output = 'release-assets';
const readJson = path => JSON.parse(readFileSync(path, 'utf8'));
const targets = ['windows-x86_64', 'darwin-aarch64'];
const one = (dir, suffix) => {
  const files = readdirSync(dir).filter(name => name.endsWith(suffix));
  if (files.length !== 1) throw new Error(`Expected one ${suffix} in ${dir}`);
  return join(dir, files[0]);
};
const copy = (path, name = basename(path)) => copyFileSync(path, join(output, name));
if (mode === 'verify') {
  const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
  if ([readJson('package.json').version, readJson('src-tauri/tauri.conf.json').version, cargo].some(v => v !== version)) throw new Error('Version mismatch');
} else if (mode === 'stage') {
  if (!targets.includes(platform)) throw new Error('Unsupported platform');
  mkdirSync(output);
  let updater;
  if (platform === 'windows-x86_64') {
    updater = one(join(root, 'bundle/nsis'), '-setup.exe');
    copy(join(root, 'droidpilot.exe'), `DroidPilot_${tag}_windows_x64_portable.exe`);
  } else {
    updater = one(join(root, 'bundle/macos'), '.app.tar.gz');
    copy(one(join(root, 'bundle/dmg'), '.dmg'));
  }
  const signature = readFileSync(`${updater}.sig`, 'utf8').trim();
  if (!signature) throw new Error('Missing signature');
  // Detect signing with a different project's key before publishing an unusable update.
  const pubkey = readJson('src-tauri/tauri.conf.json').plugins.updater.pubkey;
  const packet = value => Buffer.from(Buffer.from(value, 'base64').toString('utf8').split(/\r?\n/)[1], 'base64');
  if (!packet(signature).subarray(2, 10).equals(packet(pubkey).subarray(2, 10))) throw new Error('Updater signing key mismatch');
  copy(updater);
  copy(`${updater}.sig`);
  writeFileSync(join(output, `${platform}.json`), JSON.stringify({ version, signature, url: `https://github.com/gshu55/droidpilot/releases/download/${tag}/${encodeURIComponent(basename(updater))}` }));
} else if (mode === 'merge') {
  const platforms = {};
  for (const target of targets) {
    const fragment = readJson(join(output, `${target}.json`));
    if (fragment.version !== version) throw new Error('Artifact version mismatch');
    const { signature, url } = fragment;
    const file = decodeURIComponent(new URL(url).pathname.split('/').pop());
    if (!readdirSync(output).includes(file)) throw new Error('Missing update package');
    platforms[target] = { signature, url };
  }
  for (const target of targets) unlinkSync(join(output, `${target}.json`));
  writeFileSync(join(output, 'latest.json'), JSON.stringify({ version, notes: readFileSync(`docs/releases/${tag}.md`, 'utf8'), pub_date: new Date().toISOString(), platforms }, null, 2));
  const sums = readdirSync(output).sort().filter(name => name !== 'SHA256SUMS.txt').map(name => `${createHash('sha256').update(readFileSync(join(output, name))).digest('hex')}  ${name}`);
  writeFileSync(join(output, 'SHA256SUMS.txt'), sums.join('\n') + '\n');
} else throw new Error('Unknown release operation');
