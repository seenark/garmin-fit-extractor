import {
  Link,
  Outlet,
  createRootRoute,
  useRouter,
  useRouterState,
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
        return { user: null };
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

function SignInScreen({ authError = false }: { authError?: boolean }) {
  return (
    <main className="shell auth-shell">
      <section className="card auth-card">
        <span className="brand-mark" aria-hidden="true">FIT</span>
        <h1>Garmin FIT Extractor</h1>
        <p>เข้าสู่ระบบด้วย Google เพื่อไปต่อ</p>
        {authError ? (
          <p className="error" role="alert">
            <span>เข้าสู่ระบบด้วย Google ไม่สำเร็จ ลองใหม่อีกครั้ง</span>
          </p>
        ) : null}
        <button type="button" onClick={startGoogleLogin}>
          เข้าสู่ระบบด้วย Google
        </button>
      </section>
    </main>
  );
}

function RootLayout() {
  const user = Route.useLoaderData().user;
  const search = Route.useSearch();
  const router = useRouter();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const [busy, setBusy] = useState(false);
  const isPublicHome = pathname === "/" && search.authError !== "AUTH_FAILED";

  if (!user && !isPublicHome) {
    return <SignInScreen authError={search.authError === "AUTH_FAILED"} />;
  }

  return (
    <div className="shell">
      <header className="site-header">
        <Link className="brand" to="/">
          <span className="brand-mark" aria-hidden="true">FIT</span>
          <span className="brand-text">Garmin FIT Extractor</span>
        </Link>
        <nav aria-label="เมนูหลัก">
          <Link to="/" activeProps={{ "aria-current": "page" }}>
            หน้าหลัก
          </Link>
          <Link to="/upload" activeProps={{ "aria-current": "page" }}>
            อัปโหลด
          </Link>
          <Link
            to="/history"
            search={{ offset: 0, order: "desc" }}
            activeProps={{ "aria-current": "page" }}
          >
            ประวัติ
          </Link>
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
