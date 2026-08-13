import type { ApiErrorDetail, BatchCreateResponse, CurrentUserResponse, ExtractionDetail, ExtractionPage } from "./api-types";
const API_BASE = "/api/v1/extractions";
export class ApiError extends Error { readonly status: number; readonly code: string; readonly fileName?: string; constructor(status:number, detail:ApiErrorDetail){ super(detail.message); this.name="ApiError"; this.status=status; this.code=detail.code; this.fileName=detail.fileName; } }
async function parseResponse<T>(response:Response):Promise<T>{ if(!response.ok){const body=await response.json().catch(()=>null) as {error?:ApiErrorDetail}|null; throw new ApiError(response.status,body?.error??{code:"REQUEST_FAILED",message:"The request could not be completed."});} return response.json() as Promise<T>; }
async function request<T>(path:string,init?:RequestInit):Promise<T>{ return parseResponse<T>(await fetch(path,{...init,credentials:"same-origin"})); }
export async function getCurrentUser():Promise<CurrentUserResponse>{ return request("/api/v1/auth/me"); }
export function startGoogleLogin():void{ window.location.assign("/api/v1/auth/login"); }
export async function logout():Promise<void>{ const response=await fetch("/api/v1/auth/logout",{method:"POST",credentials:"same-origin"}); if(!response.ok) await parseResponse<never>(response); }
export async function createExtractions(files:File[]):Promise<BatchCreateResponse>{const form=new FormData(); for(const file of files) form.append("files",file,file.name); return request(API_BASE,{method:"POST",body:form});}
export async function listExtractions(params:{limit?:number;offset?:number;order?:"asc"|"desc"}={}):Promise<ExtractionPage>{const limit=params.limit??50,offset=params.offset??0,order=params.order??"desc"; return request(`${API_BASE}?limit=${limit}&offset=${offset}&order=${order}`);}
export function getExtraction(id:string):Promise<ExtractionDetail>{return request(`${API_BASE}/${encodeURIComponent(id)}`);}
export async function deleteExtraction(id:string):Promise<void>{const r=await fetch(`${API_BASE}/${encodeURIComponent(id)}`,{method:"DELETE",credentials:"same-origin"});if(!r.ok)await parseResponse<never>(r);}
export async function clearExtractions():Promise<void>{const r=await fetch(API_BASE,{method:"DELETE",credentials:"same-origin"});if(!r.ok)await parseResponse<never>(r);}
export async function downloadExtraction(id:string,view:"normalized"|"raw"):Promise<Blob>{const r=await fetch(`${API_BASE}/${encodeURIComponent(id)}/download?view=${view}`,{credentials:"same-origin"});if(!r.ok)await parseResponse<never>(r);return r.blob();}
