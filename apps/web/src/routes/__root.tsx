import {
  Link,
  Outlet,
  createRootRoute,
  useRouter,
} from "@tanstack/react-router";
import { useState } from "react";
import { ApiError, getCurrentUser, logout, startGoogleLogin } from "../lib/api";
import { formatApiError } from "../lib/copy";

type RootSearch = { authError?: "AUTH_FAILED" };

export const Route = createRootRoute({
  validateSearch: (search: Record<string, unknown>): RootSearch => ({
    authError:
      search.authError === "AUTH_FAILED" ? "AUTH_FAILED" : undefined,
  }),
  loader: async () => {
    try {
      return await getCurrentUser();
    } catch (error) {
      if (error instanceof ApiError && error.code === "AUTH_REQUIRED") {
        return { user: null, isAdmin: false };
      }
      throw error;
    }
  },
  component: RootLayout,
  errorComponent: ({ error }) =>
    error instanceof ApiError && error.code === "AUTH_REQUIRED" ? (
      <SignInScreen />
    ) : (
      <main className="shell page">
        <section className="card error">
          <div>
            <h1>เกิดข้อผิดพลาด</h1>
            <p>
              {error instanceof ApiError
                ? formatApiError(error)
                : "โหลดหน้านี้ไม่สำเร็จ ลองใหม่อีกครั้ง"}
            </p>
            <Link to="/">กลับหน้าหลัก</Link>
          </div>
        </section>
      </main>
    ),
});

interface SignInScreenProps {
  authError?: boolean;
  eyebrow?: string;
  title?: string;
  description?: string;
}

export function SignInScreen({
  authError = false,
  eyebrow = "RUNS / ข้อมูลการวิ่ง",
  title = "เปิดข้อมูลวิ่งของคุณ",
  description = "เข้าสู่ระบบเพื่อเพิ่มข้อมูลวิ่งและกลับไปดูข้อมูลที่บันทึกไว้",
}: SignInScreenProps) {
  return (
    <main className="shell auth-shell">
      <div className="auth-gate">
        <div className="auth-context">
          <p className="auth-eyebrow">{eyebrow}</p>
          <h1>{title}</h1>
          <p>{description}</p>
        </div>
        <section className="card auth-card" aria-labelledby="auth-card-title">
          <div className="auth-card-brand">
            <img className="brand-mark" src="/favicon.svg" alt="" width="32" height="32" />
            <div>
              <p className="auth-brand-byline">by น้ำเน่ารันคลับ</p>
              <span className="auth-card-product">Runner’s Garage</span>
            </div>
          </div>
          <h2 id="auth-card-title">เข้าสู่ระบบเพื่อไปต่อ</h2>
          <p className="auth-card-description">ใช้ Google เพื่อเข้าถึงข้อมูล Runs ของคุณ</p>
          {authError ? (
            <p className="error" role="alert">
              <span>เข้าสู่ระบบด้วย Google ไม่สำเร็จ ลองใหม่อีกครั้ง</span>
            </p>
          ) : null}
          <div className="auth-actions">
            <button type="button" onClick={startGoogleLogin}>
              เข้าสู่ระบบด้วย Google
            </button>
            <Link className="button quiet" to="/">
              กลับหน้าหลัก
            </Link>
          </div>
        </section>
      </div>
    </main>
  );
}

function RootLayout() {
  const currentUser = Route.useLoaderData();
  const user = currentUser.user;
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  return (
    <div className="shell">
      <header className="site-header">
        <Link className="brand" to="/" aria-label="Runner’s Garage กลับหน้าหลัก">
          <img className="brand-mark" src="/favicon.svg" alt="" width="32" height="32" />
          <span className="brand-lockup">
            <span className="brand-text">Runner’s Garage</span>
            <span className="brand-byline">by น้ำเน่ารันคลับ</span>
          </span>
        </Link>
        <nav aria-label="เมนูหลัก">
          <Link
            to="/history"
            search={{ offset: 0, order: "desc" }}
            activeProps={{ "aria-current": "page" }}
          >
            Runs
          </Link>
          <Link to="/shoes" activeProps={{ "aria-current": "page" }}>
            Shoes
          </Link>
          <Link className="nav-add-run" to="/upload">
            เพิ่มข้อมูลวิ่ง
          </Link>
          {currentUser.isAdmin ? (
            <Link to="/admin/transcripts" activeProps={{ "aria-current": "page" }}>
              Admin queue
            </Link>
          ) : null}
        </nav>
        <div className="account-area">
          {user ? (
            <>
              <span className="account-name">{user.displayName ?? user.email}</span>
              <button
                className="quiet"
                type="button"
                disabled={busy}
                aria-busy={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    await logout();
                    await router.invalidate();
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                ออกจากระบบ
              </button>
            </>
          ) : (
            <button
              className="button secondary"
              type="button"
              onClick={startGoogleLogin}
            >
              เข้าสู่ระบบ
            </button>
          )}
        </div>
      </header>
      <main className="page">
        <Outlet />
      </main>
    </div>
  );
}
