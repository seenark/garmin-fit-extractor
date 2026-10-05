import type { ApiErrorDetail, CurrentUserResponse } from "./api-types";
export class ApiError extends Error { readonly status: number; readonly code: string; readonly fileName?: string; constructor(status:number, detail:ApiErrorDetail){ super(detail.message); this.name="ApiError"; this.status=status; this.code=detail.code; this.fileName=detail.fileName; } }
async function parseResponse<T>(response:Response):Promise<T>{ if(!response.ok){const body=await response.json().catch(()=>null) as {error?:ApiErrorDetail}|null; throw new ApiError(response.status,body?.error??{code:"REQUEST_FAILED",message:"The request could not be completed."});} return response.json() as Promise<T>; }
async function request<T>(path:string,init?:RequestInit):Promise<T>{ return parseResponse<T>(await fetch(path,{...init,credentials:"same-origin"})); }
export async function getCurrentUser():Promise<CurrentUserResponse>{ return request("/api/v1/auth/me"); }
export function startGoogleLogin():void{ window.location.assign("/api/v1/auth/login"); }
export async function logout():Promise<void>{ const response=await fetch("/api/v1/auth/logout",{method:"POST",credentials:"same-origin"}); if(!response.ok) await parseResponse<never>(response); }
