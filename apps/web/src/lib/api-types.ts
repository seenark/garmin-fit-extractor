export interface UserProfile { id: string; email: string; displayName: string | null; }
export interface CurrentUserResponse { user: UserProfile | null; isAdmin: boolean; }

export interface Metric { value: number | null; unit: string; }
export interface ApiErrorDetail { code: string; message: string; fileName?: string; }
