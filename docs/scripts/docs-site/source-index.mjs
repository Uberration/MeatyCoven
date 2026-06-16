#!/usr/bin/env node

/**
 * Create the lightweight JSON source index for the curated public docs.
 * Pagefind builds the production search files from dist/docs-site; this file is
 * kept for local tooling and smoke tests.
 */

import fs from 'fs';
import path from 'path';
import matter from './frontmatter.mjs';

const rootDir = process.cwd();
const config = JSON.parse(fs.readFileSync(path.join(rootDir, 'docs.json'), 'utf8'));
const indexPath = path.join(rootDir, 'dist', 'docs-site', 'search-index.json');

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

function firstHeading(markdown) {
  const match = markdown.match(/^#\s+(.+)$/m);
  return match?.[1]?.trim();
}

function firstParagraph(markdown) {
  return markdown
    .split(/\n{2,}/)
    .map((chunk) => chunk.trim())
    .find((chunk) => chunk && !chunk.startsWith('#') && !chunk.startsWith('<'))
    ?.replace(/\s+/g, ' ')
    .slice(0, 180);
}

function pageUrl(page) {
  const normalized = page.replace(/^\/+/, '').replace(/\.md$/, '').replace(/\/index$/, '');
  return normalized === 'index' ? '/' : `/${normalized}`;
}

console.log('Indexing public documentation...');

const pages = [...new Set(collectPages(config.navigation ?? {}))];
const index = [];

for (const page of pages) {
  const file = `${page.replace(/^\/+/, '').replace(/\.md$/, '')}.md`;
  const fullPath = path.join(rootDir, file);
  const raw = fs.readFileSync(fullPath, 'utf8');
  const { data, content } = matter(raw);

  index.push({
    id: file,
    title: data.title || firstHeading(content) || 'Untitled',
    description: data.description || data.summary || firstParagraph(content) || '',
    url: pageUrl(page),
    content: content.replace(/\s+/g, ' ').slice(0, 700),
    tags: data.tags || []
  });
}

fs.mkdirSync(path.dirname(indexPath), { recursive: true });
fs.writeFileSync(indexPath, JSON.stringify(index, null, 2));

console.log(`Created search index with ${index.length} public documents`);
