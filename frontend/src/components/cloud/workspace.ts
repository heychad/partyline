export type Preferences={notes_seconds:number;sidebar_width:number;transcript_percent:number;chat_width:number;sidebar_visible:boolean;transcript_visible:boolean;notes_visible:boolean;chat_visible:boolean;pin_transcript:boolean;collapsed_notes:string[]};
export const defaults:Preferences={notes_seconds:120,sidebar_width:225,transcript_percent:50,chat_width:360,sidebar_visible:true,transcript_visible:true,notes_visible:true,chat_visible:false,pin_transcript:true,collapsed_notes:[]};
export const refreshRates=[30,60,120,300,0];
export function resized(value:number,delta:number,min:number,max:number){return Math.min(max,Math.max(min,Math.round(value+delta)));}
export type OutlineSection={id:string;title:string;body:string;children:OutlineSection[];depth:number};
export function outline(markdown:string):OutlineSection[]{
  const root:OutlineSection={id:'root',title:'',body:'',children:[],depth:0};const stack=[root];const counts=new Map<string,number>();let fenced=false;
  for(const line of markdown.split('\n')){if(/^\s*(```|~~~)/.test(line))fenced=!fenced;const heading=!fenced&&/^(#{1,6})\s+(.+?)\s*#*\s*$/.exec(line);
    if(heading){const depth=heading[1].length;while(stack.length>1&&stack.at(-1)!.depth>=depth)stack.pop();const parent=stack.at(-1)!;const base=`section/${heading[2].normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}\s]/gu,'').trim().replace(/\s+/g,' ').slice(0,180)}`;const count=counts.get(base)??0;counts.set(base,count+1);const section={id:`${base}/${count}`,title:heading[2],body:'',children:[],depth};parent.children.push(section);stack.push(section);}else stack.at(-1)!.body+=`${line}\n`;
  }return root.body.trim()?[{...root,children:[]},...root.children]:root.children;
}
