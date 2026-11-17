import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import { outline, type OutlineSection } from './workspace.ts';

export type OutlineState = { default_collapsed: boolean; overrides: Record<string, boolean> };
export type OutlineNode = { id: string; label: string; depth: number; expanded: boolean };
export const emptyOutline: OutlineState = { default_collapsed: false, overrides: {} };
export const expanded = (state: OutlineState, id: string) => state.overrides[id] ?? !state.default_collapsed;
export const outlineKey = (text: string) => text.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}\s]/gu, '').trim().replace(/\s+/g, ' ').slice(0, 180);
type Node = { type: string; value?: string; children?: Node[]; data?: { hProperties?: Record<string, unknown> } };
const plain = (node: Node): string => node.value ?? (node.children ?? []).filter(n => n.type !== 'list').map(plain).join('');
export function annotateItems(tree: Node, section: string, visit?: (id: string, label: string, depth: number) => void) {
  const counts = new Map<string, number>();
  function walk(node: Node, depth: number) {
    if (node.type === 'listItem' && node.children?.some(n => n.type === 'list')) {
      const label = plain(node).trim();
      const key = `${section}/item/${outlineKey(label)}`;
      const occurrence = counts.get(key) ?? 0; counts.set(key, occurrence + 1);
      const id = `${key}/${occurrence}`;
      node.data = { ...node.data, hProperties: { ...node.data?.hProperties, 'data-outline-id': id } };
      visit?.(id, label, depth);
    }
    for (const child of node.children ?? []) walk(child, depth + (node.type === 'list' ? 1 : 0));
  }
  walk(tree, 0);
}
export function outlineDocument(markdown: string, state: OutlineState) {
  const sections = outline(markdown);
  const nodes: OutlineNode[] = [];
  function walk(section: OutlineSection) {
    if (section.title) nodes.push({ id: section.id, label: section.title, depth: section.depth, expanded: expanded(state, section.id) });
    const tree = unified().use(remarkParse).use(remarkGfm).parse(section.body);
    annotateItems(tree as Node, section.id, (id, label, depth) => nodes.push({ id, label, depth: section.depth + depth + 1, expanded: expanded(state, id) }));
    section.children.forEach(walk);
  }
  sections.forEach(walk);
  return { sections, nodes };
}
