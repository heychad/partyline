import { Children, isValidElement, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { annotateItems, expanded, type OutlineState } from './outline';
import type { OutlineSection } from './workspace';
export default function NotesOutline({sections,state,toggle}:{sections:OutlineSection[];state:OutlineState;toggle:(id:string,expanded:boolean)=>void}){
  function section(s:OutlineSection):ReactNode{
    const body=<><ReactMarkdown remarkPlugins={[remarkGfm,()=>tree=>annotateItems(tree,s.id)]} components={{li:({children,node})=>{const id=node?.properties?.['data-outline-id'] as string|undefined;const parts=Children.toArray(children);const at=parts.findIndex(p=>isValidElement(p)&&(p.type==='ul'||p.type==='ol'));if(!id||at<0)return <li>{children}</li>;return <li className="cl-outline-parent"><details data-outline-id={id} open={expanded(state,id)}><summary onClick={e=>{e.preventDefault();toggle(id,!expanded(state,id));}}>{parts.slice(0,at)}</summary>{parts.slice(at)}</details></li>;}}}>{s.body}</ReactMarkdown>{s.children.map(section)}</>;
    return s.title?<details className="cl-outline-section" key={s.id} data-outline-id={s.id} open={expanded(state,s.id)}><summary onClick={e=>{e.preventDefault();toggle(s.id,!expanded(state,s.id));}}>{s.title}</summary>{body}</details>:<div key={s.id}>{body}</div>;
  }
  return <div className="cl-markdown cl-outline">{sections.map(section)}</div>;
}
