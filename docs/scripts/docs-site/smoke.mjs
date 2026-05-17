#!/usr/bin/env node

/**
 * Smoke test the curated docs build.
 */

import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import { JSDOM } from 'jsdom';

const rootDir = process.cwd();
const distDir = path.join(rootDir, 'dist', 'docs-site');
const config = JSON.parse(fs.readFileSync(path.join(rootDir, 'docs.json'), 'utf8'));
const publicPlaceholderPattern = /\b(Image asset prompt|lorem ipsum|FIXME|coming soon|TBD)\b/i;

function collectPages(node, pages = []) {
  if (Array.isArray(node)) {
    for (const item of node) collectPages(item, pages);
    return pages;
  }

  if (!node || typeof node !== 'object') return pages;

  if (Array.isArray(node.pages)) {
    for (const page of node.pages) {
      if (typeof page === 'string') pages.push(page);
      else collectPages(page, pages);
    }
  }

  for (const value of Object.values(node)) {
    if (value !== node.pages) collectPages(value, pages);
  }

  return pages;
}

function outputPathForPage(page) {
  const normalized = page.replace(/^\/+/, '').replace(/\.md$/, '');
  if (normalized === 'index') return path.join(distDir, 'index.html');
  return path.join(distDir, normalized, 'index.html');
}

function pageUrl(page) {
  const normalized = page.replace(/^\/+/, '').replace(/\.md$/, '').replace(/\/index$/, '');
  return normalized === 'index' ? '/' : `/${normalized}`;
}

function assertFile(file, label = file) {
  if (!fs.existsSync(file)) {
    throw new Error(`${label} not found`);
  }
}

function findMarkdownFiles(directory, files = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    const relativePath = path.relative(rootDir, fullPath);

    if (
      relativePath === 'node_modules' ||
      relativePath.startsWith(`node_modules${path.sep}`) ||
      relativePath === 'dist' ||
      relativePath.startsWith(`dist${path.sep}`)
    ) {
      continue;
    }

    if (entry.isDirectory()) {
      findMarkdownFiles(fullPath, files);
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      files.push(fullPath);
    }
  }

  return files;
}

function validateFrontmatter(page, markdown) {
  const { data } = matter(markdown);

  for (const field of ['title', 'summary', 'read_when']) {
    if (!data[field] || (field === 'read_when' && (!Array.isArray(data[field]) || data[field].length === 0))) {
      throw new Error(`${page} is missing frontmatter field "${field}"`);
    }
  }
}

function validateRawPublicLinks(page, markdown, publicUrls) {
  const linkPattern = /(?:href="|src="|\]\()([^")]+)(?:"|\))/g;
  let match;

  while ((match = linkPattern.exec(markdown))) {
    validateInternalLink(page, match[1], publicUrls);
  }
}

function validateBuiltLinks(page, html, publicUrls) {
  const hrefPattern = /\s(?:href|src)="([^"]+)"/g;
  let match;

  while ((match = hrefPattern.exec(html))) {
    validateInternalLink(page, match[1], publicUrls);
  }
}

function validateInternalLink(page, rawUrl, publicUrls) {
  if (
    rawUrl.startsWith('http://') ||
    rawUrl.startsWith('https://') ||
    rawUrl.startsWith('mailto:') ||
    rawUrl.startsWith('#') ||
    rawUrl.startsWith('//')
  ) {
    return;
  }

  if (!rawUrl.startsWith('/')) {
    const markdownPath = path.join(rootDir, `${page.replace(/^\/+/, '').replace(/\.md$/, '')}.md`);
    const target = rawUrl.split('#')[0];
    if (!target) return;
    const resolved = path.resolve(path.dirname(markdownPath), target);
    if (!fs.existsSync(resolved)) {
      throw new Error(`${page} links to missing relative path: ${rawUrl}`);
    }
    return;
  }

  const url = rawUrl.split('#')[0].replace(/\/$/, '') || '/';
  if (url === '/style.css' || url === '/sidebar-nav.js' || url.startsWith('/assets/')) {
    const assetPath = path.join(rootDir, url);
    if (!fs.existsSync(assetPath)) {
      throw new Error(`${page} links to missing asset: ${rawUrl}`);
    }
    return;
  }

  if (!publicUrls.has(url)) {
    throw new Error(`${page} links to non-public page: ${rawUrl}`);
  }
}

async function validateMermaid() {
  globalThis.window = new JSDOM('<!doctype html><html><body></body></html>').window;
  globalThis.document = globalThis.window.document;
  const { default: mermaid } = await import('mermaid');

  mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' });

  for (const file of findMarkdownFiles(rootDir)) {
    const markdown = fs.readFileSync(file, 'utf8');
    const relativePath = path.relative(rootDir, file);
    const blocks = [...markdown.matchAll(/```mermaid\s*\n([\s\S]*?)```/g)];

    for (const [index, block] of blocks.entries()) {
      try {
        await mermaid.parse(block[1]);
      } catch (error) {
        throw new Error(`${relativePath} has invalid Mermaid block ${index + 1}: ${error.message}`);
      }
    }
  }
}

console.log('Running docs smoke tests...');

const pages = [...new Set(collectPages(config.navigation ?? {}))];
const publicUrls = new Set(pages.map(pageUrl));

for (const page of pages) {
  assertFile(outputPathForPage(page), page);
}

assertFile(path.join(distDir, 'search-index.json'), 'search-index.json');
assertFile(path.join(distDir, 'manifest.json'), 'manifest.json');
assertFile(path.join(distDir, 'style.css'), 'style.css');
assertFile(path.join(distDir, 'assets', 'opencoven-icon.svg'), 'opencoven-icon.svg');

const searchIndex = JSON.parse(fs.readFileSync(path.join(distDir, 'search-index.json'), 'utf8'));
if (searchIndex.length !== pages.length) {
  throw new Error(`search-index.json has ${searchIndex.length} entries, expected ${pages.length}`);
}

const stalePaths = [
  path.join(distDir, 'docs', 'guides', 'create-agent', 'index.html'),
  path.join(distDir, 'guides', 'create-agent', 'index.html'),
  path.join(distDir, 'core', 'agents', 'index', 'index.html'),
  path.join(distDir, 'resources', 'examples', 'basic-workflow', 'index.html')
];

for (const stalePath of stalePaths) {
  if (fs.existsSync(stalePath)) {
    throw new Error(`stale unrelated page was built: ${path.relative(distDir, stalePath)}`);
  }
}

const disallowedLogoFiles = [
  'opencoven-black.svg',
  'opencoven-logo.svg',
  'opencoven-mark.svg',
  'opencoven-monoline.svg',
  'opencoven-white.svg'
];

for (const file of disallowedLogoFiles) {
  if (fs.existsSync(path.join(distDir, 'assets', file))) {
    throw new Error(`non-approved logo variant was copied: assets/${file}`);
  }
}

for (const page of pages) {
  const file = path.join(rootDir, `${page.replace(/^\/+/, '').replace(/\.md$/, '')}.md`);
  const markdown = fs.readFileSync(file, 'utf8');
  const html = fs.readFileSync(outputPathForPage(page), 'utf8');

  validateFrontmatter(page, markdown);
  if (!/<h1\b/.test(html)) {
    throw new Error(`${page} emitted no h1`);
  }
  if (publicPlaceholderPattern.test(markdown)) {
    throw new Error(`${page} contains placeholder or unsituated copy`);
  }
  if (/<\/?(Columns|Card|Steps|Step|Tabs|Tab|Tip|Note|Info|Warning|Frame)\b/.test(html)) {
    throw new Error(`${page} emitted unrendered Mintlify component markup`);
  }
  if (/opencoven-(?:black|mark|logo|monoline|white)\.svg/.test(html)) {
    throw new Error(`${page} emitted a non-approved logo variant`);
  }
  if (/&lt;\/(?:code|pre)&gt;|```&lt;/.test(html)) {
    throw new Error(`${page} emitted malformed escaped code markup`);
  }

  validateRawPublicLinks(page, markdown, publicUrls);
  validateBuiltLinks(page, html, publicUrls);
}

await validateMermaid();

console.log(`Smoke passed for ${pages.length} public pages`);
