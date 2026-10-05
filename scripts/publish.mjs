// Publish the app to GitHub Pages: build it for the repo's path and force-push it as a single commit
// to the repo's `gh-pages` branch (Pages serves that branch; the source is on main). Every generator
// version published stays on the site as its own build (v<N>/).
// Needs a `site` remote (git remote add site https://github.com/<user>/<repo>.git) and, for the
// commit author, `git config site.email <id>+<user>@users.noreply.github.com`.
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const git = (args, cwd = root) => execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
const tryGit = (args) => {
  try {
    return git(args);
  } catch {
    return '';
  }
};

/** The branch GitHub Pages serves. */
const BRANCH = 'gh-pages';
const remote = tryGit(['remote', 'get-url', 'site']);
const m = /github\.com[/:]([^/]+)\/([^/]+?)(\.git)?$/.exec(remote);
if (!m) {
  console.error('No `site` remote on GitHub. Add one: git remote add site https://github.com/<user>/<repo>.git');
  process.exit(1);
}
const [, user, repo] = m;
const email = tryGit(['config', 'site.email']);
if (!email) {
  console.error('Set the commit email for the public site (your GitHub noreply address): git config site.email <id>+<user>@users.noreply.github.com');
  process.exit(1);
}
const name = tryGit(['config', 'user.name']) || user;

const head = git(['rev-parse', '--short', 'HEAD']);
const dirty = git(['status', '--porcelain', '--untracked-files=no']) !== '';
if (dirty) console.warn('Warning: uncommitted changes are included in this build.');

const base = repo.toLowerCase() === `${user.toLowerCase()}.github.io` ? '/' : `/${repo}/`;
const gen = Number(/GEN_VERSION = (\d+)/.exec(readFileSync(join(root, 'app/src/world/world.ts'), 'utf8'))?.[1]);
if (!gen) throw new Error('GEN_VERSION not found in app/src/world/world.ts');
const site = join(root, 'target/site');
const pinned = join(root, 'target/site-pinned');
const kept = join(root, 'target/site-kept');

// The newest build at the root; the same generator kept at v<gen>/ (its base differs), for the
// worlds made with it once a later generator is published (src/world/versions.ts).
const shell = process.platform === 'win32';
execFileSync('npm', ['run', 'build'], { cwd: root, stdio: 'inherit', shell, env: { ...process.env, WS_BASE: base, VITE_SITE_ROOT: base } });
execFileSync('npm', ['--prefix', 'app', 'run', 'build', '--', '--outDir', pinned, '--emptyOutDir'], { cwd: root, stdio: 'inherit', shell, env: { ...process.env, WS_BASE: `${base}v${gen}/`, VITE_SITE_ROOT: base } });

// Builds of earlier generators come from the site as published, byte for byte.
rmSync(kept, { recursive: true, force: true });
try {
  execFileSync('git', ['-c', 'core.autocrlf=false', 'clone', '-q', '--depth', '1', '--branch', BRANCH, remote, kept], { stdio: 'inherit' });
} catch {
  console.warn('Could not read the published site: no earlier versions kept.');
}

rmSync(site, { recursive: true, force: true });
mkdirSync(site, { recursive: true });
cpSync(join(root, 'app/dist'), site, { recursive: true });
const versions = [gen];
if (existsSync(kept)) {
  for (const e of readdirSync(kept, { withFileTypes: true })) {
    const n = Number(/^v(\d+)$/.exec(e.name)?.[1]);
    if (!e.isDirectory() || !n || n === gen) continue;
    cpSync(join(kept, e.name), join(site, e.name), { recursive: true });
    versions.push(n);
  }
}
cpSync(pinned, join(site, `v${gen}`), { recursive: true });
versions.sort((a, b) => a - b);
writeFileSync(join(site, 'versions.json'), `${JSON.stringify({ latest: gen, kept: versions })}\n`);
writeFileSync(join(site, '.nojekyll'), '');
const url = `https://${user.toLowerCase()}.github.io${base}`;

let bytes = 0;
const walk = (d) => readdirSync(d, { withFileTypes: true }).forEach((e) => (e.isDirectory() ? walk(join(d, e.name)) : (bytes += statSync(join(d, e.name)).size)));
walk(site);

git(['init', '-q', '-b', 'main'], site);
// Files go up exactly as built (and earlier versions' builds as they were).
git(['config', 'core.autocrlf', 'false'], site);
git(['add', '-A'], site);
git(['-c', `user.name=${name}`, '-c', `user.email=${email}`, 'commit', '-q', '-m', `Site from ${head}${dirty ? ' (dirty)' : ''}`], site);
execFileSync('git', ['push', '--force', remote, `HEAD:${BRANCH}`], { cwd: site, stdio: 'inherit' });

console.log(`Published generator v${gen} (kept: ${versions.join(', ')}), ${(bytes / 1048576).toFixed(1)} MB: ${url}`);
